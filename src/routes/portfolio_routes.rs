use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::{
    error::AppError,
    middleware::auth::AuthUser,
    routes::auth_routes::ApiResponse,
    services::{
        portfolio_analytics::{
            AggregatePnlResponse, PnlHeatmapResponse, PortfolioAllocationResponse,
            PortfolioAnalyticsService, RealizedPnlResponse, RiskMetricsResponse,
            UnrealizedPnlResponse,
        },
        portfolio_service::{
            self, CashBalanceResponse, PortfolioSummary, SerializedHolding, SerializedPortfolio,
        },
    },
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct CreatePortfolioRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePortfolioRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RealizedPnlQuery {
    pub symbol: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UnrealizedPnlQuery {
    #[serde(rename = "rejectStale")]
    pub reject_stale: Option<bool>,
    #[serde(rename = "allowStale")]
    pub allow_stale: Option<bool>,
    #[serde(rename = "maxAgeSeconds")]
    pub max_age_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SnapshotsQuery {
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct AllocationQuery {
    pub dimension: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RiskMetricsQuery {
    pub period: Option<String>,
    #[serde(rename = "riskFreeRate")]
    pub risk_free_rate: Option<f64>,
}

async fn list_portfolios(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<ApiResponse<Vec<SerializedPortfolio>>>, AppError> {
    let portfolios = portfolio_service::list_user_portfolios(&state.pool, &auth.user_id).await?;
    Ok(Json(ApiResponse {
        success: true,
        data: portfolios,
    }))
}

async fn create_portfolio(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<CreatePortfolioRequest>,
) -> Result<Json<ApiResponse<SerializedPortfolio>>, AppError> {
    let portfolio = portfolio_service::create_portfolio(
        &state.pool,
        &auth.user_id,
        &payload.name,
        payload.description,
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: portfolio,
    }))
}

async fn get_portfolio(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
) -> Result<Json<ApiResponse<SerializedPortfolio>>, AppError> {
    let portfolio =
        portfolio_service::get_portfolio_by_id(&state.pool, &auth.user_id, &portfolio_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: portfolio,
    }))
}

async fn update_portfolio(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
    Json(payload): Json<UpdatePortfolioRequest>,
) -> Result<Json<ApiResponse<SerializedPortfolio>>, AppError> {
    let portfolio = portfolio_service::update_portfolio(
        &state.pool,
        &auth.user_id,
        &portfolio_id,
        payload.name,
        payload.description,
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: portfolio,
    }))
}

async fn delete_portfolio(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
) -> Result<Json<ApiResponse<SerializedPortfolio>>, AppError> {
    let portfolio =
        portfolio_service::deactivate_portfolio(&state.pool, &auth.user_id, &portfolio_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: portfolio,
    }))
}

async fn reactivate_portfolio(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
) -> Result<Json<ApiResponse<SerializedPortfolio>>, AppError> {
    let portfolio =
        portfolio_service::reactivate_portfolio(&state.pool, &auth.user_id, &portfolio_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: portfolio,
    }))
}

async fn get_portfolio_holdings(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<SerializedHolding>>>, AppError> {
    let holdings =
        portfolio_service::list_portfolio_holdings(&state.pool, &auth.user_id, &portfolio_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: holdings,
    }))
}

async fn get_cash_balance(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
) -> Result<Json<ApiResponse<CashBalanceResponse>>, AppError> {
    let balance =
        portfolio_service::get_portfolio_cash_balance(&state.pool, &auth.user_id, &portfolio_id)
            .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: balance,
    }))
}

async fn get_portfolio_summary(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
) -> Result<Json<ApiResponse<PortfolioSummary>>, AppError> {
    let summary =
        portfolio_service::get_portfolio_summary(&state.pool, &auth.user_id, &portfolio_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: summary,
    }))
}

async fn get_realized_pnl(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
    Query(query): Query<RealizedPnlQuery>,
) -> Result<Json<ApiResponse<RealizedPnlResponse>>, AppError> {
    let res = PortfolioAnalyticsService::get_realized_pnl(
        &state.pool,
        &auth.user_id,
        &portfolio_id,
        query.symbol.as_deref(),
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn get_unrealized_pnl(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
    Query(query): Query<UnrealizedPnlQuery>,
) -> Result<Json<ApiResponse<UnrealizedPnlResponse>>, AppError> {
    let res = PortfolioAnalyticsService::get_unrealized_pnl(
        &state,
        &auth.user_id,
        &portfolio_id,
        query.reject_stale.unwrap_or(false),
        query.allow_stale.unwrap_or(false),
        query.max_age_seconds,
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn get_aggregate_pnl(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
    Query(query): Query<UnrealizedPnlQuery>,
) -> Result<Json<ApiResponse<AggregatePnlResponse>>, AppError> {
    let res = PortfolioAnalyticsService::get_aggregate_pnl(
        &state,
        &auth.user_id,
        &portfolio_id,
        query.reject_stale.unwrap_or(false),
        query.allow_stale.unwrap_or(false),
        query.max_age_seconds,
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn get_portfolio_snapshots(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
    Query(query): Query<SnapshotsQuery>,
) -> Result<Json<ApiResponse<Vec<crate::models::PerformanceSnapshot>>>, AppError> {
    let limit = query.limit.unwrap_or(100).clamp(1, 1000);
    let res = PortfolioAnalyticsService::get_snapshots(&state.pool, &auth.user_id, &portfolio_id, limit).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn get_dashboard_summary(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let res = PortfolioAnalyticsService::get_dashboard(&state, &auth.user_id, &portfolio_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn get_allocation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
    Query(query): Query<AllocationQuery>,
) -> Result<Json<ApiResponse<PortfolioAllocationResponse>>, AppError> {
    let dim = query.dimension.as_deref().unwrap_or("symbol");
    let res = PortfolioAnalyticsService::get_allocation(&state, &auth.user_id, &portfolio_id, dim).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn get_risk_metrics(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
    Query(query): Query<RiskMetricsQuery>,
) -> Result<Json<ApiResponse<RiskMetricsResponse>>, AppError> {
    let period = query.period.as_deref().unwrap_or("1M");
    let rf = query.risk_free_rate.unwrap_or(0.02);
    let res = PortfolioAnalyticsService::get_risk_metrics(&state.pool, &auth.user_id, &portfolio_id, period, rf).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn get_pnl_heatmap(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
    Query(query): Query<RiskMetricsQuery>,
) -> Result<Json<ApiResponse<PnlHeatmapResponse>>, AppError> {
    let period = query.period.as_deref().unwrap_or("1M");
    let res = PortfolioAnalyticsService::get_pnl_heatmap(&state.pool, &auth.user_id, &portfolio_id, period).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_portfolios).post(create_portfolio))
        .route(
            "/{portfolioId}",
            get(get_portfolio)
                .patch(update_portfolio)
                .delete(delete_portfolio),
        )
        .route("/{portfolioId}/reactivate", post(reactivate_portfolio))
        .route("/{portfolioId}/holdings", get(get_portfolio_holdings))
        .route("/{portfolioId}/cash-balance", get(get_cash_balance))
        .route("/{portfolioId}/summary", get(get_portfolio_summary))
        .route("/{portfolioId}/realized-pnl", get(get_realized_pnl))
        .route("/{portfolioId}/unrealized-pnl", get(get_unrealized_pnl))
        .route("/{portfolioId}/pnl", get(get_aggregate_pnl))
        .route("/{portfolioId}/snapshots", get(get_portfolio_snapshots))
        .route("/{portfolioId}/dashboard", get(get_dashboard_summary))
        .route("/{portfolioId}/allocation", get(get_allocation))
        .route("/{portfolioId}/risk-metrics", get(get_risk_metrics))
        .route("/{portfolioId}/pnl-heatmap", get(get_pnl_heatmap))
}
