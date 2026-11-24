use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::{
    error::AppError,
    middleware::auth::AuthUser,
    routes::auth_routes::ApiResponse,
    services::user_service::{self, MonthlyTradeStatistics, PublicUserProfile, UserStatistics},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct StatisticsQuery {
    pub symbol: Option<String>,
}

async fn get_my_statistics(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<StatisticsQuery>,
) -> Result<Json<ApiResponse<UserStatistics>>, AppError> {
    let stats = user_service::get_user_statistics(
        &state.pool,
        &auth.user_id,
        query.symbol.as_deref(),
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: stats,
    }))
}

async fn get_my_monthly_statistics(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<ApiResponse<Vec<MonthlyTradeStatistics>>>, AppError> {
    let stats = user_service::get_monthly_statistics(&state.pool, &auth.user_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: stats,
    }))
}

async fn get_user_profile(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<PublicUserProfile>>, AppError> {
    let profile = user_service::get_public_profile(&state.pool, &id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: profile,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me/statistics", get(get_my_statistics))
        .route("/me/statistics/monthly", get(get_my_monthly_statistics))
        .route("/{id}", get(get_user_profile))
}
