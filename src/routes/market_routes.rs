use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::collections::HashMap;

use crate::{
    error::AppError,
    services::market_data_service::{MarketDataService, QuoteOptions},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub query: String,
    #[serde(rename = "assetClass")]
    pub asset_class: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct QuotesQuery {
    pub symbols: String,
    #[serde(rename = "assetTypes")]
    pub asset_types: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct QuoteQuery {
    #[serde(rename = "rejectStale")]
    pub reject_stale: Option<bool>,
    #[serde(rename = "maxAgeSeconds")]
    pub max_age_seconds: Option<i64>,
    #[serde(rename = "bypassCache")]
    pub bypass_cache: Option<bool>,
    #[serde(rename = "skipCache")]
    pub skip_cache: Option<bool>,
    #[serde(rename = "noCache")]
    pub no_cache: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub interval: String,
    pub from: i64,
    pub to: i64,
    pub limit: Option<usize>,
    #[serde(rename = "assetClass")]
    pub asset_class: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OverviewQuery {
    #[serde(rename = "assetClass")]
    pub asset_class: String,
    pub limit: Option<usize>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/search", get(search_market_symbols))
        .route("/quotes", get(get_quotes))
        .route("/quote/{symbol}", get(get_crypto_quote))
        .route("/quote/stock/{symbol}", get(get_stock_quote))
        .route("/history/{symbol}", get(get_historical_prices))
        .route("/overview", get(get_market_overview))
}

async fn search_market_symbols(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<impl IntoResponse, AppError> {
    let q = query.query.trim();
    if q.len() < 2 || q.len() > 50 {
        return Err(AppError::bad_request(
            "Query length must be between 2 and 50 characters.",
            "VALIDATION_ERROR",
        ));
    }

    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let results = MarketDataService::search_symbols(
        &state,
        q,
        query.asset_class.as_deref(),
        limit,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": results
    })))
}

async fn get_quotes(
    State(state): State<AppState>,
    Query(query): Query<QuotesQuery>,
) -> Result<impl IntoResponse, AppError> {
    let symbols: Vec<String> = query
        .symbols
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if symbols.is_empty() {
        return Err(AppError::bad_request("symbols is required.", "VALIDATION_ERROR"));
    }
    if symbols.len() > 20 {
        return Err(AppError::bad_request(
            "At most 20 symbols may be requested at once.",
            "VALIDATION_ERROR",
        ));
    }

    let mut asset_types = HashMap::new();
    if let Some(types_str) = query.asset_types {
        let trimmed = types_str.trim();
        if trimmed.starts_with('{') {
            if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(trimmed) {
                asset_types = map;
            }
        } else {
            for item in trimmed.split(',') {
                if let Some((sym, cls)) = item.split_once(':') {
                    asset_types.insert(sym.trim().to_string(), cls.trim().to_uppercase());
                }
            }
        }
    }

    let quotes = MarketDataService::get_quotes(&state, &symbols, &asset_types).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "quotes": quotes
        }
    })))
}

async fn get_crypto_quote(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<QuoteQuery>,
) -> Result<impl IntoResponse, AppError> {
    let bypass = query.bypass_cache.unwrap_or(false)
        || query.skip_cache.unwrap_or(false)
        || query.no_cache.unwrap_or(false);

    let options = QuoteOptions {
        asset_class: Some("CRYPTO".to_string()),
        reject_stale: query.reject_stale.unwrap_or(false),
        max_age_seconds: query.max_age_seconds,
        bypass_cache: bypass,
    };

    let quote = MarketDataService::get_quote(&state, &symbol, &options).await?;

    let mut headers = HeaderMap::new();
    if let Some(ref freshness) = quote.freshness {
        if let Ok(val) = HeaderValue::from_str(freshness) {
            headers.insert("X-Quote-Freshness", val);
        }
    }
    if let Some(age) = quote.age_seconds {
        if let Ok(val) = HeaderValue::from_str(&age.to_string()) {
            headers.insert("X-Quote-Age-Seconds", val);
        }
    }

    Ok((
        StatusCode::OK,
        headers,
        Json(serde_json::json!({
            "success": true,
            "data": quote
        })),
    ))
}

async fn get_stock_quote(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<QuoteQuery>,
) -> Result<impl IntoResponse, AppError> {
    let bypass = query.bypass_cache.unwrap_or(false)
        || query.skip_cache.unwrap_or(false)
        || query.no_cache.unwrap_or(false);

    let options = QuoteOptions {
        asset_class: Some("STOCK".to_string()),
        reject_stale: query.reject_stale.unwrap_or(false),
        max_age_seconds: query.max_age_seconds,
        bypass_cache: bypass,
    };

    let quote = MarketDataService::get_quote(&state, &symbol, &options).await?;

    let mut headers = HeaderMap::new();
    if let Some(ref freshness) = quote.freshness {
        if let Ok(val) = HeaderValue::from_str(freshness) {
            headers.insert("X-Quote-Freshness", val);
        }
    }
    if let Some(age) = quote.age_seconds {
        if let Ok(val) = HeaderValue::from_str(&age.to_string()) {
            headers.insert("X-Quote-Age-Seconds", val);
        }
    }

    Ok((
        StatusCode::OK,
        headers,
        Json(serde_json::json!({
            "success": true,
            "data": quote
        })),
    ))
}

async fn get_historical_prices(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> Result<impl IntoResponse, AppError> {
    let limit = query.limit.unwrap_or(500).clamp(1, 1000);
    let history = MarketDataService::get_historical_prices(
        &state,
        &symbol,
        &query.interval,
        query.from,
        query.to,
        limit,
        query.asset_class.as_deref(),
    )
    .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": history
    })))
}

async fn get_market_overview(
    State(state): State<AppState>,
    Query(query): Query<OverviewQuery>,
) -> Result<impl IntoResponse, AppError> {
    let limit = query.limit.unwrap_or(10).clamp(1, 100);
    let overview = MarketDataService::get_market_overview(&state, &query.asset_class, limit).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": overview
    })))
}
