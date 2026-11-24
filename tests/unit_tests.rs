use chrono::{Duration, Utc};
use rust_decimal::Decimal;

// Test symbol mapping logic
#[test]
fn test_symbol_resolution() {
    use stox_backend_rust::services::symbol_mapping::resolve_symbol;

    // Binance crypto prefix
    let res = resolve_symbol("BINANCE:BTCUSDT", None).expect("Should resolve");
    assert_eq!(res.asset_class, "CRYPTO");
    assert_eq!(res.canonical_symbol, "BTCUSDT");

    // Finnhub stock prefix
    let res = resolve_symbol("FINNHUB:AAPL", None).expect("Should resolve");
    assert_eq!(res.asset_class, "STOCK");
    assert_eq!(res.canonical_symbol, "AAPL");

    // Ambiguous crypto-related stocks
    let res = resolve_symbol("GBTC", None).expect("Should resolve");
    assert_eq!(res.asset_class, "STOCK");

    let res = resolve_symbol("COIN", None).expect("Should resolve");
    assert_eq!(res.asset_class, "STOCK");

    let res = resolve_symbol("RIOT", None).expect("Should resolve");
    assert_eq!(res.asset_class, "STOCK");

    // Crypto pair without prefix
    let res = resolve_symbol("ETHUSDT", None).expect("Should resolve");
    assert_eq!(res.asset_class, "CRYPTO");

    // Stock without prefix
    let res = resolve_symbol("MSFT", None).expect("Should resolve");
    assert_eq!(res.asset_class, "STOCK");
}

// Test league status calculation
#[test]
fn test_effective_league_status() {
    use stox_backend_rust::models::LeagueStatus;
    use stox_backend_rust::services::competition_service::get_effective_league_status;

    let now = Utc::now();
    let future_start = now + Duration::days(2);
    let future_end = now + Duration::days(5);
    let past_start = now - Duration::days(5);
    let past_end = now - Duration::days(1);

    // Upcoming
    assert_eq!(
        get_effective_league_status(LeagueStatus::Upcoming, future_start, future_end, now),
        LeagueStatus::Upcoming
    );

    // Active
    assert_eq!(
        get_effective_league_status(LeagueStatus::Upcoming, past_start, future_end, now),
        LeagueStatus::Active
    );

    // Completed
    assert_eq!(
        get_effective_league_status(LeagueStatus::Active, past_start, past_end, now),
        LeagueStatus::Completed
    );

    // Cancelled always stays Cancelled
    assert_eq!(
        get_effective_league_status(LeagueStatus::Cancelled, future_start, future_end, now),
        LeagueStatus::Cancelled
    );
}

// Test error envelope format
#[test]
fn test_app_error_serialization() {
    use stox_backend_rust::error::AppError;

    let err = AppError::not_found("Resource not found", "NOT_FOUND");
    assert_eq!(err.status_code.as_u16(), 404);
    assert_eq!(err.code, "NOT_FOUND");
    assert_eq!(err.message, "Resource not found");

    let bad_req = AppError::bad_request("Invalid input", "BAD_REQUEST");
    assert_eq!(bad_req.status_code.as_u16(), 400);
}

// Test decimal precision calculations
#[test]
fn test_financial_precision() {
    let price = Decimal::from_str_exact("12345.67890123").unwrap();
    let quantity = Decimal::from_str_exact("0.00500000").unwrap();
    let total = price * quantity;
    assert_eq!(total.to_string(), "61.7283945061500000");
}

// Test sentiment classification and thresholds
#[test]
fn test_sentiment_rules() {
    use stox_backend_rust::services::sentiment_service::MIN_SAMPLE_THRESHOLD;
    assert_eq!(MIN_SAMPLE_THRESHOLD, 5);
}
