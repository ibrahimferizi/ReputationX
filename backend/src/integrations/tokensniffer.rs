use crate::config::Config;
use reqwest::Client;
use serde_json::Value;

#[derive(Debug, Clone, serde::Serialize)]
pub struct TokenSnifferResult {
    pub chain_id: i32,
    pub address: String,
    pub available: bool,
    pub is_honeypot: bool,
    pub is_flagged: bool,
    pub exploits: Vec<String>,
    pub score: Option<i64>,
    pub note: String,
}

/// TokenSniffer is EVM-focused; exposed for future cross-chain API product tier.
pub async fn scan_evm_token(
    client: &Client,
    config: &Config,
    chain_id: i32,
    address: &str,
) -> TokenSnifferResult {
    let Some(api_key) = config.tokensniffer_api_key.as_ref() else {
        return TokenSnifferResult {
            chain_id,
            address: address.to_string(),
            available: false,
            is_honeypot: false,
            is_flagged: false,
            exploits: vec![],
            score: None,
            note: "Set TOKENSNIFFER_API_KEY for EVM honeypot scans".into(),
        };
    };

    let url = format!(
        "{}/tokens/{}/{}",
        config.tokensniffer_base_url.trim_end_matches('/'),
        chain_id,
        address
    );

    let response = client
        .get(&url)
        .query(&[
            ("include_metrics", "true"),
            ("include_tests", "true"),
        ])
        .header("X-API-KEY", api_key)
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            let body: Value = resp.json().await.unwrap_or(Value::Null);
            let exploits: Vec<String> = body
                .get("exploits")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|e| e.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();

            let is_honeypot = exploits
                .iter()
                .any(|e| e.to_ascii_lowercase().contains("honeypot"));

            TokenSnifferResult {
                chain_id,
                address: address.to_string(),
                available: true,
                is_honeypot,
                is_flagged: body
                    .get("is_flagged")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                exploits,
                score: body.get("score").and_then(|v| v.as_i64()),
                note: "TokenSniffer EVM scan".into(),
            }
        }
        Ok(resp) => TokenSnifferResult {
            chain_id,
            address: address.to_string(),
            available: false,
            is_honeypot: false,
            is_flagged: false,
            exploits: vec![],
            score: None,
            note: format!("http_{}", resp.status()),
        },
        Err(err) => TokenSnifferResult {
            chain_id,
            address: address.to_string(),
            available: false,
            is_honeypot: false,
            is_flagged: false,
            exploits: vec![],
            score: None,
            note: err.to_string(),
        },
    }
}
