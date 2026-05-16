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
pub async fn fetch_recent_timestamps(
    client: &Client,
    api_key: &str,
    address: &str,
    limit: u32,
) -> Result<Vec<i64>> {
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

    let mut timestamps: Vec<i64> = txs
        .iter()
        .filter_map(|tx| {
            tx.get("timestamp")
                .and_then(|v| v.as_i64())
                .or_else(|| tx.get("blockTime").and_then(|v| v.as_i64()))
                .map(normalize_unix_timestamp)
        })
        .collect();

    timestamps.sort_unstable();
    Ok(timestamps)
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
        let failed = entry
            .get("err")
            .map(|e| !e.is_null())
            .unwrap_or(false);
        if failed {
            continue;
        }
        if let Some(bt) = entry.get("blockTime").and_then(|v| v.as_i64()) {
            return Some(normalize_unix_timestamp(bt));
        }
    }
    None
}

fn extract_oldest_signature(result: Value) -> Option<String> {
    let entries = gtfa_entries(&result)?;
    for entry in entries {
        let failed = entry
            .get("err")
            .map(|e| !e.is_null())
            .unwrap_or(false);
        if failed {
            continue;
        }
        if let Some(sig) = entry.get("signature").and_then(|v| v.as_str()) {
            return Some(sig.to_string());
        }
    }
    None
}

async fn fetch_oldest_signature_gtfa(history_rpc: &SolanaRpc, address: &str) -> Result<Option<String>> {
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
        if let Some(sig) = extract_oldest_signature(result) {
            return Ok(Some(sig));
        }
    }

    Ok(None)
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

/// Oldest successful signature via Helius GTFA (one RPC call).
pub async fn try_gtfa_oldest_signature(
    config: &Config,
    history_rpc: &SolanaRpc,
    address: &str,
) -> Result<Option<String>> {
    let gtfa_url = match gtfa_rpc_url(config) {
        Some(u) => u,
        None => return Ok(None),
    };

    if gtfa_url == config.solana_history_rpc_url {
        return fetch_oldest_signature_gtfa(history_rpc, address).await;
    }

    tracing::debug!(
        url = %redact_api_key(&gtfa_url),
        "Helius GTFA for oldest signature (funding trace)"
    );
    fetch_oldest_signature_gtfa(&history_rpc.with_url(gtfa_url), address).await
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

    tracing::info!(
        url = %redact_api_key(&gtfa_url),
        "using Helius RPC for wallet age (getTransactionsForAddress)"
    );
    fetch_first_activity_timestamp(&history_rpc.with_url(gtfa_url), address).await
}

fn redact_api_key(url: &str) -> String {
    if let Some(start) = url.find("api-key=") {
        let mut s = url.to_string();
        if let Some(end) = s[start + 8..].find('&') {
            s.replace_range(start + 8..start + 8 + end, "***");
        } else {
            s.truncate(start + 8);
            s.push_str("***");
        }
        return s;
    }
    url.to_string()
}
