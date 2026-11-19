use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::AppError,
    models::{CopyTrader, Portfolio, User},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraderSummary {
    pub id: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioBasic {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializedCopyTrader {
    pub id: String,
    pub copier_id: String,
    pub trader_id: String,
    pub trader: Option<TraderSummary>,
    pub copy_portfolio_id: Option<String>,
    pub copy_portfolio: Option<PortfolioBasic>,
    pub allocation_pct: String,
    pub max_trade_size: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyTraderPagination {
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    #[serde(rename = "totalPages")]
    pub total_pages: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListCopyTradersResponse {
    pub copy_traders: Vec<SerializedCopyTrader>,
    pub pagination: CopyTraderPagination,
}

#[derive(Debug, Deserialize)]
pub struct CreateCopyTraderPayload {
    #[serde(rename = "traderId")]
    pub trader_id: String,
    #[serde(rename = "allocationPct", default = "default_allocation_pct")]
    pub allocation_pct: Decimal,
    #[serde(rename = "maxTradeSize")]
    pub max_trade_size: Option<Decimal>,
    #[serde(rename = "copyPortfolioId")]
    pub copy_portfolio_id: Option<String>,
    #[serde(rename = "isActive", default = "default_true")]
    pub is_active: bool,
}

fn default_allocation_pct() -> Decimal {
    Decimal::from(100)
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct UpdateCopyTraderPayload {
    #[serde(rename = "allocationPct")]
    pub allocation_pct: Option<Decimal>,
    #[serde(rename = "maxTradeSize")]
    pub max_trade_size: Option<Decimal>,
    #[serde(rename = "copyPortfolioId")]
    pub copy_portfolio_id: Option<String>,
    #[serde(rename = "isActive")]
    pub is_active: Option<bool>,
}

pub struct CopyTradingService;

impl CopyTradingService {
    async fn ensure_active_user(pool: &PgPool, user_id: &str) -> Result<User, AppError> {
        let user = sqlx::query_as::<_, User>(r#"SELECT * FROM "User" WHERE "id" = $1"#)
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::not_found("User not found.", "USER_NOT_FOUND"))?;

        if user.is_suspended {
            return Err(AppError::forbidden(
                "Account is suspended.",
                "ACCOUNT_SUSPENDED",
            ));
        }

        Ok(user)
    }

    async fn resolve_destination_portfolio(
        pool: &PgPool,
        user_id: &str,
        copy_portfolio_id: Option<&str>,
    ) -> Result<Portfolio, AppError> {
        if let Some(p_id) = copy_portfolio_id {
            let p = sqlx::query_as::<_, Portfolio>(
                r#"SELECT * FROM "Portfolio" WHERE "id" = $1 AND "userId" = $2"#,
            )
            .bind(p_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::not_found("Portfolio not found.", "PORTFOLIO_NOT_FOUND"))?;

            if !p.is_active {
                return Err(AppError::bad_request(
                    "Portfolio is inactive.",
                    "PORTFOLIO_INACTIVE",
                ));
            }

            Ok(p)
        } else {
            let p = sqlx::query_as::<_, Portfolio>(
                r#"
                SELECT * FROM "Portfolio"
                WHERE "userId" = $1 AND "isActive" = true
                ORDER BY "isDefault" DESC, "createdAt" ASC
                LIMIT 1
                "#,
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| {
                AppError::bad_request(
                    "An active portfolio is required to copy trades.",
                    "COPY_TRADING_PORTFOLIO_REQUIRED",
                )
            })?;

            Ok(p)
        }
    }

    pub async fn list_copy_traders(
        pool: &PgPool,
        user_id: &str,
        page: i64,
        limit: i64,
        is_active: Option<bool>,
    ) -> Result<ListCopyTradersResponse, AppError> {
        Self::ensure_active_user(pool, user_id).await?;

        let offset = (page - 1) * limit;

        let rows = sqlx::query_as::<_, CopyTrader>(
            r#"
            SELECT * FROM "CopyTrader"
            WHERE "copierId" = $1 AND ($2::boolean IS NULL OR "isActive" = $2)
            ORDER BY "createdAt" DESC, "id" DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(user_id)
        .bind(is_active)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let total_row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM "CopyTrader"
            WHERE "copierId" = $1 AND ($2::boolean IS NULL OR "isActive" = $2)
            "#,
        )
        .bind(user_id)
        .bind(is_active)
        .fetch_one(pool)
        .await?;

        let total = total_row.0;
        let mut copy_traders = Vec::new();

        for ct in rows {
            let trader = sqlx::query_as::<_, User>(r#"SELECT * FROM "User" WHERE "id" = $1"#)
                .bind(&ct.copied_id)
                .fetch_optional(pool)
                .await?;

            let portfolio = match ct.copy_portfolio_id.as_deref() {
                Some(p_id) => {
                    sqlx::query_as::<_, Portfolio>(r#"SELECT * FROM "Portfolio" WHERE "id" = $1"#)
                        .bind(p_id)
                        .fetch_optional(pool)
                        .await?
                }
                None => None,
            };

            copy_traders.push(SerializedCopyTrader {
                id: ct.id,
                copier_id: ct.copier_id,
                trader_id: ct.copied_id,
                trader: trader.map(|u| TraderSummary {
                    id: u.id,
                    name: u.name,
                    avatar_url: u.avatar_url,
                }),
                copy_portfolio_id: ct.copy_portfolio_id,
                copy_portfolio: portfolio.map(|p| PortfolioBasic {
                    id: p.id,
                    name: p.name,
                }),
                allocation_pct: ct.allocation_pct.to_string(),
                max_trade_size: ct.max_trade_size.map(|s| s.to_string()),
                is_active: ct.is_active,
                created_at: ct.created_at,
                updated_at: ct.updated_at,
            });
        }

        let total_pages = if limit > 0 {
            (total + limit - 1) / limit
        } else {
            1
        };

        Ok(ListCopyTradersResponse {
            copy_traders,
            pagination: CopyTraderPagination {
                page,
                limit,
                total,
                total_pages,
            },
        })
    }

    pub async fn create_copy_trader(
        pool: &PgPool,
        user_id: &str,
        payload: CreateCopyTraderPayload,
    ) -> Result<SerializedCopyTrader, AppError> {
        Self::ensure_active_user(pool, user_id).await?;

        if payload.trader_id == user_id {
            return Err(AppError::bad_request(
                "You cannot copy yourself.",
                "COPY_TRADING_SELF_REFERENCE",
            ));
        }

        let target_trader = sqlx::query_as::<_, User>(r#"SELECT * FROM "User" WHERE "id" = $1"#)
            .bind(&payload.trader_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::not_found("Trader not found.", "USER_NOT_FOUND"))?;

        if target_trader.is_suspended {
            return Err(AppError::bad_request(
                "Cannot copy a suspended trader.",
                "USER_SUSPENDED",
            ));
        }

        let portfolio = Self::resolve_destination_portfolio(
            pool,
            user_id,
            payload.copy_portfolio_id.as_deref(),
        )
        .await?;

        let ct_id = Uuid::new_v4().to_string();
        let copy_trader = sqlx::query_as::<_, CopyTrader>(
            r#"
            INSERT INTO "CopyTrader" (
                "id", "copierId", "copiedId", "copyPortfolioId", "allocationPct", "maxTradeSize", "isActive", "createdAt", "updatedAt"
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
            RETURNING *
            "#,
        )
        .bind(&ct_id)
        .bind(user_id)
        .bind(&payload.trader_id)
        .bind(&portfolio.id)
        .bind(payload.allocation_pct)
        .bind(payload.max_trade_size)
        .bind(payload.is_active)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.is_unique_violation() {
                    return AppError::conflict(
                        "This trader is already configured for copy trading.",
                        "COPY_TRADING_ALREADY_EXISTS",
                    );
                }
            }
            AppError::from(e)
        })?;

        Ok(SerializedCopyTrader {
            id: copy_trader.id,
            copier_id: copy_trader.copier_id,
            trader_id: copy_trader.copied_id,
            trader: Some(TraderSummary {
                id: target_trader.id,
                name: target_trader.name,
                avatar_url: target_trader.avatar_url,
            }),
            copy_portfolio_id: copy_trader.copy_portfolio_id,
            copy_portfolio: Some(PortfolioBasic {
                id: portfolio.id,
                name: portfolio.name,
            }),
            allocation_pct: copy_trader.allocation_pct.to_string(),
            max_trade_size: copy_trader.max_trade_size.map(|s| s.to_string()),
            is_active: copy_trader.is_active,
            created_at: copy_trader.created_at,
            updated_at: copy_trader.updated_at,
        })
    }

    pub async fn update_copy_trader(
        pool: &PgPool,
        user_id: &str,
        copy_trader_id: &str,
        payload: UpdateCopyTraderPayload,
    ) -> Result<SerializedCopyTrader, AppError> {
        Self::ensure_active_user(pool, user_id).await?;

        let existing = sqlx::query_as::<_, CopyTrader>(
            r#"SELECT * FROM "CopyTrader" WHERE "id" = $1 AND "copierId" = $2"#,
        )
        .bind(copy_trader_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| {
            AppError::not_found(
                "Copy-trading relationship not found.",
                "COPY_TRADING_NOT_FOUND",
            )
        })?;

        let target_trader = sqlx::query_as::<_, User>(r#"SELECT * FROM "User" WHERE "id" = $1"#)
            .bind(&existing.copied_id)
            .fetch_one(pool)
            .await?;

        let effective_allocation = payload.allocation_pct.unwrap_or(existing.allocation_pct);
        let effective_max_size = payload.max_trade_size.or(existing.max_trade_size);
        let effective_active = payload.is_active.unwrap_or(existing.is_active);

        let effective_portfolio_id = if let Some(ref p_id) = payload.copy_portfolio_id {
            let p = Self::resolve_destination_portfolio(pool, user_id, Some(p_id)).await?;
            Some(p.id)
        } else {
            existing.copy_portfolio_id
        };

        let updated = sqlx::query_as::<_, CopyTrader>(
            r#"
            UPDATE "CopyTrader"
            SET "allocationPct" = $1,
                "maxTradeSize" = $2,
                "copyPortfolioId" = $3,
                "isActive" = $4,
                "updatedAt" = NOW()
            WHERE "id" = $5 AND "copierId" = $6
            RETURNING *
            "#,
        )
        .bind(effective_allocation)
        .bind(effective_max_size)
        .bind(&effective_portfolio_id)
        .bind(effective_active)
        .bind(copy_trader_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let portfolio = match updated.copy_portfolio_id.as_deref() {
            Some(p_id) => {
                sqlx::query_as::<_, Portfolio>(r#"SELECT * FROM "Portfolio" WHERE "id" = $1"#)
                    .bind(p_id)
                    .fetch_optional(pool)
                    .await?
            }
            None => None,
        };

        Ok(SerializedCopyTrader {
            id: updated.id,
            copier_id: updated.copier_id,
            trader_id: updated.copied_id,
            trader: Some(TraderSummary {
                id: target_trader.id,
                name: target_trader.name,
                avatar_url: target_trader.avatar_url,
            }),
            copy_portfolio_id: updated.copy_portfolio_id,
            copy_portfolio: portfolio.map(|p| PortfolioBasic {
                id: p.id,
                name: p.name,
            }),
            allocation_pct: updated.allocation_pct.to_string(),
            max_trade_size: updated.max_trade_size.map(|s| s.to_string()),
            is_active: updated.is_active,
            created_at: updated.created_at,
            updated_at: updated.updated_at,
        })
    }

    pub async fn delete_copy_trader(
        pool: &PgPool,
        user_id: &str,
        copy_trader_id: &str,
    ) -> Result<SerializedCopyTrader, AppError> {
        Self::ensure_active_user(pool, user_id).await?;

        let _existing = sqlx::query_as::<_, CopyTrader>(
            r#"SELECT * FROM "CopyTrader" WHERE "id" = $1 AND "copierId" = $2"#,
        )
        .bind(copy_trader_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| {
            AppError::not_found(
                "Copy-trading relationship not found.",
                "COPY_TRADING_NOT_FOUND",
            )
        })?;

        // Soft delete: sets isActive = false
        let updated = sqlx::query_as::<_, CopyTrader>(
            r#"
            UPDATE "CopyTrader"
            SET "isActive" = false,
                "updatedAt" = NOW()
            WHERE "id" = $1 AND "copierId" = $2
            RETURNING *
            "#,
        )
        .bind(copy_trader_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let trader = sqlx::query_as::<_, User>(r#"SELECT * FROM "User" WHERE "id" = $1"#)
            .bind(&updated.copied_id)
            .fetch_optional(pool)
            .await?;

        let portfolio = match updated.copy_portfolio_id.as_deref() {
            Some(p_id) => {
                sqlx::query_as::<_, Portfolio>(r#"SELECT * FROM "Portfolio" WHERE "id" = $1"#)
                    .bind(p_id)
                    .fetch_optional(pool)
                    .await?
            }
            None => None,
        };

        Ok(SerializedCopyTrader {
            id: updated.id,
            copier_id: updated.copier_id,
            trader_id: updated.copied_id,
            trader: trader.map(|u| TraderSummary {
                id: u.id,
                name: u.name,
                avatar_url: u.avatar_url,
            }),
            copy_portfolio_id: updated.copy_portfolio_id,
            copy_portfolio: portfolio.map(|p| PortfolioBasic {
                id: p.id,
                name: p.name,
            }),
            allocation_pct: updated.allocation_pct.to_string(),
            max_trade_size: updated.max_trade_size.map(|s| s.to_string()),
            is_active: updated.is_active,
            created_at: updated.created_at,
            updated_at: updated.updated_at,
        })
    }
}
