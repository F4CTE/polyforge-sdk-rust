use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Paginated wrapper
// ---------------------------------------------------------------------------

/// A paginated API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub page: u64,
    #[serde(default)]
    pub limit: u64,
    #[serde(default)]
    pub total_pages: u64,
    #[serde(default)]
    pub has_next: bool,
}

// ---------------------------------------------------------------------------
// Markets
// ---------------------------------------------------------------------------

/// A prediction market.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub tokens: Vec<Token>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub resolved: Option<bool>,
    /// Captures any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A token inside a market.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub id: String,
    #[serde(default)]
    pub outcome: Option<String>,
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Parameters for listing markets.
#[derive(Debug, Default)]
pub struct ListMarketsParams {
    pub search: Option<String>,
    pub category: Option<String>,
    pub limit: Option<u32>,
    pub page: Option<u32>,
}

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Strategy status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StrategyStatus {
    Idle,
    Running,
    Paused,
    Paper,
}

/// Trading mode used when starting a strategy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TradingMode {
    Live,
    Paper,
}

/// A trading strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Strategy {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<StrategyStatus>,
    #[serde(default)]
    pub mode: Option<TradingMode>,
    #[serde(default)]
    pub pnl: Option<f64>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A strategy template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyTemplate {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Portfolio & Orders
// ---------------------------------------------------------------------------

/// The user's portfolio.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    #[serde(default)]
    pub balance: Option<f64>,
    #[serde(default)]
    pub positions: Vec<Position>,
    #[serde(default)]
    pub total_value: Option<f64>,
    #[serde(default)]
    pub total_pnl: Option<f64>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A portfolio position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    #[serde(default)]
    pub market_id: Option<String>,
    #[serde(default)]
    pub token_id: Option<String>,
    #[serde(default)]
    pub size: Option<f64>,
    #[serde(default)]
    pub avg_price: Option<f64>,
    #[serde(default)]
    pub current_price: Option<f64>,
    #[serde(default)]
    pub pnl: Option<f64>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// An order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    #[serde(default)]
    pub market_id: Option<String>,
    #[serde(default)]
    pub token_id: Option<String>,
    #[serde(default)]
    pub side: Option<String>,
    #[serde(default)]
    pub size: Option<f64>,
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Parameters for listing orders.
#[derive(Debug, Default)]
pub struct ListOrdersParams {
    pub limit: Option<u32>,
    pub status: Option<String>,
    pub strategy_id: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

/// Parameters for closing a position.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ClosePositionParams {
    #[serde(rename = "tokenId")]
    pub token_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
}

/// Parameters for redeeming a resolved position.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct RedeemPositionParams {
    #[serde(rename = "tokenId")]
    pub token_id: String,
    #[serde(rename = "conditionId", skip_serializing_if = "Option::is_none")]
    pub condition_id: Option<String>,
}

/// Parameters for splitting a position.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SplitPositionParams {
    #[serde(rename = "tokenId")]
    pub token_id: String,
    pub size: f64,
    pub price: f64,
}

/// Parameters for merging positions.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MergePositionParams {
    #[serde(rename = "tokenIds")]
    pub token_ids: Vec<String>,
}

/// Parameters for placing a direct order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceOrderParams {
    #[serde(rename = "tokenId")]
    pub token_id: String,
    pub side: String,
    pub outcome: String,
    pub size: f64,
    pub price: f64,
    #[serde(rename = "orderType", skip_serializing_if = "Option::is_none")]
    pub order_type: Option<String>,
}

/// Response from placing a direct order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceOrderResponse {
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "intentId")]
    pub intent_id: String,
    pub status: String,
}

/// Response from cancelling an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelOrderResponse {
    #[serde(rename = "orderId")]
    pub order_id: String,
    pub status: String,
}

/// Trader score / reputation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraderScore {
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub rank: Option<u64>,
    #[serde(default)]
    pub total_trades: Option<u64>,
    #[serde(default)]
    pub win_rate: Option<f64>,
    #[serde(default)]
    pub profit_factor: Option<f64>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Social & Signals
// ---------------------------------------------------------------------------

/// A whale trade from the social feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhaleTrade {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub market_id: Option<String>,
    #[serde(default)]
    pub side: Option<String>,
    #[serde(default)]
    pub size: Option<f64>,
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default)]
    pub trader: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A news-based trading signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsSignal {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub headline: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub confidence: Option<u32>,
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default)]
    pub market_id: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Configuration: Alerts, Copy Trading, Webhooks
// ---------------------------------------------------------------------------

/// A price or event alert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    #[serde(default)]
    pub market_id: Option<String>,
    #[serde(default)]
    pub condition: Option<String>,
    #[serde(default)]
    pub threshold: Option<f64>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A copy-trading configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyConfig {
    pub id: String,
    #[serde(default)]
    pub source_trader: Option<String>,
    #[serde(default)]
    pub max_size: Option<f64>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Webhook event types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WebhookEvent {
    OrderFilled,
    StrategyError,
    WhaleTrade,
    NewsSignal,
    BacktestComplete,
    DailyLossLimit,
    MarketResolved,
    PriceAlert,
}

/// A registered webhook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Webhook {
    pub id: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub events: Vec<WebhookEvent>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// AI
// ---------------------------------------------------------------------------

/// Response from the AI query endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiQueryResponse {
    #[serde(default)]
    pub answer: Option<String>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub sources: Vec<serde_json::Value>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Strategy Execution Events (SSE)
// ---------------------------------------------------------------------------

/// A single event emitted by the strategy execution SSE stream.
///
/// Iterate over these via [`crate::client::StrategyEventStream::next`].
///
/// Common event types: `CONNECTED`, `STRATEGY_STARTED`, `STRATEGY_STOPPED`,
/// `STRATEGY_ERROR`, `ORDER_PLACED`, `ORDER_FILLED`, `ORDER_CANCELLED`,
/// `BACKTEST_PROGRESS`, `BACKTEST_COMPLETED`, `BACKTEST_FAILED`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyEvent {
    /// Event type identifier.
    #[serde(rename = "type")]
    pub event_type: String,
    /// The strategy this event belongs to.
    #[serde(rename = "strategyId", default)]
    pub strategy_id: Option<String>,
    /// Event-specific payload (varies per type).
    #[serde(default)]
    pub data: serde_json::Value,
    /// Unix millisecond timestamp when the event was emitted server-side.
    #[serde(default)]
    pub timestamp: u64,
}
