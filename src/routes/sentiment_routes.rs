use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::Deserialize;

use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::models::Sentiment;
use crate::services::sentiment_service;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct VoteSentimentPayload {
    pub sentiment: Sentiment,
}

#[derive(Debug, Deserialize)]
pub struct SentimentQuery {
    pub window: Option<String>,
}

async fn vote_sentiment_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Json(payload): Json<VoteSentimentPayload>,
) -> Result<impl IntoResponse, AppError> {
    let vote = sentiment_service::record_vote(
        &state.pool,
        &auth.user_id,
        &symbol,
        payload.sentiment,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "vote": vote
        }
    })))
}

async fn get_sentiment_handler(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<SentimentQuery>,
) -> Result<impl IntoResponse, AppError> {
    let now = Utc::now();
    let sentiment = sentiment_service::get_sentiment(
        &state.pool,
        &symbol,
        query.window.as_deref(),
        None,
        now,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": sentiment
    })))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{symbol}/vote", post(vote_sentiment_handler))
        .route("/{symbol}", get(get_sentiment_handler))
}
