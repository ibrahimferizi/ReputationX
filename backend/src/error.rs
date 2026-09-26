use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Invalid Solana wallet address")]
    InvalidAddress,
    #[error("Wallet address is required")]
    MissingAddress,
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Internal(String),
    #[error("Scan limit reached; please retry later")]
    RateLimited(u64),
    #[error("The service is busy; please retry shortly")]
    Busy,
    #[error("The scan timed out; try fewer wallets or retry later")]
    Timeout,
}

impl ApiError {
    pub fn internal<E: std::fmt::Display>(err: E) -> Self {
        ApiError::Internal(err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let retry = match &self {
            ApiError::RateLimited(seconds) => Some(*seconds),
            ApiError::Busy => Some(5),
            _ => None,
        };
        let (status, user_message) = match &self {
            ApiError::InvalidAddress => (
                StatusCode::BAD_REQUEST,
                "Invalid Solana wallet address".to_string(),
            ),
            ApiError::MissingAddress => (
                StatusCode::BAD_REQUEST,
                "Wallet address is required".to_string(),
            ),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::RateLimited(_) => (StatusCode::TOO_MANY_REQUESTS, self.to_string()),
            ApiError::Busy => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            ApiError::Timeout => (StatusCode::GATEWAY_TIMEOUT, self.to_string()),
            ApiError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
        };

        // Upstream errors may include credential-bearing URLs; never log raw errors.
        if matches!(self, ApiError::Internal(_)) {
            tracing::error!("Upstream scan failed");
        }

        let body = json!({
            "error": user_message,
        });

        let mut response = (status, Json(body)).into_response();
        if let Some(seconds) = retry {
            response
                .headers_mut()
                .insert("retry-after", seconds.to_string().parse().unwrap());
        }
        response
    }
}
