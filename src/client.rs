use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::json;
use urlencoding::encode;

use crate::errors::{PolyforgeError, Result};
use crate::types::*;

// ---------------------------------------------------------------------------
// StrategyEventStream — lazy SSE reader returned by watch_strategy()
// ---------------------------------------------------------------------------

/// An open SSE connection to a strategy's execution event stream.
///
/// Call [`StrategyEventStream::next`] in a loop to receive events one at a time.
/// Drop the struct to close the underlying HTTP connection.
///
/// # Example
/// ```no_run
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let client = polyforge::PolyforgeClient::new("key");
/// let mut stream = client.watch_strategy("strat-uuid").await?;
/// while let Some(event) = stream.next().await {
///     let event = event?;
///     println!("{}: {:?}", event.event_type, event.data);
///     if event.event_type == "STRATEGY_STOPPED" { break; }
/// }
/// # Ok(())
/// # }
/// ```
pub struct StrategyEventStream {
    response: reqwest::Response,
    buffer: String,
}

impl StrategyEventStream {
    /// Receive the next event from the stream.
    ///
    /// Returns `None` when the server closes the connection.
    pub async fn next(&mut self) -> Option<Result<StrategyEvent>> {
        loop {
            // Parse any complete SSE lines already in the buffer
            if let Some(pos) = self.buffer.find('\n') {
                let line = self.buffer[..pos].to_string();
                self.buffer = self.buffer[pos + 1..].to_string();

                if let Some(raw) = line.strip_prefix("data: ") {
                    let raw = raw.trim();
                    if !raw.is_empty() {
                        return Some(
                            serde_json::from_str::<StrategyEvent>(raw)
                                .map_err(PolyforgeError::from),
                        );
                    }
                }
                // Skip comment / heartbeat lines and loop
                continue;
            }

            // Need more bytes from the network
            match self.response.chunk().await {
                Ok(Some(chunk)) => {
                    self.buffer.push_str(&String::from_utf8_lossy(&chunk));
                }
                Ok(None) => return None, // Server closed the stream
                Err(e) => return Some(Err(PolyforgeError::from(e))),
            }
        }
    }
}

const DEFAULT_BASE_URL: &str = "https://localhost:3002";

/// Async client for the Polyforge trading platform REST API.
pub struct PolyforgeClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl std::fmt::Debug for PolyforgeClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolyforgeClient")
            .field("base_url", &self.base_url)
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

impl PolyforgeClient {
    /// Create a new client pointing at the default local URL (`https://localhost:3002`).
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_url(api_key, DEFAULT_BASE_URL)
    }

    /// Create a new client with a custom base URL.
    pub fn with_url(api_key: impl Into<String>, api_url: impl Into<String>) -> Self {
        let api_key = api_key.into();
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .expect("failed to build reqwest client");

        Self {
            http,
            base_url: api_url.into().trim_end_matches('/').to_string(),
            api_key,
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn auth_header(&self) -> HeaderValue {
        HeaderValue::from_str(&format!("Bearer {}", self.api_key))
            .expect("invalid api key characters")
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .http
            .get(self.url(path))
            .header(AUTHORIZATION, self.auth_header())
            .send()
            .await?;
        self.handle_response(resp).await
    }

    async fn post<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let resp = self
            .http
            .post(self.url(path))
            .header(AUTHORIZATION, self.auth_header())
            .json(body)
            .send()
            .await?;
        self.handle_response(resp).await
    }

    async fn patch<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let resp = self
            .http
            .patch(self.url(path))
            .header(AUTHORIZATION, self.auth_header())
            .json(body)
            .send()
            .await?;
        self.handle_response(resp).await
    }

    async fn delete<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .http
            .delete(self.url(path))
            .header(AUTHORIZATION, self.auth_header())
            .send()
            .await?;
        self.handle_response(resp).await
    }

    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<T> {
        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            return Err(PolyforgeError::Api {
                status,
                code: body
                    .get("code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UNKNOWN")
                    .to_string(),
                message: body
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error")
                    .to_string(),
                request_id: body
                    .get("request_id")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            });
        }
        let body = resp.text().await?;
        serde_json::from_str(&body).map_err(PolyforgeError::from)
    }

    // -----------------------------------------------------------------------
    // Markets
    // -----------------------------------------------------------------------

    /// List markets with optional filtering and pagination.
    pub async fn list_markets(
        &self,
        params: &ListMarketsParams,
    ) -> Result<PaginatedResponse<Market>> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(ref s) = params.search {
            qp.push(("search", s.clone()));
        }
        if let Some(ref c) = params.category {
            qp.push(("category", c.clone()));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(p) = params.page {
            qp.push(("page", p.to_string()));
        }

        let qs = if qp.is_empty() {
            String::new()
        } else {
            let pairs: Vec<String> = qp.iter().map(|(k, v)| format!("{}={}", k, encode(v))).collect();
            format!("?{}", pairs.join("&"))
        };

        self.get(&format!("/api/v1/markets{qs}")).await
    }

    /// Get a single market by ID.
    pub async fn get_market(&self, id: &str) -> Result<Market> {
        self.get(&format!("/api/v1/markets/{}", encode(id))).await
    }

    // -----------------------------------------------------------------------
    // Strategies
    // -----------------------------------------------------------------------

    /// List strategies, optionally filtered by status.
    pub async fn list_strategies(
        &self,
        status: Option<StrategyStatus>,
    ) -> Result<Vec<Strategy>> {
        let qs = match status {
            Some(s) => {
                let val = serde_json::to_value(&s).unwrap_or_default();
                format!("?status={}", encode(val.as_str().unwrap_or_default()))
            }
            None => String::new(),
        };
        self.get(&format!("/api/v1/strategies{qs}")).await
    }

    /// Get a single strategy by ID.
    pub async fn get_strategy(&self, id: &str) -> Result<Strategy> {
        self.get(&format!("/api/v1/strategies/{}", encode(id))).await
    }

    /// Create a new strategy with a name and optional description.
    pub async fn create_strategy(
        &self,
        name: &str,
        description: Option<&str>,
        market_id: Option<&str>,
    ) -> Result<Strategy> {
        let mut body = json!({ "name": name });
        if let Some(desc) = description {
            body["description"] = json!(desc);
        }
        if let Some(mid) = market_id {
            body["marketId"] = json!(mid);
        }
        self.post("/api/v1/strategies", &body).await
    }

    /// Create a strategy from a natural-language description (AI-powered).
    pub async fn create_strategy_from_description(
        &self,
        description: &str,
        market_id: Option<&str>,
    ) -> Result<Strategy> {
        let mut body = json!({ "description": description });
        if let Some(mid) = market_id {
            body["market_id"] = json!(mid);
        }
        self.post("/api/v1/strategies/from-description", &body).await
    }

    /// Start a strategy in the given trading mode.
    pub async fn start_strategy(&self, id: &str, mode: TradingMode) -> Result<Strategy> {
        let body = json!({ "mode": mode });
        self.post(&format!("/api/v1/strategies/{}/start", encode(id)), &body)
            .await
    }

    /// Stop a running strategy.
    pub async fn stop_strategy(&self, id: &str) -> Result<Strategy> {
        self.post(&format!("/api/v1/strategies/{}/stop", encode(id)), &json!({}))
            .await
    }

    /// Get available strategy templates.
    pub async fn get_strategy_templates(&self) -> Result<Vec<StrategyTemplate>> {
        self.get("/api/v1/strategies/templates").await
    }

    /// Export a strategy configuration as JSON.
    pub async fn export_strategy(&self, id: &str) -> Result<serde_json::Value> {
        self.get(&format!("/api/v1/strategies/{}/export", encode(id))).await
    }

    /// Update a strategy's name and/or description.
    pub async fn update_strategy(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<&str>,
        market_id: Option<&str>,
    ) -> Result<Strategy> {
        let mut body = serde_json::json!({});
        if let Some(n) = name {
            body["name"] = serde_json::json!(n);
        }
        if let Some(d) = description {
            body["description"] = serde_json::json!(d);
        }
        if let Some(mid) = market_id {
            body["marketId"] = serde_json::json!(mid);
        }
        self.patch(&format!("/api/v1/strategies/{}", encode(id)), &body).await
    }

    /// Delete a strategy by ID.
    pub async fn delete_strategy(&self, id: &str) -> Result<serde_json::Value> {
        self.delete(&format!("/api/v1/strategies/{}", encode(id))).await
    }

    /// Import a strategy from a .polyforge JSON export.
    pub async fn import_strategy(&self, data: &serde_json::Value) -> Result<Strategy> {
        let body = serde_json::json!({ "data": data });
        self.post("/api/v1/strategies/import", &body).await
    }

    /// Pause a running strategy.
    pub async fn pause_strategy(&self, id: &str) -> Result<Strategy> {
        self.post(&format!("/api/v1/strategies/{}/pause", encode(id)), &serde_json::json!({})).await
    }

    /// Resume a paused strategy.
    pub async fn resume_strategy(&self, id: &str) -> Result<Strategy> {
        self.post(&format!("/api/v1/strategies/{}/resume", encode(id)), &serde_json::json!({})).await
    }

    /// Fork a strategy to create a new editable copy.
    pub async fn fork_strategy(&self, id: &str) -> Result<Strategy> {
        self.post(&format!("/api/v1/strategies/{}/fork", encode(id)), &serde_json::json!({})).await
    }

    // -----------------------------------------------------------------------
    // Portfolio & Orders
    // -----------------------------------------------------------------------

    /// Get the current portfolio.
    pub async fn get_portfolio(&self) -> Result<Portfolio> {
        self.get("/api/v1/portfolio").await
    }

    /// Get orders with optional filtering.
    pub async fn get_orders(&self, params: &ListOrdersParams) -> Result<Vec<Order>> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(ref s) = params.status {
            qp.push(("status", s.clone()));
        }
        if let Some(ref s) = params.strategy_id {
            qp.push(("strategyId", s.clone()));
        }
        if let Some(ref f) = params.from {
            qp.push(("from", f.clone()));
        }
        if let Some(ref t) = params.to {
            qp.push(("to", t.clone()));
        }

        let qs = if qp.is_empty() {
            String::new()
        } else {
            let pairs: Vec<String> = qp.iter().map(|(k, v)| format!("{k}={v}")).collect();
            format!("?{}", pairs.join("&"))
        };

        self.get(&format!("/api/v1/orders{qs}")).await
    }

    /// Get the trader score / reputation.
    pub async fn get_score(&self) -> Result<TraderScore> {
        self.get("/api/v1/scores/me").await
    }

    // -----------------------------------------------------------------------
    // Direct Trading
    // -----------------------------------------------------------------------

    /// Place a direct buy or sell order on a prediction market.
    pub async fn place_order(&self, params: &PlaceOrderParams) -> Result<PlaceOrderResponse> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/orders/place", &body).await
    }

    /// Cancel a pending or live order.
    pub async fn cancel_order(&self, order_id: &str) -> Result<CancelOrderResponse> {
        self.delete(&format!("/api/v1/orders/{}", encode(order_id))).await
    }

    /// Close an open position (sell all shares at market price).
    pub async fn close_position(&self, params: &ClosePositionParams) -> Result<PlaceOrderResponse> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/orders/close-position", &body).await
    }

    /// Redeem winning shares after a market resolves.
    pub async fn redeem_position(&self, params: &RedeemPositionParams) -> Result<PlaceOrderResponse> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/orders/redeem", &body).await
    }

    /// Split a position into smaller positions.
    pub async fn split_position(&self, params: &SplitPositionParams) -> Result<PlaceOrderResponse> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/orders/split", &body).await
    }

    /// Merge multiple positions into one.
    pub async fn merge_positions(&self, params: &MergePositionParams) -> Result<PlaceOrderResponse> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/orders/merge", &body).await
    }

    // -----------------------------------------------------------------------
    // Arbitrage
    // -----------------------------------------------------------------------

    /// Scan all active markets for merge arbitrage opportunities (YES + NO < $1.00).
    pub async fn get_arbitrage_opportunities(
        &self,
        min_margin: Option<f64>,
    ) -> Result<Vec<ArbitrageOpportunity>> {
        let qs = match min_margin {
            Some(m) => format!("?minMargin={m}"),
            None => String::new(),
        };
        self.get(&format!("/api/v1/arbitrage{qs}")).await
    }

    // -----------------------------------------------------------------------
    // Smart Orders
    // -----------------------------------------------------------------------

    /// Place an advanced smart order (TWAP, DCA, BRACKET, or OCO).
    pub async fn place_smart_order(
        &self,
        params: &PlaceSmartOrderParams,
    ) -> Result<PlaceSmartOrderResponse> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/orders/smart", &body).await
    }

    /// List your smart orders with child order progress.
    pub async fn list_smart_orders(&self) -> Result<Vec<SmartOrder>> {
        self.get("/api/v1/orders/smart").await
    }

    /// Cancel a pending or active smart order and its child orders.
    pub async fn cancel_smart_order(&self, id: &str) -> Result<serde_json::Value> {
        self.delete(&format!("/api/v1/orders/smart/{}", encode(id))).await
    }

    // -----------------------------------------------------------------------
    // Marketplace
    // -----------------------------------------------------------------------

    /// Browse marketplace listings with optional sort and tag filter.
    pub async fn browse_marketplace(
        &self,
        params: &BrowseMarketplaceParams,
    ) -> Result<serde_json::Value> {
        let mut qp: Vec<String> = Vec::new();
        if let Some(ref sort) = params.sort {
            qp.push(format!("sort={}", encode(sort)));
        }
        if let Some(ref tag) = params.tag {
            qp.push(format!("tag={}", encode(tag)));
        }
        if let Some(limit) = params.limit {
            qp.push(format!("limit={limit}"));
        }
        let qs = if qp.is_empty() {
            String::new()
        } else {
            format!("?{}", qp.join("&"))
        };
        self.get(&format!("/api/v1/marketplace{qs}")).await
    }

    /// Get a single marketplace listing by ID.
    pub async fn get_marketplace_listing(&self, id: &str) -> Result<MarketplaceListing> {
        self.get(&format!("/api/v1/marketplace/{}", encode(id))).await
    }

    /// Purchase a marketplace strategy. Receive a private fork in your account.
    pub async fn purchase_strategy(&self, listing_id: &str) -> Result<MarketplacePurchaseResult> {
        self.post(
            &format!("/api/v1/marketplace/{}/purchase", encode(listing_id)),
            &json!({}),
        )
        .await
    }

    // -----------------------------------------------------------------------
    // Social & Signals
    // -----------------------------------------------------------------------

    /// Get the whale trade feed.
    pub async fn get_whale_feed(&self, min_size: Option<u64>) -> Result<Vec<WhaleTrade>> {
        let qs = match min_size {
            Some(s) => format!("?min_size={}", encode(&s.to_string())),
            None => String::new(),
        };
        self.get(&format!("/api/v1/whales/feed{qs}")).await
    }

    /// Get AI-powered news signals.
    pub async fn get_news_signals(
        &self,
        min_confidence: Option<u32>,
    ) -> Result<Vec<NewsSignal>> {
        let qs = match min_confidence {
            Some(c) => format!("?min_confidence={}", encode(&c.to_string())),
            None => String::new(),
        };
        self.get(&format!("/api/v1/news/signals{qs}")).await
    }

    // -----------------------------------------------------------------------
    // Configuration
    // -----------------------------------------------------------------------

    /// List configured alerts.
    pub async fn list_alerts(&self) -> Result<Vec<Alert>> {
        self.get("/api/v1/alerts").await
    }

    /// List copy-trading configurations.
    pub async fn list_copy_configs(&self) -> Result<Vec<CopyConfig>> {
        self.get("/api/v1/copy").await
    }

    /// List registered webhooks.
    pub async fn list_webhooks(&self) -> Result<Vec<Webhook>> {
        self.get("/api/v1/webhooks").await
    }

    /// Register a new webhook for the given events.
    pub async fn create_webhook(
        &self,
        url: &str,
        events: &[WebhookEvent],
    ) -> Result<Webhook> {
        let body = json!({
            "url": url,
            "events": events,
        });
        self.post("/api/v1/webhooks", &body).await
    }

    // -----------------------------------------------------------------------
    // AI
    // -----------------------------------------------------------------------

    /// Send a natural-language query to the AI assistant.
    pub async fn ai_query(&self, query: &str) -> Result<AiQueryResponse> {
        let body = json!({ "query": query });
        self.post("/api/v1/ai/query", &body).await
    }

    // -----------------------------------------------------------------------
    // Accuracy & Portfolio Review
    // -----------------------------------------------------------------------

    /// Get prediction accuracy and calibration score for the authenticated user.
    pub async fn get_accuracy(&self) -> Result<AccuracyScore> {
        self.get("/api/v1/accuracy/me").await
    }

    /// Get AI-generated portfolio review and optimization suggestions.
    pub async fn get_portfolio_review(&self) -> Result<PortfolioReview> {
        self.get("/api/v1/ai/portfolio-review").await
    }

    /// Get aggregated news sentiment for a specific market.
    pub async fn get_market_sentiment(&self, market_id: &str) -> Result<MarketSentiment> {
        self.get(&format!("/api/v1/news/sentiment/{}", encode(market_id))).await
    }

    /// Provide liquidity by placing two-sided quotes on a market token.
    pub async fn provide_liquidity(&self, params: &ProvideLiquidityParams) -> Result<LpPosition> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/lp/provide", &body).await
    }

    // -----------------------------------------------------------------------
    // Strategy Execution Watching (SSE)
    // -----------------------------------------------------------------------

    /// Open a live SSE event stream for a running (or backtesting) strategy.
    ///
    /// Returns a [`StrategyEventStream`] that you poll with `.next().await`.
    /// The first event always has `event_type == "CONNECTED"`.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the server returns a
    /// non-2xx status (e.g. strategy not found, insufficient scope).
    pub async fn watch_strategy(&self, strategy_id: &str) -> Result<StrategyEventStream> {
        let path = format!("/api/v1/strategies/{}/events", encode(strategy_id));
        let resp = self
            .http
            .get(self.url(&path))
            .header(AUTHORIZATION, self.auth_header())
            .header("Accept", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            return Err(PolyforgeError::Api {
                status,
                code: body.get("code").and_then(|v| v.as_str()).unwrap_or("STREAM_ERROR").to_string(),
                message: body.get("message").and_then(|v| v.as_str()).unwrap_or("SSE stream request failed").to_string(),
                request_id: None,
            });
        }

        Ok(StrategyEventStream {
            response: resp,
            buffer: String::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_construction() {
        let client = PolyforgeClient::new("test-api-key");
        assert_eq!(client.base_url, DEFAULT_BASE_URL);
    }

    #[test]
    fn test_client_with_custom_url() {
        let client = PolyforgeClient::with_url("test-api-key", "https://api.example.com");
        assert_eq!(client.base_url, "https://api.example.com");
    }

    #[test]
    fn test_client_url_normalization() {
        let client = PolyforgeClient::with_url("test-api-key", "http://localhost:3002/");
        assert_eq!(client.base_url, "http://localhost:3002");
    }

    #[test]
    fn test_client_url_construction() {
        let client = PolyforgeClient::new("test-api-key");
        let url = client.url("/api/v1/markets");
        assert_eq!(url, "https://localhost:3002/api/v1/markets");
    }

    #[test]
    fn test_client_url_construction_with_custom_base() {
        let client = PolyforgeClient::with_url("test-api-key", "https://api.example.com");
        let url = client.url("/api/v1/strategies");
        assert_eq!(url, "https://api.example.com/api/v1/strategies");
    }

    #[test]
    fn test_client_auth_header() {
        let client = PolyforgeClient::new("my-secret-key");
        let header = client.auth_header();
        assert_eq!(
            header.to_str().unwrap(),
            "Bearer my-secret-key"
        );
    }

    #[test]
    fn test_api_error_construction() {
        let error = PolyforgeError::Api {
            status: 404,
            code: "NOT_FOUND".to_string(),
            message: "Resource not found".to_string(),
            request_id: Some("req-123".to_string()),
        };

        if let PolyforgeError::Api { status, code, message, request_id } = error {
            assert_eq!(status, 404);
            assert_eq!(code, "NOT_FOUND");
            assert_eq!(message, "Resource not found");
            assert_eq!(request_id, Some("req-123".to_string()));
        } else {
            panic!("Expected Api error variant");
        }
    }

    #[test]
    fn test_api_error_without_request_id() {
        let error = PolyforgeError::Api {
            status: 500,
            code: "INTERNAL_ERROR".to_string(),
            message: "Internal server error".to_string(),
            request_id: None,
        };

        if let PolyforgeError::Api { status, code, message, request_id } = error {
            assert_eq!(status, 500);
            assert_eq!(code, "INTERNAL_ERROR");
            assert_eq!(message, "Internal server error");
            assert_eq!(request_id, None);
        } else {
            panic!("Expected Api error variant");
        }
    }

    #[test]
    fn test_client_with_different_api_keys() {
        let client1 = PolyforgeClient::new("key-1");
        let client2 = PolyforgeClient::new("key-2");

        let header1 = client1.auth_header();
        let header2 = client2.auth_header();

        assert_ne!(header1.to_str().unwrap(), header2.to_str().unwrap());
    }
}
