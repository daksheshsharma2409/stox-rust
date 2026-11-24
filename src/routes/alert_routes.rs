use axum::{
    extract::{Path, Query, State},
    routing::{get, patch},
    Json, Router,
};
use serde::Deserialize;

use crate::{
    error::AppError,
    middleware::auth::AuthUser,
    models::enums::AlertStatus,
    routes::auth_routes::ApiResponse,
    services::alert_service::{
        AlertService, CreateAlertPayload, ListAlertsResponse, SerializedAlert, UpdateAlertPayload,
    },
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct ListAlertsQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub status: Option<AlertStatus>,
    pub symbol: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_alerts).post(create_alert))
        .route("/{alertId}", patch(update_alert).delete(delete_alert))
}

async fn list_alerts(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListAlertsQuery>,
) -> Result<Json<ApiResponse<ListAlertsResponse>>, AppError> {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);

    let res = AlertService::list_alerts(
        &state.pool,
        &auth.user_id,
        page,
        limit,
        query.status,
        query.symbol,
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn create_alert(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<CreateAlertPayload>,
) -> Result<Json<ApiResponse<SerializedAlert>>, AppError> {
    let res = AlertService::create_alert(&state, &auth.user_id, payload).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn update_alert(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(alert_id): Path<String>,
    Json(payload): Json<UpdateAlertPayload>,
) -> Result<Json<ApiResponse<SerializedAlert>>, AppError> {
    let res = AlertService::update_alert(&state, &auth.user_id, &alert_id, payload).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn delete_alert(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(alert_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let res = AlertService::delete_alert(&state.pool, &auth.user_id, &alert_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}
