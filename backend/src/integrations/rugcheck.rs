use crate::config::Config;
use anyhow::Result;
use reqwest::Client;
use serde_json::Value;

#[derive(Debug, Clone, serde::Serialize)]
pub struct RugCheckResult {
    pub mint: String,
    pub available: bool,
    pub score: Option<i64>,
    pub risk_level: Option<String>,
    pub is_honeypot: bool,
    pub flags: Vec<String>,
    pub raw_summary: Option<Value>,
}

pub async fn scan_token_mint(client: &Client, config: &Config, mint: &str) -> RugCheckResult {
    let url = format!(
        "{}/v1/tokens/{}/report/summary",
        config.rugcheck_base_url.trim_end_matches('/'),
        mint
    );

    let mut request = client.get(&url);
    if let Some(key) = &config.rugcheck_api_key {
        request = request.header("X-API-KEY", key);
    }

    match request.send().await {
        Ok(resp) if resp.status().is_success() => {
            let body: Value = resp.json().await.unwrap_or(Value::Null);
            parse_rugcheck_body(mint, body)
        }
        Ok(resp) => RugCheckResult {
            mint: mint.to_string(),
            available: false,
            score: None,
            risk_level: Some(format!("http_{}", resp.status())),
            is_honeypot: false,
            flags: vec![],
            raw_summary: None,
        },
        Err(_) => RugCheckResult {
            mint: mint.to_string(),
            available: false,
            score: None,
            risk_level: None,
            is_honeypot: false,
            flags: vec![],
            raw_summary: None,
        },
    }
}

fn parse_rugcheck_body(mint: &str, body: Value) -> RugCheckResult {
    let score = body
        .get("score")
        .or_else(|| body.pointer("/risk/score"))
        .and_then(|v| v.as_i64());

    let risk_level = body
        .get("riskLevel")
        .or_else(|| body.get("risk_level"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let mut flags: Vec<String> = Vec::new();
    if let Some(risks) = body.get("risks").and_then(|v| v.as_array()) {
        for risk in risks {
            if let Some(name) = risk.get("name").and_then(|v| v.as_str()) {
                flags.push(name.to_string());
            }
            if let Some(level) = risk.get("level").and_then(|v| v.as_str()) {
                if level.eq_ignore_ascii_case("danger") {
                    flags.push("danger".to_string());
                }
            }
        }
    }

    let is_honeypot = flags.iter().any(|f| {
        f.to_ascii_lowercase().contains("honeypot")
            || f.to_ascii_lowercase().contains("cannot sell")
    }) || body
        .get("isHoneypot")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    RugCheckResult {
        mint: mint.to_string(),
        available: true,
        score,
        risk_level,
        is_honeypot,
        flags,
        raw_summary: Some(body),
    }
}

pub async fn scan_wallet_mints(
    client: &Client,
    config: &Config,
    mints: &[String],
) -> Result<Vec<RugCheckResult>> {
    let mut results = Vec::new();
    for mint in mints.iter().take(config.max_token_scans) {
        results.push(scan_token_mint(client, config, mint).await);
    }
    Ok(results)
}
