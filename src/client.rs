use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::json;
use urlencoding::encode;

use crate::errors::{PolyforgeError, Result};
use crate::types::*;

const DEFAULT_BASE_URL: &str = "http://localhost:3002";

/// Async client for the Polyforge trading platform REST API.
pub struct PolyforgeClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl PolyforgeClient {
    /// Create a new client pointing at the default local URL (`http://localhost:3002`).
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
        self.get(&format!("/api/v1/markets/{id}")).await
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
        self.get(&format!("/api/v1/strategies/{id}")).await
    }

    /// Create a new strategy with a name and optional description.
    pub async fn create_strategy(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<Strategy> {
        let mut body = json!({ "name": name });
        if let Some(desc) = description {
            body["description"] = json!(desc);
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
        self.post(&format!("/api/v1/strategies/{id}/start"), &body)
            .await
    }

    /// Stop a running strategy.
    pub async fn stop_strategy(&self, id: &str) -> Result<Strategy> {
        self.post(&format!("/api/v1/strategies/{id}/stop"), &json!({}))
            .await
    }

    /// Get available strategy templates.
    pub async fn get_strategy_templates(&self) -> Result<Vec<StrategyTemplate>> {
        self.get("/api/v1/strategies/templates").await
    }

    /// Export a strategy configuration as JSON.
    pub async fn export_strategy(&self, id: &str) -> Result<serde_json::Value> {
        self.get(&format!("/api/v1/strategies/{id}/export")).await
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
        self.get("/api/v1/score").await
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
        self.delete(&format!("/api/v1/orders/{order_id}")).await
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
        self.get(&format!("/api/v1/whale-feed{qs}")).await
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
        self.get(&format!("/api/v1/news-signals{qs}")).await
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
        self.get("/api/v1/copy-configs").await
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
        assert_eq!(url, "http://localhost:3002/api/v1/markets");
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
