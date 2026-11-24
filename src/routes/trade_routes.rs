use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};

use crate::{
    error::AppError,
    middleware::auth::AuthUser,
    routes::auth_routes::ApiResponse,
    services::trade_service::{self, ListTradesQuery, ListTradesResponse, SerializedTrade},
    state::AppState,
};

async fn list_trades(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListTradesQuery>,
) -> Result<Json<ApiResponse<ListTradesResponse>>, AppError> {
    let response = trade_service::list_trades(&state.pool, &auth.user_id, query).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: response,
    }))
}

async fn get_trade(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(trade_id): Path<String>,
) -> Result<Json<ApiResponse<SerializedTrade>>, AppError> {
    let trade = trade_service::get_trade_by_id(&state.pool, &auth.user_id, &trade_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: trade,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_trades))
        .route("/{id}", get(get_trade))
}
