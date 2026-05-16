use crate::config::{Config, ScanMode};
use crate::solana::helius;
use crate::solana::rpc::SolanaRpc;
use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use tokio::time::sleep;

const SECONDS_PER_DAY: f64 = 86_400.0;

#[derive(Debug, Clone)]
pub struct TransactionHistory {
    pub timestamps_sec: Vec<i64>,
    pub total_count: u64,
    pub count_capped: bool,
    pub wallet_age_days: f64,
    pub first_activity_unix: Option<i64>,
    pub max_txs_per_hour: u32,
    pub max_txs_first_day: u32,
    pub mean_interval_hours: f64,
    pub interval_cv: f64,
    pub scan_source: String,
}

#[derive(Debug, Deserialize)]
struct SignatureInfo {
    signature: String,
    blockTime: Option<i64>,
    err: Option<Value>,
}

pub async fn get_transactions(
    rpc: &SolanaRpc,
    http: &Client,
    config: &Config,
    address: &str,
) -> Result<TransactionHistory> {
    // Prefer Helius REST in fast/balanced mode — single HTTP call, avoids signature RPC 429s.
    if let Some(ref api_key) = config.helius_api_key {
        if config.scan_mode != ScanMode::Deep {
            if let Ok(timestamps) =
                helius::fetch_recent_timestamps(http, api_key, address, config.helius_tx_limit).await
            {
                if !timestamps.is_empty() {
                    let count = timestamps.len() as u64;
                    return Ok(build_history(timestamps, count, true, "helius"));
                }
            }
            tracing::warn!("helius tx fetch failed or empty, falling back to rpc signatures");
        }
    }

    fetch_via_rpc_signatures(rpc, config, address).await
}

async fn fetch_via_rpc_signatures(
    rpc: &SolanaRpc,
    config: &Config,
    address: &str,
) -> Result<TransactionHistory> {
    let mut all_signatures: Vec<SignatureInfo> = Vec::new();
    let mut before: Option<String> = None;
    let mut pages = 0usize;

    loop {
        if pages > 0 && config.signature_page_delay_ms > 0 {
            sleep(std::time::Duration::from_millis(config.signature_page_delay_ms)).await;
        }

        let batch: Vec<SignatureInfo> = rpc
            .call(
                "getSignaturesForAddress",
                signature_params(address, config.signature_page_size, before.as_deref()),
            )
            .await?;

        if batch.is_empty() {
            break;
        }

        before = batch.last().map(|s| s.signature.clone());
        all_signatures.extend(batch);
        pages += 1;

        if pages >= config.max_signature_pages {
            break;
        }
    }

    let count_capped = pages >= config.max_signature_pages
        || (pages > 0 && all_signatures.len() >= config.signature_page_size);

    let timestamps_sec: Vec<i64> = all_signatures
        .iter()
        .filter(|s| s.err.is_none())
        .filter_map(|s| s.blockTime)
        .collect();

    Ok(build_history(
        timestamps_sec,
        all_signatures.len() as u64,
        count_capped,
        "rpc_signatures",
    ))
}

fn build_history(
    mut timestamps_sec: Vec<i64>,
    total_count: u64,
    count_capped: bool,
    scan_source: &str,
) -> TransactionHistory {
    timestamps_sec.sort_unstable();

    let (wallet_age_days, first_activity_unix) = if let Some(&first) = timestamps_sec.first() {
        let now = chrono::Utc::now().timestamp();
        (
            ((now - first) as f64 / SECONDS_PER_DAY).max(0.0),
            Some(first),
        )
    } else {
        (0.0, None)
    };

    let max_txs_per_hour = max_in_window(&timestamps_sec, 3600);
    let max_txs_first_day = if let Some(first) = timestamps_sec.first() {
        timestamps_sec
            .iter()
            .filter(|ts| **ts <= first + 86_400)
            .count() as u32
    } else {
        0
    };

    let intervals = interval_hours(&timestamps_sec);
    let mean_interval_hours = if intervals.is_empty() {
        0.0
    } else {
        intervals.iter().sum::<f64>() / intervals.len() as f64
    };
    let interval_cv = coefficient_of_variation(&intervals);

    TransactionHistory {
        timestamps_sec,
        total_count,
        count_capped,
        wallet_age_days,
        first_activity_unix,
        max_txs_per_hour,
        max_txs_first_day,
        mean_interval_hours,
        interval_cv,
        scan_source: scan_source.to_string(),
    }
}

fn signature_params(address: &str, limit: usize, before: Option<&str>) -> Value {
    let mut opts = Map::new();
    opts.insert("limit".into(), json!(limit));
    if let Some(cursor) = before {
        opts.insert("before".into(), json!(cursor));
    }
    json!([address, Value::Object(opts)])
}

fn interval_hours(timestamps: &[i64]) -> Vec<f64> {
    let mut out = Vec::new();
    for pair in timestamps.windows(2) {
        out.push((pair[1] - pair[0]) as f64 / 3600.0);
    }
    out
}

fn coefficient_of_variation(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    if mean == 0.0 {
        return 0.0;
    }
    let variance =
        values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    variance.sqrt() / mean
}

fn max_in_window(timestamps: &[i64], window_sec: i64) -> u32 {
    if timestamps.is_empty() {
        return 0;
    }
    let mut max_count = 0u32;
    let mut left = 0usize;
    for right in 0..timestamps.len() {
        while timestamps[right] - timestamps[left] > window_sec {
            left += 1;
        }
        max_count = max_count.max((right - left + 1) as u32);
    }
    max_count
}
