use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub port: u16,
    pub node_env: String,
    pub log_level: String,

    pub database_url: String,
    pub direct_url: Option<String>,
    pub redis_url: String,

    pub cache_ttl_price_seconds: u64,
    pub cache_ttl_portfolio_summary_seconds: u64,
    pub cache_ttl_leaderboard_seconds: u64,
    pub cache_ttl_historical_prices_seconds: u64,
    pub cache_ttl_symbol_seconds: u64,
    pub cache_ttl_session_seconds: u64,

    pub jwt_access_secret: String,
    pub jwt_refresh_secret: String,
    pub jwt_access_expiry: String,
    pub jwt_refresh_expiry: String,
    pub bcrypt_salt_rounds: u32,
    pub google_client_id: Option<String>,

    pub finnhub_api_key: Option<String>,
    pub finnhub_api_url: String,
    pub binance_api_url: String,
    pub yahoo_finance_api_url: String,

    pub cors_origin: String,

    pub rate_limit_enabled: bool,
    pub rate_limit_window_ms: u64,
    pub rate_limit_global_max: u32,
    pub rate_limit_auth_max: u32,
    pub rate_limit_order_window_ms: u64,
    pub rate_limit_order_max: u32,

    pub security_headers_enabled: bool,
    pub metrics_enabled: bool,
    pub queues_enabled: bool,
    pub queue_prefix: String,

    pub order_scan_interval_ms: u64,
    pub notification_scan_interval_ms: u64,
    pub alert_scan_interval_ms: u64,
    pub copy_trade_scan_interval_ms: u64,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, String> {
        let _ = dotenvy::dotenv();

        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8000);

        let database_url = env::var("DATABASE_URL")
            .map_err(|_| "DATABASE_URL must be set in environment".to_string())?;

        let redis_url = env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://localhost:6379".to_string());

        let jwt_access_secret = env::var("JWT_ACCESS_SECRET")
            .unwrap_or_else(|_| "stox-default-secret-access-key-development-minimum-32-chars".to_string());

        let jwt_refresh_secret = env::var("JWT_REFRESH_SECRET")
            .unwrap_or_else(|_| "stox-default-secret-refresh-key-development-minimum-32-chars".to_string());

        Ok(Self {
            port,
            node_env: env::var("NODE_ENV").unwrap_or_else(|_| "development".to_string()),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            database_url,
            direct_url: env::var("DIRECT_URL").ok(),
            redis_url,
            cache_ttl_price_seconds: env::var("CACHE_TTL_PRICE_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(5),
            cache_ttl_portfolio_summary_seconds: env::var("CACHE_TTL_PORTFOLIO_SUMMARY_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(30),
            cache_ttl_leaderboard_seconds: env::var("CACHE_TTL_LEADERBOARD_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(60),
            cache_ttl_historical_prices_seconds: env::var("CACHE_TTL_HISTORICAL_PRICES_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(3600),
            cache_ttl_symbol_seconds: env::var("CACHE_TTL_SYMBOL_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(86400),
            cache_ttl_session_seconds: env::var("CACHE_TTL_SESSION_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(604800),
            jwt_access_secret,
            jwt_refresh_secret,
            jwt_access_expiry: env::var("JWT_ACCESS_EXPIRY").unwrap_or_else(|_| "15m".to_string()),
            jwt_refresh_expiry: env::var("JWT_REFRESH_EXPIRY").unwrap_or_else(|_| "7d".to_string()),
            bcrypt_salt_rounds: env::var("BCRYPT_SALT_ROUNDS").ok().and_then(|v| v.parse().ok()).unwrap_or(10),
            google_client_id: env::var("GOOGLE_CLIENT_ID").ok(),
            finnhub_api_key: env::var("FINNHUB_API_KEY").ok(),
            finnhub_api_url: env::var("FINNHUB_API_URL").unwrap_or_else(|_| "https://finnhub.io/api/v1".to_string()),
            binance_api_url: env::var("BINANCE_API_URL").unwrap_or_else(|_| "https://data-api.binance.vision".to_string()),
            yahoo_finance_api_url: env::var("YAHOO_FINANCE_API_URL").unwrap_or_else(|_| "https://query1.finance.yahoo.com".to_string()),
            cors_origin: env::var("CORS_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".to_string()),
            rate_limit_enabled: env::var("RATE_LIMIT_ENABLED").map(|v| v == "true" || v == "1").unwrap_or(false),
            rate_limit_window_ms: env::var("RATE_LIMIT_WINDOW_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(900000),
            rate_limit_global_max: env::var("RATE_LIMIT_GLOBAL_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(100),
            rate_limit_auth_max: env::var("RATE_LIMIT_AUTH_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(10),
            rate_limit_order_window_ms: env::var("RATE_LIMIT_ORDER_WINDOW_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(60000),
            rate_limit_order_max: env::var("RATE_LIMIT_ORDER_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(30),
            security_headers_enabled: env::var("SECURITY_HEADERS_ENABLED").map(|v| v != "false").unwrap_or(true),
            metrics_enabled: env::var("METRICS_ENABLED").map(|v| v != "false").unwrap_or(true),
            queues_enabled: env::var("QUEUES_ENABLED").map(|v| v != "false").unwrap_or(true),
            queue_prefix: env::var("QUEUE_PREFIX").unwrap_or_else(|_| "stox".to_string()),
            order_scan_interval_ms: env::var("ORDER_SCAN_INTERVAL_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(5000),
            notification_scan_interval_ms: env::var("NOTIFICATION_SCAN_INTERVAL_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(30000),
            alert_scan_interval_ms: env::var("ALERT_SCAN_INTERVAL_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(15000),
            copy_trade_scan_interval_ms: env::var("COPY_TRADE_SCAN_INTERVAL_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(60000),
        })
    }
}
