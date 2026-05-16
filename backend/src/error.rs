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
        let (status, message) = match &self {
            ApiError::InvalidAddress | ApiError::MissingAddress | ApiError::BadRequest(_) => {
                (StatusCode::BAD_REQUEST, self.to_string())
            }
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        let body = json!({
            "error": message,
            "details": message,
        });

        (status, Json(body)).into_response()
    }
}
