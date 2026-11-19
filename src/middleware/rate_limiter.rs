use std::time::Duration;
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

use crate::{error::AppError, state::AppState};

pub async fn global_rate_limit_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    if !state.config.rate_limit_enabled {
        return Ok(next.run(request).await);
    }

    let ip = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    let allowed = state
        .rate_limiter
        .check_and_record(
            &format!("global:{}", ip),
            Duration::from_millis(state.config.rate_limit_window_ms),
            state.config.rate_limit_global_max,
        )
        .await;

    if !allowed {
        return Err(AppError::rate_limited(
            "Too many requests. Please try again later.",
        ));
    }

    Ok(next.run(request).await)
}
