use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

use crate::error::AppError;
use crate::models::{
    OrderSide, OrderStatus, OrderType, SanitizedUser,
};

#[derive(Debug, Deserialize)]
pub struct AdminOrderFilter {
    pub status: Option<String>,
    pub symbol: Option<String>,
    pub side: Option<String>,
    #[serde(rename = "type")]
    pub order_type: Option<String>,
    #[serde(rename = "portfolioId")]
    pub portfolio_id: Option<String>,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminOrderPortfolioSummary {
    pub id: String,
    pub name: String,
    #[serde(rename = "isActive")]
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminOrderOwnerSummary {
    pub id: String,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminOrderSummary {
    pub id: String,
    #[serde(rename = "portfolioId")]
    pub portfolio_id: String,
    pub symbol: String,
    pub side: OrderSide,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    pub status: OrderStatus,
    pub quantity: String,
    #[serde(rename = "filledQuantity")]
    pub filled_quantity: String,
    pub price: Option<String>,
    #[serde(rename = "stopPrice")]
    pub stop_price: Option<String>,
    #[serde(rename = "averageExecutionPrice")]
    pub average_execution_price: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
    pub portfolio: AdminOrderPortfolioSummary,
    pub owner: AdminOrderOwnerSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationMeta {
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    #[serde(rename = "totalPages")]
    pub total_pages: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminOrderListResponse {
    pub orders: Vec<AdminOrderSummary>,
    pub pagination: PaginationMeta,
}

pub async fn list_admin_orders(
    pool: &PgPool,
    filter: AdminOrderFilter,
) -> Result<AdminOrderListResponse, AppError> {
    let page_num = filter.page.unwrap_or(1).max(1);
    let limit_num = filter.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page_num - 1) * limit_num;

    let mut count_query = String::from(
        r#"
        SELECT COUNT(*) as count
        FROM "Order" o
        JOIN "Portfolio" p ON p.id = o."portfolioId"
        JOIN "User" u ON u.id = p."userId"
        WHERE 1=1
        "#,
    );

    let mut select_query = String::from(
        r#"
        SELECT
            o.id,
            o."portfolioId",
            o.symbol,
            o.side,
            o.type,
            o.status,
            o.quantity,
            o."filledQuantity",
            o.price,
            o."stopPrice",
            o."averageExecutionPrice",
            o."createdAt",
            o."updatedAt",
            p.id as p_id,
            p.name as p_name,
            p."isActive" as p_is_active,
            u.id as u_id,
            u.name as u_name,
            u.email as u_email
        FROM "Order" o
        JOIN "Portfolio" p ON p.id = o."portfolioId"
        JOIN "User" u ON u.id = p."userId"
        WHERE 1=1
        "#,
    );

    let mut conditions = Vec::new();

    if let Some(ref st) = filter.status {
        conditions.push(format!("o.status = '{}'::\"OrderStatus\"", st.to_uppercase()));
    }
    if let Some(ref sym) = filter.symbol {
        conditions.push(format!("o.symbol = '{}'", sym.trim().to_uppercase()));
    }
    if let Some(ref sd) = filter.side {
        conditions.push(format!("o.side = '{}'::\"Side\"", sd.to_uppercase()));
    }
    if let Some(ref ty) = filter.order_type {
        conditions.push(format!("o.type = '{}'::\"OrderType\"", ty.to_uppercase()));
    }
    if let Some(ref pid) = filter.portfolio_id {
        conditions.push(format!("o.\"portfolioId\" = '{}'", pid));
    }
    if let Some(ref uid) = filter.user_id {
        conditions.push(format!("p.\"userId\" = '{}'", uid));
    }
    if let Some(from) = filter.from {
        conditions.push(format!("o.\"createdAt\" >= '{}'", from.to_rfc3339()));
    }
    if let Some(to) = filter.to {
        conditions.push(format!("o.\"createdAt\" < '{}'", to.to_rfc3339()));
    }

    for cond in conditions {
        count_query.push_str(" AND ");
        count_query.push_str(&cond);
        select_query.push_str(" AND ");
        select_query.push_str(&cond);
    }

    select_query.push_str(" ORDER BY o.\"createdAt\" DESC, o.id ASC");
    select_query.push_str(&format!(" LIMIT {} OFFSET {}", limit_num, offset));

    let count_row = sqlx::query(&count_query)
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to count admin orders: {}", e)))?;

    let total: i64 = count_row.get("count");
    let total_pages = if total == 0 {
        0
    } else {
        (total + limit_num - 1) / limit_num
    };

    let rows = sqlx::query(&select_query)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to fetch admin orders: {}", e)))?;

    let orders = rows
        .into_iter()
        .map(|r| {
            let id: String = r.get("id");
            let portfolio_id: String = r.get("portfolioId");
            let symbol: String = r.get("symbol");
            let side: OrderSide = r.get("side");
            let order_type: OrderType = r.get("type");
            let status: OrderStatus = r.get("status");
            let quantity: Decimal = r.get("quantity");
            let filled_quantity: Decimal = r.get("filledQuantity");
            let price: Option<Decimal> = r.get("price");
            let stop_price: Option<Decimal> = r.get("stopPrice");
            let average_execution_price: Option<Decimal> = r.get("averageExecutionPrice");
            let created_at: DateTime<Utc> = r.get("createdAt");
            let updated_at: DateTime<Utc> = r.get("updatedAt");

            let p_id: String = r.get("p_id");
            let p_name: String = r.get("p_name");
            let p_is_active: bool = r.get("p_is_active");

            let u_id: String = r.get("u_id");
            let u_name: String = r.get("u_name");
            let u_email: String = r.get("u_email");

            AdminOrderSummary {
                id,
                portfolio_id,
                symbol,
                side,
                order_type,
                status,
                quantity: quantity.to_string(),
                filled_quantity: filled_quantity.to_string(),
                price: price.map(|p| p.to_string()),
                stop_price: stop_price.map(|p| p.to_string()),
                average_execution_price: average_execution_price.map(|p| p.to_string()),
                created_at,
                updated_at,
                portfolio: AdminOrderPortfolioSummary {
                    id: p_id,
                    name: p_name,
                    is_active: p_is_active,
                },
                owner: AdminOrderOwnerSummary {
                    id: u_id,
                    name: u_name,
                    email: u_email,
                },
            }
        })
        .collect();

    Ok(AdminOrderListResponse {
        orders,
        pagination: PaginationMeta {
            page: page_num,
            limit: limit_num,
            total,
            total_pages,
        },
    })
}

#[derive(Debug, Deserialize)]
pub struct AdminPortfolioFilter {
    pub search: Option<String>,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    #[serde(rename = "isActive")]
    pub is_active: Option<bool>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminPortfolioSummary {
    pub id: String,
    #[serde(rename = "userId")]
    pub user_id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "startingBalance")]
    pub starting_balance: String,
    #[serde(rename = "cashBalance")]
    pub cash_balance: String,
    #[serde(rename = "isDefault")]
    pub is_default: bool,
    #[serde(rename = "isActive")]
    pub is_active: bool,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
    pub owner: AdminOrderOwnerSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminPortfolioListResponse {
    pub portfolios: Vec<AdminPortfolioSummary>,
    pub pagination: PaginationMeta,
}

pub async fn list_admin_portfolios(
    pool: &PgPool,
    filter: AdminPortfolioFilter,
) -> Result<AdminPortfolioListResponse, AppError> {
    let page_num = filter.page.unwrap_or(1).max(1);
    let limit_num = filter.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page_num - 1) * limit_num;

    let mut count_query = String::from(
        r#"
        SELECT COUNT(*) as count
        FROM "Portfolio" p
        JOIN "User" u ON u.id = p."userId"
        WHERE 1=1
        "#,
    );

    let mut select_query = String::from(
        r#"
        SELECT
            p.id,
            p."userId",
            p.name,
            p.description,
            p."startingBalance",
            p."cashBalance",
            p."isDefault",
            p."isActive",
            p."createdAt",
            p."updatedAt",
            u.id as u_id,
            u.name as u_name,
            u.email as u_email
        FROM "Portfolio" p
        JOIN "User" u ON u.id = p."userId"
        WHERE 1=1
        "#,
    );

    let mut conditions = Vec::new();

    if let Some(ref uid) = filter.user_id {
        conditions.push(format!("p.\"userId\" = '{}'", uid));
    }
    if let Some(active) = filter.is_active {
        conditions.push(format!("p.\"isActive\" = {}", active));
    }
    if let Some(ref search) = filter.search {
        let s = search.trim();
        if !s.is_empty() {
            conditions.push(format!(
                "(p.name ILIKE '%{}%' OR u.name ILIKE '%{}%' OR u.email ILIKE '%{}%')",
                s, s, s
            ));
        }
    }

    for cond in conditions {
        count_query.push_str(" AND ");
        count_query.push_str(&cond);
        select_query.push_str(" AND ");
        select_query.push_str(&cond);
    }

    select_query.push_str(" ORDER BY p.\"createdAt\" DESC, p.id ASC");
    select_query.push_str(&format!(" LIMIT {} OFFSET {}", limit_num, offset));

    let count_row = sqlx::query(&count_query)
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to count admin portfolios: {}", e)))?;

    let total: i64 = count_row.get("count");
    let total_pages = if total == 0 {
        0
    } else {
        (total + limit_num - 1) / limit_num
    };

    let rows = sqlx::query(&select_query)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to fetch admin portfolios: {}", e)))?;

    let portfolios = rows
        .into_iter()
        .map(|r| {
            let id: String = r.get("id");
            let user_id: String = r.get("userId");
            let name: String = r.get("name");
            let description: Option<String> = r.get("description");
            let starting_balance: Decimal = r.get("startingBalance");
            let cash_balance: Decimal = r.get("cashBalance");
            let is_default: bool = r.get("isDefault");
            let is_active: bool = r.get("isActive");
            let created_at: DateTime<Utc> = r.get("createdAt");
            let updated_at: DateTime<Utc> = r.get("updatedAt");

            let u_id: String = r.get("u_id");
            let u_name: String = r.get("u_name");
            let u_email: String = r.get("u_email");

            AdminPortfolioSummary {
                id,
                user_id,
                name,
                description,
                starting_balance: starting_balance.to_string(),
                cash_balance: cash_balance.to_string(),
                is_default,
                is_active,
                created_at,
                updated_at,
                owner: AdminOrderOwnerSummary {
                    id: u_id,
                    name: u_name,
                    email: u_email,
                },
            }
        })
        .collect();

    Ok(AdminPortfolioListResponse {
        portfolios,
        pagination: PaginationMeta {
            page: page_num,
            limit: limit_num,
            total,
            total_pages,
        },
    })
}

pub async fn get_admin_portfolio_by_id(
    pool: &PgPool,
    portfolio_id: &str,
) -> Result<AdminPortfolioSummary, AppError> {
    let row = sqlx::query(
        r#"
        SELECT
            p.id,
            p."userId",
            p.name,
            p.description,
            p."startingBalance",
            p."cashBalance",
            p."isDefault",
            p."isActive",
            p."createdAt",
            p."updatedAt",
            u.id as u_id,
            u.name as u_name,
            u.email as u_email
        FROM "Portfolio" p
        JOIN "User" u ON u.id = p."userId"
        WHERE p.id = $1
        "#,
    )
    .bind(portfolio_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to fetch admin portfolio: {}", e)))?;

    let r = match row {
        Some(r) => r,
        None => return Err(AppError::not_found("Portfolio not found.", "PORTFOLIO_NOT_FOUND")),
    };

    let id: String = r.get("id");
    let user_id: String = r.get("userId");
    let name: String = r.get("name");
    let description: Option<String> = r.get("description");
    let starting_balance: Decimal = r.get("startingBalance");
    let cash_balance: Decimal = r.get("cashBalance");
    let is_default: bool = r.get("isDefault");
    let is_active: bool = r.get("isActive");
    let created_at: DateTime<Utc> = r.get("createdAt");
    let updated_at: DateTime<Utc> = r.get("updatedAt");

    let u_id: String = r.get("u_id");
    let u_name: String = r.get("u_name");
    let u_email: String = r.get("u_email");

    Ok(AdminPortfolioSummary {
        id,
        user_id,
        name,
        description,
        starting_balance: starting_balance.to_string(),
        cash_balance: cash_balance.to_string(),
        is_default,
        is_active,
        created_at,
        updated_at,
        owner: AdminOrderOwnerSummary {
            id: u_id,
            name: u_name,
            email: u_email,
        },
    })
}

#[derive(Debug, Deserialize)]
pub struct AdminUserFilter {
    pub search: Option<String>,
    pub role: Option<String>,
    #[serde(rename = "isSuspended")]
    pub is_suspended: Option<bool>,
    #[serde(rename = "isVerified")]
    pub is_verified: Option<bool>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminUserListResponse {
    pub users: Vec<SanitizedUser>,
    pub pagination: PaginationMeta,
}

pub async fn list_admin_users(
    pool: &PgPool,
    filter: AdminUserFilter,
) -> Result<AdminUserListResponse, AppError> {
    let page_num = filter.page.unwrap_or(1).max(1);
    let limit_num = filter.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page_num - 1) * limit_num;

    let mut count_query = String::from(r#"SELECT COUNT(*) as count FROM "User" WHERE 1=1"#);
    let mut select_query = String::from(r#"SELECT * FROM "User" WHERE 1=1"#);

    let mut conditions = Vec::new();

    if let Some(ref r) = filter.role {
        conditions.push(format!("role = '{}'::\"Role\"", r.to_uppercase()));
    }
    if let Some(suspended) = filter.is_suspended {
        conditions.push(format!("\"isSuspended\" = {}", suspended));
    }
    if let Some(verified) = filter.is_verified {
        conditions.push(format!("\"isVerified\" = {}", verified));
    }
    if let Some(ref search) = filter.search {
        let s = search.trim();
        if !s.is_empty() {
            conditions.push(format!(
                "(name ILIKE '%{}%' OR email ILIKE '%{}%')",
                s, s
            ));
        }
    }

    for cond in conditions {
        count_query.push_str(" AND ");
        count_query.push_str(&cond);
        select_query.push_str(" AND ");
        select_query.push_str(&cond);
    }

    select_query.push_str(" ORDER BY \"createdAt\" DESC");
    select_query.push_str(&format!(" LIMIT {} OFFSET {}", limit_num, offset));

    let count_row = sqlx::query(&count_query)
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to count admin users: {}", e)))?;

    let total: i64 = count_row.get("count");
    let total_pages = if total == 0 {
        0
    } else {
        (total + limit_num - 1) / limit_num
    };

    let users = sqlx::query_as::<_, crate::models::User>(&select_query)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to fetch admin users: {}", e)))?;

    let sanitized: Vec<SanitizedUser> = users.into_iter().map(SanitizedUser::from).collect();

    Ok(AdminUserListResponse {
        users: sanitized,
        pagination: PaginationMeta {
            page: page_num,
            limit: limit_num,
            total,
            total_pages,
        },
    })
}

pub async fn get_admin_user_by_id(
    pool: &PgPool,
    user_id: &str,
) -> Result<SanitizedUser, AppError> {
    let user = sqlx::query_as::<_, crate::models::User>(
        r#"SELECT * FROM "User" WHERE id = $1"#
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to fetch admin user: {}", e)))?;

    match user {
        Some(u) => Ok(SanitizedUser::from(u)),
        None => Err(AppError::not_found("User not found.", "USER_NOT_FOUND")),
    }
}
