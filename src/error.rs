use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub success: bool,
    pub error: ErrorDetail,
}

#[derive(Debug, Clone)]
pub struct AppError {
    pub status_code: StatusCode,
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl AppError {
    pub fn new(
        status_code: StatusCode,
        code: impl Into<String>,
        message: impl Into<String>,
        details: Option<serde_json::Value>,
    ) -> Self {
        Self {
            status_code,
            code: code.into(),
            message: message.into(),
            details,
        }
    }

    pub fn bad_request(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message, None)
    }

    pub fn bad_request_with_details(
        message: impl Into<String>,
        code: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message, Some(details))
    }

    pub fn unauthorized(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, code, message, None)
    }

    pub fn forbidden(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, code, message, None)
    }

    pub fn not_found(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message, None)
    }

    pub fn conflict(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message, None)
    }

    pub fn unprocessable_entity(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, code, message, None)
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
            message,
            None,
        )
    }

    pub fn payload_too_large(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            message,
            None,
        )
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_SERVER_ERROR",
            message,
            None,
        )
    }

    pub fn market_error(status_code: StatusCode, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(status_code, code, message, None)
    }

    pub fn validation_error(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "VALIDATION_ERROR", message, None)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {}: {}",
            self.status_code.as_u16(),
            self.code,
            self.message
        )
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let envelope = ErrorEnvelope {
            success: false,
            error: ErrorDetail {
                code: self.code,
                message: self.message,
                details: self.details,
            },
        };

        (self.status_code, Json(envelope)).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        tracing::error!("Database error: {:?}", err);
        match err {
            sqlx::Error::Database(db_err) => {
                if db_err.is_unique_violation() {
                    AppError::conflict("Resource conflict.", "CONFLICT")
                } else if db_err.is_foreign_key_violation() {
                    AppError::bad_request("Referenced entity does not exist.", "FOREIGN_KEY_VIOLATION")
                } else {
                    AppError::internal("An unexpected database error occurred.")
                }
            }
            sqlx::Error::RowNotFound => AppError::not_found("Resource not found.", "NOT_FOUND"),
            _ => AppError::internal("An unexpected database error occurred."),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::bad_request(
            format!("Invalid JSON payload: {}", err),
            "VALIDATION_ERROR",
        )
    }
}
