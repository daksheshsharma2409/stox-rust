use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::AppError,
    models::{
        AssetType, Order, OrderEventType, OrderSide, OrderStatus, OrderType, Trade,
    },
    services::order_service::{record_order_event_tx, Pagination},
};

const ZERO: Decimal = dec!(0);
const ONE_HUNDRED: Decimal = dec!(100);

#[derive(Debug, Serialize, Deserialize)]
pub struct SerializedTrade {
    pub id: String,
    #[serde(rename = "portfolioId")]
    pub portfolio_id: String,
    #[serde(rename = "orderId")]
    pub order_id: String,
    pub symbol: String,
    #[serde(rename = "assetType")]
    pub asset_type: AssetType,
    pub side: OrderSide,
    pub quantity: String,
    pub price: String,
    #[serde(rename = "totalValue")]
    pub total_value: String,
    pub fees: String,
    #[serde(rename = "realizedPnl")]
    pub realized_pnl: Option<String>,
    #[serde(rename = "realizedPnlPct")]
    pub realized_pnl_pct: Option<String>,
    #[serde(rename = "executedAt")]
    pub executed_at: DateTime<Utc>,
}

impl From<Trade> for SerializedTrade {
    fn from(t: Trade) -> Self {
        Self {
            id: t.id,
            portfolio_id: t.portfolio_id,
            order_id: t.order_id,
            symbol: t.symbol,
            asset_type: t.asset_type,
            side: t.side,
            quantity: t.quantity.to_string(),
            price: t.price.to_string(),
            total_value: t.total_value.to_string(),
            fees: t.fees.to_string(),
            realized_pnl: t.realized_pnl.map(|p| p.to_string()),
            realized_pnl_pct: t.realized_pnl_pct.map(|p| p.to_string()),
            executed_at: t.executed_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListTradesQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub symbol: Option<String>,
    pub side: Option<OrderSide>,
    #[serde(rename = "portfolioId")]
    pub portfolio_id: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub sort: Option<String>,
    pub order: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListTradesResponse {
    pub trades: Vec<SerializedTrade>,
    pub pagination: Pagination,
}

pub async fn list_trades(
    pool: &PgPool,
    user_id: &str,
    query: ListTradesQuery,
) -> Result<ListTradesResponse, AppError> {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * limit;

    let trades: Vec<Trade> = sqlx::query_as(
        r#"
        SELECT t."id", t."portfolioId", t."orderId", t."symbol", t."assetType", t."side",
               t."quantity", t."price", t."totalValue", t."fees", t."realizedPnl",
               t."realizedPnlPct", t."executedAt", t."copyFanOutAt"
        FROM "Trade" t
        JOIN "Portfolio" p ON t."portfolioId" = p."id"
        WHERE p."userId" = $1
          AND ($2::TEXT IS NULL OR t."symbol" = $2)
          AND ($3::"OrderSide" IS NULL OR t."side" = $3)
          AND ($4::TEXT IS NULL OR t."portfolioId" = $4)
          AND ($5::TIMESTAMPTZ IS NULL OR t."executedAt" >= $5)
          AND ($6::TIMESTAMPTZ IS NULL OR t."executedAt" < $6)
        ORDER BY t."executedAt" DESC
        LIMIT $7 OFFSET $8
        "#
    )
    .bind(user_id)
    .bind(query.symbol.as_deref())
    .bind(query.side)
    .bind(query.portfolio_id.as_deref())
    .bind(query.from)
    .bind(query.to)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let total: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM "Trade" t
        JOIN "Portfolio" p ON t."portfolioId" = p."id"
        WHERE p."userId" = $1
          AND ($2::TEXT IS NULL OR t."symbol" = $2)
          AND ($3::"OrderSide" IS NULL OR t."side" = $3)
          AND ($4::TEXT IS NULL OR t."portfolioId" = $4)
          AND ($5::TIMESTAMPTZ IS NULL OR t."executedAt" >= $5)
          AND ($6::TIMESTAMPTZ IS NULL OR t."executedAt" < $6)
        "#
    )
    .bind(user_id)
    .bind(query.symbol.as_deref())
    .bind(query.side)
    .bind(query.portfolio_id.as_deref())
    .bind(query.from)
    .bind(query.to)
    .fetch_one(pool)
    .await?;

    let total_count = total.0;
    let total_pages = (total_count + limit - 1) / limit;

    Ok(ListTradesResponse {
        trades: trades.into_iter().map(Into::into).collect(),
        pagination: Pagination {
            page,
            limit,
            total: total_count,
            total_pages,
        },
    })
}

pub async fn get_trade_by_id(
    pool: &PgPool,
    user_id: &str,
    trade_id: &str,
) -> Result<SerializedTrade, AppError> {
    let trade: Option<Trade> = sqlx::query_as(
        r#"
        SELECT t."id", t."portfolioId", t."orderId", t."symbol", t."assetType", t."side",
               t."quantity", t."price", t."totalValue", t."fees", t."realizedPnl",
               t."realizedPnlPct", t."executedAt", t."copyFanOutAt"
        FROM "Trade" t
        JOIN "Portfolio" p ON t."portfolioId" = p."id"
        WHERE t."id" = $1 AND p."userId" = $2
        LIMIT 1
        "#
    )
    .bind(trade_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    let t = trade.ok_or_else(|| AppError::not_found("Trade not found.", "TRADE_NOT_FOUND"))?;
    Ok(t.into())
}

pub fn evaluate_order(order: &Order, current_price: Decimal) -> bool {
    if current_price <= ZERO {
        return false;
    }

    match order.order_type {
        OrderType::Market => true,
        OrderType::Limit => {
            if let Some(limit_price) = order.limit_price {
                if limit_price <= ZERO {
                    return false;
                }
                match order.side {
                    OrderSide::Buy => current_price <= limit_price,
                    OrderSide::Sell => current_price >= limit_price,
                }
            } else {
                false
            }
        }
        OrderType::StopLoss => {
            if let Some(stop_price) = order.stop_price {
                stop_price > ZERO && current_price <= stop_price
            } else {
                false
            }
        }
        OrderType::TakeProfit => {
            if let Some(stop_price) = order.stop_price {
                stop_price > ZERO && current_price >= stop_price
            } else {
                false
            }
        }
    }
}

pub async fn execute_order(
    pool: &PgPool,
    order_id: &str,
    execution_price: Decimal,
    now: DateTime<Utc>,
    fees: Decimal,
) -> Result<Trade, AppError> {
    if execution_price <= ZERO {
        return Err(AppError::bad_request(
            "Execution price must be greater than zero.",
            "INVALID_EXECUTION_PRICE",
        ));
    }

    let mut tx = pool.begin().await?;

    let order: Option<Order> = sqlx::query_as(
        r#"
        SELECT "id", "portfolioId", "symbol", "assetType", "side", "type", "status",
               "quantity", "reservedCash", "limitPrice", "stopPrice", "executedPrice",
               "executedAt", "expiresAt", "executionAttempts", "lastExecutionAttemptAt",
               "notes", "source", "copyTradeExecutionId", "createdAt", "updatedAt"
        FROM "Order"
        WHERE "id" = $1
        FOR UPDATE
        "#
    )
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?;

    let order = match order {
        Some(o) => o,
        None => return Err(AppError::not_found("Order not found.", "ORDER_NOT_FOUND")),
    };

    if order.status != OrderStatus::Pending {
        return Err(AppError::bad_request(
            "Order is not executable.",
            "ORDER_NOT_EXECUTABLE",
        ));
    }

    if let Some(exp) = order.expires_at {
        if exp <= now {
            return Err(AppError::bad_request(
                "Order has expired.",
                "ORDER_EXPIRED",
            ));
        }
    }

    // Lock portfolio FOR UPDATE
    let portfolio: Option<(String, Decimal, Decimal, bool)> = sqlx::query_as(
        r#"
        SELECT "id", "cashBalance", "reservedCash", "isActive"
        FROM "Portfolio"
        WHERE "id" = $1
        FOR UPDATE
        "#
    )
    .bind(&order.portfolio_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (portfolio_id, cash_balance, portfolio_reserved_cash, is_active) = match portfolio {
        Some(p) => p,
        None => return Err(AppError::not_found("Portfolio not found.", "PORTFOLIO_NOT_FOUND")),
    };

    if !is_active {
        return Err(AppError::bad_request(
            "Order is not executable.",
            "ORDER_NOT_EXECUTABLE",
        ));
    }

    let total_value = (order.quantity * execution_price).round_dp(8);
    let mut realized_pnl: Option<Decimal> = None;
    let mut realized_pnl_pct: Option<Decimal> = None;

    if order.side == OrderSide::Buy {
        let total_cash_outlay = total_value + fees;
        let additional_required = (total_cash_outlay - order.reserved_cash).max(ZERO);
        let available_cash = cash_balance - portfolio_reserved_cash;

        if available_cash < additional_required {
            return Err(AppError::bad_request(
                "Insufficient available portfolio cash.",
                "INSUFFICIENT_BALANCE",
            ));
        }

        let cost_basis = total_value + fees;
        let average_buy_price = cost_basis / order.quantity;
        let holding_id = Uuid::new_v4().to_string();

        sqlx::query(
            r#"
            INSERT INTO "Holding" ("id", "portfolioId", "symbol", "assetType", "quantity", "averageBuyPrice", "totalInvested", "isShort", "createdAt", "updatedAt")
            VALUES ($1, $2, $3, $4, $5, $6, $7, false, now(), now())
            ON CONFLICT ("portfolioId", "symbol") DO UPDATE SET
                "quantity" = "Holding"."quantity" + EXCLUDED."quantity",
                "totalInvested" = "Holding"."totalInvested" + EXCLUDED."totalInvested",
                "averageBuyPrice" = ("Holding"."totalInvested" + EXCLUDED."totalInvested") / ("Holding"."quantity" + EXCLUDED."quantity"),
                "isShort" = false,
                "updatedAt" = now()
            "#
        )
        .bind(holding_id)
        .bind(&portfolio_id)
        .bind(&order.symbol)
        .bind(order.asset_type)
        .bind(order.quantity)
        .bind(average_buy_price)
        .bind(cost_basis)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            UPDATE "Portfolio"
            SET "cashBalance" = "cashBalance" - $2,
                "reservedCash" = GREATEST(0, "reservedCash" - $3),
                "updatedAt" = now()
            WHERE "id" = $1
            "#
        )
        .bind(&portfolio_id)
        .bind(total_cash_outlay)
        .bind(order.reserved_cash)
        .execute(&mut *tx)
        .await?;
    } else {
        // SELL side
        let holding: Option<(String, Decimal, Decimal)> = sqlx::query_as(
            r#"
            SELECT "id", "quantity", "totalInvested"
            FROM "Holding"
            WHERE "portfolioId" = $1 AND "symbol" = $2 AND "quantity" >= $3
            FOR UPDATE
            "#
        )
        .bind(&portfolio_id)
        .bind(&order.symbol)
        .bind(order.quantity)
        .fetch_optional(&mut *tx)
        .await?;

        let (holding_id, current_qty, current_total_invested) = match holding {
            Some(h) => h,
            None => {
                return Err(AppError::bad_request(
                    "Insufficient position to sell.",
                    "INSUFFICIENT_POSITION",
                ))
            }
        };

        let is_full_close = current_qty == order.quantity;
        let new_qty = if is_full_close { ZERO } else { current_qty - order.quantity };
        let new_total_invested = if is_full_close {
            ZERO
        } else {
            (current_total_invested - (current_total_invested * order.quantity / current_qty)).round_dp(8)
        };

        let removed_cost_basis = current_total_invested - new_total_invested;
        let pnl = total_value - removed_cost_basis - fees;
        let pnl_pct = if removed_cost_basis > ZERO {
            Some(((pnl / removed_cost_basis) * ONE_HUNDRED).round_dp(4))
        } else {
            None
        };

        realized_pnl = Some(pnl);
        realized_pnl_pct = pnl_pct;

        sqlx::query(
            r#"
            UPDATE "Holding"
            SET "quantity" = $2, "totalInvested" = $3, "updatedAt" = now()
            WHERE "id" = $1
            "#
        )
        .bind(holding_id)
        .bind(new_qty)
        .bind(new_total_invested)
        .execute(&mut *tx)
        .await?;

        let net_cash_credit = total_value - fees;
        sqlx::query(
            r#"
            UPDATE "Portfolio"
            SET "cashBalance" = "cashBalance" + $2, "updatedAt" = now()
            WHERE "id" = $1
            "#
        )
        .bind(&portfolio_id)
        .bind(net_cash_credit)
        .execute(&mut *tx)
        .await?;
    }

    // Transition order to EXECUTED
    sqlx::query(
        r#"
        UPDATE "Order"
        SET "status" = 'EXECUTED', "executedPrice" = $2, "executedAt" = $3, "reservedCash" = 0, "updatedAt" = now()
        WHERE "id" = $1
        "#
    )
    .bind(order_id)
    .bind(execution_price)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    let trade_id = Uuid::new_v4().to_string();
    let trade: Trade = sqlx::query_as(
        r#"
        INSERT INTO "Trade" (
            "id", "portfolioId", "orderId", "symbol", "assetType", "side",
            "quantity", "price", "totalValue", "fees", "realizedPnl",
            "realizedPnlPct", "executedAt"
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        RETURNING "id", "portfolioId", "orderId", "symbol", "assetType", "side",
                  "quantity", "price", "totalValue", "fees", "realizedPnl",
                  "realizedPnlPct", "executedAt", "copyFanOutAt"
        "#
    )
    .bind(trade_id)
    .bind(&portfolio_id)
    .bind(order_id)
    .bind(&order.symbol)
    .bind(order.asset_type)
    .bind(order.side)
    .bind(order.quantity)
    .bind(execution_price)
    .bind(total_value)
    .bind(fees)
    .bind(realized_pnl)
    .bind(realized_pnl_pct)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;

    record_order_event_tx(
        &mut tx,
        order_id,
        OrderEventType::Executed,
        Some(OrderStatus::Pending),
        OrderStatus::Executed,
        Some(serde_json::json!({
            "tradeId": trade.id,
            "executionPrice": execution_price.to_string(),
            "quantity": order.quantity.to_string(),
            "totalValue": total_value.to_string(),
            "realizedPnl": realized_pnl.map(|p| p.to_string()),
            "realizedPnlPct": realized_pnl_pct.map(|p| p.to_string()),
            "fees": fees.to_string()
        })),
    )
    .await?;

    tx.commit().await?;

    Ok(trade)
}

pub async fn expire_overdue_orders(pool: &PgPool, now: DateTime<Utc>) -> Result<u64, AppError> {
    let overdue: Vec<(String, String, OrderSide, Decimal)> = sqlx::query_as(
        r#"
        SELECT "id", "portfolioId", "side", "reservedCash"
        FROM "Order"
        WHERE "status" = 'PENDING' AND "expiresAt" IS NOT NULL AND "expiresAt" <= $1
        LIMIT 100
        "#
    )
    .bind(now)
    .fetch_all(pool)
    .await?;

    let mut expired_count = 0u64;

    for (order_id, portfolio_id, side, reserved_cash) in overdue {
        let mut tx = pool.begin().await?;

        if side == OrderSide::Buy && reserved_cash > ZERO {
            sqlx::query(
                r#"
                UPDATE "Portfolio"
                SET "reservedCash" = GREATEST(0, "reservedCash" - $2), "updatedAt" = now()
                WHERE "id" = $1
                "#
            )
            .bind(&portfolio_id)
            .bind(reserved_cash)
            .execute(&mut *tx)
            .await?;
        }

        let transition = sqlx::query(
            r#"
            UPDATE "Order"
            SET "status" = 'EXPIRED', "reservedCash" = 0, "updatedAt" = now()
            WHERE "id" = $1 AND "status" = 'PENDING'
            "#
        )
        .bind(&order_id)
        .execute(&mut *tx)
        .await?;

        if transition.rows_affected() > 0 {
            record_order_event_tx(
                &mut tx,
                &order_id,
                OrderEventType::Expired,
                Some(OrderStatus::Pending),
                OrderStatus::Expired,
                Some(serde_json::json!({
                    "releasedReservedCash": reserved_cash.to_string(),
                    "reason": "ORDER_EXPIRED"
                })),
            )
            .await?;

            tx.commit().await?;
            expired_count += 1;
        }
    }

    Ok(expired_count)
}
