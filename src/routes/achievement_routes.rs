use axum::{
    extract::State,
    response::IntoResponse,
    routing::get,
    Json, Router,
};

use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::services::achievement_service;
use crate::state::AppState;

async fn list_achievements_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let achievements = achievement_service::list_achievements(&state.pool).await?;
    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "achievements": achievements
        }
    })))
}

async fn list_my_achievements_handler(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let achievements = achievement_service::list_user_achievements(&state.pool, &auth.user_id).await?;
    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "achievements": achievements
        }
    })))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_achievements_handler))
        .route("/mine", get(list_my_achievements_handler))
}
