use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;

/// One Helius REST call — much faster than paging `getSignaturesForAddress` on rate-limited RPCs.
pub async fn fetch_recent_timestamps(
    client: &Client,
    api_key: &str,
    address: &str,
    limit: u32,
) -> Result<Vec<i64>> {
    let url = format!(
        "https://api.helius.xyz/v0/addresses/{address}/transactions"
    );

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
        })
        .collect();

    timestamps.sort_unstable();
    Ok(timestamps)
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct HeliusTx {
    #[serde(default)]
    timestamp: Option<i64>,
}
