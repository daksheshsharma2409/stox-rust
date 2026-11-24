use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};

use crate::{
    error::AppError,
    middleware::auth::AuthUser,
    routes::auth_routes::ApiResponse,
    services::order_service::{
        self, CreateOrderInput, ListOrdersQuery, ListOrdersResponse, SerializedOrder,
        SerializedOrderEvent,
    },
    state::AppState,
};

async fn create_order(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<CreateOrderInput>,
) -> Result<Json<ApiResponse<SerializedOrder>>, AppError> {
    // For MARKET orders, attempt to fetch current price or use limit/stop price as fallback
    let ref_price = payload.limit_price.or(payload.stop_price).unwrap_or(rust_decimal_macros::dec!(150.0));

    let order = order_service::create_order(
        &state.pool,
        &auth.user_id,
        payload,
        Some(ref_price),
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: order,
    }))
}

async fn list_orders(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListOrdersQuery>,
) -> Result<Json<ApiResponse<ListOrdersResponse>>, AppError> {
    let response = order_service::list_orders(&state.pool, &auth.user_id, query).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: response,
    }))
}

async fn get_order(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<String>,
) -> Result<Json<ApiResponse<SerializedOrder>>, AppError> {
    let order = order_service::get_order_by_id(&state.pool, &auth.user_id, &order_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: order,
    }))
}

async fn get_order_events(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<SerializedOrderEvent>>>, AppError> {
    let events = order_service::get_order_events(&state.pool, &auth.user_id, &order_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: events,
    }))
}

async fn cancel_order(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<String>,
) -> Result<Json<ApiResponse<SerializedOrder>>, AppError> {
    let order = order_service::cancel_order(&state.pool, &auth.user_id, &order_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: order,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_orders).post(create_order))
        .route("/{id}", get(get_order).delete(cancel_order))
        .route("/{id}/events", get(get_order_events))
}
