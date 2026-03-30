# polyforge

Async Rust SDK for the [Polyforge](https://polyforge.io) trading platform REST API.

## Installation

```toml
[dependencies]
polyforge = "1.0"
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
polyforge = { version = "1.0", default-features = false, features = ["native-tls"] }
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
| `PolyforgeClient::new(api_key)` | Connect to `http://localhost:3002` |
| `PolyforgeClient::with_url(api_key, url)` | Connect to a custom URL |

### Markets

| Method | Description |
|--------|-------------|
| `list_markets(params)` | List markets with search, category, pagination |
| `get_market(id)` | Get a single market by ID |

### Strategies

| Method | Description |
|--------|-------------|
| `list_strategies(status)` | List strategies, optionally filtered by status |
| `get_strategy(id)` | Get a strategy by ID |
| `create_strategy(name, description)` | Create a new strategy |
| `create_strategy_from_description(desc, market_id)` | AI-powered strategy creation |
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

**Common event types:** `CONNECTED` · `STRATEGY_STARTED` · `STRATEGY_STOPPED` · `STRATEGY_ERROR` · `ORDER_PLACED` · `ORDER_FILLED` · `ORDER_CANCELLED` · `BACKTEST_PROGRESS` · `BACKTEST_COMPLETED` · `BACKTEST_FAILED`

### Portfolio & Orders

| Method | Description |
|--------|-------------|
| `get_portfolio()` | Current portfolio and positions |
| `get_orders(params)` | List orders with optional filters |
| `get_score()` | Trader score and reputation |
| `place_order(params)` | Place a direct buy/sell order |
| `cancel_order(order_id)` | Cancel a pending or live order |

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

MIT
