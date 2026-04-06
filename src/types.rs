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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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

/// Parameters for providing liquidity on a market token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvideLiquidityParams {
    #[serde(rename = "tokenId")]
    pub token_id: String,
    pub spread: f64,
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
