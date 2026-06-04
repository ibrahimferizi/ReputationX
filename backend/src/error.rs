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
}

impl ApiError {
    pub fn internal<E: std::fmt::Display>(err: E) -> Self {
        ApiError::Internal(err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, user_message) = match &self {
            ApiError::InvalidAddress => (StatusCode::BAD_REQUEST, "Invalid Solana wallet address".to_string()),
            ApiError::MissingAddress => (StatusCode::BAD_REQUEST, "Wallet address is required".to_string()),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string()),
        };

        tracing::error!("API error: {:?}", self);

        let body = json!({
            "error": user_message,
        });

        (status, Json(body)).into_response()
    }
}
