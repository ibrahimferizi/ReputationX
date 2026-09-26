pub mod diagnostics;
pub mod rugcheck;
pub mod solsniffer;

use self::diagnostics::ProviderIssue;
use self::rugcheck::RugCheckResult;
use self::solsniffer::SolsnifferResult;
use crate::config::Config;
use reqwest::Client;
use serde::Serialize;
use std::time::Duration;
use tokio::time::{timeout_at, Instant};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenScanStatus {
    #[default]
    Skipped,
    NoHoldings,
    TimedOut,
    Unavailable,
    Partial,
    Complete,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TokenScanCoverage {
    pub status: TokenScanStatus,
    pub held_mints: usize,
    pub selected_mints: usize,
    /// Mints with a usable result from at least one provider.
    pub checked_mints: usize,
    /// Mints successfully checked by every enabled provider.
    pub fully_checked_mints: usize,
    pub rugcheck_checked: usize,
    pub solsniffer_checked: usize,
    pub solsniffer_enabled: bool,
    pub issues: Vec<ProviderIssue>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ExternalScans {
    pub rugcheck: Vec<RugCheckResult>,
    pub solsniffer: Vec<SolsnifferResult>,
    pub honeypot_detected: bool,
    pub high_risk_tokens: usize,
    pub timed_out: bool,
    pub deadline_issues: Vec<ProviderIssue>,
}

impl ExternalScans {
    pub fn coverage(
        &self,
        held_mints: usize,
        max_scans: usize,
        skipped: bool,
        solsniffer_enabled: bool,
    ) -> TokenScanCoverage {
        let selected_mints = if skipped {
            0
        } else {
            held_mints.min(max_scans)
        };
        let mut checked = std::collections::HashSet::new();
        let mut rug_mints = std::collections::HashSet::new();
        let mut sniff_mints = std::collections::HashSet::new();
        for result in self.rugcheck.iter().filter(|r| r.available) {
            checked.insert(&result.mint);
            rug_mints.insert(&result.mint);
        }
        for result in self.solsniffer.iter().filter(|r| r.available) {
            checked.insert(&result.mint);
            sniff_mints.insert(&result.mint);
        }
        let fully_checked_mints = rug_mints
            .iter()
            .filter(|mint| !solsniffer_enabled || sniff_mints.contains(**mint))
            .count();
        let status = if held_mints == 0 {
            TokenScanStatus::NoHoldings
        } else if skipped || max_scans == 0 {
            TokenScanStatus::Skipped
        } else if self.timed_out {
            TokenScanStatus::TimedOut
        } else if checked.is_empty() {
            TokenScanStatus::Unavailable
        } else if fully_checked_mints < held_mints {
            TokenScanStatus::Partial
        } else {
            TokenScanStatus::Complete
        };
        TokenScanCoverage {
            status,
            held_mints,
            selected_mints,
            checked_mints: checked.len(),
            fully_checked_mints,
            rugcheck_checked: rug_mints.len(),
            solsniffer_checked: sniff_mints.len(),
            solsniffer_enabled,
            issues: self
                .rugcheck
                .iter()
                .flat_map(|r| r.issues.iter())
                .chain(self.solsniffer.iter().flat_map(|r| r.issues.iter()))
                .chain(self.deadline_issues.iter())
                .cloned()
                .collect(),
        }
    }

    fn summarize_risks(&mut self) {
        let mut risky = std::collections::HashSet::new();
        for rug in &self.rugcheck {
            if rug.is_honeypot {
                self.honeypot_detected = true;
            }
            if rug.is_honeypot
                || rug.score.unwrap_or(0) > 4000
                || rug.flags.iter().any(|f| f == "danger")
            {
                risky.insert(&rug.mint);
            }
        }
        for sniff in &self.solsniffer {
            if sniff.snifscore.is_some_and(|score| score < 40.0) {
                risky.insert(&sniff.mint);
            }
        }
        self.high_risk_tokens = risky.len();
    }
}

pub async fn scan_wallet_tokens(
    client: &Client,
    config: &Config,
    mints: &[String],
) -> ExternalScans {
    scan_until(
        client,
        config,
        mints,
        Instant::now() + Duration::from_secs(8),
    )
    .await
}

async fn scan_until(
    client: &Client,
    config: &Config,
    mints: &[String],
    deadline: Instant,
) -> ExternalScans {
    let mut scans = ExternalScans::default();
    for mint in mints.iter().take(config.max_token_scans) {
        match timeout_at(deadline, rugcheck::scan_token_mint(client, config, mint)).await {
            Ok(result) => scans.rugcheck.push(result),
            Err(_) => {
                scans.timed_out = true;
                scans
                    .deadline_issues
                    .push(ProviderIssue::timeout("rugcheck", mint));
                break;
            }
        }
        if config.solsniffer_api_key.is_some() {
            match timeout_at(deadline, solsniffer::scan_token_mint(client, config, mint)).await {
                Ok(result) => scans.solsniffer.push(result),
                Err(_) => {
                    scans.timed_out = true;
                    scans
                        .deadline_issues
                        .push(ProviderIssue::timeout("solsniffer", mint));
                    break;
                }
            }
        }
    }
    scans.summarize_risks();
    scans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rug(mint: &str, available: bool, dangerous: bool) -> RugCheckResult {
        RugCheckResult {
            mint: mint.into(),
            available,
            score: available.then_some(if dangerous { 5000 } else { 0 }),
            risk_level: None,
            is_honeypot: dangerous,
            flags: vec![],
            raw_summary: None,
            issues: vec![],
        }
    }

    #[test]
    fn coverage_tracks_disabled_unavailable_partial_and_complete_checks() {
        let mut scans = ExternalScans::default();
        assert_eq!(
            scans.coverage(0, 3, true, false).status,
            TokenScanStatus::NoHoldings
        );
        assert_eq!(
            scans.coverage(5, 3, true, false).status,
            TokenScanStatus::Skipped
        );
        assert_eq!(
            scans.coverage(5, 0, false, false).status,
            TokenScanStatus::Skipped
        );
        assert_eq!(
            scans.coverage(5, 3, false, false).status,
            TokenScanStatus::Unavailable
        );
        scans.rugcheck.push(rug("a", true, false));
        assert_eq!(
            scans.coverage(2, 1, false, false).status,
            TokenScanStatus::Partial
        );
        assert_eq!(
            scans.coverage(1, 1, false, false).status,
            TokenScanStatus::Complete
        );
        assert_eq!(
            scans.coverage(1, 1, false, true).status,
            TokenScanStatus::Partial
        );
        scans.solsniffer.push(SolsnifferResult {
            mint: "a".into(),
            available: true,
            checked_at: None,
            snifscore: Some(90.0),
            risk_label: None,
            raw: None,
            issues: vec![],
        });
        assert_eq!(
            scans.coverage(1, 1, false, true).status,
            TokenScanStatus::Complete
        );
        assert_eq!(scans.coverage(1, 1, false, true).checked_mints, 1);
    }

    #[test]
    fn provider_duplicates_do_not_double_count_token_risk() {
        let mut scans = ExternalScans::default();
        scans.rugcheck.push(rug("a", true, true));
        scans.solsniffer.push(SolsnifferResult {
            mint: "a".into(),
            available: true,
            checked_at: None,
            snifscore: Some(10.0),
            risk_label: None,
            raw: None,
            issues: vec![],
        });
        scans.summarize_risks();
        assert_eq!(scans.high_risk_tokens, 1);
        assert!(scans.honeypot_detected);
    }

    #[tokio::test]
    async fn timeout_preserves_the_risk_result_already_received() {
        use axum::{routing::get, Json, Router};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new()
            .route(
                "/v1/tokens/:mint/report/summary",
                get(|| async { Json(serde_json::json!({"score": 5000, "isHoneypot": true})) }),
            )
            .route(
                "/token/:mint",
                get(|| async {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    Json(serde_json::json!({"score": 90}))
                }),
            );
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let mut config = Config::from_env();
        config.rugcheck_base_url = base.clone();
        config.rugcheck_api_key = None;
        config.solsniffer_base_url = base;
        config.solsniffer_api_key = Some("test-only".into());
        config.max_token_scans = 1;
        let result = scan_until(
            &Client::new(),
            &config,
            &["a".into()],
            Instant::now() + Duration::from_millis(500),
        )
        .await;
        server.abort();
        assert!(result.timed_out);
        assert!(result.honeypot_detected);
        assert_eq!(result.high_risk_tokens, 1);
        assert_eq!(
            result.coverage(1, 1, false, true).status,
            TokenScanStatus::TimedOut
        );
        assert_eq!(result.coverage(1, 1, false, true).checked_mints, 1);
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Calls live token providers using backend/.env; uses provider quota"]
    async fn live_provider_contract_smoke() {
        let _ = dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/.env"));
        let mut config = Config::from_env();
        config.max_token_scans = 1;
        // Token from SolSniffer's official OpenAPI examples.
        let scans = scan_wallet_tokens(
            &Client::new(),
            &config,
            &["MEW1gQWJ3nEXg2qgERiKu7FAFj79PHvQVREQUzScPP5".into()],
        )
        .await;
        let coverage = scans.coverage(1, 1, false, config.solsniffer_api_key.is_some());
        println!(
            "Live provider coverage: {}",
            serde_json::to_string(&coverage).unwrap()
        );
        assert_eq!(coverage.rugcheck_checked, 1);
        assert_eq!(coverage.solsniffer_checked, 1);
        assert_eq!(coverage.status, TokenScanStatus::Complete);
    }
}
