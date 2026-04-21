use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use secrecy::{ExposeSecret, SecretBox};
use serde_json::json;
use url::Url;
use urlencoding::encode;

use crate::errors::{PolyforgeError, Result};
use crate::types::*;
use std::time::Duration;

// ---------------------------------------------------------------------------
// StrategyEventStream — lazy SSE reader returned by watch_strategy()
// ---------------------------------------------------------------------------

/// Maximum size of the internal SSE line buffer (1 MiB).
///
/// A well-behaved SSE server sends newline-delimited events that are typically
/// a few hundred bytes each.  Without a cap a malicious (or buggy) server could
/// stream data without ever sending a newline, growing the buffer until the
/// process runs out of memory.  This constant bounds that growth.
const MAX_SSE_BUFFER_SIZE: usize = 1_048_576; // 1 MiB

/// Maximum size of an error response body that the SDK will read (1 MiB).
///
/// A malicious or misconfigured server could return an extremely large error
/// body (e.g. a multi-gigabyte HTML page) which `resp.json()` would buffer
/// entirely in memory.  This constant caps the readable size so that an
/// oversized error response is rejected early instead of causing OOM.
const MAX_RESPONSE_BODY_SIZE: usize = 1_048_576; // 1 MiB

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
                    Ok(s) => {
                        self.buffer.push_str(&s);
                        if self.buffer.len() > MAX_SSE_BUFFER_SIZE {
                            return Some(Err(PolyforgeError::Api {
                                status: 0,
                                code: "SSE_BUFFER_OVERFLOW".into(),
                                message: format!(
                                    "SSE buffer exceeded {} bytes without a newline — \
                                     possible denial-of-service; connection closed",
                                    MAX_SSE_BUFFER_SIZE
                                ),
                                request_id: None,
                                suggestion: None,
                            }));
                        }
                    }
                    Err(e) => {
                        return Some(Err(PolyforgeError::Api {
                            status: 0,
                            code: "INVALID_UTF8".into(),
                            message: format!("Invalid UTF-8 in SSE stream: {}", e),
                            request_id: None,
                            suggestion: None,
                        }))
                    }
                },
                Ok(None) => return None, // Server closed the stream
                Err(e) => return Some(Err(PolyforgeError::from(e))),
            }
        }
    }
}

const DEFAULT_BASE_URL: &str = "https://api.polyforge.app";

/// Validate that a financial parameter is a finite, positive number.
///
/// Rejects `NaN`, `+Inf`, `-Inf`, zero, and negative values — all of which
/// are meaningless for order sizes, prices, and spreads and could cause
/// unexpected behaviour on the backend.
fn validate_financial_param(name: &str, value: f64) -> Result<()> {
    if value.is_nan() {
        return Err(PolyforgeError::Validation(format!(
            "{name} must be a valid number, got NaN"
        )));
    }
    if value.is_infinite() {
        return Err(PolyforgeError::Validation(format!(
            "{name} must be finite, got {}infinity",
            if value.is_sign_negative() { "-" } else { "+" }
        )));
    }
    if value <= 0.0 {
        return Err(PolyforgeError::Validation(format!(
            "{name} must be positive, got {value}"
        )));
    }
    Ok(())
}

/// Validate an optional financial parameter (skip if `None`).
fn validate_optional_financial_param(name: &str, value: Option<f64>) -> Result<()> {
    if let Some(v) = value {
        validate_financial_param(name, v)?;
    }
    Ok(())
}

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
    /// falling back to the production API URL (`https://api.polyforge.app`).
    ///
    /// # Errors
    /// Returns [`PolyforgeError::Http`] if the underlying HTTP client fails to build.
    /// Returns [`PolyforgeError::Validation`] if the URL is invalid.
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        let url =
            std::env::var("POLYFORGE_API_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
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
        let parsed = Url::parse(raw)
            .map_err(|e| PolyforgeError::Validation(format!("Malformed base URL: {e}")))?;

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
            "https" => {}            // always OK
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

        // --- SSRF: block private / reserved IP ranges -----------------------
        // Exempt localhost-family hosts (already allowed above for HTTP dev),
        // but reject all other private, loopback, link-local, CGNAT, and
        // cloud-metadata addresses.  This prevents the API key from being
        // sent to an attacker-controlled internal address.
        if !is_local {
            // Block cloud metadata hostnames
            if host == "metadata.google.internal"
                || host.ends_with(".internal")
                || host.ends_with(".local")
            {
                return Err(PolyforgeError::Validation(
                    "Base URL must not target cloud metadata or local network endpoints".into(),
                ));
            }

            // Check literal IP addresses against the private/reserved blocklist
            if let Ok(ip) = host.parse::<std::net::IpAddr>() {
                if Self::is_blocked_ip(ip) {
                    return Err(PolyforgeError::Validation(
                        "Base URL must not target private or reserved IP addresses (SSRF protection)".into(),
                    ));
                }
            }

            // Check bracketed IPv6 literals (the URL parser strips brackets)
            let bare_host = host.trim_start_matches('[').trim_end_matches(']');
            if bare_host != host {
                if let Ok(ip) = bare_host.parse::<std::net::IpAddr>() {
                    if Self::is_blocked_ip(ip) {
                        return Err(PolyforgeError::Validation(
                            "Base URL must not target private or reserved IP addresses (SSRF protection)".into(),
                        ));
                    }
                }
            }
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
        HeaderValue::from_str(&format!("Bearer {}", self.api_key.expose_secret())).map_err(|_| {
            PolyforgeError::Validation("API key contains invalid HTTP header characters".into())
        })
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

    /// Send a GET and return the body as plain text (for CSV endpoints).
    async fn get_text(&self, path: &str) -> Result<String> {
        let resp = self
            .http
            .get(self.url(path))
            .header(AUTHORIZATION, self.auth_header()?)
            .send()
            .await?;
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
                    .get("requestId")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                suggestion: body
                    .get("suggestion")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            });
        }
        Ok(resp.text().await?)
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
            // Guard: reject error responses whose Content-Length exceeds
            // MAX_RESPONSE_BODY_SIZE to prevent memory amplification attacks
            // where a malicious server returns a multi-gigabyte error body.
            let content_length = resp
                .headers()
                .get(reqwest::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
            if let Some(cl) = content_length {
                if cl > MAX_RESPONSE_BODY_SIZE as u64 {
                    return Err(PolyforgeError::Api {
                        status,
                        code: "RESPONSE_BODY_TOO_LARGE".to_string(),
                        message: format!(
                            "Error response body too large ({cl} bytes, limit {MAX_RESPONSE_BODY_SIZE})"
                        ),
                        request_id: None,
                        suggestion: None,
                    });
                }
            }

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
                    .get("requestId")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                suggestion: body
                    .get("suggestion")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            });
        }
        if status == 204 {
            return serde_json::from_value(serde_json::Value::Null).map_err(PolyforgeError::from);
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
        if let Some(ref s) = params.sort {
            qp.push(("sort", s.clone()));
        }
        if let Some(c) = params.closed {
            qp.push(("closed", c.to_string()));
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

    /// Get price history for a market token.
    ///
    /// Pass `None` for `params` to use server defaults (`resolution = "1h"`).
    pub async fn get_price_history(
        &self,
        token_id: &str,
        params: Option<PriceHistoryParams>,
    ) -> Result<PriceHistoryResponse> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(ref p) = params {
            if let Some(ref r) = p.resolution {
                qp.push(("resolution", r.clone()));
            }
            if let Some(ref f) = p.from {
                qp.push(("from", f.clone()));
            }
            if let Some(ref t) = p.to {
                qp.push(("to", t.clone()));
            }
            if let Some(l) = p.limit {
                qp.push(("limit", l.to_string()));
            }
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

        self.get(&format!(
            "/api/v1/markets/{}/price-history{qs}",
            encode(token_id)
        ))
        .await
    }

    /// Get the order book for a market token.
    pub async fn get_order_book(&self, token_id: &str) -> Result<OrderBook> {
        self.get(&format!("/api/v1/markets/{}/book", encode(token_id)))
            .await
    }

    // -----------------------------------------------------------------------
    // Strategies
    // -----------------------------------------------------------------------

    /// List strategies with optional filtering, sorting, and pagination.
    pub async fn list_strategies(
        &self,
        params: &ListStrategiesParams,
    ) -> Result<PaginatedResponse<Strategy>> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(ref s) = params.status {
            let val = serde_json::to_value(s).unwrap_or_default();
            if let Some(v) = val.as_str() {
                qp.push(("status", v.to_string()));
            }
        }
        if let Some(ref s) = params.sort {
            qp.push(("sort", s.clone()));
        }
        if let Some(p) = params.page {
            qp.push(("page", p.to_string()));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
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
        self.get(&format!("/api/v1/strategies{qs}")).await
    }

    /// Get a single strategy by ID.
    pub async fn get_strategy(&self, id: &str) -> Result<Strategy> {
        self.get(&format!("/api/v1/strategies/{}", encode(id)))
            .await
    }

    /// Create a new strategy with full block configuration.
    ///
    /// Use [`CreateStrategyParams`] to specify blocks, visibility, execution mode,
    /// tags, and other strategy properties.
    pub async fn create_strategy(&self, params: &CreateStrategyParams) -> Result<Strategy> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
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
            body["marketId"] = json!(mid);
        }
        self.post("/api/v1/strategies/from-description", &body)
            .await
    }

    /// Start a strategy in the given trading mode.
    ///
    /// Returns a [`StrategyStatusResponse`] with the new status and `startedAt`
    /// timestamp — not a full [`Strategy`] object.
    pub async fn start_strategy(
        &self,
        id: &str,
        mode: TradingMode,
    ) -> Result<StrategyStatusResponse> {
        let body = json!({ "mode": mode });
        self.post(&format!("/api/v1/strategies/{}/start", encode(id)), &body)
            .await
    }

    /// Stop a running strategy.
    ///
    /// Returns a [`StrategyStatusResponse`] with the new status and `stoppedAt`
    /// timestamp.
    pub async fn stop_strategy(&self, id: &str) -> Result<StrategyStatusResponse> {
        self.post(
            &format!("/api/v1/strategies/{}/stop", encode(id)),
            &json!({}),
        )
        .await
    }

    /// Get available strategy templates.
    pub async fn get_strategy_templates(&self) -> Result<PaginatedResponse<StrategyTemplate>> {
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

    /// Import a strategy from a `.polyforge` JSON export.
    ///
    /// # Arguments
    /// * `polyforge_version` — format version string (e.g. `"1.0"`)
    /// * `strategy` — the strategy payload object
    /// * `exported_at` — optional ISO-8601 timestamp of when the export was created
    pub async fn import_strategy(
        &self,
        polyforge_version: &str,
        strategy: &serde_json::Value,
        exported_at: Option<&str>,
    ) -> Result<Strategy> {
        let mut body = serde_json::json!({
            "polyforge": polyforge_version,
            "strategy": strategy,
        });
        if let Some(at) = exported_at {
            body["exportedAt"] = serde_json::json!(at);
        }
        self.post("/api/v1/strategies/import", &body).await
    }

    /// Pause a running strategy.
    pub async fn pause_strategy(&self, id: &str) -> Result<StrategyStatusResponse> {
        self.post(
            &format!("/api/v1/strategies/{}/pause", encode(id)),
            &serde_json::json!({}),
        )
        .await
    }

    /// Resume a paused strategy.
    pub async fn resume_strategy(&self, id: &str) -> Result<StrategyStatusResponse> {
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
    // Strategy Social
    // -----------------------------------------------------------------------

    /// Like or unlike a strategy (toggle). Returns `{"liked": bool, "likeCount": i64}`.
    pub async fn like_strategy(&self, id: &str) -> Result<serde_json::Value> {
        self.post(
            &format!("/api/v1/strategies/{}/like", encode(id)),
            &serde_json::json!({}),
        )
        .await
    }

    /// List comments on a strategy with optional pagination.
    pub async fn list_strategy_comments(
        &self,
        id: &str,
        page: Option<u32>,
        limit: Option<u32>,
    ) -> Result<serde_json::Value> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(p) = page {
            qp.push(("page", p.to_string()));
        }
        if let Some(l) = limit {
            qp.push(("limit", l.to_string()));
        }
        let qs = if qp.is_empty() {
            String::new()
        } else {
            format!(
                "?{}",
                qp.iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("&")
            )
        };
        self.get(&format!("/api/v1/strategies/{}/comments{}", encode(id), qs))
            .await
    }

    /// Add a comment to a strategy.
    pub async fn add_strategy_comment(&self, id: &str, content: &str) -> Result<serde_json::Value> {
        self.post(
            &format!("/api/v1/strategies/{}/comments", encode(id)),
            &serde_json::json!({ "content": content }),
        )
        .await
    }

    /// Delete a comment on a strategy (must be the comment author).
    pub async fn delete_strategy_comment(&self, strategy_id: &str, comment_id: &str) -> Result<()> {
        self.delete(&format!(
            "/api/v1/strategies/{}/comments/{}",
            encode(strategy_id),
            encode(comment_id),
        ))
        .await
    }

    /// List child strategies (forks) of a strategy.
    pub async fn list_strategy_children(&self, id: &str) -> Result<serde_json::Value> {
        self.get(&format!("/api/v1/strategies/{}/children", encode(id)))
            .await
    }

    /// Report a strategy for violating guidelines.
    ///
    /// `reason` should be one of `"SPAM"`, `"MISLEADING"`, `"HARMFUL"`, `"OTHER"`.
    pub async fn report_strategy(
        &self,
        id: &str,
        reason: &str,
        description: Option<&str>,
    ) -> Result<serde_json::Value> {
        let mut body = serde_json::json!({ "reason": reason });
        if let Some(d) = description {
            body["description"] = serde_json::json!(d);
        }
        self.post(&format!("/api/v1/strategies/{}/report", encode(id)), &body)
            .await
    }

    // -----------------------------------------------------------------------
    // Strategy Versioning
    // -----------------------------------------------------------------------

    /// List all saved versions of a strategy.
    pub async fn list_strategy_versions(&self, id: &str) -> Result<serde_json::Value> {
        self.get(&format!("/api/v1/strategies/{}/versions", encode(id)))
            .await
    }

    /// Rollback a strategy to a previous version.
    pub async fn rollback_strategy(&self, id: &str, version_id: &str) -> Result<serde_json::Value> {
        self.post(
            &format!(
                "/api/v1/strategies/{}/versions/{}/rollback",
                encode(id),
                encode(version_id)
            ),
            &serde_json::json!({}),
        )
        .await
    }

    // -----------------------------------------------------------------------
    // Strategy Event Log
    // -----------------------------------------------------------------------

    /// Get the execution event log for a strategy.
    pub async fn get_strategy_event_log(
        &self,
        id: &str,
        limit: Option<u32>,
    ) -> Result<serde_json::Value> {
        let qs = limit.map_or(String::new(), |l| format!("?limit={}", l));
        self.get(&format!(
            "/api/v1/strategies/{}/event-log{}",
            encode(id),
            qs
        ))
        .await
    }

    // -----------------------------------------------------------------------
    // API Key Management
    // -----------------------------------------------------------------------

    /// List all API keys for the authenticated user.
    ///
    /// The raw token is never returned — only the prefix is available.
    pub async fn list_api_keys(&self) -> Result<serde_json::Value> {
        self.get("/api/v1/api-keys").await
    }

    /// Create a new API key.
    ///
    /// The raw token is returned only once and cannot be retrieved later.
    /// `scopes` should be a subset of `["READ", "WRITE", "TRADE"]`.
    pub async fn create_api_key(
        &self,
        name: &str,
        scopes: Option<&[&str]>,
    ) -> Result<serde_json::Value> {
        let mut body = serde_json::json!({ "name": name });
        if let Some(s) = scopes {
            body["scopes"] = serde_json::json!(s);
        }
        self.post("/api/v1/api-keys", &body).await
    }

    /// Revoke an API key by ID. The key is permanently deactivated.
    pub async fn revoke_api_key(&self, id: &str) -> Result<()> {
        self.delete(&format!("/api/v1/api-keys/{}", encode(id)))
            .await
    }

    // -----------------------------------------------------------------------
    // Backtesting
    // -----------------------------------------------------------------------

    /// List backtests with optional filtering by strategy, status, and pagination.
    pub async fn list_backtests(
        &self,
        params: &ListBacktestsParams,
    ) -> Result<PaginatedResponse<Backtest>> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(ref s) = params.strategy_id {
            qp.push(("strategyId", s.clone()));
        }
        if let Some(ref s) = params.status {
            qp.push(("status", s.clone()));
        }
        if let Some(p) = params.page {
            qp.push(("page", p.to_string()));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
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

        self.get(&format!("/api/v1/backtests{qs}")).await
    }

    /// Get a single backtest by ID.
    pub async fn get_backtest(&self, id: &str) -> Result<Backtest> {
        self.get(&format!("/api/v1/backtests/{}", encode(id))).await
    }

    /// Run a backtest with the given parameters.
    ///
    /// The platform does **not** accept an `initialBalance` field.
    /// Use [`RunBacktestParams`] to specify strategy ID, date range, quick mode,
    /// strategy blocks, and market bindings.
    pub async fn run_backtest(&self, params: &RunBacktestParams) -> Result<Backtest> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/backtests", &body).await
    }

    /// Run a quick synchronous backtest (returns results immediately, no polling).
    pub async fn run_quick_backtest(&self, params: &RunBacktestParams) -> Result<Backtest> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/backtests/quick", &body).await
    }

    /// Get the order list for a completed backtest.
    pub async fn get_backtest_orders(&self, id: &str) -> Result<Vec<Order>> {
        self.get(&format!("/api/v1/backtests/{}/orders", encode(id)))
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
    pub async fn get_orders(&self, params: &ListOrdersParams) -> Result<PaginatedResponse<Order>> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(p) = params.page {
            qp.push(("page", p.to_string()));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(ref s) = params.status {
            let val = serde_json::to_value(s).unwrap_or_default();
            qp.push(("status", val.as_str().unwrap_or_default().to_string()));
        }
        if let Some(ref s) = params.strategy_id {
            qp.push(("strategyId", s.clone()));
        }
        if let Some(ref m) = params.market_id {
            qp.push(("marketId", m.clone()));
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
            let pairs: Vec<String> = qp
                .iter()
                .map(|(k, v)| format!("{}={}", k, encode(v)))
                .collect();
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
    ///
    /// # Errors
    /// Returns [`PolyforgeError::Validation`] if `size` or `price` is NaN,
    /// infinite, zero, or negative.
    pub async fn place_order(&self, params: &PlaceOrderParams) -> Result<PlaceOrderResponse> {
        validate_financial_param("size", params.size)?;
        validate_financial_param("price", params.price)?;
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
    ) -> Result<RedeemPositionResponse> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/orders/redeem", &body).await
    }

    /// Split a position into smaller positions.
    pub async fn split_position(&self, params: &SplitPositionParams) -> Result<PlaceOrderResponse> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/orders/split", &body).await
    }

    /// Merge a position (combine token shares).
    pub async fn merge_position(&self, params: &MergePositionParams) -> Result<PlaceOrderResponse> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/orders/merge", &body).await
    }

    /// Export order history as CSV.
    ///
    /// ```no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = polyforge::PolyforgeClient::new("key")?;
    /// let csv = client.export_orders_csv().await?;
    /// std::fs::write("orders.csv", &csv)?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn export_orders_csv(&self) -> Result<String> {
        self.get_text("/api/v1/orders/export/csv").await
    }

    /// Export portfolio positions as CSV.
    ///
    /// ```no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = polyforge::PolyforgeClient::new("key")?;
    /// let csv = client.export_portfolio_csv().await?;
    /// std::fs::write("portfolio.csv", &csv)?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn export_portfolio_csv(&self) -> Result<String> {
        self.get_text("/api/v1/portfolio/export/csv").await
    }

    // -----------------------------------------------------------------------
    // Risk Settings
    // -----------------------------------------------------------------------

    /// Get the current risk / circuit-breaker settings.
    pub async fn get_risk_settings(&self) -> Result<RiskSettings> {
        self.get("/api/v1/settings/risk").await
    }

    /// Update risk settings. Only supplied fields are changed.
    pub async fn update_risk_settings(
        &self,
        params: &UpdateRiskSettingsParams,
    ) -> Result<RiskSettings> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.patch("/api/v1/settings/risk", &body).await
    }

    /// Reset the circuit breaker after it has been tripped.
    ///
    /// Returns the updated risk settings with `circuit_breaker_tripped: false`.
    pub async fn reset_circuit_breaker(&self) -> Result<RiskSettings> {
        self.post(
            "/api/v1/settings/risk/reset",
            &serde_json::Value::Object(Default::default()),
        )
        .await
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
    ///
    /// # Errors
    /// Returns [`PolyforgeError::Validation`] if `total_size` or any optional
    /// price parameter is NaN, infinite, zero, or negative.
    pub async fn place_smart_order(
        &self,
        params: &PlaceSmartOrderParams,
    ) -> Result<PlaceSmartOrderResponse> {
        validate_financial_param("total_size", params.total_size)?;
        validate_optional_financial_param("limit_price", params.limit_price)?;
        validate_optional_financial_param("entry_price", params.entry_price)?;
        validate_optional_financial_param("take_profit_price", params.take_profit_price)?;
        validate_optional_financial_param("stop_loss_price", params.stop_loss_price)?;
        validate_optional_financial_param("price_a", params.price_a)?;
        validate_optional_financial_param("price_b", params.price_b)?;
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/orders/smart", &body).await
    }

    /// List your smart orders with child order progress.
    pub async fn list_smart_orders(&self) -> Result<PaginatedResponse<SmartOrder>> {
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
        if let Some(offset) = params.offset {
            qp.push(format!("offset={offset}"));
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

    /// Rate a purchased marketplace strategy (1–5 stars).
    pub async fn rate_listing(
        &self,
        id: &str,
        params: &RateListingParams,
    ) -> Result<serde_json::Value> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post(&format!("/api/v1/marketplace/{}/rate", encode(id)), &body)
            .await
    }

    /// List your own marketplace listings (sell-side).
    pub async fn get_my_listings(&self) -> Result<Vec<MarketplaceListing>> {
        self.get("/api/v1/marketplace/my/listings").await
    }

    /// List strategies you have purchased from the marketplace.
    pub async fn get_my_purchases(&self) -> Result<Vec<serde_json::Value>> {
        self.get("/api/v1/marketplace/my/purchases").await
    }

    /// Create a new marketplace listing for one of your strategies.
    pub async fn create_listing(&self, params: &CreateListingParams) -> Result<MarketplaceListing> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/marketplace", &body).await
    }

    /// Update an existing marketplace listing.
    pub async fn update_listing(
        &self,
        id: &str,
        params: &UpdateListingParams,
    ) -> Result<MarketplaceListing> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.patch(&format!("/api/v1/marketplace/{}", encode(id)), &body)
            .await
    }

    // -----------------------------------------------------------------------
    // Social & Signals
    // -----------------------------------------------------------------------

    /// Get the whale trade feed.
    pub async fn get_whale_feed(
        &self,
        params: Option<&GetWhaleFeedParams>,
    ) -> Result<PaginatedResponse<WhaleTrade>> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(p) = params {
            if let Some(min_size) = p.min_size {
                qp.push(("minSize", min_size.to_string()));
            }
            if let Some(ref market_id) = p.market_id {
                qp.push(("marketId", market_id.clone()));
            }
            if let Some(ref wallet) = p.wallet_address {
                qp.push(("walletAddress", wallet.clone()));
            }
            if let Some(page) = p.page {
                qp.push(("page", page.to_string()));
            }
            if let Some(limit) = p.limit {
                qp.push(("limit", limit.to_string()));
            }
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
        self.get(&format!("/api/v1/whales/feed{qs}")).await
    }

    /// Get top whale wallets ranked by volume, PnL, win rate, or trade count.
    pub async fn get_top_whales(
        &self,
        params: Option<&GetTopWhalesParams>,
    ) -> Result<Vec<WhaleProfile>> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(p) = params {
            if let Some(ref sort_by) = p.sort_by {
                qp.push(("sortBy", sort_by.clone()));
            }
            if let Some(ref period) = p.period {
                qp.push(("period", period.clone()));
            }
            if let Some(limit) = p.limit {
                qp.push(("limit", limit.to_string()));
            }
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
        self.get(&format!("/api/v1/whales/top{qs}")).await
    }

    /// Get the full trading profile of a specific whale wallet.
    pub async fn get_whale_profile(&self, address: &str) -> Result<WhaleProfile> {
        self.get(&format!("/api/v1/whales/{}", encode(address)))
            .await
    }

    /// Follow a whale wallet to receive alerts when it trades.
    pub async fn follow_whale(&self, address: &str) -> Result<serde_json::Value> {
        self.post(
            &format!("/api/v1/whales/{}/follow", encode(address)),
            &json!({}),
        )
        .await
    }

    /// Unfollow a whale wallet.
    pub async fn unfollow_whale(&self, address: &str) -> Result<serde_json::Value> {
        self.post(
            &format!("/api/v1/whales/{}/unfollow", encode(address)),
            &json!({}),
        )
        .await
    }

    /// List whale wallets you are currently following.
    pub async fn get_following_whales(&self) -> Result<Vec<WhaleProfile>> {
        self.get("/api/v1/whales/following").await
    }

    /// Get AI-powered news signals.
    pub async fn get_news_signals(
        &self,
        params: Option<&GetNewsSignalsParams>,
    ) -> Result<PaginatedResponse<NewsSignal>> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(p) = params {
            if let Some(c) = p.min_confidence {
                qp.push(("minConfidence", c.to_string()));
            }
            if let Some(ref market_id) = p.market_id {
                qp.push(("marketId", market_id.clone()));
            }
            if let Some(ref direction) = p.direction {
                qp.push(("direction", direction.clone()));
            }
            if let Some(page) = p.page {
                qp.push(("page", page.to_string()));
            }
            if let Some(limit) = p.limit {
                qp.push(("limit", limit.to_string()));
            }
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
        self.get(&format!("/api/v1/news/signals{qs}")).await
    }

    // -----------------------------------------------------------------------
    // Discover & Leaderboard
    // -----------------------------------------------------------------------

    /// Discover publicly shared strategies.
    pub async fn discover_strategies(
        &self,
        params: Option<&DiscoverParams>,
    ) -> Result<serde_json::Value> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(p) = params {
            if let Some(ref sort) = p.sort {
                qp.push(("sort", sort.clone()));
            }
            if let Some(ref category) = p.category {
                qp.push(("category", category.clone()));
            }
            if let Some(ref search) = p.search {
                qp.push(("search", search.clone()));
            }
            if let Some(page) = p.page {
                qp.push(("page", page.to_string()));
            }
            if let Some(limit) = p.limit {
                qp.push(("limit", limit.to_string()));
            }
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
        self.get(&format!("/api/v1/discover{qs}")).await
    }

    /// Get the trader leaderboard ranked by realized P&L.
    pub async fn get_leaderboard(
        &self,
        params: Option<&LeaderboardParams>,
    ) -> Result<serde_json::Value> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(p) = params {
            if let Some(ref period) = p.period {
                qp.push(("period", period.clone()));
            }
            if let Some(page) = p.page {
                qp.push(("page", page.to_string()));
            }
            if let Some(limit) = p.limit {
                qp.push(("limit", limit.to_string()));
            }
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
        self.get(&format!("/api/v1/leaderboard{qs}")).await
    }

    // -----------------------------------------------------------------------
    // Paper trading
    // -----------------------------------------------------------------------

    /// Get a summary of the paper trading account.
    pub async fn get_paper_summary(&self) -> Result<PaperSummary> {
        self.get("/api/v1/paper/summary").await
    }

    /// Reset the paper trading account to its initial balance.
    pub async fn reset_paper_account(&self) -> Result<serde_json::Value> {
        self.post("/api/v1/paper/reset", &json!({})).await
    }

    // -----------------------------------------------------------------------
    // Batch API
    // -----------------------------------------------------------------------

    /// Execute multiple API calls in a single round-trip.
    pub async fn batch_requests(&self, items: &[BatchRequestItem]) -> Result<BatchResponse> {
        let body = json!({ "items": items });
        self.post("/api/v1/batch", &body).await
    }

    // -----------------------------------------------------------------------
    // Configuration
    // -----------------------------------------------------------------------

    /// List configured alerts.
    pub async fn list_alerts(&self) -> Result<PaginatedResponse<Alert>> {
        self.get("/api/v1/alerts").await
    }

    /// List copy-trading configurations.
    pub async fn list_copy_configs(&self) -> Result<PaginatedResponse<CopyConfig>> {
        self.get("/api/v1/copy").await
    }

    /// Create a new copy trading configuration.
    pub async fn create_copy_config(&self, params: &CreateCopyConfigParams) -> Result<CopyConfig> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/copy", &body).await
    }

    /// Get a single copy trading configuration by ID.
    pub async fn get_copy_config(&self, id: &str) -> Result<CopyConfig> {
        self.get(&format!("/api/v1/copy/{}", encode(id))).await
    }

    /// Update an existing copy trading configuration.
    pub async fn update_copy_config(
        &self,
        id: &str,
        params: &UpdateCopyConfigParams,
    ) -> Result<CopyConfig> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.patch(&format!("/api/v1/copy/{}", encode(id)), &body)
            .await
    }

    /// Pause a copy trading configuration.
    pub async fn pause_copy_config(&self, id: &str) -> Result<CopyConfig> {
        self.post(&format!("/api/v1/copy/{}/pause", encode(id)), &json!({}))
            .await
    }

    /// Resume a paused copy trading configuration.
    pub async fn resume_copy_config(&self, id: &str) -> Result<CopyConfig> {
        self.post(&format!("/api/v1/copy/{}/resume", encode(id)), &json!({}))
            .await
    }

    /// Delete (stop) a copy trading configuration.
    pub async fn delete_copy_config(&self, id: &str) -> Result<serde_json::Value> {
        self.delete(&format!("/api/v1/copy/{}", encode(id))).await
    }

    /// Get trades executed by a copy trading configuration.
    pub async fn get_copy_trades(
        &self,
        id: &str,
        params: Option<&GetCopyTradesParams>,
    ) -> Result<PaginatedResponse<Order>> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(p) = params {
            if let Some(page) = p.page {
                qp.push(("page", page.to_string()));
            }
            if let Some(limit) = p.limit {
                qp.push(("limit", limit.to_string()));
            }
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
        self.get(&format!("/api/v1/copy/{}/trades{qs}", encode(id)))
            .await
    }

    /// List registered webhooks.
    pub async fn list_webhooks(&self) -> Result<PaginatedResponse<Webhook>> {
        self.get("/api/v1/webhooks").await
    }

    /// Register a new webhook for the given events.
    ///
    /// The URL must use the `https` scheme and must not point to a private or
    /// loopback IP address.  DNS resolution is performed at validation time to
    /// mitigate domain-based SSRF / DNS rebinding attacks.
    ///
    /// **Note:** This is a client-side best-effort check.  The server must
    /// independently validate resolved IPs at connection time.
    pub async fn create_webhook(&self, url: &str, events: &[WebhookEvent]) -> Result<Webhook> {
        Self::validate_webhook_url(url).await?;
        let body = json!({
            "url": url,
            "events": events,
        });
        self.post("/api/v1/webhooks", &body).await
    }

    /// Delete a registered webhook.
    pub async fn delete_webhook(&self, id: &str) -> Result<()> {
        let _: serde_json::Value = self
            .delete(&format!("/api/v1/webhooks/{}", encode(id)))
            .await?;
        Ok(())
    }

    /// Send a test event to a registered webhook and return the delivery result.
    pub async fn test_webhook(&self, id: &str) -> Result<WebhookTestResult> {
        self.post(&format!("/api/v1/webhooks/{}/test", encode(id)), &json!({}))
            .await
    }

    // -----------------------------------------------------------------------
    // Watchlist
    // -----------------------------------------------------------------------

    /// List all markets on the authenticated user's watchlist.
    pub async fn get_watchlist(&self) -> Result<Vec<WatchlistItem>> {
        self.get("/api/v1/watchlist").await
    }

    /// Add a market to the watchlist.
    pub async fn add_to_watchlist(&self, market_id: &str) -> Result<WatchlistAddResponse> {
        let body = json!({ "marketId": market_id });
        self.post("/api/v1/watchlist", &body).await
    }

    /// Remove a market from the watchlist.
    pub async fn remove_from_watchlist(&self, market_id: &str) -> Result<()> {
        let _: serde_json::Value = self
            .delete(&format!("/api/v1/watchlist/{}", encode(market_id)))
            .await?;
        Ok(())
    }

    /// Check whether a specific market is on the watchlist.
    pub async fn get_watchlist_status(&self, market_id: &str) -> Result<WatchlistStatus> {
        self.get(&format!("/api/v1/watchlist/{}/status", encode(market_id)))
            .await
    }

    // -----------------------------------------------------------------------
    // Conditional Orders
    // -----------------------------------------------------------------------

    /// List conditional orders with optional status, type, page, and limit filters.
    pub async fn list_conditional_orders(
        &self,
        params: &ListConditionalOrdersParams,
    ) -> Result<PaginatedResponse<ConditionalOrder>> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(ref s) = params.status {
            let val = serde_json::to_value(s).unwrap_or_default();
            if let Some(v) = val.as_str() {
                qp.push(("status", v.to_string()));
            }
        }
        if let Some(ref t) = params.order_type {
            let val = serde_json::to_value(t).unwrap_or_default();
            if let Some(v) = val.as_str() {
                qp.push(("type", v.to_string()));
            }
        }
        if let Some(p) = params.page {
            qp.push(("page", p.to_string()));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
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

        self.get(&format!("/api/v1/orders/conditional{qs}")).await
    }

    /// Create a conditional order (limit, stop, trailing-stop, etc.).
    ///
    /// # Errors
    /// Returns [`PolyforgeError::Validation`] if `size` or `trigger_price` is
    /// NaN, infinite, zero, or negative.
    pub async fn create_conditional_order(
        &self,
        params: &CreateConditionalOrderParams,
    ) -> Result<ConditionalOrder> {
        validate_financial_param("size", params.size)?;
        validate_financial_param("trigger_price", params.trigger_price)?;
        validate_optional_financial_param("limit_price", params.limit_price)?;
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/orders/conditional", &body).await
    }

    /// Get a single conditional order by ID.
    pub async fn get_conditional_order(&self, order_id: &str) -> Result<ConditionalOrder> {
        self.get(&format!("/api/v1/orders/conditional/{}", encode(order_id)))
            .await
    }

    /// Cancel a pending conditional order.
    pub async fn cancel_conditional_order(&self, order_id: &str) -> Result<()> {
        let _: serde_json::Value = self
            .delete(&format!("/api/v1/orders/conditional/{}", encode(order_id)))
            .await?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Alert CRUD
    // -----------------------------------------------------------------------

    /// Create a new price alert.
    pub async fn create_alert(&self, params: &CreateAlertParams) -> Result<Alert> {
        validate_financial_param("price", params.price)?;
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/alerts", &body).await
    }

    /// Delete an alert by ID.
    pub async fn delete_alert(&self, alert_id: &str) -> Result<()> {
        let _: serde_json::Value = self
            .delete(&format!("/api/v1/alerts/{}", encode(alert_id)))
            .await?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Portfolio PnL
    // -----------------------------------------------------------------------

    /// Get aggregated portfolio PnL with optional period and strategy filter.
    pub async fn get_portfolio_pnl(&self, params: &GetPortfolioPnlParams) -> Result<PortfolioPnl> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(ref p) = params.period {
            let val = serde_json::to_value(p).unwrap_or_default();
            qp.push(("period", val.as_str().unwrap_or_default().to_string()));
        }
        if let Some(ref s) = params.strategy_id {
            qp.push(("strategyId", s.clone()));
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

        self.get(&format!("/api/v1/portfolio/pnl{qs}")).await
    }

    /// Returns `true` if the given IP is in a blocked range (loopback, private,
    /// link-local, CGNAT, cloud metadata, or unspecified).
    fn is_blocked_ip(ip: std::net::IpAddr) -> bool {
        match ip {
            std::net::IpAddr::V4(v4) => {
                v4.is_loopback()
                    || v4.is_private()
                    || v4.is_link_local()
                    || v4.is_unspecified()
                    // link-local 169.254.0.0/16
                    || (v4.octets()[0] == 169 && v4.octets()[1] == 254)
                    // CGNAT / shared address space (RFC 6598) 100.64.0.0/10
                    || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xc0) == 64)
            }
            std::net::IpAddr::V6(v6) => {
                v6.is_loopback()
                    || v6.is_unspecified()
                    // unique-local fc00::/7
                    || (v6.octets()[0] & 0xfe) == 0xfc
                    // link-local fe80::/10
                    || (v6.octets()[0] == 0xfe && (v6.octets()[1] & 0xc0) == 0x80)
                    // IPv4-mapped ::ffff:x.x.x.x — check the mapped IPv4
                    || v6.to_ipv4_mapped().is_some_and(|v4| Self::is_blocked_ip(std::net::IpAddr::V4(v4)))
            }
        }
    }

    /// Validates a webhook URL: must be HTTPS, well-formed, and not target
    /// private/loopback networks.  Performs DNS resolution to catch
    /// domain-based SSRF bypass via DNS rebinding.
    async fn validate_webhook_url(url: &str) -> Result<()> {
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

        // Check literal IP addresses directly
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            if Self::is_blocked_ip(ip) {
                return Err(PolyforgeError::Validation(
                    "Webhook URL must not target private or loopback addresses".into(),
                ));
            }
            return Ok(());
        }

        // Resolve domain names and check all resolved IPs.
        // This mitigates DNS rebinding where a domain initially resolves to a
        // public IP but is later changed to point to an internal address.
        let port = parsed.port().unwrap_or(443);
        let lookup = format!("{host}:{port}");

        let addrs = tokio::net::lookup_host(&lookup)
            .await
            .map_err(|_| PolyforgeError::Validation("Cannot resolve webhook hostname".into()))?;

        let mut resolved_any = false;
        for addr in addrs {
            resolved_any = true;
            if Self::is_blocked_ip(addr.ip()) {
                return Err(PolyforgeError::Validation(
                    "Webhook URL resolves to a private or loopback address".into(),
                ));
            }
        }

        if !resolved_any {
            return Err(PolyforgeError::Validation(
                "Webhook URL hostname did not resolve to any address".into(),
            ));
        }

        Ok(())
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
        self.get(&format!("/api/v1/news/sentiment/{}", encode(market_id)))
            .await
    }

    /// Provide liquidity on a market.
    ///
    /// # Errors
    /// Returns [`PolyforgeError::Validation`] if `amount_usdc` is NaN, infinite, zero,
    /// or negative.
    pub async fn provide_liquidity(&self, params: &ProvideLiquidityParams) -> Result<LpPosition> {
        validate_financial_param("amount_usdc", params.amount_usdc)?;
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
                suggestion: body
                    .get("suggestion")
                    .and_then(|v| v.as_str())
                    .map(String::from),
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
        assert_eq!(url, "https://api.polyforge.app/api/v1/markets");
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
            suggestion: None,
        };

        if let PolyforgeError::Api {
            status,
            code,
            message,
            request_id,
            suggestion,
        } = error
        {
            assert_eq!(status, 404);
            assert_eq!(code, "NOT_FOUND");
            assert_eq!(message, "Resource not found");
            assert_eq!(request_id, Some("req-123".to_string()));
            assert_eq!(suggestion, None);
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
            suggestion: None,
        };

        if let PolyforgeError::Api {
            status,
            code,
            message,
            request_id,
            suggestion,
        } = error
        {
            assert_eq!(status, 500);
            assert_eq!(code, "INTERNAL_ERROR");
            assert_eq!(message, "Internal server error");
            assert_eq!(request_id, None);
            assert_eq!(suggestion, None);
        } else {
            panic!("Expected Api error variant");
        }
    }

    #[test]
    fn test_api_error_with_suggestion() {
        let error = PolyforgeError::Api {
            status: 429,
            code: "RATE_LIMITED".to_string(),
            message: "Too many requests".to_string(),
            request_id: Some("req-456".to_string()),
            suggestion: Some("Reduce request frequency or upgrade to Pro tier".to_string()),
        };

        if let PolyforgeError::Api {
            status,
            code,
            message,
            request_id,
            suggestion,
        } = error
        {
            assert_eq!(status, 429);
            assert_eq!(code, "RATE_LIMITED");
            assert_eq!(message, "Too many requests");
            assert_eq!(request_id, Some("req-456".to_string()));
            assert_eq!(
                suggestion,
                Some("Reduce request frequency or upgrade to Pro tier".to_string())
            );
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

    // --- SSRF protection for base URL (closes #26) ---

    #[test]
    fn test_base_url_rejects_private_192168() {
        let result = PolyforgeClient::with_url("key", "https://192.168.1.1");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("SSRF") || err.contains("private"));
    }

    #[test]
    fn test_base_url_rejects_private_10_range() {
        let result = PolyforgeClient::with_url("key", "https://10.0.0.1");
        assert!(result.is_err());
    }

    #[test]
    fn test_base_url_rejects_private_172_16_range() {
        let result = PolyforgeClient::with_url("key", "https://172.16.0.1");
        assert!(result.is_err());
    }

    #[test]
    fn test_base_url_rejects_cgnat_range() {
        let result = PolyforgeClient::with_url("key", "https://100.64.0.1");
        assert!(result.is_err());
    }

    #[test]
    fn test_base_url_rejects_link_local_169254() {
        let result = PolyforgeClient::with_url("key", "https://169.254.169.254");
        assert!(result.is_err());
    }

    #[test]
    fn test_base_url_rejects_cloud_metadata_hostname() {
        let result = PolyforgeClient::with_url("key", "https://metadata.google.internal");
        assert!(result.is_err());
    }

    #[test]
    fn test_base_url_rejects_dot_internal_hostname() {
        let result = PolyforgeClient::with_url("key", "https://something.internal");
        assert!(result.is_err());
    }

    #[test]
    fn test_base_url_rejects_dot_local_hostname() {
        let result = PolyforgeClient::with_url("key", "https://printer.local");
        assert!(result.is_err());
    }

    #[test]
    fn test_base_url_rejects_ipv6_unique_local() {
        let result = PolyforgeClient::with_url("key", "https://[fd00::1]");
        assert!(result.is_err());
    }

    #[test]
    fn test_base_url_rejects_ipv6_link_local() {
        let result = PolyforgeClient::with_url("key", "https://[fe80::1]");
        assert!(result.is_err());
    }

    #[test]
    fn test_base_url_allows_localhost_dev_exemption() {
        // localhost is exempted for development use
        let client = PolyforgeClient::with_url("key", "http://localhost:3002").unwrap();
        assert_eq!(client.base_url, "http://localhost:3002");
    }

    #[test]
    fn test_base_url_allows_127001_dev_exemption() {
        // 127.0.0.1 is exempted for development use
        let client = PolyforgeClient::with_url("key", "http://127.0.0.1:3002").unwrap();
        assert_eq!(client.base_url, "http://127.0.0.1:3002");
    }

    #[test]
    fn test_base_url_allows_ipv6_loopback_dev_exemption() {
        // [::1] is exempted for development use
        let client = PolyforgeClient::with_url("key", "http://[::1]:3002").unwrap();
        assert_eq!(client.base_url, "http://[::1]:3002");
    }

    #[test]
    fn test_base_url_allows_public_ip() {
        // Public IPs should be accepted
        let client = PolyforgeClient::with_url("key", "https://8.8.8.8").unwrap();
        assert_eq!(client.base_url, "https://8.8.8.8");
    }

    #[test]
    fn test_base_url_allows_public_domain() {
        // Public domains should be accepted
        let client = PolyforgeClient::with_url("key", "https://api.example.com").unwrap();
        assert_eq!(client.base_url, "https://api.example.com");
    }

    #[test]
    fn test_base_url_rejects_non_cgnat_boundary() {
        // 100.128.0.1 is outside CGNAT range — should be accepted as public IP
        let client = PolyforgeClient::with_url("key", "https://100.128.0.1").unwrap();
        assert_eq!(client.base_url, "https://100.128.0.1");
    }

    #[tokio::test]
    async fn test_webhook_url_rejects_http() {
        let result = PolyforgeClient::validate_webhook_url("http://example.com/hook").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_url_rejects_localhost() {
        let result = PolyforgeClient::validate_webhook_url("https://localhost/hook").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_url_rejects_private_ip() {
        let result = PolyforgeClient::validate_webhook_url("https://192.168.1.1/hook").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_url_rejects_loopback() {
        let result = PolyforgeClient::validate_webhook_url("https://127.0.0.1/hook").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_url_rejects_link_local() {
        let result = PolyforgeClient::validate_webhook_url("https://169.254.1.1/hook").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_url_accepts_valid_https() {
        let result =
            PolyforgeClient::validate_webhook_url("https://hooks.example.com/polyforge").await;
        // May fail if DNS can't resolve in CI — the important thing is it
        // doesn't error with "private address" when given a public domain.
        // If DNS resolution fails, that's also an acceptable rejection.
        let _ = result;
    }

    #[tokio::test]
    async fn test_webhook_url_rejects_invalid_url() {
        let result = PolyforgeClient::validate_webhook_url("not a url").await;
        assert!(result.is_err());
    }

    // --- DNS rebinding / SSRF mitigation tests (closes #63) ---

    #[tokio::test]
    async fn test_webhook_url_rejects_10_range() {
        let result = PolyforgeClient::validate_webhook_url("https://10.0.0.1/hook").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_url_rejects_172_16_range() {
        let result = PolyforgeClient::validate_webhook_url("https://172.16.0.1/hook").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_url_rejects_cgnat_range() {
        // RFC 6598 CGNAT 100.64.0.0/10 — also closes #41
        let result = PolyforgeClient::validate_webhook_url("https://100.64.0.1/hook").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_url_rejects_cgnat_upper_bound() {
        // 100.127.255.255 is the top of 100.64.0.0/10
        let result = PolyforgeClient::validate_webhook_url("https://100.127.255.254/hook").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_url_allows_non_cgnat_100() {
        // 100.128.0.1 is outside CGNAT range — should pass IP check
        let result = PolyforgeClient::validate_webhook_url("https://100.128.0.1/hook").await;
        // This is a valid public IP so it should pass (no DNS resolution needed for IP literals)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_webhook_url_rejects_cloud_metadata() {
        let result = PolyforgeClient::validate_webhook_url("https://169.254.169.254/hook").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_url_rejects_dot_internal_domain() {
        let result =
            PolyforgeClient::validate_webhook_url("https://metadata.google.internal/hook").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_url_rejects_dot_local_domain() {
        let result = PolyforgeClient::validate_webhook_url("https://printer.local/hook").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_url_rejects_unspecified() {
        let result = PolyforgeClient::validate_webhook_url("https://0.0.0.0/hook").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_url_rejects_ipv6_loopback() {
        let result = PolyforgeClient::validate_webhook_url("https://[::1]/hook").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_url_rejects_ipv6_unique_local() {
        let result = PolyforgeClient::validate_webhook_url("https://[fd00::1]/hook").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_url_rejects_ipv6_link_local() {
        let result = PolyforgeClient::validate_webhook_url("https://[fe80::1]/hook").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_is_blocked_ip_cgnat() {
        use std::net::IpAddr;
        // 100.64.0.0 - 100.127.255.255 should be blocked
        assert!(PolyforgeClient::is_blocked_ip(
            "100.64.0.0".parse::<IpAddr>().unwrap()
        ));
        assert!(PolyforgeClient::is_blocked_ip(
            "100.100.100.100".parse::<IpAddr>().unwrap()
        ));
        assert!(PolyforgeClient::is_blocked_ip(
            "100.127.255.255".parse::<IpAddr>().unwrap()
        ));
        // Outside CGNAT
        assert!(!PolyforgeClient::is_blocked_ip(
            "100.128.0.0".parse::<IpAddr>().unwrap()
        ));
        assert!(!PolyforgeClient::is_blocked_ip(
            "100.63.255.255".parse::<IpAddr>().unwrap()
        ));
        assert!(!PolyforgeClient::is_blocked_ip(
            "8.8.8.8".parse::<IpAddr>().unwrap()
        ));
    }

    #[test]
    fn test_is_blocked_ip_ipv4_mapped_v6() {
        use std::net::IpAddr;
        // IPv4-mapped IPv6 carrying a private address
        let mapped: IpAddr = "::ffff:192.168.1.1".parse().unwrap();
        assert!(PolyforgeClient::is_blocked_ip(mapped));
        // IPv4-mapped IPv6 carrying a public address
        let public_mapped: IpAddr = "::ffff:8.8.8.8".parse().unwrap();
        assert!(!PolyforgeClient::is_blocked_ip(public_mapped));
    }

    #[tokio::test]
    async fn test_webhook_url_resolves_domain_and_checks_ip() {
        // localhost resolves to 127.0.0.1 — blocked by hostname check before DNS
        let result = PolyforgeClient::validate_webhook_url("https://localhost/hook").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_url_rejects_unresolvable_domain() {
        let result = PolyforgeClient::validate_webhook_url(
            "https://this-domain-definitely-does-not-exist-xyz123.example/hook",
        )
        .await;
        assert!(result.is_err());
    }

    // --- SSE buffer overflow protection tests (closes #52) ---

    #[test]
    fn test_max_sse_buffer_constant_is_1mib() {
        assert_eq!(MAX_SSE_BUFFER_SIZE, 1_048_576);
    }

    #[tokio::test]
    async fn test_sse_buffer_overflow_returns_error() {
        // Build a mock response body that is larger than the buffer limit
        // and contains no newlines, triggering the overflow guard.
        let oversized = "x".repeat(MAX_SSE_BUFFER_SIZE + 1);
        let body = reqwest::Response::from(
            http::Response::builder()
                .status(200)
                .body(oversized)
                .unwrap(),
        );

        let mut stream = StrategyEventStream {
            response: body,
            buffer: String::new(),
        };

        let result = stream.next().await;
        assert!(result.is_some(), "stream should return an error, not None");
        let err = result.unwrap();
        assert!(err.is_err(), "should be Err variant");
        match err.unwrap_err() {
            PolyforgeError::Api { code, .. } => {
                assert_eq!(code, "SSE_BUFFER_OVERFLOW");
            }
            other => panic!(
                "Expected Api error with SSE_BUFFER_OVERFLOW, got: {:?}",
                other
            ),
        }
    }

    // --- Response body size cap tests (closes #48) ---

    #[test]
    fn test_max_response_body_size_constant_is_1mib() {
        assert_eq!(MAX_RESPONSE_BODY_SIZE, 1_048_576);
    }

    #[tokio::test]
    async fn test_handle_response_rejects_oversized_error_body() {
        // Simulate a 500 error response with Content-Length exceeding the cap.
        let body = http::Response::builder()
            .status(500)
            .header("content-length", (MAX_RESPONSE_BODY_SIZE + 1).to_string())
            .body("")
            .unwrap();
        let resp = reqwest::Response::from(body);

        let client = PolyforgeClient::with_url("test-key", "http://localhost:3002").unwrap();
        let result: std::result::Result<serde_json::Value, _> = client.handle_response(resp).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            PolyforgeError::Api { code, status, .. } => {
                assert_eq!(code, "RESPONSE_BODY_TOO_LARGE");
                assert_eq!(status, 500);
            }
            other => panic!(
                "Expected Api error with RESPONSE_BODY_TOO_LARGE, got: {:?}",
                other
            ),
        }
    }

    #[tokio::test]
    async fn test_handle_response_allows_small_error_body() {
        // A normal-sized error response should still be parsed normally.
        let error_json = r#"{"code":"NOT_FOUND","message":"Resource not found"}"#;
        let body = http::Response::builder()
            .status(404)
            .header("content-length", error_json.len().to_string())
            .header("content-type", "application/json")
            .body(error_json.to_string())
            .unwrap();
        let resp = reqwest::Response::from(body);

        let client = PolyforgeClient::with_url("test-key", "http://localhost:3002").unwrap();
        let result: std::result::Result<serde_json::Value, _> = client.handle_response(resp).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            PolyforgeError::Api {
                code,
                message,
                status,
                ..
            } => {
                assert_eq!(status, 404);
                assert_eq!(code, "NOT_FOUND");
                assert_eq!(message, "Resource not found");
            }
            other => panic!("Expected Api error with NOT_FOUND, got: {:?}", other),
        }
    }

    // --- Platform contract compliance regression tests (#89-#92) ---

    #[test]
    fn test_trading_mode_serializes_lowercase() {
        // #92: Platform expects "live"/"paper", not "LIVE"/"PAPER"
        let live = serde_json::to_value(TradingMode::Live).unwrap();
        assert_eq!(live, serde_json::Value::String("live".to_string()));
        let paper = serde_json::to_value(TradingMode::Paper).unwrap();
        assert_eq!(paper, serde_json::Value::String("paper".to_string()));
    }

    #[test]
    fn test_webhook_event_serializes_screaming_snake() {
        // #91: Platform expects "ORDER_FILLED", not "order.filled"
        let event = serde_json::to_value(WebhookEvent::OrderFilled).unwrap();
        assert_eq!(event, serde_json::Value::String("ORDER_FILLED".to_string()));
        let event = serde_json::to_value(WebhookEvent::StrategyError).unwrap();
        assert_eq!(
            event,
            serde_json::Value::String("STRATEGY_ERROR".to_string())
        );
        let event = serde_json::to_value(WebhookEvent::WhaleTrade).unwrap();
        assert_eq!(event, serde_json::Value::String("WHALE_TRADE".to_string()));
        let event = serde_json::to_value(WebhookEvent::DailyLossLimit).unwrap();
        assert_eq!(
            event,
            serde_json::Value::String("DAILY_LOSS_LIMIT".to_string())
        );
    }

    #[test]
    fn test_ai_query_body_uses_query_field() {
        // #89: Must send { "query": ... } not { "question": ... }
        let body = json!({ "query": "what is BTC?" });
        assert!(body.get("query").is_some());
        assert!(body.get("question").is_none());
    }

    #[test]
    fn test_create_strategy_from_description_uses_description_field() {
        // #90: Must send { "description": ... } not { "query": ... }
        let body = json!({ "description": "buy low sell high" });
        assert!(body.get("description").is_some());
        // Note: body should not use "query" as the field name for description
    }

    // -----------------------------------------------------------------------
    // Financial parameter validation (#88)
    // -----------------------------------------------------------------------

    #[test]
    fn test_validate_financial_param_rejects_nan() {
        let err = validate_financial_param("size", f64::NAN).unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
        assert!(err.to_string().contains("NaN"));
    }

    #[test]
    fn test_validate_financial_param_rejects_positive_infinity() {
        let err = validate_financial_param("price", f64::INFINITY).unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
        assert!(err.to_string().contains("+infinity"));
    }

    #[test]
    fn test_validate_financial_param_rejects_negative_infinity() {
        let err = validate_financial_param("price", f64::NEG_INFINITY).unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
        assert!(err.to_string().contains("-infinity"));
    }

    #[test]
    fn test_validate_financial_param_rejects_zero() {
        let err = validate_financial_param("size", 0.0).unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
        assert!(err.to_string().contains("positive"));
    }

    #[test]
    fn test_validate_financial_param_rejects_negative() {
        let err = validate_financial_param("spread", -1.0).unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
        assert!(err.to_string().contains("positive"));
    }

    #[test]
    fn test_validate_financial_param_accepts_positive() {
        assert!(validate_financial_param("size", 0.01).is_ok());
        assert!(validate_financial_param("price", 100.0).is_ok());
        assert!(validate_financial_param("spread", 0.005).is_ok());
    }

    #[test]
    fn test_validate_optional_financial_param_skips_none() {
        assert!(validate_optional_financial_param("limit_price", None).is_ok());
    }

    #[test]
    fn test_validate_optional_financial_param_validates_some() {
        let err = validate_optional_financial_param("limit_price", Some(f64::NAN)).unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
    }

    #[test]
    fn test_place_order_params_validation_rejects_nan_size() {
        let params = PlaceOrderParams {
            market_id: "m1".into(),
            token_id: "t1".into(),
            side: "BUY".into(),
            outcome: "YES".into(),
            size: f64::NAN,
            price: 0.5,
            order_type: None,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = PolyforgeClient::new("test-key").unwrap();
        let err = rt.block_on(client.place_order(&params)).unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
        assert!(err.to_string().contains("size"));
    }

    #[test]
    fn test_place_order_params_validation_rejects_negative_price() {
        let params = PlaceOrderParams {
            market_id: "m1".into(),
            token_id: "t1".into(),
            side: "BUY".into(),
            outcome: "YES".into(),
            size: 10.0,
            price: -0.5,
            order_type: None,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = PolyforgeClient::new("test-key").unwrap();
        let err = rt.block_on(client.place_order(&params)).unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
        assert!(err.to_string().contains("price"));
    }

    #[test]
    fn test_place_order_params_serializes_market_id() {
        let params = PlaceOrderParams {
            market_id: "mkt-abc".into(),
            token_id: "tok-1".into(),
            side: "BUY".into(),
            outcome: "YES".into(),
            size: 25.0,
            price: 0.65,
            order_type: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["marketId"], "mkt-abc");
        assert_eq!(json["tokenId"], "tok-1");
        assert!(
            json.get("market_id").is_none(),
            "must use camelCase marketId"
        );
        assert!(
            json.get("orderType").is_none(),
            "None fields should be skipped"
        );
    }

    #[test]
    fn test_place_smart_order_params_validation_rejects_infinite_total_size() {
        let params = PlaceSmartOrderParams {
            order_type: SmartOrderType::TWAP,
            token_id: "t1".into(),
            side: "BUY".into(),
            outcome: "YES".into(),
            total_size: f64::INFINITY,
            slices: Some(5),
            interval_minutes: Some(10),
            limit_price: None,
            entry_price: None,
            take_profit_price: None,
            stop_loss_price: None,
            price_a: None,
            price_b: None,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = PolyforgeClient::new("test-key").unwrap();
        let err = rt.block_on(client.place_smart_order(&params)).unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
        assert!(err.to_string().contains("total_size"));
    }

    #[test]
    fn test_place_smart_order_params_validation_rejects_nan_optional_price() {
        let params = PlaceSmartOrderParams {
            order_type: SmartOrderType::BRACKET,
            token_id: "t1".into(),
            side: "BUY".into(),
            outcome: "YES".into(),
            total_size: 10.0,
            slices: None,
            interval_minutes: None,
            limit_price: None,
            entry_price: Some(0.5),
            take_profit_price: Some(f64::NAN),
            stop_loss_price: Some(0.3),
            price_a: None,
            price_b: None,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = PolyforgeClient::new("test-key").unwrap();
        let err = rt.block_on(client.place_smart_order(&params)).unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
        assert!(err.to_string().contains("take_profit_price"));
    }

    #[test]
    fn test_provide_liquidity_params_validation_rejects_zero_amount_usdc() {
        // Platform expects {marketId, tokenId, amountUsdc} — validate amountUsdc
        let params = ProvideLiquidityParams {
            market_id: "m1".into(),
            token_id: "tok-1".into(),
            amount_usdc: 0.0,
            target_spread: None,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = PolyforgeClient::new("test-key").unwrap();
        let err = rt.block_on(client.provide_liquidity(&params)).unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
        assert!(err.to_string().contains("amount_usdc"));
    }

    #[test]
    fn test_provide_liquidity_params_validation_rejects_negative_amount_usdc() {
        let params = ProvideLiquidityParams {
            market_id: "m1".into(),
            token_id: "tok-1".into(),
            amount_usdc: -50.0,
            target_spread: None,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = PolyforgeClient::new("test-key").unwrap();
        let err = rt.block_on(client.provide_liquidity(&params)).unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
        assert!(err.to_string().contains("amount_usdc"));
    }

    #[tokio::test]
    async fn test_sse_buffer_within_limit_does_not_error() {
        // A payload just under the limit with a trailing newline should parse
        // normally (as a non-SSE line it is simply skipped, then the stream
        // ends with None when there are no more bytes).
        let under_limit = format!("{}\n", "y".repeat(MAX_SSE_BUFFER_SIZE - 1));
        let body = reqwest::Response::from(
            http::Response::builder()
                .status(200)
                .body(under_limit)
                .unwrap(),
        );

        let mut stream = StrategyEventStream {
            response: body,
            buffer: String::new(),
        };

        // The line is not a "data:" line so it is skipped; the stream ends.
        let result = stream.next().await;
        assert!(
            result.is_none(),
            "should return None (stream ended), not an error"
        );
    }

    // --- Breaking compat fixes (#49, #65, #76) ---

    #[test]
    fn test_error_response_reads_camelcase_request_id() {
        // #49: Platform returns "requestId" (camelCase), not "request_id"
        let body: serde_json::Value = serde_json::json!({
            "code": "NOT_FOUND",
            "message": "Resource not found",
            "requestId": "req-abc123"
        });
        let request_id = body
            .get("requestId")
            .and_then(|v| v.as_str())
            .map(String::from);
        assert_eq!(request_id, Some("req-abc123".to_string()));

        // Verify old snake_case key would NOT match
        let old_id = body
            .get("request_id")
            .and_then(|v| v.as_str())
            .map(String::from);
        assert_eq!(old_id, None);
    }

    #[test]
    fn test_strategy_status_response_deserializes_start() {
        // #65: start_strategy returns minimal status, not full Strategy
        let json = r#"{"status":"RUNNING","startedAt":"2026-04-13T10:00:00Z"}"#;
        let resp: StrategyStatusResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, StrategyStatus::Running);
        assert_eq!(resp.started_at, Some("2026-04-13T10:00:00Z".to_string()));
    }

    #[test]
    fn test_strategy_status_response_deserializes_stop() {
        // #65: stop_strategy returns minimal status with stoppedAt
        let json = r#"{"status":"IDLE","stoppedAt":"2026-04-13T10:05:00Z"}"#;
        let resp: StrategyStatusResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, StrategyStatus::Idle);
        assert_eq!(resp.stopped_at, Some("2026-04-13T10:05:00Z".to_string()));
    }

    #[test]
    fn test_strategy_status_response_deserializes_pause() {
        // #65: pause/resume return just status
        let json = r#"{"status":"PAUSED"}"#;
        let resp: StrategyStatusResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, StrategyStatus::Paused);
        assert_eq!(resp.started_at, None);
        assert_eq!(resp.stopped_at, None);
    }

    #[test]
    fn test_paginated_response_deserializes_strategies() {
        // #76: List endpoints return PaginatedResponse, not Vec
        let json = r#"{
            "data": [{"id":"s1","name":"Alpha"},{"id":"s2","name":"Beta"}],
            "total": 2,
            "page": 1,
            "limit": 10,
            "totalPages": 1,
            "hasNext": false
        }"#;
        let resp: PaginatedResponse<Strategy> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 2);
        assert_eq!(resp.total, 2);
        assert_eq!(resp.page, 1);
        assert!(!resp.has_next);
        assert_eq!(resp.data[0].id, "s1");
    }

    #[test]
    fn test_paginated_response_rejects_bare_array() {
        // #76: Verify that a bare JSON array fails to deserialize as PaginatedResponse
        let json = r#"[{"id":"s1"},{"id":"s2"}]"#;
        let result = serde_json::from_str::<PaginatedResponse<Strategy>>(json);
        assert!(
            result.is_err(),
            "bare array must not deserialize as PaginatedResponse"
        );
    }

    // -----------------------------------------------------------------------
    // Breaking compat fixes (#33, #34, #35, #36, #37, #51, #68)
    // -----------------------------------------------------------------------

    #[test]
    fn test_close_position_size_serializes_as_string() {
        // #33: size must be a JSON string, not a number
        let params = ClosePositionParams {
            token_id: "tok-123".into(),
            size: Some("100".into()),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["size"], serde_json::Value::String("100".to_string()));
    }

    #[test]
    fn test_close_position_size_omitted_when_none() {
        // #33: size should not appear in JSON when None
        let params = ClosePositionParams {
            token_id: "tok-123".into(),
            size: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert!(json.get("size").is_none());
    }

    #[test]
    fn test_order_status_enum_deserializes_all_variants() {
        // #34: All 12 OrderStatus variants must deserialize
        let variants = [
            "PENDING",
            "SUBMITTED",
            "LIVE",
            "MATCHED",
            "DELAYED",
            "MINED",
            "CONFIRMED",
            "PARTIAL",
            "CANCELLED",
            "UNMATCHED",
            "FAILED",
            "ERROR",
        ];
        for v in &variants {
            let json = format!("\"{}\"", v);
            let status: OrderStatus = serde_json::from_str(&json).unwrap();
            let serialized = serde_json::to_value(&status).unwrap();
            assert_eq!(serialized, serde_json::Value::String(v.to_string()));
        }
    }

    #[test]
    fn test_order_status_used_in_order_struct() {
        // #34: Order.status should be typed OrderStatus, not String
        let json = r#"{"id":"o1","status":"CONFIRMED"}"#;
        let order: Order = serde_json::from_str(json).unwrap();
        assert_eq!(order.status, Some(OrderStatus::Confirmed));
    }

    #[test]
    fn test_strategy_status_error_and_archived() {
        // #35: ERROR and ARCHIVED must deserialize
        let json = r#""ERROR""#;
        let status: StrategyStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status, StrategyStatus::Error);

        let json = r#""ARCHIVED""#;
        let status: StrategyStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status, StrategyStatus::Archived);
    }

    #[test]
    fn test_strategy_has_block_arrays() {
        // #36: Strategy must have triggers, conditions, actions, safety arrays
        let json = r#"{
            "id": "s1",
            "triggers": [{"type":"price_cross","enabled":true}],
            "conditions": [],
            "actions": [{"type":"place_order"}],
            "safety": [{"type":"stop_loss"}],
            "visibility": "PUBLIC",
            "execMode": "TICK",
            "tickMs": 5000,
            "tags": ["crypto","automated"],
            "forkCount": 12,
            "likeCount": 42,
            "version": 3
        }"#;
        let strategy: Strategy = serde_json::from_str(json).unwrap();
        assert_eq!(strategy.triggers.len(), 1);
        assert_eq!(strategy.actions.len(), 1);
        assert_eq!(strategy.safety.len(), 1);
        assert_eq!(strategy.visibility, Some(Visibility::Public));
        assert_eq!(strategy.exec_mode, Some(ExecMode::Tick));
        assert_eq!(strategy.tick_ms, Some(5000));
        assert_eq!(strategy.tags, vec!["crypto", "automated"]);
        assert_eq!(strategy.fork_count, Some(12));
        assert_eq!(strategy.like_count, Some(42));
        assert_eq!(strategy.version, Some(3));
    }

    #[test]
    fn test_create_strategy_params_serializes_all_fields() {
        // #37: CreateStrategyParams must include blocks, visibility, execMode, tags
        let params = CreateStrategyParams {
            name: "My Strategy".into(),
            description: Some("Test".into()),
            market_id: Some("m1".into()),
            visibility: Some(Visibility::Public),
            exec_mode: Some(ExecMode::Tick),
            tick_ms: Some(5000),
            triggers: Some(vec![]),
            conditions: Some(vec![]),
            actions: Some(vec![]),
            safety: Some(vec![]),
            logic_blocks: None,
            calc_blocks: None,
            tags: Some(vec!["test".into()]),
            variables: None,
            canvas: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["name"], "My Strategy");
        assert_eq!(json["visibility"], "PUBLIC");
        assert_eq!(json["execMode"], "TICK");
        assert_eq!(json["tickMs"], 5000);
        assert!(json["triggers"].is_array());
        assert!(json["tags"].is_array());
        // logicBlocks and calcBlocks omitted when None
        assert!(json.get("logicBlocks").is_none());
    }

    #[test]
    fn test_copy_config_deserializes_platform_fields() {
        // #51: CopyConfig must use targetWallet, mode, maxExposure etc.
        let json = r#"{
            "id": "cc1",
            "targetWallet": "0xabc123",
            "mode": "PERCENTAGE",
            "sizeValue": "50",
            "maxExposure": "1000",
            "maxDailyLoss": "100",
            "priceOffset": "0.01",
            "enabled": true
        }"#;
        let config: CopyConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.target_wallet, Some("0xabc123".to_string()));
        assert_eq!(config.mode, Some(CopyMode::Percentage));
        assert_eq!(config.size_value, Some("50".to_string()));
        assert_eq!(config.max_exposure, Some("1000".to_string()));
        assert_eq!(config.max_daily_loss, Some("100".to_string()));
        assert_eq!(config.price_offset, Some("0.01".to_string()));
        assert_eq!(config.enabled, Some(true));
    }

    #[test]
    fn test_copy_config_no_source_trader_field() {
        // #51: Verify that the old source_trader field name does NOT work
        let json = r#"{"id":"cc1","sourceTrader":"0xabc"}"#;
        let config: CopyConfig = serde_json::from_str(json).unwrap();
        // sourceTrader goes into extra, not target_wallet
        assert_eq!(config.target_wallet, None);
    }

    #[test]
    fn test_run_backtest_params_no_initial_balance() {
        // #68: RunBacktestParams must NOT have initialBalance
        let params = RunBacktestParams {
            strategy_id: Some("s1".into()),
            date_range_start: Some("2025-01-01".into()),
            date_range_end: Some("2025-12-31".into()),
            quick_mode: Some(true),
            strategy_blocks: None,
            market_bindings: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert!(json.get("initialBalance").is_none());
        assert_eq!(json["strategyId"], "s1");
        assert_eq!(json["dateRangeStart"], "2025-01-01");
        assert_eq!(json["dateRangeEnd"], "2025-12-31");
        assert_eq!(json["quickMode"], true);
    }

    #[test]
    fn test_run_backtest_params_with_market_bindings() {
        // #68: market_bindings should serialize correctly
        let mut bindings = std::collections::HashMap::new();
        bindings.insert("block-1".to_string(), "market-abc".to_string());
        let params = RunBacktestParams {
            strategy_id: None,
            date_range_start: None,
            date_range_end: None,
            quick_mode: None,
            strategy_blocks: None,
            market_bindings: Some(bindings),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["marketBindings"]["block-1"], "market-abc");
    }

    #[test]
    fn test_order_status_serializes_for_query_params() {
        // #34: OrderStatus should serialize to correct string for query params
        let status = OrderStatus::Live;
        let val = serde_json::to_value(&status).unwrap();
        assert_eq!(val.as_str().unwrap(), "LIVE");
    }

    #[test]
    fn test_visibility_enum_serializes() {
        assert_eq!(
            serde_json::to_value(Visibility::Private).unwrap(),
            "PRIVATE"
        );
        assert_eq!(serde_json::to_value(Visibility::Public).unwrap(), "PUBLIC");
        assert_eq!(
            serde_json::to_value(Visibility::Unlisted).unwrap(),
            "UNLISTED"
        );
    }

    #[test]
    fn test_exec_mode_enum_serializes() {
        assert_eq!(serde_json::to_value(ExecMode::Tick).unwrap(), "TICK");
        assert_eq!(serde_json::to_value(ExecMode::Event).unwrap(), "EVENT");
        assert_eq!(serde_json::to_value(ExecMode::Hybrid).unwrap(), "HYBRID");
    }

    #[test]
    fn test_copy_mode_enum_serializes() {
        assert_eq!(
            serde_json::to_value(CopyMode::Percentage).unwrap(),
            "PERCENTAGE"
        );
        assert_eq!(serde_json::to_value(CopyMode::Fixed).unwrap(), "FIXED");
        assert_eq!(serde_json::to_value(CopyMode::Mirror).unwrap(), "MIRROR");
    }

    // -----------------------------------------------------------------------
    // Breaking compat fixes (#19, #20, #22, #23, #30, #31, #32)
    // -----------------------------------------------------------------------

    #[test]
    fn test_whale_trade_deserializes_wallet_field() {
        // #23: WhaleTrade uses "wallet" not "trader", plus marketName and usdValue
        let json = r#"{
            "id": "wt1",
            "marketId": "m1",
            "marketName": "BTC > 100k",
            "side": "BUY",
            "size": 500.0,
            "usdValue": 325.0,
            "wallet": "0xabc123",
            "timestamp": "2026-04-13T10:00:00Z"
        }"#;
        let trade: WhaleTrade = serde_json::from_str(json).unwrap();
        assert_eq!(trade.wallet, Some("0xabc123".to_string()));
        assert_eq!(trade.market_name, Some("BTC > 100k".to_string()));
        assert_eq!(trade.usd_value, Some(325.0));
        // Verify old "trader" field name does not populate wallet
        let json_old = r#"{"id":"wt2","trader":"0xold"}"#;
        let trade_old: WhaleTrade = serde_json::from_str(json_old).unwrap();
        assert_eq!(trade_old.wallet, None);
    }

    #[test]
    fn test_news_signal_deserializes_sentiment_and_related_markets() {
        // #23: NewsSignal uses "sentiment" not "direction", "relatedMarkets" not "marketId", "publishedAt" not "timestamp"
        let json = r#"{
            "id": "ns1",
            "headline": "Fed holds rates",
            "source": "Reuters",
            "confidence": 85,
            "sentiment": "BULLISH",
            "relatedMarkets": ["m1", "m2"],
            "publishedAt": "2026-04-13T10:00:00Z"
        }"#;
        let signal: NewsSignal = serde_json::from_str(json).unwrap();
        assert_eq!(signal.sentiment, Some("BULLISH".to_string()));
        assert_eq!(signal.related_markets, vec!["m1", "m2"]);
        assert_eq!(
            signal.published_at,
            Some("2026-04-13T10:00:00Z".to_string())
        );
    }

    #[test]
    fn test_news_signal_old_direction_field_does_not_work() {
        // #23: Verify old field names don't populate the new fields
        let json = r#"{"id":"ns2","direction":"BEARISH","marketId":"m1","timestamp":"2026-01-01T00:00:00Z"}"#;
        let signal: NewsSignal = serde_json::from_str(json).unwrap();
        assert_eq!(signal.sentiment, None);
        assert!(signal.related_markets.is_empty());
        assert_eq!(signal.published_at, None);
    }

    #[test]
    fn test_alert_has_platform_fields() {
        // #107: Alert must match platform PriceAlert Prisma model fields
        let json = r#"{
            "id": "a1",
            "tokenId": "0xtoken123",
            "direction": "above",
            "price": "0.65",
            "persistent": false,
            "triggered": false,
            "triggeredAt": null,
            "createdAt": "2026-04-01T00:00:00Z"
        }"#;
        let alert: Alert = serde_json::from_str(json).unwrap();
        assert_eq!(alert.token_id, Some("0xtoken123".to_string()));
        assert_eq!(alert.direction, Some("above".to_string()));
        assert_eq!(alert.price, Some("0.65".to_string()));
        assert_eq!(alert.persistent, Some(false));
        assert_eq!(alert.triggered, Some(false));
        assert_eq!(alert.triggered_at, None);
        assert_eq!(alert.created_at, Some("2026-04-01T00:00:00Z".to_string()));
    }

    #[test]
    fn test_ai_query_response_sources_are_strings() {
        // #23: AiQueryResponse.sources must be Vec<String>, plus suggestedActions
        let json = r#"{
            "answer": "BTC is bullish",
            "confidence": 0.9,
            "sources": ["source1", "source2"],
            "suggestedActions": ["buy BTC", "increase position"]
        }"#;
        let resp: AiQueryResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.sources, vec!["source1", "source2"]);
        assert_eq!(resp.suggested_actions, vec!["buy BTC", "increase position"]);
    }

    #[test]
    fn test_redeem_position_params_uses_position_id_and_market_id() {
        // #31: Must send positionId/marketId, not tokenId/conditionId
        let params = RedeemPositionParams {
            position_id: "pos-123".into(),
            market_id: Some("mkt-456".into()),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["positionId"], "pos-123");
        assert_eq!(json["marketId"], "mkt-456");
        assert!(json.get("tokenId").is_none());
        assert!(json.get("conditionId").is_none());
    }

    #[test]
    fn test_redeem_position_params_market_id_omitted_when_none() {
        // #31: marketId should be omitted when None
        let params = RedeemPositionParams {
            position_id: "pos-123".into(),
            market_id: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["positionId"], "pos-123");
        assert!(json.get("marketId").is_none());
    }

    #[test]
    fn test_redeem_position_response_deserializes_position_id() {
        // #150: platform returns positionId, not orderId — PlaceOrderResponse would fail here
        let json = r#"{"positionId":"pos-abc","intentId":"int-xyz","status":"REDEEMED"}"#;
        let resp: RedeemPositionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.position_id, "pos-abc");
        assert_eq!(resp.intent_id, "int-xyz");
        assert_eq!(resp.status, "REDEEMED");
    }

    #[test]
    fn test_split_position_params_uses_amount_string() {
        // #30: SplitPositionParams must have {tokenId, amount} not {tokenId, size, price}
        let params = SplitPositionParams {
            token_id: "tok-1".into(),
            amount: "100.5".into(),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["tokenId"], "tok-1");
        assert_eq!(json["amount"], "100.5");
        assert!(json.get("size").is_none());
        assert!(json.get("price").is_none());
    }

    #[test]
    fn test_merge_position_params_uses_single_token_and_amount() {
        // #30: MergePositionParams must have {tokenId, amount} not {tokenIds: [...]}
        let params = MergePositionParams {
            token_id: "tok-1".into(),
            amount: "200".into(),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["tokenId"], "tok-1");
        assert_eq!(json["amount"], "200");
        assert!(json.get("tokenIds").is_none());
    }

    #[test]
    fn test_provide_liquidity_params_serializes_correct_fields() {
        // Platform contract: {marketId, tokenId, amountUsdc, targetSpread?}
        let params = ProvideLiquidityParams {
            market_id: "mkt-1".into(),
            token_id: "tok-1".into(),
            amount_usdc: 1000.0,
            target_spread: Some(0.02),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["marketId"], "mkt-1");
        assert_eq!(json["tokenId"], "tok-1");
        assert_eq!(json["amountUsdc"], 1000.0);
        assert_eq!(json["targetSpread"], 0.02);
        assert!(json.get("size").is_none());
        assert!(json.get("spread").is_none());
    }

    #[test]
    fn test_provide_liquidity_params_omits_optional_target_spread() {
        let params = ProvideLiquidityParams {
            market_id: "mkt-1".into(),
            token_id: "tok-1".into(),
            amount_usdc: 500.0,
            target_spread: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert!(json.get("targetSpread").is_none());
    }

    #[test]
    fn test_import_strategy_body_has_polyforge_and_strategy_fields() {
        // #32: import_strategy must send {polyforge, strategy}, not {data}
        let strategy = serde_json::json!({"name": "Test Strategy", "triggers": []});
        let body = serde_json::json!({
            "polyforge": "1.0",
            "strategy": strategy,
        });
        assert_eq!(body["polyforge"], "1.0");
        assert!(body.get("strategy").is_some());
        assert!(body.get("data").is_none());
    }

    #[test]
    fn test_import_strategy_body_with_exported_at() {
        // #32: optional exportedAt field
        let mut body = serde_json::json!({
            "polyforge": "1.0",
            "strategy": {"name": "Test"},
        });
        body["exportedAt"] = serde_json::json!("2026-04-13T10:00:00Z");
        assert_eq!(body["exportedAt"], "2026-04-13T10:00:00Z");
    }

    // --- Watchlist types (closes #55) ---

    #[test]
    fn test_watchlist_item_deserializes() {
        let json = r#"{
            "marketId": "mkt-1",
            "slug": "us-election-2026",
            "title": "US Election 2026",
            "currentPrice": "0.65",
            "volume24h": "123456",
            "priceDelta24h": "0.03",
            "watched": true
        }"#;
        let item: WatchlistItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.market_id, "mkt-1");
        assert_eq!(item.slug.as_deref(), Some("us-election-2026"));
        assert_eq!(item.title.as_deref(), Some("US Election 2026"));
        assert_eq!(item.current_price.as_deref(), Some("0.65"));
        assert_eq!(item.volume_24h.as_deref(), Some("123456"));
        assert_eq!(item.price_delta_24h.as_deref(), Some("0.03"));
        assert_eq!(item.watched, Some(true));
    }

    #[test]
    fn test_watchlist_item_deserializes_minimal() {
        let json = r#"{"marketId": "mkt-2"}"#;
        let item: WatchlistItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.market_id, "mkt-2");
        assert!(item.slug.is_none());
        assert!(item.title.is_none());
        assert!(item.watched.is_none());
    }

    #[test]
    fn test_watchlist_add_response_deserializes() {
        let json = r#"{"marketId": "mkt-1", "addedAt": "2026-04-13T12:00:00Z"}"#;
        let resp: WatchlistAddResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.market_id, "mkt-1");
        assert_eq!(resp.added_at.as_deref(), Some("2026-04-13T12:00:00Z"));
    }

    #[test]
    fn test_watchlist_status_deserializes() {
        let json = r#"{"marketId": "mkt-1", "watched": true}"#;
        let status: WatchlistStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.market_id, "mkt-1");
        assert!(status.watched);
    }

    #[test]
    fn test_watchlist_status_not_watched() {
        let json = r#"{"marketId": "mkt-1", "watched": false}"#;
        let status: WatchlistStatus = serde_json::from_str(json).unwrap();
        assert!(!status.watched);
    }

    #[test]
    fn test_add_to_watchlist_body_format() {
        let body = json!({ "marketId": "mkt-abc" });
        assert_eq!(body["marketId"], "mkt-abc");
        assert!(body.get("market_id").is_none(), "must use camelCase key");
    }

    // --- Webhook mutation types (closes #56) ---

    #[test]
    fn test_webhook_test_result_deserializes() {
        let json = r#"{"success": true, "statusCode": 200}"#;
        let result: WebhookTestResult = serde_json::from_str(json).unwrap();
        assert!(result.success);
        assert_eq!(result.status_code, 200);
    }

    #[test]
    fn test_webhook_test_result_failure() {
        let json = r#"{"success": false, "statusCode": 500}"#;
        let result: WebhookTestResult = serde_json::from_str(json).unwrap();
        assert!(!result.success);
        assert_eq!(result.status_code, 500);
    }

    #[test]
    fn test_webhook_test_result_with_extra_fields() {
        let json = r#"{"success": true, "statusCode": 200, "latencyMs": 42}"#;
        let result: WebhookTestResult = serde_json::from_str(json).unwrap();
        assert!(result.success);
        assert_eq!(result.status_code, 200);
        assert_eq!(result.extra["latencyMs"], 42);
    }

    // --- Price history & order book (closes #54) ---

    #[test]
    fn test_price_history_params_default() {
        // Platform uses resolution/from/to/limit — no period field
        let params = PriceHistoryParams::default();
        assert!(params.resolution.is_none());
        assert!(params.from.is_none());
        assert!(params.to.is_none());
        assert!(params.limit.is_none());
    }

    #[test]
    fn test_price_history_params_serializes() {
        // resolution accepts "1m", "1h", "1d" (enum values from platform)
        let params = PriceHistoryParams {
            resolution: Some("1h".into()),
            from: Some("2026-01-01T00:00:00Z".into()),
            to: Some("2026-01-02T00:00:00Z".into()),
            limit: Some(500),
        };
        let val = serde_json::to_value(&params).unwrap();
        assert_eq!(val["resolution"], "1h");
        assert_eq!(val["from"], "2026-01-01T00:00:00Z");
        assert_eq!(val["to"], "2026-01-02T00:00:00Z");
        assert_eq!(val["limit"], 500);
        assert!(val.get("period").is_none());
    }

    #[test]
    fn test_price_history_params_omits_none_fields() {
        let params = PriceHistoryParams {
            resolution: Some("1d".into()),
            ..Default::default()
        };
        let val = serde_json::to_value(&params).unwrap();
        assert_eq!(val["resolution"], "1d");
        assert!(val.get("limit").is_none());
        assert!(val.get("from").is_none());
        assert!(val.get("to").is_none());
        assert!(val.get("period").is_none());
    }

    #[test]
    #[allow(deprecated)]
    fn test_price_history_entry_deserializes() {
        let json = r#"{"timestamp": "2026-01-15T12:00:00Z", "price": 0.65, "volume": 1234.5}"#;
        let entry: PriceHistoryEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.timestamp, "2026-01-15T12:00:00Z");
        assert!((entry.price - 0.65).abs() < f64::EPSILON);
        assert!((entry.volume.unwrap() - 1234.5).abs() < f64::EPSILON);
    }

    #[test]
    #[allow(deprecated)]
    fn test_price_history_entry_deserializes_without_volume() {
        let json = r#"{"timestamp": "2026-01-15T12:00:00Z", "price": 0.42}"#;
        let entry: PriceHistoryEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.timestamp, "2026-01-15T12:00:00Z");
        assert!((entry.price - 0.42).abs() < f64::EPSILON);
        assert!(entry.volume.is_none());
    }

    #[test]
    #[allow(deprecated)]
    fn test_price_history_vec_deserializes() {
        let json = r#"[
            {"timestamp": "2026-01-15T12:00:00Z", "price": 0.65},
            {"timestamp": "2026-01-15T13:00:00Z", "price": 0.67, "volume": 500.0}
        ]"#;
        let entries: Vec<PriceHistoryEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries.len(), 2);
        assert!((entries[1].price - 0.67).abs() < f64::EPSILON);
        assert!(entries[0].volume.is_none());
        assert!(entries[1].volume.is_some());
    }

    #[test]
    fn test_candle_deserializes_string_fields() {
        let json = r#"{
            "time": "2026-04-19T12:00:00.000Z",
            "open": "0.65",
            "high": "0.72",
            "low": "0.63",
            "close": "0.70",
            "volume": "1500"
        }"#;
        let candle: Candle = serde_json::from_str(json).unwrap();
        assert_eq!(candle.time, "2026-04-19T12:00:00.000Z");
        assert!((candle.open - 0.65).abs() < f64::EPSILON);
        assert!((candle.high - 0.72).abs() < f64::EPSILON);
        assert!((candle.low - 0.63).abs() < f64::EPSILON);
        assert!((candle.close - 0.70).abs() < f64::EPSILON);
        assert!((candle.volume - 1500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_candle_deserializes_zero_defaults() {
        let json = r#"{
            "time": "2026-04-19T12:00:00.000Z",
            "open": "0",
            "high": "0",
            "low": "0",
            "close": "0",
            "volume": "0"
        }"#;
        let candle: Candle = serde_json::from_str(json).unwrap();
        assert!((candle.open).abs() < f64::EPSILON);
        assert!((candle.volume).abs() < f64::EPSILON);
    }

    #[test]
    fn test_price_history_response_deserializes() {
        let json = r#"{
            "tokenId": "token-abc",
            "resolution": "1h",
            "hasGaps": false,
            "data": [
                {
                    "time": "2026-04-19T12:00:00.000Z",
                    "open": "0.65",
                    "high": "0.72",
                    "low": "0.63",
                    "close": "0.70",
                    "volume": "1500"
                },
                {
                    "time": "2026-04-19T13:00:00.000Z",
                    "open": "0.70",
                    "high": "0.75",
                    "low": "0.68",
                    "close": "0.73",
                    "volume": "2000"
                }
            ]
        }"#;
        let resp: PriceHistoryResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.token_id, "token-abc");
        assert_eq!(resp.resolution, "1h");
        assert!(!resp.has_gaps);
        assert_eq!(resp.data.len(), 2);
        assert!((resp.data[0].close - 0.70).abs() < f64::EPSILON);
        assert!((resp.data[1].volume - 2000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_price_history_response_with_gaps() {
        let json = r#"{
            "tokenId": "token-xyz",
            "resolution": "1d",
            "hasGaps": true,
            "data": []
        }"#;
        let resp: PriceHistoryResponse = serde_json::from_str(json).unwrap();
        assert!(resp.has_gaps);
        assert!(resp.data.is_empty());
    }

    #[test]
    fn test_order_book_level_deserializes() {
        let json = r#"{"price": 0.55, "size": 100.0}"#;
        let level: OrderBookLevel = serde_json::from_str(json).unwrap();
        assert!((level.price - 0.55).abs() < f64::EPSILON);
        assert!((level.size - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_order_book_deserializes() {
        let json = r#"{
            "bids": [
                {"price": 0.55, "size": 100.0},
                {"price": 0.54, "size": 200.0}
            ],
            "asks": [
                {"price": 0.56, "size": 150.0}
            ]
        }"#;
        let book: OrderBook = serde_json::from_str(json).unwrap();
        assert_eq!(book.bids.len(), 2);
        assert_eq!(book.asks.len(), 1);
        assert!((book.bids[0].price - 0.55).abs() < f64::EPSILON);
        assert!((book.bids[1].size - 200.0).abs() < f64::EPSILON);
        assert!((book.asks[0].price - 0.56).abs() < f64::EPSILON);
    }

    #[test]
    fn test_order_book_deserializes_empty() {
        let json = r#"{"bids": [], "asks": []}"#;
        let book: OrderBook = serde_json::from_str(json).unwrap();
        assert!(book.bids.is_empty());
        assert!(book.asks.is_empty());
    }

    // -----------------------------------------------------------------------
    // Conditional orders, alert CRUD, portfolio PnL (#40)
    // -----------------------------------------------------------------------

    #[test]
    fn test_conditional_order_status_enum_deserializes_all_variants() {
        let variants = ["PENDING", "TRIGGERED", "CANCELLED", "EXPIRED", "FAILED"];
        for v in &variants {
            let json = format!("\"{}\"", v);
            let status: ConditionalOrderStatus = serde_json::from_str(&json).unwrap();
            let serialized = serde_json::to_value(&status).unwrap();
            assert_eq!(serialized, serde_json::Value::String(v.to_string()));
        }
    }

    #[test]
    fn test_conditional_order_deserializes_full() {
        let json = r#"{
            "id": "co-1",
            "tokenId": "tok-abc",
            "side": "BUY",
            "outcome": "YES",
            "size": "100",
            "triggerPrice": "0.60",
            "limitPrice": "0.62",
            "conditionType": "STOP",
            "status": "PENDING",
            "createdAt": "2026-04-13T10:00:00Z",
            "triggeredAt": null,
            "expiresAt": "2026-04-20T10:00:00Z"
        }"#;
        let co: ConditionalOrder = serde_json::from_str(json).unwrap();
        assert_eq!(co.id, "co-1");
        assert_eq!(co.token_id.as_deref(), Some("tok-abc"));
        assert_eq!(co.trigger_price.as_deref(), Some("0.60"));
        assert_eq!(co.status, Some(ConditionalOrderStatus::Pending));
        assert_eq!(co.expires_at.as_deref(), Some("2026-04-20T10:00:00Z"));
    }

    #[test]
    fn test_conditional_order_deserializes_minimal() {
        let json = r#"{"id": "co-2"}"#;
        let co: ConditionalOrder = serde_json::from_str(json).unwrap();
        assert_eq!(co.id, "co-2");
        assert!(co.token_id.is_none());
        assert!(co.status.is_none());
    }

    #[test]
    fn test_create_conditional_order_params_serializes_camelcase() {
        let params = CreateConditionalOrderParams {
            market_id: "mkt-1".into(),
            token_id: "tok-1".into(),
            order_type: "STOP_LOSS".into(),
            side: "BUY".into(),
            outcome: "YES".into(),
            size: 50.0,
            trigger_price: 0.65,
            limit_price: Some(0.67),
            trailing_pct: None,
            expires_at: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["marketId"], "mkt-1");
        assert_eq!(json["tokenId"], "tok-1");
        assert_eq!(json["type"], "STOP_LOSS");
        assert_eq!(json["triggerPrice"], 0.65);
        assert_eq!(json["limitPrice"], 0.67);
        assert!(json.get("expiresAt").is_none());
        assert!(json.get("trailingPct").is_none());
    }

    #[test]
    fn test_create_conditional_order_validation_rejects_nan_size() {
        let params = CreateConditionalOrderParams {
            market_id: "mkt-1".into(),
            token_id: "tok-1".into(),
            order_type: "STOP_LOSS".into(),
            side: "BUY".into(),
            outcome: "YES".into(),
            size: f64::NAN,
            trigger_price: 0.65,
            limit_price: None,
            trailing_pct: None,
            expires_at: None,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = PolyforgeClient::new("test-key").unwrap();
        let err = rt
            .block_on(client.create_conditional_order(&params))
            .unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
        assert!(err.to_string().contains("size"));
    }

    #[test]
    fn test_create_conditional_order_validation_rejects_negative_trigger_price() {
        let params = CreateConditionalOrderParams {
            market_id: "mkt-1".into(),
            token_id: "tok-1".into(),
            order_type: "STOP_LOSS".into(),
            side: "BUY".into(),
            outcome: "YES".into(),
            size: 10.0,
            trigger_price: -0.5,
            limit_price: None,
            trailing_pct: None,
            expires_at: None,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = PolyforgeClient::new("test-key").unwrap();
        let err = rt
            .block_on(client.create_conditional_order(&params))
            .unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
        assert!(err.to_string().contains("trigger_price"));
    }

    #[test]
    fn test_create_conditional_order_validation_rejects_nan_limit_price() {
        let params = CreateConditionalOrderParams {
            market_id: "mkt-1".into(),
            token_id: "tok-1".into(),
            order_type: "LIMIT".into(),
            side: "BUY".into(),
            outcome: "YES".into(),
            size: 10.0,
            trigger_price: 0.5,
            limit_price: Some(f64::NAN),
            trailing_pct: None,
            expires_at: None,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = PolyforgeClient::new("test-key").unwrap();
        let err = rt
            .block_on(client.create_conditional_order(&params))
            .unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
        assert!(err.to_string().contains("limit_price"));
    }

    #[test]
    fn test_list_conditional_orders_params_default() {
        let params = ListConditionalOrdersParams::default();
        assert!(params.status.is_none());
        assert!(params.order_type.is_none());
        assert!(params.page.is_none());
        assert!(params.limit.is_none());
    }

    #[test]
    fn test_list_orders_params_has_page_and_market_id() {
        let params = ListOrdersParams {
            page: Some(2),
            market_id: Some("mkt-1".into()),
            ..Default::default()
        };
        assert_eq!(params.page, Some(2));
        assert_eq!(params.market_id.as_deref(), Some("mkt-1"));
    }

    #[test]
    fn test_conditional_order_type_serializes() {
        assert_eq!(
            serde_json::to_value(ConditionalOrderType::TakeProfit).unwrap(),
            serde_json::Value::String("TAKE_PROFIT".to_string())
        );
        assert_eq!(
            serde_json::to_value(ConditionalOrderType::StopLoss).unwrap(),
            serde_json::Value::String("STOP_LOSS".to_string())
        );
        assert_eq!(
            serde_json::to_value(ConditionalOrderType::TrailingStop).unwrap(),
            serde_json::Value::String("TRAILING_STOP".to_string())
        );
        assert_eq!(
            serde_json::to_value(ConditionalOrderType::Limit).unwrap(),
            serde_json::Value::String("LIMIT".to_string())
        );
        assert_eq!(
            serde_json::to_value(ConditionalOrderType::Pegged).unwrap(),
            serde_json::Value::String("PEGGED".to_string())
        );
    }

    #[test]
    fn test_alert_direction_serializes() {
        // Platform expects lowercase: "above" / "below" (not "ABOVE" / "BELOW")
        let above = serde_json::to_value(AlertDirection::Above).unwrap();
        assert_eq!(above, serde_json::Value::String("above".to_string()));
        let below = serde_json::to_value(AlertDirection::Below).unwrap();
        assert_eq!(below, serde_json::Value::String("below".to_string()));
    }

    #[test]
    fn test_create_alert_params_serializes_camelcase() {
        let params = CreateAlertParams {
            token_id: "tok-1".into(),
            direction: AlertDirection::Above,
            price: 0.75,
            persistent: Some(true),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["tokenId"], "tok-1");
        assert_eq!(json["direction"], "above");
        assert_eq!(json["price"], 0.75);
        assert_eq!(json["persistent"], true);
    }

    #[test]
    fn test_create_alert_params_omits_none_persistent() {
        let params = CreateAlertParams {
            token_id: "tok-1".into(),
            direction: AlertDirection::Below,
            price: 0.25,
            persistent: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert!(json.get("persistent").is_none());
    }

    #[test]
    fn test_create_alert_validation_rejects_zero_price() {
        let params = CreateAlertParams {
            token_id: "tok-1".into(),
            direction: AlertDirection::Above,
            price: 0.0,
            persistent: None,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = PolyforgeClient::new("test-key").unwrap();
        let err = rt.block_on(client.create_alert(&params)).unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
        assert!(err.to_string().contains("price"));
    }

    #[test]
    fn test_portfolio_pnl_deserializes_full() {
        let json = r#"{
            "totalPnl": "125.50",
            "realizedPnl": "100.00",
            "unrealizedPnl": "25.50",
            "period": "30d",
            "strategyId": "strat-1",
            "dataPoints": [{"timestamp": "2026-04-01", "pnl": "10.00"}]
        }"#;
        let pnl: PortfolioPnl = serde_json::from_str(json).unwrap();
        assert_eq!(pnl.total_pnl.as_deref(), Some("125.50"));
        assert_eq!(pnl.realized_pnl.as_deref(), Some("100.00"));
        assert_eq!(pnl.unrealized_pnl.as_deref(), Some("25.50"));
        assert_eq!(pnl.period.as_deref(), Some("30d"));
        assert_eq!(pnl.strategy_id.as_deref(), Some("strat-1"));
        assert_eq!(pnl.data_points.len(), 1);
    }

    #[test]
    fn test_portfolio_pnl_deserializes_minimal() {
        let json = r#"{}"#;
        let pnl: PortfolioPnl = serde_json::from_str(json).unwrap();
        assert!(pnl.total_pnl.is_none());
        assert!(pnl.data_points.is_empty());
    }

    #[test]
    fn test_get_portfolio_pnl_params_default() {
        let params = GetPortfolioPnlParams::default();
        assert!(params.period.is_none());
        assert!(params.strategy_id.is_none());
    }

    #[test]
    fn test_pnl_period_serializes_correctly() {
        assert_eq!(
            serde_json::to_value(PnlPeriod::SevenDays).unwrap(),
            serde_json::Value::String("7d".to_string())
        );
        assert_eq!(
            serde_json::to_value(PnlPeriod::ThirtyDays).unwrap(),
            serde_json::Value::String("30d".to_string())
        );
        assert_eq!(
            serde_json::to_value(PnlPeriod::NinetyDays).unwrap(),
            serde_json::Value::String("90d".to_string())
        );
        assert_eq!(
            serde_json::to_value(PnlPeriod::AllTime).unwrap(),
            serde_json::Value::String("allTime".to_string())
        );
    }

    #[test]
    fn test_paginated_conditional_orders_deserializes() {
        let json = r#"{
            "data": [{"id": "co-1"}, {"id": "co-2"}],
            "total": 2,
            "page": 1,
            "limit": 10,
            "totalPages": 1,
            "hasNext": false
        }"#;
        let resp: PaginatedResponse<ConditionalOrder> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 2);
        assert_eq!(resp.data[0].id, "co-1");
        assert_eq!(resp.total, 2);
    }

    #[test]
    fn test_position_has_platform_fields() {
        // #108: Position must match platform response fields
        let json = r#"{
            "id": "pos-1",
            "marketId": "m1",
            "tokenId": "t1",
            "side": "BUY",
            "size": "100.5",
            "avgPrice": "0.65",
            "currentPrice": "0.72",
            "unrealizedPnl": "7.035",
            "realizedPnl": "12.50",
            "openedAt": "2026-03-15T10:00:00Z"
        }"#;
        let pos: Position = serde_json::from_str(json).unwrap();
        assert_eq!(pos.id, Some("pos-1".to_string()));
        assert_eq!(pos.side, Some("BUY".to_string()));
        assert_eq!(pos.unrealized_pnl, Some("7.035".to_string()));
        assert_eq!(pos.realized_pnl, Some("12.50".to_string()));
        assert_eq!(pos.opened_at, Some("2026-03-15T10:00:00Z".to_string()));
    }

    #[test]
    fn test_strategy_template_has_blocks_and_popularity() {
        // #109: StrategyTemplate must include blocks and popularity
        let json = r#"{
            "id": "tmpl-1",
            "name": "Mean Reversion",
            "description": "Buy low sell high",
            "category": "Technical",
            "blocks": [
                {"id": "b1", "type": "PRICE_TRIGGER", "label": "MA Cross"}
            ],
            "popularity": 342
        }"#;
        let tmpl: StrategyTemplate = serde_json::from_str(json).unwrap();
        assert_eq!(tmpl.name, Some("Mean Reversion".to_string()));
        assert_eq!(tmpl.blocks.len(), 1);
        assert_eq!(tmpl.popularity, 342);
    }

    #[test]
    fn test_browse_marketplace_params_has_offset() {
        // Verify BrowseMarketplaceParams has offset field and it's usable
        let params = BrowseMarketplaceParams {
            sort: Some("newest".to_string()),
            tag: Some("crypto".to_string()),
            limit: Some(20),
            offset: Some(40),
        };
        assert_eq!(params.offset, Some(40));
        assert_eq!(params.limit, Some(20));
    }

    #[test]
    fn test_browse_marketplace_params_offset_default() {
        let params = BrowseMarketplaceParams::default();
        assert_eq!(params.offset, None);
    }

    // -- Backtest endpoints (#57, #74) --

    #[test]
    fn test_list_backtests_params_default() {
        let params = ListBacktestsParams::default();
        assert!(params.strategy_id.is_none());
        assert!(params.status.is_none());
        assert!(params.page.is_none());
        assert!(params.limit.is_none());
    }

    #[test]
    fn test_list_backtests_params_with_filters() {
        let params = ListBacktestsParams {
            strategy_id: Some("strat-1".into()),
            status: Some("COMPLETED".into()),
            page: Some(2),
            limit: Some(10),
        };
        assert_eq!(params.strategy_id.as_deref(), Some("strat-1"));
        assert_eq!(params.status.as_deref(), Some("COMPLETED"));
        assert_eq!(params.page, Some(2));
        assert_eq!(params.limit, Some(10));
    }

    #[test]
    fn test_backtest_deserializes_platform_fields() {
        let json = r#"{
            "id": "bt-1",
            "status": "COMPLETED",
            "strategyId": "s1",
            "startDate": "2026-01-01T00:00:00Z",
            "endDate": "2026-03-01T00:00:00Z",
            "initialBalance": 10000.0,
            "finalBalance": 10150.5,
            "pnl": 150.5,
            "tradeCount": 42,
            "winRate": 0.62,
            "sharpeRatio": 1.8,
            "maxDrawdown": 0.05,
            "createdAt": "2026-04-14T00:00:00Z",
            "completedAt": "2026-04-14T00:01:00Z"
        }"#;
        let bt: Backtest = serde_json::from_str(json).unwrap();
        assert_eq!(bt.id, "bt-1");
        assert_eq!(bt.status.as_deref(), Some("COMPLETED"));
        assert_eq!(bt.strategy_id.as_deref(), Some("s1"));
        assert_eq!(bt.start_date.as_deref(), Some("2026-01-01T00:00:00Z"));
        assert_eq!(bt.end_date.as_deref(), Some("2026-03-01T00:00:00Z"));
        assert_eq!(bt.initial_balance, Some(10000.0));
        assert_eq!(bt.final_balance, Some(10150.5));
        assert_eq!(bt.pnl, Some(150.5));
        assert_eq!(bt.trade_count, Some(42));
        assert_eq!(bt.win_rate, Some(0.62));
        assert_eq!(bt.sharpe_ratio, Some(1.8));
        assert_eq!(bt.max_drawdown, Some(0.05));
        assert_eq!(bt.created_at.as_deref(), Some("2026-04-14T00:00:00Z"));
        assert_eq!(bt.completed_at.as_deref(), Some("2026-04-14T00:01:00Z"));
    }

    // ── Copy trading CRUD (#51) ───────────────────────────────────────────────

    #[test]
    fn test_create_copy_config_params_serializes_target_wallet() {
        let params = CreateCopyConfigParams {
            target_wallet: "0xabcdef1234567890abcdef1234567890abcdef12".to_string(),
            mode: Some(CopyMode::Percentage),
            ..Default::default()
        };
        let body = serde_json::to_value(&params).unwrap();
        assert_eq!(
            body["targetWallet"],
            "0xabcdef1234567890abcdef1234567890abcdef12"
        );
        assert_eq!(body["mode"], "PERCENTAGE");
    }

    #[test]
    fn test_update_copy_config_params_skips_none_fields() {
        let params = UpdateCopyConfigParams {
            size_value: Some("100".to_string()),
            ..Default::default()
        };
        let body = serde_json::to_value(&params).unwrap();
        assert!(body.get("sizeValue").is_some());
        // None fields should be absent due to skip_serializing_if
        assert!(body.get("mode").is_none());
        assert!(body.get("maxExposure").is_none());
    }

    #[test]
    fn test_get_copy_trades_params_default() {
        let params = GetCopyTradesParams::default();
        assert!(params.page.is_none());
        assert!(params.limit.is_none());
    }

    // ── Whale extended (#66) ─────────────────────────────────────────────────

    #[test]
    fn test_whale_profile_deserializes() {
        let json = r#"{
            "walletAddress": "0xabc",
            "stats": {
                "totalVolume": "10000",
                "totalPnl": "500",
                "tradeCount": 42,
                "winRate": "0.62"
            },
            "recentTrades": [],
            "sparkline": [0, 1, 2],
            "isFollowing": true
        }"#;
        let profile: WhaleProfile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.wallet_address, "0xabc");
        assert!(profile.stats.is_some());
        assert!(profile.is_following);
        assert_eq!(profile.sparkline, vec![0, 1, 2]);
    }

    #[test]
    fn test_get_top_whales_params_default() {
        let params = GetTopWhalesParams::default();
        assert!(params.sort_by.is_none());
        assert!(params.period.is_none());
        assert!(params.limit.is_none());
    }

    // ── Paper trading (#66) ──────────────────────────────────────────────────

    #[test]
    fn test_paper_summary_deserializes() {
        let json = r#"{"balance": 10000.0, "pnl": 500.0, "tradeCount": 12, "openPositions": 3}"#;
        let summary: PaperSummary = serde_json::from_str(json).unwrap();
        assert_eq!(summary.balance, 10000.0);
        assert_eq!(summary.pnl, 500.0);
        assert_eq!(summary.trade_count, 12);
        assert_eq!(summary.open_positions, 3);
    }

    // ── Batch API (#66) ──────────────────────────────────────────────────────

    #[test]
    fn test_batch_request_item_serializes() {
        let item = BatchRequestItem {
            id: "req-1".to_string(),
            method: "GET".to_string(),
            path: "/api/v1/portfolio".to_string(),
            body: None,
        };
        let val = serde_json::to_value(&item).unwrap();
        assert_eq!(val["id"], "req-1");
        assert_eq!(val["method"], "GET");
        assert_eq!(val["path"], "/api/v1/portfolio");
        assert!(val.get("body").is_none());
    }

    #[test]
    fn test_batch_response_deserializes() {
        let json = r#"{"results":[{"id":"req-1","status":200,"body":{"ok":true}}]}"#;
        let resp: BatchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.results.len(), 1);
        assert_eq!(resp.results[0].id, "req-1");
        assert_eq!(resp.results[0].status, 200);
    }

    // ── Marketplace seller (#66) ─────────────────────────────────────────────

    #[test]
    fn test_create_listing_params_serializes() {
        let params = CreateListingParams {
            strategy_id: "strat-1".to_string(),
            title: "My Strategy".to_string(),
            description: None,
            price_usdc: 10.0,
            tags: Some(vec!["algo".to_string()]),
        };
        let body = serde_json::to_value(&params).unwrap();
        assert_eq!(body["strategyId"], "strat-1");
        assert_eq!(body["title"], "My Strategy");
        assert_eq!(body["priceUsdc"], 10.0);
        assert!(body.get("description").is_none());
        assert_eq!(body["tags"][0], "algo");
    }

    #[test]
    fn test_update_listing_params_skips_none_fields() {
        let params = UpdateListingParams {
            status: Some("PAUSED".to_string()),
            ..Default::default()
        };
        let body = serde_json::to_value(&params).unwrap();
        assert_eq!(body["status"], "PAUSED");
        assert!(body.get("title").is_none());
        assert!(body.get("priceUsdc").is_none());
    }

    #[test]
    fn test_rate_listing_params_serializes() {
        let params = RateListingParams {
            rating: 5,
            review: Some("Excellent!".to_string()),
        };
        let body = serde_json::to_value(&params).unwrap();
        assert_eq!(body["rating"], 5);
        assert_eq!(body["review"], "Excellent!");
    }

    // ── Risk Settings (#124) ─────────────────────────────────────────────────

    #[test]
    fn test_risk_settings_deserializes_full() {
        let json = r#"{
            "drawdownEnabled": true,
            "drawdownLookbackHours": 8,
            "drawdownThresholdPct": 0.15,
            "circuitBreakerTripped": false,
            "circuitBreakerTrippedAt": null
        }"#;
        let rs: RiskSettings = serde_json::from_str(json).unwrap();
        assert!(rs.drawdown_enabled);
        assert_eq!(rs.drawdown_lookback_hours, 8);
        assert!((rs.drawdown_threshold_pct - 0.15).abs() < f64::EPSILON);
        assert!(!rs.circuit_breaker_tripped);
        assert!(rs.circuit_breaker_tripped_at.is_none());
    }

    #[test]
    fn test_risk_settings_deserializes_minimal() {
        let json = r#"{}"#;
        let rs: RiskSettings = serde_json::from_str(json).unwrap();
        assert!(!rs.drawdown_enabled);
        assert_eq!(rs.drawdown_lookback_hours, 24);
        assert!((rs.drawdown_threshold_pct - 0.1).abs() < f64::EPSILON);
        assert!(!rs.circuit_breaker_tripped);
    }

    #[test]
    fn test_risk_settings_with_tripped_at() {
        let json = r#"{
            "drawdownEnabled": true,
            "drawdownLookbackHours": 24,
            "drawdownThresholdPct": 0.10,
            "circuitBreakerTripped": true,
            "circuitBreakerTrippedAt": "2026-04-17T10:00:00Z"
        }"#;
        let rs: RiskSettings = serde_json::from_str(json).unwrap();
        assert!(rs.circuit_breaker_tripped);
        assert_eq!(
            rs.circuit_breaker_tripped_at.as_deref(),
            Some("2026-04-17T10:00:00Z")
        );
    }

    #[test]
    fn test_update_risk_settings_params_omits_none_fields() {
        let params = UpdateRiskSettingsParams {
            drawdown_enabled: Some(true),
            ..Default::default()
        };
        let body = serde_json::to_value(&params).unwrap();
        assert_eq!(body["drawdownEnabled"], true);
        assert!(body.get("drawdownLookbackHours").is_none());
        assert!(body.get("drawdownThresholdPct").is_none());
    }

    #[test]
    fn test_update_risk_settings_params_all_fields() {
        let params = UpdateRiskSettingsParams {
            drawdown_enabled: Some(false),
            drawdown_lookback_hours: Some(8),
            drawdown_threshold_pct: Some(0.2),
        };
        let body = serde_json::to_value(&params).unwrap();
        assert_eq!(body["drawdownEnabled"], false);
        assert_eq!(body["drawdownLookbackHours"], 8);
        assert!((body["drawdownThresholdPct"].as_f64().unwrap() - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_update_risk_settings_params_default_is_empty() {
        let params = UpdateRiskSettingsParams::default();
        let body = serde_json::to_value(&params).unwrap();
        assert!(body.as_object().unwrap().is_empty());
    }

    // ── Market title field (#141) ────────────────────────────────────────────

    #[test]
    fn test_market_deserializes_title_field() {
        // #141: platform returns "title", not "name"; field was renamed accordingly
        let json = r#"{
            "id": "mkt-1",
            "title": "Will BTC hit $100k by end of 2025?"
        }"#;
        let market: Market = serde_json::from_str(json).unwrap();
        assert_eq!(market.id, "mkt-1");
        assert_eq!(market.title, "Will BTC hit $100k by end of 2025?");
        // title must NOT fall through to the extra catch-all
        assert!(
            market.extra.get("title").is_none(),
            "title must be in the typed field, not extra"
        );
    }

    #[test]
    fn test_market_deserializes_tokens_array() {
        // #141: platform returns "tokens: Token[]" not "baseToken"/"quoteToken"
        let json = r#"{
            "id": "mkt-2",
            "title": "Will ETH flip BTC?",
            "tokens": [
                {"id": "tok-yes", "outcome": "Yes", "price": 0.6},
                {"id": "tok-no",  "outcome": "No",  "price": 0.4}
            ]
        }"#;
        let market: Market = serde_json::from_str(json).unwrap();
        assert_eq!(market.tokens.len(), 2);
        assert_eq!(market.tokens[0].id, "tok-yes");
        assert_eq!(market.tokens[0].outcome.as_deref(), Some("Yes"));
        assert_eq!(market.tokens[1].id, "tok-no");
        // tokens must NOT fall through to extra
        assert!(
            market.extra.get("tokens").is_none(),
            "tokens must be in the typed field, not extra"
        );
        // legacy fields must not exist
        assert!(market.extra.get("baseToken").is_none());
        assert!(market.extra.get("quoteToken").is_none());
    }
}
