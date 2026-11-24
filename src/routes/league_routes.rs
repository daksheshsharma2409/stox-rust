use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::Deserialize;

use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::services::competition_service::{self, CreateLeagueRequest};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListLeaguesQuery {
    pub status: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct JoinLeaguePayload {
    #[serde(rename = "portfolioId")]
    pub portfolio_id: Option<String>,
}

async fn create_league_handler(
    _auth: AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateLeagueRequest>,
) -> Result<impl IntoResponse, AppError> {
    let league = competition_service::create_league(&state.pool, payload).await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "data": {
                "league": league
            }
        })),
    ))
}

async fn list_leagues_handler(
    State(state): State<AppState>,
    Query(query): Query<ListLeaguesQuery>,
) -> Result<impl IntoResponse, AppError> {
    let now = Utc::now();
    let result = competition_service::list_leagues(
        &state.pool,
        query.status.as_deref(),
        query.page,
        query.limit,
        now,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": result.data,
        "pagination": result.pagination
    })))
}

async fn get_league_by_id_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let now = Utc::now();
    let league = competition_service::get_league_by_id(&state.pool, &id, now).await?;
    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "league": league
        }
    })))
}

async fn join_league_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    payload: Option<Json<JoinLeaguePayload>>,
) -> Result<impl IntoResponse, AppError> {
    let now = Utc::now();
    let portfolio_id = payload.and_then(|p| p.portfolio_id.clone());

    let member = competition_service::join_league(
        &state.pool,
        &id,
        &auth.user_id,
        portfolio_id,
        now,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "member": member
        }
    })))
}

async fn get_league_leaderboard_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let now = Utc::now();
    let result = competition_service::get_league_leaderboard(
        &state,
        &id,
        now,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": result
    })))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", post(create_league_handler).get(list_leagues_handler))
        .route("/{id}", get(get_league_by_id_handler))
        .route("/{id}/join", post(join_league_handler))
        .route("/{id}/leaderboard", get(get_league_leaderboard_handler))
}
