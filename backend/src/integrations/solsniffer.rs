use crate::config::Config;
use reqwest::Client;
use serde_json::Value;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SolsnifferResult {
    pub mint: String,
    pub available: bool,
    pub snifscore: Option<f64>,
    pub risk_label: Option<String>,
    pub raw: Option<Value>,
}

/// Solsniffer token scan (requires `SOLSNIFFER_API_KEY` for production use).
pub async fn scan_token_mint(
    client: &Client,
    config: &Config,
    mint: &str,
) -> SolsnifferResult {
    let Some(api_key) = config.solsniffer_api_key.as_ref() else {
        return SolsnifferResult {
            mint: mint.to_string(),
            available: false,
            snifscore: None,
            risk_label: Some("api_key_not_configured".into()),
            raw: None,
        };
    };

    let url = format!(
        "{}/v1/token/{}",
        config.solsniffer_base_url.trim_end_matches('/'),
        mint
    );

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            let body: Value = resp.json().await.unwrap_or(Value::Null);
            let snifscore = body
                .get("snifscore")
                .or_else(|| body.get("score"))
                .and_then(|v| v.as_f64());
            let risk_label = body
                .get("risk")
                .and_then(|v| v.as_str())
                .map(str::to_string);

            SolsnifferResult {
                mint: mint.to_string(),
                available: true,
                snifscore,
                risk_label,
                raw: Some(body),
            }
        }
        Ok(resp) => SolsnifferResult {
            mint: mint.to_string(),
            available: false,
            snifscore: None,
            risk_label: Some(format!("http_{}", resp.status())),
            raw: None,
        },
        Err(_) => SolsnifferResult {
            mint: mint.to_string(),
            available: false,
            snifscore: None,
            risk_label: None,
            raw: None,
        },
    }
}
