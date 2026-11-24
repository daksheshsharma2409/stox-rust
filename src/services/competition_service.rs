use uuid::Uuid;
use chrono::{DateTime, Utc};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

use crate::error::AppError;
use crate::models::{League, LeagueMember, LeagueStatus};
use crate::services::market_data_service::MarketDataService;

#[derive(Debug, Deserialize)]
pub struct CreateLeagueRequest {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "startingBalance")]
    pub starting_balance: Option<Decimal>,
    #[serde(rename = "startDate")]
    pub start_date: DateTime<Utc>,
    #[serde(rename = "endDate")]
    pub end_date: DateTime<Utc>,
    #[serde(rename = "maxParticipants")]
    pub max_participants: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeagueSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "startingBalance")]
    pub starting_balance: f64,
    #[serde(rename = "maxParticipants")]
    pub max_participants: Option<i32>,
    #[serde(rename = "startDate")]
    pub start_date: DateTime<Utc>,
    #[serde(rename = "endDate")]
    pub end_date: DateTime<Utc>,
    pub status: LeagueStatus,
    #[serde(rename = "memberCount")]
    pub member_count: i64,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
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
pub struct LeagueListResponse {
    pub data: Vec<LeagueSummary>,
    pub pagination: PaginationMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeagueLeaderboardMember {
    pub rank: Option<i32>,
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "userName")]
    pub user_name: Option<String>,
    #[serde(rename = "userAvatar")]
    pub user_avatar: Option<String>,
    #[serde(rename = "portfolioId")]
    pub portfolio_id: String,
    #[serde(rename = "returnPct")]
    pub return_pct: f64,
    #[serde(rename = "totalValue")]
    pub total_value: Option<f64>,
    #[serde(rename = "tradeCount")]
    pub trade_count: i64,
    #[serde(rename = "joinedAt")]
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeagueLeaderboardInfo {
    pub id: String,
    pub name: String,
    pub status: LeagueStatus,
    #[serde(rename = "startDate")]
    pub start_date: DateTime<Utc>,
    #[serde(rename = "endDate")]
    pub end_date: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeagueLeaderboardResponse {
    pub league: LeagueLeaderboardInfo,
    pub leaderboard: Vec<LeagueLeaderboardMember>,
}

pub fn get_effective_league_status(
    status: LeagueStatus,
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
    now: DateTime<Utc>,
) -> LeagueStatus {
    if status == LeagueStatus::Cancelled {
        return LeagueStatus::Cancelled;
    }
    if now < start_date {
        LeagueStatus::Upcoming
    } else if now >= end_date {
        LeagueStatus::Completed
    } else {
        LeagueStatus::Active
    }
}

pub async fn create_league(
    pool: &PgPool,
    req: CreateLeagueRequest,
) -> Result<League, AppError> {
    if req.start_date >= req.end_date {
        return Err(AppError::bad_request("endDate must be after startDate.", "INVALID_DATE_RANGE"));
    }

    let now = Utc::now();
    let effective_status = get_effective_league_status(
        LeagueStatus::Upcoming,
        req.start_date,
        req.end_date,
        now,
    );
    let starting_balance = req.starting_balance.unwrap_or_else(|| Decimal::from(10000));
    let id = Uuid::new_v4().to_string();

    let league = sqlx::query_as::<_, League>(
        r#"
        INSERT INTO "League" (
            id, name, description, "startingBalance", "maxParticipants",
            "startDate", "endDate", status, "createdAt", "updatedAt"
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(req.name)
    .bind(req.description)
    .bind(starting_balance)
    .bind(req.max_participants)
    .bind(req.start_date)
    .bind(req.end_date)
    .bind(effective_status)
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to create league: {}", e)))?;

    Ok(league)
}

pub async fn list_leagues(
    pool: &PgPool,
    status_filter: Option<&str>,
    page: Option<i64>,
    limit: Option<i64>,
    now: DateTime<Utc>,
) -> Result<LeagueListResponse, AppError> {
    let page_num = page.unwrap_or(1).max(1);
    let limit_num = limit.unwrap_or(20).clamp(1, 100);
    let offset = (page_num - 1) * limit_num;

    let total_row = if let Some(st) = status_filter {
        sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM "League"
            WHERE status = $1::"LeagueStatus"
            "#,
        )
        .bind(st.to_uppercase())
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to count leagues: {}", e)))?
    } else {
        sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM "League"
            "#,
        )
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to count leagues: {}", e)))?
    };

    let total: i64 = total_row.get("count");
    let total_pages = if total == 0 {
        0
    } else {
        (total + limit_num - 1) / limit_num
    };

    let rows = if let Some(st) = status_filter {
        sqlx::query(
            r#"
            SELECT
                l.*,
                (SELECT COUNT(*) FROM "LeagueMember" m WHERE m."leagueId" = l.id) as member_count
            FROM "League" l
            WHERE l.status = $1::"LeagueStatus"
            ORDER BY l."startDate" DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(st.to_uppercase())
        .bind(limit_num)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to fetch leagues: {}", e)))?
    } else {
        sqlx::query(
            r#"
            SELECT
                l.*,
                (SELECT COUNT(*) FROM "LeagueMember" m WHERE m."leagueId" = l.id) as member_count
            FROM "League" l
            ORDER BY l."startDate" DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit_num)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to fetch leagues: {}", e)))?
    };

    let leagues = rows
        .into_iter()
        .map(|r| {
            let id: String = r.get("id");
            let name: String = r.get("name");
            let description: Option<String> = r.get("description");
            let starting_balance: Decimal = r.get("startingBalance");
            let max_participants: Option<i32> = r.get("maxParticipants");
            let start_date: DateTime<Utc> = r.get("startDate");
            let end_date: DateTime<Utc> = r.get("endDate");
            let status: LeagueStatus = r.get("status");
            let member_count: i64 = r.get("member_count");
            let created_at: DateTime<Utc> = r.get("createdAt");
            let updated_at: DateTime<Utc> = r.get("updatedAt");

            let eff_status = get_effective_league_status(status, start_date, end_date, now);

            LeagueSummary {
                id,
                name,
                description,
                starting_balance: starting_balance.to_f64().unwrap_or(10000.0),
                max_participants,
                start_date,
                end_date,
                status: eff_status,
                member_count,
                created_at,
                updated_at,
            }
        })
        .collect();

    Ok(LeagueListResponse {
        data: leagues,
        pagination: PaginationMeta {
            page: page_num,
            limit: limit_num,
            total,
            total_pages,
        },
    })
}

pub async fn get_league_by_id(
    pool: &PgPool,
    league_id: &str,
    now: DateTime<Utc>,
) -> Result<LeagueSummary, AppError> {
    let row = sqlx::query(
        r#"
        SELECT
            l.*,
            (SELECT COUNT(*) FROM "LeagueMember" m WHERE m."leagueId" = l.id) as member_count
        FROM "League" l
        WHERE l.id = $1
        "#,
    )
    .bind(league_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to fetch league: {}", e)))?;

    let r = match row {
        Some(r) => r,
        None => return Err(AppError::not_found("League not found.", "LEAGUE_NOT_FOUND")),
    };

    let id: String = r.get("id");
    let name: String = r.get("name");
    let description: Option<String> = r.get("description");
    let starting_balance: Decimal = r.get("startingBalance");
    let max_participants: Option<i32> = r.get("maxParticipants");
    let start_date: DateTime<Utc> = r.get("startDate");
    let end_date: DateTime<Utc> = r.get("endDate");
    let status: LeagueStatus = r.get("status");
    let member_count: i64 = r.get("member_count");
    let created_at: DateTime<Utc> = r.get("createdAt");
    let updated_at: DateTime<Utc> = r.get("updatedAt");

    let eff_status = get_effective_league_status(status, start_date, end_date, now);

    // If status in db is out of date, update it
    if status != eff_status {
        let _ = sqlx::query(
            r#"UPDATE "League" SET status = $1, "updatedAt" = $2 WHERE id = $3"#
        )
        .bind(eff_status)
        .bind(now)
        .bind(&id)
        .execute(pool)
        .await;
    }

    Ok(LeagueSummary {
        id,
        name,
        description,
        starting_balance: starting_balance.to_f64().unwrap_or(10000.0),
        max_participants,
        start_date,
        end_date,
        status: eff_status,
        member_count,
        created_at,
        updated_at,
    })
}

pub async fn join_league(
    pool: &PgPool,
    league_id: &str,
    user_id: &str,
    portfolio_id: Option<String>,
    now: DateTime<Utc>,
) -> Result<LeagueMember, AppError> {
    if portfolio_id.is_some() {
        return Err(AppError::bad_request("Existing portfolios are not permitted for leagues. A dedicated league portfolio is automatically allocated.", "EXISTING_PORTFOLIO_NOT_ALLOWED"));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::internal(format!("Failed to start transaction: {}", e)))?;

    // Row lock FOR UPDATE
    let league = sqlx::query_as::<_, League>(
        r#"
        SELECT *
        FROM "League"
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(league_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| AppError::internal(format!("Failed to lock league: {}", e)))?;

    let league = match league {
        Some(l) => l,
        None => return Err(AppError::not_found("League not found.", "LEAGUE_NOT_FOUND")),
    };

    let eff_status = get_effective_league_status(league.status, league.start_date, league.end_date, now);

    if now >= league.start_date || eff_status != LeagueStatus::Upcoming {
        return Err(AppError::bad_request("Cannot join league after it has started.", "LEAGUE_ALREADY_STARTED"));
    }

    if let Some(max_p) = league.max_participants {
        let count_row = sqlx::query(
            r#"SELECT COUNT(*) as count FROM "LeagueMember" WHERE "leagueId" = $1"#
        )
        .bind(league_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::internal(format!("Failed to check capacity: {}", e)))?;

        let current_count: i64 = count_row.get("count");
        if current_count >= max_p as i64 {
            return Err(AppError::bad_request("League is full.", "LEAGUE_FULL"));
        }
    }

    let existing = sqlx::query(
        r#"SELECT id FROM "LeagueMember" WHERE "leagueId" = $1 AND "userId" = $2"#
    )
    .bind(league_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| AppError::internal(format!("Failed to check membership: {}", e)))?;

    if existing.is_some() {
        return Err(AppError::conflict("User is already a member of this league.", "ALREADY_JOINED"));
    }

    // Create fresh dedicated portfolio
    let portfolio_id = Uuid::new_v4().to_string();
    let portfolio_name = format!("{} - {}", league.name, now.timestamp_millis());
    let portfolio_desc = format!("Competition portfolio for {}", league.name);

    sqlx::query(
        r#"
        INSERT INTO "Portfolio" (
            id, "userId", name, description, "startingBalance",
            "cashBalance", "reservedCash", "isDefault", "isActive",
            "createdAt", "updatedAt"
        )
        VALUES ($1, $2, $3, $4, $5, $5, 0, false, true, $6, $6)
        "#,
    )
    .bind(&portfolio_id)
    .bind(user_id)
    .bind(&portfolio_name)
    .bind(&portfolio_desc)
    .bind(league.starting_balance)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::internal(format!("Failed to create league portfolio: {}", e)))?;

    let member_id = Uuid::new_v4().to_string();
    let member = sqlx::query_as::<_, LeagueMember>(
        r#"
        INSERT INTO "LeagueMember" (
            id, "leagueId", "userId", "portfolioId", "joinedAt"
        )
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(&member_id)
    .bind(league_id)
    .bind(user_id)
    .bind(&portfolio_id)
    .bind(now)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| AppError::internal(format!("Failed to create league member: {}", e)))?;

    tx.commit()
        .await
        .map_err(|e| AppError::internal(format!("Failed to commit league join: {}", e)))?;

    Ok(member)
}

pub async fn get_league_leaderboard(
    state: &crate::state::AppState,
    league_id: &str,
    now: DateTime<Utc>,
) -> Result<LeagueLeaderboardResponse, AppError> {
    let pool = &state.pool;
    let league = sqlx::query_as::<_, League>(
        r#"
        SELECT *
        FROM "League"
        WHERE id = $1
        "#,
    )
    .bind(league_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to fetch league: {}", e)))?;

    let league = match league {
        Some(l) => l,
        None => return Err(AppError::not_found("League not found.", "LEAGUE_NOT_FOUND")),
    };

    let eff_status = get_effective_league_status(league.status, league.start_date, league.end_date, now);

    // Fetch members with user info and portfolio info
    let member_rows = sqlx::query(
        r#"
        SELECT
            m.id as member_id,
            m."userId",
            m."portfolioId",
            m.rank,
            m."finalReturn",
            m."joinedAt",
            u.name as user_name,
            u."avatarUrl" as user_avatar_url,
            u."isSuspended" as user_is_suspended,
            p."cashBalance" as portfolio_cash_balance,
            p."startingBalance" as portfolio_starting_balance
        FROM "LeagueMember" m
        JOIN "User" u ON u.id = m."userId"
        JOIN "Portfolio" p ON p.id = m."portfolioId"
        WHERE m."leagueId" = $1
        "#,
    )
    .bind(league_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to fetch league members: {}", e)))?;

    // If completed and all non-suspended have rank & finalReturn
    let eligible_members: Vec<_> = member_rows
        .iter()
        .filter(|r| !r.get::<bool, _>("user_is_suspended"))
        .collect();

    let all_ranked = !eligible_members.is_empty()
        && eligible_members.iter().all(|r| {
            r.get::<Option<i32>, _>("rank").is_some()
                && r.get::<Option<Decimal>, _>("finalReturn").is_some()
        });

    if league.status == LeagueStatus::Completed && all_ranked {
        let mut sorted = eligible_members;
        sorted.sort_by_key(|r| r.get::<Option<i32>, _>("rank").unwrap_or(999999));

        let mut items = Vec::new();
        for r in sorted {
            let p_id: String = r.get("portfolioId");
            let trade_count_row = sqlx::query(
                r#"
                SELECT COUNT(*) as count
                FROM "Trade"
                WHERE "portfolioId" = $1 AND "executedAt" >= $2 AND "executedAt" <= $3
                "#,
            )
            .bind(&p_id)
            .bind(league.start_date)
            .bind(league.end_date)
            .fetch_one(pool)
            .await
            .map_err(|e| AppError::internal(format!("Failed to count trades: {}", e)))?;

            let trade_count: i64 = trade_count_row.get("count");
            let final_ret: Decimal = r.get::<Option<Decimal>, _>("finalReturn").unwrap_or(Decimal::ZERO);

            items.push(LeagueLeaderboardMember {
                rank: r.get("rank"),
                user_id: r.get("userId"),
                user_name: r.get("user_name"),
                user_avatar: r.get("user_avatar_url"),
                portfolio_id: p_id,
                return_pct: final_ret.to_f64().unwrap_or(0.0),
                total_value: None,
                trade_count,
                joined_at: r.get("joinedAt"),
            });
        }

        return Ok(LeagueLeaderboardResponse {
            league: LeagueLeaderboardInfo {
                id: league.id,
                name: league.name,
                status: LeagueStatus::Completed,
                start_date: league.start_date,
                end_date: league.end_date,
            },
            leaderboard: items,
        });
    }

    if league.status == LeagueStatus::Completed && eligible_members.is_empty() {
        return Ok(LeagueLeaderboardResponse {
            league: LeagueLeaderboardInfo {
                id: league.id,
                name: league.name,
                status: LeagueStatus::Completed,
                start_date: league.start_date,
                end_date: league.end_date,
            },
            leaderboard: Vec::new(),
        });
    }

    // Dynamic valuation
    let mut evaluated = Vec::new();

    for r in &eligible_members {
        let user_id: String = r.get("userId");
        let portfolio_id: String = r.get("portfolioId");
        let joined_at: DateTime<Utc> = r.get("joinedAt");
        let user_name: Option<String> = r.get("user_name");
        let user_avatar: Option<String> = r.get("user_avatar_url");
        let cash: Decimal = r.get("portfolio_cash_balance");
        let starting_balance: Decimal = r.get("portfolio_starting_balance");

        // Calculate holdings market value
        let holdings = sqlx::query(
            r#"
            SELECT symbol, quantity
            FROM "Holding"
            WHERE "portfolioId" = $1 AND quantity > 0
            "#,
        )
        .bind(&portfolio_id)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to fetch holdings: {}", e)))?;

        let mut holdings_value = Decimal::ZERO;
        for h in holdings {
            let symbol: String = h.get("symbol");
            let qty: Decimal = h.get("quantity");
            let current_price = match MarketDataService::get_quote(state, &symbol, &crate::services::market_data_service::QuoteOptions::default()).await {
                Ok(q) => Decimal::from_str(&q.price).unwrap_or(Decimal::ZERO),
                Err(_) => Decimal::ZERO,
            };
            holdings_value += qty * current_price;
        }

        let total_val = cash + holdings_value;
        let return_pct = if starting_balance.is_zero() {
            Decimal::ZERO
        } else {
            ((total_val - starting_balance) / starting_balance) * Decimal::from(100)
        };

        let trade_count_row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM "Trade"
            WHERE "portfolioId" = $1 AND "executedAt" >= $2 AND "executedAt" <= $3
            "#,
        )
        .bind(&portfolio_id)
        .bind(league.start_date)
        .bind(league.end_date)
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to count trades: {}", e)))?;

        let trade_count: i64 = trade_count_row.get("count");

        evaluated.push((
            user_id,
            user_name,
            user_avatar,
            portfolio_id,
            return_pct,
            total_val,
            trade_count,
            joined_at,
        ));
    }

    // Sort evaluated:
    // 1. returnPct DESC
    // 2. totalValue DESC
    // 3. tradeCount DESC
    // 4. joinedAt ASC
    // 5. userId ASC
    evaluated.sort_by(|a, b| {
        b.4.cmp(&a.4)
            .then_with(|| b.5.cmp(&a.5))
            .then_with(|| b.6.cmp(&a.6))
            .then_with(|| a.7.cmp(&b.7))
            .then_with(|| a.0.cmp(&b.0))
    });

    let items: Vec<LeagueLeaderboardMember> = evaluated
        .into_iter()
        .enumerate()
        .map(|(idx, (u_id, u_name, u_avatar, p_id, ret, tot, tc, joined))| {
            LeagueLeaderboardMember {
                rank: Some((idx + 1) as i32),
                user_id: u_id,
                user_name: u_name,
                user_avatar: u_avatar,
                portfolio_id: p_id,
                return_pct: (ret.to_f64().unwrap_or(0.0) * 10000.0).round() / 10000.0,
                total_value: Some((tot.to_f64().unwrap_or(0.0) * 100.0).round() / 100.0),
                trade_count: tc,
                joined_at: joined,
            }
        })
        .collect();

    Ok(LeagueLeaderboardResponse {
        league: LeagueLeaderboardInfo {
            id: league.id,
            name: league.name,
            status: eff_status,
            start_date: league.start_date,
            end_date: league.end_date,
        },
        leaderboard: items,
    })
}
