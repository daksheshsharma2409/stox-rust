use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    error::AppError,
    services::symbol_mapping::{
        normalize_symbol, resolve_symbol, ResolvedSymbol, BINANCE_QUOTE_ASSETS,
    },
    state::AppState,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketQuote {
    pub symbol: String,
    pub asset_class: String,
    pub price: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_percent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub high: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub low: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_close: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    pub source: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_stale: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_future: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freshness: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalCandle {
    pub timestamp: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalPricesResponse {
    pub symbol: String,
    pub asset_class: String,
    pub interval: String,
    pub prices: Vec<HistoricalCandle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketOverviewMover {
    pub symbol: String,
    pub price: String,
    pub change_percent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_volume: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketStatus {
    pub is_open: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holiday: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exchange: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketOverviewResponse {
    pub asset_class: String,
    pub as_of: String,
    pub market_status: MarketStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub movers: Option<Vec<MarketOverviewMover>>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultItem {
    pub symbol: String,
    pub name: String,
    pub asset_class: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exchange: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Default)]
pub struct QuoteOptions {
    pub asset_class: Option<String>,
    pub reject_stale: bool,
    pub max_age_seconds: Option<i64>,
    pub bypass_cache: bool,
}

pub struct MarketDataService;

impl MarketDataService {
    pub async fn get_quote(
        state: &AppState,
        symbol: &str,
        options: &QuoteOptions,
    ) -> Result<MarketQuote, AppError> {
        let resolved = resolve_symbol(symbol, options.asset_class.as_deref())?;
        let cache_key = format!(
            "stox:v1:price:{}:{}",
            resolved.asset_class,
            normalize_symbol(&resolved.canonical_symbol)
        );

        // Try Redis cache if not bypassed
        if !options.bypass_cache {
            if let Some(ref client) = state.redis_client {
                if let Ok(mut con) = client.get_multiplexed_async_connection().await {
                    let cached: Result<String, _> = redis::cmd("GET")
                        .arg(&cache_key)
                        .query_async(&mut con)
                        .await;
                    if let Ok(json_str) = cached {
                        if let Ok(mut quote) = serde_json::from_str::<MarketQuote>(&json_str) {
                            Self::decorate_and_assert_freshness(
                                &mut quote,
                                options.reject_stale,
                                options.max_age_seconds,
                            )?;
                            return Ok(quote);
                        }
                    }
                }
            }
        }

        // Fetch from upstream provider
        let mut quote = match resolved.asset_class.as_str() {
            "CRYPTO" => Self::fetch_binance_quote(state, &resolved).await?,
            _ => Self::fetch_stock_quote(state, &resolved).await?,
        };

        // Cache quote in Redis
        if let Some(ref client) = state.redis_client {
            if let Ok(mut con) = client.get_multiplexed_async_connection().await {
                if let Ok(json_str) = serde_json::to_string(&quote) {
                    let _: Result<(), _> = redis::cmd("SET")
                        .arg(&cache_key)
                        .arg(&json_str)
                        .arg("EX")
                        .arg(state.config.cache_ttl_price_seconds)
                        .query_async(&mut con)
                        .await;
                }
            }
        }

        Self::decorate_and_assert_freshness(
            &mut quote,
            options.reject_stale,
            options.max_age_seconds,
        )?;

        Ok(quote)
    }

    pub async fn get_quotes(
        state: &AppState,
        symbols: &[String],
        asset_types: &HashMap<String, String>,
    ) -> Result<Vec<MarketQuote>, AppError> {
        let mut results = Vec::new();
        for symbol in symbols {
            let asset_class = asset_types
                .get(symbol)
                .or_else(|| asset_types.get(&symbol.to_uppercase()))
                .cloned();

            let options = QuoteOptions {
                asset_class,
                bypass_cache: false,
                reject_stale: false,
                max_age_seconds: None,
            };

            if let Ok(quote) = Self::get_quote(state, symbol, &options).await {
                results.push(quote);
            }
        }
        Ok(results)
    }

    async fn fetch_binance_quote(
        state: &AppState,
        resolved: &ResolvedSymbol,
    ) -> Result<MarketQuote, AppError> {
        let url = format!(
            "{}/api/v3/ticker/24hr?symbol={}",
            state.config.binance_api_url, resolved.provider_symbol
        );

        let resp = state
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| map_reqwest_error(e, "binance"))?;

        if resp.status() == StatusCode::NOT_FOUND || resp.status() == StatusCode::BAD_REQUEST {
            return Err(AppError::market_error(
                StatusCode::NOT_FOUND,
                "MARKET_DATA_NOT_FOUND",
                "Market data is unavailable for the requested symbol or range.",
            ));
        }

        if !resp.status().is_success() {
            return Err(AppError::market_error(
                StatusCode::BAD_GATEWAY,
                "MARKET_DATA_INVALID_RESPONSE",
                "Market data provider returned an invalid response.",
            ));
        }

        let json: serde_json::Value = resp.json().await.map_err(|_| {
            AppError::market_error(
                StatusCode::BAD_GATEWAY,
                "MARKET_DATA_INVALID_RESPONSE",
                "Market data provider returned an invalid response.",
            )
        })?;

        let price = json["lastPrice"]
            .as_str()
            .ok_or_else(|| invalid_response("Missing lastPrice"))?
            .to_string();

        let previous_close = json["prevClosePrice"]
            .as_str()
            .filter(|&v| v != "0" && v != "0.00000000")
            .map(String::from);

        let currency = BINANCE_QUOTE_ASSETS
            .iter()
            .find(|&&q| resolved.provider_symbol.ends_with(q))
            .map(|&q| q.to_string())
            .unwrap_or_else(|| "USD".to_string());

        let timestamp = json["closeTime"]
            .as_i64()
            .and_then(|ms| DateTime::from_timestamp_millis(ms))
            .map(|dt| dt.to_rfc3339());

        Ok(MarketQuote {
            symbol: format!("BINANCE:{}", resolved.provider_symbol),
            asset_class: "CRYPTO".to_string(),
            price,
            change: json["priceChange"].as_str().map(String::from),
            change_percent: json["priceChangePercent"].as_str().map(String::from),
            high: json["highPrice"].as_str().map(String::from),
            low: json["lowPrice"].as_str().map(String::from),
            open: json["openPrice"].as_str().map(String::from),
            previous_close,
            volume: json["volume"].as_str().map(String::from),
            currency: Some(currency),
            timestamp,
            source: "binance".to_string(),
            is_stale: None,
            is_future: None,
            freshness: None,
            age_seconds: None,
        })
    }

    async fn fetch_stock_quote(
        state: &AppState,
        resolved: &ResolvedSymbol,
    ) -> Result<MarketQuote, AppError> {
        let api_key = match state.config.finnhub_api_key.as_deref() {
            Some(key) if !key.is_empty() => key,
            _ => {
                // If Finnhub is unconfigured, try Yahoo finance quote or return 503
                return Self::fetch_yahoo_quote(state, resolved).await;
            }
        };

        let url = format!(
            "{}/quote?symbol={}",
            state.config.finnhub_api_url, resolved.provider_symbol
        );

        let resp = state
            .http_client
            .get(&url)
            .header("X-Finnhub-Token", api_key)
            .send()
            .await
            .map_err(|e| map_reqwest_error(e, "finnhub"))?;

        if !resp.status().is_success() {
            if resp.status() == StatusCode::TOO_MANY_REQUESTS {
                return Err(AppError::market_error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "MARKET_DATA_RATE_LIMITED",
                    "Market data provider rate limit reached.",
                ));
            }
            return Self::fetch_yahoo_quote(state, resolved).await;
        }

        let json: serde_json::Value = resp.json().await.map_err(|_| {
            AppError::market_error(
                StatusCode::BAD_GATEWAY,
                "MARKET_DATA_INVALID_RESPONSE",
                "Market data provider returned an invalid response.",
            )
        })?;

        let c = json["c"].as_f64().unwrap_or(0.0);
        let t = json["t"].as_i64().unwrap_or(0);

        if c == 0.0 && t == 0 {
            // Finnhub returns { c: 0, d: null, dp: null, h: 0, l: 0, o: 0, pc: 0, t: 0 } for unknown symbol
            return Self::fetch_yahoo_quote(state, resolved).await;
        }

        let timestamp = if t > 0 {
            DateTime::from_timestamp(t, 0).map(|dt| dt.to_rfc3339())
        } else {
            None
        };

        Ok(MarketQuote {
            symbol: resolved.canonical_symbol.clone(),
            asset_class: "STOCK".to_string(),
            price: format!("{:.4}", c).trim_end_matches('0').trim_end_matches('.').to_string(),
            change: json["d"].as_f64().map(|v| format!("{:.4}", v)),
            change_percent: json["dp"].as_f64().map(|v| format!("{:.4}", v)),
            high: json["h"].as_f64().filter(|&v| v > 0.0).map(|v| format!("{:.4}", v)),
            low: json["l"].as_f64().filter(|&v| v > 0.0).map(|v| format!("{:.4}", v)),
            open: json["o"].as_f64().filter(|&v| v > 0.0).map(|v| format!("{:.4}", v)),
            previous_close: json["pc"].as_f64().filter(|&v| v > 0.0).map(|v| format!("{:.4}", v)),
            volume: None,
            currency: Some("USD".to_string()),
            timestamp,
            source: "finnhub".to_string(),
            is_stale: None,
            is_future: None,
            freshness: None,
            age_seconds: None,
        })
    }

    async fn fetch_yahoo_quote(
        state: &AppState,
        resolved: &ResolvedSymbol,
    ) -> Result<MarketQuote, AppError> {
        let url = format!(
            "{}/v8/finance/chart/{}?interval=1d&range=1d",
            state.config.yahoo_finance_api_url, resolved.provider_symbol
        );

        let resp = state
            .http_client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (compatible; Stox/1.0)")
            .send()
            .await
            .map_err(|e| map_reqwest_error(e, "yahoo"))?;

        if resp.status() == StatusCode::NOT_FOUND {
            return Err(AppError::market_error(
                StatusCode::NOT_FOUND,
                "MARKET_DATA_NOT_FOUND",
                "Market data is unavailable for the requested symbol or range.",
            ));
        }

        if !resp.status().is_success() {
            return Err(AppError::market_error(
                StatusCode::BAD_GATEWAY,
                "MARKET_DATA_INVALID_RESPONSE",
                "Stock history provider returned an invalid response.",
            ));
        }

        let json: serde_json::Value = resp.json().await.map_err(|_| {
            AppError::market_error(
                StatusCode::BAD_GATEWAY,
                "MARKET_DATA_INVALID_RESPONSE",
                "Stock history provider returned an invalid response.",
            )
        })?;

        let meta = &json["chart"]["result"][0]["meta"];
        let price = meta["regularMarketPrice"]
            .as_f64()
            .ok_or_else(|| {
                AppError::market_error(
                    StatusCode::NOT_FOUND,
                    "MARKET_DATA_NOT_FOUND",
                    "Market data is unavailable for the requested symbol or range.",
                )
            })?;

        let previous_close = meta["chartPreviousClose"].as_f64();
        let currency = meta["currency"].as_str().map(|s| s.to_string());
        let t = meta["regularMarketTime"].as_i64().unwrap_or_else(|| Utc::now().timestamp());

        let timestamp = DateTime::from_timestamp(t, 0).map(|dt| dt.to_rfc3339());

        let change = previous_close.map(|pc| price - pc);
        let change_percent = previous_close.filter(|&pc| pc > 0.0).map(|pc| ((price - pc) / pc) * 100.0);

        Ok(MarketQuote {
            symbol: resolved.canonical_symbol.clone(),
            asset_class: "STOCK".to_string(),
            price: format!("{:.4}", price),
            change: change.map(|v| format!("{:.4}", v)),
            change_percent: change_percent.map(|v| format!("{:.4}", v)),
            high: meta["regularMarketDayHigh"].as_f64().map(|v| format!("{:.4}", v)),
            low: meta["regularMarketDayLow"].as_f64().map(|v| format!("{:.4}", v)),
            open: None,
            previous_close: previous_close.map(|v| format!("{:.4}", v)),
            volume: meta["regularMarketVolume"].as_i64().map(|v| v.to_string()),
            currency,
            timestamp,
            source: "yahoo".to_string(),
            is_stale: None,
            is_future: None,
            freshness: None,
            age_seconds: None,
        })
    }

    pub async fn get_historical_prices(
        state: &AppState,
        symbol: &str,
        interval: &str,
        from_ms: i64,
        to_ms: i64,
        limit: usize,
        explicit_asset_class: Option<&str>,
    ) -> Result<HistoricalPricesResponse, AppError> {
        let resolved = resolve_symbol(symbol, explicit_asset_class)?;

        // Validate historical range
        let now_ms = Utc::now().timestamp_millis();
        if from_ms < 0 || from_ms >= to_ms || to_ms > now_ms {
            return Err(AppError::market_error(
                StatusCode::BAD_REQUEST,
                "MARKET_DATA_INVALID_REQUEST",
                "Historical range, interval, or limit is invalid or exceeds the supported window.",
            ));
        }

        let interval_ms = match interval {
            "1m" => 60_000,
            "5m" => 5 * 60_000,
            "15m" => 15 * 60_000,
            "30m" => 30 * 60_000,
            "1h" => 60 * 60_000,
            "4h" => 4 * 60 * 60_000,
            "1d" => 24 * 60 * 60_000,
            "1w" => 7 * 24 * 60 * 60_000,
            "1M" => 31 * 24 * 60 * 60_000,
            _ => {
                return Err(AppError::market_error(
                    StatusCode::BAD_REQUEST,
                    "MARKET_DATA_INVALID_REQUEST",
                    "Historical range, interval, or limit is invalid or exceeds the supported window.",
                ));
            }
        };

        if (to_ms - from_ms) > (interval_ms * limit as i64) {
            return Err(AppError::market_error(
                StatusCode::BAD_REQUEST,
                "MARKET_DATA_INVALID_REQUEST",
                "Historical range, interval, or limit is invalid or exceeds the supported window.",
            ));
        }

        if resolved.asset_class == "CRYPTO" {
            Self::fetch_binance_history(state, &resolved, interval, from_ms, to_ms, limit).await
        } else {
            Self::fetch_stock_history(state, &resolved, interval, from_ms, to_ms, limit).await
        }
    }

    async fn fetch_binance_history(
        state: &AppState,
        resolved: &ResolvedSymbol,
        interval: &str,
        from_ms: i64,
        to_ms: i64,
        limit: usize,
    ) -> Result<HistoricalPricesResponse, AppError> {
        let url = format!(
            "{}/api/v3/klines?symbol={}&interval={}&startTime={}&endTime={}&limit={}",
            state.config.binance_api_url,
            resolved.provider_symbol,
            interval,
            from_ms,
            to_ms,
            limit.min(1000)
        );

        let resp = state
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| map_reqwest_error(e, "binance"))?;

        if !resp.status().is_success() {
            return Err(AppError::market_error(
                StatusCode::BAD_GATEWAY,
                "MARKET_DATA_INVALID_RESPONSE",
                "Market data provider returned an invalid response.",
            ));
        }

        let raw_candles: Vec<serde_json::Value> = resp.json().await.map_err(|_| {
            AppError::market_error(
                StatusCode::BAD_GATEWAY,
                "MARKET_DATA_INVALID_RESPONSE",
                "Market data provider returned an invalid response.",
            )
        })?;

        let mut prices = Vec::new();
        for k in raw_candles {
            if let Some(arr) = k.as_array() {
                if arr.len() >= 6 {
                    let ts_ms = arr[0].as_i64().unwrap_or(0);
                    let open = arr[1].as_str().unwrap_or("0").to_string();
                    let high = arr[2].as_str().unwrap_or("0").to_string();
                    let low = arr[3].as_str().unwrap_or("0").to_string();
                    let close = arr[4].as_str().unwrap_or("0").to_string();
                    let volume = arr[5].as_str().unwrap_or("0").to_string();

                    if let Some(dt) = DateTime::from_timestamp_millis(ts_ms) {
                        prices.push(HistoricalCandle {
                            timestamp: dt.to_rfc3339(),
                            open,
                            high,
                            low,
                            close,
                            volume,
                        });
                    }
                }
            }
        }

        Ok(HistoricalPricesResponse {
            symbol: resolved.canonical_symbol.clone(),
            asset_class: "CRYPTO".to_string(),
            interval: interval.to_string(),
            prices,
        })
    }

    async fn fetch_stock_history(
        state: &AppState,
        resolved: &ResolvedSymbol,
        interval: &str,
        from_ms: i64,
        to_ms: i64,
        limit: usize,
    ) -> Result<HistoricalPricesResponse, AppError> {
        // Fallback to Yahoo Finance chart API
        let yahoo_interval = match interval {
            "1m" => "1m",
            "5m" => "5m",
            "15m" => "15m",
            "30m" => "30m",
            "1h" | "4h" => "60m",
            "1d" => "1d",
            "1w" => "1wk",
            "1M" => "1mo",
            _ => "1d",
        };

        let p1 = from_ms / 1000;
        let p2 = to_ms / 1000;

        let url = format!(
            "{}/v8/finance/chart/{}?interval={}&period1={}&period2={}",
            state.config.yahoo_finance_api_url, resolved.provider_symbol, yahoo_interval, p1, p2
        );

        let resp = state
            .http_client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (compatible; Stox/1.0)")
            .send()
            .await
            .map_err(|e| map_reqwest_error(e, "yahoo"))?;

        if !resp.status().is_success() {
            return Err(AppError::market_error(
                StatusCode::BAD_GATEWAY,
                "MARKET_DATA_INVALID_RESPONSE",
                "Stock history provider returned an invalid response.",
            ));
        }

        let json: serde_json::Value = resp.json().await.map_err(|_| {
            AppError::market_error(
                StatusCode::BAD_GATEWAY,
                "MARKET_DATA_INVALID_RESPONSE",
                "Stock history provider returned an invalid response.",
            )
        })?;

        let result = &json["chart"]["result"][0];
        let timestamps = result["timestamp"].as_array();
        let quote = &result["indicators"]["quote"][0];

        let mut prices = Vec::new();
        if let Some(ts_list) = timestamps {
            let opens = quote["open"].as_array();
            let highs = quote["high"].as_array();
            let lows = quote["low"].as_array();
            let closes = quote["close"].as_array();
            let volumes = quote["volume"].as_array();

            for (idx, ts_val) in ts_list.iter().enumerate() {
                if let Some(t_sec) = ts_val.as_i64() {
                    let o = opens.and_then(|a| a.get(idx)).and_then(|v| v.as_f64());
                    let h = highs.and_then(|a| a.get(idx)).and_then(|v| v.as_f64());
                    let l = lows.and_then(|a| a.get(idx)).and_then(|v| v.as_f64());
                    let c = closes.and_then(|a| a.get(idx)).and_then(|v| v.as_f64());
                    let v = volumes.and_then(|a| a.get(idx)).and_then(|v| v.as_f64()).unwrap_or(0.0);

                    if let (Some(o), Some(h), Some(l), Some(c)) = (o, h, l, c) {
                        if let Some(dt) = DateTime::from_timestamp(t_sec, 0) {
                            prices.push(HistoricalCandle {
                                timestamp: dt.to_rfc3339(),
                                open: format!("{:.4}", o),
                                high: format!("{:.4}", h),
                                low: format!("{:.4}", l),
                                close: format!("{:.4}", c),
                                volume: format!("{:.0}", v),
                            });
                        }
                    }
                }
            }
        }

        prices.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        if prices.len() > limit {
            prices = prices.split_off(prices.len() - limit);
        }

        Ok(HistoricalPricesResponse {
            symbol: resolved.canonical_symbol.clone(),
            asset_class: "STOCK".to_string(),
            interval: interval.to_string(),
            prices,
        })
    }

    pub async fn get_market_overview(
        state: &AppState,
        asset_class: &str,
        limit: usize,
    ) -> Result<MarketOverviewResponse, AppError> {
        let normalized_class = asset_class.trim().to_uppercase();
        if normalized_class == "CRYPTO" {
            let url = format!("{}/api/v3/ticker/24hr", state.config.binance_api_url);
            let resp = state
                .http_client
                .get(&url)
                .send()
                .await
                .map_err(|e| map_reqwest_error(e, "binance"))?;

            if !resp.status().is_success() {
                return Err(AppError::market_error(
                    StatusCode::BAD_GATEWAY,
                    "MARKET_DATA_INVALID_RESPONSE",
                    "Market data provider returned an invalid response.",
                ));
            }

            let tickers: Vec<serde_json::Value> = resp.json().await.map_err(|_| {
                AppError::market_error(
                    StatusCode::BAD_GATEWAY,
                    "MARKET_DATA_INVALID_RESPONSE",
                    "Market data provider returned an invalid response.",
                )
            })?;

            let mut movers = Vec::new();
            for item in tickers {
                if let (Some(sym), Some(price), Some(change_pct)) = (
                    item["symbol"].as_str(),
                    item["lastPrice"].as_str(),
                    item["priceChangePercent"].as_str(),
                ) {
                    movers.push(MarketOverviewMover {
                        symbol: format!("BINANCE:{}", sym.to_uppercase()),
                        price: price.to_string(),
                        change_percent: change_pct.to_string(),
                        quote_volume: item["quoteVolume"].as_str().map(String::from),
                    });
                }
            }

            movers.sort_by(|a, b| {
                let a_val: f64 = a.change_percent.parse().unwrap_or(0.0);
                let b_val: f64 = b.change_percent.parse().unwrap_or(0.0);
                b_val.partial_cmp(&a_val).unwrap_or(std::cmp::Ordering::Equal)
            });

            movers.truncate(limit);

            Ok(MarketOverviewResponse {
                asset_class: "CRYPTO".to_string(),
                as_of: Utc::now().to_rfc3339(),
                market_status: MarketStatus {
                    is_open: true,
                    session: Some("24/7".to_string()),
                    holiday: None,
                    exchange: Some("BINANCE".to_string()),
                    timezone: Some("UTC".to_string()),
                },
                movers: Some(movers),
                source: "binance".to_string(),
            })
        } else if normalized_class == "STOCK" {
            let mut is_open = true;
            let mut session = Some("regular".to_string());
            let mut holiday = None;
            let mut timezone = Some("America/New_York".to_string());

            if let Some(ref key) = state.config.finnhub_api_key {
                let url = format!("{}/stock/market-status?exchange=US", state.config.finnhub_api_url);
                if let Ok(resp) = state.http_client.get(&url).header("X-Finnhub-Token", key).send().await {
                    if let Ok(data) = resp.json::<serde_json::Value>().await {
                        if let Some(open) = data["isOpen"].as_bool() {
                            is_open = open;
                        }
                        session = data["session"].as_str().map(String::from);
                        holiday = data["holiday"].as_str().map(String::from);
                        timezone = data["timezone"].as_str().map(String::from);
                    }
                }
            }

            Ok(MarketOverviewResponse {
                asset_class: "STOCK".to_string(),
                as_of: Utc::now().to_rfc3339(),
                market_status: MarketStatus {
                    is_open,
                    session,
                    holiday,
                    exchange: Some("US".to_string()),
                    timezone,
                },
                movers: None,
                source: "finnhub".to_string(),
            })
        } else {
            Err(AppError::market_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "MARKET_DATA_UNSUPPORTED_ASSET_CLASS",
                "Market data is not supported for this asset class.",
            ))
        }
    }

    pub async fn search_symbols(
        state: &AppState,
        query: &str,
        asset_class: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchResultItem>, AppError> {
        let q = query.trim();
        if q.is_empty() {
            return Err(AppError::market_error(
                StatusCode::BAD_REQUEST,
                "MARKET_DATA_INVALID_REQUEST",
                "A symbol search query is required.",
            ));
        }

        let classes = match asset_class.map(|s| s.trim().to_uppercase()) {
            Some(ref c) if c == "STOCK" => vec!["STOCK"],
            Some(ref c) if c == "CRYPTO" => vec!["CRYPTO"],
            _ => vec!["STOCK", "CRYPTO"],
        };

        let mut all_results = Vec::new();
        for class in &classes {
            if *class == "STOCK" {
                if let Some(ref key) = state.config.finnhub_api_key {
                    let url = format!("{}/search?q={}", state.config.finnhub_api_url, q);
                    if let Ok(resp) = state.http_client.get(&url).header("X-Finnhub-Token", key).send().await {
                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                            if let Some(arr) = json["result"].as_array() {
                                for item in arr {
                                    if let (Some(sym), Some(desc)) = (
                                        item["symbol"].as_str().or_else(|| item["displaySymbol"].as_str()),
                                        item["description"].as_str(),
                                    ) {
                                        all_results.push(SearchResultItem {
                                            symbol: sym.to_uppercase(),
                                            name: desc.to_string(),
                                            asset_class: "STOCK".to_string(),
                                            exchange: item["exchange"].as_str().map(|s| s.to_uppercase()),
                                            currency: item["currency"].as_str().map(|s| s.to_uppercase()),
                                            r#type: item["type"].as_str().map(String::from),
                                            source: "finnhub".to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            } else if *class == "CRYPTO" {
                let url = format!("{}/api/v3/exchangeInfo", state.config.binance_api_url);
                if let Ok(resp) = state.http_client.get(&url).send().await {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if let Some(symbols) = json["symbols"].as_array() {
                            let upper_q = q.to_uppercase();
                            for s in symbols {
                                let sym = s["symbol"].as_str().unwrap_or("");
                                let base = s["baseAsset"].as_str().unwrap_or("");
                                let quote = s["quoteAsset"].as_str().unwrap_or("");
                                if sym.contains(&upper_q) || base.contains(&upper_q) {
                                    all_results.push(SearchResultItem {
                                        symbol: format!("BINANCE:{}", sym.to_uppercase()),
                                        name: format!("{}/{}", base, quote),
                                        asset_class: "CRYPTO".to_string(),
                                        exchange: Some("BINANCE".to_string()),
                                        currency: Some(quote.to_string()),
                                        r#type: Some("SPOT".to_string()),
                                        source: "binance".to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        all_results.truncate(limit);
        Ok(all_results)
    }

    fn decorate_and_assert_freshness(
        quote: &mut MarketQuote,
        reject_stale: bool,
        max_age_seconds: Option<i64>,
    ) -> Result<(), AppError> {
        let now = Utc::now();
        let threshold_seconds = if quote.asset_class == "CRYPTO" { 60 } else { 300 };
        let max_skew_seconds = 5;

        let (is_stale, is_future, freshness, age_seconds) = match quote.timestamp.as_deref() {
            Some(ts_str) => match DateTime::parse_from_rfc3339(ts_str) {
                Ok(dt) => {
                    let dt_utc = dt.with_timezone(&Utc);
                    let diff_secs = (now - dt_utc).num_seconds();
                    if diff_secs < -max_skew_seconds {
                        (true, true, "FUTURE".to_string(), None)
                    } else if diff_secs < 0 {
                        (false, false, "FRESH".to_string(), Some(0))
                    } else {
                        let stale = diff_secs > threshold_seconds;
                        let freshness = if stale { "STALE" } else { "FRESH" }.to_string();
                        (stale, false, freshness, Some(diff_secs))
                    }
                }
                Err(_) => (true, false, "STALE".to_string(), None),
            },
            None => (true, false, "STALE".to_string(), None),
        };

        quote.is_stale = Some(is_stale);
        quote.is_future = Some(is_future);
        quote.freshness = Some(freshness);
        quote.age_seconds = age_seconds;

        if reject_stale && is_stale {
            return Err(AppError::market_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "MARKET_DATA_STALE_QUOTE",
                "Market data quote is stale or delayed.",
            ));
        }

        if let Some(max_age) = max_age_seconds {
            if age_seconds.is_none() || age_seconds.unwrap() > max_age {
                return Err(AppError::market_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "MARKET_DATA_STALE_QUOTE",
                    "Market data quote is stale or delayed.",
                ));
            }
        }

        Ok(())
    }
}

fn map_reqwest_error(err: reqwest::Error, provider: &str) -> AppError {
    if err.is_timeout() {
        AppError::market_error(
            StatusCode::GATEWAY_TIMEOUT,
            "MARKET_DATA_TIMEOUT",
            format!("{} provider request timed out.", provider),
        )
    } else {
        AppError::market_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "MARKET_DATA_PROVIDER_UNAVAILABLE",
            format!("{} provider is temporarily unavailable.", provider),
        )
    }
}

fn invalid_response(msg: &str) -> AppError {
    AppError::market_error(
        StatusCode::BAD_GATEWAY,
        "MARKET_DATA_INVALID_RESPONSE",
        format!("Market data provider returned an invalid response: {}", msg),
    )
}
