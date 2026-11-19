use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::{get, patch},
    Json, Router,
};
use serde::Deserialize;

use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::services::notification_service;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListNotificationsQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    #[serde(rename = "isRead")]
    pub is_read: Option<bool>,
}

async fn list_notifications_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<ListNotificationsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let result = notification_service::list_notifications(
        &state.pool,
        &auth.user_id,
        query.page,
        query.limit,
        query.is_read,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": result
    })))
}

async fn mark_notification_read_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(notification_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let notification = notification_service::mark_notification_read(
        &state.pool,
        &auth.user_id,
        &notification_id,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "notification": notification
        }
    })))
}

async fn mark_all_read_handler(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let result =
        notification_service::mark_all_notifications_read(&state.pool, &auth.user_id).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": result
    })))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_notifications_handler))
        .route("/read-all", patch(mark_all_read_handler))
        .route("/{notificationId}/read", patch(mark_notification_read_handler))
}
