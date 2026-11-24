use std::time::Instant;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::Utc;

use crate::state::AppState;

async fn health_check_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let started = Instant::now();
    let db_res = sqlx::query("SELECT 1").execute(&state.pool).await;
    let latency_ms = started.elapsed().as_millis();
    let timestamp = Utc::now().to_rfc3339();

    match db_res {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "data": {
                    "status": "ok",
                    "database": "connected",
                    "dbLatencyMs": latency_ms,
                    "timestamp": timestamp
                }
            })),
        ),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "success": false,
                "data": {
                    "status": "degraded",
                    "database": "unreachable",
                    "timestamp": timestamp
                }
            })),
        ),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/", get(health_check_handler))
}
