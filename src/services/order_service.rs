use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    error::AppError,
    models::{
        AssetType, LeagueStatus, Order, OrderEvent, OrderEventType, OrderSide, OrderSource,
        OrderStatus, OrderType,
    },
};

const ZERO: Decimal = dec!(0);
const MAX_MONEY: Decimal = dec!(9999999999.99999999);

#[derive(Debug, Serialize, Deserialize)]
pub struct SerializedOrder {
    pub id: String,
    #[serde(rename = "portfolioId")]
    pub portfolio_id: String,
    pub symbol: String,
    #[serde(rename = "assetType")]
    pub asset_type: AssetType,
    pub side: OrderSide,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    pub status: OrderStatus,
    pub quantity: String,
    #[serde(rename = "reservedCash")]
    pub reserved_cash: String,
    #[serde(rename = "limitPrice")]
    pub limit_price: Option<String>,
    #[serde(rename = "stopPrice")]
    pub stop_price: Option<String>,
    #[serde(rename = "executedPrice")]
    pub executed_price: Option<String>,
    #[serde(rename = "executedAt")]
    pub executed_at: Option<DateTime<Utc>>,
    #[serde(rename = "expiresAt")]
    pub expires_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub source: OrderSource,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

impl From<Order> for SerializedOrder {
    fn from(o: Order) -> Self {
        Self {
            id: o.id,
            portfolio_id: o.portfolio_id,
            symbol: o.symbol,
            asset_type: o.asset_type,
            side: o.side,
            order_type: o.order_type,
            status: o.status,
            quantity: o.quantity.to_string(),
            reserved_cash: o.reserved_cash.to_string(),
            limit_price: o.limit_price.map(|p| p.to_string()),
            stop_price: o.stop_price.map(|p| p.to_string()),
            executed_price: o.executed_price.map(|p| p.to_string()),
            executed_at: o.executed_at,
            expires_at: o.expires_at,
            notes: o.notes,
            source: o.source,
            created_at: o.created_at,
            updated_at: o.updated_at,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SerializedOrderEvent {
    pub id: String,
    #[serde(rename = "orderId")]
    pub order_id: String,
    pub sequence: i32,
    #[serde(rename = "eventType")]
    pub event_type: OrderEventType,
    #[serde(rename = "previousStatus")]
    pub previous_status: Option<OrderStatus>,
    #[serde(rename = "newStatus")]
    pub new_status: OrderStatus,
    pub metadata: Option<serde_json::Value>,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
}

impl From<OrderEvent> for SerializedOrderEvent {
    fn from(e: OrderEvent) -> Self {
        Self {
            id: e.id,
            order_id: e.order_id,
            sequence: e.sequence,
            event_type: e.event_type,
            previous_status: e.previous_status,
            new_status: e.new_status,
            metadata: e.metadata,
            created_at: e.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateOrderInput {
    #[serde(rename = "portfolioId")]
    pub portfolio_id: String,
    pub symbol: String,
    #[serde(rename = "assetType")]
    pub asset_type: AssetType,
    pub side: OrderSide,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    pub quantity: Decimal,
    #[serde(rename = "limitPrice")]
    pub limit_price: Option<Decimal>,
    #[serde(rename = "stopPrice")]
    pub stop_price: Option<Decimal>,
    #[serde(rename = "expiresAt")]
    pub expires_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub source: Option<OrderSource>,
    #[serde(rename = "copyTradeExecutionId")]
    pub copy_trade_execution_id: Option<String>,
}

pub async fn record_order_event_tx(
    tx: &mut Transaction<'_, Postgres>,
    order_id: &str,
    event_type: OrderEventType,
    previous_status: Option<OrderStatus>,
    new_status: OrderStatus,
    metadata: Option<serde_json::Value>,
) -> Result<OrderEvent, AppError> {
    let max_seq: Option<(Option<i32>,)> = sqlx::query_as(
        r#"
        SELECT MAX("sequence")
        FROM "OrderEvent"
        WHERE "orderId" = $1
        "#
    )
    .bind(order_id)
    .fetch_optional(&mut **tx)
    .await?;

    let next_seq = max_seq.and_then(|(s,)| s).unwrap_or(0) + 1;
    let event_id = Uuid::new_v4().to_string();

    let event: OrderEvent = sqlx::query_as(
        r#"
        INSERT INTO "OrderEvent" ("id", "orderId", "sequence", "eventType", "previousStatus", "newStatus", "metadata", "createdAt")
        VALUES ($1, $2, $3, $4, $5, $6, $7, now())
        RETURNING "id", "orderId", "sequence", "eventType", "previousStatus", "newStatus", "metadata", "createdAt"
        "#
    )
    .bind(event_id)
    .bind(order_id)
    .bind(next_seq)
    .bind(event_type)
    .bind(previous_status)
    .bind(new_status)
    .bind(metadata)
    .fetch_one(&mut **tx)
    .await?;

    Ok(event)
}

pub async fn create_order(
    pool: &PgPool,
    user_id: &str,
    input: CreateOrderInput,
    reference_market_price: Option<Decimal>,
) -> Result<SerializedOrder, AppError> {
    if input.quantity <= ZERO {
        return Err(AppError::bad_request(
            "Quantity must be greater than zero.",
            "INVALID_QUANTITY",
        ));
    }

    if input.asset_type == AssetType::Forex {
        return Err(AppError::bad_request(
            "Forex assets are not supported for trading.",
            "UNSUPPORTED_ASSET_TYPE",
        ));
    }

    let symbol = input.symbol.trim().to_uppercase();
    if symbol.is_empty() {
        return Err(AppError::bad_request(
            "Symbol is required.",
            "INVALID_SYMBOL",
        ));
    }

    if let Some(exp) = input.expires_at {
        if exp <= Utc::now() {
            return Err(AppError::bad_request(
                "expiresAt must be in the future.",
                "INVALID_EXPIRES_AT",
            ));
        }
    }

    match input.order_type {
        OrderType::Limit => {
            if input.limit_price.map(|p| p <= ZERO).unwrap_or(true) {
                return Err(AppError::bad_request(
                    "Limit price must be greater than zero.",
                    "INVALID_LIMIT_PRICE",
                ));
            }
        }
        OrderType::StopLoss | OrderType::TakeProfit => {
            if input.stop_price.map(|p| p <= ZERO).unwrap_or(true) {
                return Err(AppError::bad_request(
                    "Stop price must be greater than zero.",
                    "INVALID_STOP_PRICE",
                ));
            }
        }
        OrderType::Market => {}
    }

    let mut tx = pool.begin().await?;

    // Verify user is not suspended
    let user_suspended: Option<(bool,)> = sqlx::query_as(
        r#"SELECT "isSuspended" FROM "User" WHERE "id" = $1 LIMIT 1"#
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;

    match user_suspended {
        Some((true,)) => {
            return Err(AppError::forbidden(
                "Account is suspended.",
                "ACCOUNT_SUSPENDED",
            ))
        }
        None => return Err(AppError::not_found("User not found.", "USER_NOT_FOUND")),
        _ => {}
    }

    // Lock portfolio FOR UPDATE
    let portfolio: Option<(String, String, Decimal, Decimal, bool)> = sqlx::query_as(
        r#"
        SELECT "id", "userId", "cashBalance", "reservedCash", "isActive"
        FROM "Portfolio"
        WHERE "id" = $1 AND "userId" = $2
        FOR UPDATE
        "#
    )
    .bind(&input.portfolio_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (portfolio_id, _p_user_id, cash_balance, reserved_cash, is_active) = match portfolio {
        Some(p) => p,
        None => return Err(AppError::not_found("Portfolio not found.", "PORTFOLIO_NOT_FOUND")),
    };

    if !is_active {
        return Err(AppError::bad_request(
            "Portfolio is inactive.",
            "PORTFOLIO_INACTIVE",
        ));
    }

    // Check if portfolio belongs to a league
    let league_info: Option<(DateTime<Utc>, DateTime<Utc>, LeagueStatus)> = sqlx::query_as(
        r#"
        SELECT l."startDate", l."endDate", l."status"
        FROM "LeagueMember" lm
        JOIN "League" l ON lm."leagueId" = l."id"
        WHERE lm."portfolioId" = $1
        LIMIT 1
        "#
    )
    .bind(&portfolio_id)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some((start_date, end_date, league_status)) = league_info {
        let now = Utc::now();
        if league_status == LeagueStatus::Upcoming || now < start_date {
            return Err(AppError::bad_request(
                "Trading is not allowed before league start.",
                "LEAGUE_NOT_ACTIVE",
            ));
        }
        if league_status == LeagueStatus::Completed || league_status == LeagueStatus::Cancelled || now >= end_date {
            return Err(AppError::bad_request(
                "Trading is not allowed after league has ended.",
                "LEAGUE_ENDED",
            ));
        }
    }

    let mut order_reserved_cash = ZERO;

    if input.side == OrderSide::Buy {
        let reference_price = match input.order_type {
            OrderType::Market => reference_market_price.ok_or_else(|| {
                AppError::bad_request(
                    "Unable to calculate market price for reservation.",
                    "INVALID_RESERVATION_AMOUNT",
                )
            })?,
            OrderType::Limit => input.limit_price.unwrap(),
            OrderType::StopLoss | OrderType::TakeProfit => input.stop_price.unwrap(),
        };

        if reference_price <= ZERO {
            return Err(AppError::bad_request(
                "Reference price must be greater than zero.",
                "INVALID_RESERVATION_AMOUNT",
            ));
        }

        let required = (input.quantity * reference_price).round_dp(8);
        if required > MAX_MONEY {
            return Err(AppError::bad_request(
                "Order value exceeds the supported portfolio balance.",
                "RESERVATION_AMOUNT_TOO_LARGE",
            ));
        }

        let available_cash = cash_balance - reserved_cash;
        if available_cash < required {
            return Err(AppError::bad_request_with_details(
                "Insufficient available portfolio cash.",
                "INSUFFICIENT_BALANCE",
                serde_json::json!({
                    "required": required.to_string(),
                    "available": available_cash.to_string()
                }),
            ));
        }

        order_reserved_cash = required;

        sqlx::query(
            r#"
            UPDATE "Portfolio"
            SET "reservedCash" = "reservedCash" + $2, "updatedAt" = now()
            WHERE "id" = $1
            "#
        )
        .bind(&portfolio_id)
        .bind(order_reserved_cash)
        .execute(&mut *tx)
        .await?;
    } else {
        // SELL order: verify holding position
        let holding_qty: Option<(Decimal,)> = sqlx::query_as(
            r#"
            SELECT "quantity"
            FROM "Holding"
            WHERE "portfolioId" = $1 AND "symbol" = $2
            FOR UPDATE
            "#
        )
        .bind(&portfolio_id)
        .bind(&symbol)
        .fetch_optional(&mut *tx)
        .await?;

        let held = holding_qty.map(|(q,)| q).unwrap_or(ZERO);

        let pending_sells: Option<(Option<Decimal>,)> = sqlx::query_as(
            r#"
            SELECT SUM("quantity")
            FROM "Order"
            WHERE "portfolioId" = $1 AND "symbol" = $2 AND "side" = 'SELL' AND "status" = 'PENDING'
            "#
        )
        .bind(&portfolio_id)
        .bind(&symbol)
        .fetch_optional(&mut *tx)
        .await?;

        let already_pending = pending_sells.and_then(|(q,)| q).unwrap_or(ZERO);
        let available_qty = (held - already_pending).max(ZERO);

        if available_qty < input.quantity {
            return Err(AppError::bad_request(
                "Insufficient position to sell.",
                "INSUFFICIENT_POSITION",
            ));
        }
    }

    let order_id = Uuid::new_v4().to_string();
    let source = input.source.unwrap_or(OrderSource::Manual);

    let order: Order = sqlx::query_as(
        r#"
        INSERT INTO "Order" (
            "id", "portfolioId", "symbol", "assetType", "side", "type", "status",
            "quantity", "reservedCash", "limitPrice", "stopPrice", "expiresAt",
            "notes", "source", "copyTradeExecutionId", "createdAt", "updatedAt"
        )
        VALUES ($1, $2, $3, $4, $5, $6, 'PENDING', $7, $8, $9, $10, $11, $12, $13, $14, now(), now())
        RETURNING "id", "portfolioId", "symbol", "assetType", "side", "type", "status", "quantity", "reservedCash", "limitPrice", "stopPrice", "executedPrice", "executedAt", "expiresAt", "executionAttempts", "lastExecutionAttemptAt", "notes", "source", "copyTradeExecutionId", "createdAt", "updatedAt"
        "#
    )
    .bind(&order_id)
    .bind(&portfolio_id)
    .bind(&symbol)
    .bind(input.asset_type)
    .bind(input.side)
    .bind(input.order_type)
    .bind(input.quantity)
    .bind(order_reserved_cash)
    .bind(input.limit_price)
    .bind(input.stop_price)
    .bind(input.expires_at)
    .bind(input.notes)
    .bind(source)
    .bind(input.copy_trade_execution_id)
    .fetch_one(&mut *tx)
    .await?;

    record_order_event_tx(
        &mut tx,
        &order.id,
        OrderEventType::Created,
        None,
        OrderStatus::Pending,
        Some(serde_json::json!({
            "reservedCash": order_reserved_cash.to_string(),
            "expiresAt": input.expires_at.map(|e| e.to_rfc3339())
        })),
    )
    .await?;

    tx.commit().await?;

    Ok(order.into())
}

#[derive(Debug, Deserialize)]
pub struct ListOrdersQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub status: Option<OrderStatus>,
    pub symbol: Option<String>,
    pub side: Option<OrderSide>,
    #[serde(rename = "portfolioId")]
    pub portfolio_id: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct Pagination {
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    #[serde(rename = "totalPages")]
    pub total_pages: i64,
}

#[derive(Debug, Serialize)]
pub struct ListOrdersResponse {
    pub orders: Vec<SerializedOrder>,
    pub pagination: Pagination,
}

pub async fn list_orders(
    pool: &PgPool,
    user_id: &str,
    query: ListOrdersQuery,
) -> Result<ListOrdersResponse, AppError> {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * limit;

    let orders: Vec<Order> = sqlx::query_as(
        r#"
        SELECT o."id", o."portfolioId", o."symbol", o."assetType", o."side", o."type", o."status",
               o."quantity", o."reservedCash", o."limitPrice", o."stopPrice", o."executedPrice",
               o."executedAt", o."expiresAt", o."executionAttempts", o."lastExecutionAttemptAt",
               o."notes", o."source", o."copyTradeExecutionId", o."createdAt", o."updatedAt"
        FROM "Order" o
        JOIN "Portfolio" p ON o."portfolioId" = p."id"
        WHERE p."userId" = $1
          AND ($2::"OrderStatus" IS NULL OR o."status" = $2)
          AND ($3::TEXT IS NULL OR o."symbol" = $3)
          AND ($4::"OrderSide" IS NULL OR o."side" = $4)
          AND ($5::TEXT IS NULL OR o."portfolioId" = $5)
          AND ($6::TIMESTAMPTZ IS NULL OR o."createdAt" >= $6)
          AND ($7::TIMESTAMPTZ IS NULL OR o."createdAt" < $7)
        ORDER BY o."createdAt" DESC
        LIMIT $8 OFFSET $9
        "#
    )
    .bind(user_id)
    .bind(query.status)
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
        FROM "Order" o
        JOIN "Portfolio" p ON o."portfolioId" = p."id"
        WHERE p."userId" = $1
          AND ($2::"OrderStatus" IS NULL OR o."status" = $2)
          AND ($3::TEXT IS NULL OR o."symbol" = $3)
          AND ($4::"OrderSide" IS NULL OR o."side" = $4)
          AND ($5::TEXT IS NULL OR o."portfolioId" = $5)
          AND ($6::TIMESTAMPTZ IS NULL OR o."createdAt" >= $6)
          AND ($7::TIMESTAMPTZ IS NULL OR o."createdAt" < $7)
        "#
    )
    .bind(user_id)
    .bind(query.status)
    .bind(query.symbol.as_deref())
    .bind(query.side)
    .bind(query.portfolio_id.as_deref())
    .bind(query.from)
    .bind(query.to)
    .fetch_one(pool)
    .await?;

    let total_count = total.0;
    let total_pages = (total_count + limit - 1) / limit;

    Ok(ListOrdersResponse {
        orders: orders.into_iter().map(Into::into).collect(),
        pagination: Pagination {
            page,
            limit,
            total: total_count,
            total_pages,
        },
    })
}

pub async fn get_order_by_id(
    pool: &PgPool,
    user_id: &str,
    order_id: &str,
) -> Result<SerializedOrder, AppError> {
    let order: Option<Order> = sqlx::query_as(
        r#"
        SELECT o."id", o."portfolioId", o."symbol", o."assetType", o."side", o."type", o."status",
               o."quantity", o."reservedCash", o."limitPrice", o."stopPrice", o."executedPrice",
               o."executedAt", o."expiresAt", o."executionAttempts", o."lastExecutionAttemptAt",
               o."notes", o."source", o."copyTradeExecutionId", o."createdAt", o."updatedAt"
        FROM "Order" o
        JOIN "Portfolio" p ON o."portfolioId" = p."id"
        WHERE o."id" = $1 AND p."userId" = $2
        LIMIT 1
        "#
    )
    .bind(order_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    let o = order.ok_or_else(|| AppError::not_found("Order not found.", "ORDER_NOT_FOUND"))?;
    Ok(o.into())
}

pub async fn get_order_events(
    pool: &PgPool,
    user_id: &str,
    order_id: &str,
) -> Result<Vec<SerializedOrderEvent>, AppError> {
    let _ = get_order_by_id(pool, user_id, order_id).await?;

    let events: Vec<OrderEvent> = sqlx::query_as(
        r#"
        SELECT "id", "orderId", "sequence", "eventType", "previousStatus", "newStatus", "metadata", "createdAt"
        FROM "OrderEvent"
        WHERE "orderId" = $1
        ORDER BY "sequence" ASC
        "#
    )
    .bind(order_id)
    .fetch_all(pool)
    .await?;

    Ok(events.into_iter().map(Into::into).collect())
}

pub async fn cancel_order(
    pool: &PgPool,
    user_id: &str,
    order_id: &str,
) -> Result<SerializedOrder, AppError> {
    let mut tx = pool.begin().await?;

    let order_data: Option<(String, String, OrderSide, OrderStatus, Decimal)> = sqlx::query_as(
        r#"
        SELECT o."id", o."portfolioId", o."side", o."status", o."reservedCash"
        FROM "Order" o
        JOIN "Portfolio" p ON o."portfolioId" = p."id"
        WHERE o."id" = $1 AND p."userId" = $2
        FOR UPDATE
        "#
    )
    .bind(order_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (_oid, portfolio_id, side, status, reserved_cash) = match order_data {
        Some(d) => d,
        None => return Err(AppError::not_found("Order not found.", "ORDER_NOT_FOUND")),
    };

    if status != OrderStatus::Pending {
        return Err(AppError::bad_request(
            "Order is not in pending status.",
            "ORDER_NOT_CANCELABLE",
        ));
    }

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

    let updated: Order = sqlx::query_as(
        r#"
        UPDATE "Order"
        SET "status" = 'CANCELLED', "reservedCash" = 0, "updatedAt" = now()
        WHERE "id" = $1
        RETURNING "id", "portfolioId", "symbol", "assetType", "side", "type", "status", "quantity", "reservedCash", "limitPrice", "stopPrice", "executedPrice", "executedAt", "expiresAt", "executionAttempts", "lastExecutionAttemptAt", "notes", "source", "copyTradeExecutionId", "createdAt", "updatedAt"
        "#
    )
    .bind(order_id)
    .fetch_one(&mut *tx)
    .await?;

    record_order_event_tx(
        &mut tx,
        order_id,
        OrderEventType::Cancelled,
        Some(OrderStatus::Pending),
        OrderStatus::Cancelled,
        Some(serde_json::json!({
            "releasedReservedCash": reserved_cash.to_string(),
            "reason": "USER_CANCELLED"
        })),
    )
    .await?;

    tx.commit().await?;

    Ok(updated.into())
}
