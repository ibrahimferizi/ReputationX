use super::diagnostics::ProviderIssue;
use crate::config::Config;
use reqwest::Client;
use serde_json::Value;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

const CACHE_TTL: Duration = Duration::from_secs(3600);
const COOLDOWN: Duration = Duration::from_secs(600);

/// Process-local guard, not a persistent account/monthly billing meter.
#[derive(Debug)]
pub struct ScanState {
    calls: usize,
    limit: usize,
    cooldown_until: Option<Instant>,
    cache: HashMap<String, (Instant, SolsnifferResult)>,
}
impl ScanState {
    pub fn new(limit: usize) -> Self {
        Self {
            calls: 0,
            limit,
            cooldown_until: None,
            cache: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SolsnifferResult {
    pub mint: String,
    pub available: bool,
    pub snifscore: Option<f64>,
    pub checked_at: Option<String>,
    pub risk_label: Option<String>,
    pub raw: Option<Value>,
    pub issues: Vec<ProviderIssue>,
}

impl SolsnifferResult {
    fn failed(mint: &str, issue: ProviderIssue) -> Self {
        Self {
            mint: mint.into(),
            available: false,
            snifscore: None,
            checked_at: None,
            risk_label: None,
            raw: None,
            issues: vec![issue],
        }
    }
}

/// Official API v2: GET /token/{address}, X-API-KEY auth, tokenData.score.
pub async fn scan_token_mint(client: &Client, config: &Config, mint: &str) -> SolsnifferResult {
    let Some(key) = config.solsniffer_api_key.as_ref() else {
        return SolsnifferResult::failed(
            mint,
            ProviderIssue::new(
                "solsniffer",
                mint,
                "not_configured",
                "SolSniffer is not enabled.",
            ),
        );
    };
    // Serialize misses so duplicate mints across wallets spend only one call.
    // Cancellation releases the lock; attempted calls still consume the local allowance.
    let mut state = config.solsniffer_state.lock().await;
    state.cache.retain(|_, (at, _)| at.elapsed() < CACHE_TTL);
    if let Some((_, cached)) = state.cache.get(mint) {
        let mut cached = cached.clone();
        cached.issues.push(ProviderIssue::new(
            "solsniffer",
            mint,
            "cached_result",
            "Reused a SolSniffer token result from the last hour; see its checked timestamp.",
        ));
        return cached;
    }
    if state.calls >= state.limit {
        return SolsnifferResult::failed(mint, ProviderIssue::new("solsniffer", mint, "local_budget_exhausted",
            "This server's SolSniffer request allowance is exhausted. Other checks remain available."));
    }
    if state
        .cooldown_until
        .is_some_and(|until| until > Instant::now())
    {
        return SolsnifferResult::failed(mint, ProviderIssue::new("solsniffer", mint, "provider_cooldown",
            "SolSniffer requests are paused after an access or quota error. Other checks remain available."));
    }
    state.calls += 1;
    let url = format!(
        "{}/token/{mint}",
        config.solsniffer_base_url.trim_end_matches('/')
    );
    let result = match client.get(url).header("X-API-KEY", key).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<Value>().await {
            Ok(body) => parse_solsniffer_body(mint, body),
            Err(_) => {
                SolsnifferResult::failed(mint, ProviderIssue::invalid_response("solsniffer", mint))
            }
        },
        Ok(resp) => {
            let status = resp.status().as_u16();
            if matches!(status, 401 | 402 | 403 | 429) {
                state.cooldown_until = Some(Instant::now() + COOLDOWN);
            }
            SolsnifferResult::failed(mint, ProviderIssue::http("solsniffer", mint, status))
        }
        Err(error) => {
            SolsnifferResult::failed(mint, ProviderIssue::transport("solsniffer", mint, &error))
        }
    };
    if result.available {
        if state.cache.len() >= 100 {
            if let Some(oldest) = state
                .cache
                .iter()
                .min_by_key(|(_, (at, _))| *at)
                .map(|(key, _)| key.clone())
            {
                state.cache.remove(&oldest);
            }
        }
        state
            .cache
            .insert(mint.into(), (Instant::now(), result.clone()));
    }
    result
}

fn parse_solsniffer_body(mint: &str, body: Value) -> SolsnifferResult {
    let snifscore = body
        .pointer("/tokenData/score")
        .and_then(Value::as_f64)
        .filter(|score| (0.0..=100.0).contains(score));
    let available = snifscore.is_some();
    SolsnifferResult {
        mint: mint.into(),
        available,
        snifscore,
        checked_at: available.then(|| chrono::Utc::now().to_rfc3339()),
        risk_label: None,
        raw: Some(body),
        issues: if available {
            vec![]
        } else {
            vec![ProviderIssue::invalid_response("solsniffer", mint)]
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        http::{HeaderMap, StatusCode},
        routing::get,
        Json, Router,
    };

    #[test]
    fn parses_documented_nested_score_and_rejects_unusable_payloads() {
        for score in [0, 47, 100] {
            let result =
                parse_solsniffer_body("mint", serde_json::json!({"tokenData":{"score":score}}));
            assert!(result.available);
            assert_eq!(result.snifscore, Some(score as f64));
        }
        for body in [
            Value::Null,
            serde_json::json!({"score":90}),
            serde_json::json!({"tokenData":{"score":101}}),
        ] {
            let result = parse_solsniffer_body("mint", body);
            assert!(!result.available);
            assert_eq!(result.issues[0].code, "invalid_response");
        }
    }

    #[tokio::test]
    async fn uses_api_v2_path_and_api_key_header() {
        let app = Router::new().route(
            "/api/v2/token/:mint",
            get(|headers: HeaderMap| async move {
                assert_eq!(headers["x-api-key"], "test-key");
                assert!(!headers.contains_key("authorization"));
                (
                    StatusCode::CREATED,
                    Json(serde_json::json!({"tokenData":{"score":47},"tokenInfo":{}})),
                )
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut config = Config::from_env();
        config.solsniffer_base_url = format!("http://{}/api/v2", listener.local_addr().unwrap());
        config.solsniffer_api_key = Some("test-key".into());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let result = scan_token_mint(&Client::new(), &config, "mint").await;
        assert_eq!(result.snifscore, Some(47.0));
        assert!(result.available);
        assert!(result.issues.is_empty());
        server.abort();
    }
    #[tokio::test]
    async fn shared_cache_coalesces_calls_and_preserves_budget() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let app = Router::new().route(
            "/token/:mint",
            get(move || {
                let observed = observed.clone();
                async move {
                    observed.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    Json(serde_json::json!({"tokenData":{"score":47}}))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut config = Config::from_env();
        config.solsniffer_base_url = format!("http://{}", listener.local_addr().unwrap());
        config.solsniffer_api_key = Some("test-key".into());
        config.solsniffer_state = Arc::new(tokio::sync::Mutex::new(ScanState::new(2)));
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let client = Client::new();
        let (first, duplicate) = tokio::join!(
            scan_token_mint(&client, &config, "a"),
            scan_token_mint(&client, &config, "a")
        );
        assert!(first.available && duplicate.available);
        assert_eq!(first.checked_at, duplicate.checked_at);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(duplicate.issues[0].code, "cached_result");
        assert!(scan_token_mint(&client, &config, "b").await.available);
        assert_eq!(
            scan_token_mint(&client, &config, "c").await.issues[0].code,
            "local_budget_exhausted"
        );
        assert!(scan_token_mint(&client, &config, "a").await.available);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        config
            .solsniffer_state
            .lock()
            .await
            .cache
            .get_mut("a")
            .unwrap()
            .0 = Instant::now() - CACHE_TTL;
        assert!(!scan_token_mint(&client, &config, "a").await.available);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        server.abort();
    }

    #[tokio::test]
    async fn quota_errors_pause_further_calls_without_hiding_other_provider_results() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        for status in [StatusCode::PAYMENT_REQUIRED, StatusCode::TOO_MANY_REQUESTS] {
            let calls = Arc::new(AtomicUsize::new(0));
            let observed = calls.clone();
            let app = Router::new().route(
                "/token/:mint",
                get(move || {
                    observed.fetch_add(1, Ordering::SeqCst);
                    async move { status }
                }),
            );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let mut config = Config::from_env();
            config.solsniffer_base_url = format!("http://{}", listener.local_addr().unwrap());
            config.solsniffer_api_key = Some("test-key".into());
            let server = tokio::spawn(async move {
                axum::serve(listener, app).await.unwrap();
            });
            let client = Client::new();
            let failed = scan_token_mint(&client, &config, "a").await;
            assert!(!failed.available);
            assert_eq!(failed.issues[0].http_status, Some(status.as_u16()));
            assert_eq!(
                scan_token_mint(&client, &config, "b").await.issues[0].code,
                "provider_cooldown"
            );
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            let mut scans = crate::integrations::ExternalScans::default();
            scans
                .rugcheck
                .push(crate::integrations::rugcheck::RugCheckResult {
                    mint: "a".into(),
                    available: true,
                    score: Some(0),
                    risk_level: None,
                    is_honeypot: false,
                    flags: vec![],
                    raw_summary: None,
                    issues: vec![],
                });
            scans.solsniffer.push(failed);
            assert_eq!(
                scans.coverage(1, 1, false, true).status,
                crate::integrations::TokenScanStatus::Partial
            );
            server.abort();
        }
    }
}
