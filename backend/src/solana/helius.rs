use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{json, Value};

use crate::config::Config;
use crate::solana::rpc::SolanaRpc;

const UNIX_SEC_UPPER_BOUND: i64 = 10_000_000_000;

fn normalize_unix_timestamp(ts: i64) -> i64 {
    if ts > UNIX_SEC_UPPER_BOUND {
        ts / 1000
    } else {
        ts
    }
}

/// One Helius REST call — much faster than paging `getSignaturesForAddress` on rate-limited RPCs.
#[derive(Debug)]
pub struct RecentTransactions {
    pub timestamps: Vec<i64>,
    pub successful_count: u64,
    pub fetched_count: usize,
}

pub async fn fetch_recent_timestamps(
    client: &Client,
    api_key: &str,
    address: &str,
    limit: u32,
) -> Result<RecentTransactions> {
    let url = format!("https://api.helius.xyz/v0/addresses/{address}/transactions");

    let response = client
        .get(&url)
        .query(&[("api-key", api_key), ("limit", &limit.to_string())])
        .send()
        .await
        .context("helius transactions request failed")?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "helius http {status}: {}",
            body.chars().take(200).collect::<String>()
        );
    }

    let txs: Vec<Value> = response.json().await.context("helius invalid json")?;

    Ok(parse_recent_transactions(&txs))
}

fn parse_recent_transactions(txs: &[Value]) -> RecentTransactions {
    let successful: Vec<_> = txs
        .iter()
        .filter(|tx| {
            tx.get("transactionError").is_none_or(Value::is_null)
                && tx.get("err").is_none_or(Value::is_null)
        })
        .collect();
    let mut timestamps: Vec<i64> = successful
        .iter()
        .filter_map(|tx| {
            tx.get("timestamp")
                .and_then(|v| v.as_i64())
                .or_else(|| tx.get("blockTime").and_then(|v| v.as_i64()))
                .map(normalize_unix_timestamp)
        })
        .collect();

    timestamps.sort_unstable();
    RecentTransactions {
        timestamps,
        successful_count: successful.len() as u64,
        fetched_count: txs.len(),
    }
}

pub fn history_rpc_supports_gtfa(history_rpc_url: &str) -> bool {
    let url = history_rpc_url.to_ascii_lowercase();
    url.contains("helius-rpc.com") || url.contains("helius.xyz")
}

/// Append `api-key` query param when missing (Helius RPC requires it for GTFA).
pub fn ensure_helius_api_key(url: &str, api_key: Option<&str>) -> String {
    let url = url.trim().trim_matches('"');
    if url.contains("api-key=") || url.contains("api_key=") {
        return url.to_string();
    }
    let Some(key) = api_key.filter(|k| !k.is_empty()) else {
        return url.to_string();
    };
    if url.contains('?') {
        format!("{url}&api-key={key}")
    } else {
        format!("{url}?api-key={key}")
    }
}

/// Best RPC URL for `getTransactionsForAddress` (oldest tx, one call).
pub fn gtfa_rpc_url(config: &Config) -> Option<String> {
    let key = config.helius_api_key.as_deref();
    let history = ensure_helius_api_key(&config.solana_history_rpc_url, key);
    if history_rpc_supports_gtfa(&history) {
        return Some(history);
    }
    key.map(|k| format!("https://mainnet.helius-rpc.com/?api-key={k}"))
}

fn gtfa_entries(result: &Value) -> Option<&Vec<Value>> {
    result
        .get("data")
        .and_then(|d| d.as_array())
        .or_else(|| result.as_array())
}

fn extract_oldest_block_time(result: Value) -> Option<i64> {
    let entries = gtfa_entries(&result)?;
    for entry in entries {
        let failed = entry.get("err").map(|e| !e.is_null()).unwrap_or(false);
        if failed {
            continue;
        }
        if let Some(bt) = entry.get("blockTime").and_then(|v| v.as_i64()) {
            return Some(normalize_unix_timestamp(bt));
        }
    }
    None
}

fn extract_oldest_signatures(result: Value, limit: usize) -> Vec<String> {
    let Some(entries) = gtfa_entries(&result) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries {
        if out.len() >= limit {
            break;
        }
        let failed = entry.get("err").map(|e| !e.is_null()).unwrap_or(false);
        if failed {
            continue;
        }
        if let Some(sig) = entry.get("signature").and_then(|v| v.as_str()) {
            out.push(sig.to_string());
        }
    }
    out
}

async fn fetch_oldest_signatures_gtfa(
    history_rpc: &SolanaRpc,
    address: &str,
    limit: u32,
) -> Result<Vec<String>> {
    let attempts = [
        json!({
            "transactionDetails": "signatures",
            "sortOrder": "asc",
            "limit": limit
        }),
        json!({
            "transactionDetails": "signatures",
            "sortOrder": "asc",
            "limit": limit,
            "filters": { "status": "any" }
        }),
    ];

    for opts in attempts {
        let params = json!([address, opts]);
        let result: Value = history_rpc
            .call("getTransactionsForAddress", params)
            .await?;
        let sigs = extract_oldest_signatures(result, limit as usize);
        if !sigs.is_empty() {
            return Ok(sigs);
        }
    }

    Ok(Vec::new())
}

/// Oldest tx via Helius `getTransactionsForAddress` (sort asc, limit 1).
pub async fn fetch_first_activity_timestamp(
    history_rpc: &SolanaRpc,
    address: &str,
) -> Result<Option<i64>> {
    let attempts = [
        json!({
            "transactionDetails": "signatures",
            "sortOrder": "asc",
            "limit": 1
        }),
        json!({
            "transactionDetails": "signatures",
            "sortOrder": "asc",
            "limit": 1,
            "filters": { "status": "any" }
        }),
    ];

    for opts in attempts {
        let params = json!([address, opts]);
        let result: Value = history_rpc
            .call("getTransactionsForAddress", params)
            .await?;
        if let Some(ts) = extract_oldest_block_time(result) {
            return Ok(Some(ts));
        }
    }

    Ok(None)
}

const GTFA_FUNDING_SIG_LIMIT: u32 = 8;

/// Oldest successful signatures via Helius GTFA (ascending, one RPC call).
pub async fn try_gtfa_oldest_signatures(
    config: &Config,
    history_rpc: &SolanaRpc,
    address: &str,
) -> Result<Vec<String>> {
    let gtfa_url = match gtfa_rpc_url(config) {
        Some(u) => u,
        None => return Ok(Vec::new()),
    };

    if gtfa_url == config.solana_history_rpc_url {
        return fetch_oldest_signatures_gtfa(history_rpc, address, GTFA_FUNDING_SIG_LIMIT).await;
    }

    tracing::debug!("Helius GTFA for oldest signatures (funding trace)");
    fetch_oldest_signatures_gtfa(
        &history_rpc.with_url(gtfa_url),
        address,
        GTFA_FUNDING_SIG_LIMIT,
    )
    .await
}

/// Try GTFA on the best Helius endpoint (may differ from configured history RPC).
pub async fn try_gtfa_first_activity(
    config: &Config,
    history_rpc: &SolanaRpc,
    address: &str,
) -> Result<Option<i64>> {
    let gtfa_url = match gtfa_rpc_url(config) {
        Some(u) => u,
        None => return Ok(None),
    };

    if gtfa_url == config.solana_history_rpc_url {
        return fetch_first_activity_timestamp(history_rpc, address).await;
    }

    tracing::info!("using Helius RPC for wallet age (getTransactionsForAddress)");
    fetch_first_activity_timestamp(&history_rpc.with_url(gtfa_url), address).await
}

#[cfg(test)]
mod recent_tests {
    use super::*;

    #[test]
    fn counts_successes_separately_from_timestamps_and_page_size() {
        let recent = parse_recent_transactions(&[
            json!({"timestamp": 1_700_000_000, "transactionError": null}),
            json!({"timestamp": 1_700_000_001, "transactionError": "failed"}),
            json!({"timestamp": 1_700_000_002, "err": {"InstructionError": [0, "failed"]}}),
            json!({"transactionError": null}),
        ]);
        assert_eq!(recent.fetched_count, 4);
        assert_eq!(recent.successful_count, 2);
        assert_eq!(recent.timestamps, vec![1_700_000_000]);
    }
}
