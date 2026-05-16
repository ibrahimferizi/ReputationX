use crate::config::{Config, ScanMode};
use crate::solana::helius;
use crate::solana::rpc::SolanaRpc;
use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use tokio::time::sleep;

const SECONDS_PER_DAY: f64 = 86_400.0;
/// Timestamps above this are treated as milliseconds (Solana/Helius edge cases).
const UNIX_SEC_UPPER_BOUND: i64 = 10_000_000_000;

#[derive(Debug, Clone)]
pub struct TransactionHistory {
    pub timestamps_sec: Vec<i64>,
    pub total_count: u64,
    pub count_capped: bool,
    pub age_capped: bool,
    pub wallet_age_days: f64,
    pub first_activity_unix: Option<i64>,
    pub max_txs_per_hour: u32,
    pub max_txs_first_day: u32,
    pub mean_interval_hours: f64,
    pub interval_cv: f64,
    pub scan_source: String,
    /// How first activity was resolved: helius_gtfa | pagination | unknown
    pub age_source: String,
}

#[derive(Debug, Deserialize)]
struct SignatureInfo {
    signature: String,
    blockTime: Option<i64>,
    err: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct TxResult {
    #[serde(rename = "blockTime")]
    block_time: Option<i64>,
}

pub async fn get_transactions(
    _rpc: &SolanaRpc,
    history_rpc: &SolanaRpc,
    http: &Client,
    config: &Config,
    address: &str,
) -> Result<TransactionHistory> {
    if let Some(ref api_key) = config.helius_api_key {
        if config.scan_mode != ScanMode::Deep {
            if let Ok(timestamps) =
                helius::fetch_recent_timestamps(http, api_key, address, config.helius_tx_limit).await
            {
                if !timestamps.is_empty() {
                    let (first_activity_unix, age_capped, age_source) =
                        fetch_first_activity_unix(history_rpc, config, address).await?;
                    let count = timestamps.len() as u64;
                    return Ok(build_history(
                        timestamps,
                        count,
                        false,
                        age_capped,
                        first_activity_unix,
                        "helius",
                        age_source,
                    ));
                }
            }
            tracing::warn!("helius tx fetch failed or empty, falling back to rpc signatures");
        }
    }

    fetch_via_rpc_signatures(history_rpc, config, address).await
}

async fn fetch_via_rpc_signatures(
    rpc: &SolanaRpc,
    config: &Config,
    address: &str,
) -> Result<TransactionHistory> {
    let (first_activity_unix, age_capped, age_source) =
        fetch_first_activity_unix(rpc, config, address).await?;

    let mut all_signatures: Vec<SignatureInfo> = Vec::new();
    let mut before: Option<String> = None;
    let mut pages = 0usize;
    let mut hit_page_limit = false;

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

        let last_batch_len = batch.len();
        all_signatures.extend(batch);
        pages += 1;

        if pages >= config.max_signature_pages {
            // Full last page ⇒ there may be older signatures we did not fetch.
            hit_page_limit = last_batch_len >= config.signature_page_size;
            break;
        }
    }

    let count_capped = hit_page_limit;

    let timestamps_sec: Vec<i64> = all_signatures
        .iter()
        .filter(|s| s.err.is_none())
        .filter_map(|s| s.blockTime.map(normalize_unix_timestamp))
        .collect();

    Ok(build_history(
        timestamps_sec,
        all_signatures.len() as u64,
        count_capped,
        age_capped,
        first_activity_unix,
        "rpc_signatures",
        age_source,
    ))
}

/// Paginate signatures until genesis or page cap; returns oldest known activity timestamp.
pub async fn fetch_first_activity_unix(
    rpc: &SolanaRpc,
    config: &Config,
    address: &str,
) -> Result<(Option<i64>, bool, String)> {
    match helius::try_gtfa_first_activity(config, rpc, address).await {
        Ok(Some(ts)) => return Ok((Some(ts), false, "helius_gtfa".into())),
        Ok(None) => tracing::debug!("getTransactionsForAddress asc returned no txs"),
        Err(err) => {
            tracing::warn!(
                error = %err,
                "getTransactionsForAddress failed; falling back to signature pagination"
            );
        }
    }

    let max_age_pages = config.max_age_signature_pages;

    let mut before: Option<String> = None;
    let mut pages = 0usize;
    let mut oldest: Option<i64> = None;
    let mut fallback_sig: Option<String> = None;

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
        if let Some(last) = batch.last() {
            if last.err.is_none() {
                fallback_sig = Some(last.signature.clone());
            }
        }

        for sig in &batch {
            if sig.err.is_some() {
                continue;
            }
            if let Some(bt) = sig.blockTime {
                let t = normalize_unix_timestamp(bt);
                oldest = Some(oldest.map(|o| o.min(t)).unwrap_or(t));
            }
        }

        let last_batch_len = batch.len();
        pages += 1;
        if pages >= max_age_pages {
            let age_capped = last_batch_len >= config.signature_page_size;
            if oldest.is_none() {
                if let Some(sig) = fallback_sig {
                    oldest = block_time_for_signature(rpc, &sig).await?;
                }
            }
            return Ok((oldest, age_capped, "pagination".into()));
        }
    }

    let age_capped = false;

    if oldest.is_none() {
        if let Some(sig) = fallback_sig {
            oldest = block_time_for_signature(rpc, &sig).await?;
        }
    }

    Ok((
        oldest,
        age_capped,
        if oldest.is_some() {
            "pagination"
        } else {
            "unknown"
        }
        .into(),
    ))
}

async fn block_time_for_signature(rpc: &SolanaRpc, signature: &str) -> Result<Option<i64>> {
    let params = json!([
        signature,
        {
            "encoding": "jsonParsed",
            "maxSupportedTransactionVersion": 0
        }
    ]);

    let tx: Option<TxResult> = rpc.call("getTransaction", params).await?;
    Ok(tx.and_then(|t| t.block_time.map(normalize_unix_timestamp)))
}

fn build_history(
    mut timestamps_sec: Vec<i64>,
    total_count: u64,
    count_capped: bool,
    age_capped: bool,
    first_activity_unix: Option<i64>,
    scan_source: &str,
    age_source: String,
) -> TransactionHistory {
    timestamps_sec.sort_unstable();

    let (wallet_age_days, first_activity_unix) =
        wallet_age_from_first_activity(first_activity_unix);

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
        age_capped,
        wallet_age_days,
        first_activity_unix,
        max_txs_per_hour,
        max_txs_first_day,
        mean_interval_hours,
        interval_cv,
        scan_source: scan_source.to_string(),
        age_source,
    }
}

pub fn normalize_unix_timestamp(ts: i64) -> i64 {
    if ts > UNIX_SEC_UPPER_BOUND {
        ts / 1000
    } else {
        ts
    }
}

pub fn wallet_age_from_first_activity(first_activity_unix: Option<i64>) -> (f64, Option<i64>) {
    if let Some(first) = first_activity_unix {
        let now = chrono::Utc::now().timestamp();
        let age = ((now - first) as f64 / SECONDS_PER_DAY).max(0.0);
        (age, Some(first))
    } else {
        (0.0, None)
    }
}

pub fn format_wallet_age(days: f64) -> String {
    if days <= 0.0 {
        "0 days".to_string()
    } else if days < 1.0 {
        let hours = (days * 24.0).round();
        if hours < 1.0 {
            "< 1 hour".to_string()
        } else {
            format!("{hours:.0} hours")
        }
    } else if days < 60.0 {
        format!("{days:.1} days")
    } else if days < 365.0 {
        format!("{:.0} days", days.round())
    } else {
        format!("{:.1} years", days / 365.25)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_millisecond_timestamps() {
        let sec = 1_700_000_000_i64;
        let ms = sec * 1000;
        assert_eq!(normalize_unix_timestamp(ms), sec);
        assert_eq!(normalize_unix_timestamp(sec), sec);
    }

    #[test]
    fn wallet_age_from_unix() {
        let now = chrono::Utc::now().timestamp();
        let thirty_days_ago = now - 30 * 86_400;
        let (days, _) = wallet_age_from_first_activity(Some(thirty_days_ago));
        assert!(days >= 29.9 && days <= 30.1);
    }

    #[test]
    fn format_wallet_age_readable() {
        assert_eq!(format_wallet_age(0.0), "0 days");
        assert_eq!(format_wallet_age(0.5), "12 hours");
        assert_eq!(format_wallet_age(45.0), "45.0 days");
    }
}
