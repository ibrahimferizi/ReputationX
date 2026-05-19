use crate::config::Config;
use crate::solana::helius;
use crate::solana::rpc::SolanaRpc;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::OnceLock;
use tokio::time::sleep;

const FUNDING_SIG_PAGE_SIZE: usize = 1000;
const MAX_OLDEST_TX_ATTEMPTS: usize = 8;

#[derive(Debug, Clone, Serialize)]
pub struct FundingSource {
    pub source_type: String,
    pub source_address: Option<String>,
    pub confidence: String,
}

impl FundingSource {
    pub fn unknown() -> Self {
        Self {
            source_type: "unknown".into(),
            source_address: None,
            confidence: "low".into(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct SignatureRow {
    signature: String,
    err: Option<Value>,
}

/// Labeled mainnet hot / deposit wallets (public explorer tags; not exhaustive).
fn cex_hot_wallets() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        HashSet::from([
            // Binance
            "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
            "5tzFkiKscXHK5ZXCGbXZxdw7gTjjD1mBwuoFbhUvuAi9",
            "2ojv9BAiExVTriQuerVomkRC9VoziQUjMSimDsmgc99o",
            // Coinbase
            "GJRs4FwHtemZ5ZE9XfVjc5aXqkpKThPjr8ub1QV6K9kp",
            "H8sMJSCQxfKiFTTdKrHS1rWMuanQqNejUpMXwJ8xGFFg",
            "3yFwqXBfZY4jBVUafQ1YEXw189y2dUG8467ssZpMr9o5",
            // Kraken
            "BZ36EGt83cE8fK3DM1kQ3nPtmC39m7Ur672JkXgq8gV9",
            "DfXygSm4jLyNCJTQmTKD4FYb1whzfYdnRABaoz96C747",
            "FWznbcNXWQuHTawe9RxvF6ZJXWqxYzfF7upfonqur8y",
        ])
    })
}

fn bridge_program_ids() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        HashSet::from([
            // Wormhole
            "worm2ZoG2kUd4vFXhvEH81HhmDs2YRb86kobaVb94zfd",
            "wormDTUJ6AWPNvkJCtbD26SJg3gm6K1WmXvmK8m5",
            "WormT3McKhFJ2RkiGpdw9GKvNCrB2aB54gb2uV9MfQC",
            "WnFt12ZrnzZrFZkt2xsNsaNWoQribnuQ5B5FrDbwDhD",
            // Allbridge
            "BrdgN2RPzEMWF96ZbnnJaUtQDQx7VRXYaHHbYCBvceWB",
            "BBbD1WSjbHKfyE3TSFWF6vx1JV51c8msKSQy4ess6pXp",
        ])
    })
}

/// Trace the first SOL inflow to `address` via its oldest on-chain activity.
pub async fn get_funding_source(
    address: &str,
    rpc_client: &SolanaRpc,
    config: &Config,
) -> FundingSource {
    match trace_funding_source(address, rpc_client, config).await {
        Ok(source) => source,
        Err(err) => {
            tracing::debug!(error = %err, %address, "funding source trace failed");
            FundingSource::unknown()
        }
    }
}

async fn trace_funding_source(
    address: &str,
    rpc: &SolanaRpc,
    config: &Config,
) -> Result<FundingSource> {
    // Prefer Helius oldest-tx lookup (1 RPC) over paging getSignaturesForAddress.
    if let Ok(signatures) = helius::try_gtfa_oldest_signatures(config, rpc, address).await {
        for signature in signatures {
            if let Some(source) = funding_from_signature(rpc, address, &signature).await? {
                return Ok(source);
            }
        }
    }

    let oldest_batch = fetch_oldest_signature_batch(
        rpc,
        address,
        config.funding_max_signature_pages,
        config.funding_page_delay_ms,
    )
    .await?;
    let Some(oldest_batch) = oldest_batch else {
        tracing::warn!(%address, "funding trace: no signatures found");
        return Ok(FundingSource::unknown());
    };

    // Oldest signatures are at the end of the final page (RPC returns newest-first per page).
    let candidates: Vec<&SignatureRow> = oldest_batch
        .iter()
        .rev()
        .filter(|row| row.err.is_none())
        .take(MAX_OLDEST_TX_ATTEMPTS)
        .collect();
    let candidate_count = candidates.len();

    for row in candidates {
        if let Some(source) = funding_from_signature(rpc, address, &row.signature).await? {
            return Ok(source);
        }
    }

    tracing::warn!(
        %address,
        candidates = candidate_count,
        "funding trace: could not resolve funder from oldest transactions"
    );
    Ok(FundingSource::unknown())
}

async fn funding_from_signature(
    rpc: &SolanaRpc,
    address: &str,
    signature: &str,
) -> Result<Option<FundingSource>> {
    let tx = fetch_parsed_transaction(rpc, signature).await?;
    let Some(tx) = tx else {
        return Ok(None);
    };
    let sender = extract_first_sol_inflow_sender(&tx, address)
        .or_else(|| extract_funding_from_balance_changes(&tx, address));
    Ok(sender.map(|s| classify_funding(&s, &tx)))
}

/// Walk `getSignaturesForAddress` pages until genesis; return the final (oldest) page.
async fn fetch_oldest_signature_batch(
    rpc: &SolanaRpc,
    address: &str,
    max_pages: usize,
    page_delay_ms: u64,
) -> Result<Option<Vec<SignatureRow>>> {
    let mut before: Option<String> = None;
    let mut pages = 0usize;
    let mut last_batch: Vec<SignatureRow> = Vec::new();

    loop {
        if pages >= max_pages {
            break;
        }

        if pages > 0 && page_delay_ms > 0 {
            sleep(std::time::Duration::from_millis(page_delay_ms)).await;
        }

        let batch: Vec<SignatureRow> = rpc
            .call(
                "getSignaturesForAddress",
                signature_page_params(address, FUNDING_SIG_PAGE_SIZE, before.as_deref()),
            )
            .await
            .context("getSignaturesForAddress")?;

        if batch.is_empty() {
            break;
        }

        before = batch.last().map(|row| row.signature.clone());
        last_batch = batch;
        pages += 1;

        if last_batch.len() < FUNDING_SIG_PAGE_SIZE {
            break;
        }
    }

    if last_batch.is_empty() {
        Ok(None)
    } else {
        Ok(Some(last_batch))
    }
}

fn signature_page_params(address: &str, limit: usize, before: Option<&str>) -> Value {
    let mut opts = serde_json::Map::new();
    opts.insert("limit".into(), json!(limit));
    if let Some(cursor) = before {
        opts.insert("before".into(), json!(cursor));
    }
    json!([address, Value::Object(opts)])
}

async fn fetch_parsed_transaction(rpc: &SolanaRpc, signature: &str) -> Result<Option<Value>> {
    let params = json!([
        signature,
        {
            "encoding": "jsonParsed",
            "maxSupportedTransactionVersion": 0
        }
    ]);
    let tx: Option<Value> = rpc.call("getTransaction", params).await?;
    Ok(tx)
}

/// First system transfer or createAccount funding `destination` in instruction order.
fn extract_first_sol_inflow_sender(tx: &Value, destination: &str) -> Option<String> {
    let mut earliest: Option<(usize, String)> = None;

    if let Some(message) = tx
        .get("transaction")
        .and_then(|t| t.get("message"))
    {
        scan_instructions(message.get("instructions"), destination, &mut earliest, 0);
    }

    if let Some(inner) = tx.get("meta").and_then(|m| m.get("innerInstructions")) {
        if let Some(groups) = inner.as_array() {
            for (group_idx, group) in groups.iter().enumerate() {
                let base = 10_000 + group_idx * 100;
                if let Some(ixs) = group.get("instructions") {
                    scan_instructions(Some(ixs), destination, &mut earliest, base);
                }
            }
        }
    }

    earliest.map(|(_, src)| src)
}

fn scan_instructions(
    instructions: Option<&Value>,
    destination: &str,
    earliest: &mut Option<(usize, String)>,
    index_offset: usize,
) {
    let Some(ixs) = instructions.and_then(|v| v.as_array()) else {
        return;
    };

    for (idx, ix) in ixs.iter().enumerate() {
        let order = index_offset + idx;
        if let Some(source) = inflow_source_from_instruction(ix, destination) {
            match earliest {
                Some((best, _)) if order >= *best => {}
                _ => *earliest = Some((order, source)),
            }
        }
    }
}

fn inflow_source_from_instruction(ix: &Value, destination: &str) -> Option<String> {
    let parsed = ix.get("parsed")?;
    let info = parsed.get("info")?;
    let kind = parsed.get("type").and_then(|t| t.as_str())?;

    match kind {
        "transfer" if info.get("destination").and_then(|d| d.as_str()) == Some(destination) => {
            info.get("source").and_then(|s| s.as_str()).map(str::to_string)
        }
        "transferWithSeed"
            if info.get("toPubkey").and_then(|d| d.as_str()) == Some(destination) =>
        {
            info.get("fromPubkey")
                .and_then(|s| s.as_str())
                .map(str::to_string)
        }
        "createAccount" if info.get("newAccount").and_then(|d| d.as_str()) == Some(destination) => {
            info.get("source").and_then(|s| s.as_str()).map(str::to_string)
        }
        "createAccountWithSeed"
            if info.get("newAccount").and_then(|d| d.as_str()) == Some(destination) =>
        {
            info.get("source").and_then(|s| s.as_str()).map(str::to_string)
        }
        _ => None,
    }
}

/// Fallback when parsed instructions omit the funder (common on v0 txs and inner transfers).
fn extract_funding_from_balance_changes(tx: &Value, destination: &str) -> Option<String> {
    let meta = tx.get("meta")?;
    let pre = meta.get("preBalances")?.as_array()?;
    let post = meta.get("postBalances")?.as_array()?;
    if pre.len() != post.len() || pre.is_empty() {
        return None;
    }

    let message = tx.get("transaction")?.get("message")?;
    let keys = resolve_account_keys(message, meta);
    if keys.len() != pre.len() {
        return None;
    }

    let dest_idx = keys.iter().position(|k| k == destination)?;
    let pre_amt = pre[dest_idx].as_u64()?;
    let post_amt = post[dest_idx].as_u64()?;
    if post_amt <= pre_amt {
        return None;
    }
    let gained = post_amt - pre_amt;
    if gained < 5_000 {
        return None;
    }

    let mut best_sender: Option<(u64, usize)> = None;
    for (i, (pre_b, post_b)) in pre.iter().zip(post.iter()).enumerate() {
        if i == dest_idx {
            continue;
        }
        let pre_v = pre_b.as_u64()?;
        let post_v = post_b.as_u64()?;
        if pre_v > post_v {
            let loss = pre_v - post_v;
            match best_sender {
                Some((best_loss, _)) if loss <= best_loss => {}
                _ => best_sender = Some((loss, i)),
            }
        }
    }

    let (loss, idx) = best_sender?;
    if loss < gained / 2 {
        return None;
    }

    let sender = keys.get(idx)?.as_str();
    if is_non_funder_account(sender) {
        return None;
    }
    Some(sender.to_string())
}

fn resolve_account_keys(message: &Value, meta: &Value) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(arr) = message.get("accountKeys").and_then(|v| v.as_array()) {
        for entry in arr {
            if let Some(s) = entry.as_str() {
                keys.push(s.to_string());
            } else if let Some(pk) = entry.get("pubkey").and_then(|v| v.as_str()) {
                keys.push(pk.to_string());
            }
        }
    }
    if let Some(loaded) = meta.get("loadedAddresses") {
        if let Some(writable) = loaded.get("writable").and_then(|v| v.as_array()) {
            for key in writable {
                if let Some(s) = key.as_str() {
                    keys.push(s.to_string());
                }
            }
        }
        if let Some(readonly) = loaded.get("readonly").and_then(|v| v.as_array()) {
            for key in readonly {
                if let Some(s) = key.as_str() {
                    keys.push(s.to_string());
                }
            }
        }
    }
    keys
}

fn is_non_funder_account(addr: &str) -> bool {
    matches!(
        addr,
        "11111111111111111111111111111111"
            | "Stake11111111111111111111111111111111111111"
            | "Vote111111111111111111111111111111111111111"
            | "ComputeBudget111111111111111111111111111111"
            | "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr"
            | "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
            | "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
    )
}

fn classify_funding(sender: &str, tx: &Value) -> FundingSource {
    if cex_hot_wallets().contains(sender) {
        return FundingSource {
            source_type: "cex".into(),
            source_address: Some(sender.to_string()),
            confidence: "high".into(),
        };
    }

    if transaction_involves_bridge(tx) || bridge_program_ids().contains(sender) {
        return FundingSource {
            source_type: "bridge".into(),
            source_address: Some(sender.to_string()),
            confidence: "medium".into(),
        };
    }

    FundingSource {
        source_type: "wallet".into(),
        source_address: Some(sender.to_string()),
        confidence: "medium".into(),
    }
}

fn transaction_involves_bridge(tx: &Value) -> bool {
    collect_program_ids(tx)
        .iter()
        .any(|id| bridge_program_ids().contains(id.as_str()))
}

fn collect_program_ids(tx: &Value) -> Vec<String> {
    let mut ids = Vec::new();

    if let Some(message) = tx
        .get("transaction")
        .and_then(|t| t.get("message"))
    {
        push_program_ids(message.get("instructions"), &mut ids);
    }

    if let Some(inner) = tx.get("meta").and_then(|m| m.get("innerInstructions")) {
        if let Some(groups) = inner.as_array() {
            for group in groups {
                push_program_ids(group.get("instructions"), &mut ids);
            }
        }
    }

    ids.sort_unstable();
    ids.dedup();
    ids
}

fn push_program_ids(instructions: Option<&Value>, out: &mut Vec<String>) {
    let Some(ixs) = instructions.and_then(|v| v.as_array()) else {
        return;
    };
    for ix in ixs {
        if let Some(pid) = ix
            .get("programId")
            .and_then(|v| v.as_str())
            .map(str::to_string)
        {
            out.push(pid);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_cex_sender() {
        let tx = json!({});
        let src = classify_funding("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", &tx);
        assert_eq!(src.source_type, "cex");
        assert_eq!(src.confidence, "high");
    }

    #[test]
    fn classifies_bridge_tx() {
        let tx = json!({
            "transaction": {
                "message": {
                    "instructions": [{
                        "programId": "wormDTUJ6AWPNvkJCtbD26SJg3gm6K1WmXvmK8m5"
                    }]
                }
            }
        });
        let src = classify_funding("SomeWallet1111111111111111111111111111111", &tx);
        assert_eq!(src.source_type, "bridge");
    }

    #[test]
    fn extracts_funding_from_balance_changes() {
        let tx = json!({
            "transaction": {
                "message": {
                    "accountKeys": [
                        "Funder1111111111111111111111111111111111111",
                        "Target1111111111111111111111111111111111"
                    ]
                }
            },
            "meta": {
                "preBalances": [2_000_000_000, 0],
                "postBalances": [1_000_000_000, 1_000_000_000]
            }
        });
        let sender = extract_funding_from_balance_changes(
            &tx,
            "Target1111111111111111111111111111111111",
        );
        assert_eq!(
            sender.as_deref(),
            Some("Funder1111111111111111111111111111111111111")
        );
    }

    #[test]
    fn extracts_transfer_inflow() {
        let tx = json!({
            "transaction": {
                "message": {
                    "instructions": [{
                        "program": "system",
                        "parsed": {
                            "type": "transfer",
                            "info": {
                                "source": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
                                "destination": "Target1111111111111111111111111111111111",
                                "lamports": 1000000
                            }
                        }
                    }]
                }
            }
        });
        let sender = extract_first_sol_inflow_sender(
            &tx,
            "Target1111111111111111111111111111111111",
        );
        assert_eq!(
            sender.as_deref(),
            Some("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM")
        );
    }
}
