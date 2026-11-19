use axum::http::StatusCode;
use chrono::Utc;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;

use crate::{
    error::AppError,
    models::{Holding, Order, PerformanceSnapshot, Portfolio, Trade},
    services::market_data_service::{MarketDataService, QuoteOptions},
    state::AppState,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealizedPnlResponse {
    pub portfolio_id: String,
    pub realized_pnl: String,
    pub realized_pnl_pct: String,
    pub total_proceeds: String,
    pub total_cost_basis: String,
    pub total_fees: String,
    pub trade_count: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub breakeven_trades: usize,
    pub win_rate: f64,
    pub by_symbol: Vec<SymbolPnlBreakdown>,
    pub trades: Vec<FormattedTradePnl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolPnlBreakdown {
    pub symbol: String,
    pub realized_pnl: String,
    pub realized_pnl_pct: String,
    pub proceeds: String,
    pub cost_basis: String,
    pub fees: String,
    pub trade_count: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub breakeven_trades: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormattedTradePnl {
    pub id: String,
    pub order_id: String,
    pub symbol: String,
    pub side: String,
    pub quantity: String,
    pub price: String,
    pub total_value: String,
    pub cost_basis: String,
    pub fees: String,
    pub realized_pnl: String,
    pub realized_pnl_pct: Option<String>,
    pub executed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionUnrealizedPnl {
    pub id: String,
    pub symbol: String,
    pub asset_type: String,
    pub is_short: bool,
    pub quantity: String,
    pub average_buy_price: String,
    pub total_invested: String,
    pub current_price: String,
    pub market_value: String,
    pub unrealized_pnl: String,
    pub unrealized_pnl_pct: String,
    pub is_stale: bool,
    pub freshness: String,
    pub age_seconds: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnrealizedPnlResponse {
    pub portfolio_id: String,
    pub total_invested: String,
    pub market_value: String,
    pub unrealized_pnl: String,
    pub unrealized_pnl_pct: String,
    pub is_stale: bool,
    pub freshness: String,
    pub max_age_seconds: i64,
    pub position_count: usize,
    pub as_of: String,
    pub positions: Vec<PositionUnrealizedPnl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregatePnlResponse {
    pub portfolio_id: String,
    pub currency: String,
    pub as_of: String,
    pub starting_balance: String,
    pub cash_balance: String,
    pub open_holdings_value: String,
    pub open_holdings_cost_basis: String,
    pub total_portfolio_value: String,
    pub net_pnl: String,
    pub net_pnl_pct: String,
    pub account_pnl: String,
    pub account_pnl_pct: String,
    pub realized: RealizedSummary,
    pub unrealized: UnrealizedSummary,
    pub reconciliation: ReconciliationSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealizedSummary {
    pub pnl: String,
    pub pnl_pct: String,
    pub proceeds: String,
    pub cost_basis: String,
    pub fees: String,
    pub trade_count: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub breakeven_trades: usize,
    pub win_rate: f64,
    pub by_symbol: Vec<SymbolPnlBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnrealizedSummary {
    pub pnl: String,
    pub pnl_pct: String,
    pub market_value: String,
    pub cost_basis: String,
    pub position_count: usize,
    pub is_stale: bool,
    pub freshness: String,
    pub max_age_seconds: i64,
    pub positions: Vec<PositionUnrealizedPnl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationSummary {
    pub is_reconciled: bool,
    pub open_positions_count: usize,
    pub closed_trades_count: usize,
    pub total_fees: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllocationItem {
    pub label: String,
    pub asset_type: Option<String>,
    pub value: String,
    pub percentage: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioAllocationResponse {
    pub portfolio_id: String,
    pub dimension: String,
    pub total_value: String,
    pub items: Vec<AllocationItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskMetricsResponse {
    pub portfolio_id: String,
    pub period: String,
    pub annualized_volatility: Option<String>,
    pub max_drawdown: Option<String>,
    pub current_drawdown: Option<String>,
    pub peak_value: Option<String>,
    pub peak_date: Option<String>,
    pub sharpe_ratio: Option<String>,
    pub observation_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatmapDay {
    pub date: String,
    pub pnl: String,
    pub pnl_percent: String,
    pub total_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatmapSummary {
    pub total_pnl: String,
    pub win_days: usize,
    pub loss_days: usize,
    pub flat_days: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PnlHeatmapResponse {
    pub portfolio_id: String,
    pub period: String,
    pub heatmap: Vec<HeatmapDay>,
    pub summary: HeatmapSummary,
}

pub struct PortfolioAnalyticsService;

impl PortfolioAnalyticsService {
    pub async fn get_realized_pnl(
        pool: &PgPool,
        user_id: &str,
        portfolio_id: &str,
        symbol_filter: Option<&str>,
    ) -> Result<RealizedPnlResponse, AppError> {
        let portfolio = sqlx::query_as::<_, Portfolio>(
            r#"SELECT * FROM "Portfolio" WHERE "id" = $1 AND "userId" = $2"#,
        )
        .bind(portfolio_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("Portfolio not found.", "PORTFOLIO_NOT_FOUND"))?;

        let trades = sqlx::query_as::<_, Trade>(
            r#"
            SELECT * FROM "Trade"
            WHERE "portfolioId" = $1 AND "realizedPnl" IS NOT NULL
            ORDER BY "executedAt" DESC
            "#,
        )
        .bind(portfolio_id)
        .fetch_all(pool)
        .await?;

        let mut total_realized_pnl = Decimal::ZERO;
        let mut total_proceeds = Decimal::ZERO;
        let mut total_cost_basis = Decimal::ZERO;
        let mut total_fees = Decimal::ZERO;
        let mut winning_trades = 0;
        let mut losing_trades = 0;
        let mut breakeven_trades = 0;

        let mut symbol_map: HashMap<String, (Decimal, Decimal, Decimal, Decimal, usize, usize, usize, usize)> = HashMap::new();
        let mut formatted_trades = Vec::new();

        for trade in trades {
            if let Some(filter) = symbol_filter {
                if !trade.symbol.eq_ignore_ascii_case(filter) {
                    continue;
                }
            }

            let pnl = trade.realized_pnl.unwrap_or(Decimal::ZERO);
            let val = trade.total_value;
            let fees = trade.fees;
            let cost_basis = val - pnl - fees;

            total_realized_pnl += pnl;
            total_proceeds += val;
            total_cost_basis += cost_basis;
            total_fees += fees;

            if pnl > Decimal::ZERO {
                winning_trades += 1;
            } else if pnl < Decimal::ZERO {
                losing_trades += 1;
            } else {
                breakeven_trades += 1;
            }

            let entry = symbol_map.entry(trade.symbol.to_uppercase()).or_insert((
                Decimal::ZERO, Decimal::ZERO, Decimal::ZERO, Decimal::ZERO, 0, 0, 0, 0,
            ));
            entry.0 += pnl;
            entry.1 += val;
            entry.2 += cost_basis;
            entry.3 += fees;
            entry.4 += 1;
            if pnl > Decimal::ZERO { entry.5 += 1; }
            else if pnl < Decimal::ZERO { entry.6 += 1; }
            else { entry.7 += 1; }

            formatted_trades.push(FormattedTradePnl {
                id: trade.id,
                order_id: trade.order_id,
                symbol: trade.symbol,
                side: format!("{:?}", trade.side).to_uppercase(),
                quantity: trade.quantity.to_string(),
                price: trade.price.to_string(),
                total_value: trade.total_value.to_string(),
                cost_basis: cost_basis.to_string(),
                fees: fees.to_string(),
                realized_pnl: pnl.to_string(),
                realized_pnl_pct: trade.realized_pnl_pct.map(|p| p.to_string()),
                executed_at: trade.executed_at.to_rfc3339(),
            });
        }

        let trade_count = formatted_trades.len();
        let total_realized_pnl_pct = if total_cost_basis > Decimal::ZERO {
            ((total_realized_pnl / total_cost_basis) * dec!(100)).round_dp(4)
        } else {
            Decimal::ZERO
        };

        let win_rate = if trade_count > 0 {
            (winning_trades as f64 / trade_count as f64 * 10000.0).round() / 10000.0
        } else {
            0.0
        };

        let by_symbol = symbol_map
            .into_iter()
            .map(|(sym, (pnl, proceeds, cost, fees, count, wins, losses, breakevens))| {
                let pnl_pct = if cost > Decimal::ZERO {
                    ((pnl / cost) * dec!(100)).round_dp(4).to_string()
                } else {
                    "0.0000".to_string()
                };
                SymbolPnlBreakdown {
                    symbol: sym,
                    realized_pnl: pnl.to_string(),
                    realized_pnl_pct: pnl_pct,
                    proceeds: proceeds.to_string(),
                    cost_basis: cost.to_string(),
                    fees: fees.to_string(),
                    trade_count: count,
                    winning_trades: wins,
                    losing_trades: losses,
                    breakeven_trades: breakevens,
                }
            })
            .collect();

        Ok(RealizedPnlResponse {
            portfolio_id: portfolio.id,
            realized_pnl: total_realized_pnl.to_string(),
            realized_pnl_pct: total_realized_pnl_pct.to_string(),
            total_proceeds: total_proceeds.to_string(),
            total_cost_basis: total_cost_basis.to_string(),
            total_fees: total_fees.to_string(),
            trade_count,
            winning_trades,
            losing_trades,
            breakeven_trades,
            win_rate,
            by_symbol,
            trades: formatted_trades,
        })
    }

    pub async fn get_unrealized_pnl(
        state: &AppState,
        user_id: &str,
        portfolio_id: &str,
        reject_stale: bool,
        allow_stale: bool,
        max_age_seconds: Option<i64>,
    ) -> Result<UnrealizedPnlResponse, AppError> {
        let portfolio = sqlx::query_as::<_, Portfolio>(
            r#"SELECT * FROM "Portfolio" WHERE "id" = $1 AND "userId" = $2"#,
        )
        .bind(portfolio_id)
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("Portfolio not found.", "PORTFOLIO_NOT_FOUND"))?;

        let holdings = sqlx::query_as::<_, Holding>(
            r#"
            SELECT * FROM "Holding"
            WHERE "portfolioId" = $1 AND "quantity" > 0
            ORDER BY "symbol" ASC
            "#,
        )
        .bind(portfolio_id)
        .fetch_all(&state.pool)
        .await?;

        let mut positions = Vec::new();
        let mut total_invested = Decimal::ZERO;
        let mut total_market_value = Decimal::ZERO;
        let mut total_unrealized_pnl = Decimal::ZERO;
        let mut has_stale_position = false;
        let mut max_age = 0;

        for holding in holdings {
            let quote_opts = QuoteOptions {
                asset_class: Some(format!("{:?}", holding.asset_type).to_uppercase()),
                reject_stale: false,
                max_age_seconds: None,
                bypass_cache: false,
            };

            let quote = MarketDataService::get_quote(state, &holding.symbol, &quote_opts).await?;
            let current_price = Decimal::from_str(&quote.price).unwrap_or(Decimal::ZERO);
            let market_value = (holding.quantity * current_price).round_dp(8);
            let unrealized_pnl = market_value - holding.total_invested;
            let unrealized_pnl_pct = if holding.total_invested > Decimal::ZERO {
                ((unrealized_pnl / holding.total_invested) * dec!(100)).round_dp(4)
            } else {
                Decimal::ZERO
            };

            let is_stale = quote.is_stale.unwrap_or(false);
            let freshness = quote.freshness.unwrap_or_else(|| "FRESH".to_string());
            let age = quote.age_seconds.unwrap_or(0);

            if is_stale && reject_stale && !allow_stale {
                return Err(AppError::market_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "MARKET_DATA_STALE_QUOTE",
                    format!("Market data quote is stale for {}.", holding.symbol),
                ));
            }

            if let Some(max_sec) = max_age_seconds {
                if age > max_sec && !allow_stale {
                    return Err(AppError::market_error(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "MARKET_DATA_STALE_QUOTE",
                        format!("Market data quote is stale for {}.", holding.symbol),
                    ));
                }
            }

            if is_stale {
                has_stale_position = true;
            }
            if age > max_age {
                max_age = age;
            }

            total_invested += holding.total_invested;
            total_market_value += market_value;
            total_unrealized_pnl += unrealized_pnl;

            positions.push(PositionUnrealizedPnl {
                id: holding.id,
                symbol: holding.symbol,
                asset_type: format!("{:?}", holding.asset_type).to_uppercase(),
                is_short: false,
                quantity: holding.quantity.to_string(),
                average_buy_price: holding.average_buy_price.to_string(),
                total_invested: holding.total_invested.to_string(),
                current_price: current_price.to_string(),
                market_value: market_value.to_string(),
                unrealized_pnl: unrealized_pnl.to_string(),
                unrealized_pnl_pct: unrealized_pnl_pct.to_string(),
                is_stale,
                freshness,
                age_seconds: age,
                price_timestamp: quote.timestamp,
            });
        }

        let total_unrealized_pnl_pct = if total_invested > Decimal::ZERO {
            ((total_unrealized_pnl / total_invested) * dec!(100)).round_dp(4)
        } else {
            Decimal::ZERO
        };

        Ok(UnrealizedPnlResponse {
            portfolio_id: portfolio.id,
            total_invested: total_invested.to_string(),
            market_value: total_market_value.to_string(),
            unrealized_pnl: total_unrealized_pnl.to_string(),
            unrealized_pnl_pct: total_unrealized_pnl_pct.to_string(),
            is_stale: has_stale_position,
            freshness: if has_stale_position { "STALE".to_string() } else { "FRESH".to_string() },
            max_age_seconds: max_age,
            position_count: positions.len(),
            as_of: Utc::now().to_rfc3339(),
            positions,
        })
    }

    pub async fn get_aggregate_pnl(
        state: &AppState,
        user_id: &str,
        portfolio_id: &str,
        reject_stale: bool,
        allow_stale: bool,
        max_age_seconds: Option<i64>,
    ) -> Result<AggregatePnlResponse, AppError> {
        let portfolio = sqlx::query_as::<_, Portfolio>(
            r#"SELECT * FROM "Portfolio" WHERE "id" = $1 AND "userId" = $2"#,
        )
        .bind(portfolio_id)
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("Portfolio not found.", "PORTFOLIO_NOT_FOUND"))?;

        let realized = Self::get_realized_pnl(&state.pool, user_id, portfolio_id, None).await?;
        let unrealized = Self::get_unrealized_pnl(
            state,
            user_id,
            portfolio_id,
            reject_stale,
            allow_stale,
            max_age_seconds,
        )
        .await?;

        let starting_balance = portfolio.starting_balance;
        let cash_balance = portfolio.cash_balance;
        let open_holdings_value = Decimal::from_str(&unrealized.market_value).unwrap_or(Decimal::ZERO);
        let open_holdings_cost = Decimal::from_str(&unrealized.total_invested).unwrap_or(Decimal::ZERO);
        let total_portfolio_value = cash_balance + open_holdings_value;

        let realized_pnl = Decimal::from_str(&realized.realized_pnl).unwrap_or(Decimal::ZERO);
        let unrealized_pnl = Decimal::from_str(&unrealized.unrealized_pnl).unwrap_or(Decimal::ZERO);
        let net_pnl = realized_pnl + unrealized_pnl;

        let total_cost = open_holdings_cost + Decimal::from_str(&realized.total_cost_basis).unwrap_or(Decimal::ZERO);
        let net_pnl_pct = if total_cost > Decimal::ZERO {
            ((net_pnl / total_cost) * dec!(100)).round_dp(4)
        } else {
            Decimal::ZERO
        };

        let account_pnl = total_portfolio_value - starting_balance;
        let account_pnl_pct = if starting_balance > Decimal::ZERO {
            ((account_pnl / starting_balance) * dec!(100)).round_dp(4)
        } else {
            Decimal::ZERO
        };

        Ok(AggregatePnlResponse {
            portfolio_id: portfolio.id,
            currency: "USD".to_string(),
            as_of: Utc::now().to_rfc3339(),
            starting_balance: starting_balance.to_string(),
            cash_balance: cash_balance.to_string(),
            open_holdings_value: open_holdings_value.to_string(),
            open_holdings_cost_basis: open_holdings_cost.to_string(),
            total_portfolio_value: total_portfolio_value.to_string(),
            net_pnl: net_pnl.to_string(),
            net_pnl_pct: net_pnl_pct.to_string(),
            account_pnl: account_pnl.to_string(),
            account_pnl_pct: account_pnl_pct.to_string(),
            realized: RealizedSummary {
                pnl: realized.realized_pnl,
                pnl_pct: realized.realized_pnl_pct,
                proceeds: realized.total_proceeds,
                cost_basis: realized.total_cost_basis,
                fees: realized.total_fees.clone(),
                trade_count: realized.trade_count,
                winning_trades: realized.winning_trades,
                losing_trades: realized.losing_trades,
                breakeven_trades: realized.breakeven_trades,
                win_rate: realized.win_rate,
                by_symbol: realized.by_symbol,
            },
            unrealized: UnrealizedSummary {
                pnl: unrealized.unrealized_pnl,
                pnl_pct: unrealized.unrealized_pnl_pct,
                market_value: unrealized.market_value,
                cost_basis: unrealized.total_invested,
                position_count: unrealized.position_count,
                is_stale: unrealized.is_stale,
                freshness: unrealized.freshness,
                max_age_seconds: unrealized.max_age_seconds,
                positions: unrealized.positions,
            },
            reconciliation: ReconciliationSummary {
                is_reconciled: true,
                open_positions_count: unrealized.position_count,
                closed_trades_count: realized.trade_count,
                total_fees: realized.total_fees,
            },
        })
    }

    pub async fn get_snapshots(
        pool: &PgPool,
        user_id: &str,
        portfolio_id: &str,
        limit: usize,
    ) -> Result<Vec<PerformanceSnapshot>, AppError> {
        let _ = sqlx::query_as::<_, Portfolio>(
            r#"SELECT * FROM "Portfolio" WHERE "id" = $1 AND "userId" = $2"#,
        )
        .bind(portfolio_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("Portfolio not found.", "PORTFOLIO_NOT_FOUND"))?;

        let snapshots = sqlx::query_as::<_, PerformanceSnapshot>(
            r#"
            SELECT * FROM "PerformanceSnapshot"
            WHERE "portfolioId" = $1
            ORDER BY "snapshotDate" ASC
            LIMIT $2
            "#,
        )
        .bind(portfolio_id)
        .bind(limit as i64)
        .fetch_all(pool)
        .await?;

        Ok(snapshots)
    }

    pub async fn get_allocation(
        state: &AppState,
        user_id: &str,
        portfolio_id: &str,
        dimension: &str,
    ) -> Result<PortfolioAllocationResponse, AppError> {
        let portfolio = sqlx::query_as::<_, Portfolio>(
            r#"SELECT * FROM "Portfolio" WHERE "id" = $1 AND "userId" = $2"#,
        )
        .bind(portfolio_id)
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("Portfolio not found.", "PORTFOLIO_NOT_FOUND"))?;

        let unrealized = Self::get_unrealized_pnl(state, user_id, portfolio_id, false, true, None).await?;
        let cash = portfolio.cash_balance;
        let mut total_val = cash;

        let mut map: HashMap<String, (Option<String>, Decimal)> = HashMap::new();
        // Insert Cash
        map.insert("CASH".to_string(), (None, cash));

        for pos in unrealized.positions {
            let val = Decimal::from_str(&pos.market_value).unwrap_or(Decimal::ZERO);
            total_val += val;

            if dimension.eq_ignore_ascii_case("asset") || dimension.eq_ignore_ascii_case("assetType") {
                let entry = map.entry(pos.asset_type.clone()).or_insert((Some(pos.asset_type), Decimal::ZERO));
                entry.1 += val;
            } else {
                map.insert(pos.symbol.clone(), (Some(pos.asset_type), val));
            }
        }

        let mut items = Vec::new();
        let mut sum_formatted = Decimal::ZERO;
        let mut max_idx = 0;
        let mut max_val = Decimal::ZERO;

        for (idx, (label, (asset_type, val))) in map.into_iter().enumerate() {
            let raw_pct = if total_val > Decimal::ZERO {
                ((val / total_val) * dec!(100)).round_dp(4)
            } else {
                Decimal::ZERO
            };
            sum_formatted += raw_pct;

            if val > max_val {
                max_val = val;
                max_idx = idx;
            }

            items.push(AllocationItem {
                label,
                asset_type,
                value: val.to_string(),
                percentage: raw_pct.to_string(),
            });
        }

        // Reconcile residual against largest item
        let residual = dec!(100) - sum_formatted;
        if !residual.is_zero() && !items.is_empty() {
            if let Ok(cur_pct) = Decimal::from_str(&items[max_idx].percentage) {
                items[max_idx].percentage = (cur_pct + residual).round_dp(4).to_string();
            }
        }

        Ok(PortfolioAllocationResponse {
            portfolio_id: portfolio.id,
            dimension: dimension.to_string(),
            total_value: total_val.to_string(),
            items,
        })
    }

    pub async fn get_risk_metrics(
        pool: &PgPool,
        user_id: &str,
        portfolio_id: &str,
        period: &str,
        risk_free_rate: f64,
    ) -> Result<RiskMetricsResponse, AppError> {
        let _ = sqlx::query_as::<_, Portfolio>(
            r#"SELECT * FROM "Portfolio" WHERE "id" = $1 AND "userId" = $2"#,
        )
        .bind(portfolio_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("Portfolio not found.", "PORTFOLIO_NOT_FOUND"))?;

        let snapshots = sqlx::query_as::<_, PerformanceSnapshot>(
            r#"
            SELECT * FROM "PerformanceSnapshot"
            WHERE "portfolioId" = $1
            ORDER BY "snapshotDate" ASC
            "#,
        )
        .bind(portfolio_id)
        .fetch_all(pool)
        .await?;

        let n = snapshots.len();
        if n < 2 {
            return Ok(RiskMetricsResponse {
                portfolio_id: portfolio_id.to_string(),
                period: period.to_string(),
                annualized_volatility: None,
                max_drawdown: None,
                current_drawdown: None,
                peak_value: None,
                peak_date: None,
                sharpe_ratio: None,
                observation_count: n,
            });
        }

        let mut daily_returns = Vec::new();
        let mut running_peak = Decimal::ZERO;
        let mut peak_date = None;
        let mut max_drawdown = 0.0f64;
        let mut current_drawdown = 0.0f64;

        for (idx, snap) in snapshots.iter().enumerate() {
            let ret = snap.daily_pnl_pct.to_f64().unwrap_or(0.0);
            daily_returns.push(ret);

            if snap.total_value > running_peak {
                running_peak = snap.total_value;
                peak_date = Some(snap.snapshot_date.to_string());
            }

            if running_peak > Decimal::ZERO {
                let peak_f = running_peak.to_f64().unwrap_or(1.0);
                let val_f = snap.total_value.to_f64().unwrap_or(1.0);
                let dd = (val_f - peak_f) / peak_f;
                if dd < max_drawdown {
                    max_drawdown = dd;
                }
                if idx == n - 1 {
                    current_drawdown = dd;
                }
            }
        }

        // Calculate sample std dev
        let mean = daily_returns.iter().sum::<f64>() / n as f64;
        let var = daily_returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
        let std_dev = var.sqrt();
        let annual_factor = 365.0f64;
        let ann_vol = std_dev * annual_factor.sqrt();

        let r_ann = mean * annual_factor;
        let sharpe = if ann_vol > 0.0 {
            Some(format!("{:.2}", (r_ann - risk_free_rate) / ann_vol))
        } else {
            None
        };

        Ok(RiskMetricsResponse {
            portfolio_id: portfolio_id.to_string(),
            period: period.to_string(),
            annualized_volatility: Some(format!("{:.4}", ann_vol)),
            max_drawdown: Some(format!("{:.4}", max_drawdown)),
            current_drawdown: Some(format!("{:.4}", current_drawdown)),
            peak_value: Some(running_peak.to_string()),
            peak_date,
            sharpe_ratio: sharpe,
            observation_count: n,
        })
    }

    pub async fn get_pnl_heatmap(
        pool: &PgPool,
        user_id: &str,
        portfolio_id: &str,
        period: &str,
    ) -> Result<PnlHeatmapResponse, AppError> {
        let _ = sqlx::query_as::<_, Portfolio>(
            r#"SELECT * FROM "Portfolio" WHERE "id" = $1 AND "userId" = $2"#,
        )
        .bind(portfolio_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("Portfolio not found.", "PORTFOLIO_NOT_FOUND"))?;

        let snapshots = sqlx::query_as::<_, PerformanceSnapshot>(
            r#"
            SELECT * FROM "PerformanceSnapshot"
            WHERE "portfolioId" = $1
            ORDER BY "snapshotDate" ASC
            "#,
        )
        .bind(portfolio_id)
        .fetch_all(pool)
        .await?;

        let mut heatmap = Vec::new();
        let mut total_pnl = Decimal::ZERO;
        let mut win_days = 0;
        let mut loss_days = 0;
        let mut flat_days = 0;

        for s in snapshots {
            total_pnl += s.daily_pnl;
            if s.daily_pnl > Decimal::ZERO {
                win_days += 1;
            } else if s.daily_pnl < Decimal::ZERO {
                loss_days += 1;
            } else {
                flat_days += 1;
            }

            heatmap.push(HeatmapDay {
                date: s.snapshot_date.to_string(),
                pnl: s.daily_pnl.to_string(),
                pnl_percent: s.daily_pnl_pct.to_string(),
                total_value: s.total_value.to_string(),
            });
        }

        Ok(PnlHeatmapResponse {
            portfolio_id: portfolio_id.to_string(),
            period: period.to_string(),
            heatmap,
            summary: HeatmapSummary {
                total_pnl: total_pnl.to_string(),
                win_days,
                loss_days,
                flat_days,
            },
        })
    }

    pub async fn get_dashboard(
        state: &AppState,
        user_id: &str,
        portfolio_id: &str,
    ) -> Result<serde_json::Value, AppError> {
        let agg = Self::get_aggregate_pnl(state, user_id, portfolio_id, false, true, None).await?;

        let recent_trades = sqlx::query_as::<_, Trade>(
            r#"
            SELECT * FROM "Trade"
            WHERE "portfolioId" = $1
            ORDER BY "executedAt" DESC
            LIMIT 5
            "#,
        )
        .bind(portfolio_id)
        .fetch_all(&state.pool)
        .await?;

        let recent_orders = sqlx::query_as::<_, Order>(
            r#"
            SELECT * FROM "Order"
            WHERE "portfolioId" = $1
            ORDER BY "createdAt" DESC
            LIMIT 5
            "#,
        )
        .bind(portfolio_id)
        .fetch_all(&state.pool)
        .await?;

        let pending_orders_count: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "Order" WHERE "portfolioId" = $1 AND "status" = 'PENDING'"#,
        )
        .bind(portfolio_id)
        .fetch_one(&state.pool)
        .await?;

        let latest_snapshot = sqlx::query_as::<_, PerformanceSnapshot>(
            r#"
            SELECT * FROM "PerformanceSnapshot"
            WHERE "portfolioId" = $1
            ORDER BY "snapshotDate" DESC
            LIMIT 1
            "#,
        )
        .bind(portfolio_id)
        .fetch_optional(&state.pool)
        .await?;

        let mut top_positions = agg.unrealized.positions.clone();
        top_positions.sort_by(|a, b| {
            let b_val = Decimal::from_str(&b.market_value).unwrap_or(Decimal::ZERO);
            let a_val = Decimal::from_str(&a.market_value).unwrap_or(Decimal::ZERO);
            b_val.cmp(&a_val)
        });
        top_positions.truncate(5);

        Ok(serde_json::json!({
            "portfolio": {
                "id": agg.portfolio_id,
                "currency": agg.currency,
                "startingBalance": agg.starting_balance,
                "cashBalance": agg.cash_balance,
                "holdingsValue": agg.open_holdings_value,
                "totalValue": agg.total_portfolio_value,
                "netPnl": agg.net_pnl,
                "netPnlPct": agg.net_pnl_pct,
                "realizedPnl": agg.realized.pnl,
                "unrealizedPnl": agg.unrealized.pnl,
                "positionsCount": agg.unrealized.position_count,
                "closedTradesCount": agg.realized.trade_count,
                "pendingOrdersCount": pending_orders_count.0,
            },
            "recentTrades": recent_trades,
            "recentOrders": recent_orders,
            "topPositions": top_positions,
            "latestSnapshot": latest_snapshot,
        }))
    }
}
