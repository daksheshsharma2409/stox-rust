use axum::{
    extract::{Path, Query, State},
    routing::{get, patch},
    Json, Router,
};
use serde::Deserialize;

use crate::{
    error::AppError,
    middleware::auth::AuthUser,
    routes::auth_routes::ApiResponse,
    services::copy_trading_service::{
        CopyTradingService, CreateCopyTraderPayload, ListCopyTradersResponse, SerializedCopyTrader,
        UpdateCopyTraderPayload,
    },
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct ListCopyTradersQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    #[serde(rename = "isActive")]
    pub is_active: Option<bool>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_copy_traders).post(create_copy_trader))
        .route(
            "/{id}",
            patch(update_copy_trader).delete(delete_copy_trader),
        )
}

async fn list_copy_traders(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListCopyTradersQuery>,
) -> Result<Json<ApiResponse<ListCopyTradersResponse>>, AppError> {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);

    let res = CopyTradingService::list_copy_traders(
        &state.pool,
        &auth.user_id,
        page,
        limit,
        query.is_active,
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn create_copy_trader(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<CreateCopyTraderPayload>,
) -> Result<Json<ApiResponse<SerializedCopyTrader>>, AppError> {
    let res = CopyTradingService::create_copy_trader(&state.pool, &auth.user_id, payload).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn update_copy_trader(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(payload): Json<UpdateCopyTraderPayload>,
) -> Result<Json<ApiResponse<SerializedCopyTrader>>, AppError> {
    let res = CopyTradingService::update_copy_trader(&state.pool, &auth.user_id, &id, payload).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn delete_copy_trader(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<SerializedCopyTrader>>, AppError> {
    let res = CopyTradingService::delete_copy_trader(&state.pool, &auth.user_id, &id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}
