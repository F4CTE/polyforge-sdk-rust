use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use secrecy::{ExposeSecret, SecretBox};
use serde_json::json;
use url::Url;
use urlencoding::encode;

use std::time::Duration;
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
/// # let client = polyforge::PolyforgeClient::new("key")?;
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
                Ok(Some(chunk)) => match String::from_utf8(chunk.to_vec()) {
                    Ok(s) => self.buffer.push_str(&s),
                    Err(e) => {
                        return Some(Err(PolyforgeError::Api {
                            status: 0,
                            code: "INVALID_UTF8".into(),
                            message: format!("Invalid UTF-8 in SSE stream: {}", e),
                            request_id: None,
                        }))
                    }
                },
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
    api_key: SecretBox<String>,
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
    /// Create a new client using the `POLYFORGE_API_URL` environment variable,
    /// falling back to the default local URL (`https://localhost:3002`).
    ///
    /// # Errors
    /// Returns [`PolyforgeError::Http`] if the underlying HTTP client fails to build.
    /// Returns [`PolyforgeError::Validation`] if the URL is invalid.
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        let url = std::env::var("POLYFORGE_API_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        Self::with_url(api_key, url)
    }

    /// Create a new client with a custom base URL.
    ///
    /// # Errors
    /// Returns [`PolyforgeError::Validation`] if the URL is malformed, uses a
    /// non-HTTPS scheme (HTTP is only allowed for `localhost` / `127.0.0.1`),
    /// or contains path-traversal sequences.
    ///
    /// Returns [`PolyforgeError::Http`] if the underlying HTTP client fails to
    /// build (e.g. TLS misconfiguration).
    pub fn with_url(api_key: impl Into<String>, api_url: impl Into<String>) -> Result<Self> {
        let raw_url = api_url.into();
        let base_url = Self::validate_base_url(&raw_url)?;

        let api_key = SecretBox::new(Box::new(api_key.into()));
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(PolyforgeError::Http)?;

        Ok(Self {
            http,
            base_url,
            api_key,
        })
    }

    /// Validate and normalise a base URL.
    ///
    /// Rules:
    /// - Must be a well-formed URL with a host.
    /// - Must use the `https` scheme, **unless** the host is `localhost` or
    ///   `127.0.0.1` (development convenience).
    /// - Must not contain path-traversal sequences (`..`) or fragment/query
    ///   parts that could lead to injection.
    /// - Trailing slashes are stripped for consistent path joining.
    fn validate_base_url(raw: &str) -> Result<String> {
        let parsed = Url::parse(raw).map_err(|e| {
            PolyforgeError::Validation(format!("Malformed base URL: {e}"))
        })?;

        // --- scheme check ---------------------------------------------------
        let scheme = parsed.scheme();
        let host = parsed.host_str().unwrap_or_default();
        let is_local = host == "localhost"
            || host == "127.0.0.1"
            || host == "[::1]"
            || host == "::1"
            || host == "0.0.0.0"
            || host == "localhost.localdomain"
            || host.starts_with("127.");

        match scheme {
            "https" => {} // always OK
            "http" if is_local => {} // allowed for local dev
            "http" => {
                return Err(PolyforgeError::Validation(
                    "Base URL must use HTTPS (HTTP is only allowed for localhost/127.0.0.1)".into(),
                ));
            }
            other => {
                return Err(PolyforgeError::Validation(format!(
                    "Unsupported URL scheme \"{other}\"; expected \"https\" (or \"http\" for localhost)"
                )));
            }
        }

        // --- host must be present -------------------------------------------
        if host.is_empty() {
            return Err(PolyforgeError::Validation(
                "Base URL must contain a host".into(),
            ));
        }

        // --- reject path traversal and injection characters -----------------
        // Check the raw input because the URL parser normalises `..` segments away.
        if raw.contains("..") {
            return Err(PolyforgeError::Validation(
                "Base URL must not contain path-traversal sequences (..)".into(),
            ));
        }

        if parsed.query().is_some() {
            return Err(PolyforgeError::Validation(
                "Base URL must not contain a query string".into(),
            ));
        }

        if parsed.fragment().is_some() {
            return Err(PolyforgeError::Validation(
                "Base URL must not contain a fragment".into(),
            ));
        }

        // --- normalise: strip trailing slashes ------------------------------
        let normalised = raw.trim_end_matches('/').to_string();
        Ok(normalised)
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn auth_header(&self) -> Result<HeaderValue> {
        HeaderValue::from_str(&format!("Bearer {}", self.api_key.expose_secret()))
            .map_err(|_| PolyforgeError::Validation("API key contains invalid HTTP header characters".into()))
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .http
            .get(self.url(path))
            .header(AUTHORIZATION, self.auth_header()?)
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
            .header(AUTHORIZATION, self.auth_header()?)
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
            .header(AUTHORIZATION, self.auth_header()?)
            .json(body)
            .send()
            .await?;
        self.handle_response(resp).await
    }

    async fn delete<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .http
            .delete(self.url(path))
            .header(AUTHORIZATION, self.auth_header()?)
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
        if status == 204 {
            return serde_json::from_value(serde_json::Value::Null)
                .map_err(PolyforgeError::from);
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
            let pairs: Vec<String> = qp
                .iter()
                .map(|(k, v)| format!("{}={}", k, encode(v)))
                .collect();
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
    pub async fn list_strategies(&self, status: Option<StrategyStatus>) -> Result<Vec<Strategy>> {
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
        self.get(&format!("/api/v1/strategies/{}", encode(id)))
            .await
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
        let mut body = json!({ "query": description });
        if let Some(mid) = market_id {
            body["marketId"] = json!(mid);
        }
        self.post("/api/v1/strategies/from-description", &body)
            .await
    }

    /// Start a strategy in the given trading mode.
    pub async fn start_strategy(&self, id: &str, mode: TradingMode) -> Result<Strategy> {
        let body = json!({ "mode": mode });
        self.post(&format!("/api/v1/strategies/{}/start", encode(id)), &body)
            .await
    }

    /// Stop a running strategy.
    pub async fn stop_strategy(&self, id: &str) -> Result<Strategy> {
        self.post(
            &format!("/api/v1/strategies/{}/stop", encode(id)),
            &json!({}),
        )
        .await
    }

    /// Get available strategy templates.
    pub async fn get_strategy_templates(&self) -> Result<Vec<StrategyTemplate>> {
        self.get("/api/v1/strategies/templates").await
    }

    /// Export a strategy configuration as JSON.
    pub async fn export_strategy(&self, id: &str) -> Result<serde_json::Value> {
        self.get(&format!("/api/v1/strategies/{}/export", encode(id)))
            .await
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
        self.patch(&format!("/api/v1/strategies/{}", encode(id)), &body)
            .await
    }

    /// Delete a strategy by ID. Returns `()` on success (platform returns 204 No Content).
    pub async fn delete_strategy(&self, id: &str) -> Result<()> {
        self.delete::<serde_json::Value>(&format!("/api/v1/strategies/{}", encode(id)))
            .await?;
        Ok(())
    }

    /// Import a strategy from a .polyforge JSON export.
    pub async fn import_strategy(&self, data: &serde_json::Value) -> Result<Strategy> {
        let body = serde_json::json!({ "data": data });
        self.post("/api/v1/strategies/import", &body).await
    }

    /// Pause a running strategy.
    pub async fn pause_strategy(&self, id: &str) -> Result<Strategy> {
        self.post(
            &format!("/api/v1/strategies/{}/pause", encode(id)),
            &serde_json::json!({}),
        )
        .await
    }

    /// Resume a paused strategy.
    pub async fn resume_strategy(&self, id: &str) -> Result<Strategy> {
        self.post(
            &format!("/api/v1/strategies/{}/resume", encode(id)),
            &serde_json::json!({}),
        )
        .await
    }

    /// Fork a strategy to create a new editable copy.
    pub async fn fork_strategy(&self, id: &str) -> Result<Strategy> {
        self.post(
            &format!("/api/v1/strategies/{}/fork", encode(id)),
            &serde_json::json!({}),
        )
        .await
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
            let pairs: Vec<String> = qp.iter().map(|(k, v)| format!("{}={}", k, encode(v))).collect();
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
        self.delete(&format!("/api/v1/orders/{}", encode(order_id)))
            .await
    }

    /// Close an open position (sell all shares at market price).
    pub async fn close_position(&self, params: &ClosePositionParams) -> Result<PlaceOrderResponse> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/orders/close-position", &body).await
    }

    /// Redeem winning shares after a market resolves.
    pub async fn redeem_position(
        &self,
        params: &RedeemPositionParams,
    ) -> Result<PlaceOrderResponse> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/orders/redeem", &body).await
    }

    /// Split a position into smaller positions.
    pub async fn split_position(&self, params: &SplitPositionParams) -> Result<PlaceOrderResponse> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/orders/split", &body).await
    }

    /// Merge multiple positions into one.
    pub async fn merge_positions(
        &self,
        params: &MergePositionParams,
    ) -> Result<PlaceOrderResponse> {
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
        self.delete(&format!("/api/v1/orders/smart/{}", encode(id)))
            .await
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
        self.get(&format!("/api/v1/marketplace/{}", encode(id)))
            .await
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
    pub async fn get_news_signals(&self, min_confidence: Option<u32>) -> Result<Vec<NewsSignal>> {
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
    ///
    /// The URL must use the `https` scheme and must not point to a private or
    /// loopback IP address.
    pub async fn create_webhook(&self, url: &str, events: &[WebhookEvent]) -> Result<Webhook> {
        Self::validate_webhook_url(url)?;
        let body = json!({
            "url": url,
            "events": events,
        });
        self.post("/api/v1/webhooks", &body).await
    }

    /// Validates a webhook URL: must be HTTPS, well-formed, and not target
    /// private/loopback networks.
    fn validate_webhook_url(url: &str) -> Result<()> {
        let parsed = Url::parse(url)
            .map_err(|_| PolyforgeError::Validation("Invalid webhook URL".into()))?;

        if parsed.scheme() != "https" {
            return Err(PolyforgeError::Validation(
                "Webhook URL must use HTTPS".into(),
            ));
        }

        let host = parsed.host_str().unwrap_or_default();

        if host == "localhost" || host.ends_with(".local") {
            return Err(PolyforgeError::Validation(
                "Webhook URL must not target localhost".into(),
            ));
        }

        // Block cloud metadata hostnames
        if host == "metadata.google.internal" || host.ends_with(".internal") {
            return Err(PolyforgeError::Validation(
                "Webhook URL must not target cloud metadata endpoints".into(),
            ));
        }

        // Block private and link-local IP ranges to prevent SSRF
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            let is_private = match ip {
                std::net::IpAddr::V4(v4) => {
                    v4.is_loopback()
                        || v4.is_private()
                        || v4.is_link_local()
                        || v4.is_unspecified()
                        || (v4.octets()[0] == 169 && v4.octets()[1] == 254)
                }
                std::net::IpAddr::V6(v6) => {
                    v6.is_loopback()
                        || v6.is_unspecified()
                        // unique-local fc00::/7
                        || (v6.octets()[0] & 0xfe) == 0xfc
                        // link-local fe80::/10
                        || (v6.octets()[0] == 0xfe && (v6.octets()[1] & 0xc0) == 0x80)
                        // IPv4-mapped ::ffff:x.x.x.x — check the mapped IPv4
                        || v6.to_ipv4_mapped().is_some_and(|v4| {
                            v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
                        })
                }
            };
            if is_private {
                return Err(PolyforgeError::Validation(
                    "Webhook URL must not target private or loopback addresses".into(),
                ));
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // AI
    // -----------------------------------------------------------------------

    /// Send a natural-language query to the AI assistant.
    pub async fn ai_query(&self, query: &str) -> Result<AiQueryResponse> {
        let body = json!({ "question": query });
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
        self.get(&format!("/api/v1/news/sentiment/{}", encode(market_id)))
            .await
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
            .header(AUTHORIZATION, self.auth_header()?)
            .header("Accept", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            return Err(PolyforgeError::Api {
                status,
                code: body
                    .get("code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("STREAM_ERROR")
                    .to_string(),
                message: body
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("SSE stream request failed")
                    .to_string(),
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
        let client = PolyforgeClient::new("test-api-key").unwrap();
        assert_eq!(client.base_url, DEFAULT_BASE_URL);
    }

    #[test]
    fn test_client_with_custom_url() {
        let client = PolyforgeClient::with_url("test-api-key", "https://api.example.com").unwrap();
        assert_eq!(client.base_url, "https://api.example.com");
    }

    #[test]
    fn test_client_url_normalization() {
        let client = PolyforgeClient::with_url("test-api-key", "http://localhost:3002/").unwrap();
        assert_eq!(client.base_url, "http://localhost:3002");
    }

    #[test]
    fn test_client_url_construction() {
        let client = PolyforgeClient::new("test-api-key").unwrap();
        let url = client.url("/api/v1/markets");
        assert_eq!(url, "https://localhost:3002/api/v1/markets");
    }

    #[test]
    fn test_client_url_construction_with_custom_base() {
        let client = PolyforgeClient::with_url("test-api-key", "https://api.example.com").unwrap();
        let url = client.url("/api/v1/strategies");
        assert_eq!(url, "https://api.example.com/api/v1/strategies");
    }

    #[test]
    fn test_client_auth_header() {
        let client = PolyforgeClient::new("my-secret-key").unwrap();
        let header = client.auth_header().unwrap();
        assert_eq!(header.to_str().unwrap(), "Bearer my-secret-key");
    }

    #[test]
    fn test_api_error_construction() {
        let error = PolyforgeError::Api {
            status: 404,
            code: "NOT_FOUND".to_string(),
            message: "Resource not found".to_string(),
            request_id: Some("req-123".to_string()),
        };

        if let PolyforgeError::Api {
            status,
            code,
            message,
            request_id,
        } = error
        {
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

        if let PolyforgeError::Api {
            status,
            code,
            message,
            request_id,
        } = error
        {
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
        let client1 = PolyforgeClient::new("key-1").unwrap();
        let client2 = PolyforgeClient::new("key-2").unwrap();

        let header1 = client1.auth_header().unwrap();
        let header2 = client2.auth_header().unwrap();

        assert_ne!(header1.to_str().unwrap(), header2.to_str().unwrap());
    }

    #[test]
    fn test_base_url_rejects_http_non_localhost() {
        let result = PolyforgeClient::with_url("key", "http://api.example.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_base_url_allows_http_localhost() {
        let client = PolyforgeClient::with_url("key", "http://localhost:3002").unwrap();
        assert_eq!(client.base_url, "http://localhost:3002");
    }

    #[test]
    fn test_base_url_allows_http_127() {
        let client = PolyforgeClient::with_url("key", "http://127.0.0.1:3002").unwrap();
        assert_eq!(client.base_url, "http://127.0.0.1:3002");
    }

    #[test]
    fn test_base_url_allows_http_127_x() {
        let client = PolyforgeClient::with_url("key", "http://127.0.0.2:3002").unwrap();
        assert_eq!(client.base_url, "http://127.0.0.2:3002");
    }

    #[test]
    fn test_base_url_allows_http_0000() {
        let client = PolyforgeClient::with_url("key", "http://0.0.0.0:3002").unwrap();
        assert_eq!(client.base_url, "http://0.0.0.0:3002");
    }

    #[test]
    fn test_base_url_allows_http_ipv6_loopback() {
        let client = PolyforgeClient::with_url("key", "http://[::1]:3002").unwrap();
        assert_eq!(client.base_url, "http://[::1]:3002");
    }

    #[test]
    fn test_new_reads_env_var() {
        std::env::set_var("POLYFORGE_API_URL", "https://api.staging.example.com");
        let client = PolyforgeClient::new("key").unwrap();
        assert_eq!(client.base_url, "https://api.staging.example.com");
        std::env::remove_var("POLYFORGE_API_URL");
    }

    #[test]
    fn test_base_url_rejects_malformed() {
        let result = PolyforgeClient::with_url("key", "not a url");
        assert!(result.is_err());
    }

    #[test]
    fn test_base_url_rejects_path_traversal() {
        let result = PolyforgeClient::with_url("key", "https://example.com/../admin");
        assert!(result.is_err());
    }

    #[test]
    fn test_base_url_rejects_query_string() {
        let result = PolyforgeClient::with_url("key", "https://example.com?redirect=evil");
        assert!(result.is_err());
    }

    #[test]
    fn test_base_url_rejects_fragment() {
        let result = PolyforgeClient::with_url("key", "https://example.com#frag");
        assert!(result.is_err());
    }

    #[test]
    fn test_base_url_rejects_ftp_scheme() {
        let result = PolyforgeClient::with_url("key", "ftp://example.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_base_url_strips_trailing_slash() {
        let client = PolyforgeClient::with_url("key", "https://api.example.com/").unwrap();
        assert_eq!(client.base_url, "https://api.example.com");
    }

    #[test]
    fn test_webhook_url_rejects_http() {
        let result = PolyforgeClient::validate_webhook_url("http://example.com/hook");
        assert!(result.is_err());
    }

    #[test]
    fn test_webhook_url_rejects_localhost() {
        let result = PolyforgeClient::validate_webhook_url("https://localhost/hook");
        assert!(result.is_err());
    }

    #[test]
    fn test_webhook_url_rejects_private_ip() {
        let result = PolyforgeClient::validate_webhook_url("https://192.168.1.1/hook");
        assert!(result.is_err());
    }

    #[test]
    fn test_webhook_url_rejects_loopback() {
        let result = PolyforgeClient::validate_webhook_url("https://127.0.0.1/hook");
        assert!(result.is_err());
    }

    #[test]
    fn test_webhook_url_rejects_link_local() {
        let result = PolyforgeClient::validate_webhook_url("https://169.254.1.1/hook");
        assert!(result.is_err());
    }

    #[test]
    fn test_webhook_url_accepts_valid_https() {
        let result = PolyforgeClient::validate_webhook_url("https://hooks.example.com/polyforge");
        assert!(result.is_ok());
    }

    #[test]
    fn test_webhook_url_rejects_invalid_url() {
        let result = PolyforgeClient::validate_webhook_url("not a url");
        assert!(result.is_err());
    }
}
