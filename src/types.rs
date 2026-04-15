use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Paginated wrapper
// ---------------------------------------------------------------------------

/// A paginated API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub page: u64,
    #[serde(default)]
    pub limit: u64,
    #[serde(rename = "totalPages", default)]
    pub total_pages: u64,
    #[serde(rename = "hasNext", default)]
    pub has_next: bool,
}

// ---------------------------------------------------------------------------
// Markets
// ---------------------------------------------------------------------------

/// A prediction market.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Market {
    pub id: String,
    pub name: String,
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
    #[serde(rename = "baseToken", default)]
    pub base_token: Option<serde_json::Value>,
    #[serde(rename = "quoteToken", default)]
    pub quote_token: Option<serde_json::Value>,
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

/// Trading mode used when starting a strategy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TradingMode {
    #[serde(rename = "live")]
    Live,
    #[serde(rename = "paper")]
    Paper,
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

/// Copy trading mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CopyMode {
    Percentage,
    Fixed,
    Mirror,
}

/// A copy-trading configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyConfig {
    pub id: String,
    /// The Ethereum wallet address to copy trades to.
    #[serde(default)]
    pub target_wallet: Option<String>,
    /// Copy mode: PERCENTAGE, FIXED, or MIRROR.
    #[serde(default)]
    pub mode: Option<CopyMode>,
    /// Size value used with the selected mode.
    #[serde(default)]
    pub size_value: Option<String>,
    /// Maximum exposure as a number string.
    #[serde(default)]
    pub max_exposure: Option<String>,
    /// Maximum daily loss as a number string.
    #[serde(default)]
    pub max_daily_loss: Option<String>,
    /// Price offset as a number string.
    #[serde(default)]
    pub price_offset: Option<String>,
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
    pub size: f64,
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
#[serde(rename_all = "camelCase")]
pub struct PriceHistoryParams {
    /// Candle period: `"1h"`, `"6h"`, or `"24h"` (default `"1h"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,
    /// Maximum number of entries (1–500, default server-side).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// A single price history entry (candle).
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
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
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
