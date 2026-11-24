# Stox Backend (Rust / Axum)

High-performance, memory-safe, asynchronous rewrite of the Stox financial trading simulation and social platform backend using **Rust**, **Axum 0.8**, **SQLx (PostgreSQL)**, and **Redis**.

---

## 🚀 Key Architectural Highlights

1. **Framework & Runtime**:
   - **Axum 0.8** with **Tokio** multi-threaded asynchronous runtime.
   - **Tower & Tower-HTTP**: Security headers (HSTS, CSP, XSS protection, anti-clickjacking), CORS, request tracking, rate limiting.
   - Exact financial precision: `rust_decimal::Decimal` (128-bit fixed-point decimal arithmetic) preserving 8-to-18 decimal places for prices, quantities, valuations, cash balances, and P&L.

2. **Database & Persistence**:
   - **SQLx 0.8** connection pooling with PostgreSQL.
   - Native mapping of screaming snake case database enums (`OrderStatus`, `OrderType`, `OrderSide`, `AssetType`, `LeagueStatus`, `LeaderboardScope`, `LeaderboardPeriod`, `Sentiment`, etc.).
   - Atomic transactions (`SERIALIZABLE` / `FOR UPDATE` row locks) for orders, cash reservations, copy trading, and league capacity checks.

3. **Multi-Provider Market Data Engine**:
   - Support for **Binance** (spot websocket/REST ticker) and **Finnhub** (equities quotes & candles) with **Yahoo Finance** fallback.
   - Intelligent symbol resolution: handles asset prefixes (`BINANCE:`, `FINNHUB:`), quote asset suffixes (`USDT`, `FDUSD`, etc.), and ambiguous stock tickers (`GBTC`, `COIN`, `RIOT`, `MSTR`, `BITO`).
   - Redis-backed distributed cache with TTL and stale data grace windows.

4. **Trading & Execution Engine**:
   - Limit, Market, and Stop-Loss orders with atomic cash reservation and reversal.
   - Average cost basis tracking, realized and mark-to-market unrealized P&L calculation.
   - Order cancellation, expiration, and automated trade execution.

5. **Gamification & Social**:
   - Real-time and snapshot-based Global & Friends Leaderboards with period aggregation (`DAILY`, `WEEKLY`, `MONTHLY`, `ALL_TIME`).
   - Dedicated league competition portfolios, strict capacity caps, and mark-to-market percentage return rankings.
   - Automated achievement unlock pipeline (`FIRST_TRADE`, `FIRST_PROFIT`, `TEN_SUCCESSFUL_TRADES`, `PORTFOLIO_MILESTONE`).
   - Crowd sentiment voting system (`Bullish`, `Bearish`, `Neutral`) with 5-vote threshold and consensus calculation.
   - Follower graph, feed generation with decay scoring, and copy trading pipeline.

6. **Real-time Streaming & WebSockets**:
   - `/ws` WebSocket endpoint supporting channel subscriptions (`market:<symbol>`, `trading:<userId>`, `notifications:<userId>`).
   - Tokio broadcast channels bridging market ticks and order execution events directly to connected web clients.

7. **Background Schedulers**:
   - Order expiration worker (scans and settles overdue pending orders).
   - Price alert evaluation loop (evaluates active alerts against live market quotes and delivers notifications).
   - League status synchronization loop (transitions competitions between `Upcoming`, `Active`, and `Completed`).

---

## 🛠️ Project Structure

```
backend-rust/
├── Cargo.toml
├── src/
│   ├── main.rs                   # Entrypoint: sets up listeners, state, routes & workers
│   ├── lib.rs                    # Exported library root
│   ├── state.rs                  # Shared AppState (pool, redis, broadcast channels)
│   ├── config.rs                 # Typed configuration from environment
│   ├── error.rs                  # Standardized AppError and response envelope
│   ├── middleware/
│   │   ├── auth.rs               # JWT Bearer authentication extractor (AuthUser)
│   │   ├── request_context.rs    # Correlation ID (x-request-id) injection
│   │   ├── security_headers.rs   # Comprehensive HTTP security headers
│   │   └── rate_limiter.rs       # In-memory / distributed rate limiting
│   ├── models/
│   │   ├── mod.rs                # Domain structs matching PostgreSQL schema
│   │   └── enums.rs              # Strongly-typed database enums
│   ├── services/
│   │   ├── auth_service.rs
│   │   ├── user_service.rs
│   │   ├── portfolio_service.rs
│   │   ├── portfolio_analytics.rs
│   │   ├── order_service.rs
│   │   ├── trade_service.rs
│   │   ├── market_data_service.rs
│   │   ├── symbol_mapping.rs
│   │   ├── alert_service.rs
│   │   ├── social_service.rs
│   │   ├── copy_trading_service.rs
│   │   ├── leaderboard_service.rs
│   │   ├── competition_service.rs
│   │   ├── achievement_service.rs
│   │   ├── sentiment_service.rs
│   │   ├── user_preference_service.rs
│   │   ├── notification_service.rs
│   │   └── admin_service.rs
│   ├── routes/
│   │   ├── mod.rs                # API v1 route aggregator
│   │   ├── health_routes.rs      # /health & /api/v1/health probe
│   │   ├── auth_routes.rs
│   │   ├── user_routes.rs
│   │   ├── portfolio_routes.rs
│   │   ├── order_routes.rs
│   │   ├── trade_routes.rs
│   │   ├── market_routes.rs
│   │   ├── alert_routes.rs
│   │   ├── social_routes.rs
│   │   ├── copy_trading_routes.rs
│   │   ├── leaderboard_routes.rs
│   │   ├── league_routes.rs
│   │   ├── achievement_routes.rs
│   │   ├── sentiment_routes.rs
│   │   ├── preference_routes.rs
│   │   ├── notification_routes.rs
│   │   └── admin_routes.rs
│   ├── realtime/
│   │   ├── mod.rs
│   │   └── websocket.rs          # /ws client connection and subscription handler
│   └── workers/
│       └── mod.rs                # Background tokio workers (orders, alerts, leagues)
└── tests/
    └── unit_tests.rs             # Unit and integration test suite
```

---

## 🏃 Getting Started

### Prerequisites
- Rust 1.80+ (with Cargo)
- Running PostgreSQL database (matching Stox schema)
- Running Redis instance (optional, graceful fallback provided)

### Environment Setup
Environment files are copied directly from the original project:
- `.env`
- `.env.example`
- `.env.2`

### Run Tests
```bash
cargo test
```

### Build and Run Server
```bash
# Debug run
cargo run

# Production release build
cargo build --release
./target/release/stox-backend-rust
```
The server will start on port `8000` (or `PORT` specified in `.env`).

---

## 📡 Key API Routes

| Endpoint | Method | Description |
|---|---|---|
| `/health` & `/api/v1/health` | GET | Database connectivity and latency health check |
| `/metrics` | GET | Prometheus metrics exporter |
| `/ws` | GET | WebSocket streaming upgrade |
| `/api/v1/auth/register` | POST | User registration |
| `/api/v1/auth/login` | POST | User login & JWT issuance |
| `/api/v1/auth/refresh` | POST | Token rotation |
| `/api/v1/portfolios` | GET / POST | List user portfolios / create new portfolio |
| `/api/v1/portfolios/:id/valuation` | GET | Real-time mark-to-market valuation |
| `/api/v1/portfolios/:id/analytics/dashboard`| GET | Complete P&L, Sharpe ratio, risk metrics |
| `/api/v1/orders` | GET / POST | Order history / place new order |
| `/api/v1/market/quote/:symbol` | GET | Multi-provider real-time quotes |
| `/api/v1/leaderboard` | GET | Snapshot global leaderboard |
| `/api/v1/leagues` | GET / POST | List competitions / create competition |
| `/api/v1/leagues/:id/join` | POST | Atomic join with dedicated competition portfolio |
| `/api/v1/achievements` | GET | Achievement catalog and user unlocks |
| `/api/v1/sentiment/:symbol` | GET / POST | Read aggregate sentiment / cast user vote |
| `/api/v1/admin/orders` | GET | Admin: system-wide order audit |
