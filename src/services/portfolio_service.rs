use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    error::AppError,
    models::{AssetType, Holding, Portfolio},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct SerializedPortfolio {
    pub id: String,
    #[serde(rename = "userId")]
    pub user_id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "startingBalance")]
    pub starting_balance: String,
    #[serde(rename = "cashBalance")]
    pub cash_balance: String,
    #[serde(rename = "reservedCash")]
    pub reserved_cash: String,
    #[serde(rename = "availableCash")]
    pub available_cash: String,
    #[serde(rename = "isDefault")]
    pub is_default: bool,
    #[serde(rename = "isActive")]
    pub is_active: bool,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

impl From<Portfolio> for SerializedPortfolio {
    fn from(p: Portfolio) -> Self {
        let available = p.cash_balance - p.reserved_cash;
        Self {
            id: p.id,
            user_id: p.user_id,
            name: p.name,
            description: p.description,
            starting_balance: p.starting_balance.to_string(),
            cash_balance: p.cash_balance.to_string(),
            reserved_cash: p.reserved_cash.to_string(),
            available_cash: available.to_string(),
            is_default: p.is_default,
            is_active: p.is_active,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SerializedHolding {
    pub id: String,
    #[serde(rename = "portfolioId")]
    pub portfolio_id: String,
    pub symbol: String,
    #[serde(rename = "assetType")]
    pub asset_type: AssetType,
    pub quantity: String,
    #[serde(rename = "reservedQuantity")]
    pub reserved_quantity: String,
    #[serde(rename = "availableQuantity")]
    pub available_quantity: String,
    #[serde(rename = "averageBuyPrice")]
    pub average_buy_price: String,
    #[serde(rename = "totalInvested")]
    pub total_invested: String,
    #[serde(rename = "isShort")]
    pub is_short: bool,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PortfolioSummary {
    #[serde(flatten)]
    pub portfolio: SerializedPortfolio,
    #[serde(rename = "holdingsValue")]
    pub holdings_value: String,
    #[serde(rename = "totalValue")]
    pub total_value: String,
    #[serde(rename = "holdingsCount")]
    pub holdings_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CashBalanceResponse {
    #[serde(rename = "portfolioId")]
    pub portfolio_id: String,
    #[serde(rename = "cashBalance")]
    pub cash_balance: String,
    #[serde(rename = "reservedCash")]
    pub reserved_cash: String,
    #[serde(rename = "availableCash")]
    pub available_cash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PortfolioValuation {
    #[serde(rename = "portfolioId")]
    pub portfolio_id: String,
    #[serde(rename = "cashBalance")]
    pub cash_balance: String,
    #[serde(rename = "holdingsValue")]
    pub holdings_value: String,
    #[serde(rename = "totalValue")]
    pub total_value: String,
    pub pnl: String,
    #[serde(rename = "asOf")]
    pub as_of: String,
}

pub async fn list_user_portfolios(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<SerializedPortfolio>, AppError> {
    let portfolios: Vec<Portfolio> = sqlx::query_as(
        r#"
        SELECT "id", "userId", "name", "description", "startingBalance", "cashBalance", "reservedCash", "isDefault", "isActive", "createdAt", "updatedAt"
        FROM "Portfolio"
        WHERE "userId" = $1
        ORDER BY "isDefault" DESC, "createdAt" ASC
        "#
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(portfolios.into_iter().map(Into::into).collect())
}

pub async fn get_portfolio_by_id(
    pool: &PgPool,
    user_id: &str,
    portfolio_id: &str,
) -> Result<SerializedPortfolio, AppError> {
    let portfolio: Option<Portfolio> = sqlx::query_as(
        r#"
        SELECT "id", "userId", "name", "description", "startingBalance", "cashBalance", "reservedCash", "isDefault", "isActive", "createdAt", "updatedAt"
        FROM "Portfolio"
        WHERE "id" = $1 AND "userId" = $2
        LIMIT 1
        "#
    )
    .bind(portfolio_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    let p = portfolio.ok_or_else(|| {
        AppError::not_found("Portfolio not found.", "PORTFOLIO_NOT_FOUND")
    })?;

    Ok(p.into())
}

pub async fn create_portfolio(
    pool: &PgPool,
    user_id: &str,
    name: &str,
    description: Option<String>,
) -> Result<SerializedPortfolio, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::bad_request(
            "Portfolio name is required.",
            "PORTFOLIO_NAME_REQUIRED",
        ));
    }

    let existing: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT "id" FROM "Portfolio"
        WHERE "userId" = $1 AND "name" = $2
        LIMIT 1
        "#
    )
    .bind(user_id)
    .bind(trimmed)
    .fetch_optional(pool)
    .await?;

    if existing.is_some() {
        return Err(AppError::conflict(
            "A portfolio with this name already exists.",
            "PORTFOLIO_NAME_EXISTS",
        ));
    }

    let id = Uuid::new_v4().to_string();
    let balance = rust_decimal_macros::dec!(10000);

    let portfolio: Portfolio = sqlx::query_as(
        r#"
        INSERT INTO "Portfolio" ("id", "userId", "name", "description", "startingBalance", "cashBalance", "reservedCash", "isDefault", "isActive", "createdAt", "updatedAt")
        VALUES ($1, $2, $3, $4, $5, $5, 0, false, true, now(), now())
        RETURNING "id", "userId", "name", "description", "startingBalance", "cashBalance", "reservedCash", "isDefault", "isActive", "createdAt", "updatedAt"
        "#
    )
    .bind(id)
    .bind(user_id)
    .bind(trimmed)
    .bind(description.map(|d| d.trim().to_string()))
    .bind(balance)
    .fetch_one(pool)
    .await?;

    Ok(portfolio.into())
}

pub async fn update_portfolio(
    pool: &PgPool,
    user_id: &str,
    portfolio_id: &str,
    name: Option<String>,
    description: Option<String>,
) -> Result<SerializedPortfolio, AppError> {
    let current = get_portfolio_by_id(pool, user_id, portfolio_id).await?;

    let new_name = match name {
        Some(n) => {
            let trimmed = n.trim().to_string();
            if trimmed.is_empty() {
                return Err(AppError::bad_request(
                    "Portfolio name is required.",
                    "PORTFOLIO_NAME_REQUIRED",
                ));
            }
            trimmed
        }
        None => current.name,
    };

    let new_desc = description.map(|d| d.trim().to_string()).or(current.description);

    let updated: Portfolio = sqlx::query_as(
        r#"
        UPDATE "Portfolio"
        SET "name" = $3, "description" = $4, "updatedAt" = now()
        WHERE "id" = $1 AND "userId" = $2
        RETURNING "id", "userId", "name", "description", "startingBalance", "cashBalance", "reservedCash", "isDefault", "isActive", "createdAt", "updatedAt"
        "#
    )
    .bind(portfolio_id)
    .bind(user_id)
    .bind(new_name)
    .bind(new_desc)
    .fetch_one(pool)
    .await?;

    Ok(updated.into())
}

pub async fn deactivate_portfolio(
    pool: &PgPool,
    user_id: &str,
    portfolio_id: &str,
) -> Result<SerializedPortfolio, AppError> {
    let current = get_portfolio_by_id(pool, user_id, portfolio_id).await?;

    if current.is_default {
        return Err(AppError::conflict(
            "The default portfolio cannot be deleted.",
            "PORTFOLIO_DEFAULT",
        ));
    }

    if !current.is_active {
        return Ok(current);
    }

    let updated: Portfolio = sqlx::query_as(
        r#"
        UPDATE "Portfolio"
        SET "isActive" = false, "updatedAt" = now()
        WHERE "id" = $1 AND "userId" = $2
        RETURNING "id", "userId", "name", "description", "startingBalance", "cashBalance", "reservedCash", "isDefault", "isActive", "createdAt", "updatedAt"
        "#
    )
    .bind(portfolio_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    Ok(updated.into())
}

pub async fn reactivate_portfolio(
    pool: &PgPool,
    user_id: &str,
    portfolio_id: &str,
) -> Result<SerializedPortfolio, AppError> {
    let current = get_portfolio_by_id(pool, user_id, portfolio_id).await?;

    if current.is_active {
        return Ok(current);
    }

    let updated: Portfolio = sqlx::query_as(
        r#"
        UPDATE "Portfolio"
        SET "isActive" = true, "updatedAt" = now()
        WHERE "id" = $1 AND "userId" = $2
        RETURNING "id", "userId", "name", "description", "startingBalance", "cashBalance", "reservedCash", "isDefault", "isActive", "createdAt", "updatedAt"
        "#
    )
    .bind(portfolio_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    Ok(updated.into())
}

pub async fn get_portfolio_cash_balance(
    pool: &PgPool,
    user_id: &str,
    portfolio_id: &str,
) -> Result<CashBalanceResponse, AppError> {
    let portfolio = get_portfolio_by_id(pool, user_id, portfolio_id).await?;

    Ok(CashBalanceResponse {
        portfolio_id: portfolio.id,
        cash_balance: portfolio.cash_balance,
        reserved_cash: portfolio.reserved_cash,
        available_cash: portfolio.available_cash,
    })
}

pub async fn list_portfolio_holdings(
    pool: &PgPool,
    user_id: &str,
    portfolio_id: &str,
) -> Result<Vec<SerializedHolding>, AppError> {
    let _ = get_portfolio_by_id(pool, user_id, portfolio_id).await?;

    let holdings: Vec<Holding> = sqlx::query_as(
        r#"
        SELECT "id", "portfolioId", "symbol", "assetType", "quantity", "averageBuyPrice", "totalInvested", "isShort", "createdAt", "updatedAt"
        FROM "Holding"
        WHERE "portfolioId" = $1 AND "quantity" > 0
        ORDER BY "symbol" ASC
        "#
    )
    .bind(portfolio_id)
    .fetch_all(pool)
    .await?;

    let pending_sells: Vec<(String, AssetType, Decimal)> = sqlx::query_as(
        r#"
        SELECT "symbol", "assetType", "quantity"
        FROM "Order"
        WHERE "portfolioId" = $1 AND "side" = 'SELL' AND "status" = 'PENDING'
        "#
    )
    .bind(portfolio_id)
    .fetch_all(pool)
    .await?;

    let mut reserved_map: HashMap<(String, String), Decimal> = HashMap::new();
    for (sym, asset_type, qty) in pending_sells {
        let key = (sym, asset_type.as_str().to_string());
        *reserved_map.entry(key).or_default() += qty;
    }

    let mut result = Vec::new();
    for h in holdings {
        let key = (h.symbol.clone(), h.asset_type.as_str().to_string());
        let unallocated = reserved_map.get(&key).copied().unwrap_or(Decimal::ZERO);
        let row_reserved = if h.quantity > unallocated { unallocated } else { h.quantity };
        if let Some(val) = reserved_map.get_mut(&key) {
            *val = (*val - row_reserved).max(Decimal::ZERO);
        }
        let available = (h.quantity - row_reserved).max(Decimal::ZERO);

        result.push(SerializedHolding {
            id: h.id,
            portfolio_id: h.portfolio_id,
            symbol: h.symbol,
            asset_type: h.asset_type,
            quantity: h.quantity.to_string(),
            reserved_quantity: row_reserved.to_string(),
            available_quantity: available.to_string(),
            average_buy_price: h.average_buy_price.to_string(),
            total_invested: h.total_invested.to_string(),
            is_short: h.is_short,
            created_at: h.created_at,
            updated_at: h.updated_at,
        });
    }

    Ok(result)
}

pub async fn get_portfolio_summary(
    pool: &PgPool,
    user_id: &str,
    portfolio_id: &str,
) -> Result<PortfolioSummary, AppError> {
    let portfolio = get_portfolio_by_id(pool, user_id, portfolio_id).await?;

    let aggregate: (Option<Decimal>, i64) = sqlx::query_as(
        r#"
        SELECT SUM("totalInvested") as holdings_value, COUNT(*) as holdings_count
        FROM "Holding"
        WHERE "portfolioId" = $1 AND "quantity" > 0
        "#
    )
    .bind(portfolio_id)
    .fetch_one(pool)
    .await?;

    let holdings_value = aggregate.0.unwrap_or(Decimal::ZERO);
    let cash: Decimal = portfolio.cash_balance.parse().unwrap_or(Decimal::ZERO);
    let total_value = cash + holdings_value;

    Ok(PortfolioSummary {
        portfolio,
        holdings_value: holdings_value.to_string(),
        total_value: total_value.to_string(),
        holdings_count: aggregate.1,
    })
}

pub async fn get_portfolio_valuation(
    pool: &PgPool,
    portfolio_id: &str,
) -> Result<Option<PortfolioValuation>, AppError> {
    let p: Option<(Decimal, Decimal)> = sqlx::query_as(
        r#"
        SELECT "cashBalance", "startingBalance"
        FROM "Portfolio"
        WHERE "id" = $1
        LIMIT 1
        "#
    )
    .bind(portfolio_id)
    .fetch_optional(pool)
    .await?;

    let (cash_balance, starting_balance) = match p {
        Some(pair) => pair,
        None => return Ok(None),
    };

    let agg: (Option<Decimal>,) = sqlx::query_as(
        r#"
        SELECT SUM("totalInvested")
        FROM "Holding"
        WHERE "portfolioId" = $1 AND "quantity" > 0
        "#
    )
    .bind(portfolio_id)
    .fetch_one(pool)
    .await?;

    let holdings_value = agg.0.unwrap_or(Decimal::ZERO);
    let total_value = cash_balance + holdings_value;
    let pnl = total_value - starting_balance;

    Ok(Some(PortfolioValuation {
        portfolio_id: portfolio_id.to_string(),
        cash_balance: cash_balance.to_string(),
        holdings_value: holdings_value.to_string(),
        total_value: total_value.to_string(),
        pnl: pnl.to_string(),
        as_of: Utc::now().to_rfc3339(),
    }))
}
