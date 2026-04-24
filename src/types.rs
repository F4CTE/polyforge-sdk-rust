use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Paginated wrapper
// ---------------------------------------------------------------------------

/// Pagination metadata nested under the `pagination` key.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    pub page: u64,
    pub limit: u64,
    pub total: u64,
    pub total_pages: u64,
}

/// A paginated API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub pagination: Pagination,
}

// ---------------------------------------------------------------------------
// Markets
// ---------------------------------------------------------------------------

/// A prediction market.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Market {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub price: Option<String>,
    #[serde(rename = "volume24h", default)]
    pub volume_24h: Option<String>,
    #[serde(rename = "change24h", default)]
    pub change_24h: Option<String>,
    #[serde(default)]
    pub liquidity: Option<String>,
    #[serde(default)]
    pub tokens: Vec<Token>,
    #[serde(rename = "createdAt", default)]
    pub created_at: Option<String>,
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
    /// Sort order: `"volume"`, `"endDate"`, `"firstSeenAt"`, `"newest"`, `"closing_soon"`, `"liquidity"`.
    pub sort: Option<String>,
    /// Whether to include closed/resolved markets.
    pub closed: Option<bool>,
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
    Error,
    Paper,
    Archived,
}

/// Parameters for listing strategies.
#[derive(Debug, Default)]
pub struct ListStrategiesParams {
    /// Filter by status: `IDLE`, `RUNNING`, `PAUSED`, `PAPER`, etc.
    pub status: Option<StrategyStatus>,
    /// Sort order: `"createdAt"`, `"updatedAt"`, `"name"`, `"status"`, `"likeCount"`.
    pub sort: Option<String>,
    /// Page number (1-based, default 1).
    pub page: Option<u32>,
    /// Items per page (default 20, max 100).
    pub limit: Option<u32>,
}

/// Strategy visibility.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Visibility {
    Private,
    Public,
    Unlisted,
}

/// Strategy execution mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecMode {
    Tick,
    Event,
    Hybrid,
}

/// A strategy block (trigger, condition, action, or safety).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "type", default)]
    pub block_type: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub config: Option<serde_json::Value>,
    #[serde(default)]
    pub connections: Vec<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A logic block used in strategy flow control.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicBlock {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "type", default)]
    pub block_type: Option<String>,
    #[serde(default)]
    pub config: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A calculation block used in strategy computations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalcBlock {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "type", default)]
    pub block_type: Option<String>,
    #[serde(default)]
    pub config: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Trading mode read from a strategy response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TradingMode {
    #[serde(rename = "live")]
    Live,
    #[serde(rename = "paper")]
    Paper,
}

/// Deployment mode sent when starting a strategy.
///
/// Platform contract: `"LIVE"` or `"SIMULATION"`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeploymentMode {
    #[serde(rename = "LIVE")]
    Live,
    #[serde(rename = "SIMULATION")]
    Simulation,
}

/// Parameters for [`PolyforgeClient::start_strategy`].
///
/// Platform contract (POST `/api/v1/strategies/{id}/start`):
/// `{ "paperMode": bool, "deploymentMode"?: "LIVE"|"SIMULATION" }`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartStrategyParams {
    pub paper_mode: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_mode: Option<DeploymentMode>,
}

impl StartStrategyParams {
    /// Convenience constructor: paper trading with no explicit deployment mode.
    pub fn paper() -> Self {
        Self { paper_mode: true, deployment_mode: None }
    }

    /// Convenience constructor: live trading with no explicit deployment mode.
    pub fn live() -> Self {
        Self { paper_mode: false, deployment_mode: None }
    }
}

/// A trading strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    pub visibility: Option<Visibility>,
    #[serde(default)]
    pub exec_mode: Option<ExecMode>,
    #[serde(default)]
    pub tick_ms: Option<u64>,
    #[serde(default)]
    pub triggers: Vec<Block>,
    #[serde(default)]
    pub conditions: Vec<Block>,
    #[serde(default)]
    pub actions: Vec<Block>,
    #[serde(default)]
    pub safety: Vec<Block>,
    #[serde(default)]
    pub logic_blocks: Option<Vec<LogicBlock>>,
    #[serde(default)]
    pub calc_blocks: Option<Vec<CalcBlock>>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub version: Option<u32>,
    #[serde(default)]
    pub fork_count: Option<u64>,
    #[serde(default)]
    pub like_count: Option<u64>,
    #[serde(default)]
    pub pnl: Option<f64>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Response from strategy lifecycle operations (start/stop/pause/resume).
///
/// The platform returns a minimal status object rather than the full `Strategy`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyStatusResponse {
    pub status: StrategyStatus,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub stopped_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A strategy template with block configuration and popularity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyTemplate {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    /// The template's block configuration.
    #[serde(default)]
    pub blocks: Vec<Block>,
    /// Usage/popularity score.
    #[serde(default)]
    pub popularity: u32,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Parameters for creating a strategy with full block configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateStrategyParams {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<Visibility>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec_mode: Option<ExecMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tick_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub triggers: Option<Vec<Block>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<Block>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<Block>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety: Option<Vec<Block>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logic_blocks: Option<Vec<LogicBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calc_blocks: Option<Vec<CalcBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canvas: Option<serde_json::Value>,
}

/// Parameters for running a backtest.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunBacktestParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_range_start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_range_end: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quick_mode: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy_blocks: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_bindings: Option<std::collections::HashMap<String, String>>,
}

/// Parameters for listing backtests with optional filtering.
#[derive(Debug, Clone, Default)]
pub struct ListBacktestsParams {
    pub strategy_id: Option<String>,
    pub status: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

/// A backtest result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backtest {
    pub id: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub strategy_id: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub initial_balance: Option<f64>,
    #[serde(default)]
    pub final_balance: Option<f64>,
    #[serde(default)]
    pub pnl: Option<f64>,
    #[serde(default)]
    pub trade_count: Option<i64>,
    #[serde(default)]
    pub win_rate: Option<f64>,
    #[serde(default)]
    pub sharpe_ratio: Option<f64>,
    #[serde(default)]
    pub max_drawdown: Option<f64>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Portfolio & Orders
// ---------------------------------------------------------------------------

/// The user's portfolio.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Portfolio {
    #[serde(rename = "availableBalance", default)]
    pub available_balance: Option<String>,
    #[serde(default)]
    pub positions: Vec<Position>,
    #[serde(default)]
    pub total_value: Option<String>,
    #[serde(rename = "unrealizedPnl", default)]
    pub unrealized_pnl: Option<String>,
    #[serde(rename = "realizedPnl", default)]
    pub realized_pnl: Option<String>,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A portfolio position.
///
/// Field names match the platform position response: `id`, `marketId`,
/// `tokenId`, `side`, `size`, `avgPrice`, `currentPrice`,
/// `unrealizedPnl`, `realizedPnl`, `openedAt`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub market_id: Option<String>,
    #[serde(default)]
    pub token_id: Option<String>,
    /// Position direction: "BUY" or "SELL".
    #[serde(default)]
    pub side: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub avg_price: Option<String>,
    #[serde(default)]
    pub current_price: Option<String>,
    #[serde(default)]
    pub unrealized_pnl: Option<String>,
    #[serde(default)]
    pub realized_pnl: Option<String>,
    #[serde(default)]
    pub opened_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Order lifecycle status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    Pending,
    Submitted,
    Live,
    Matched,
    Delayed,
    Mined,
    Confirmed,
    Partial,
    Cancelled,
    Unmatched,
    Failed,
    Error,
}

/// An order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub id: String,
    #[serde(default)]
    pub market_id: Option<String>,
    #[serde(default)]
    pub token_id: Option<String>,
    #[serde(default)]
    pub side: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub price: Option<String>,
    #[serde(default)]
    pub fill_size: Option<String>,
    #[serde(default)]
    pub fill_price: Option<String>,
    #[serde(default)]
    pub status: Option<OrderStatus>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Parameters for listing orders.
#[derive(Debug, Default)]
pub struct ListOrdersParams {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub status: Option<OrderStatus>,
    pub strategy_id: Option<String>,
    pub market_id: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

/// Parameters for closing a position.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ClosePositionParams {
    #[serde(rename = "tokenId")]
    pub token_id: String,
    /// Size as a number string (e.g. `"100"`). Platform validates with `@IsNumberString()`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
}

/// Parameters for redeeming a resolved position.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct RedeemPositionParams {
    #[serde(rename = "positionId")]
    pub position_id: String,
    #[serde(rename = "marketId", skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
}

/// Parameters for splitting a position.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SplitPositionParams {
    #[serde(rename = "tokenId")]
    pub token_id: String,
    /// Amount as a decimal string (e.g. `"100.5"`).
    pub amount: String,
}

/// Parameters for merging positions.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MergePositionParams {
    #[serde(rename = "tokenId")]
    pub token_id: String,
    /// Amount as a decimal string (e.g. `"100.5"`).
    pub amount: String,
}

/// Parameters for placing a direct order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceOrderParams {
    #[serde(rename = "marketId")]
    pub market_id: String,
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

/// Response from redeeming a resolved position.
///
/// The platform `/api/v1/orders/redeem` returns `{ positionId, intentId, status }`,
/// which is distinct from `PlaceOrderResponse` (which carries `orderId`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedeemPositionResponse {
    #[serde(rename = "positionId")]
    pub position_id: String,
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
#[serde(rename_all = "camelCase")]
pub struct TraderScore {
    #[serde(default)]
    pub overall: Option<f64>,
    #[serde(default)]
    pub rank: Option<u64>,
    #[serde(default)]
    pub profitability: Option<f64>,
    #[serde(default)]
    pub consistency: Option<f64>,
    #[serde(rename = "riskManagement", default)]
    pub risk_management: Option<f64>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub percentile: Option<f64>,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Social & Signals
// ---------------------------------------------------------------------------

/// A whale trade from the social feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhaleTrade {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub market_id: Option<String>,
    #[serde(default)]
    pub market_name: Option<String>,
    #[serde(default)]
    pub side: Option<String>,
    #[serde(default)]
    pub size: Option<f64>,
    #[serde(default)]
    pub usd_value: Option<f64>,
    #[serde(default)]
    pub wallet: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A news-based trading signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    pub sentiment: Option<String>,
    #[serde(default)]
    pub related_markets: Vec<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Configuration: Alerts, Copy Trading, Webhooks
// ---------------------------------------------------------------------------

/// A price alert on a specific token.
///
/// Field names match the platform `PriceAlert` Prisma model:
/// `tokenId`, `direction`, `price`, `persistent`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    pub id: String,
    #[serde(default)]
    pub token_id: Option<String>,
    /// Alert direction: "above" or "below".
    #[serde(default)]
    pub direction: Option<String>,
    /// Price threshold as a decimal string.
    #[serde(default)]
    pub price: Option<String>,
    #[serde(default)]
    pub persistent: Option<bool>,
    #[serde(default)]
    pub triggered: Option<bool>,
    #[serde(default)]
    pub triggered_at: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A copy-trading configuration (strategy-based).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyConfig {
    pub id: String,
    /// The strategy being copied.
    #[serde(default)]
    pub source_strategy_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    /// Allocation as a percentage (0–100).
    #[serde(default)]
    pub allocation_percent: Option<u8>,
    #[serde(default)]
    pub initial_balance: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Webhook event types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WebhookEvent {
    #[serde(rename = "ORDER_FILLED")]
    OrderFilled,
    #[serde(rename = "STRATEGY_ERROR")]
    StrategyError,
    #[serde(rename = "WHALE_TRADE")]
    WhaleTrade,
    #[serde(rename = "NEWS_SIGNAL")]
    NewsSignal,
    #[serde(rename = "BACKTEST_COMPLETE")]
    BacktestComplete,
    #[serde(rename = "DAILY_LOSS_LIMIT")]
    DailyLossLimit,
    #[serde(rename = "MARKET_RESOLVED")]
    MarketResolved,
    #[serde(rename = "PRICE_ALERT")]
    PriceAlert,
}

/// A registered webhook.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Webhook {
    pub id: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub events: Vec<WebhookEvent>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing)]
    pub secret: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

impl std::fmt::Debug for Webhook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Webhook")
            .field("id", &self.id)
            .field("url", &self.url)
            .field("secret", &"[REDACTED]")
            .field("events", &self.events)
            .field("enabled", &self.enabled)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Watchlist
// ---------------------------------------------------------------------------

/// An item in the user's watchlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistItem {
    pub market_id: String,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub current_price: Option<String>,
    #[serde(rename = "volume24h", default)]
    pub volume_24h: Option<String>,
    #[serde(rename = "priceDelta24h", default)]
    pub price_delta_24h: Option<String>,
    #[serde(default)]
    pub watched: Option<bool>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Response from adding a market to the watchlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistAddResponse {
    pub market_id: String,
    #[serde(default)]
    pub added_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Watchlist status for a specific market.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistStatus {
    pub market_id: String,
    pub watched: bool,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Result from testing a webhook.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookTestResult {
    pub success: bool,
    pub status_code: u16,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// AI
// ---------------------------------------------------------------------------

/// Response from the AI query endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiQueryResponse {
    #[serde(default)]
    pub answer: Option<String>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub suggested_actions: Vec<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Arbitrage
// ---------------------------------------------------------------------------

/// A merge arbitrage opportunity (YES + NO prices < $1.00).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    #[serde(rename = "marketId")]
    pub market_id: String,
    #[serde(rename = "marketTitle")]
    pub market_title: String,
    #[serde(default)]
    pub category: String,
    #[serde(rename = "endDate", default)]
    pub end_date: Option<String>,
    #[serde(rename = "yesTokenId")]
    pub yes_token_id: String,
    #[serde(rename = "noTokenId")]
    pub no_token_id: String,
    #[serde(rename = "yesPrice")]
    pub yes_price: String,
    #[serde(rename = "noPrice")]
    pub no_price: String,
    pub sum: String,
    #[serde(rename = "marginPct")]
    pub margin_pct: String,
    #[serde(rename = "costPerUnit")]
    pub cost_per_unit: String,
    #[serde(rename = "profitPerUnit")]
    pub profit_per_unit: String,
}

// ---------------------------------------------------------------------------
// Smart Orders
// ---------------------------------------------------------------------------

/// Smart order execution algorithm.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SmartOrderType {
    TWAP,
    DCA,
    BRACKET,
    OCO,
}

/// Smart order lifecycle status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SmartOrderStatus {
    PENDING,
    ACTIVE,
    COMPLETED,
    CANCELLED,
    FAILED,
}

/// Parameters for placing a smart order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceSmartOrderParams {
    #[serde(rename = "type")]
    pub order_type: SmartOrderType,
    #[serde(rename = "tokenId")]
    pub token_id: String,
    pub side: String,
    pub outcome: String,
    #[serde(rename = "totalSize")]
    pub total_size: f64,
    // TWAP / DCA
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slices: Option<u32>,
    #[serde(rename = "intervalMinutes", skip_serializing_if = "Option::is_none")]
    pub interval_minutes: Option<u32>,
    #[serde(rename = "limitPrice", skip_serializing_if = "Option::is_none")]
    pub limit_price: Option<f64>,
    // BRACKET
    #[serde(rename = "entryPrice", skip_serializing_if = "Option::is_none")]
    pub entry_price: Option<f64>,
    #[serde(rename = "takeProfitPrice", skip_serializing_if = "Option::is_none")]
    pub take_profit_price: Option<f64>,
    #[serde(rename = "stopLossPrice", skip_serializing_if = "Option::is_none")]
    pub stop_loss_price: Option<f64>,
    // OCO
    #[serde(rename = "priceA", skip_serializing_if = "Option::is_none")]
    pub price_a: Option<f64>,
    #[serde(rename = "priceB", skip_serializing_if = "Option::is_none")]
    pub price_b: Option<f64>,
}

/// Response from placing a smart order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceSmartOrderResponse {
    #[serde(rename = "smartOrderId")]
    pub smart_order_id: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub status: String,
    #[serde(rename = "slicesTotal")]
    pub slices_total: u32,
}

/// A child order spawned by a smart order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartOrderChild {
    pub id: String,
    pub status: String,
    #[serde(rename = "fillSize", default)]
    pub fill_size: Option<String>,
    #[serde(rename = "fillPrice", default)]
    pub fill_price: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

/// A smart order with execution progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartOrder {
    pub id: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub status: String,
    #[serde(rename = "marketId")]
    pub market_id: String,
    #[serde(rename = "tokenId")]
    pub token_id: String,
    pub outcome: String,
    pub side: String,
    #[serde(rename = "totalSize")]
    pub total_size: String,
    #[serde(rename = "slicesFilled", default)]
    pub slices_filled: u32,
    #[serde(rename = "slicesTotal", default)]
    pub slices_total: u32,
    #[serde(rename = "nextExecuteAt", default)]
    pub next_execute_at: Option<String>,
    #[serde(rename = "completedAt", default)]
    pub completed_at: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(default)]
    pub orders: Vec<SmartOrderChild>,
}

// ---------------------------------------------------------------------------
// Marketplace
// ---------------------------------------------------------------------------

/// A strategy listing in the marketplace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceListing {
    pub id: String,
    #[serde(rename = "strategyId")]
    pub strategy_id: String,
    #[serde(rename = "sellerId")]
    pub seller_id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "priceUsdc")]
    pub price_usdc: String,
    pub status: String,
    #[serde(rename = "purchaseCount", default)]
    pub purchase_count: u64,
    #[serde(rename = "forkCount", default)]
    pub fork_count: u64,
    #[serde(rename = "avgRating", default)]
    pub avg_rating: Option<String>,
    #[serde(rename = "ratingCount", default)]
    pub rating_count: u64,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Response from purchasing a marketplace strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplacePurchaseResult {
    #[serde(rename = "purchaseId")]
    pub purchase_id: String,
    #[serde(rename = "forkedStrategyId")]
    pub forked_strategy_id: String,
    #[serde(rename = "priceUsdc")]
    pub price_usdc: f64,
    #[serde(rename = "platformFee")]
    pub platform_fee: f64,
    #[serde(rename = "sellerNet")]
    pub seller_net: f64,
}

/// Parameters for browsing the marketplace.
#[derive(Debug, Default)]
pub struct BrowseMarketplaceParams {
    pub sort: Option<String>,
    pub tag: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// ---------------------------------------------------------------------------
// Accuracy & Portfolio Review
// ---------------------------------------------------------------------------

/// A calibration bucket for the accuracy score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationBucket {
    #[serde(rename = "bucketMid")]
    pub bucket_mid: f64,
    pub frequency: f64,
    pub count: u32,
}

/// Prediction accuracy and calibration score for the authenticated user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccuracyScore {
    #[serde(rename = "brierScore")]
    pub brier_score: Option<f64>,
    #[serde(rename = "totalPredictions")]
    pub total_predictions: u32,
    #[serde(rename = "correctPredictions")]
    pub correct_predictions: u32,
    #[serde(rename = "winRate")]
    pub win_rate: String,
    pub calibration: Vec<CalibrationBucket>,
    #[serde(rename = "byCategory")]
    pub by_category: std::collections::HashMap<String, serde_json::Value>,
}

/// AI-generated portfolio review and optimization suggestions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioReview {
    pub review: String,
    pub suggestions: Vec<String>,
    pub score: u32,
    #[serde(rename = "generatedAt")]
    pub generated_at: String,
}

/// Aggregated news sentiment for a specific market.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSentiment {
    #[serde(rename = "marketId")]
    pub market_id: String,
    pub score: f64,
    pub direction: String,
    #[serde(rename = "signalCount")]
    pub signal_count: u32,
    #[serde(rename = "lastUpdated")]
    pub last_updated: Option<String>,
}

/// Parameters for providing liquidity on a market.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvideLiquidityParams {
    #[serde(rename = "marketId")]
    pub market_id: String,
    #[serde(rename = "tokenId")]
    pub token_id: String,
    #[serde(rename = "amountUsdc")]
    pub amount_usdc: f64,
    #[serde(rename = "targetSpread", skip_serializing_if = "Option::is_none")]
    pub target_spread: Option<f64>,
}

/// A liquidity position resulting from a provide_liquidity call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LpPosition {
    #[serde(rename = "buyOrderId")]
    pub buy_order_id: String,
    #[serde(rename = "sellOrderId")]
    pub sell_order_id: String,
    #[serde(rename = "tokenId")]
    pub token_id: String,
    #[serde(rename = "buyPrice")]
    pub buy_price: String,
    #[serde(rename = "sellPrice")]
    pub sell_price: String,
    pub size: String,
}

// ---------------------------------------------------------------------------
// Price History & Order Book
// ---------------------------------------------------------------------------

/// Query parameters for fetching price history.
#[derive(Debug, Default, Clone, Serialize)]
pub struct PriceHistoryParams {
    /// Candle resolution: `"1m"`, `"1h"`, or `"1d"` (default `"1h"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    /// ISO 8601 start datetime (e.g. `"2026-01-01T00:00:00Z"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// ISO 8601 end datetime (e.g. `"2026-01-31T23:59:59Z"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// Maximum number of entries (1–1000, default server-side).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

fn deserialize_string_as_f64<'de, D>(deserializer: D) -> std::result::Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse::<f64>().map_err(serde::de::Error::custom)
}

/// A single OHLCV candle returned by the price-history endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub time: String,
    #[serde(deserialize_with = "deserialize_string_as_f64")]
    pub open: f64,
    #[serde(deserialize_with = "deserialize_string_as_f64")]
    pub high: f64,
    #[serde(deserialize_with = "deserialize_string_as_f64")]
    pub low: f64,
    #[serde(deserialize_with = "deserialize_string_as_f64")]
    pub close: f64,
    #[serde(deserialize_with = "deserialize_string_as_f64")]
    pub volume: f64,
}

/// Envelope returned by `GET /api/v1/markets/{tokenId}/price-history`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceHistoryResponse {
    pub token_id: String,
    pub resolution: String,
    pub has_gaps: bool,
    pub data: Vec<Candle>,
}

#[deprecated(
    since = "1.8.0",
    note = "Use `Candle` instead — the platform returns OHLCV data"
)]
/// Legacy type — does not match the platform's OHLCV response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceHistoryEntry {
    pub timestamp: String,
    pub price: f64,
    #[serde(default)]
    pub volume: Option<f64>,
}

/// A single level in an order book (bid or ask).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderBookLevel {
    pub price: f64,
    pub size: f64,
}

/// An order book snapshot with bids and asks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderBook {
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
}

// ---------------------------------------------------------------------------
// Conditional Orders
// ---------------------------------------------------------------------------

/// Conditional order status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConditionalOrderStatus {
    Pending,
    Triggered,
    Cancelled,
    Expired,
    Failed,
}

/// Conditional order type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConditionalOrderType {
    TakeProfit,
    StopLoss,
    TrailingStop,
    Limit,
    Pegged,
}

/// A conditional order (limit, stop, trailing-stop, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionalOrder {
    pub id: String,
    #[serde(default)]
    pub token_id: Option<String>,
    #[serde(default)]
    pub side: Option<String>,
    #[serde(default)]
    pub outcome: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub trigger_price: Option<String>,
    #[serde(default)]
    pub limit_price: Option<String>,
    #[serde(default)]
    pub condition_type: Option<String>,
    #[serde(default)]
    pub status: Option<ConditionalOrderStatus>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub triggered_at: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Parameters for listing conditional orders.
#[derive(Debug, Default)]
pub struct ListConditionalOrdersParams {
    pub status: Option<ConditionalOrderStatus>,
    pub order_type: Option<ConditionalOrderType>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

/// Parameters for creating a conditional order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateConditionalOrderParams {
    pub market_id: String,
    pub token_id: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub side: String,
    pub outcome: String,
    pub size: f64,
    pub trigger_price: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trailing_pct: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Alert CRUD
// ---------------------------------------------------------------------------

/// Alert direction (price movement direction).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AlertDirection {
    Above,
    Below,
}

/// Parameters for creating a price alert.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAlertParams {
    pub token_id: String,
    pub direction: AlertDirection,
    pub price: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistent: Option<bool>,
}

// ---------------------------------------------------------------------------
// Portfolio PnL
// ---------------------------------------------------------------------------

/// Aggregated portfolio profit-and-loss data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioPnl {
    #[serde(default)]
    pub total_pnl: Option<String>,
    #[serde(default)]
    pub realized_pnl: Option<String>,
    #[serde(default)]
    pub unrealized_pnl: Option<String>,
    #[serde(default)]
    pub period: Option<String>,
    #[serde(default)]
    pub strategy_id: Option<String>,
    #[serde(default)]
    pub data_points: Vec<serde_json::Value>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Valid periods for portfolio PnL queries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PnlPeriod {
    #[serde(rename = "7d")]
    SevenDays,
    #[serde(rename = "30d")]
    ThirtyDays,
    #[serde(rename = "90d")]
    NinetyDays,
    #[serde(rename = "allTime")]
    AllTime,
}

/// Parameters for fetching portfolio PnL.
#[derive(Debug, Default)]
pub struct GetPortfolioPnlParams {
    /// Time period: `"7d"`, `"30d"`, `"90d"`, `"allTime"`.
    pub period: Option<PnlPeriod>,
    /// Filter by strategy ID.
    pub strategy_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Copy trading CRUD params
// ---------------------------------------------------------------------------

/// Parameters for creating a copy trading configuration (platform contract).
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCopyConfigParams {
    pub source_strategy_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_balance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allocation_percent: Option<u8>,
}

/// Parameters for updating a copy trading configuration.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCopyConfigParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allocation_percent: Option<u8>,
}

/// Parameters for fetching trades of a copy config.
#[derive(Debug, Default)]
pub struct GetCopyTradesParams {
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

// ---------------------------------------------------------------------------
// Whale extended types
// ---------------------------------------------------------------------------

/// Statistics for a whale wallet.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WhaleStats {
    #[serde(default)]
    pub total_volume: String,
    #[serde(default)]
    pub total_pnl: String,
    #[serde(default)]
    pub trade_count: u64,
    #[serde(default)]
    pub win_rate: String,
}

/// Full trading profile for a whale wallet.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WhaleProfile {
    pub wallet_address: String,
    #[serde(default)]
    pub stats: Option<WhaleStats>,
    #[serde(default)]
    pub recent_trades: Vec<serde_json::Value>,
    #[serde(default)]
    pub sparkline: Vec<u64>,
    #[serde(default)]
    pub is_following: bool,
}

/// Parameters for fetching the whale-trade feed.
#[derive(Debug, Default)]
pub struct GetWhaleFeedParams {
    /// Minimum trade size in USDC (e.g. `10000` for $10k+).
    pub min_size: Option<u64>,
    /// Filter by market ID.
    pub market_id: Option<String>,
    /// Filter by wallet address.
    pub wallet_address: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

/// Parameters for fetching AI-powered news signals.
#[derive(Debug, Default)]
pub struct GetNewsSignalsParams {
    /// Minimum confidence threshold (0–100).
    pub min_confidence: Option<u32>,
    /// Filter by market ID.
    pub market_id: Option<String>,
    /// Filter by direction: `"BUY"` or `"SELL"`.
    pub direction: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

/// Parameters for fetching top whale wallets.
#[derive(Debug, Default)]
pub struct GetTopWhalesParams {
    pub sort_by: Option<String>,
    pub period: Option<String>,
    pub limit: Option<u32>,
}

// ---------------------------------------------------------------------------
// Discover & Leaderboard params
// ---------------------------------------------------------------------------

/// Parameters for the strategy discover endpoint.
#[derive(Debug, Default)]
pub struct DiscoverParams {
    pub sort: Option<String>,
    pub category: Option<String>,
    pub search: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

/// Parameters for the leaderboard endpoint.
#[derive(Debug, Default)]
pub struct LeaderboardParams {
    pub period: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

// ---------------------------------------------------------------------------
// Paper trading
// ---------------------------------------------------------------------------

/// Summary of the paper trading account.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PaperSummary {
    #[serde(default)]
    pub balance: f64,
    #[serde(default)]
    pub pnl: f64,
    #[serde(default)]
    pub trade_count: u64,
    #[serde(default)]
    pub open_positions: u64,
}

// ---------------------------------------------------------------------------
// Batch API
// ---------------------------------------------------------------------------

/// A single request item in a batch call.
#[derive(Debug, Serialize)]
pub struct BatchRequestItem {
    pub id: String,
    pub method: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
}

/// Response from a batch API call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResponse {
    pub results: Vec<BatchResultItem>,
}

/// Individual result within a batch response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResultItem {
    pub id: String,
    pub status: u16,
    pub body: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Marketplace seller params
// ---------------------------------------------------------------------------

/// Parameters for creating a marketplace listing.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateListingParams {
    pub strategy_id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub price_usdc: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

/// Parameters for updating a marketplace listing.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateListingParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_usdc: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// Parameters for rating a marketplace listing.
#[derive(Debug, Serialize)]
pub struct RateListingParams {
    pub rating: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review: Option<String>,
}

// ---------------------------------------------------------------------------
// Risk Settings
// ---------------------------------------------------------------------------

/// Current risk / circuit-breaker settings for the authenticated user.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RiskSettings {
    #[serde(default)]
    pub daily_loss_limit: String,
    #[serde(default)]
    pub max_position_size: String,
    #[serde(default)]
    pub max_bets_per_day: u32,
    #[serde(default)]
    pub circuit_breaker_triggered: bool,
}

/// Parameters for updating risk settings. Only supplied fields are changed.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRiskSettingsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daily_loss_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_position_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_bets_per_day: Option<u32>,
}

// ---------------------------------------------------------------------------
// Markets — extended data (search, CLOB, tick-size, spread, midpoint)
// ---------------------------------------------------------------------------

/// Parameters for the full-text market search endpoint.
#[derive(Debug, Default)]
pub struct SearchMarketsParams {
    /// Full-text search query.
    pub q: String,
    pub limit: Option<u32>,
}

/// Tick-size for a market token (minimum price increment).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TickSizeResponse {
    #[serde(rename = "tokenId", default)]
    pub token_id: Option<String>,
    #[serde(default)]
    pub tick_size: Option<f64>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Bid-ask spread for a market token.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpreadResponse {
    #[serde(rename = "tokenId", default)]
    pub token_id: Option<String>,
    #[serde(default)]
    pub spread: Option<f64>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Midpoint price for a market token.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MidpointResponse {
    #[serde(rename = "tokenId", default)]
    pub token_id: Option<String>,
    #[serde(default)]
    pub mid: Option<f64>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A single price level in a CLOB order book.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClobLevel {
    /// Price as a decimal string (platform canonical format).
    #[serde(default)]
    pub price: Option<String>,
    /// Size at this level as a decimal string.
    #[serde(default)]
    pub size: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Full CLOB order book for a market token.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClobBook {
    #[serde(default)]
    pub bids: Vec<ClobLevel>,
    #[serde(default)]
    pub asks: Vec<ClobLevel>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A single CLOB price-history data point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClobPricePoint {
    /// Unix millisecond timestamp.
    #[serde(default)]
    pub t: Option<u64>,
    /// Price at this point.
    #[serde(default)]
    pub p: Option<f64>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Parameters for the CLOB prices-history endpoint.
#[derive(Debug, Default)]
pub struct ClobPricesHistoryParams {
    /// Candle interval, e.g. `"1m"`, `"5m"`, `"1h"`.
    pub interval: Option<String>,
    /// Number of data points.
    pub fidelity: Option<u32>,
}

// ---------------------------------------------------------------------------
// Orders — bulk operations
// ---------------------------------------------------------------------------

/// Request body for the batch-order endpoint (up to 15 orders).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatchOrdersParams {
    pub orders: Vec<PlaceOrderParams>,
}

/// Result for a single order within a batch response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchOrderResult {
    pub status: String,
    #[serde(rename = "orderId", default)]
    pub order_id: Option<String>,
    #[serde(rename = "intentId", default)]
    pub intent_id: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Response from the batch-order endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatchOrdersResponse {
    #[serde(default)]
    pub results: Vec<BatchOrderResult>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Request body for the bulk-cancel endpoint (up to 3 000 order IDs).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BulkCancelParams {
    pub order_ids: Vec<String>,
}

/// Response from the bulk-cancel endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BulkCancelResponse {
    #[serde(default)]
    pub cancelled: Vec<String>,
    #[serde(default)]
    pub failed: Vec<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// News — articles
// ---------------------------------------------------------------------------

/// A raw news article.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsArticle {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub sentiment: Option<String>,
    #[serde(rename = "publishedAt", default)]
    pub published_at: Option<String>,
    #[serde(rename = "relatedMarkets", default)]
    pub related_markets: Vec<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Parameters for listing news articles.
#[derive(Debug, Default)]
pub struct ListNewsParams {
    pub source: Option<String>,
    pub sentiment: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

// ---------------------------------------------------------------------------
// Scores — badges and extended
// ---------------------------------------------------------------------------

/// A single entry on the global scores leaderboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreEntry {
    #[serde(rename = "userId", default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub rank: Option<u64>,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// An achievement badge awarded to a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Badge {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "awardedAt", default)]
    pub awarded_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Portfolio — Polymarket-native
// ---------------------------------------------------------------------------

/// Native Polymarket portfolio snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketPortfolio {
    #[serde(default)]
    pub positions: Vec<serde_json::Value>,
    #[serde(default)]
    pub total_value: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Native Polymarket earnings summary.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketEarnings {
    #[serde(default)]
    pub total: Option<String>,
    #[serde(default)]
    pub realized: Option<String>,
    #[serde(default)]
    pub unrealized: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A single Polymarket portfolio activity event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketActivityItem {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "type", default)]
    pub activity_type: Option<String>,
    #[serde(default)]
    pub amount: Option<String>,
    #[serde(rename = "createdAt", default)]
    pub created_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Query parameters for the Polymarket activity endpoint.
#[derive(Debug, Default)]
pub struct GetPolymarketActivityParams {
    /// Activity type filter, e.g. `"trade"`, `"deposit"`, `"withdrawal"`.
    pub activity_type: Option<String>,
}

// ---------------------------------------------------------------------------
// Rewards
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewardMarket {
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewardMarketDetail {
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserRewards {
    #[serde(default)]
    pub rewards: Vec<serde_json::Value>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserRewardsTotal {
    #[serde(default)]
    pub total: Option<String>,
    #[serde(rename = "byDate", default)]
    pub by_date: Vec<serde_json::Value>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserRewardsPercentages {
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserRewardsPerMarket {
    #[serde(default)]
    pub markets: Vec<serde_json::Value>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rebates {
    #[serde(default)]
    pub rebates: Vec<serde_json::Value>,
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

// ---------------------------------------------------------------------------
// Cross-Venue Arbitrage (POLA-782)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossVenueArbitrageOpportunity {
    pub id: String,
    #[serde(default)]
    pub market_title: Option<String>,
    #[serde(default)]
    pub venue_a: Option<String>,
    #[serde(default)]
    pub venue_b: Option<String>,
    #[serde(default)]
    pub price_a: Option<String>,
    #[serde(default)]
    pub price_b: Option<String>,
    #[serde(default)]
    pub spread: Option<String>,
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossVenueComparison {
    pub match_id: String,
    #[serde(default)]
    pub polymarket_price: Option<String>,
    #[serde(default)]
    pub kalshi_price: Option<String>,
    #[serde(default)]
    pub spread: Option<String>,
    #[serde(default)]
    pub arbitrage_pct: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArbitrageMatch {
    pub id: String,
    #[serde(default)]
    pub polymarket_market_id: Option<String>,
    #[serde(default)]
    pub kalshi_market_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub verified: Option<bool>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateArbitrageMatchParams {
    pub polymarket_market_id: String,
    pub kalshi_market_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

// ---------------------------------------------------------------------------
// Whale Leaderboard & Alert Filter (POLA-782)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WhaleLeaderboardEntry {
    #[serde(default)]
    pub rank: Option<u64>,
    #[serde(default)]
    pub wallet_address: Option<String>,
    #[serde(default)]
    pub total_volume: Option<String>,
    #[serde(default)]
    pub total_pnl: Option<String>,
    #[serde(default)]
    pub win_rate: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WhaleAlertFilter {
    #[serde(default)]
    pub min_size_usd: Option<u64>,
    #[serde(default)]
    pub market_ids: Vec<String>,
    #[serde(default)]
    pub wallet_addresses: Vec<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWhaleAlertFilterParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_size_usd: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_addresses: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Profile (POLA-782)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordParams {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub follower_count: Option<u64>,
    #[serde(default)]
    pub following_count: Option<u64>,
    #[serde(default)]
    pub is_following: Option<bool>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Settings (POLA-782)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsProfileParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSettings {
    #[serde(default)]
    pub email_enabled: Option<bool>,
    #[serde(default)]
    pub push_enabled: Option<bool>,
    #[serde(default)]
    pub order_fills: Option<bool>,
    #[serde(default)]
    pub strategy_errors: Option<bool>,
    #[serde(default)]
    pub whale_alerts: Option<bool>,
    #[serde(default)]
    pub market_resolutions: Option<bool>,
    #[serde(default)]
    pub daily_summary: Option<bool>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNotificationSettingsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_fills: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy_errors: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whale_alerts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_resolutions: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daily_summary: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePasswordParams {
    pub current_password: String,
    pub new_password: String,
}

// ---------------------------------------------------------------------------
// Support Tickets (POLA-782)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TicketCategory {
    #[serde(rename = "GENERAL")]
    General,
    #[serde(rename = "BILLING")]
    Billing,
    #[serde(rename = "TECHNICAL")]
    Technical,
    #[serde(rename = "ACCOUNT")]
    Account,
    #[serde(rename = "BUG")]
    Bug,
    #[serde(rename = "FEATURE_REQUEST")]
    FeatureRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TicketPriority {
    #[serde(rename = "LOW")]
    Low,
    #[serde(rename = "MEDIUM")]
    Medium,
    #[serde(rename = "HIGH")]
    High,
    #[serde(rename = "URGENT")]
    Urgent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTicketParams {
    pub subject: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<TicketCategory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<TicketPriority>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ticket {
    pub id: String,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub messages: Vec<serde_json::Value>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketMessage {
    pub id: String,
    #[serde(default)]
    pub ticket_id: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub is_staff: Option<bool>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Notification Preferences (POLA-782)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPreferences {
    #[serde(default)]
    pub order_filled: Option<bool>,
    #[serde(default)]
    pub strategy_error: Option<bool>,
    #[serde(default)]
    pub whale_alert: Option<bool>,
    #[serde(default)]
    pub market_resolved: Option<bool>,
    #[serde(default)]
    pub price_alert: Option<bool>,
    #[serde(default)]
    pub daily_summary: Option<bool>,
    #[serde(default)]
    pub marketing: Option<bool>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNotificationPreferencesParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_filled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy_error: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whale_alert: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_resolved: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_alert: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daily_summary: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marketing: Option<bool>,
}
