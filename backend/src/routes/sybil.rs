use std::sync::Arc;

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::analysis::cluster::{build_clusters, WalletCluster};
use crate::analysis::reputation::build_wallet_reputation;
use crate::analysis::risk::RiskLevel;
use crate::error::ApiError;
use crate::models::response::ReputationResponse;
use crate::solana;
use crate::AppState;

const MAX_SYBIL_SCAN_ADDRESSES: usize = 50;

#[derive(Debug, Deserialize)]
pub struct SybilScanRequest {
    pub addresses: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SybilScanResponse {
    pub wallets: Vec<ReputationResponse>,
    pub clusters: Vec<WalletCluster>,
    pub total_scanned: usize,
    pub flagged: usize,
}

/// `POST /api/sybil-scan` — batch reputation scan with in-memory funder clustering.
pub async fn post_sybil_scan(
    State(state): State<AppState>,
    Json(body): Json<SybilScanRequest>,
) -> Result<Json<SybilScanResponse>, ApiError> {
    if body.addresses.is_empty() {
        return Err(ApiError::BadRequest("addresses must not be empty".into()));
    }
    if body.addresses.len() > MAX_SYBIL_SCAN_ADDRESSES {
        return Err(ApiError::BadRequest(format!(
            "at most {MAX_SYBIL_SCAN_ADDRESSES} addresses per request"
        )));
    }

    let mut normalized: Vec<String> = Vec::with_capacity(body.addresses.len());
    for raw in &body.addresses {
        let address = raw.trim();
        if address.is_empty() {
            return Err(ApiError::BadRequest(
                "addresses must not contain empty entries".into(),
            ));
        }
        solana::validate_address(address).map_err(|_| ApiError::InvalidAddress)?;
        normalized.push(address.to_string());
    }

    let wallet_sem = Arc::new(Semaphore::new(state.config.sybil_scan_concurrency));
    let mut tasks = Vec::with_capacity(normalized.len());

    for address in normalized {
        let state = state.clone();
        let wallet_sem = wallet_sem.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = wallet_sem
                .acquire()
                .await
                .map_err(|e| ApiError::internal(format!("sybil scan slot: {e}")))?;

            let report = build_wallet_reputation(
                &state.http,
                &state.rpc,
                &state.history_rpc,
                &state.config,
                &address,
            )
            .await
            .map_err(ApiError::internal)?;

            Ok::<_, ApiError>((address, report))
        }));
    }

    let mut paired: Vec<(String, ReputationResponse)> = Vec::with_capacity(tasks.len());
    for task in tasks {
        let pair = task
            .await
            .map_err(|e| ApiError::internal(format!("scan task failed: {e}")))??;
        paired.push(pair);
    }

    let clusters = build_clusters(&paired);
    let mut flagged_addresses: std::collections::HashSet<String> = clusters
        .iter()
        .flat_map(|c| c.members.iter().cloned())
        .collect();

    for (_, report) in &paired {
        if wallet_has_elevated_risk(report) {
            flagged_addresses.insert(report.address.clone());
        }
    }

    let wallets: Vec<ReputationResponse> = paired.into_iter().map(|(_, r)| r).collect();
    let total_scanned = wallets.len();
    let flagged = flagged_addresses.len();

    Ok(Json(SybilScanResponse {
        wallets,
        clusters,
        total_scanned,
        flagged,
    }))
}

/// Wallets that look risky on their own (not only via shared-funder clustering).
fn wallet_has_elevated_risk(report: &ReputationResponse) -> bool {
    if report.legacy.reputation_score <= 40 {
        return true;
    }
    if report.token_risks.iter().any(|t| t.honeypot) {
        return true;
    }

    let high_metrics = report
        .metrics
        .iter()
        .filter(|m| m.risk == RiskLevel::High)
        .count();
    if high_metrics > 0 {
        return true;
    }

    let medium_risk_signals = report
        .metrics
        .iter()
        .filter(|m| {
            m.risk == RiskLevel::Medium
                && matches!(
                    m.id.as_str(),
                    "wallet_age" | "tx_burst" | "tx_count" | "tx_spacing" | "token_risk"
                )
        })
        .count();

    medium_risk_signals >= 2
}
