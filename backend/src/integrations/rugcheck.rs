use super::diagnostics::ProviderIssue;
use crate::config::Config;
use reqwest::Client;
use serde_json::Value;

// Public RugCheck access permits one request per second per caller. Shared by wallets.
static PUBLIC_SLOT: tokio::sync::Mutex<Option<tokio::time::Instant>> =
    tokio::sync::Mutex::const_new(None);
async fn public_slot() {
    let mut previous = PUBLIC_SLOT.lock().await;
    if let Some(at) = *previous {
        tokio::time::sleep_until(at + std::time::Duration::from_millis(1100)).await;
    }
    *previous = Some(tokio::time::Instant::now());
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RugCheckResult {
    pub mint: String,
    pub available: bool,
    pub score: Option<i64>,
    pub risk_level: Option<String>,
    pub is_honeypot: bool,
    pub flags: Vec<String>,
    pub raw_summary: Option<Value>,
    pub issues: Vec<ProviderIssue>,
}

impl RugCheckResult {
    fn failed(mint: &str, issue: ProviderIssue) -> Self {
        Self {
            mint: mint.into(),
            available: false,
            score: None,
            risk_level: None,
            is_honeypot: false,
            flags: vec![],
            raw_summary: None,
            issues: vec![issue],
        }
    }
}

pub async fn scan_token_mint(client: &Client, config: &Config, mint: &str) -> RugCheckResult {
    let base = config.rugcheck_base_url.trim_end_matches('/');
    let url = format!("{base}/v1/tokens/{mint}/report/summary");
    // This official summary route is public. Never retry other/custom services without auth.
    request_summary(
        client,
        &url,
        mint,
        config.rugcheck_api_key.as_deref(),
        base == "https://api.rugcheck.xyz",
    )
    .await
}

async fn request_summary(
    client: &Client,
    url: &str,
    mint: &str,
    key: Option<&str>,
    allow_public_fallback: bool,
) -> RugCheckResult {
    let mut request = client.get(url);
    if let Some(key) = key {
        // FluxRPC-issued RugCheck keys use X-API-KEY. JWT callers opt into Bearer.
        request = if let Some(jwt) = key.strip_prefix("Bearer ") {
            request.bearer_auth(jwt)
        } else {
            request.header("X-API-KEY", key)
        };
    }
    if allow_public_fallback && key.is_none() {
        public_slot().await;
    }
    let mut response = request.send().await;
    let mut warning = None;
    if key.is_some()
        && allow_public_fallback
        && response.as_ref().is_ok_and(|r| r.status().as_u16() == 401)
    {
        warning = Some(ProviderIssue {
            http_status: Some(401),
            ..ProviderIssue::new("rugcheck", mint, "public_fallback",
                "Configured credential was rejected; retried the public summary endpoint without it.")
        });
        public_slot().await;
        response = client.get(url).send().await;
    }
    let mut result = match response {
        Ok(resp) if resp.status().is_success() => match resp.json::<Value>().await {
            Ok(body) => parse_rugcheck_body(mint, body),
            Err(_) => {
                RugCheckResult::failed(mint, ProviderIssue::invalid_response("rugcheck", mint))
            }
        },
        Ok(resp) => RugCheckResult::failed(
            mint,
            ProviderIssue::http("rugcheck", mint, resp.status().as_u16()),
        ),
        Err(error) => {
            RugCheckResult::failed(mint, ProviderIssue::transport("rugcheck", mint, &error))
        }
    };
    if let Some(issue) = warning {
        result.issues.insert(0, issue);
    }
    result
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

    let available = score.is_some() || !flags.is_empty() || is_honeypot;
    RugCheckResult {
        mint: mint.to_string(),
        available,
        score,
        risk_level,
        is_honeypot,
        flags,
        raw_summary: Some(body),
        issues: if available {
            vec![]
        } else {
            vec![ProviderIssue::invalid_response("rugcheck", mint)]
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_success_payload_is_not_a_completed_scan() {
        assert!(!parse_rugcheck_body("a", Value::Null).available);
        assert!(!parse_rugcheck_body("a", serde_json::json!({"message": "unavailable"})).available);
        assert!(parse_rugcheck_body("a", serde_json::json!({"score": 0})).available);
    }
}

#[cfg(test)]
mod request_tests {
    use super::*;
    use axum::{
        http::{HeaderMap, StatusCode},
        routing::get,
        Json, Router,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[tokio::test]
    async fn rejected_auth_can_recover_only_on_the_public_summary_route() {
        let public_calls = Arc::new(AtomicUsize::new(0));
        let calls = public_calls.clone();
        let app = Router::new().route(
            "/summary",
            get(move |headers: HeaderMap| {
                let calls = calls.clone();
                async move {
                    if headers.contains_key("authorization") || headers.contains_key("x-api-key") {
                        if let Some(key) = headers.get("x-api-key") {
                            assert_eq!(key, "test-key");
                            assert!(!headers.contains_key("authorization"));
                        } else {
                            assert_eq!(headers["authorization"], "Bearer test-key");
                        }
                        (
                            StatusCode::UNAUTHORIZED,
                            Json(serde_json::json!({"error":"test-key must never be echoed"})),
                        )
                    } else {
                        calls.fetch_add(1, Ordering::SeqCst);
                        (StatusCode::OK, Json(serde_json::json!({"score": 0})))
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/summary", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let result = request_summary(&Client::new(), &url, "mint", Some("test-key"), true).await;
        assert!(result.available);
        assert_eq!(result.issues[0].code, "public_fallback");
        assert_eq!(result.issues[0].http_status, Some(401));
        assert_eq!(public_calls.load(Ordering::SeqCst), 1);
        let result = request_summary(&Client::new(), &url, "mint", Some("test-key"), false).await;
        assert!(!result.available);
        assert_eq!(result.issues[0].code, "authentication_failed");
        assert_eq!(public_calls.load(Ordering::SeqCst), 1);
        assert!(!serde_json::to_string(&result.issues)
            .unwrap()
            .contains("test-key"));
        let jwt_result =
            request_summary(&Client::new(), &url, "mint", Some("Bearer test-key"), false).await;
        assert_eq!(jwt_result.issues[0].code, "authentication_failed");
        assert_eq!(public_calls.load(Ordering::SeqCst), 1);
        server.abort();
    }
}
