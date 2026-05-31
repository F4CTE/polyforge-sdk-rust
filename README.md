# polyforge

Async Rust SDK for the [Polyforge](https://polyforge.io) trading platform REST API.

## Installation

```toml
[dependencies]
polyforge = "3.0"
tokio = { version = "1", features = ["full"] }
```

Or via the command line:

```sh
cargo add polyforge
cargo add tokio --features full
```

### TLS backends

The crate uses `rustls` by default. To use the platform-native TLS instead:

```toml
polyforge = { version = "3.0", default-features = false, features = ["native-tls"] }
```

## Quick Start

```rust
use polyforge::{PolyforgeClient, ListMarketsParams};

#[tokio::main]
async fn main() -> polyforge::Result<()> {
    let client = PolyforgeClient::new("your-api-key");

    // List markets
    let markets = client.list_markets(&ListMarketsParams {
        category: Some("politics".into()),
        limit: Some(10),
        ..Default::default()
    }).await?;

    for m in &markets.data {
        println!("{}: {}", m.id, m.title);
    }

    // Get portfolio
    let portfolio = client.get_portfolio().await?;
    println!("Balance: {:?}", portfolio.balance);

    Ok(())
}
```

## API Reference

### Client Construction

| Method | Description |
|--------|-------------|
| `PolyforgeClient::new(api_key)` | Use `POLYFORGE_API_URL` when set; otherwise connect to `https://api.polyforge.app` |
| `PolyforgeClient::with_url(api_key, url)` | Connect to a custom URL, including local/dev endpoints such as `http://localhost:3002` |

### Markets

| Method | Description |
|--------|-------------|
| `list_markets(params)` | List markets with search, category, sort, closed filter, pagination |
| `get_market(id)` | Get a single market by ID |

### Strategies

| Method | Description |
|--------|-------------|
| `list_strategies(params)` | List strategies with optional status filter, sorting, and pagination |
| `get_strategy(id)` | Get a strategy by ID |
| `create_strategy(name, description)` | Create a new strategy |
| `create_strategy_from_description(desc, market_id)` | AI-powered strategy creation |
| `get_strategy_health(id)` | Get execution health metrics for a strategy |
| `start_strategy(id, mode)` | Start a strategy (live or paper) |
| `stop_strategy(id)` | Stop a running strategy |
| `get_strategy_templates()` | List available templates |
| `export_strategy(id)` | Export strategy config as JSON |
| `watch_strategy(id)` | Stream live execution events via SSE |

### Live Execution Watching

`watch_strategy` opens a persistent SSE connection and returns a `StrategyEventStream`. Poll it with `.next().await` — drop the struct to close the connection.

```rust
use polyforge::{PolyforgeClient, StrategyEventStream};

#[tokio::main]
async fn main() -> polyforge::Result<()> {
    let client = PolyforgeClient::new("your-api-key");

    client.start_strategy("strat-uuid", polyforge::TradingMode::Paper).await?;

    let mut stream = client.watch_strategy("strat-uuid").await?;
    while let Some(event) = stream.next().await {
        let event = event?;
        match event.event_type.as_str() {
            "CONNECTED"         => println!("Stream live"),
            "ORDER_FILLED"      => println!("Filled: {:?}", event.data),
            "BACKTEST_PROGRESS" => println!("Progress: {:?}", event.data),
            "STRATEGY_STOPPED" | "BACKTEST_COMPLETED" => break,
            _ => {}
        }
    }
    Ok(())
}
```

**`StrategyEvent` fields:** `event_type: String` · `strategy_id: Option<String>` · `data: serde_json::Value` · `timestamp: u64`

**Common event types:** `CONNECTED` · `STRATEGY_STARTED` · `STRATEGY_STOPPED` · `STRATEGY_PAUSED` · `STRATEGY_RESUMED` · `STRATEGY_ERROR` · `ORDER_PLACED` · `ORDER_SUBMITTED` · `ORDER_PARTIAL` · `ORDER_FILLED` · `ORDER_FAILED` · `ORDER_CANCELLED` · `ORDER_ERROR` · `BACKTEST_PROGRESS` · `BACKTEST_COMPLETED` · `BACKTEST_FAILED`

### Portfolio & Orders

Trading write methods automatically attach a fresh 32-character `Idempotency-Key`
header on each request, matching the platform requirement for order placement,
liquidity, position, smart-order, and conditional-order mutations.

| Method | Description |
|--------|-------------|
| `get_portfolio()` | Current portfolio and positions |
| `get_orders(params)` | List orders with optional filters |
| `get_score()` | Trader score and reputation |
| `place_order(params)` | Place a direct buy/sell order |
| `cancel_order(order_id)` | Cancel a pending or live order |

### Arbitrage

Cross-venue arbitrage between Polymarket and Kalshi.  Key points:

- **Sweep-close only** — arbitrage positions are always closed in full; partial closes are not supported.
- **Idempotency-Key required** — `execute_arbitrage` and `close_arbitrage_position` both require a caller-supplied idempotency key (8–128 characters).  The SDK sends it as the `Idempotency-Key` header and the backend enforces at-most-once semantics per key.
- **Rate limit** — the backend enforces 5 req/min/user on execute and close; exceeding it returns HTTP 429.
- **UUID match_id** — `match_id` must be a valid RFC 4122 UUID; non-UUID input is rejected with HTTP 400.

```rust
use polyforge::{PolyforgeClient, PolyforgeError, ExecuteArbitrageParams};
use uuid::Uuid;

#[tokio::main]
async fn main() -> polyforge::Result<()> {
    let client = PolyforgeClient::new("your-api-key")?;

    // Execute an arbitrage trade
    let idempotency_key = Uuid::new_v4().to_string();
    let result = client
        .execute_arbitrage(
            &ExecuteArbitrageParams {
                match_id: "550e8400-e29b-41d4-a716-446655440000".into(),
                size: 100,
                max_slippage_pct: Some(0.5),
            },
            &idempotency_key,
        )
        .await?;
    println!("Opened position: {:?}", result.arb_position_id);

    // Later: sweep-close the position
    let pos_id = result.arb_position_id.as_deref().ok_or_else(|| {
        PolyforgeError::Validation(
            "execute_arbitrage response missing arb_position_id".into(),
        )
    })?;
    let close_key = Uuid::new_v4().to_string();
    let close_result = client
        .close_arbitrage_position(pos_id, &close_key)
        .await?;
    println!("Close status: {:?}", close_result.status);

    Ok(())
}
```

| Method | Description |
|--------|-------------|
| `list_arbitrage_opportunities(min_spread)` | List cross-venue Polymarket/Kalshi opportunities |
| `get_arbitrage_comparison(match_id)` | Compare prices for a matched cross-venue market |
| `execute_arbitrage(params, idempotency_key)` | Execute a real cross-venue arbitrage trade; sends `Idempotency-Key` and validates UUID `match_id`, integer `size` 1..=10000, and optional slippage 0..=5 |
| `list_arbitrage_positions(status, limit, offset)` | List arbitrage positions with typed `ArbPositionStatus` and `limit` 1..=100 |
| `get_arbitrage_position(position_id)` | Fetch one arbitrage position |
| `close_arbitrage_position(position_id, idempotency_key)` | Sweep-close an open arbitrage position with real reverse orders and `Idempotency-Key` |
| `get_arbitrage_risk_dashboard()` | Get aggregate arbitrage exposure and P&L |
| `get_arbitrage_settlement_risks()` | List settlement-date and resolution-criteria risks |
| `refresh_arbitrage_pnl()` | Recompute unrealized arbitrage P&L |

### Social & Signals

| Method | Description |
|--------|-------------|
| `get_whale_feed(min_size)` | Large trades feed |
| `get_news_signals(min_confidence)` | AI news signals |

### Configuration

| Method | Description |
|--------|-------------|
| `list_alerts()` | List price/event alerts |
| `list_copy_configs()` | List copy-trading configs |
| `list_webhooks()` | List webhooks |
| `create_webhook(url, events)` | Register a new webhook |

### AI

| Method | Description |
|--------|-------------|
| `ai_query(query)` | Natural-language AI query |

### Accuracy & AI Insights

| Method | Description |
|--------|-------------|
| `get_accuracy()` | Brier score, win rate, calibration buckets, and per-category breakdown |
| `get_accuracy_leaderboard(params)` | Paginated accuracy leaderboard ranked by prediction accuracy with P&L, win rate, trade count |
| `get_portfolio_review()` | AI-generated portfolio review with suggestions and score (1–10) |
| `get_market_sentiment(market_id)` | Sentiment score (−100 to +100) with BULLISH / BEARISH / NEUTRAL label |

### Liquidity Provisioning

| Method | Description |
|--------|-------------|
| `provide_liquidity(params)` | Post liquidity; returns `LpPosition` with buy and sell order IDs |

```rust
// Accuracy & AI
client.get_accuracy().await?
client.get_portfolio_review().await?
client.get_market_sentiment("market-id").await?
client.provide_liquidity(&ProvideLiquidityParams { token_id, spread, size }).await?
```

### Rewards

| Method | Description |
|--------|-------------|
| `list_rewards_markets()` | `Vec<RewardMarket>` — all markets with active liquidity rewards |
| `get_rewards_for_market(condition_id)` | `RewardMarketDetail` — reward details for a specific market by condition ID |
| `get_market_rewards_detail(market_id)` | `Option<RewardsMarketDetail>` — CLOB liquidity-reward details by platform market ID; returns `None` when the market has no active rewards config |
| `get_rewards_sponsor_url(market_id)` | `RewardsSponsorUrl` — Polymarket sponsor page URL for a market |
| `get_user_rewards()` | `UserRewards` — authenticated user's accrued liquidity rewards |
| `get_user_rewards_total()` | `UserRewardsTotal` — total accumulated rewards with date breakdown |
| `get_user_rewards_percentages()` | `UserRewardsPercentages` — reward allocation percentages |
| `get_user_rewards_per_market()` | `UserRewardsPerMarket` — rewards broken down by individual market |
| `get_user_sponsored_markets()` | `UserSponsoredMarkets` — authenticated user's sponsored rewards markets |
| `get_rebates()` | `Rebates` — Polymarket trading rebates earned from trading activity |

### Sports

| Method | Description |
|--------|-------------|
| `list_sports_categories()` | `Vec<SportsCategory>` — labelled categories with series tickers and market counts |
| `list_sports_markets(params)` | `PaginatedResponse<serde_json::Value>` — sports markets with optional category / search / live-only filters |
| `list_sports_events(params)` | `PaginatedResponse<serde_json::Value>` — sports events filtered by category / series / status |
| `get_sports_event(event_ticker)` | `serde_json::Value` — `{ event, markets }` for one event |
| `list_sports_milestones(params)` | `serde_json::Value` — `{ milestones, cursor }` |
| `get_sports_live_data(milestone_id)` | `serde_json::Value` — `{ liveData }` snapshot |
| `list_sports_combos(params)` | `serde_json::Value` — `{ collections, cursor }` |
| `get_sports_combo_collection(collection_ticker)` | `serde_json::Value` — single combo collection |
| `lookup_sports_combo(params)` | `serde_json::Value` — `{ eventTicker, marketTicker }` or `null` for the combo matching the supplied legs |

```rust
use polyforge::{
    ListSportsMarketsParams, SportsComboLookupParams, SportsComboSelection,
};

let cats = client.list_sports_categories().await?;

let markets = client
    .list_sports_markets(&ListSportsMarketsParams {
        category: Some("NBA".into()),
        live_only: Some(true),
        sort: Some("closing_soon".into()),
        ..Default::default()
    })
    .await?;

let combo = client
    .lookup_sports_combo(&SportsComboLookupParams {
        collection_ticker: "KXNBACOMBO".into(),
        selected_markets: vec![SportsComboSelection {
            market_ticker: "KXNBAGAME-25-LAL".into(),
            event_ticker: "KXNBAGAME-25".into(),
            side: "yes".into(),
        }],
    })
    .await?;
```

Sports payloads are intentionally permissive (`serde_json::Value`) where the
upstream controller types them as `Record<string, unknown>` / `unknown[]` —
the SDK mirrors that fidelity instead of inventing strict shapes that could
drift from the server.

### Account & Data Export

| Method | Description |
|--------|-------------|
| `export_personal_data()` | Download GDPR personal-data export as typed `PersonalDataExport` JSON (requires JWT/API-key with `READ` scope) |
| `export_personal_data_csv()` | Download GDPR personal-data export as raw CSV text |
| `export_orders_csv()` | Download order history as CSV text |
| `export_portfolio_csv()` | Download portfolio positions as CSV text |

```rust
// GDPR personal-data export (JSON)
let data = client.export_personal_data().await?;
println!("Generated at: {}", data.generated_at.as_deref().unwrap_or("N/A"));
println!("Account: {:?}", data.account);

// GDPR personal-data export (CSV)
let csv = client.export_personal_data_csv().await?;
std::fs::write("polyforge-export.csv", csv)?;
```

## Error Handling

All methods return `polyforge::Result<T>`. Errors are represented by `PolyforgeError`:

- `Api` -- the server returned a non-2xx response with a code and message.
- `Http` -- a transport-level error (network, timeout, etc.).
- `Json` -- failed to serialize or deserialize JSON.

```rust
use polyforge::PolyforgeError;

match client.get_market("invalid").await {
    Ok(m) => println!("{}", m.title),
    Err(PolyforgeError::Api { status, message, .. }) => {
        eprintln!("API error {status}: {message}");
    }
    Err(e) => eprintln!("Other error: {e}"),
}
```

## Testing

```bash
cargo test
```

## License

Apache 2.0 — see [LICENSE](LICENSE) for details.
