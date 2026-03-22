use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Token error: {0}")]
    TokenError(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Unauthorized(_)    => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::Forbidden(_)       => (StatusCode::FORBIDDEN, self.to_string()),
            AppError::ValidationError(_) => (StatusCode::UNPROCESSABLE_ENTITY, self.to_string()),
            AppError::DatabaseError(_)   => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            AppError::TokenError(_)      => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            AppError::NotFound(_)        => (StatusCode::NOT_FOUND, self.to_string()),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}