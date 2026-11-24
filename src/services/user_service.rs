use chrono::{DateTime, Datelike, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::{
    error::AppError,
    models::{Role, Trade},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct TradeSummary {
    pub symbol: String,
    #[serde(rename = "realizedPnl")]
    pub realized_pnl: Option<String>,
    #[serde(rename = "realizedPnlPct")]
    pub realized_pnl_pct: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserStatistics {
    #[serde(rename = "totalTrades")]
    pub total_trades: i64,
    #[serde(rename = "winningTrades")]
    pub winning_trades: i64,
    #[serde(rename = "losingTrades")]
    pub losing_trades: i64,
    #[serde(rename = "breakEvenTrades")]
    pub break_even_trades: i64,
    #[serde(rename = "winRate")]
    pub win_rate: f64,
    #[serde(rename = "totalRealizedPnl")]
    pub total_realized_pnl: String,
    #[serde(rename = "bestTrade")]
    pub best_trade: Option<TradeSummary>,
    #[serde(rename = "worstTrade")]
    pub worst_trade: Option<TradeSummary>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MonthlyTradeStatistics {
    pub month: String,
    pub year: i32,
    #[serde(rename = "totalTrades")]
    pub total_trades: i64,
    #[serde(rename = "winningTrades")]
    pub winning_trades: i64,
    #[serde(rename = "losingTrades")]
    pub losing_trades: i64,
    #[serde(rename = "winRate")]
    pub win_rate: f64,
    #[serde(rename = "realizedPnl")]
    pub realized_pnl: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PublicUserProfile {
    pub id: String,
    pub name: String,
    #[serde(rename = "avatarUrl")]
    pub avatar_url: Option<String>,
    pub role: Role,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "totalTrades")]
    pub total_trades: i64,
    #[serde(rename = "winRate")]
    pub win_rate: f64,
}

pub async fn get_user_statistics(
    pool: &PgPool,
    user_id: &str,
    symbol: Option<&str>,
) -> Result<UserStatistics, AppError> {
    let trades: Vec<Trade> = match symbol {
        Some(sym) => {
            sqlx::query_as(
                r#"
                SELECT t."id", t."portfolioId", t."orderId", t."symbol", t."assetType", t."side", t."quantity", t."price", t."totalValue", t."fees", t."realizedPnl", t."realizedPnlPct", t."executedAt", t."copyFanOutAt"
                FROM "Trade" t
                JOIN "Portfolio" p ON t."portfolioId" = p."id"
                WHERE p."userId" = $1 AND t."symbol" = $2 AND t."realizedPnl" IS NOT NULL
                ORDER BY t."executedAt" ASC
                "#
            )
            .bind(user_id)
            .bind(sym)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query_as(
                r#"
                SELECT t."id", t."portfolioId", t."orderId", t."symbol", t."assetType", t."side", t."quantity", t."price", t."totalValue", t."fees", t."realizedPnl", t."realizedPnlPct", t."executedAt", t."copyFanOutAt"
                FROM "Trade" t
                JOIN "Portfolio" p ON t."portfolioId" = p."id"
                WHERE p."userId" = $1 AND t."realizedPnl" IS NOT NULL
                ORDER BY t."executedAt" ASC
                "#
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?
        }
    };

    let total_trades = trades.len() as i64;
    let mut winning_trades = 0i64;
    let mut losing_trades = 0i64;
    let mut break_even_trades = 0i64;
    let mut total_pnl = Decimal::ZERO;

    let mut best_trade: Option<&Trade> = None;
    let mut worst_trade: Option<&Trade> = None;

    for trade in &trades {
        if let Some(pnl) = trade.realized_pnl {
            total_pnl += pnl;
            if pnl > Decimal::ZERO {
                winning_trades += 1;
            } else if pnl < Decimal::ZERO {
                losing_trades += 1;
            } else {
                break_even_trades += 1;
            }

            if best_trade.is_none() || best_trade.unwrap().realized_pnl.unwrap_or_default() < pnl {
                best_trade = Some(trade);
            }
            if worst_trade.is_none() || worst_trade.unwrap().realized_pnl.unwrap_or_default() > pnl {
                worst_trade = Some(trade);
            }
        }
    }

    let decided = winning_trades + losing_trades;
    let win_rate = if decided > 0 {
        ((winning_trades as f64 / decided as f64) * 10000.0).round() / 10000.0
    } else {
        0.0
    };

    Ok(UserStatistics {
        total_trades,
        winning_trades,
        losing_trades,
        break_even_trades,
        win_rate,
        total_realized_pnl: total_pnl.to_string(),
        best_trade: best_trade.map(|t| TradeSummary {
            symbol: t.symbol.clone(),
            realized_pnl: t.realized_pnl.map(|p| p.to_string()),
            realized_pnl_pct: t.realized_pnl_pct.map(|p| p.to_string()),
        }),
        worst_trade: worst_trade.map(|t| TradeSummary {
            symbol: t.symbol.clone(),
            realized_pnl: t.realized_pnl.map(|p| p.to_string()),
            realized_pnl_pct: t.realized_pnl_pct.map(|p| p.to_string()),
        }),
    })
}

pub async fn get_monthly_statistics(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<MonthlyTradeStatistics>, AppError> {
    let trades: Vec<Trade> = sqlx::query_as(
        r#"
        SELECT t."id", t."portfolioId", t."orderId", t."symbol", t."assetType", t."side", t."quantity", t."price", t."totalValue", t."fees", t."realizedPnl", t."realizedPnlPct", t."executedAt", t."copyFanOutAt"
        FROM "Trade" t
        JOIN "Portfolio" p ON t."portfolioId" = p."id"
        WHERE p."userId" = $1 AND t."realizedPnl" IS NOT NULL
        ORDER BY t."executedAt" ASC
        "#
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    use std::collections::BTreeMap;
    let mut months: BTreeMap<(i32, u32), (i64, i64, Decimal)> = BTreeMap::new();

    for trade in trades {
        let dt = trade.executed_at;
        let year = dt.year();
        let month = dt.month();
        let pnl = trade.realized_pnl.unwrap_or_default();

        let entry = months.entry((year, month)).or_insert((0, 0, Decimal::ZERO));
        if pnl > Decimal::ZERO {
            entry.0 += 1; // win
        } else if pnl < Decimal::ZERO {
            entry.1 += 1; // loss
        }
        entry.2 += pnl;
    }

    let mut result = Vec::new();
    for ((year, month), (wins, losses, pnl)) in months {
        let total = wins + losses;
        let win_rate = if total > 0 {
            ((wins as f64 / total as f64) * 10000.0).round() / 10000.0
        } else {
            0.0
        };

        result.push(MonthlyTradeStatistics {
            month: format!("{:02}", month),
            year,
            total_trades: total,
            winning_trades: wins,
            losing_trades: losses,
            win_rate,
            realized_pnl: pnl.to_string(),
        });
    }

    Ok(result)
}

pub async fn get_public_profile(
    pool: &PgPool,
    user_id: &str,
) -> Result<PublicUserProfile, AppError> {
    let row: Option<(String, String, Option<String>, Role, chrono::NaiveDateTime, bool)> = sqlx::query_as(
        r#"
        SELECT "id", "name", "avatarUrl", "role", "createdAt", "isSuspended"
        FROM "User"
        WHERE "id" = $1
        LIMIT 1
        "#
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    let (id, name, avatar_url, role, created_at, is_suspended) = match row {
        Some(r) => r,
        None => return Err(AppError::not_found("User not found.", "USER_NOT_FOUND")),
    };

    if is_suspended {
        return Err(AppError::forbidden(
            "Account is suspended.",
            "ACCOUNT_SUSPENDED",
        ));
    }

    let stats = get_user_statistics(pool, user_id, None).await?;

    Ok(PublicUserProfile {
        id,
        name,
        avatar_url,
        role,
        created_at: DateTime::from_naive_utc_and_offset(created_at, Utc),
        total_trades: stats.total_trades,
        win_rate: stats.win_rate,
    })
}
