use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::NaiveDate;
use serde::Deserialize;

use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::services::leaderboard_service;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LeaderboardQuery {
    pub period: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct LeaderboardHistoryQuery {
    pub period: Option<String>,
    pub date: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

async fn global_leaderboard_handler(
    _auth: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<LeaderboardQuery>,
) -> Result<impl IntoResponse, AppError> {
    let result = leaderboard_service::get_global_leaderboard(
        &state.pool,
        query.period.as_deref(),
        query.page,
        query.limit,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": result
    })))
}

async fn friends_leaderboard_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<LeaderboardQuery>,
) -> Result<impl IntoResponse, AppError> {
    let result = leaderboard_service::get_friends_leaderboard(
        &state.pool,
        &auth.user_id,
        query.period.as_deref(),
        query.page,
        query.limit,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": result
    })))
}

async fn leaderboard_history_handler(
    _auth: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<LeaderboardHistoryQuery>,
) -> Result<impl IntoResponse, AppError> {
    let parse_date = |s: Option<String>| -> Result<Option<NaiveDate>, AppError> {
        match s {
            Some(v) if !v.trim().is_empty() => {
                let d = NaiveDate::parse_from_str(v.trim(), "%Y-%m-%d").map_err(|_| {
                    AppError::bad_request("Date must be in YYYY-MM-DD format", "INVALID_DATE_FORMAT")
                })?;
                Ok(Some(d))
            }
            _ => Ok(None),
        }
    };

    let date = parse_date(query.date)?;
    let from = parse_date(query.from)?;
    let to = parse_date(query.to)?;

    let result = leaderboard_service::get_leaderboard_history(
        &state.pool,
        query.period.as_deref(),
        date,
        from,
        to,
        query.page,
        query.limit,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": result
    })))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(global_leaderboard_handler))
        .route("/friends", get(friends_leaderboard_handler))
        .route("/history", get(leaderboard_history_handler))
}
