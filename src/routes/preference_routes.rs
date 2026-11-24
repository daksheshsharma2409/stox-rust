use axum::{
    extract::State,
    response::IntoResponse,
    routing::get,
    Json, Router,
};

use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::services::user_preference_service::{self, UpdatePreferencesRequest};
use crate::state::AppState;

async fn get_preferences_handler(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let preferences = user_preference_service::get_preferences(&state.pool, &auth.user_id).await?;
    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "preferences": preferences
        }
    })))
}

async fn update_preferences_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<UpdatePreferencesRequest>,
) -> Result<impl IntoResponse, AppError> {
    let preferences =
        user_preference_service::update_preferences(&state.pool, &auth.user_id, payload).await?;
    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "preferences": preferences
        }
    })))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(get_preferences_handler).patch(update_preferences_handler))
}
