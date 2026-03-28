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

### Portfolio & Orders

| Method | Description |
|--------|-------------|
| `get_portfolio()` | Current portfolio and positions |
| `get_orders(params)` | List orders with optional filters |
| `get_score()` | Trader score and reputation |

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
