use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Paginated wrapper
// ---------------------------------------------------------------------------

/// A paginated API response with flat pagination fields matching the platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub limit: u64,
    pub total_pages: u64,
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
}

// ---------------------------------------------------------------------------
// GDPR Personal Data Export (POLA-3846)
// ---------------------------------------------------------------------------

/// Metadata for a personal data export payload.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PersonalDataExportMeta {
    #[serde(default)]
    pub max_records_per_collection: Option<u64>,
    #[serde(default)]
    pub collections_truncated: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// GDPR personal data export response from `GET /api/v1/me/export`.
///
/// Contains all user data across the platform grouped into sections.
/// Webhook URLs are redacted to hostname only.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PersonalDataExport {
    #[serde(default)]
    pub generated_at: Option<String>,
    #[serde(default)]
    pub format_version: Option<String>,
    #[serde(rename = "_meta", default)]
    pub meta: Option<PersonalDataExportMeta>,
    #[serde(default)]
    pub account: serde_json::Value,
    #[serde(default)]
    pub settings: serde_json::Value,
    #[serde(default)]
    pub security: serde_json::Value,
    #[serde(default)]
    pub trading: serde_json::Value,
    #[serde(default)]
    pub communications: serde_json::Value,
    #[serde(default)]
    pub social: serde_json::Value,
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

/// Parameters for [`crate::PolyforgeClient::start_strategy`].
///
/// Platform contract (POST `/api/v1/strategies/{id}/start`):
/// `{ "mode": "live"|"paper" }`.
#[derive(Debug, Clone, Serialize)]
pub struct StartStrategyParams {
    pub mode: TradingMode,
}

impl StartStrategyParams {
    pub fn paper() -> Self {
        Self {
            mode: TradingMode::Paper,
        }
    }

    pub fn live() -> Self {
        Self {
            mode: TradingMode::Live,
        }
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

/// Parameters for closing a prediction-market position.
///
/// `size` controls the close mode:
/// - `None` (default) — **sweep**: close the entire position at market price
///   by placing a market sell order for the full held quantity.
/// - `Some("100")` — **partial close**: sell only that number of shares,
///   leaving the remainder of the position open.
///
/// # Sweep semantics
///
/// GTC orders are priced at `0.001` SELL / `0.999` BUY and behave as a
/// **market-equivalent sweep, not a resting limit order**.  Slippage is
/// bounded only by venue depth at call time, not by the on-paper price.
/// The fill price is whatever the order book offers at the time of
/// execution.
///
/// For cross-venue arbitrage positions use the dedicated
/// `POST /api/v1/arbitrage/positions/:id/close` endpoint
/// (see [`crate::PolyforgeClient::close_arbitrage_position`]); arbitrage closes
/// are always full sweeps — partial closes are not supported for arb positions.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ClosePositionParams {
    #[serde(rename = "tokenId")]
    pub token_id: String,
    /// Size to close as a number string (e.g. `"100"`).  Omit to sweep the
    /// entire position.  Platform validates with `@IsNumberString()`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
}

// The platform RedeemPositionDto permits either marketId or positionId
// — the caller can supply whichever is available, leaving the other `None`.
/// Parameters for redeeming a resolved position.
///
/// At least one of `position_id` or `market_id` must be provided.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct RedeemPositionParams {
    #[serde(rename = "positionId", skip_serializing_if = "Option::is_none")]
    pub position_id: Option<String>,
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

/// Copy mode matching the platform's `CopyModeDto`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CopyMode {
    Percentage,
    Fixed,
    Mirror,
}

/// A copy-trading configuration (wallet-based platform model).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyConfig {
    pub id: String,
    #[serde(default)]
    pub target_wallet: Option<String>,
    #[serde(default)]
    pub mode: Option<CopyMode>,
    #[serde(default)]
    pub size_value: Option<String>,
    #[serde(default)]
    pub max_exposure: Option<String>,
    #[serde(default)]
    pub max_daily_loss: Option<String>,
    #[serde(default)]
    pub price_offset: Option<String>,
    /// Platform status: ACTIVE | PAUSED | STOPPED | ERROR
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub stopped_at: Option<String>,
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
    pub price: String,
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

/// Parameters for creating a copy trading configuration (wallet-based platform contract).
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCopyConfigParams {
    pub target_wallet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<CopyMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_exposure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_daily_loss: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_offset: Option<String>,
}

/// Parameters for updating a copy trading configuration (wallet-based platform contract).
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCopyConfigParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<CopyMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_exposure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_daily_loss: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_offset: Option<String>,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskSettings {
    #[serde(default)]
    pub drawdown_enabled: bool,
    #[serde(default = "default_lookback_hours")]
    pub drawdown_lookback_hours: u32,
    #[serde(default = "default_threshold_pct")]
    pub drawdown_threshold_pct: f64,
    #[serde(default)]
    pub circuit_breaker_tripped: bool,
    #[serde(default)]
    pub circuit_breaker_tripped_at: Option<String>,
}

impl Default for RiskSettings {
    fn default() -> Self {
        Self {
            drawdown_enabled: false,
            drawdown_lookback_hours: default_lookback_hours(),
            drawdown_threshold_pct: default_threshold_pct(),
            circuit_breaker_tripped: false,
            circuit_breaker_tripped_at: None,
        }
    }
}

fn default_lookback_hours() -> u32 {
    24
}
fn default_threshold_pct() -> f64 {
    0.1
}

/// Parameters for updating risk settings. Only supplied fields are changed.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRiskSettingsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drawdown_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drawdown_lookback_hours: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drawdown_threshold_pct: Option<f64>,
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

/// A single native Polymarket portfolio entry.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketPortfolioEntry {
    pub asset: String,
    pub size: String,
    pub avg_price: String,
    pub realized_pnl: String,
    pub unrealized_pnl: String,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Native Polymarket portfolio response.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketPortfolio {
    #[serde(default)]
    pub entries: Vec<PolymarketPortfolioEntry>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A single native Polymarket earnings entry.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketEarningsEntry {
    pub date: String,
    pub earnings: String,
    pub volume: String,
    pub win_rate: String,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Native Polymarket earnings response.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketEarnings {
    #[serde(default)]
    pub entries: Vec<PolymarketEarningsEntry>,
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
    #[serde(default)]
    pub asset: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(rename = "createdAt", default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Native Polymarket activity response.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketActivityResponse {
    #[serde(default)]
    pub activities: Vec<PolymarketActivityItem>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Query parameters for the Polymarket activity endpoint.
#[derive(Debug, Default)]
pub struct GetPolymarketActivityParams {
    /// Activity type filter, e.g. `"TRADE"`, `"SPLIT"`, or `"REDEEM"`.
    pub activity_type: Option<String>,
}

// ---------------------------------------------------------------------------
// Rewards
// ---------------------------------------------------------------------------

/// A market that distributes liquidity rewards.
#[derive(Debug, Clone)]
pub struct RewardMarket {
    pub condition_id: Option<String>,
    pub rewards_daily: Option<String>,
    pub rewards_max_spread: Option<String>,
    pub rewards_min_size: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub extra: serde_json::Value,
}

const REWARD_MARKET_PROMOTED: &[&str] = &[
    "conditionId",
    "rewardsDaily",
    "rewardsMaxSpread",
    "rewardsMinSize",
    "startDate",
    "endDate",
];

impl Serialize for RewardMarket {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        if let Some(ref v) = self.condition_id {
            map.serialize_entry("conditionId", v)?;
        }
        if let Some(ref v) = self.rewards_daily {
            map.serialize_entry("rewardsDaily", v)?;
        }
        if let Some(ref v) = self.rewards_max_spread {
            map.serialize_entry("rewardsMaxSpread", v)?;
        }
        if let Some(ref v) = self.rewards_min_size {
            map.serialize_entry("rewardsMinSize", v)?;
        }
        if let Some(ref v) = self.start_date {
            map.serialize_entry("startDate", v)?;
        }
        if let Some(ref v) = self.end_date {
            map.serialize_entry("endDate", v)?;
        }
        if let Some(obj) = self.extra.as_object() {
            for (k, v) in obj {
                if !REWARD_MARKET_PROMOTED.contains(&k.as_str()) {
                    map.serialize_entry(k, v)?;
                }
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for RewardMarket {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw: serde_json::Value = serde_json::Value::deserialize(deserializer)?;

        if !raw.is_object() {
            return Err(serde::de::Error::custom(
                "expected a JSON object for RewardMarket",
            ));
        }

        for key in REWARD_MARKET_PROMOTED {
            if let Some(v) = raw.get(key) {
                if !v.is_string() && !v.is_null() {
                    return Err(serde::de::Error::custom(format!(
                        "expected string for field `{key}` in RewardMarket"
                    )));
                }
            }
        }

        let get_str = |key: &str| -> Option<String> {
            raw.get(key).and_then(|v| v.as_str()).map(String::from)
        };

        Ok(RewardMarket {
            condition_id: get_str("conditionId"),
            rewards_daily: get_str("rewardsDaily"),
            rewards_max_spread: get_str("rewardsMaxSpread"),
            rewards_min_size: get_str("rewardsMinSize"),
            start_date: get_str("startDate"),
            end_date: get_str("endDate"),
            extra: raw,
        })
    }
}

#[derive(Debug, Clone)]
pub struct RewardMarketDetail {
    pub condition_id: Option<String>,
    pub rewards_daily: Option<String>,
    pub rewards_max_spread: Option<String>,
    pub rewards_min_size: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub extra: serde_json::Value,
}

impl Serialize for RewardMarketDetail {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        if let Some(ref v) = self.condition_id {
            map.serialize_entry("conditionId", v)?;
        }
        if let Some(ref v) = self.rewards_daily {
            map.serialize_entry("rewardsDaily", v)?;
        }
        if let Some(ref v) = self.rewards_max_spread {
            map.serialize_entry("rewardsMaxSpread", v)?;
        }
        if let Some(ref v) = self.rewards_min_size {
            map.serialize_entry("rewardsMinSize", v)?;
        }
        if let Some(ref v) = self.start_date {
            map.serialize_entry("startDate", v)?;
        }
        if let Some(ref v) = self.end_date {
            map.serialize_entry("endDate", v)?;
        }
        if let Some(obj) = self.extra.as_object() {
            for (k, v) in obj {
                if !REWARD_MARKET_PROMOTED.contains(&k.as_str()) {
                    map.serialize_entry(k, v)?;
                }
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for RewardMarketDetail {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw: serde_json::Value = serde_json::Value::deserialize(deserializer)?;

        if !raw.is_object() {
            return Err(serde::de::Error::custom(
                "expected a JSON object for RewardMarketDetail",
            ));
        }

        for key in REWARD_MARKET_PROMOTED {
            if let Some(v) = raw.get(key) {
                if !v.is_string() && !v.is_null() {
                    return Err(serde::de::Error::custom(format!(
                        "expected string for field `{key}` in RewardMarketDetail"
                    )));
                }
            }
        }

        let get_str = |key: &str| -> Option<String> {
            raw.get(key).and_then(|v| v.as_str()).map(String::from)
        };

        Ok(RewardMarketDetail {
            condition_id: get_str("conditionId"),
            rewards_daily: get_str("rewardsDaily"),
            rewards_max_spread: get_str("rewardsMaxSpread"),
            rewards_min_size: get_str("rewardsMinSize"),
            start_date: get_str("startDate"),
            end_date: get_str("endDate"),
            extra: raw,
        })
    }
}

/// Detailed reward information for a market by platform market ID.
///
/// Distinct from [`RewardMarketDetail`]; returned by `GET /api/v1/rewards/market/{marketId}`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RewardsMarketDetail {
    #[serde(default)]
    pub condition_id: Option<String>,
    #[serde(default)]
    pub rate_per_day: Option<String>,
    #[serde(default)]
    pub total_rewards: Option<String>,
    #[serde(default)]
    pub remaining_reward_amount: Option<String>,
    #[serde(default)]
    pub max_spread: Option<String>,
    #[serde(default)]
    pub min_size: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// The authenticated user's sponsored rewards markets.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserSponsoredMarkets {
    #[serde(default)]
    pub markets: Vec<serde_json::Value>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Polymarket sponsor page URL for a specific market.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RewardsSponsorUrl {
    pub url: String,
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
/// `STRATEGY_PAUSED`, `STRATEGY_RESUMED`, `STRATEGY_ERROR`,
/// `ORDER_PLACED`, `ORDER_SUBMITTED`, `ORDER_PARTIAL`, `ORDER_FILLED`,
/// `ORDER_FAILED`, `ORDER_CANCELLED`, `ORDER_ERROR`,
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

/// Known strategy event types emitted by the platform.
///
/// The platform may emit additional event types not listed here;
/// clients should handle unknown types gracefully.
pub const KNOWN_STRATEGY_EVENT_TYPES: &[&str] = &[
    "CONNECTED",
    "STRATEGY_STARTED",
    "STRATEGY_STOPPED",
    "STRATEGY_PAUSED",
    "STRATEGY_RESUMED",
    "STRATEGY_ERROR",
    "ORDER_PLACED",
    "ORDER_SUBMITTED",
    "ORDER_PARTIAL",
    "ORDER_FILLED",
    "ORDER_FAILED",
    "ORDER_CANCELLED",
    "ORDER_ERROR",
    "BACKTEST_PROGRESS",
    "BACKTEST_COMPLETED",
    "BACKTEST_FAILED",
];

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

/// Bid/ask info for a single venue in a spread comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VenuePriceInfo {
    #[serde(default)]
    pub market_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub yes_bid: Option<f64>,
    #[serde(default)]
    pub no_ask: Option<f64>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Bid/ask spread comparison between Polymarket and Kalshi for a matched pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpreadSummary {
    #[serde(default)]
    pub match_id: Option<String>,
    #[serde(default)]
    pub polymarket: Option<VenuePriceInfo>,
    #[serde(default)]
    pub kalshi: Option<VenuePriceInfo>,
    #[serde(default)]
    pub yes_spread_pct: Option<f64>,
    #[serde(default)]
    pub no_spread_pct: Option<f64>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub verified: Option<bool>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Result of a manual matching sync pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchSyncResult {
    #[serde(default)]
    pub matched: Option<u64>,
    #[serde(default)]
    pub created: Option<u64>,
    #[serde(default)]
    pub updated: Option<u64>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A user subscription for cross-venue arbitrage notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArbitrageAlertSubscription {
    pub id: String,
    #[serde(default)]
    pub min_spread_pct: Option<f64>,
    #[serde(default)]
    pub market_id: Option<String>,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub triggered_at: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Parameters for creating an arbitrage alert subscription.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateArbitrageAlertParams {
    pub min_spread_pct: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Cross-Venue Arb Execution / Positions / Risk (POLA-1852)
// ---------------------------------------------------------------------------
//
// These types describe the trading-impact-bearing arbitrage execution surface:
//   - `POST /api/v1/arbitrage/execute`            — place real offsetting orders
//   - `GET  /api/v1/arbitrage/positions[/:id]`    — position lifecycle
//   - `POST /api/v1/arbitrage/positions/:id/close`
//   - `GET  /api/v1/arbitrage/risk/dashboard`     — net exposure + P&L
//   - `GET  /api/v1/arbitrage/risk/settlement`    — resolution-criteria mismatches
//   - `POST /api/v1/arbitrage/risk/refresh-pnl`   — recompute unrealized P&L
//
// Decimal columns (sizes, prices, P&L) generally arrive as JSON strings from
// Prisma; we keep them as `Option<String>` so callers can convert with full
// precision rather than relying on f64 coercion. Mirrors the Python SDK shapes.

/// Parameters for `POST /api/v1/arbitrage/execute`.
///
/// `match_id` must be a valid UUID (RFC 4122).  The backend validates this
/// server-side and **returns HTTP 400** for non-UUID input.  The SDK also
/// performs a client-side validation before any real-money order hits the
/// wire.
///
/// `size` must be an integer USDC amount in the `1..=10000` range, and
/// `max_slippage_pct`, if set, must be in `0..=5`.  These mirror the
/// server-side `class-validator` bounds in `ExecuteArbDto`.  Use
/// [`crate::PolyforgeClient::execute_arbitrage`] which validates before
/// any real-money order hits the wire.
///
/// The backend requires an `Idempotency-Key` header (8–128 characters) on
/// every request; the SDK sends the caller-supplied `idempotency_key`
/// argument as that header.  See
/// [`crate::PolyforgeClient::execute_arbitrage`] for rate-limit and error
/// code details.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteArbitrageParams {
    pub match_id: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_slippage_pct: Option<f64>,
}

/// Supported prediction-market venue.
///
/// Serializes as `"POLYMARKET"`, `"KALSHI"`, or `"POLYMARKET_US"` over the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Venue {
    Polymarket,
    Kalshi,
    #[serde(rename = "POLYMARKET_US")]
    PolymarketUs,
    #[serde(other)]
    Unknown,
}

impl Venue {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Polymarket => "POLYMARKET",
            Self::Kalshi => "KALSHI",
            Self::PolymarketUs => "POLYMARKET_US",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// Lifecycle status for a cross-venue arbitrage position.
///
/// Flow: `PENDING` → `OPEN` → `CLOSING` → `CLOSED` (normal path)
/// or `PENDING` → `OPEN` → `CLOSING` → `FAILED` (close failure).
/// `PARTIAL` may appear transiently when only one leg has filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ArbPositionStatus {
    Pending,
    Partial,
    Open,
    Closing,
    Closed,
    Failed,
}

impl ArbPositionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Partial => "PARTIAL",
            Self::Open => "OPEN",
            Self::Closing => "CLOSING",
            Self::Closed => "CLOSED",
            Self::Failed => "FAILED",
        }
    }
}

fn deserialize_optional_string_or_number<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<serde_json::Value>::deserialize(deserializer)? {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(value)) => Ok(Some(value)),
        Some(serde_json::Value::Number(value)) => Ok(Some(value.to_string())),
        Some(value) => Err(serde::de::Error::custom(format!(
            "expected string, number, or null, got {value}"
        ))),
    }
}

/// A single leg of an arbitrage execution result.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArbExecutionLeg {
    #[serde(default)]
    pub venue: Option<Venue>,
    #[serde(default)]
    pub intent_id: Option<String>,
    #[serde(default)]
    pub token_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string_or_number")]
    pub price: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Server response for `POST /api/v1/arbitrage/execute`.
///
/// On success this opens a new [`ArbPosition`] in `OPEN` state.  The position
/// consists of two offsetting legs (buy on one venue, sell on the other) and
/// can later be exited only via a **full sweep-close** using
/// `POST /api/v1/arbitrage/positions/:id/close`
/// (see [`crate::PolyforgeClient::close_arbitrage_position`]).  Partial closes
/// are not supported for arbitrage positions.
///
/// Both legs carry optional `intent_id` and `price` fields; fill confirmation
/// may arrive asynchronously and is available on the full [`ArbPosition`]
/// record after the orders execute.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArbExecutionResult {
    #[serde(default)]
    pub arb_position_id: Option<String>,
    #[serde(default)]
    pub buy_leg: Option<ArbExecutionLeg>,
    #[serde(default)]
    pub sell_leg: Option<ArbExecutionLeg>,
    #[serde(default)]
    pub entry_spread_pct: Option<f64>,
    #[serde(default)]
    pub status: Option<ArbPositionStatus>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A cross-venue arbitrage position (mirrors the Prisma `ArbPosition` row).
///
/// Created via `POST /api/v1/arbitrage/execute` and closed via a **full
/// sweep-close** (`POST /api/v1/arbitrage/positions/:id/close`).  The
/// lifecycle is tracked in [`status`](Self::status): positions start in
/// `OPEN`, transition through `CLOSING` during the sweep, and settle in
/// `CLOSED` or `FAILED`.
///
/// Decimal columns (`buyPrice`, `buySize`, P&L fields, etc.) arrive as JSON
/// strings — kept as `Option<String>` for lossless precision.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArbPosition {
    pub id: String,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub match_id: Option<String>,
    #[serde(default)]
    pub status: Option<ArbPositionStatus>,

    #[serde(default)]
    pub buy_venue: Option<Venue>,
    #[serde(default)]
    pub buy_order_id: Option<String>,
    #[serde(default)]
    pub buy_token_id: Option<String>,
    #[serde(default)]
    pub buy_price: Option<String>,
    #[serde(default)]
    pub buy_size: Option<String>,
    #[serde(default)]
    pub buy_fill_price: Option<String>,
    #[serde(default)]
    pub buy_fill_size: Option<String>,

    #[serde(default)]
    pub sell_venue: Option<Venue>,
    #[serde(default)]
    pub sell_order_id: Option<String>,
    #[serde(default)]
    pub sell_token_id: Option<String>,
    #[serde(default)]
    pub sell_price: Option<String>,
    #[serde(default)]
    pub sell_size: Option<String>,
    #[serde(default)]
    pub sell_fill_price: Option<String>,
    #[serde(default)]
    pub sell_fill_size: Option<String>,

    #[serde(default)]
    pub entry_spread_pct: Option<String>,
    #[serde(default)]
    pub current_spread_pct: Option<String>,
    #[serde(default)]
    pub realized_pnl: Option<String>,
    #[serde(default)]
    pub unrealized_pnl: Option<String>,

    #[serde(default)]
    pub opened_at: Option<String>,
    #[serde(default)]
    pub closed_at: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,

    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Paginated response for `GET /api/v1/arbitrage/positions`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArbPositionsResponse {
    #[serde(default)]
    pub positions: Vec<ArbPosition>,
    #[serde(default)]
    pub total: u64,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Server response for `POST /api/v1/arbitrage/positions/:id/close`.
///
/// Returned after a **full sweep-close** of a cross-venue arbitrage position.
/// Both legs (buy and sell) are reversed with market orders placed on the
/// respective venues.  The close is always a complete sweep — no partial
/// close is supported for arbitrage positions.
///
/// `status` reflects the terminal outcome of the sweep:
/// - `CLOSED` — both reversing market orders were successfully placed.
/// - `FAILED` — one or both reverse orders could not be placed (e.g.
///   insufficient liquidity, venue connectivity issue).
///
/// Once closed, call [`crate::PolyforgeClient::get_arbitrage_position`] to
/// fetch the full position record including fill prices and realised P&L.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArbCloseResponse {
    #[serde(default)]
    pub status: Option<ArbPositionStatus>,
    #[serde(default)]
    pub position_id: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Net deployed capital broken out by venue (component of [`ArbRiskDashboard`]).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArbNetExposure {
    #[serde(default)]
    pub polymarket: f64,
    #[serde(default)]
    pub kalshi: f64,
    #[serde(default)]
    pub polymarket_us: f64,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Server response for `GET /api/v1/arbitrage/risk/dashboard`.
///
/// The backend pre-aggregates these to numeric summaries, hence `f64` rather
/// than the lossless string form used on `ArbPosition`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArbRiskDashboard {
    #[serde(default)]
    pub open_positions: u64,
    #[serde(default)]
    pub pending_positions: u64,
    #[serde(default)]
    pub total_deployed: f64,
    #[serde(default)]
    pub net_exposure: ArbNetExposure,
    #[serde(default)]
    pub total_realized_pnl: f64,
    #[serde(default)]
    pub total_unrealized_pnl: f64,
    #[serde(default)]
    pub avg_spread_pct: f64,
    #[serde(default)]
    pub positions_by_status: std::collections::HashMap<ArbPositionStatus, u64>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// An entry from `GET /api/v1/arbitrage/risk/settlement`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArbSettlementRisk {
    #[serde(default)]
    pub match_id: Option<String>,
    #[serde(default)]
    pub polymarket_title: Option<String>,
    #[serde(default)]
    pub kalshi_title: Option<String>,
    #[serde(default)]
    pub polymarket_end_date: Option<String>,
    #[serde(default)]
    pub kalshi_end_date: Option<String>,
    #[serde(default)]
    pub end_date_diff_days: Option<f64>,
    #[serde(default)]
    pub confidence: Option<f64>,
    /// `"LOW"` | `"MEDIUM"` | `"HIGH"`
    #[serde(default)]
    pub risk_level: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Server response for `POST /api/v1/arbitrage/risk/refresh-pnl`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArbPnlRefreshResult {
    #[serde(default)]
    pub updated: u64,
    #[serde(flatten)]
    pub extra: serde_json::Value,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitter_handle: Option<String>,
}

/// Notification settings as returned by `GET /api/v1/settings/notifications`.
///
/// Field names mirror the platform's `UpdateNotificationsDto` plus the
/// underlying `NotificationPreference` Prisma row (camelCase on the wire).
/// Unknown fields returned by the server (`userId`, `updatedAt`,
/// `eventPrefs`, `emailDigest`, `notificationFreq`, `minFillNotifyUsdc`,
/// `onTicketReply`, ...) are preserved in `extra` so that callers using a
/// newer platform release do not lose data on round-trip.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telegram_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discord_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_order_filled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_strategy_error: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_backtest_complete: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_daily_loss_limit: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_market_resolved: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_someone_forked: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_someone_followed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_someone_liked: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_someone_commented: Option<bool>,
    /// Forward-compat bucket for any additional fields the platform
    /// returns (e.g. `userId`, `updatedAt`, `eventPrefs`, `emailDigest`,
    /// `notificationFreq`, `minFillNotifyUsdc`, `onTicketReply`).
    #[serde(default, flatten)]
    pub extra: serde_json::Value,
}

/// Parameters for `PATCH /api/v1/settings/notifications`.
///
/// Mirrors the platform's `UpdateNotificationsDto` exactly. The platform
/// runs `ValidationPipe` with `whitelist: true, forbidNonWhitelisted: true`,
/// so this struct intentionally only contains fields that the DTO accepts
/// — sending anything else returns 400. Use `None` for any field you do
/// not want to touch; `serde(skip_serializing_if = "Option::is_none")`
/// keeps the request body to a true partial update.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNotificationSettingsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telegram_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discord_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_order_filled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_strategy_error: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_backtest_complete: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_daily_loss_limit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_market_resolved: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_someone_forked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_someone_followed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_someone_liked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_someone_commented: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePasswordParams {
    pub current_password: String,
    pub new_password: String,
}

// ---------------------------------------------------------------------------
// System Health (POLA-3327)
// ---------------------------------------------------------------------------

/// Public health payload returned by `GET /health`.
///
/// Contains only the public status; no operational internals are exposed.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SystemHealthPublic {
    pub status: String,
    #[serde(default)]
    pub service: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub uptime: Option<u64>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Authenticated health payload returned by `GET /api/v1/status`.
///
/// Extends the public health shape with operational metrics (DB, Redis, queue).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SystemHealthAuthenticated {
    pub status: String,
    #[serde(default)]
    pub service: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub uptime: Option<u64>,
    #[serde(default)]
    pub db: Option<serde_json::Value>,
    #[serde(default)]
    pub redis: Option<serde_json::Value>,
    #[serde(default)]
    pub queue_depth: Option<u64>,
    #[serde(default)]
    pub services: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
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

// ---------------------------------------------------------------------------
// Venue Preferences (POLA-3330)
// ---------------------------------------------------------------------------

/// The authenticated user's venue/platform preferences.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferences {
    #[serde(default)]
    pub default_venue: Option<String>,
    #[serde(default)]
    pub enabled_venues: Option<Vec<String>>,
    #[serde(default)]
    pub single_platform_mode: Option<bool>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Parameters for updating venue/platform preferences. Only supplied fields are changed.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserPreferencesParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_venue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled_venues: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub single_platform_mode: Option<bool>,
}

// ---------------------------------------------------------------------------
// Sports markets (POLA-1841)
// ---------------------------------------------------------------------------
//
// Many sports endpoints return weakly-typed payloads at the controller
// (`Record<string, unknown>` / `unknown[]`). The SDK mirrors that fidelity
// by surfacing those payloads as `serde_json::Value` rather than inventing
// strict types that could drift from the server.

/// A sports category aggregate returned by `GET /api/v1/sports/categories`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SportsCategory {
    pub category: String,
    pub label: String,
    #[serde(default)]
    pub series_tickers: Vec<String>,
    #[serde(default)]
    pub market_count: u64,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Parameters for `GET /api/v1/sports/markets`.
#[derive(Debug, Default, Clone)]
pub struct ListSportsMarketsParams {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub category: Option<String>,
    pub search: Option<String>,
    pub series_ticker: Option<String>,
    pub event_ticker: Option<String>,
    pub live_only: Option<bool>,
    /// Sort order: `"volume"`, `"closing_soon"`, `"newest"`. Defaults to `"volume"` server-side.
    pub sort: Option<String>,
}

/// Parameters for `GET /api/v1/sports/events`.
#[derive(Debug, Default, Clone)]
pub struct ListSportsEventsParams {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub category: Option<String>,
    pub series_ticker: Option<String>,
    /// Status filter: `"SCHEDULED"`, `"PREGAME"`, `"LIVE"`, `"HALFTIME"`, `"FINAL"`.
    pub status: Option<String>,
}

/// Parameters for `GET /api/v1/sports/milestones`.
#[derive(Debug, Default, Clone)]
pub struct ListSportsMilestonesParams {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub event_ticker: Option<String>,
    pub status: Option<String>,
}

/// Parameters for `GET /api/v1/sports/combos`.
#[derive(Debug, Default, Clone)]
pub struct ListSportsCombosParams {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub series_ticker: Option<String>,
}

/// One leg of a `POST /api/v1/sports/combos/lookup` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SportsComboSelection {
    pub market_ticker: String,
    pub event_ticker: String,
    /// Either `"yes"` or `"no"`.
    pub side: String,
}

/// Body for `POST /api/v1/sports/combos/lookup`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SportsComboLookupParams {
    pub collection_ticker: String,
    pub selected_markets: Vec<SportsComboSelection>,
}

// ---------------------------------------------------------------------------
// Public user profile lookups (POLA-1844)
// ---------------------------------------------------------------------------

/// One day on a public user's PnL curve. ``date`` is ``YYYY-MM-DD``.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPerformancePoint {
    pub date: String,
    pub pnl: f64,
    pub cum_pnl: f64,
}

/// Public summary of one of a user's strategies.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserStrategySummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub win_rate: f64,
    pub trade_count: u64,
    pub price_usdc: f64,
    pub fork_count: u64,
    pub like_count: u64,
    pub is_liked: bool,
}

/// Resolved-position activity entry shown on a user's profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserActivityEntry {
    pub id: String,
    pub market_question: String,
    pub outcome: String,
    pub side: String,
    pub size: f64,
    pub pnl: f64,
    pub resolved_at: String,
}

/// Badge entry returned by the public-profile endpoint (id is the badge type).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfileBadge {
    pub id: String,
    pub unlocked_at: String,
}

/// Pared-down user record returned by ``me/following``.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowedUser {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

/// Envelope returned by `/users/:username/{performance,strategies,activity,badges}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDataEnvelope<T> {
    pub data: Vec<T>,
}

// ---------------------------------------------------------------------------
// Misc public utility endpoints (POLA-1858)
// ---------------------------------------------------------------------------

/// A row in the trading journal (order annotated with mood + optional note).
///
/// `GET /api/v1/journal` returns a flat paginated list of orders that have been
/// annotated with a journal mood/note.  The response merges order fields
/// (`marketId`, `side`, `outcome`, `price`, `size`, `status`) with the journal
/// annotation (`mood`, `note`).  Unknown fields are preserved in `extra`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalEntry {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub order_id: Option<String>,
    #[serde(default)]
    pub market_id: Option<String>,
    /// One of `CONFIDENT | UNCERTAIN | FOMO | DISCIPLINED | REVENGE`.
    #[serde(default)]
    pub mood: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub side: Option<String>,
    #[serde(default)]
    pub outcome: Option<String>,
    #[serde(default)]
    pub price: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Optional filters for `list_journal`.
#[derive(Debug, Default, Clone)]
pub struct ListJournalParams {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    /// One of `ORDER_MOODS`: `CONFIDENT | UNCERTAIN | FOMO | DISCIPLINED | REVENGE`.
    pub mood: Option<String>,
}

/// A user-facing notification record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub read: Option<bool>,
    #[serde(default)]
    pub read_at: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Plain page/limit pagination params (matches platform `PaginationDto`).
#[derive(Debug, Default, Clone)]
pub struct PaginationParams {
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

/// Stats block on the referrals payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferralStats {
    #[serde(default)]
    pub invited: u32,
    #[serde(default)]
    pub signed_up: u32,
    #[serde(default)]
    pub active: u32,
    #[serde(default)]
    pub credits_earned: u32,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Response of `GET /api/v1/referrals/me`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MyReferralsResponse {
    #[serde(default)]
    pub referral_code: Option<String>,
    #[serde(default)]
    pub referral_link: Option<String>,
    #[serde(default)]
    pub stats: Option<ReferralStats>,
    #[serde(default)]
    pub referrals: Vec<serde_json::Value>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[deprecated(note = "use MyReferralsResponse instead")]
pub use self::MyReferralsResponse as ReferralsInfo;

/// Side of an order preview request — `BUY` or `SELL`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PreviewSide {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
}

/// Body of `POST /api/v1/fees/preview`.
///
/// Mirrors the server's `OrderPreviewDto` class-validator bounds: `size >= 1`,
/// `0.001 <= price <= 0.999`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderPreviewParams {
    pub token_id: String,
    pub side: PreviewSide,
    pub size: f64,
    pub price: f64,
    /// Set to `"POST_ONLY"` to preview a maker-fee estimate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_type: Option<String>,
}

/// Per-venue fee estimate inside an `OrderPreviewResponse`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VenueFeeEstimate {
    #[serde(default)]
    pub venue: Option<String>,
    #[serde(default)]
    pub fee_bps: Option<f64>,
    #[serde(default)]
    pub fee_usd: Option<f64>,
    #[serde(default)]
    pub total_cost_usd: Option<f64>,
    #[serde(default)]
    pub is_maker: Option<bool>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Match metadata when a Polymarket order can be cross-quoted on Kalshi.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketMatchRef {
    #[serde(default)]
    pub match_id: Option<String>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Response of `POST /api/v1/fees/preview` — cross-venue fee comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderPreviewResponse {
    #[serde(default)]
    pub polymarket: Option<VenueFeeEstimate>,
    #[serde(default)]
    pub kalshi: Option<VenueFeeEstimate>,
    #[serde(default)]
    pub savings: Option<f64>,
    #[serde(default)]
    pub recommended_venue: Option<String>,
    #[serde(default)]
    pub market_match: Option<MarketMatchRef>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// One row in `/api/v1/fees/schedules` for Polymarket (no price band).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketFeeSchedule {
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub fee_bps: Option<f64>,
    #[serde(default)]
    pub effective_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// One row in `/api/v1/fees/schedules` for Kalshi (with price band).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KalshiFeeSchedule {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub fee_bps: Option<f64>,
    #[serde(default)]
    pub min_price: Option<f64>,
    #[serde(default)]
    pub max_price: Option<f64>,
    #[serde(default)]
    pub effective_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Response of `GET /api/v1/fees/schedules`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FeeSchedules {
    #[serde(default)]
    pub polymarket: Vec<PolymarketFeeSchedule>,
    #[serde(default)]
    pub kalshi: Vec<KalshiFeeSchedule>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Outcome side for a per-market alert (matches server `IsIn` set:
/// `YES | NO | Yes | No`). Serialized in the canonical uppercase form.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MarketAlertOutcome {
    #[serde(rename = "YES")]
    Yes,
    #[serde(rename = "NO")]
    No,
}

/// Direction comparator for a per-market alert.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MarketAlertCondition {
    Above,
    Below,
}

/// A per-market price alert (response shape).
///
/// Distinct from the top-level token-based [`Alert`]; per-market alerts are
/// scoped to a `marketId` + outcome (`YES`/`NO`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketAlert {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub market_id: Option<String>,
    #[serde(default)]
    pub outcome: Option<String>,
    #[serde(default)]
    pub condition: Option<String>,
    #[serde(default)]
    pub threshold: Option<f64>,
    #[serde(default)]
    pub triggered: Option<bool>,
    #[serde(default)]
    pub triggered_at: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Response of `GET /api/v1/markets/:marketId/alerts`.
///
/// The platform wraps per-market alerts in a `{ "data": [...] }` envelope.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MarketAlertsResponse {
    #[serde(default)]
    pub data: Vec<MarketAlert>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Body of `POST /api/v1/markets/:marketId/alerts`.
///
/// `threshold` is validated server-side to `0.01 <= threshold <= 0.99`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMarketAlertParams {
    pub outcome: MarketAlertOutcome,
    pub condition: MarketAlertCondition,
    pub threshold: f64,
}

/// Lookback period for `get_market_history` (matches the server's
/// `MarketHistoryQueryDto`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MarketHistoryPeriod {
    #[serde(rename = "1d")]
    OneDay,
    #[serde(rename = "7d")]
    SevenDays,
    #[serde(rename = "30d")]
    ThirtyDays,
    #[serde(rename = "90d")]
    NinetyDays,
}

impl MarketHistoryPeriod {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            MarketHistoryPeriod::OneDay => "1d",
            MarketHistoryPeriod::SevenDays => "7d",
            MarketHistoryPeriod::ThirtyDays => "30d",
            MarketHistoryPeriod::NinetyDays => "90d",
        }
    }
}

/// The authenticated user's sentiment vote inside a market sentiment report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketSentimentVote {
    pub direction: String,
    pub confidence: f64,
}

/// Aggregated, market-controller-derived sentiment report.
///
/// Distinct from [`MarketSentiment`], which is news-derived sentiment from
/// `/news/sentiment/:marketId`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MarketSentimentReport {
    #[serde(default)]
    pub yes_percent: u32,
    #[serde(default)]
    pub no_percent: u32,
    #[serde(default)]
    pub total_votes: u32,
    #[serde(default)]
    pub user_vote: Option<MarketSentimentVote>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Mood tag attached to a placed order via `update_order_journal`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OrderJournalMood {
    #[serde(rename = "CONFIDENT")]
    Confident,
    #[serde(rename = "UNCERTAIN")]
    Uncertain,
    #[serde(rename = "FOMO")]
    Fomo,
    #[serde(rename = "DISCIPLINED")]
    Disciplined,
    #[serde(rename = "REVENGE")]
    Revenge,
}

/// Body of `PATCH /api/v1/orders/:id/journal`.
///
/// `note` is capped at 2000 characters server-side.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOrderJournalParams {
    pub mood: OrderJournalMood,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A Kalshi combo collection (group of related combo markets).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComboCollection {
    #[serde(default)]
    pub ticker: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub series_ticker: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Optional filters/cursor for `list_combo_collections`.
#[derive(Debug, Default, Clone)]
pub struct ListComboCollectionsParams {
    pub series_ticker: Option<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

/// One leg of a combo lookup — a market ticker plus its outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComboLeg {
    pub ticker: String,
    /// `"yes"` or `"no"` — must be lowercase to match the server enum.
    pub outcome: String,
}

/// Body of `POST /api/v1/markets/combo/lookup`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComboLookupParams {
    pub collection_ticker: String,
    pub legs: Vec<ComboLeg>,
}

// ---------------------------------------------------------------------------
// Actions Catalog (POLA-3329)
// ---------------------------------------------------------------------------

/// A parameter descriptor within an action definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(rename = "in", default)]
    pub param_in: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "enum", default)]
    pub enum_values: Option<Vec<String>>,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A single action from the platform's API actions catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionDefinition {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub method: String,
    pub path: String,
    pub scope: String,
    pub category: String,
    #[serde(default)]
    pub parameters: Option<Vec<ActionParameter>>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// The platform's public API actions catalog.
///
/// Returned by `GET /api/v1/actions` — a capability manifest for agent/tooling discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionsSchema {
    pub version: String,
    pub actions: Vec<ActionDefinition>,
}

// ---------------------------------------------------------------------------

/// Aggregated correlation matrix between top market categories.
///
/// `matrix[i][j]` is the correlation between `categories[i]` and
/// `categories[j]` (square, symmetric, with `1.0` on the diagonal).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CategoryCorrelation {
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub matrix: Vec<Vec<f64>>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}
