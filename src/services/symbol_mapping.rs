use crate::error::AppError;
use axum::http::StatusCode;
use std::collections::HashSet;
use std::sync::LazyLock;

static KNOWN_AMBIGUOUS_STOCKS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "GBTC", "METH", "SOL", "COIN", "RIOT", "MARA", "HUT", "BITO", "ETHE", "OBTC",
        "FBTC", "IBIT", "ARKB", "BTCO", "EZBC", "BRRR", "HODL", "BTCW",
    ]
    .into_iter()
    .collect()
});

static MAJOR_CRYPTO_BASE_ASSETS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "BTC", "ETH", "SOL", "BNB", "XRP", "DOGE", "ADA", "AVAX", "DOT", "MATIC",
        "LINK", "LTC", "BCH", "UNI", "ATOM", "XLM", "NEAR", "ALGO",
    ]
    .into_iter()
    .collect()
});

static KNOWN_BASE_EXCEPTIONS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "1000SATS", "1000BONK", "1000PEPE", "1000FLOKI", "1000SHIB", "1000RATS",
        "1000XEC", "1000LUNC",
    ]
    .into_iter()
    .collect()
});

pub static CRYPTO_QUOTE_ASSETS: &[&str] = &[
    "FDUSD", "USDT", "USDC", "TUSD", "BUSD", "BTC", "ETH",
];

pub static BINANCE_QUOTE_ASSETS: &[&str] = &[
    "FDUSD", "RLUSD", "USDT", "USDC", "TUSD", "BUSD", "USDS", "USD1", "EURI", "USDP",
    "BTC", "ETH", "BNB", "XRP", "SOL", "TRY", "EUR", "ZAR", "BRL", "ARS", "JPY", "MXN",
    "COP", "IDR", "KZT", "AED", "THB", "USD", "U",
];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedSymbol {
    pub raw_symbol: String,
    pub canonical_symbol: String,
    pub asset_class: String, // "STOCK" or "CRYPTO"
    pub provider_symbol: String,
}

pub fn normalize_symbol(symbol: &str) -> String {
    symbol.trim().to_uppercase()
}

pub fn infer_market_region(symbol: &str) -> &'static str {
    let clean = symbol.trim().to_uppercase();
    if let Some(pos) = clean.rfind('.') {
        let suffix = &clean[pos + 1..];
        match suffix {
            "L" => "GB",
            "T" => "JP",
            "NS" | "BO" => "IN",
            "TO" | "V" => "CA",
            "AX" => "AU",
            "HK" => "HK",
            "DE" | "F" => "DE",
            "PA" => "FR",
            "AS" => "NL",
            "MI" => "IT",
            "MC" => "ES",
            "ST" => "SE",
            "OL" => "NO",
            "CO" => "DK",
            "HE" => "FI",
            "SW" => "CH",
            "SI" => "SG",
            "KS" | "KQ" => "KR",
            "TW" => "TW",
            "SS" | "SZ" => "CN",
            "SA" => "BR",
            "MX" => "MX",
            _ => "US",
        }
    } else {
        "US"
    }
}

pub fn resolve_symbol(
    raw: &str,
    explicit_asset_class: Option<&str>,
) -> Result<ResolvedSymbol, AppError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(AppError::market_error(
            StatusCode::BAD_REQUEST,
            "MARKET_DATA_INVALID_REQUEST",
            "A symbol is required.",
        ));
    }

    let upper = raw.to_uppercase();
    let is_binance_prefix = upper.starts_with("BINANCE:");
    let is_finnhub_prefix = upper.starts_with("FINNHUB:");

    let clean = if is_binance_prefix {
        &upper["BINANCE:".len()..]
    } else if is_finnhub_prefix {
        &upper["FINNHUB:".len()..]
    } else {
        &upper
    };

    let has_slash = clean.contains('/');
    let without_slash = clean.replace('/', "");

    let explicit = explicit_asset_class.map(|s| s.trim().to_uppercase());
    if let Some(ref class) = explicit {
        if class != "STOCK" && class != "CRYPTO" {
            return Err(AppError::market_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "MARKET_DATA_UNSUPPORTED_ASSET_CLASS",
                "Market data is not supported for this asset class.",
            ));
        }
        if is_finnhub_prefix && class == "CRYPTO" {
            return Err(AppError::market_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "MARKET_DATA_UNSUPPORTED_ASSET_CLASS",
                "Market data is not supported for this asset class.",
            ));
        }
        if is_binance_prefix && class == "STOCK" {
            return Err(AppError::market_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "MARKET_DATA_UNSUPPORTED_ASSET_CLASS",
                "Market data is not supported for this asset class.",
            ));
        }
    }

    let asset_class: String = if let Some(class) = explicit {
        class
    } else if is_binance_prefix || has_slash {
        "CRYPTO".to_string()
    } else if is_finnhub_prefix {
        "STOCK".to_string()
    } else {
        // Infer asset class
        if KNOWN_AMBIGUOUS_STOCKS.contains(clean) {
            "STOCK".to_string()
        } else if is_crypto_pair(&without_slash) {
            "CRYPTO".to_string()
        } else if MAJOR_CRYPTO_BASE_ASSETS.contains(clean)
            || KNOWN_BASE_EXCEPTIONS.contains(clean)
        {
            "CRYPTO".to_string()
        } else {
            "STOCK".to_string()
        }
    };

    let (canonical_symbol, provider_symbol) = if asset_class == "CRYPTO" {
        let pair = if has_slash {
            without_slash.clone()
        } else if (MAJOR_CRYPTO_BASE_ASSETS.contains(clean)
            || KNOWN_BASE_EXCEPTIONS.contains(clean)
            || clean.len() <= 5)
            && !is_crypto_pair(&without_slash)
        {
            format!("{}USDT", without_slash)
        } else {
            without_slash.clone()
        };

        if !is_binance_prefix && KNOWN_AMBIGUOUS_STOCKS.contains(clean) {
            return Err(AppError::market_error(
                StatusCode::BAD_REQUEST,
                "MARKET_DATA_INVALID_REQUEST",
                "The cryptocurrency symbol format is invalid.",
            ));
        }

        (pair.clone(), pair)
    } else {
        // STOCK
        (clean.to_string(), clean.to_string())
    };

    Ok(ResolvedSymbol {
        raw_symbol: raw.to_string(),
        canonical_symbol,
        asset_class,
        provider_symbol,
    })
}

fn is_crypto_pair(symbol: &str) -> bool {
    for quote in CRYPTO_QUOTE_ASSETS {
        if symbol.ends_with(quote) && symbol.len() > quote.len() + 1 {
            return true;
        }
    }
    false
}
