use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;

use crate::analysis::reputation::build_wallet_reputation;
use crate::error::ApiError;
use crate::models::response::ReputationResponse;
use crate::solana;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ReputationQuery {
    pub address: Option<String>,
}

/// `GET /api/reputation?address=...` — backward compatible with the demo frontend.
pub async fn get_reputation(
    State(state): State<AppState>,
    Query(query): Query<ReputationQuery>,
) -> Result<Json<ReputationResponse>, ApiError> {
    let address = query
        .address
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or(ApiError::MissingAddress)?;

    solana::validate_address(address).map_err(|_| ApiError::InvalidAddress)?;

    let report = build_wallet_reputation(
        &state.http,
        &state.rpc,
        &state.history_rpc,
        &state.config,
        address,
    )
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(report))
}
