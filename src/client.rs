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

/// Maximum size of a GDPR personal-data export response body that the SDK will
/// read (500 MiB).  GDPR exports can legitimately be very large and must not be
/// silently truncated.
const MAX_GDPR_EXPORT_SIZE: usize = 500 * 1024 * 1024; // 500 MiB

const IDEMPOTENCY_KEY_HEADER: &str = "Idempotency-Key";

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

/// Reject arb sizes outside the server-enforced `1..=10000` USDC range.
///
/// Mirrors `class-validator` bounds in `ExecuteArbDto` so the SDK rejects bad
/// input before any real-money order ever hits the wire.
fn validate_arb_size(value: f64) -> Result<()> {
    if value.is_nan() || value.is_infinite() {
        return Err(PolyforgeError::Validation(format!(
            "size must be a finite number, got {value}"
        )));
    }
    if value.fract() != 0.0 {
        return Err(PolyforgeError::Validation(format!(
            "size must be an integer USDC amount, got {value}"
        )));
    }
    if !(1.0..=10000.0).contains(&value) {
        return Err(PolyforgeError::Validation(format!(
            "size must be between 1 and 10000, got {value}"
        )));
    }
    Ok(())
}

/// Reject match IDs outside the server-enforced 1..=255 character range.
fn validate_arb_match_id(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > 255 {
        return Err(PolyforgeError::Validation(format!(
            "match_id must be between 1 and 255 characters, got {}",
            value.len()
        )));
    }
    if !is_uuid_like(value) {
        return Err(PolyforgeError::Validation(format!(
            "match_id must be a valid UUID, got {value}"
        )));
    }
    Ok(())
}

fn is_uuid_like(value: &str) -> bool {
    if value.len() != 36 {
        return false;
    }
    value.bytes().enumerate().all(|(idx, byte)| match idx {
        8 | 13 | 18 | 23 => byte == b'-',
        _ => byte.is_ascii_hexdigit(),
    })
}

/// Reject slippage outside the server-enforced `0..=5` percent range.
fn validate_arb_slippage(value: f64) -> Result<()> {
    if value.is_nan() || value.is_infinite() {
        return Err(PolyforgeError::Validation(format!(
            "max_slippage_pct must be a finite number, got {value}"
        )));
    }
    if !(0.0..=5.0).contains(&value) {
        return Err(PolyforgeError::Validation(format!(
            "max_slippage_pct must be between 0 and 5, got {value}"
        )));
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

    fn idempotency_key_header(idempotency_key: &str) -> Result<HeaderValue> {
        if idempotency_key.trim().len() < 8 || idempotency_key.len() > 128 {
            return Err(PolyforgeError::Validation(format!(
                "idempotency_key must be 8–128 non-whitespace characters, got {} ({} after trim)",
                idempotency_key.len(),
                idempotency_key.trim().len()
            )));
        }
        HeaderValue::from_str(idempotency_key).map_err(|_| {
            PolyforgeError::Validation(
                "idempotency_key contains invalid HTTP header characters".into(),
            )
        })
    }

    fn optional_auth_header(&self) -> Result<Option<HeaderValue>> {
        if self.api_key.expose_secret().is_empty() {
            return Ok(None);
        }
        self.auth_header().map(Some)
    }

    fn generate_idempotency_key() -> String {
        uuid::Uuid::new_v4().simple().to_string()
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

    async fn get_with_max_body_size<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        max_body_size: usize,
    ) -> Result<T> {
        let resp = self
            .http
            .get(self.url(path))
            .header(AUTHORIZATION, self.auth_header()?)
            .send()
            .await?;
        self.handle_response_with_max(resp, Some(max_body_size))
            .await
    }

    async fn get_no_auth<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self.http.get(self.url(path)).send().await?;
        self.handle_response(resp).await
    }

    async fn get_with_optional_auth<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<T> {
        let mut req = self.http.get(self.url(path));
        if let Some(auth_header) = self.optional_auth_header()? {
            req = req.header(AUTHORIZATION, auth_header);
        }
        let resp = req.send().await?;
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
            let body = Self::read_error_body(resp, status).await?;
            return Err(Self::api_error_from_body(status, body));
        }
        Ok(resp.text().await?)
    }

    /// Send a GET and return the body as plain text with a configurable
    /// body size limit (for large CSV responses like GDPR exports).
    async fn get_text_with_max_body_size(
        &self,
        path: &str,
        max_body_size: usize,
    ) -> Result<String> {
        let resp = self
            .http
            .get(self.url(path))
            .header(AUTHORIZATION, self.auth_header()?)
            .send()
            .await?;
        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body = Self::read_error_body_with_max(resp, status, Some(max_body_size)).await?;
            return Err(Self::api_error_from_body(status, body));
        }
        let bytes = Self::read_bytes_capped(resp, status, max_body_size).await?;
        String::from_utf8(bytes)
            .map_err(|e| PolyforgeError::Validation(format!("Non-UTF-8 response body: {e}")))
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

    async fn post_with_idempotency_key<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
        idempotency_key: &str,
    ) -> Result<T> {
        let resp = self
            .http
            .post(self.url(path))
            .header(AUTHORIZATION, self.auth_header()?)
            .header(
                IDEMPOTENCY_KEY_HEADER,
                Self::idempotency_key_header(idempotency_key)?,
            )
            .json(body)
            .send()
            .await?;
        self.handle_response(resp).await
    }

    async fn post_idempotent<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let resp = self
            .http
            .post(self.url(path))
            .header(AUTHORIZATION, self.auth_header()?)
            .header(IDEMPOTENCY_KEY_HEADER, Self::generate_idempotency_key())
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

    async fn delete_idempotent<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .http
            .delete(self.url(path))
            .header(AUTHORIZATION, self.auth_header()?)
            .header(IDEMPOTENCY_KEY_HEADER, Self::generate_idempotency_key())
            .send()
            .await?;
        self.handle_response(resp).await
    }

    async fn delete_with_body_idempotent<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let resp = self
            .http
            .delete(self.url(path))
            .header(AUTHORIZATION, self.auth_header()?)
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .header(IDEMPOTENCY_KEY_HEADER, Self::generate_idempotency_key())
            .json(body)
            .send()
            .await?;
        self.handle_response(resp).await
    }

    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<T> {
        self.handle_response_with_max(resp, None).await
    }

    async fn handle_response_with_max<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
        max_body_size: Option<usize>,
    ) -> Result<T> {
        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body = Self::read_error_body_with_max(resp, status, max_body_size).await?;
            return Err(Self::api_error_from_body(status, body));
        }
        if status == 204 {
            return serde_json::from_value(serde_json::Value::Null).map_err(PolyforgeError::from);
        }
        match max_body_size {
            Some(limit) => {
                let bytes = Self::read_bytes_capped(resp, status, limit).await?;
                let body = String::from_utf8(bytes).map_err(|e| {
                    PolyforgeError::Validation(format!("Non-UTF-8 response body: {e}"))
                })?;
                serde_json::from_str(&body).map_err(PolyforgeError::from)
            }
            None => {
                let body = resp.text().await?;
                serde_json::from_str(&body).map_err(PolyforgeError::from)
            }
        }
    }

    /// Read an error response body, allowing bodies exactly 1 MiB and rejecting the first byte over.
    async fn read_error_body(resp: reqwest::Response, status: u16) -> Result<serde_json::Value> {
        Self::read_error_body_with_max(resp, status, None).await
    }

    /// Read an error response body with a configurable size limit.
    async fn read_error_body_with_max(
        mut resp: reqwest::Response,
        status: u16,
        max_body_size: Option<usize>,
    ) -> Result<serde_json::Value> {
        let limit = max_body_size.unwrap_or(MAX_RESPONSE_BODY_SIZE);

        let content_length = resp
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        if let Some(cl) = content_length {
            if cl > limit as u64 {
                return Err(Self::response_body_too_large_error_with_limit(
                    status, cl, limit,
                ));
            }
        }

        let mut body = Vec::new();
        while let Some(chunk) = resp.chunk().await? {
            let next_len = body.len().saturating_add(chunk.len());
            if next_len > limit {
                return Err(Self::response_body_too_large_error_with_limit(
                    status,
                    next_len as u64,
                    limit,
                ));
            }
            body.extend_from_slice(&chunk);
        }

        Ok(serde_json::from_slice(&body).unwrap_or_default())
    }

    /// Read raw bytes from a response body with a hard size cap.
    /// Rejects bodies that exceed `limit` bytes, regardless of status code.
    async fn read_bytes_capped(
        mut resp: reqwest::Response,
        status: u16,
        limit: usize,
    ) -> Result<Vec<u8>> {
        let content_length = resp
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        if let Some(cl) = content_length {
            if cl > limit as u64 {
                return Err(Self::response_body_too_large_error_with_limit(
                    status, cl, limit,
                ));
            }
        }
        let mut body = Vec::new();
        while let Some(chunk) = resp.chunk().await? {
            let next_len = body.len().saturating_add(chunk.len());
            if next_len > limit {
                return Err(Self::response_body_too_large_error_with_limit(
                    status,
                    next_len as u64,
                    limit,
                ));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }

    fn response_body_too_large_error_with_limit(
        status: u16,
        size: u64,
        limit: usize,
    ) -> PolyforgeError {
        PolyforgeError::Api {
            status,
            code: "RESPONSE_BODY_TOO_LARGE".to_string(),
            message: format!("Error response body too large ({size} bytes, limit {limit})"),
            request_id: None,
            suggestion: None,
        }
    }

    fn api_error_from_body(status: u16, body: serde_json::Value) -> PolyforgeError {
        PolyforgeError::Api {
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
        }
    }

    // -----------------------------------------------------------------------
    // System Health (POLA-3327)
    // -----------------------------------------------------------------------

    /// Get the public API health payload (unauthenticated).
    ///
    /// Returns only public status; operational internals are not exposed.
    pub async fn get_health(&self) -> Result<SystemHealthPublic> {
        self.get_no_auth("/health").await
    }

    /// Get the authenticated health/status payload.
    ///
    /// Full operational metrics (DB, Redis, queue depth) are returned when
    /// an API key is provided.
    pub async fn get_health_authenticated(&self) -> Result<SystemHealthAuthenticated> {
        self.get("/api/v1/status").await
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

    /// Full-text search across all markets.
    ///
    /// The platform returns `{ results: [...] }` rather than the standard
    /// paginated envelope.  The SDK deserializes into `SearchResults<Market>`
    /// internally and converts to `PaginatedResponse<Market>` so callers see
    /// a uniform paginated shape.
    pub async fn search_markets(
        &self,
        params: &SearchMarketsParams,
    ) -> Result<PaginatedResponse<Market>> {
        let mut qp: Vec<(&str, String)> = vec![("q", params.q.clone())];
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        let pairs: Vec<String> = qp
            .iter()
            .map(|(k, v)| format!("{}={}", k, encode(v)))
            .collect();
        let qs = format!("?{}", pairs.join("&"));
        let results: SearchResults<Market> =
            self.get(&format!("/api/v1/markets/search{qs}")).await?;
        Ok(results.into_paginated_response(params.limit.unwrap_or(20)))
    }

    /// Get the minimum price tick size for a market token.
    pub async fn get_tick_size(&self, token_id: &str) -> Result<TickSizeResponse> {
        self.get(&format!("/api/v1/markets/{}/tick-size", encode(token_id)))
            .await
    }

    /// Get the current bid-ask spread for a market token.
    pub async fn get_spread(&self, token_id: &str) -> Result<SpreadResponse> {
        self.get(&format!("/api/v1/markets/{}/spread", encode(token_id)))
            .await
    }

    /// Get the current midpoint price for a market token.
    pub async fn get_midpoint(&self, token_id: &str) -> Result<MidpointResponse> {
        self.get(&format!("/api/v1/markets/{}/midpoint", encode(token_id)))
            .await
    }

    /// Get the full CLOB order book for a market token.
    pub async fn get_clob_book(&self, token_id: &str) -> Result<ClobBook> {
        self.get(&format!("/api/v1/markets/{}/clob-book", encode(token_id)))
            .await
    }

    /// Get CLOB price history for a market token.
    pub async fn get_clob_prices_history(
        &self,
        token_id: &str,
        params: Option<ClobPricesHistoryParams>,
    ) -> Result<Vec<ClobPricePoint>> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(ref p) = params {
            if let Some(ref interval) = p.interval {
                qp.push(("interval", interval.clone()));
            }
            if let Some(fidelity) = p.fidelity {
                qp.push(("fidelity", fidelity.to_string()));
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
            "/api/v1/markets/{}/clob-prices-history{qs}",
            encode(token_id)
        ))
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

    /// Start a strategy.
    ///
    /// Pass [`StartStrategyParams::paper()`] or [`StartStrategyParams::live()`]
    /// for the common cases, or build a struct directly for fine-grained control.
    ///
    /// Returns a [`StrategyStatusResponse`] with the new status and `startedAt`
    /// timestamp — not a full [`Strategy`] object.
    pub async fn start_strategy(
        &self,
        id: &str,
        params: StartStrategyParams,
    ) -> Result<StrategyStatusResponse> {
        let body = serde_json::to_value(&params)?;
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
    ///
    /// This is a convenience wrapper around [`update_strategy_with`](Self::update_strategy_with)
    /// that preserves backward compatibility with the original call signature.
    pub async fn update_strategy(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<&str>,
        market_id: Option<&str>,
    ) -> Result<Strategy> {
        self.update_strategy_with(
            id,
            UpdateStrategyParams {
                name: name.map(|s| s.to_string()),
                description: description.map(|s| s.to_string()),
                market_id: market_id.map(|s| s.to_string()),
                ..Default::default()
            },
        )
        .await
    }

    /// Update a strategy's name, description, market, or Kalshi subaccount.
    ///
    /// Accepts an [`UpdateStrategyParams`] for full field coverage, including the
    /// `kalshi_subaccount` field that is not available through the original
    /// [`update_strategy`](Self::update_strategy) method.
    pub async fn update_strategy_with(
        &self,
        id: &str,
        params: UpdateStrategyParams,
    ) -> Result<Strategy> {
        let body = serde_json::to_value(&params)?;
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
    /// `reason` should be one of `"SPAM"`, `"MISLEADING"`, `"INAPPROPRIATE"`, `"OTHER"`.
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
    // Actions Catalog (POLA-3329)
    // -----------------------------------------------------------------------

    /// Fetch the platform's public API actions catalog.
    ///
    /// This endpoint is public (no authentication required) — the client
    /// can be constructed with an empty API key.  The capability manifest
    /// is intended for agent/tooling discovery and mirrors sdk-ts's
    /// `getActions()` / sdk-python's `get_actions()` for
    /// `GET /api/v1/actions`.
    pub async fn get_actions(&self) -> Result<ActionsSchema> {
        self.get_with_optional_auth("/api/v1/actions").await
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

    /// Get the trader score / reputation for the authenticated user.
    pub async fn get_score(&self) -> Result<TraderScore> {
        self.get("/api/v1/scores/me").await
    }

    /// Get the global scores leaderboard.
    pub async fn get_scores_top(&self) -> Result<PaginatedResponse<ScoreEntry>> {
        self.get("/api/v1/scores/top").await
    }

    /// Get the badges awarded to the authenticated user.
    pub async fn get_my_badges(&self) -> Result<Vec<Badge>> {
        self.get("/api/v1/scores/me/badges").await
    }

    /// Get the score for a specific user.
    ///
    /// Sends the `Authorization` header when an API key is configured;
    /// skips it when the client is constructed with an empty key so the
    /// endpoint remains usable for public read-only consumers.
    pub async fn get_user_score(&self, user_id: &str) -> Result<TraderScore> {
        self.get_with_optional_auth(&format!("/api/v1/scores/{}", encode(user_id)))
            .await
    }

    /// Get the badges awarded to a specific user.
    ///
    /// Sends the `Authorization` header when an API key is configured;
    /// skips it when the client is constructed with an empty key so the
    /// endpoint remains usable for public read-only consumers.
    pub async fn get_user_badges(&self, user_id: &str) -> Result<Vec<Badge>> {
        self.get_with_optional_auth(&format!("/api/v1/scores/{}/badges", encode(user_id)))
            .await
    }

    // -----------------------------------------------------------------------
    // Public User Profile Lookups (POLA-1844)
    // -----------------------------------------------------------------------

    /// Fetch a public user's PnL curve over `period` (e.g. `"30d"`, `"7d"`).
    ///
    /// Each entry has a `YYYY-MM-DD` date, that day's `pnl`, and the running
    /// `cum_pnl`. Returns [`PolyforgeError::Api`] with `status = 404` if the
    /// username is unknown.
    pub async fn get_user_performance(
        &self,
        username: &str,
        period: &str,
    ) -> Result<Vec<UserPerformancePoint>> {
        let res: UserDataEnvelope<UserPerformancePoint> = self
            .get_with_optional_auth(&format!(
                "/api/v1/users/{}/performance?period={}",
                encode(username),
                encode(period)
            ))
            .await?;
        Ok(res.data)
    }

    /// List a public user's strategies. `visibility` defaults to `"PUBLIC"` on
    /// the server; the server caps `limit` at 50 (default 6). Pass `None` to
    /// take server defaults.
    pub async fn get_user_strategies(
        &self,
        username: &str,
        visibility: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<UserStrategySummary>> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(v) = visibility {
            qp.push(("visibility", v.to_string()));
        }
        if let Some(l) = limit {
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
        let res: UserDataEnvelope<UserStrategySummary> = self
            .get_with_optional_auth(&format!(
                "/api/v1/users/{}/strategies{}",
                encode(username),
                qs
            ))
            .await?;
        Ok(res.data)
    }

    /// Recent resolved-position activity for a public user. Server caps `limit`
    /// at 50 (default 5). Pass `None` to take the server default.
    pub async fn get_user_activity(
        &self,
        username: &str,
        limit: Option<u32>,
    ) -> Result<Vec<UserActivityEntry>> {
        let qs = match limit {
            Some(l) => format!("?limit={}", l),
            None => String::new(),
        };
        let res: UserDataEnvelope<UserActivityEntry> = self
            .get_with_optional_auth(&format!(
                "/api/v1/users/{}/activity{}",
                encode(username),
                qs
            ))
            .await?;
        Ok(res.data)
    }

    /// Badges earned by a public user (id is the badge type).
    pub async fn get_user_profile_badges(&self, username: &str) -> Result<Vec<UserProfileBadge>> {
        let res: UserDataEnvelope<UserProfileBadge> = self
            .get_with_optional_auth(&format!("/api/v1/users/{}/badges", encode(username)))
            .await?;
        Ok(res.data)
    }

    /// Paginated list of users the authenticated user follows.
    pub async fn get_my_following(
        &self,
        page: Option<u64>,
        limit: Option<u64>,
    ) -> Result<PaginatedResponse<FollowedUser>> {
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
            let pairs: Vec<String> = qp
                .iter()
                .map(|(k, v)| format!("{}={}", k, encode(v)))
                .collect();
            format!("?{}", pairs.join("&"))
        };
        self.get(&format!("/api/v1/users/me/following{}", qs)).await
    }

    // -----------------------------------------------------------------------
    // Sports markets (POLA-1841)
    // -----------------------------------------------------------------------
    //
    // The sports controller surfaces several payloads as
    // `Record<string, unknown>` / `unknown[]`. The SDK mirrors that fidelity
    // and returns `serde_json::Value` for those shapes rather than inventing
    // strict types that could drift from the server.

    /// List sports categories with their series tickers and market counts.
    ///
    /// `GET /api/v1/sports/categories`
    pub async fn list_sports_categories(&self) -> Result<Vec<SportsCategory>> {
        self.get("/api/v1/sports/categories").await
    }

    /// Paginated list of sports markets.
    ///
    /// `GET /api/v1/sports/markets`
    pub async fn list_sports_markets(
        &self,
        params: &ListSportsMarketsParams,
    ) -> Result<PaginatedResponse<serde_json::Value>> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(p) = params.page {
            qp.push(("page", p.to_string()));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(ref c) = params.category {
            qp.push(("category", c.clone()));
        }
        if let Some(ref s) = params.search {
            qp.push(("search", s.clone()));
        }
        if let Some(ref s) = params.series_ticker {
            qp.push(("seriesTicker", s.clone()));
        }
        if let Some(ref e) = params.event_ticker {
            qp.push(("eventTicker", e.clone()));
        }
        if let Some(b) = params.live_only {
            qp.push(("liveOnly", b.to_string()));
        }
        if let Some(ref s) = params.sort {
            qp.push(("sort", s.clone()));
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
        self.get(&format!("/api/v1/sports/markets{qs}")).await
    }

    /// Paginated list of sports events.
    ///
    /// `GET /api/v1/sports/events`
    pub async fn list_sports_events(
        &self,
        params: &ListSportsEventsParams,
    ) -> Result<PaginatedResponse<serde_json::Value>> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(p) = params.page {
            qp.push(("page", p.to_string()));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(ref c) = params.category {
            qp.push(("category", c.clone()));
        }
        if let Some(ref s) = params.series_ticker {
            qp.push(("seriesTicker", s.clone()));
        }
        if let Some(ref s) = params.status {
            qp.push(("status", s.clone()));
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
        self.get(&format!("/api/v1/sports/events{qs}")).await
    }

    /// Get a single sports event with its associated markets.
    ///
    /// `GET /api/v1/sports/events/:eventTicker`
    ///
    /// Returns a `serde_json::Value` shaped as `{ "event": {...}, "markets": [...] }`.
    pub async fn get_sports_event(&self, event_ticker: &str) -> Result<serde_json::Value> {
        self.get(&format!("/api/v1/sports/events/{}", encode(event_ticker)))
            .await
    }

    /// List sports milestones (cursor-paginated by the upstream provider).
    ///
    /// `GET /api/v1/sports/milestones`
    ///
    /// Returns a `serde_json::Value` shaped as `{ "milestones": [...], "cursor": ... }`.
    pub async fn list_sports_milestones(
        &self,
        params: &ListSportsMilestonesParams,
    ) -> Result<serde_json::Value> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(p) = params.page {
            qp.push(("page", p.to_string()));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(ref e) = params.event_ticker {
            qp.push(("eventTicker", e.clone()));
        }
        if let Some(ref s) = params.status {
            qp.push(("status", s.clone()));
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
        self.get(&format!("/api/v1/sports/milestones{qs}")).await
    }

    /// Get the live-data snapshot for a milestone.
    ///
    /// `GET /api/v1/sports/live-data/:milestoneId`
    ///
    /// Returns a `serde_json::Value` shaped as `{ "liveData": ... | null }`.
    pub async fn get_sports_live_data(&self, milestone_id: &str) -> Result<serde_json::Value> {
        self.get(&format!(
            "/api/v1/sports/live-data/{}",
            encode(milestone_id)
        ))
        .await
    }

    /// List combo collections.
    ///
    /// `GET /api/v1/sports/combos`
    ///
    /// Returns a `serde_json::Value` shaped as `{ "collections": [...], "cursor": ... }`.
    pub async fn list_sports_combos(
        &self,
        params: &ListSportsCombosParams,
    ) -> Result<serde_json::Value> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(p) = params.page {
            qp.push(("page", p.to_string()));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(ref s) = params.series_ticker {
            qp.push(("seriesTicker", s.clone()));
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
        self.get(&format!("/api/v1/sports/combos{qs}")).await
    }

    /// Get a combo collection by ticker.
    ///
    /// `GET /api/v1/sports/combos/:collectionTicker`
    ///
    /// **Server-side caveat:** at the time of writing the controller forwards
    /// to `listComboCollections({page:1,limit:1})` and ignores the
    /// `collectionTicker` path param. The SDK wraps the route as-is for
    /// fidelity; a server-side fix is tracked separately.
    pub async fn get_sports_combo_collection(
        &self,
        collection_ticker: &str,
    ) -> Result<serde_json::Value> {
        self.get(&format!(
            "/api/v1/sports/combos/{}",
            encode(collection_ticker)
        ))
        .await
    }

    /// Look up the combo market that matches a set of leg selections.
    ///
    /// `POST /api/v1/sports/combos/lookup`
    ///
    /// Returns a `serde_json::Value` — either `{ "eventTicker": ..., "marketTicker": ... }`
    /// or `null` if no combo exists for the provided legs.
    pub async fn lookup_sports_combo(
        &self,
        params: &SportsComboLookupParams,
    ) -> Result<serde_json::Value> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post("/api/v1/sports/combos/lookup", &body).await
    }

    // -----------------------------------------------------------------------
    // Direct Trading
    // -----------------------------------------------------------------------

    /// Place a direct buy or sell order on a prediction market.
    ///
    /// The SDK automatically sends an `Idempotency-Key` header required by the
    /// platform for trading writes.
    ///
    /// # Errors
    /// Returns [`PolyforgeError::Validation`] if `size` or `price` is NaN,
    /// infinite, zero, or negative.
    pub async fn place_order(&self, params: &PlaceOrderParams) -> Result<PlaceOrderResponse> {
        validate_financial_param("size", params.size)?;
        validate_financial_param("price", params.price)?;
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post_idempotent("/api/v1/orders/place", &body).await
    }

    /// Cancel a pending or live order.
    pub async fn cancel_order(&self, order_id: &str) -> Result<CancelOrderResponse> {
        self.delete_idempotent(&format!("/api/v1/orders/{}", encode(order_id)))
            .await
    }

    /// Place up to 15 orders in a single batch request.
    ///
    /// The SDK automatically sends an `Idempotency-Key` header required by the
    /// platform for trading writes.
    ///
    /// # Errors
    /// Returns [`PolyforgeError::Validation`] if any order `size` or `price` is
    /// NaN, infinite, zero, or negative.
    pub async fn batch_orders(&self, params: &BatchOrdersParams) -> Result<BatchOrdersResponse> {
        for (index, order) in params.orders.iter().enumerate() {
            validate_financial_param(&format!("orders[{index}].size"), order.size)?;
            validate_financial_param(&format!("orders[{index}].price"), order.price)?;
        }
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post_idempotent("/api/v1/orders/batch", &body).await
    }

    /// Cancel up to 3 000 orders in a single bulk request.
    ///
    /// The SDK automatically sends an `Idempotency-Key` header required by the
    /// platform for trading writes.
    pub async fn bulk_cancel_orders(
        &self,
        params: &BulkCancelParams,
    ) -> Result<BulkCancelResponse> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.delete_with_body_idempotent("/api/v1/orders/bulk", &body)
            .await
    }

    /// Close an open prediction-market position (partial or sweep).
    ///
    /// When `params.size` is `None` (the default), this is a **sweep** — the
    /// entire position is sold at market price via a market sell order.  When
    /// `size` is set to a number-string like `"100"`, only that portion is
    /// closed (partial close) and the remainder of the position stays open.
    ///
    /// # Sweep semantics
    ///
    /// GTC orders are priced at `0.001` SELL / `0.999` BUY and behave as a
    /// **market-equivalent sweep, not a resting limit order**.  Slippage is
    /// bounded only by venue depth at call time, not by the on-paper price.
    /// The fill price is whatever the order book offers at the time of
    /// execution.
    ///
    /// **For cross-venue arbitrage positions**, use
    /// [`close_arbitrage_position`](Self::close_arbitrage_position) instead —
    /// arbitrage closes are always full sweeps that place reversing market
    /// orders on both venues simultaneously; partial closes are not supported.
    ///
    /// The SDK automatically sends an `Idempotency-Key` header required by the
    /// platform for trading writes.
    pub async fn close_position(&self, params: &ClosePositionParams) -> Result<PlaceOrderResponse> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post_idempotent("/api/v1/orders/close-position", &body)
            .await
    }

    /// Redeem winning shares after a market resolves.
    ///
    /// The SDK automatically sends an `Idempotency-Key` header required by the
    /// platform for trading writes.
    ///
    /// # Errors
    /// Returns [`PolyforgeError::Validation`] if both `position_id` and
    /// `market_id` are `None`.
    pub async fn redeem_position(
        &self,
        params: &RedeemPositionParams,
    ) -> Result<RedeemPositionResponse> {
        if params.position_id.is_none() && params.market_id.is_none() {
            return Err(PolyforgeError::Validation(
                "at least one of position_id or market_id is required".into(),
            ));
        }
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post_idempotent("/api/v1/orders/redeem", &body).await
    }

    /// Split a position into smaller positions.
    ///
    /// The SDK automatically sends an `Idempotency-Key` header required by the
    /// platform for trading writes.
    pub async fn split_position(&self, params: &SplitPositionParams) -> Result<PlaceOrderResponse> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post_idempotent("/api/v1/orders/split", &body).await
    }

    /// Merge a position (combine token shares).
    ///
    /// The SDK automatically sends an `Idempotency-Key` header required by the
    /// platform for trading writes.
    pub async fn merge_position(&self, params: &MergePositionParams) -> Result<PlaceOrderResponse> {
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post_idempotent("/api/v1/orders/merge", &body).await
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

    /// Export your personal data in JSON format (GDPR compliance).
    ///
    /// Returns a structured [`PersonalDataExport`] with your account details,
    /// trading history, settings, and all platform activity grouped into
    /// `account`, `settings`, `security`, `trading`, `communications`, and
    /// `social` sections.  The response is delivered as a file download
    /// (Content-Disposition: attachment).
    ///
    /// # Errors
    /// Returns [`PolyforgeError::Api`] if the request fails (e.g., insufficient
    /// scope).
    pub async fn export_personal_data(&self) -> Result<PersonalDataExport> {
        self.get_with_max_body_size("/api/v1/me/export", MAX_GDPR_EXPORT_SIZE)
            .await
    }

    /// Export your personal data in CSV format (GDPR compliance).
    ///
    /// Returns CSV text with columns `section, index, data_json` for
    /// machine-readable processing.  The response is delivered as a file
    /// download (Content-Disposition: attachment).
    ///
    /// ```no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = polyforge::PolyforgeClient::new("key")?;
    /// let csv = client.export_personal_data_csv().await?;
    /// std::fs::write("personal-data.csv", &csv)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    /// Returns [`PolyforgeError::Api`] if the request fails.
    pub async fn export_personal_data_csv(&self) -> Result<String> {
        self.get_text_with_max_body_size("/api/v1/me/export?format=csv", MAX_GDPR_EXPORT_SIZE)
            .await
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

    /// Reset the circuit breaker after it has been triggered.
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
    /// The SDK automatically sends an `Idempotency-Key` header required by the
    /// platform for trading writes.
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
        self.post_idempotent("/api/v1/orders/smart", &body).await
    }

    /// List your smart orders with child order progress.
    pub async fn list_smart_orders(&self) -> Result<PaginatedResponse<SmartOrder>> {
        self.get("/api/v1/orders/smart").await
    }

    /// Cancel a pending or active smart order and its child orders.
    pub async fn cancel_smart_order(&self, id: &str) -> Result<serde_json::Value> {
        self.delete_idempotent(&format!("/api/v1/orders/smart/{}", encode(id)))
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

    /// List raw news articles with optional source, sentiment, and pagination filters.
    pub async fn list_news(
        &self,
        params: Option<&ListNewsParams>,
    ) -> Result<PaginatedResponse<NewsArticle>> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(p) = params {
            if let Some(ref source) = p.source {
                qp.push(("source", source.clone()));
            }
            if let Some(ref sentiment) = p.sentiment {
                qp.push(("sentiment", sentiment.clone()));
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
        self.get(&format!("/api/v1/news{qs}")).await
    }

    /// Get a single news article by ID.
    pub async fn get_news_article(&self, id: &str) -> Result<NewsArticle> {
        self.get(&format!("/api/v1/news/{}", encode(id))).await
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
    /// The SDK automatically sends an `Idempotency-Key` header required by the
    /// platform for trading writes.
    ///
    /// # Errors
    /// Returns [`PolyforgeError::Validation`] if `size` or `trigger_price` is
    /// NaN, infinite, zero, or negative, or if `limit_price` is provided but
    /// is not a valid numeric string or is non-positive/non-finite.
    pub async fn create_conditional_order(
        &self,
        params: &CreateConditionalOrderParams,
    ) -> Result<ConditionalOrder> {
        validate_financial_param("size", params.size)?;
        validate_financial_param("trigger_price", params.trigger_price)?;
        if let Some(ref lp) = params.limit_price {
            let price: f64 = lp.parse().map_err(|_| {
                PolyforgeError::Validation(format!(
                    "limit_price must be a numeric string, got {:?}",
                    lp
                ))
            })?;
            validate_financial_param("limit_price", price)?;
        }
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post_idempotent("/api/v1/orders/conditional", &body)
            .await
    }

    /// Get a single conditional order by ID.
    pub async fn get_conditional_order(&self, order_id: &str) -> Result<ConditionalOrder> {
        self.get(&format!("/api/v1/orders/conditional/{}", encode(order_id)))
            .await
    }

    /// Cancel a pending conditional order.
    pub async fn cancel_conditional_order(&self, order_id: &str) -> Result<()> {
        let _: serde_json::Value = self
            .delete_idempotent(&format!("/api/v1/orders/conditional/{}", encode(order_id)))
            .await?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Alert CRUD
    // -----------------------------------------------------------------------

    /// Create a new price alert.
    pub async fn create_alert(&self, params: &CreateAlertParams) -> Result<Alert> {
        let price: f64 = params.price.parse().map_err(|_| {
            PolyforgeError::Validation(format!(
                "price must be a numeric string, got {:?}",
                params.price
            ))
        })?;
        if !price.is_finite() || price <= 0.0 {
            return Err(PolyforgeError::Validation(format!(
                "price must be a positive finite number, got {}",
                params.price
            )));
        }
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

    // -----------------------------------------------------------------------
    // Portfolio — Polymarket-native
    // -----------------------------------------------------------------------

    /// Get the native Polymarket portfolio snapshot for the authenticated user.
    pub async fn get_polymarket_portfolio(&self) -> Result<PolymarketPortfolio> {
        self.get("/api/v1/portfolio/polymarket/portfolio").await
    }

    /// Get native Polymarket earnings summary for the authenticated user.
    pub async fn get_polymarket_earnings(&self) -> Result<PolymarketEarnings> {
        self.get("/api/v1/portfolio/polymarket/earnings").await
    }

    /// Get native Polymarket activity for the authenticated user.
    pub async fn get_polymarket_activity(
        &self,
        params: Option<&GetPolymarketActivityParams>,
    ) -> Result<PolymarketActivityResponse> {
        let qs = match params.and_then(|p| p.activity_type.as_deref()) {
            Some(t) => format!("?type={}", encode(t)),
            None => String::new(),
        };
        self.get(&format!("/api/v1/portfolio/polymarket/activity{qs}"))
            .await
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

    /// Get the accuracy leaderboard ranked by prediction accuracy.
    ///
    /// Returns a paginated list of traders with their accuracy stats.
    /// When `offset` is provided without `page`, it is converted to the
    /// platform's page-based contract.
    pub async fn get_accuracy_leaderboard(
        &self,
        params: Option<&AccuracyLeaderboardParams>,
    ) -> Result<PaginatedResponse<AccuracyLeaderboardEntry>> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(p) = params {
            if let Some(ref period) = p.period {
                qp.push(("period", period.clone()));
            }
            let page = if let Some(page) = p.page {
                Some(page)
            } else if let Some(offset) = p.offset {
                let limit = p.limit.unwrap_or(20);
                Some((offset / limit) + 1)
            } else {
                None
            };
            if let Some(page) = page {
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
        self.get(&format!("/api/v1/accuracy/leaderboard{qs}")).await
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
    /// The SDK automatically sends an `Idempotency-Key` header required by the
    /// platform for trading writes.
    ///
    /// # Errors
    /// Returns [`PolyforgeError::Validation`] if `amount_usdc` is NaN, infinite, zero,
    /// or negative.
    pub async fn provide_liquidity(&self, params: &ProvideLiquidityParams) -> Result<LpPosition> {
        validate_financial_param("amount_usdc", params.amount_usdc)?;
        let body = serde_json::to_value(params).map_err(PolyforgeError::from)?;
        self.post_idempotent("/api/v1/lp/provide", &body).await
    }

    // -----------------------------------------------------------------------
    // Rewards
    // -----------------------------------------------------------------------

    /// List all markets that have active liquidity rewards.
    pub async fn list_rewards_markets(&self) -> Result<Vec<RewardMarket>> {
        self.get("/api/v1/rewards/markets").await
    }

    /// Get reward details for a specific market by condition ID.
    pub async fn get_rewards_for_market(&self, condition_id: &str) -> Result<RewardMarketDetail> {
        self.get(&format!("/api/v1/rewards/markets/{}", encode(condition_id)))
            .await
    }

    /// Get the authenticated user's rewards.
    pub async fn get_user_rewards(&self) -> Result<UserRewards> {
        self.get("/api/v1/rewards/user").await
    }

    /// Get the authenticated user's total accumulated rewards.
    pub async fn get_user_rewards_total(&self) -> Result<UserRewardsTotal> {
        self.get("/api/v1/rewards/user/total").await
    }

    /// Get the authenticated user's reward percentages.
    pub async fn get_user_rewards_percentages(&self) -> Result<UserRewardsPercentages> {
        self.get("/api/v1/rewards/user/percentages").await
    }

    /// Get the authenticated user's rewards broken down by market.
    pub async fn get_user_rewards_per_market(&self) -> Result<UserRewardsPerMarket> {
        self.get("/api/v1/rewards/user/markets").await
    }

    /// Get the authenticated user's trading fee rebates.
    pub async fn get_rebates(&self) -> Result<Rebates> {
        self.get("/api/v1/rewards/rebates").await
    }

    /// Get CLOB liquidity-reward details for a market by platform market ID.
    ///
    /// Returns `None` when the market has no active rewards configuration
    /// (platform returns 404).
    pub async fn get_market_rewards_detail(
        &self,
        market_id: &str,
    ) -> Result<Option<RewardsMarketDetail>> {
        let path = format!("/api/v1/rewards/market/{}", encode(market_id));
        let resp = self
            .http
            .get(self.url(&path))
            .header(AUTHORIZATION, self.auth_header()?)
            .send()
            .await?;
        if resp.status().as_u16() == 404 {
            let _ = resp.bytes().await;
            return Ok(None);
        }
        self.handle_response(resp).await.map(Some)
    }

    /// List the authenticated user's sponsored rewards markets.
    pub async fn get_user_sponsored_markets(&self) -> Result<UserSponsoredMarkets> {
        self.get("/api/v1/rewards/user/sponsored-markets").await
    }

    /// Get the Polymarket sponsor page URL for a specific market.
    pub async fn get_rewards_sponsor_url(&self, market_id: &str) -> Result<RewardsSponsorUrl> {
        self.get(&format!(
            "/api/v1/rewards/sponsor-url/{}",
            encode(market_id)
        ))
        .await
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
            let body = Self::read_error_body(resp, status).await?;
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

    async fn put<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let resp = self
            .http
            .put(self.url(path))
            .header(AUTHORIZATION, self.auth_header()?)
            .json(body)
            .send()
            .await?;
        self.handle_response(resp).await
    }

    // -----------------------------------------------------------------------
    // Cross-Venue Arbitrage
    // -----------------------------------------------------------------------

    /// List live cross-venue arbitrage opportunities.
    pub async fn list_arbitrage_opportunities(
        &self,
        min_spread: Option<f64>,
    ) -> Result<Vec<CrossVenueArbitrageOpportunity>> {
        let mut url = self.url("/api/v1/arbitrage/cross-venue");
        if let Some(ms) = min_spread {
            url = format!("{}?minSpread={}", url, ms);
        }
        let resp = self
            .http
            .get(&url)
            .header(AUTHORIZATION, self.auth_header()?)
            .send()
            .await?;
        self.handle_response(resp).await
    }

    /// Cross-venue arbitrage opportunities involving a specific market.
    pub async fn get_cross_venue_opportunities_for_market(
        &self,
        market_id: &str,
        min_spread: Option<f64>,
    ) -> Result<Vec<CrossVenueArbitrageOpportunity>> {
        let base = format!("/api/v1/arbitrage/cross-venue/{}", encode(market_id));
        let mut url = self.url(&base);
        if let Some(ms) = min_spread {
            url = format!("{}?minSpread={}", url, ms);
        }
        let resp = self
            .http
            .get(&url)
            .header(AUTHORIZATION, self.auth_header()?)
            .send()
            .await?;
        self.handle_response(resp).await
    }

    /// Get the price comparison for a specific arbitrage match.
    pub async fn get_arbitrage_comparison(&self, match_id: &str) -> Result<CrossVenueComparison> {
        let path = format!(
            "/api/v1/arbitrage/cross-venue/{}/comparison",
            encode(match_id)
        );
        self.get(&path).await
    }

    /// List all arbitrage market matches.
    pub async fn list_arbitrage_matches(&self) -> Result<Vec<ArbitrageMatch>> {
        self.get("/api/v1/arbitrage/matches").await
    }

    /// Get a single arbitrage match by ID.
    pub async fn get_market_match(&self, match_id: &str) -> Result<ArbitrageMatch> {
        let path = format!("/api/v1/arbitrage/matches/{}", encode(match_id));
        self.get(&path).await
    }

    /// List arbitrage matches for a specific market.
    pub async fn list_arbitrage_matches_for_market(
        &self,
        market_id: &str,
    ) -> Result<Vec<ArbitrageMatch>> {
        let path = format!("/api/v1/arbitrage/matches/market/{}", encode(market_id));
        self.get(&path).await
    }

    /// Admin-only: create a new arbitrage match between a Polymarket and Kalshi market.
    ///
    /// Requires an admin JWT/AdminRole on the platform. Ordinary public SDK API
    /// keys receive `403 Forbidden`.
    #[doc(hidden)]
    #[deprecated(
        note = "admin-only endpoint; requires an admin JWT/AdminRole and ordinary public API keys receive 403"
    )]
    pub async fn create_arbitrage_match(
        &self,
        params: &CreateArbitrageMatchParams,
    ) -> Result<ArbitrageMatch> {
        self.post("/api/v1/arbitrage/matches", &serde_json::to_value(params)?)
            .await
    }

    /// Admin-only: verify an arbitrage match.
    ///
    /// Requires an admin JWT/AdminRole on the platform. Ordinary public SDK API
    /// keys receive `403 Forbidden`.
    #[doc(hidden)]
    #[deprecated(
        note = "admin-only endpoint; requires an admin JWT/AdminRole and ordinary public API keys receive 403"
    )]
    pub async fn verify_arbitrage_match(&self, match_id: &str) -> Result<ArbitrageMatch> {
        let path = format!("/api/v1/arbitrage/matches/{}/verify", encode(match_id));
        self.post(&path, &json!({})).await
    }

    /// Admin-only: delete an arbitrage match.
    ///
    /// Requires an admin JWT/AdminRole on the platform. Ordinary public SDK API
    /// keys receive `403 Forbidden`.
    #[doc(hidden)]
    #[deprecated(
        note = "admin-only endpoint; requires an admin JWT/AdminRole and ordinary public API keys receive 403"
    )]
    pub async fn delete_arbitrage_match(&self, match_id: &str) -> Result<()> {
        let path = format!("/api/v1/arbitrage/matches/{}", encode(match_id));
        self.delete(&path).await
    }

    /// Admin-only: trigger a sync of arbitrage matches from external sources.
    ///
    /// Requires an admin JWT/AdminRole on the platform. Ordinary public SDK API
    /// keys receive `403 Forbidden`.
    #[doc(hidden)]
    #[deprecated(
        note = "admin-only endpoint; requires an admin JWT/AdminRole and ordinary public API keys receive 403"
    )]
    pub async fn sync_arbitrage_matches(&self) -> Result<MatchSyncResult> {
        self.post("/api/v1/arbitrage/matches/sync", &json!({}))
            .await
    }

    /// Get bid/ask spread comparison across all matched venues.
    pub async fn get_spread_comparison(&self) -> Result<Vec<SpreadSummary>> {
        self.get("/api/v1/arbitrage/spread").await
    }

    /// Get historical arbitrage opportunity snapshots.
    pub async fn get_arbitrage_history(
        &self,
        match_id: Option<&str>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<serde_json::Value> {
        let mut params = Vec::new();
        if let Some(m) = match_id {
            params.push(format!("matchId={}", encode(m)));
        }
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if let Some(o) = offset {
            params.push(format!("offset={}", o));
        }
        let mut url = self.url("/api/v1/arbitrage/history");
        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }
        let resp = self
            .http
            .get(&url)
            .header(AUTHORIZATION, self.auth_header()?)
            .send()
            .await?;
        self.handle_response(resp).await
    }

    /// List user's active arbitrage alert subscriptions.
    pub async fn get_arbitrage_alerts(&self) -> Result<Vec<ArbitrageAlertSubscription>> {
        self.get("/api/v1/arbitrage/alerts").await
    }

    /// Create an arbitrage alert subscription.
    pub async fn create_arbitrage_alert(
        &self,
        params: &CreateArbitrageAlertParams,
    ) -> Result<ArbitrageAlertSubscription> {
        self.post("/api/v1/arbitrage/alerts", &serde_json::to_value(params)?)
            .await
    }

    /// Deactivate an arbitrage alert subscription.
    pub async fn delete_arbitrage_alert(&self, alert_id: &str) -> Result<()> {
        let path = format!("/api/v1/arbitrage/alerts/{}", encode(alert_id));
        self.delete(&path).await
    }

    // -----------------------------------------------------------------------
    // Cross-Venue Arb Execution / Positions / Risk (POLA-1852)
    // -----------------------------------------------------------------------

    /// Execute a cross-venue arbitrage trade — places **real** offsetting
    /// orders on Polymarket and Kalshi for a matched market pair.
    ///
    /// Opens a new [`ArbPosition`] that starts in `OPEN` state.  The position
    /// is composed of two legs (buy on one venue, sell on the other).  To exit
    /// the position later, call
    /// [`close_arbitrage_position`](Self::close_arbitrage_position) which
    /// performs a **full sweep-close** by placing reversing market orders on
    /// both venues.  There is no partial-close for arb positions — the only
    /// exit path is a complete sweep.
    ///
    /// `idempotency_key` is sent as the `Idempotency-Key` header and is
    /// **required** by the backend for this endpoint.  The key must be 8–128
    /// characters.  Reuse the same key for safe caller-managed retries of the
    /// same intended execution; the backend guarantees at-most-once semantics
    /// per key.
    ///
    /// `match_id` must be a valid UUID (RFC 4122).  The backend validates
    /// this server-side and **returns HTTP 400** for non-UUID input.
    ///
    /// `size` must be in `1..=10000` USDC; `max_slippage_pct`, if set, must be
    /// in `0..=5`. Both are validated client-side before any order is sent.
    /// Server defaults `max_slippage_pct` to 0.5 when omitted.
    ///
    /// The backend enforces a rate limit of **5 requests per minute per user**
    /// on this endpoint; exceeding it returns **HTTP 429**.
    ///
    /// Surfaces backend error codes verbatim (`VENUES_NOT_CONNECTED`,
    /// `MATCH_NOT_FOUND`, `COMPARISON_UNAVAILABLE`, `SPREAD_TOO_LOW`,
    /// `TOKEN_RESOLUTION_FAILED`).
    pub async fn execute_arbitrage(
        &self,
        params: &ExecuteArbitrageParams,
        idempotency_key: &str,
    ) -> Result<ArbExecutionResult> {
        validate_arb_match_id(&params.match_id)?;
        validate_arb_size(params.size as f64)?;
        if let Some(slip) = params.max_slippage_pct {
            validate_arb_slippage(slip)?;
        }
        self.post_with_idempotency_key(
            "/api/v1/arbitrage/execute",
            &serde_json::to_value(params)?,
            idempotency_key,
        )
        .await
    }

    /// List the authenticated user's cross-venue arbitrage positions.
    ///
    /// `status` filters by `ArbPositionStatus`
    /// (`PENDING` | `PARTIAL` | `OPEN` | `CLOSING` | `CLOSED` | `FAILED`);
    /// `limit` defaults to 50 server-side and `offset` to 0.
    pub async fn list_arbitrage_positions(
        &self,
        status: Option<ArbPositionStatus>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<ArbPositionsResponse> {
        let mut params = Vec::new();
        if let Some(s) = status {
            params.push(format!("status={}", encode(s.as_str())));
        }
        if let Some(l) = limit {
            if !(1..=100).contains(&l) {
                return Err(PolyforgeError::Validation(format!(
                    "limit must be between 1 and 100, got {l}"
                )));
            }
            params.push(format!("limit={}", l));
        }
        if let Some(o) = offset {
            params.push(format!("offset={}", o));
        }
        let mut url = self.url("/api/v1/arbitrage/positions");
        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }
        let resp = self
            .http
            .get(&url)
            .header(AUTHORIZATION, self.auth_header()?)
            .send()
            .await?;
        self.handle_response(resp).await
    }

    /// Fetch a single arbitrage position by UUID.
    pub async fn get_arbitrage_position(&self, position_id: &str) -> Result<ArbPosition> {
        let path = format!("/api/v1/arbitrage/positions/{}", encode(position_id));
        self.get(&path).await
    }

    /// Sweep-close an open cross-venue arbitrage position — places **real**
    /// reversing market orders on **both venues** (Polymarket and Kalshi) to
    /// close the **entire** position at the best available market prices.
    ///
    /// This is always a full sweep — there is no partial-close concept for
    /// arbitrage positions.  Unlike [`close_position`](Self::close_position),
    /// which supports partial closes for regular prediction-market positions,
    /// this endpoint always reverses both legs in full.
    ///
    /// The backend transitions the position through `CLOSING` → `CLOSED` (or
    /// `FAILED` if a reverse order cannot be placed on one or both venues).
    /// On success the returned [`ArbCloseResponse`] carries the terminal
    /// `CLOSED` status.  Once closed, realised P&L is available on the
    /// full [`ArbPosition`] record fetched via
    /// [`get_arbitrage_position`](Self::get_arbitrage_position).
    ///
    /// `idempotency_key` is sent as the `Idempotency-Key` header and is
    /// **required** by the backend for this endpoint.  The key must be 8–128
    /// characters.  Reuse the same key for safe caller-managed retries of the
    /// same intended close; the backend guarantees at-most-once semantics per
    /// key.
    ///
    /// The backend enforces a rate limit of **5 requests per minute per user**
    /// on this endpoint; exceeding it returns **HTTP 429**.
    ///
    /// Surfaces backend error codes verbatim (`ARB_POSITION_NOT_FOUND`,
    /// `INVALID_STATUS`).
    pub async fn close_arbitrage_position(
        &self,
        position_id: &str,
        idempotency_key: &str,
    ) -> Result<ArbCloseResponse> {
        let path = format!("/api/v1/arbitrage/positions/{}/close", encode(position_id));
        self.post_with_idempotency_key(&path, &json!({}), idempotency_key)
            .await
    }

    /// Get net exposure, P&L, and position breakdown across venues.
    pub async fn get_arbitrage_risk_dashboard(&self) -> Result<ArbRiskDashboard> {
        self.get("/api/v1/arbitrage/risk/dashboard").await
    }

    /// List resolution-criteria mismatches between venues for open arb positions.
    pub async fn get_arbitrage_settlement_risks(&self) -> Result<Vec<ArbSettlementRisk>> {
        self.get("/api/v1/arbitrage/risk/settlement").await
    }

    /// Recompute unrealized P&L for all open arb positions.
    pub async fn refresh_arbitrage_pnl(&self) -> Result<ArbPnlRefreshResult> {
        self.post("/api/v1/arbitrage/risk/refresh-pnl", &json!({}))
            .await
    }

    // -----------------------------------------------------------------------
    // Whale Leaderboard & Alert Filter
    // -----------------------------------------------------------------------

    /// Get the whale trader leaderboard.
    pub async fn get_whale_leaderboard(&self) -> Result<Vec<WhaleLeaderboardEntry>> {
        self.get("/api/v1/whales/leaderboard").await
    }

    /// Get the authenticated user's whale alert filter settings.
    pub async fn get_whale_alert_filter(&self) -> Result<WhaleAlertFilter> {
        self.get("/api/v1/whales/alerts/filter").await
    }

    /// Replace the authenticated user's whale alert filter settings.
    pub async fn update_whale_alert_filter(
        &self,
        params: &UpdateWhaleAlertFilterParams,
    ) -> Result<WhaleAlertFilter> {
        self.put(
            "/api/v1/whales/alerts/filter",
            &serde_json::to_value(params)?,
        )
        .await
    }

    /// Delete the authenticated user's whale alert filter settings.
    pub async fn delete_whale_alert_filter(&self) -> Result<()> {
        self.delete("/api/v1/whales/alerts/filter").await
    }

    // -----------------------------------------------------------------------
    // Profile
    // -----------------------------------------------------------------------

    /// Update the authenticated user's profile.
    pub async fn update_my_profile(&self, params: &UpdateProfileParams) -> Result<UserProfile> {
        self.patch("/api/v1/profile/me", &serde_json::to_value(params)?)
            .await
    }

    /// Update the authenticated user's profile with an arbitrary body.
    ///
    /// Use this when you need to set platform fields not covered by the typed
    /// [`UpdateProfileParams`], such as `"twitterHandle"`. Construct the body
    /// by calling [`UpdateProfileParams::to_value()`] and extending it:
    ///
    /// ```ignore
    /// let mut body = UpdateProfileParams {
    ///     display_name: Some("Alice".into()),
    ///     ..Default::default()
    /// }.to_value()?;
    /// body["twitterHandle"] = serde_json::json!("@alice");
    /// client.update_my_profile_raw(&body).await?;
    /// ```
    pub async fn update_my_profile_raw(&self, body: &serde_json::Value) -> Result<UserProfile> {
        self.patch("/api/v1/profile/me", body).await
    }

    /// Change the authenticated user's password (profile route).
    pub async fn change_profile_password(&self, params: &ChangePasswordParams) -> Result<()> {
        self.post("/api/v1/profile/password", &serde_json::to_value(params)?)
            .await
    }

    /// Update the authenticated user's notification preferences (profile route).
    pub async fn update_profile_notifications(
        &self,
        params: &UpdateNotificationSettingsParams,
    ) -> Result<NotificationSettings> {
        self.patch(
            "/api/v1/profile/notifications",
            &serde_json::to_value(params)?,
        )
        .await
    }

    /// Get a user profile by username.
    ///
    /// Sends the `Authorization` header when an API key is configured;
    /// skips it when the client is constructed with an empty key so the
    /// endpoint remains usable for public read-only consumers.
    pub async fn get_user_profile(&self, username: &str) -> Result<UserProfile> {
        let path = format!("/api/v1/profile/{}", encode(username));
        self.get_with_optional_auth(&path).await
    }

    /// Follow a user by username.
    pub async fn follow_user(&self, username: &str) -> Result<()> {
        let path = format!("/api/v1/profile/{}/follow", encode(username));
        self.post(&path, &json!({})).await
    }

    // -----------------------------------------------------------------------
    // Settings
    // -----------------------------------------------------------------------

    /// Update the authenticated user's settings profile.
    pub async fn update_settings_profile(
        &self,
        params: &UpdateSettingsProfileParams,
    ) -> Result<serde_json::Value> {
        self.patch("/api/v1/settings/profile", &serde_json::to_value(params)?)
            .await
    }

    /// Get the authenticated user's notification settings.
    pub async fn get_notification_settings(&self) -> Result<NotificationSettings> {
        self.get("/api/v1/settings/notifications").await
    }

    /// Update the authenticated user's notification settings.
    pub async fn update_notification_settings(
        &self,
        params: &UpdateNotificationSettingsParams,
    ) -> Result<NotificationSettings> {
        self.patch(
            "/api/v1/settings/notifications",
            &serde_json::to_value(params)?,
        )
        .await
    }

    /// Update the authenticated user's password (settings route).
    pub async fn update_settings_password(&self, params: &UpdatePasswordParams) -> Result<()> {
        self.patch("/api/v1/settings/password", &serde_json::to_value(params)?)
            .await
    }

    /// Get the authenticated user's beta feature usage.
    pub async fn get_beta_usage(&self) -> Result<serde_json::Value> {
        self.get("/api/v1/settings/beta-usage").await
    }

    /// Get the authenticated user's gas/fee settings.
    pub async fn get_gas_settings(&self) -> Result<serde_json::Value> {
        self.get("/api/v1/settings/gas").await
    }

    // -----------------------------------------------------------------------
    // Support Tickets
    // -----------------------------------------------------------------------

    /// Create a new support ticket.
    pub async fn create_ticket(&self, params: &CreateTicketParams) -> Result<Ticket> {
        self.post("/api/v1/tickets", &serde_json::to_value(params)?)
            .await
    }

    /// List the authenticated user's support tickets (paginated).
    pub async fn list_tickets(&self) -> Result<PaginatedResponse<Ticket>> {
        self.get("/api/v1/tickets").await
    }

    /// Get a specific support ticket by ID.
    pub async fn get_ticket(&self, ticket_id: &str) -> Result<Ticket> {
        let path = format!("/api/v1/tickets/{}", encode(ticket_id));
        self.get(&path).await
    }

    /// Add a message to a support ticket.
    pub async fn add_ticket_message(&self, ticket_id: &str, body: &str) -> Result<TicketMessage> {
        let path = format!("/api/v1/tickets/{}/messages", encode(ticket_id));
        self.post(&path, &json!({ "body": body })).await
    }

    // -----------------------------------------------------------------------
    // Notification Preferences
    // -----------------------------------------------------------------------

    /// Get the authenticated user's notification preferences.
    pub async fn get_notification_preferences(&self) -> Result<NotificationPreferences> {
        self.get("/api/v1/users/me/notification-preferences").await
    }

    /// Replace the authenticated user's notification preferences.
    pub async fn update_notification_preferences(
        &self,
        params: &UpdateNotificationPreferencesParams,
    ) -> Result<NotificationPreferences> {
        self.put(
            "/api/v1/users/me/notification-preferences",
            &serde_json::to_value(params)?,
        )
        .await
    }

    // -----------------------------------------------------------------------
    // Venue Preferences (POLA-3330)
    // -----------------------------------------------------------------------

    /// Get the authenticated user's venue/platform preferences.
    pub async fn get_my_preferences(&self) -> Result<UserPreferences> {
        self.get("/api/v1/users/me/venue-preferences").await
    }

    /// Update the authenticated user's venue/platform preferences.
    ///
    /// Only the fields present in `params` are changed; omitted fields keep
    /// their current values (JSON Merge Patch semantics via HTTP PATCH).
    pub async fn update_my_preferences(
        &self,
        params: &UpdateUserPreferencesParams,
    ) -> Result<UserPreferences> {
        self.patch(
            "/api/v1/users/me/venue-preferences",
            &serde_json::to_value(params)?,
        )
        .await
    }

    // -----------------------------------------------------------------------
    // Misc public utility endpoints (POLA-1858)
    // -----------------------------------------------------------------------

    /// Get the authenticated user's accuracy overview (`GET /api/v1/accuracy`).
    ///
    /// Companion to [`Self::get_accuracy`] (`/accuracy/me`); both routes return
    /// the same shape — the controller delegates to a single service method.
    pub async fn get_accuracy_overview(&self) -> Result<AccuracyScore> {
        self.get("/api/v1/accuracy").await
    }

    /// Fetch the authenticated user's whale trade feed (`GET /api/v1/feed`).
    ///
    /// The platform reuses [`GetWhaleFeedParams`] for filter+pagination params
    /// — same controller backend (`WhalesService::getFeed`).
    pub async fn get_feed(
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
        self.get(&format!("/api/v1/feed{qs}")).await
    }

    /// List the authenticated user's order journal entries
    /// (`GET /api/v1/journal`).
    ///
    /// Optional `mood` filter accepts one of `CONFIDENT | UNCERTAIN | FOMO |
    /// DISCIPLINED | REVENGE`.
    pub async fn list_journal(
        &self,
        params: Option<&ListJournalParams>,
    ) -> Result<PaginatedResponse<JournalEntry>> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(p) = params {
            if let Some(page) = p.page {
                qp.push(("page", page.to_string()));
            }
            if let Some(limit) = p.limit {
                qp.push(("limit", limit.to_string()));
            }
            if let Some(ref mood) = p.mood {
                qp.push(("mood", mood.clone()));
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
        self.get(&format!("/api/v1/journal{qs}")).await
    }

    /// List the authenticated user's notifications
    /// (`GET /api/v1/notifications`).
    ///
    /// Distinct from [`Self::get_notification_settings`] /
    /// [`Self::get_notification_preferences`], which expose user-level
    /// preference toggles — this endpoint returns the actual notification
    /// records.
    pub async fn list_notifications(
        &self,
        params: Option<&PaginationParams>,
    ) -> Result<PaginatedResponse<Notification>> {
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
        self.get(&format!("/api/v1/notifications{qs}")).await
    }

    /// Deprecated alias for [`Self::list_notifications`].
    #[deprecated(note = "use list_notifications instead")]
    pub async fn get_notifications(
        &self,
        params: Option<&PaginationParams>,
    ) -> Result<PaginatedResponse<Notification>> {
        self.list_notifications(params).await
    }

    /// Fetch the authenticated user's referral code, link, and stats
    /// (`GET /api/v1/referrals/me`).
    pub async fn get_my_referrals(&self) -> Result<MyReferralsResponse> {
        self.get("/api/v1/referrals/me").await
    }

    /// Preview the fees a hypothetical order would pay across venues
    /// (`POST /api/v1/fees/preview`).
    ///
    /// Returns Polymarket and (when available) Kalshi fee estimates, savings,
    /// and the recommended venue. `size` and `price` are validated client-side
    /// against the server's `OrderPreviewDto` bounds (`size >= 1`,
    /// `0.001 <= price <= 0.999`).
    ///
    /// # Errors
    /// Returns [`PolyforgeError::Validation`] if `size` or `price` are NaN,
    /// infinite, or outside the platform's allowed ranges.
    pub async fn preview_fees(&self, params: &OrderPreviewParams) -> Result<OrderPreviewResponse> {
        if params.size.is_nan() || params.size.is_infinite() || params.size < 1.0 {
            return Err(PolyforgeError::Validation(format!(
                "size must be a finite number >= 1, got {}",
                params.size
            )));
        }
        if params.price.is_nan()
            || params.price.is_infinite()
            || !(0.001..=0.999).contains(&params.price)
        {
            return Err(PolyforgeError::Validation(format!(
                "price must be between 0.001 and 0.999, got {}",
                params.price
            )));
        }
        self.post("/api/v1/fees/preview", &serde_json::to_value(params)?)
            .await
    }

    /// List the active fee schedules for all supported venues
    /// (`GET /api/v1/fees/schedules`).
    pub async fn list_fee_schedules(&self) -> Result<FeeSchedules> {
        self.get("/api/v1/fees/schedules").await
    }

    /// Backward-compatible alias for [`Self::list_fee_schedules`].
    pub async fn get_fee_schedules(&self) -> Result<FeeSchedules> {
        self.list_fee_schedules().await
    }

    /// List per-market price alerts the authenticated user has configured for
    /// a single market (`GET /api/v1/markets/:marketId/alerts`).
    ///
    /// Distinct from [`Self::list_alerts`], which lists top-level token-based
    /// alerts.
    pub async fn list_market_alerts(&self, market_id: &str) -> Result<MarketAlertsResponse> {
        self.get(&format!("/api/v1/markets/{}/alerts", encode(market_id)))
            .await
    }

    /// Create a per-market price alert
    /// (`POST /api/v1/markets/:marketId/alerts`).
    ///
    /// `params.threshold` is validated server-side to
    /// `0.01 <= threshold <= 0.99`.
    pub async fn create_market_alert(
        &self,
        market_id: &str,
        params: &CreateMarketAlertParams,
    ) -> Result<MarketAlert> {
        self.post(
            &format!("/api/v1/markets/{}/alerts", encode(market_id)),
            &serde_json::to_value(params)?,
        )
        .await
    }

    /// Delete a per-market price alert
    /// (`DELETE /api/v1/markets/:marketId/alerts/:alertId`).
    ///
    /// `alert_id` must be a UUID — the server applies `ParseUUIDPipe`.
    pub async fn delete_market_alert(&self, market_id: &str, alert_id: &str) -> Result<()> {
        self.delete(&format!(
            "/api/v1/markets/{}/alerts/{}",
            encode(market_id),
            encode(alert_id)
        ))
        .await
    }

    /// Get aggregated price/volume history for a single market
    /// (`GET /api/v1/markets/:marketId/history`).
    ///
    /// Server defaults `period` to `7d` when omitted.
    pub async fn get_market_history(
        &self,
        market_id: &str,
        period: Option<MarketHistoryPeriod>,
    ) -> Result<serde_json::Value> {
        let path = match period {
            Some(p) => format!(
                "/api/v1/markets/{}/history?period={}",
                encode(market_id),
                p.as_str()
            ),
            None => format!("/api/v1/markets/{}/history", encode(market_id)),
        };
        self.get(&path).await
    }

    /// Get the markets-controller sentiment report for a single market
    /// (`GET /api/v1/markets/:marketId/sentiment`).
    ///
    /// Distinct from [`Self::get_market_sentiment`], which calls the
    /// news-derived `/news/sentiment/:marketId` endpoint.
    pub async fn get_market_sentiment_report(
        &self,
        market_id: &str,
    ) -> Result<MarketSentimentReport> {
        self.get(&format!("/api/v1/markets/{}/sentiment", encode(market_id)))
            .await
    }

    /// Submit a sentiment vote for a single market
    /// (`POST /api/v1/markets/:marketId/sentiment`).
    ///
    /// Server returns the same sentiment report shape as the GET variant.
    pub async fn vote_market_sentiment(
        &self,
        market_id: &str,
        params: &VoteMarketSentimentParams,
    ) -> Result<MarketSentimentReport> {
        self.post(
            &format!("/api/v1/markets/{}/sentiment", encode(market_id)),
            &serde_json::to_value(params)?,
        )
        .await
    }

    /// Attach or update the journal note + mood on one of the user's orders
    /// (`PATCH /api/v1/orders/:id/journal`).
    pub async fn update_order_journal(
        &self,
        order_id: &str,
        params: &UpdateOrderJournalParams,
    ) -> Result<JournalEntry> {
        self.patch(
            &format!("/api/v1/orders/{}/journal", encode(order_id)),
            &serde_json::to_value(params)?,
        )
        .await
    }

    /// List Kalshi combo-market collections, optionally filtered by series and
    /// paged via cursor (`GET /api/v1/markets/combo/collections`).
    pub async fn list_combo_collections(
        &self,
        params: Option<&ListComboCollectionsParams>,
    ) -> Result<serde_json::Value> {
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(p) = params {
            if let Some(ref series) = p.series_ticker {
                qp.push(("seriesTicker", series.clone()));
            }
            if let Some(limit) = p.limit {
                qp.push(("limit", limit.to_string()));
            }
            if let Some(ref cursor) = p.cursor {
                qp.push(("cursor", cursor.clone()));
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
        self.get(&format!("/api/v1/markets/combo/collections{qs}"))
            .await
    }

    /// Fetch a single Kalshi combo-market collection by ticker
    /// (`GET /api/v1/markets/combo/collections/:ticker`).
    pub async fn get_combo_collection(&self, ticker: &str) -> Result<ComboCollection> {
        self.get(&format!(
            "/api/v1/markets/combo/collections/{}",
            encode(ticker)
        ))
        .await
    }

    /// Look up the combo market that matches a collection ticker plus the
    /// requested leg outcomes (`POST /api/v1/markets/combo/lookup`).
    pub async fn lookup_combo_market(
        &self,
        params: &ComboLookupParams,
    ) -> Result<serde_json::Value> {
        self.post(
            "/api/v1/markets/combo/lookup",
            &serde_json::to_value(params)?,
        )
        .await
    }

    /// Backward-compatible alias for [`Self::lookup_combo_market`].
    pub async fn lookup_combo_ticker(
        &self,
        params: &ComboLookupParams,
    ) -> Result<serde_json::Value> {
        self.lookup_combo_market(params).await
    }

    /// Get the cross-category correlation matrix
    /// (`GET /api/v1/analytics/correlation/categories`).
    ///
    /// Returns the top 20 market categories ordered by market count plus a
    /// symmetric correlation matrix derived from category volumes and counts.
    pub async fn get_correlation_categories(&self) -> Result<CategoryCorrelation> {
        self.get("/api/v1/analytics/correlation/categories").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;

    async fn capture_request<F, Fut>(response_body: &'static str, invoke: F) -> String
    where
        F: FnOnce(PolyforgeClient) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                let n = socket.read(&mut chunk).await.unwrap();
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }

            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                 content-type: application/json\r\n\
                 content-length: {}\r\n\
                 connection: close\r\n\
                 \r\n\
                 {}",
                response_body.len(),
                response_body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8(buf).unwrap()
        });

        let client = PolyforgeClient::with_url("test-key", format!("http://{addr}")).unwrap();
        invoke(client).await.unwrap();
        server.await.unwrap()
    }

    fn captured_header<'a>(request: &'a str, name: &str) -> Option<&'a str> {
        request.lines().find_map(|line| {
            let (header_name, value) = line.split_once(':')?;
            header_name
                .eq_ignore_ascii_case(name)
                .then_some(value.trim())
        })
    }

    fn assert_generated_idempotency_key(request: &str) {
        let key = captured_header(request, "Idempotency-Key")
            .expect("request must include Idempotency-Key header");
        assert_eq!(key.len(), 32);
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test]
    async fn test_place_order_sends_generated_idempotency_key() {
        let request = capture_request(
            r#"{"orderId":"order-1","intentId":"intent-1","status":"PENDING"}"#,
            |client| async move {
                let params = PlaceOrderParams {
                    token_id: "tok-1".into(),
                    side: "BUY".into(),
                    outcome: "YES".into(),
                    size: 10.0,
                    price: 0.55,
                    order_type: Some("GTC".into()),
                };
                client.place_order(&params).await.map(|_| ())
            },
        )
        .await;

        assert_generated_idempotency_key(&request);
    }

    #[tokio::test]
    async fn test_cancel_order_sends_generated_idempotency_key() {
        let request = capture_request(
            r#"{"orderId":"order-1","status":"CANCELLED"}"#,
            |client| async move { client.cancel_order("order-1").await.map(|_| ()) },
        )
        .await;

        assert_generated_idempotency_key(&request);
    }

    #[tokio::test]
    async fn test_bulk_cancel_orders_sends_generated_idempotency_key() {
        let request = capture_request(
            r#"{"cancelled":["order-1"],"failed":[]}"#,
            |client| async move {
                let params = BulkCancelParams {
                    order_ids: vec!["order-1".into()],
                };
                client.bulk_cancel_orders(&params).await.map(|_| ())
            },
        )
        .await;

        assert_generated_idempotency_key(&request);
    }

    #[tokio::test]
    async fn test_create_conditional_order_sends_generated_idempotency_key() {
        let request = capture_request(r#"{"id":"conditional-1"}"#, |client| async move {
            let params = CreateConditionalOrderParams {
                market_id: "mkt-1".into(),
                token_id: "tok-1".into(),
                order_type: "LIMIT".into(),
                side: "BUY".into(),
                outcome: "YES".into(),
                size: 10.0,
                trigger_price: 0.55,
                limit_price: Some("0.56".into()),
                trailing_pct: None,
                expires_at: None,
            };
            client.create_conditional_order(&params).await.map(|_| ())
        })
        .await;

        assert_generated_idempotency_key(&request);
    }

    #[tokio::test]
    async fn test_provide_liquidity_sends_generated_idempotency_key() {
        let request = capture_request(
            r#"{"buyOrderId":"buy-1","sellOrderId":"sell-1","tokenId":"tok-1","buyPrice":"0.49","sellPrice":"0.51","size":"100"}"#,
            |client| async move {
                let params = ProvideLiquidityParams {
                    market_id: "mkt-1".into(),
                    token_id: "tok-1".into(),
                    amount_usdc: 100.0,
                    target_spread: Some(0.02),
                };
                client.provide_liquidity(&params).await.map(|_| ())
            },
        )
        .await;

        assert!(request.starts_with("POST /api/v1/lp/provide "));
        assert_generated_idempotency_key(&request);
    }

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
    fn test_client_optional_auth_header_omits_empty_key() {
        let client = PolyforgeClient::new("").unwrap();
        assert!(client.optional_auth_header().unwrap().is_none());
    }

    #[tokio::test]
    async fn test_public_user_endpoints_dispatch_without_api_key() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            for _ in 0..7 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = vec![0_u8; 4096];
                let n = socket.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..n]);
                assert!(
                    !request.to_ascii_lowercase().contains("authorization:"),
                    "public endpoint request unexpectedly included Authorization header: {request}"
                );
                let body = if request.contains("/api/v1/scores/") && request.contains("/badges") {
                    r#"[]"#
                } else if request.contains("/api/v1/scores/")
                    || request.contains("/api/v1/profile/")
                {
                    r#"{}"#
                } else if request.contains("/api/v1/actions") {
                    r#"{"version":"1.0","actions":[]}"#
                } else {
                    r#"{"data":[]}"#
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\n\
                     content-type: application/json\r\n\
                     content-length: {}\r\n\
                     connection: close\r\n\
                     \r\n\
                     {}",
                    body.len(),
                    body
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });

        let client = PolyforgeClient::with_url("", format!("http://{addr}")).unwrap();
        client.get_user_score("alice").await.unwrap();
        client.get_user_badges("alice").await.unwrap();
        client.get_user_performance("alice", "30d").await.unwrap();
        client
            .get_user_strategies("alice", None, None)
            .await
            .unwrap();
        client.get_user_activity("alice", None).await.unwrap();
        client.get_user_profile_badges("alice").await.unwrap();
        client.get_actions().await.unwrap();

        server.await.unwrap();
    }

    #[tokio::test]
    async fn test_get_actions_no_auth_header_with_empty_key() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0_u8; 4096];
            let n = socket.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..n]);

            assert!(
                !request.to_ascii_lowercase().contains("authorization:"),
                "get_actions with empty API key must not send Authorization header: {request}"
            );

            let body = r#"{"version":"1.0","actions":[]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                 content-type: application/json\r\n\
                 content-length: {}\r\n\
                 connection: close\r\n\
                 \r\n\
                 {}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        let client = PolyforgeClient::with_url("", format!("http://{addr}")).unwrap();
        let actions = client.get_actions().await.unwrap();
        assert_eq!(actions.version, "1.0");
        assert!(actions.actions.is_empty());

        server.await.unwrap();
    }

    #[tokio::test]
    async fn test_protected_user_endpoints_send_authorization_header() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            for _ in 0..4 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = vec![0_u8; 4096];
                let n = socket.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..n]);
                assert!(
                    request.to_ascii_lowercase().contains("authorization:"),
                    "protected user endpoint request missing Authorization header: {request}"
                );

                let body = if request.contains("/badges") {
                    "[]"
                } else if request.contains("/scores/") || request.contains("/profile/") {
                    "{}"
                } else if request.contains("/actions") {
                    r#"{"version":"1.0","actions":[]}"#
                } else {
                    "{}"
                };

                let response = format!(
                    "HTTP/1.1 200 OK\r\n\
                     content-type: application/json\r\n\
                     content-length: {}\r\n\
                     connection: close\r\n\
                     \r\n\
                     {}",
                    body.len(),
                    body
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });

        let client = PolyforgeClient::with_url("test-api-key", format!("http://{addr}")).unwrap();
        client.get_user_score("alice").await.unwrap();
        client.get_user_badges("alice").await.unwrap();
        client.get_user_profile("alice").await.unwrap();
        client.get_actions().await.unwrap();

        server.await.unwrap();
    }

    #[tokio::test]
    async fn test_user_score_and_profile_endpoints_send_auth() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            for i in 0..3 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = vec![0_u8; 4096];
                let n = socket.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..n]);
                assert!(
                    request.to_ascii_lowercase().contains("authorization:"),
                    "user profile request missing Authorization header (request {}): {request}",
                    i
                );
                let body = match i {
                    0 => "{}",
                    1 => "[]",
                    2 => "{}",
                    _ => unreachable!(),
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\n\
                     content-type: application/json\r\n\
                     content-length: {}\r\n\
                     connection: close\r\n\
                     \r\n\
                     {}",
                    body.len(),
                    body
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });

        let client = PolyforgeClient::with_url("test-key", format!("http://{addr}")).unwrap();
        client.get_user_score("alice").await.unwrap();
        client.get_user_badges("alice").await.unwrap();
        client.get_user_profile("alice").await.unwrap();

        server.await.unwrap();
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

    #[tokio::test]
    async fn test_watch_strategy_rejects_oversized_error_body_without_content_length() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let oversized = "x".repeat(MAX_RESPONSE_BODY_SIZE + 1);

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 1024];
            let _ = socket.read(&mut request).await;
            socket
                .write_all(
                    b"HTTP/1.1 500 Internal Server Error\r\n\
                      content-type: application/json\r\n\
                      connection: close\r\n\
                      \r\n",
                )
                .await
                .unwrap();
            socket.write_all(oversized.as_bytes()).await.unwrap();
        });

        let client = PolyforgeClient::with_url("test-key", format!("http://{addr}")).unwrap();
        let result = client.watch_strategy("strategy-1").await;

        match result {
            Err(PolyforgeError::Api { code, status, .. }) => {
                assert_eq!(code, "RESPONSE_BODY_TOO_LARGE");
                assert_eq!(status, 500);
            }
            Ok(_) => panic!("Expected Api error with RESPONSE_BODY_TOO_LARGE, got stream"),
            Err(other) => panic!(
                "Expected Api error with RESPONSE_BODY_TOO_LARGE, got: {:?}",
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
    async fn test_handle_response_rejects_oversized_error_body_without_content_length() {
        let body = http::Response::builder()
            .status(500)
            .header("content-type", "application/json")
            .body("x".repeat(MAX_RESPONSE_BODY_SIZE + 1))
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
    async fn test_handle_response_rejects_multichunk_oversized_error_body_without_content_length() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let first_chunk = vec![b'x'; 700 * 1024];
        let second_chunk = vec![b'y'; 700 * 1024];

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 1024];
            let _ = socket.read(&mut request).await;
            socket
                .write_all(
                    b"HTTP/1.1 500 Internal Server Error\r\n\
                      content-type: application/json\r\n\
                      connection: close\r\n\
                      \r\n",
                )
                .await
                .unwrap();
            socket.write_all(&first_chunk).await.unwrap();
            socket.write_all(&second_chunk).await.unwrap();
        });

        let client = PolyforgeClient::with_url("test-key", format!("http://{addr}")).unwrap();
        let result: std::result::Result<serde_json::Value, _> =
            client.get("/multi-chunk-error").await;

        match result {
            Err(PolyforgeError::Api {
                code,
                status,
                message,
                ..
            }) => {
                assert_eq!(code, "RESPONSE_BODY_TOO_LARGE");
                assert_eq!(status, 500);
                assert!(
                    message.contains("limit 1048576"),
                    "message should include the exact truncation limit: {message}"
                );
            }
            Ok(_) => panic!("Expected Api error with RESPONSE_BODY_TOO_LARGE"),
            Err(other) => panic!(
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

    // --- Platform contract compliance regression tests (#89-#92, #145) ---

    #[test]
    fn test_trading_mode_serializes_lowercase() {
        // #92: TradingMode is still used when *reading* Strategy.mode from responses.
        let live = serde_json::to_value(TradingMode::Live).unwrap();
        assert_eq!(live, serde_json::Value::String("live".to_string()));
        let paper = serde_json::to_value(TradingMode::Paper).unwrap();
        assert_eq!(paper, serde_json::Value::String("paper".to_string()));
    }

    #[test]
    fn test_start_strategy_paper_payload() {
        // #173: start_strategy must send { mode: "paper" } to match platform contract
        let body = serde_json::to_value(StartStrategyParams::paper()).unwrap();
        assert_eq!(body["mode"], serde_json::Value::String("paper".to_string()));
        assert!(
            body.get("paperMode").is_none(),
            "must not send obsolete `paperMode` field"
        );
    }

    #[test]
    fn test_start_strategy_live_payload() {
        // #173: start_strategy must send { mode: "live" } to match platform contract
        let body = serde_json::to_value(StartStrategyParams::live()).unwrap();
        assert_eq!(body["mode"], serde_json::Value::String("live".to_string()));
        assert!(
            body.get("paperMode").is_none(),
            "must not send obsolete `paperMode` field"
        );
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
    fn test_known_strategy_event_types_list() {
        // #214: Verify all 16 known event types (including the 6 that
        // were previously missing) are listed in the constant.
        assert!(KNOWN_STRATEGY_EVENT_TYPES.contains(&"STRATEGY_PAUSED"));
        assert!(KNOWN_STRATEGY_EVENT_TYPES.contains(&"STRATEGY_RESUMED"));
        assert!(KNOWN_STRATEGY_EVENT_TYPES.contains(&"ORDER_SUBMITTED"));
        assert!(KNOWN_STRATEGY_EVENT_TYPES.contains(&"ORDER_PARTIAL"));
        assert!(KNOWN_STRATEGY_EVENT_TYPES.contains(&"ORDER_FAILED"));
        assert!(KNOWN_STRATEGY_EVENT_TYPES.contains(&"ORDER_ERROR"));
        assert!(KNOWN_STRATEGY_EVENT_TYPES.contains(&"CONNECTED"));
        assert!(KNOWN_STRATEGY_EVENT_TYPES.contains(&"STRATEGY_STARTED"));
        assert!(KNOWN_STRATEGY_EVENT_TYPES.contains(&"STRATEGY_STOPPED"));
        assert!(KNOWN_STRATEGY_EVENT_TYPES.contains(&"STRATEGY_ERROR"));
        assert!(KNOWN_STRATEGY_EVENT_TYPES.contains(&"ORDER_PLACED"));
        assert!(KNOWN_STRATEGY_EVENT_TYPES.contains(&"ORDER_FILLED"));
        assert!(KNOWN_STRATEGY_EVENT_TYPES.contains(&"ORDER_CANCELLED"));
        assert!(KNOWN_STRATEGY_EVENT_TYPES.contains(&"BACKTEST_PROGRESS"));
        assert!(KNOWN_STRATEGY_EVENT_TYPES.contains(&"BACKTEST_COMPLETED"));
        assert!(KNOWN_STRATEGY_EVENT_TYPES.contains(&"BACKTEST_FAILED"));
        assert_eq!(KNOWN_STRATEGY_EVENT_TYPES.len(), 16);
    }

    #[test]
    fn test_strategy_event_deserializes_known_types() {
        // #214: Ensure StrategyEvent deserializes correctly for the 6
        // event types that were previously undocumented.
        let json = serde_json::json!({
            "type": "STRATEGY_PAUSED",
            "strategyId": "strat-1",
            "data": null,
            "timestamp": 1715000000000_u64
        });
        let event: crate::types::StrategyEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.event_type, "STRATEGY_PAUSED");
        assert_eq!(event.strategy_id.as_deref(), Some("strat-1"));
        assert_eq!(event.timestamp, 1715000000000);
    }

    #[test]
    fn test_strategy_event_deserializes_order_submitted() {
        let json = serde_json::json!({
            "type": "ORDER_SUBMITTED",
            "strategyId": "strat-2",
            "data": {"orderId": "ord-123", "side": "BUY"},
            "timestamp": 1715000001000_u64
        });
        let event: crate::types::StrategyEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.event_type, "ORDER_SUBMITTED");
        assert_eq!(event.data["orderId"], "ord-123");
        assert_eq!(event.data["side"], "BUY");
    }

    #[test]
    fn test_strategy_event_deserializes_strategy_resumed() {
        let json = serde_json::json!({
            "type": "STRATEGY_RESUMED",
            "strategyId": "strat-3",
            "data": null,
            "timestamp": 1715000002000_u64
        });
        let event: crate::types::StrategyEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.event_type, "STRATEGY_RESUMED");
        assert_eq!(event.strategy_id.as_deref(), Some("strat-3"));
        assert_eq!(event.timestamp, 1715000002000);
    }

    #[test]
    fn test_strategy_event_deserializes_order_partial() {
        let json = serde_json::json!({
            "type": "ORDER_PARTIAL",
            "strategyId": "strat-4",
            "data": {"orderId": "ord-456", "side": "SELL", "filledSize": "50", "remainingSize": "50"},
            "timestamp": 1715000003000_u64
        });
        let event: crate::types::StrategyEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.event_type, "ORDER_PARTIAL");
        assert_eq!(event.data["orderId"], "ord-456");
        assert_eq!(event.data["side"], "SELL");
    }

    #[test]
    fn test_strategy_event_deserializes_order_failed() {
        let json = serde_json::json!({
            "type": "ORDER_FAILED",
            "strategyId": "strat-5",
            "data": {"orderId": "ord-789", "reason": "insufficient_funds"},
            "timestamp": 1715000004000_u64
        });
        let event: crate::types::StrategyEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.event_type, "ORDER_FAILED");
        assert_eq!(event.data["orderId"], "ord-789");
        assert_eq!(event.data["reason"], "insufficient_funds");
    }

    #[test]
    fn test_strategy_event_deserializes_order_error() {
        let json = serde_json::json!({
            "type": "ORDER_ERROR",
            "strategyId": "strat-6",
            "data": {"orderId": "ord-000", "error": "exchange_timeout"},
            "timestamp": 1715000005000_u64
        });
        let event: crate::types::StrategyEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.event_type, "ORDER_ERROR");
        assert_eq!(event.data["orderId"], "ord-000");
        assert_eq!(event.data["error"], "exchange_timeout");
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
    fn test_batch_orders_validation_rejects_invalid_size_values() {
        let invalid_values = [f64::NAN, f64::INFINITY, 0.0, -1.0];
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = PolyforgeClient::with_url("test-key", "http://127.0.0.1:9").unwrap();

        for size in invalid_values {
            let params = BatchOrdersParams {
                orders: vec![PlaceOrderParams {
                    token_id: "t1".into(),
                    side: "BUY".into(),
                    outcome: "YES".into(),
                    size,
                    price: 0.5,
                    order_type: None,
                }],
            };
            let err = rt.block_on(client.batch_orders(&params)).unwrap_err();
            assert!(matches!(err, PolyforgeError::Validation(_)));
            assert!(err.to_string().contains("orders[0].size"));
        }
    }

    #[test]
    fn test_batch_orders_validation_rejects_invalid_price_values() {
        let invalid_values = [f64::NAN, f64::INFINITY, 0.0, -1.0];
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = PolyforgeClient::with_url("test-key", "http://127.0.0.1:9").unwrap();

        for price in invalid_values {
            let params = BatchOrdersParams {
                orders: vec![PlaceOrderParams {
                    token_id: "t1".into(),
                    side: "BUY".into(),
                    outcome: "YES".into(),
                    size: 10.0,
                    price,
                    order_type: None,
                }],
            };
            let err = rt.block_on(client.batch_orders(&params)).unwrap_err();
            assert!(matches!(err, PolyforgeError::Validation(_)));
            assert!(err.to_string().contains("orders[0].price"));
        }
    }

    #[test]
    fn test_place_order_params_omits_market_id() {
        let params = PlaceOrderParams {
            token_id: "tok-1".into(),
            side: "BUY".into(),
            outcome: "YES".into(),
            size: 25.0,
            price: 0.65,
            order_type: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["tokenId"], "tok-1");
        assert!(
            json.get("marketId").is_none(),
            "platform derives marketId from tokenId and rejects explicit marketId"
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
        let json = r#"{
            "data": [{"id":"s1","name":"Alpha"},{"id":"s2","name":"Beta"}],
            "total": 2, "page": 1, "limit": 10, "totalPages": 1, "hasNext": true
        }"#;
        let resp: PaginatedResponse<Strategy> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 2);
        assert_eq!(resp.total, 2);
        assert_eq!(resp.page, 1);
        assert!(resp.has_next);
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
            kalshi_subaccount: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["name"], "My Strategy");
        assert_eq!(json["visibility"], "PUBLIC");
        assert_eq!(json["execMode"], "TICK");
        assert_eq!(json["tickMs"], 5000);
        assert!(json["triggers"].is_array());
        assert!(json["tags"].is_array());
        // logicBlocks, calcBlocks, and kalshiSubaccount omitted when None
        assert!(json.get("logicBlocks").is_none());
        assert!(json.get("kalshiSubaccount").is_none());
    }

    #[test]
    fn test_create_strategy_params_kalshi_subaccount_serializes() {
        let params = CreateStrategyParams {
            name: "Test".into(),
            kalshi_subaccount: Some(42),
            ..Default::default()
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["kalshiSubaccount"], 42);
    }

    #[test]
    fn test_create_strategy_params_omits_kalshi_subaccount_when_none() {
        let params = CreateStrategyParams::new("S");
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["name"], "S");
        assert!(json.get("kalshiSubaccount").is_none());
    }

    #[test]
    fn test_copy_config_deserializes_platform_fields() {
        // #168: CopyConfig must use wallet-based model (targetWallet, mode, sizeValue, etc.)
        let json = r#"{
            "id": "cc1",
            "targetWallet": "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
            "mode": "PERCENTAGE",
            "sizeValue": "100.5",
            "maxExposure": "500",
            "maxDailyLoss": "50",
            "priceOffset": "0.01",
            "status": "ACTIVE",
            "createdAt": "2026-01-01T00:00:00Z"
        }"#;
        let config: CopyConfig = serde_json::from_str(json).unwrap();
        assert_eq!(
            config.target_wallet.as_deref(),
            Some("0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef")
        );
        assert_eq!(config.mode, Some(CopyMode::Percentage));
        assert_eq!(config.size_value.as_deref(), Some("100.5"));
        assert_eq!(config.max_exposure.as_deref(), Some("500"));
        assert_eq!(config.max_daily_loss.as_deref(), Some("50"));
        assert_eq!(config.price_offset.as_deref(), Some("0.01"));
        assert_eq!(config.status.as_deref(), Some("ACTIVE"));
        assert_eq!(config.created_at.as_deref(), Some("2026-01-01T00:00:00Z"));
    }

    #[test]
    fn test_copy_config_strategy_fields_go_to_extra() {
        // #168: Old strategy-based fields must NOT map to named fields
        let json = r#"{"id":"cc1","sourceStrategyId":"strat-uuid","allocationPercent":50}"#;
        let config: CopyConfig = serde_json::from_str(json).unwrap();
        assert!(config.target_wallet.is_none());
        assert!(config.mode.is_none());
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
            position_id: Some("pos-123".into()),
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
            position_id: Some("pos-123".into()),
            market_id: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["positionId"], "pos-123");
        assert!(json.get("marketId").is_none());
    }

    #[test]
    fn test_redeem_position_params_position_id_omitted_when_none() {
        // #213: positionId should be omitted when None — marketId-only redemption
        let params = RedeemPositionParams {
            position_id: None,
            market_id: Some("mkt-456".into()),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert!(json.get("positionId").is_none());
        assert_eq!(json["marketId"], "mkt-456");
    }

    #[test]
    fn test_redeem_position_params_neither_field_is_empty_body() {
        // #213: omitting both fields produces an empty JSON object
        let params = RedeemPositionParams {
            position_id: None,
            market_id: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert!(json.as_object().unwrap().is_empty());
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
    fn test_conditional_order_deserializes_type_field() {
        // #250: Platform returns 'type' not 'conditionType' for condition_type
        let json = r#"{
            "id": "co-3",
            "tokenId": "tok-xyz",
            "side": "SELL",
            "outcome": "NO",
            "size": "200",
            "triggerPrice": "0.80",
            "limitPrice": "0.78",
            "type": "STOP_LOSS",
            "status": "PENDING",
            "createdAt": "2026-05-01T10:00:00Z",
            "triggeredAt": null,
            "expiresAt": "2026-05-10T10:00:00Z"
        }"#;
        let co: ConditionalOrder = serde_json::from_str(json).unwrap();
        assert_eq!(co.id, "co-3");
        assert_eq!(co.condition_type.as_deref(), Some("STOP_LOSS"));
        assert_eq!(co.status, Some(ConditionalOrderStatus::Pending));
        assert_eq!(co.limit_price.as_deref(), Some("0.78"));
        assert_eq!(co.trigger_price.as_deref(), Some("0.80"));
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
    fn test_smoke() {
        let _client = PolyforgeClient::new("test-api-key").unwrap();
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
            limit_price: Some("0.67".into()),
            trailing_pct: None,
            expires_at: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["marketId"], "mkt-1");
        assert_eq!(json["tokenId"], "tok-1");
        assert_eq!(json["type"], "STOP_LOSS");
        assert_eq!(json["triggerPrice"], 0.65);
        assert_eq!(json["limitPrice"], "0.67");
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
    fn test_create_conditional_order_validation_rejects_invalid_limit_price_string() {
        let params = CreateConditionalOrderParams {
            market_id: "mkt-1".into(),
            token_id: "tok-1".into(),
            order_type: "LIMIT".into(),
            side: "BUY".into(),
            outcome: "YES".into(),
            size: 10.0,
            trigger_price: 0.5,
            limit_price: Some("not-a-number".into()),
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
            price: "0.75".into(),
            persistent: Some(true),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["tokenId"], "tok-1");
        assert_eq!(json["direction"], "above");
        assert_eq!(json["price"], "0.75");
        assert_eq!(json["persistent"], true);
    }

    #[test]
    fn test_create_alert_params_omits_none_persistent() {
        let params = CreateAlertParams {
            token_id: "tok-1".into(),
            direction: AlertDirection::Below,
            price: "0.25".into(),
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
            price: "0.0".into(),
            persistent: None,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = PolyforgeClient::new("test-key").unwrap();
        let err = rt.block_on(client.create_alert(&params)).unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
        assert!(err.to_string().contains("price"));
    }

    #[test]
    fn test_create_alert_validation_rejects_non_numeric_price() {
        let params = CreateAlertParams {
            token_id: "tok-1".into(),
            direction: AlertDirection::Above,
            price: "not-a-number".into(),
            persistent: None,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = PolyforgeClient::new("test-key").unwrap();
        let err = rt.block_on(client.create_alert(&params)).unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
        assert!(err.to_string().contains("numeric string"));
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
            "total": 2, "page": 1, "limit": 10, "totalPages": 1, "hasNext": false
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
    fn test_create_copy_config_params_serializes_wallet_fields() {
        // #168: CreateCopyConfigParams must send targetWallet, not sourceStrategyId
        let params = CreateCopyConfigParams {
            target_wallet: "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string(),
            mode: Some(CopyMode::Percentage),
            size_value: Some("100".to_string()),
            ..Default::default()
        };
        let body = serde_json::to_value(&params).unwrap();
        assert_eq!(
            body["targetWallet"],
            "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
        );
        assert_eq!(body["mode"], "PERCENTAGE");
        assert_eq!(body["sizeValue"], "100");
        assert!(body.get("sourceStrategyId").is_none());
        assert!(body.get("allocationPercent").is_none());
    }

    #[test]
    fn test_update_copy_config_params_skips_none_fields() {
        let params = UpdateCopyConfigParams {
            mode: Some(CopyMode::Fixed),
            size_value: Some("200".to_string()),
            ..Default::default()
        };
        let body = serde_json::to_value(&params).unwrap();
        assert_eq!(body["mode"], "FIXED");
        assert_eq!(body["sizeValue"], "200");
        // None fields should be absent due to skip_serializing_if
        assert!(body.get("maxExposure").is_none());
        assert!(body.get("maxDailyLoss").is_none());
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

    // ── Risk Settings (#147) ─────────────────────────────────────────────────

    #[test]
    fn test_risk_settings_deserializes_full() {
        let json = r#"{
            "drawdownEnabled": false,
            "drawdownLookbackHours": 72,
            "drawdownThresholdPct": 0.15,
            "circuitBreakerTripped": false,
            "circuitBreakerTrippedAt": null
        }"#;
        let rs: RiskSettings = serde_json::from_str(json).unwrap();
        assert!(!rs.drawdown_enabled);
        assert_eq!(rs.drawdown_lookback_hours, 72);
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
        assert!(rs.circuit_breaker_tripped_at.is_none());
    }

    #[test]
    fn test_risk_settings_circuit_breaker_triggered() {
        let json = r#"{
            "drawdownEnabled": true,
            "drawdownLookbackHours": 48,
            "drawdownThresholdPct": 0.05,
            "circuitBreakerTripped": true,
            "circuitBreakerTrippedAt": "2025-01-15T12:00:00Z"
        }"#;
        let rs: RiskSettings = serde_json::from_str(json).unwrap();
        assert!(rs.circuit_breaker_tripped);
        assert_eq!(
            rs.circuit_breaker_tripped_at.as_deref(),
            Some("2025-01-15T12:00:00Z")
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
            drawdown_enabled: Some(true),
            drawdown_lookback_hours: Some(96),
            drawdown_threshold_pct: Some(0.25),
        };
        let body = serde_json::to_value(&params).unwrap();
        assert_eq!(body["drawdownEnabled"], true);
        assert_eq!(body["drawdownLookbackHours"], 96);
        assert!((body["drawdownThresholdPct"].as_f64().unwrap() - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn test_update_risk_settings_params_default_is_empty() {
        let params = UpdateRiskSettingsParams::default();
        let body = serde_json::to_value(&params).unwrap();
        assert!(body.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_risk_settings_default_values() {
        let rs = RiskSettings::default();
        assert!(!rs.drawdown_enabled);
        assert_eq!(rs.drawdown_lookback_hours, 24);
        assert!((rs.drawdown_threshold_pct - 0.1).abs() < f64::EPSILON);
        assert!(!rs.circuit_breaker_tripped);
        assert!(rs.circuit_breaker_tripped_at.is_none());
    }

    #[test]
    fn test_risk_settings_deprecated_circuit_breaker_triggered() {
        // Deprecated accessor delegates to circuit_breaker_tripped.
        let json = r#"{
            "drawdownEnabled": false,
            "drawdownLookbackHours": 24,
            "drawdownThresholdPct": 0.1,
            "circuitBreakerTripped": true
        }"#;
        let rs: RiskSettings = serde_json::from_str(json).unwrap();
        #[allow(deprecated)]
        let triggered = rs.circuit_breaker_triggered();
        assert!(triggered);
        assert_eq!(triggered, rs.circuit_breaker_tripped);
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

    // -----------------------------------------------------------------------
    // POLA-355: 17 new endpoint type-deserialization tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_tick_size_response_deserializes() {
        let json = r#"{"tokenId":"tok-1","tickSize":0.01}"#;
        let r: TickSizeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.token_id.as_deref(), Some("tok-1"));
        assert_eq!(r.tick_size, Some(0.01));
    }

    #[test]
    fn test_spread_response_deserializes() {
        let json = r#"{"tokenId":"tok-2","spread":0.03}"#;
        let r: SpreadResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.token_id.as_deref(), Some("tok-2"));
        assert_eq!(r.spread, Some(0.03));
    }

    #[test]
    fn test_midpoint_response_deserializes() {
        let json = r#"{"tokenId":"tok-3","mid":0.55}"#;
        let r: MidpointResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.token_id.as_deref(), Some("tok-3"));
        assert_eq!(r.mid, Some(0.55));
    }

    #[test]
    fn test_clob_book_deserializes() {
        let json = r#"{
            "bids": [{"price": "0.54", "size": "100"}],
            "asks": [{"price": "0.56", "size": "200"}]
        }"#;
        let book: ClobBook = serde_json::from_str(json).unwrap();
        assert_eq!(book.bids.len(), 1);
        assert_eq!(book.asks.len(), 1);
        assert_eq!(book.bids[0].price.as_deref(), Some("0.54"));
        assert_eq!(book.asks[0].size.as_deref(), Some("200"));
    }

    #[test]
    fn test_clob_book_empty_deserializes() {
        let json = r#"{}"#;
        let book: ClobBook = serde_json::from_str(json).unwrap();
        assert!(book.bids.is_empty());
        assert!(book.asks.is_empty());
    }

    #[test]
    fn test_clob_price_point_deserializes() {
        let json = r#"[{"t": 1700000000000, "p": 0.55}]"#;
        let points: Vec<ClobPricePoint> = serde_json::from_str(json).unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].t, Some(1700000000000));
        assert_eq!(points[0].p, Some(0.55));
    }

    #[test]
    fn test_batch_orders_response_deserializes() {
        let json = r#"{
            "results": [
                {"status": "filled", "orderId": "o-1", "intentId": "i-1"},
                {"status": "failed", "error": "insufficient balance"}
            ]
        }"#;
        let r: BatchOrdersResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.results.len(), 2);
        assert_eq!(r.results[0].status, "filled");
        assert_eq!(r.results[0].order_id.as_deref(), Some("o-1"));
        assert_eq!(r.results[1].status, "failed");
        assert_eq!(r.results[1].error.as_deref(), Some("insufficient balance"));
    }

    #[test]
    fn test_bulk_cancel_params_serializes() {
        let params = BulkCancelParams {
            order_ids: vec!["o-1".to_string(), "o-2".to_string()],
        };
        let v = serde_json::to_value(&params).unwrap();
        assert_eq!(v["orderIds"][0], "o-1");
        assert_eq!(v["orderIds"][1], "o-2");
    }

    #[test]
    fn test_bulk_cancel_response_deserializes() {
        let json = r#"{"cancelled": ["o-1"], "failed": ["o-2"]}"#;
        let r: BulkCancelResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.cancelled, vec!["o-1"]);
        assert_eq!(r.failed, vec!["o-2"]);
    }

    #[test]
    fn test_news_article_deserializes() {
        let json = r#"{
            "id": "news-1",
            "title": "Market moves",
            "source": "reuters",
            "sentiment": "positive",
            "publishedAt": "2025-01-01T00:00:00Z"
        }"#;
        let article: NewsArticle = serde_json::from_str(json).unwrap();
        assert_eq!(article.id, "news-1");
        assert_eq!(article.title.as_deref(), Some("Market moves"));
        assert_eq!(article.source.as_deref(), Some("reuters"));
        assert_eq!(article.sentiment.as_deref(), Some("positive"));
    }

    #[test]
    fn test_score_entry_deserializes() {
        let json = r#"{
            "userId": "u-1",
            "username": "trader1",
            "rank": 5,
            "score": 88.5
        }"#;
        let entry: ScoreEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.user_id.as_deref(), Some("u-1"));
        assert_eq!(entry.rank, Some(5));
        assert_eq!(entry.score, Some(88.5));
    }

    #[test]
    fn test_badge_deserializes() {
        let json = r#"{
            "id": "badge-gold",
            "name": "Gold Trader",
            "description": "Achieved top 1% profitability",
            "awardedAt": "2025-06-01T00:00:00Z"
        }"#;
        let badge: Badge = serde_json::from_str(json).unwrap();
        assert_eq!(badge.id, "badge-gold");
        assert_eq!(badge.name.as_deref(), Some("Gold Trader"));
        assert_eq!(badge.awarded_at.as_deref(), Some("2025-06-01T00:00:00Z"));
    }

    #[test]
    fn test_polymarket_portfolio_deserializes() {
        let json = r#"{
            "entries": [{
                "asset": "tok-1",
                "size": "50",
                "avgPrice": "0.6",
                "realizedPnl": "10",
                "unrealizedPnl": "5"
            }]
        }"#;
        let p: PolymarketPortfolio = serde_json::from_str(json).unwrap();
        assert_eq!(p.entries.len(), 1);
        assert_eq!(p.entries[0].asset, "tok-1");
        assert_eq!(p.entries[0].avg_price, "0.6");
        assert_eq!(p.entries[0].realized_pnl, "10");
        assert_eq!(p.entries[0].unrealized_pnl, "5");
    }

    #[test]
    fn test_polymarket_earnings_deserializes() {
        let json = r#"{
            "entries": [{
                "date": "2026-01-01",
                "earnings": "25.50",
                "volume": "500",
                "winRate": "0.60"
            }]
        }"#;
        let e: PolymarketEarnings = serde_json::from_str(json).unwrap();
        assert_eq!(e.entries.len(), 1);
        assert_eq!(e.entries[0].date, "2026-01-01");
        assert_eq!(e.entries[0].earnings, "25.50");
        assert_eq!(e.entries[0].volume, "500");
        assert_eq!(e.entries[0].win_rate, "0.60");
    }

    #[test]
    fn test_polymarket_activity_item_deserializes() {
        let json = r#"{
            "id": "act-1",
            "type": "TRADE",
            "amount": "50.00",
            "asset": "tok-1",
            "timestamp": "2026-01-01T00:00:00Z",
            "metadata": {"market": "m-1"}
        }"#;
        let item: PolymarketActivityItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.id.as_deref(), Some("act-1"));
        assert_eq!(item.activity_type.as_deref(), Some("TRADE"));
        assert_eq!(item.amount.as_deref(), Some("50.00"));
        assert_eq!(item.asset.as_deref(), Some("tok-1"));
        assert_eq!(item.timestamp.as_deref(), Some("2026-01-01T00:00:00Z"));
        assert_eq!(item.metadata.as_ref().unwrap()["market"], "m-1");
    }

    #[test]
    fn test_polymarket_activity_response_deserializes() {
        let json = r#"{
            "activities": [{
                "id": "act-1",
                "type": "TRADE",
                "amount": "50.00",
                "asset": "tok-1",
                "timestamp": "2026-01-01T00:00:00Z"
            }]
        }"#;
        let response: PolymarketActivityResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.activities.len(), 1);
        assert_eq!(response.activities[0].id.as_deref(), Some("act-1"));
        assert_eq!(
            response.activities[0].activity_type.as_deref(),
            Some("TRADE")
        );
    }

    #[test]
    fn test_search_markets_params_requires_q() {
        let params = SearchMarketsParams {
            q: "election".to_string(),
            limit: Some(10),
        };
        assert_eq!(params.q, "election");
        assert_eq!(params.limit, Some(10));
    }

    #[test]
    fn test_search_results_deserializes_markets() {
        let json = r#"{"results": [{"id": "m1", "title": "Market One"}, {"id": "m2", "title": "Market Two"}]}"#;
        let sr: SearchResults<Market> = serde_json::from_str(json).unwrap();
        assert_eq!(sr.results.len(), 2);
        assert_eq!(sr.results[0].id, "m1");
        assert_eq!(sr.results[0].title, "Market One");
        assert_eq!(sr.results[1].id, "m2");
    }

    #[test]
    fn test_search_results_into_paginated_response() {
        let json = r#"{"results": [{"id": "m1", "title": "Market One"}, {"id": "m2", "title": "Market Two"}]}"#;
        let sr: SearchResults<Market> = serde_json::from_str(json).unwrap();
        let pr: PaginatedResponse<Market> = sr.into_paginated_response(10);
        assert_eq!(pr.data.len(), 2);
        assert_eq!(pr.total, 2);
        assert_eq!(pr.page, 1);
        assert_eq!(pr.limit, 10);
        assert_eq!(pr.total_pages, 1);
        assert!(!pr.has_next);
        assert_eq!(pr.data[0].id, "m1");
    }

    #[test]
    fn test_search_results_empty_into_paginated_response() {
        let json = r#"{"results":[]}"#;
        let sr: SearchResults<Market> = serde_json::from_str(json).unwrap();
        let pr: PaginatedResponse<Market> = sr.into_paginated_response(20);
        assert_eq!(pr.data.len(), 0);
        assert_eq!(pr.total, 0);
        assert_eq!(pr.limit, 20);
        assert_eq!(pr.total_pages, 0);
        assert!(!pr.has_next);
    }

    #[test]
    fn test_search_results_missing_results_field_is_error() {
        let json = r#"{"other":"stuff"}"#;
        let result: std::result::Result<SearchResults<Market>, serde_json::Error> =
            serde_json::from_str(json);
        assert!(
            result.is_err(),
            "missing `results` field should be a decode error"
        );
    }

    #[test]
    fn test_clob_prices_history_params_defaults() {
        let params = ClobPricesHistoryParams::default();
        assert!(params.interval.is_none());
        assert!(params.fidelity.is_none());
    }

    #[test]
    fn test_get_polymarket_activity_params_default() {
        let params = GetPolymarketActivityParams::default();
        assert!(params.activity_type.is_none());
    }

    // ── Rewards types (#152) ────────────────────────────────────────────

    #[test]
    fn test_user_rewards_deserializes() {
        let json = r#"{"rewards": [{"id": "r1", "amount": "42.5"}]}"#;
        let ur: UserRewards = serde_json::from_str(json).unwrap();
        assert_eq!(ur.rewards.len(), 1);
        assert_eq!(ur.rewards[0]["id"], "r1");
    }

    #[test]
    fn test_user_rewards_deserializes_empty() {
        let json = r#"{"rewards": []}"#;
        let ur: UserRewards = serde_json::from_str(json).unwrap();
        assert!(ur.rewards.is_empty());
    }

    #[test]
    fn test_user_rewards_total_deserializes() {
        let json = r#"{"total": "123.45", "byDate": [{"date": "2026-04-01", "amount": "10"}]}"#;
        let t: UserRewardsTotal = serde_json::from_str(json).unwrap();
        assert_eq!(t.total.as_deref(), Some("123.45"));
        assert_eq!(t.by_date.len(), 1);
    }

    #[test]
    fn test_user_rewards_total_defaults() {
        let json = r#"{}"#;
        let t: UserRewardsTotal = serde_json::from_str(json).unwrap();
        assert_eq!(t.total, None);
        assert!(t.by_date.is_empty());
    }

    #[test]
    fn test_user_rewards_per_market_deserializes() {
        let json = r#"{"markets": [{"conditionId": "c1", "amount": "5"}]}"#;
        let m: UserRewardsPerMarket = serde_json::from_str(json).unwrap();
        assert_eq!(m.markets.len(), 1);
    }

    #[test]
    fn test_rebates_deserializes() {
        let json = r#"{"rebates": [{"id": "rb1", "amount": "0.5"}]}"#;
        let r: Rebates = serde_json::from_str(json).unwrap();
        assert_eq!(r.rebates.len(), 1);
    }

    #[test]
    fn test_rebates_empty() {
        let json = r#"{"rebates": []}"#;
        let r: Rebates = serde_json::from_str(json).unwrap();
        assert!(r.rebates.is_empty());
    }

    #[test]
    fn test_reward_market_captures_extra_fields() {
        let json = r#"{"conditionId": "cond-1", "rewardsDaily": "100", "rewardsMaxSpread": "0.02", "rewardsMinSize": "50", "startDate": "2026-01-01", "endDate": "2026-06-01", "unknown": true}"#;
        let rm: RewardMarket = serde_json::from_str(json).unwrap();
        assert_eq!(rm.condition_id.as_deref(), Some("cond-1"));
        assert_eq!(rm.rewards_daily.as_deref(), Some("100"));
        assert_eq!(rm.extra["unknown"], true);
        assert_eq!(rm.extra["rewardsMaxSpread"], "0.02");
        assert_eq!(rm.extra["rewardsMinSize"], "50");
        assert_eq!(rm.extra["startDate"], "2026-01-01");
        assert_eq!(rm.extra["endDate"], "2026-06-01");
    }

    #[test]
    fn test_user_rewards_percentages_captures_dynamic_keys() {
        let json = r#"{"0xabc123":42.5,"0xdef456":18.0}"#;
        let v: UserRewardsPercentages = serde_json::from_str(json).unwrap();
        assert!(v.extra.get("0xabc123").is_some());
    }

    // -----------------------------------------------------------------------
    // Rewards — newer endpoints (POLA-3328)
    // -----------------------------------------------------------------------

    #[test]
    fn test_rewards_market_detail_deserializes() {
        let json = r#"{"conditionId":"0xabc","ratePerDay":"100.5","totalRewards":"5000","remainingRewardAmount":"3500","maxSpread":"0.02","minSize":"100"}"#;
        let d: RewardsMarketDetail = serde_json::from_str(json).unwrap();
        assert_eq!(d.condition_id.as_deref(), Some("0xabc"));
        assert_eq!(d.rate_per_day.as_deref(), Some("100.5"));
        assert_eq!(d.total_rewards.as_deref(), Some("5000"));
        assert_eq!(d.remaining_reward_amount.as_deref(), Some("3500"));
        assert_eq!(d.max_spread.as_deref(), Some("0.02"));
        assert_eq!(d.min_size.as_deref(), Some("100"));
    }

    #[test]
    fn test_user_sponsored_markets_deserializes() {
        let json = r#"{"markets":[{"conditionId":"0xabc","title":"Test"}]}"#;
        let m: UserSponsoredMarkets = serde_json::from_str(json).unwrap();
        assert_eq!(m.markets.len(), 1);
    }

    #[test]
    fn test_rewards_sponsor_url_deserializes() {
        let json = r#"{"url":"https://polymarket.com/rewards/0xabc"}"#;
        let s: RewardsSponsorUrl = serde_json::from_str(json).unwrap();
        assert_eq!(s.url, "https://polymarket.com/rewards/0xabc");
    }

    #[test]
    fn test_reward_market_detail_extra_backward_compat() {
        let json = r#"{"conditionId": "cond-1", "rewardsDaily": "100", "rewardsMaxSpread": "0.02", "rewardsMinSize": "50", "startDate": "2026-01-01", "endDate": "2026-06-01", "unknown": true}"#;
        let d: RewardMarketDetail = serde_json::from_str(json).unwrap();
        assert_eq!(d.extra["conditionId"], "cond-1");
        assert_eq!(d.extra["rewardsDaily"], "100");
        assert_eq!(d.extra["rewardsMaxSpread"], "0.02");
        assert_eq!(d.extra["rewardsMinSize"], "50");
        assert_eq!(d.extra["startDate"], "2026-01-01");
        assert_eq!(d.extra["endDate"], "2026-06-01");
        assert_eq!(d.extra["unknown"], true);
    }

    #[test]
    fn test_reward_market_serialize_no_duplicate_keys() {
        let json = r#"{"conditionId":"cond-1","rewardsDaily":"100","rewardsMaxSpread":"0.02","rewardsMinSize":"50","startDate":"2026-01-01","endDate":"2026-06-01","unknown":true}"#;
        let rm: RewardMarket = serde_json::from_str(json).unwrap();
        let out = serde_json::to_string(&rm).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        let obj = parsed.as_object().unwrap();
        // No duplicate keys: each key appears exactly once
        assert_eq!(obj["conditionId"], "cond-1");
        assert_eq!(obj["rewardsDaily"], "100");
        assert_eq!(obj["rewardsMaxSpread"], "0.02");
        assert_eq!(obj["rewardsMinSize"], "50");
        assert_eq!(obj["startDate"], "2026-01-01");
        assert_eq!(obj["endDate"], "2026-06-01");
        assert_eq!(obj["unknown"], true);
    }

    #[test]
    fn test_reward_market_detail_serialize_no_duplicate_keys() {
        let json = r#"{"conditionId":"cond-1","rewardsDaily":"100","rewardsMaxSpread":"0.02","rewardsMinSize":"50","startDate":"2026-01-01","endDate":"2026-06-01","unknown":true}"#;
        let d: RewardMarketDetail = serde_json::from_str(json).unwrap();
        let out = serde_json::to_string(&d).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        let obj = parsed.as_object().unwrap();
        assert_eq!(obj["conditionId"], "cond-1");
        assert_eq!(obj["rewardsDaily"], "100");
        assert_eq!(obj["rewardsMaxSpread"], "0.02");
        assert_eq!(obj["rewardsMinSize"], "50");
        assert_eq!(obj["startDate"], "2026-01-01");
        assert_eq!(obj["endDate"], "2026-06-01");
        assert_eq!(obj["unknown"], true);
    }

    #[test]
    fn test_reward_market_serialize_roundtrip() {
        let json = r#"{"conditionId":"cond-1","rewardsDaily":"100","startDate":"2026-01-01","endDate":"2026-06-01","extraField":"survives"}"#;
        let rm: RewardMarket = serde_json::from_str(json).unwrap();
        // Round-trip: serialize → deserialize → typed fields + extra survive
        let out = serde_json::to_string(&rm).unwrap();
        let rm2: RewardMarket = serde_json::from_str(&out).unwrap();
        assert_eq!(rm2.condition_id.as_deref(), Some("cond-1"));
        assert_eq!(rm2.rewards_daily.as_deref(), Some("100"));
        assert_eq!(rm2.start_date.as_deref(), Some("2026-01-01"));
        assert_eq!(rm2.end_date.as_deref(), Some("2026-06-01"));
        assert_eq!(rm2.extra["extraField"], "survives");
    }

    #[test]
    fn test_reward_market_detail_serialize_roundtrip() {
        let json = r#"{"conditionId":"cond-1","rewardsDaily":"100","startDate":"2026-01-01","endDate":"2026-06-01","extraField":"survives"}"#;
        let d: RewardMarketDetail = serde_json::from_str(json).unwrap();
        let out = serde_json::to_string(&d).unwrap();
        let d2: RewardMarketDetail = serde_json::from_str(&out).unwrap();
        assert_eq!(d2.condition_id.as_deref(), Some("cond-1"));
        assert_eq!(d2.rewards_daily.as_deref(), Some("100"));
        assert_eq!(d2.start_date.as_deref(), Some("2026-01-01"));
        assert_eq!(d2.end_date.as_deref(), Some("2026-06-01"));
        assert_eq!(d2.extra["extraField"], "survives");
    }

    #[test]
    fn test_reward_market_serialize_empty_extra_no_spurious_keys() {
        let json = r#"{"conditionId":"cond-1","rewardsDaily":"100"}"#;
        let rm: RewardMarket = serde_json::from_str(json).unwrap();
        let out = serde_json::to_string(&rm).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        let obj = parsed.as_object().unwrap();
        // Only the two typed fields, no extra blanks
        assert_eq!(obj.len(), 2);
        assert_eq!(obj["conditionId"], "cond-1");
        assert_eq!(obj["rewardsDaily"], "100");
    }

    // -----------------------------------------------------------------------
    // Cross-Venue Arbitrage — type tests
    // -----------------------------------------------------------------------
    // Cross-Venue Arbitrage — type tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_cross_venue_arbitrage_opportunity_deserializes() {
        let json = r#"{"id":"arb-1","marketTitle":"Will X?","venueA":"polymarket","venueB":"kalshi","priceA":"0.55","priceB":"0.48","spread":"0.07","direction":"buy_poly_sell_kalshi"}"#;
        let opp: CrossVenueArbitrageOpportunity = serde_json::from_str(json).unwrap();
        assert_eq!(opp.id, "arb-1");
        assert_eq!(opp.venue_a.as_deref(), Some("polymarket"));
        assert_eq!(opp.spread.as_deref(), Some("0.07"));
    }

    #[test]
    fn test_cross_venue_comparison_deserializes() {
        let json = r#"{"matchId":"match-1","polymarketPrice":"0.6","kalshiPrice":"0.55","spread":"0.05","arbitragePct":"8.3"}"#;
        let cmp: CrossVenueComparison = serde_json::from_str(json).unwrap();
        assert_eq!(cmp.match_id, "match-1");
        assert_eq!(cmp.arbitrage_pct.as_deref(), Some("8.3"));
    }

    #[test]
    fn test_arbitrage_match_deserializes() {
        let json = r#"{"id":"m-1","polymarketMarketId":"poly-abc","kalshiMarketId":"kalshi-xyz","verified":true,"status":"active","createdAt":"2026-01-01"}"#;
        let m: ArbitrageMatch = serde_json::from_str(json).unwrap();
        assert_eq!(m.id, "m-1");
        assert_eq!(m.verified, Some(true));
    }

    #[test]
    fn test_create_arbitrage_match_params_serializes() {
        let p = CreateArbitrageMatchParams {
            polymarket_market_id: "poly-1".into(),
            kalshi_market_id: "kalshi-1".into(),
            notes: Some("test".into()),
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["polymarketMarketId"], "poly-1");
        assert_eq!(v["notes"], "test");
    }

    #[test]
    fn test_create_arbitrage_match_params_omits_none_notes() {
        let p = CreateArbitrageMatchParams {
            polymarket_market_id: "poly-1".into(),
            kalshi_market_id: "kalshi-1".into(),
            notes: None,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert!(v.get("notes").is_none());
    }

    #[test]
    fn test_admin_only_arbitrage_match_mutations_are_hidden_and_deprecated() {
        let source = include_str!("client.rs");
        for method in [
            "create_arbitrage_match",
            "verify_arbitrage_match",
            "delete_arbitrage_match",
            "sync_arbitrage_matches",
        ] {
            let method_pos = source
                .find(&format!("pub async fn {method}"))
                .unwrap_or_else(|| panic!("{method} should exist for compatibility"));
            let prefix_start = source[..method_pos].rfind("\n\n").map_or(0, |idx| idx + 2);
            let prefix = &source[prefix_start..method_pos];

            assert!(
                prefix.contains("#[doc(hidden)]"),
                "{method} must be hidden from the public Rustdoc surface"
            );
            assert!(
                prefix.contains("#[deprecated(") && prefix.contains("admin-only"),
                "{method} must be deprecated with an admin-only note"
            );
            assert!(
                prefix.contains("Admin-only"),
                "{method} docs must explicitly say the endpoint is admin-only"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Cross-Venue Arbitrage — new types (POLA-908)
    // -----------------------------------------------------------------------

    #[test]
    fn test_venue_price_info_deserializes() {
        let json = r#"{"marketId":"mkt-1","title":"Will X?","yesBid":0.55,"noAsk":0.45}"#;
        let v: VenuePriceInfo = serde_json::from_str(json).unwrap();
        assert_eq!(v.market_id.as_deref(), Some("mkt-1"));
        assert_eq!(v.yes_bid, Some(0.55));
        assert_eq!(v.no_ask, Some(0.45));
    }

    #[test]
    fn test_venue_price_info_handles_missing_fields() {
        let json = r#"{}"#;
        let v: VenuePriceInfo = serde_json::from_str(json).unwrap();
        assert_eq!(v.market_id, None);
        assert_eq!(v.yes_bid, None);
    }

    #[test]
    fn test_spread_summary_deserializes() {
        let json = r#"{"matchId":"m-1","polymarket":{"marketId":"p1","yesBid":0.6},"kalshi":{"marketId":"k1","noAsk":0.4},"yesSpreadPct":5.0,"noSpreadPct":3.2,"confidence":0.9,"verified":true}"#;
        let s: SpreadSummary = serde_json::from_str(json).unwrap();
        assert_eq!(s.match_id.as_deref(), Some("m-1"));
        assert_eq!(s.yes_spread_pct, Some(5.0));
        assert_eq!(s.no_spread_pct, Some(3.2));
        assert_eq!(s.verified, Some(true));
        assert!(s.polymarket.is_some());
        assert!(s.kalshi.is_some());
    }

    #[test]
    fn test_spread_summary_handles_null_venues() {
        let json = r#"{"matchId":"m-2","yesSpreadPct":1.0,"confidence":0.5}"#;
        let s: SpreadSummary = serde_json::from_str(json).unwrap();
        assert!(s.polymarket.is_none());
        assert!(s.kalshi.is_none());
    }

    #[test]
    fn test_match_sync_result_deserializes() {
        let json = r#"{"matched":10,"created":3,"updated":7}"#;
        let r: MatchSyncResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.matched, Some(10));
        assert_eq!(r.created, Some(3));
        assert_eq!(r.updated, Some(7));
    }

    #[test]
    fn test_match_sync_result_handles_minimal() {
        let json = r#"{"matched":5}"#;
        let r: MatchSyncResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.matched, Some(5));
        assert_eq!(r.created, None);
    }

    #[test]
    fn test_arbitrage_alert_subscription_deserializes() {
        let json = r#"{"id":"alert-1","minSpreadPct":5.0,"marketId":"mkt-1","active":true,"triggeredAt":null,"createdAt":"2026-01-01"}"#;
        let a: ArbitrageAlertSubscription = serde_json::from_str(json).unwrap();
        assert_eq!(a.id, "alert-1");
        assert_eq!(a.min_spread_pct, Some(5.0));
        assert_eq!(a.active, Some(true));
        assert_eq!(a.market_id.as_deref(), Some("mkt-1"));
    }

    #[test]
    fn test_arbitrage_alert_subscription_no_market() {
        let json = r#"{"id":"alert-2","minSpreadPct":3.0,"active":true,"createdAt":"2026-01-01"}"#;
        let a: ArbitrageAlertSubscription = serde_json::from_str(json).unwrap();
        assert_eq!(a.id, "alert-2");
        assert_eq!(a.market_id, None);
    }

    #[test]
    fn test_create_arbitrage_alert_params_serializes() {
        let p = CreateArbitrageAlertParams {
            min_spread_pct: "5.0".into(),
            market_id: Some("mkt-1".into()),
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["minSpreadPct"], "5.0");
        assert_eq!(v["marketId"], "mkt-1");
    }

    #[test]
    fn test_create_arbitrage_alert_params_omits_none_market() {
        let p = CreateArbitrageAlertParams {
            min_spread_pct: "3.0".into(),
            market_id: None,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["minSpreadPct"], "3.0");
        assert!(v.get("marketId").is_none());
    }

    // -----------------------------------------------------------------------
    // Cross-Venue Arbitrage — path tests (POLA-908)
    // -----------------------------------------------------------------------

    #[test]
    fn test_list_arbitrage_opportunities_path_no_spread() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/arbitrage/cross-venue");
        assert!(url.ends_with("/api/v1/arbitrage/cross-venue"));
    }

    #[test]
    fn test_cross_venue_market_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let path = format!("/api/v1/arbitrage/cross-venue/{}", encode("mkt-123"));
        let url = client.url(&path);
        assert!(url.contains("/api/v1/arbitrage/cross-venue/mkt-123"));
    }

    #[test]
    fn test_get_market_match_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let path = format!("/api/v1/arbitrage/matches/{}", encode("match-1"));
        let url = client.url(&path);
        assert!(url.contains("/api/v1/arbitrage/matches/match-1"));
    }

    #[test]
    fn test_spread_comparison_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/arbitrage/spread");
        assert!(url.ends_with("/api/v1/arbitrage/spread"));
    }

    #[test]
    fn test_arbitrage_history_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/arbitrage/history");
        assert!(url.ends_with("/api/v1/arbitrage/history"));
    }

    #[test]
    fn test_arbitrage_alerts_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/arbitrage/alerts");
        assert!(url.ends_with("/api/v1/arbitrage/alerts"));
    }

    #[test]
    fn test_delete_arbitrage_alert_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let path = format!("/api/v1/arbitrage/alerts/{}", encode("alert-99"));
        let url = client.url(&path);
        assert!(url.contains("/api/v1/arbitrage/alerts/alert-99"));
    }

    // -----------------------------------------------------------------------
    // Cross-Venue Arb Execution / Positions / Risk — tests (POLA-1852)
    // -----------------------------------------------------------------------

    #[test]
    fn test_execute_arbitrage_path_and_body() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/arbitrage/execute");
        assert!(url.ends_with("/api/v1/arbitrage/execute"));

        let p = ExecuteArbitrageParams {
            match_id: "m-1".into(),
            size: 100,
            max_slippage_pct: Some(0.5),
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["matchId"], "m-1");
        assert_eq!(v["size"], 100.0);
        assert_eq!(v["maxSlippagePct"], 0.5);
    }

    #[test]
    fn test_execute_arbitrage_params_omits_none_slippage() {
        let p = ExecuteArbitrageParams {
            match_id: "m-1".into(),
            size: 50,
            max_slippage_pct: None,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert!(v.get("maxSlippagePct").is_none());
    }

    #[test]
    fn test_validate_arb_size_bounds() {
        assert!(validate_arb_size(1.0).is_ok());
        assert!(validate_arb_size(10000.0).is_ok());
        assert!(validate_arb_size(0.99).is_err());
        assert!(validate_arb_size(100.5).is_err());
        assert!(validate_arb_size(10000.01).is_err());
        assert!(validate_arb_size(f64::NAN).is_err());
        assert!(validate_arb_size(f64::INFINITY).is_err());
        assert!(validate_arb_size(f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn test_validate_arb_match_id_bounds() {
        assert!(validate_arb_match_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
        assert!(validate_arb_match_id("").is_err());
        assert!(validate_arb_match_id(&"x".repeat(256)).is_err());
        assert!(validate_arb_match_id("match-1").is_err());
    }

    #[test]
    fn test_validate_arb_slippage_bounds() {
        assert!(validate_arb_slippage(0.0).is_ok());
        assert!(validate_arb_slippage(5.0).is_ok());
        assert!(validate_arb_slippage(-0.01).is_err());
        assert!(validate_arb_slippage(5.01).is_err());
        assert!(validate_arb_slippage(f64::NAN).is_err());
    }

    #[tokio::test]
    async fn test_execute_arbitrage_rejects_bad_size() {
        let client = PolyforgeClient::new("k").unwrap();
        let p = ExecuteArbitrageParams {
            match_id: "m-1".into(),
            size: 0,
            max_slippage_pct: None,
        };
        let err = client
            .execute_arbitrage(&p, "arb-execute-key-1")
            .await
            .unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
    }

    #[tokio::test]
    async fn test_execute_arbitrage_rejects_bad_match_id() {
        let client = PolyforgeClient::new("k").unwrap();
        let p = ExecuteArbitrageParams {
            match_id: "x".repeat(256),
            size: 100,
            max_slippage_pct: None,
        };
        let err = client
            .execute_arbitrage(&p, "arb-execute-key-1")
            .await
            .unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
    }

    #[tokio::test]
    async fn test_execute_arbitrage_rejects_bad_slippage() {
        let client = PolyforgeClient::new("k").unwrap();
        let p = ExecuteArbitrageParams {
            match_id: "m-1".into(),
            size: 100,
            max_slippage_pct: Some(7.5),
        };
        let err = client
            .execute_arbitrage(&p, "arb-execute-key-1")
            .await
            .unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
    }

    #[tokio::test]
    async fn test_execute_arbitrage_sends_idempotency_key() {
        let request = capture_request("{}", |client| async move {
            let p = ExecuteArbitrageParams {
                match_id: "550e8400-e29b-41d4-a716-446655440000".into(),
                size: 100,
                max_slippage_pct: Some(0.5),
            };
            client
                .execute_arbitrage(&p, "arb-execute-key-1")
                .await
                .map(|_| ())
        })
        .await;

        assert!(request.starts_with("POST /api/v1/arbitrage/execute "));
        assert_eq!(
            captured_header(&request, "Idempotency-Key"),
            Some("arb-execute-key-1")
        );
    }

    #[tokio::test]
    async fn test_execute_arbitrage_rejects_empty_idempotency_key() {
        let client = PolyforgeClient::new("k").unwrap();
        let p = ExecuteArbitrageParams {
            match_id: "550e8400-e29b-41d4-a716-446655440000".into(),
            size: 100,
            max_slippage_pct: None,
        };

        let err = client.execute_arbitrage(&p, " ").await.unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
    }

    #[test]
    fn test_arb_positions_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/arbitrage/positions");
        assert!(url.ends_with("/api/v1/arbitrage/positions"));
    }

    #[tokio::test]
    async fn test_list_arbitrage_positions_rejects_bad_limit() {
        let client = PolyforgeClient::new("k").unwrap();
        let err = client
            .list_arbitrage_positions(None, Some(101), None)
            .await
            .unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
    }

    #[test]
    fn test_arb_position_status_deserializes_screaming_snake_case() {
        let status: ArbPositionStatus = serde_json::from_str("\"PENDING\"").unwrap();
        assert_eq!(status, ArbPositionStatus::Pending);
        assert!(serde_json::from_str::<ArbPositionStatus>("\"EXPIRED\"").is_err());
    }

    #[test]
    fn test_arb_position_by_id_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let path = format!("/api/v1/arbitrage/positions/{}", encode("pos-42"));
        let url = client.url(&path);
        assert!(url.contains("/api/v1/arbitrage/positions/pos-42"));
    }

    #[test]
    fn test_arb_position_close_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let path = format!("/api/v1/arbitrage/positions/{}/close", encode("pos-42"));
        let url = client.url(&path);
        assert!(url.contains("/api/v1/arbitrage/positions/pos-42/close"));
    }

    #[tokio::test]
    async fn test_close_arbitrage_position_sends_idempotency_key() {
        let request = capture_request("{}", |client| async move {
            client
                .close_arbitrage_position("pos-42", "arb-close-key-1")
                .await
                .map(|_| ())
        })
        .await;

        assert!(request.starts_with("POST /api/v1/arbitrage/positions/pos-42/close "));
        assert_eq!(
            captured_header(&request, "Idempotency-Key"),
            Some("arb-close-key-1")
        );
    }

    #[test]
    fn test_arb_risk_dashboard_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/arbitrage/risk/dashboard");
        assert!(url.ends_with("/api/v1/arbitrage/risk/dashboard"));
    }

    #[test]
    fn test_arb_settlement_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/arbitrage/risk/settlement");
        assert!(url.ends_with("/api/v1/arbitrage/risk/settlement"));
    }

    #[test]
    fn test_arb_refresh_pnl_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/arbitrage/risk/refresh-pnl");
        assert!(url.ends_with("/api/v1/arbitrage/risk/refresh-pnl"));
    }

    #[test]
    fn test_arb_execution_result_deserializes() {
        let json = r#"{
            "arbPositionId":"ap-1",
            "buyLeg":{"venue":"POLYMARKET","intentId":"b-1","tokenId":"tok-y","price":"0.550000000000000000"},
            "sellLeg":{"venue":"KALSHI","intentId":"s-1","tokenId":"tok-n","price":"0.480000000000000000"},
            "entrySpreadPct":0.07,
            "status":"PENDING"
        }"#;
        let r: ArbExecutionResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.arb_position_id.as_deref(), Some("ap-1"));
        assert_eq!(r.entry_spread_pct, Some(0.07));
        assert_eq!(r.status, Some(ArbPositionStatus::Pending));
        let buy = r.buy_leg.as_ref().unwrap();
        assert_eq!(buy.venue, Some(Venue::Polymarket));
        assert_eq!(buy.token_id.as_deref(), Some("tok-y"));
        assert_eq!(buy.price.as_deref(), Some("0.550000000000000000"));
    }

    #[test]
    fn test_arb_execution_result_deserializes_numeric_leg_prices() {
        let json = r#"{
            "arbPositionId":"ap-1",
            "buyLeg":{"venue":"POLYMARKET","intentId":"b-1","tokenId":"tok-y","price":0.55},
            "sellLeg":{"venue":"KALSHI","intentId":"s-1","tokenId":"tok-n","price":0.48},
            "entrySpreadPct":0.07,
            "status":"PENDING"
        }"#;
        let r: ArbExecutionResult = serde_json::from_str(json).unwrap();
        let buy = r.buy_leg.as_ref().unwrap();
        let sell = r.sell_leg.as_ref().unwrap();
        assert_eq!(buy.price.as_deref(), Some("0.55"));
        assert_eq!(sell.price.as_deref(), Some("0.48"));
    }

    #[test]
    fn test_arb_position_deserializes_with_decimal_strings() {
        let json = r#"{
            "id":"ap-1","userId":"u-1","matchId":"m-1","status":"OPEN",
            "buyVenue":"POLYMARKET","buyTokenId":"tok-y","buyPrice":"0.55","buySize":"100",
            "sellVenue":"KALSHI","sellTokenId":"tok-n","sellPrice":"0.48","sellSize":"100",
            "entrySpreadPct":"0.07","unrealizedPnl":"2.50",
            "createdAt":"2026-04-01T00:00:00Z","updatedAt":"2026-04-01T00:00:00Z"
        }"#;
        let p: ArbPosition = serde_json::from_str(json).unwrap();
        assert_eq!(p.id, "ap-1");
        assert_eq!(p.status, Some(ArbPositionStatus::Open));
        assert_eq!(p.buy_venue, Some(Venue::Polymarket));
        assert_eq!(p.sell_venue, Some(Venue::Kalshi));
        assert_eq!(p.buy_price.as_deref(), Some("0.55"));
        assert_eq!(p.entry_spread_pct.as_deref(), Some("0.07"));
        assert_eq!(p.unrealized_pnl.as_deref(), Some("2.50"));
    }

    #[test]
    fn test_arb_positions_response_deserializes() {
        let json = r#"{"positions":[{"id":"ap-1"}],"total":1}"#;
        let r: ArbPositionsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.total, 1);
        assert_eq!(r.positions.len(), 1);
        assert_eq!(r.positions[0].id, "ap-1");
    }

    #[test]
    fn test_arb_close_response_deserializes() {
        let json = r#"{"status":"CLOSING","positionId":"ap-1"}"#;
        let r: ArbCloseResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.status, Some(ArbPositionStatus::Closing));
        assert_eq!(r.position_id.as_deref(), Some("ap-1"));
    }

    #[test]
    fn test_arb_risk_dashboard_deserializes() {
        let json = r#"{
            "openPositions":3,"pendingPositions":1,"totalDeployed":1500.0,
            "netExposure":{"polymarket":750.0,"kalshi":-750.0,"polymarketUs":-500.0},
            "totalRealizedPnl":12.5,"totalUnrealizedPnl":-3.25,"avgSpreadPct":0.05,
            "positionsByStatus":{"OPEN":3,"PENDING":1}
        }"#;
        let d: ArbRiskDashboard = serde_json::from_str(json).unwrap();
        assert_eq!(d.open_positions, 3);
        assert_eq!(d.pending_positions, 1);
        assert_eq!(d.net_exposure.polymarket, 750.0);
        assert_eq!(d.net_exposure.kalshi, -750.0);
        assert_eq!(d.net_exposure.polymarket_us, -500.0);
        assert_eq!(
            d.positions_by_status.get(&ArbPositionStatus::Open),
            Some(&3)
        );
    }

    #[test]
    fn test_arb_net_exposure_deserializes_polymarket_us() {
        let json = r#"{"polymarket":100.0,"kalshi":-200.0,"polymarketUs":-50.0}"#;
        let e: ArbNetExposure = serde_json::from_str(json).unwrap();
        assert_eq!(e.polymarket, 100.0);
        assert_eq!(e.kalshi, -200.0);
        assert_eq!(e.polymarket_us, -50.0);
    }

    #[test]
    fn test_arb_net_exposure_polymarket_us_defaults_to_zero() {
        let json = r#"{"polymarket":300.0}"#;
        let e: ArbNetExposure = serde_json::from_str(json).unwrap();
        assert_eq!(e.polymarket, 300.0);
        assert_eq!(e.kalshi, 0.0);
        assert_eq!(e.polymarket_us, 0.0);
    }

    #[test]
    fn test_arb_settlement_risk_deserializes() {
        let json = r#"{
            "matchId":"m-1","polymarketTitle":"Will X?","kalshiTitle":"X happens",
            "polymarketEndDate":"2026-12-31","kalshiEndDate":"2026-12-30",
            "endDateDiffDays":1.0,"confidence":0.92,"riskLevel":"MEDIUM",
            "reason":"end-date drift"
        }"#;
        let r: ArbSettlementRisk = serde_json::from_str(json).unwrap();
        assert_eq!(r.match_id.as_deref(), Some("m-1"));
        assert_eq!(r.risk_level.as_deref(), Some("MEDIUM"));
        assert_eq!(r.end_date_diff_days, Some(1.0));
    }

    #[test]
    fn test_arb_pnl_refresh_result_deserializes() {
        let json = r#"{"updated":7}"#;
        let r: ArbPnlRefreshResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.updated, 7);
    }

    #[test]
    fn test_venue_as_str() {
        assert_eq!(Venue::Polymarket.as_str(), "POLYMARKET");
        assert_eq!(Venue::Kalshi.as_str(), "KALSHI");
        assert_eq!(Venue::PolymarketUs.as_str(), "POLYMARKET_US");
        assert_eq!(Venue::Unknown.as_str(), "UNKNOWN");
    }

    #[test]
    fn test_venue_serializes() {
        assert_eq!(
            serde_json::to_string(&Venue::Polymarket).unwrap(),
            r#""POLYMARKET""#
        );
        assert_eq!(
            serde_json::to_string(&Venue::Kalshi).unwrap(),
            r#""KALSHI""#
        );
        assert_eq!(
            serde_json::to_string(&Venue::PolymarketUs).unwrap(),
            r#""POLYMARKET_US""#
        );
        assert_eq!(
            serde_json::to_string(&Venue::Unknown).unwrap(),
            r#""UNKNOWN""#
        );
    }

    #[test]
    fn test_venue_deserializes() {
        assert_eq!(
            serde_json::from_str::<Venue>(r#""POLYMARKET""#).unwrap(),
            Venue::Polymarket
        );
        assert_eq!(
            serde_json::from_str::<Venue>(r#""KALSHI""#).unwrap(),
            Venue::Kalshi
        );
        assert_eq!(
            serde_json::from_str::<Venue>(r#""POLYMARKET_US""#).unwrap(),
            Venue::PolymarketUs
        );
    }

    #[test]
    fn test_venue_unknown_deserializes() {
        assert_eq!(
            serde_json::from_str::<Venue>(r#""POLYMARKET_NEW""#).unwrap(),
            Venue::Unknown
        );
        assert_eq!(
            serde_json::from_str::<Venue>(r#""KRALSCHI""#).unwrap(),
            Venue::Unknown
        );
    }

    #[test]
    fn test_venue_unknown_preserves_arb_position_deserialization() {
        let json = r#"{"id":"ap-1","buyVenue":"POLYMARKET","sellVenue":"FUTURE_VENUE_v3"}"#;
        let p: ArbPosition = serde_json::from_str(json).unwrap();
        assert_eq!(p.buy_venue, Some(Venue::Polymarket));
        assert_eq!(p.sell_venue, Some(Venue::Unknown));
    }

    #[test]
    fn test_arb_execution_leg_deserializes_polymarket_us() {
        let json = r#"{"venue":"POLYMARKET_US","intentId":"pm-us-1"}"#;
        let leg: ArbExecutionLeg = serde_json::from_str(json).unwrap();
        assert_eq!(leg.venue, Some(Venue::PolymarketUs));
        assert_eq!(leg.intent_id.as_deref(), Some("pm-us-1"));
    }

    #[test]
    fn test_arb_position_deserializes_polymarket_us() {
        let json = r#"{"id":"ap-1","buyVenue":"POLYMARKET","sellVenue":"POLYMARKET_US"}"#;
        let p: ArbPosition = serde_json::from_str(json).unwrap();
        assert_eq!(p.buy_venue, Some(Venue::Polymarket));
        assert_eq!(p.sell_venue, Some(Venue::PolymarketUs));
    }

    // -----------------------------------------------------------------------
    // Whale — type tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_whale_leaderboard_entry_deserializes() {
        let json = r#"{"rank":1,"walletAddress":"0xabc","totalVolume":"1000000","totalPnl":"50000","winRate":"0.65"}"#;
        let e: WhaleLeaderboardEntry = serde_json::from_str(json).unwrap();
        assert_eq!(e.rank, Some(1));
        assert_eq!(e.wallet_address.as_deref(), Some("0xabc"));
    }

    #[test]
    fn test_whale_alert_filter_deserializes() {
        let json = r#"{"minSizeUsd":5000,"marketIds":["m1","m2"],"walletAddresses":["0x1"]}"#;
        let f: WhaleAlertFilter = serde_json::from_str(json).unwrap();
        assert_eq!(f.min_size_usd, Some(5000));
        assert_eq!(f.market_ids.len(), 2);
    }

    #[test]
    fn test_update_whale_alert_filter_omits_none() {
        let p = UpdateWhaleAlertFilterParams {
            min_size_usd: None,
            market_ids: None,
            wallet_addresses: None,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert!(v.get("minSizeUsd").is_none());
    }

    // -----------------------------------------------------------------------
    // Profile — type tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_update_profile_params_omits_none_fields() {
        let p = UpdateProfileParams {
            display_name: Some("Alice".into()),
            bio: None,
            avatar_url: None,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["displayName"], "Alice");
        assert!(v.get("bio").is_none());
        assert!(v.get("twitterHandle").is_none());
    }

    #[test]
    fn test_update_profile_raw_builds_twitter_handle() {
        // Callers can extend with platform fields via UpdateProfileParams::to_value()
        // and update_my_profile_raw() without any struct surface changes.
        let params = UpdateProfileParams {
            display_name: Some("Alice".into()),
            ..Default::default()
        };
        let mut body = params.to_value().unwrap();
        body["twitterHandle"] = serde_json::json!("polyforge");
        assert_eq!(body["twitterHandle"], "polyforge");
        assert!(body.get("twitter_handle").is_none());
        assert_eq!(body["displayName"], "Alice");
    }

    #[test]
    fn test_update_profile_raw_twitter_handle_max_length() {
        let handle = "a".repeat(50);
        let params = UpdateProfileParams::default();
        let mut body = params.to_value().unwrap();
        body["twitterHandle"] = serde_json::json!(handle.clone());
        assert_eq!(body["twitterHandle"].as_str().unwrap().len(), 50);
    }

    #[test]
    fn test_user_profile_deserializes() {
        let json = r#"{"userId":"u-1","username":"alice","displayName":"Alice","followerCount":42,"isFollowing":false}"#;
        let up: UserProfile = serde_json::from_str(json).unwrap();
        assert_eq!(up.username.as_deref(), Some("alice"));
        assert_eq!(up.follower_count, Some(42));
        assert_eq!(up.is_following, Some(false));
    }

    #[test]
    fn test_change_password_params_serializes() {
        let p = ChangePasswordParams {
            current_password: "old".into(),
            new_password: "new".into(),
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["currentPassword"], "old");
        assert_eq!(v["newPassword"], "new");
    }

    // -----------------------------------------------------------------------
    // Settings — type tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_notification_settings_deserializes() {
        // Real shape returned by GET /api/v1/settings/notifications, mirroring
        // the Prisma `NotificationPreference` row + `UpdateNotificationsDto`
        // contract. Includes server-only / forward-compat fields that must
        // round-trip through the `extra` flatten bucket.
        let json = r#"{
            "userId": "user_abc123",
            "emailEnabled": true,
            "telegramEnabled": false,
            "discordEnabled": false,
            "onOrderFilled": true,
            "onStrategyError": true,
            "onBacktestComplete": true,
            "onDailyLossLimit": true,
            "onMarketResolved": true,
            "onSomeoneForked": false,
            "onSomeoneFollowed": false,
            "onSomeoneLiked": false,
            "onSomeoneCommented": false,
            "onTicketReply": true,
            "minFillNotifyUsdc": "0.00",
            "notificationFreq": "IMMEDIATE",
            "emailDigest": "DAILY",
            "updatedAt": "2026-04-01T12:00:00.000Z"
        }"#;
        let ns: NotificationSettings = serde_json::from_str(json).unwrap();
        assert_eq!(ns.email_enabled, Some(true));
        assert_eq!(ns.telegram_enabled, Some(false));
        assert_eq!(ns.discord_enabled, Some(false));
        assert_eq!(ns.on_order_filled, Some(true));
        assert_eq!(ns.on_strategy_error, Some(true));
        assert_eq!(ns.on_backtest_complete, Some(true));
        assert_eq!(ns.on_daily_loss_limit, Some(true));
        assert_eq!(ns.on_market_resolved, Some(true));
        assert_eq!(ns.on_someone_forked, Some(false));
        assert_eq!(ns.on_someone_followed, Some(false));
        assert_eq!(ns.on_someone_liked, Some(false));
        assert_eq!(ns.on_someone_commented, Some(false));
        // Server-only / forward-compat fields must be preserved in `extra`.
        assert_eq!(ns.extra["userId"], "user_abc123");
        assert_eq!(ns.extra["onTicketReply"], true);
        assert_eq!(ns.extra["emailDigest"], "DAILY");
        assert_eq!(ns.extra["notificationFreq"], "IMMEDIATE");
    }

    #[test]
    fn test_update_notification_settings_omits_none() {
        let p = UpdateNotificationSettingsParams {
            email_enabled: Some(true),
            telegram_enabled: None,
            discord_enabled: None,
            on_order_filled: None,
            on_strategy_error: None,
            on_backtest_complete: None,
            on_daily_loss_limit: None,
            on_market_resolved: None,
            on_someone_forked: None,
            on_someone_followed: None,
            on_someone_liked: None,
            on_someone_commented: None,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["emailEnabled"], true);
        assert!(v.get("telegramEnabled").is_none());
        assert!(v.get("discordEnabled").is_none());
        assert!(v.get("onOrderFilled").is_none());
    }

    /// Wire-format keys must exactly match the platform `UpdateNotificationsDto`.
    /// `forbidNonWhitelisted: true` on the platform rejects any field the DTO
    /// does not declare, so this guards against future drift.
    #[test]
    fn test_update_notification_settings_camel_case_keys() {
        let p = UpdateNotificationSettingsParams {
            email_enabled: Some(true),
            telegram_enabled: Some(false),
            discord_enabled: Some(false),
            on_order_filled: Some(true),
            on_strategy_error: Some(true),
            on_backtest_complete: Some(true),
            on_daily_loss_limit: Some(true),
            on_market_resolved: Some(true),
            on_someone_forked: Some(false),
            on_someone_followed: Some(false),
            on_someone_liked: Some(false),
            on_someone_commented: Some(false),
        };
        let v = serde_json::to_value(&p).unwrap();
        let obj = v.as_object().expect("serialized as object");

        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort();
        let mut expected = vec![
            "emailEnabled",
            "telegramEnabled",
            "discordEnabled",
            "onOrderFilled",
            "onStrategyError",
            "onBacktestComplete",
            "onDailyLossLimit",
            "onMarketResolved",
            "onSomeoneForked",
            "onSomeoneFollowed",
            "onSomeoneLiked",
            "onSomeoneCommented",
        ];
        expected.sort();
        assert_eq!(keys, expected);

        // Stale / fictional fields from the previous schema must not leak.
        for stale in &[
            "pushEnabled",
            "orderFills",
            "strategyErrors",
            "whaleAlerts",
            "marketResolutions",
            "dailySummary",
        ] {
            assert!(
                obj.get(*stale).is_none(),
                "stale field `{stale}` must not appear in serialized output",
            );
        }
    }

    #[test]
    fn test_update_password_params_serializes() {
        let p = UpdatePasswordParams {
            current_password: "current".into(),
            new_password: "new_secure".into(),
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["currentPassword"], "current");
    }

    #[test]
    fn test_update_settings_profile_omits_none() {
        let p = UpdateSettingsProfileParams {
            display_name: None,
            bio: Some("Hello".into()),
            avatar_url: None,
            twitter_handle: None,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert!(v.get("displayName").is_none());
        assert_eq!(v["bio"], "Hello");
        assert!(v.get("twitterHandle").is_none());
    }

    #[test]
    fn test_update_settings_profile_serializes_twitter_handle_camel_case() {
        let p = UpdateSettingsProfileParams {
            display_name: None,
            bio: None,
            avatar_url: None,
            twitter_handle: Some("polyforge".into()),
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["twitterHandle"], "polyforge");
        assert!(v.get("twitter_handle").is_none());
    }

    #[test]
    fn test_update_settings_profile_twitter_handle_max_length_50() {
        // Mirrors the platform's @MaxLength(50) on UpdateProfileDto.twitterHandle:
        // services/api-service/src/settings/dto/update-profile.dto.ts. The SDK
        // transmits the value as-is (server enforces the cap); this regression
        // test guards against accidental client-side truncation.
        let handle = "a".repeat(50);
        let p = UpdateSettingsProfileParams {
            display_name: None,
            bio: None,
            avatar_url: None,
            twitter_handle: Some(handle.clone()),
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["twitterHandle"], handle);
        assert_eq!(v["twitterHandle"].as_str().unwrap().len(), 50);
    }

    // -----------------------------------------------------------------------
    // Support Tickets — type tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_create_ticket_params_serializes() {
        let p = CreateTicketParams {
            subject: "Help".into(),
            body: "I need help".into(),
            category: Some(TicketCategory::Technical),
            priority: Some(TicketPriority::High),
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["subject"], "Help");
        assert_eq!(v["category"], "TECHNICAL");
        assert_eq!(v["priority"], "HIGH");
    }

    #[test]
    fn test_create_ticket_params_omits_optional() {
        let p = CreateTicketParams {
            subject: "Sub".into(),
            body: "Body".into(),
            category: None,
            priority: None,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert!(v.get("category").is_none());
        assert!(v.get("priority").is_none());
    }

    #[test]
    fn test_ticket_deserializes() {
        let json = r#"{"id":"t-1","subject":"Login issue","status":"open","createdAt":"2026-04-01","messages":[]}"#;
        let t: Ticket = serde_json::from_str(json).unwrap();
        assert_eq!(t.id, "t-1");
        assert_eq!(t.status.as_deref(), Some("open"));
        assert!(t.messages.is_empty());
    }

    #[test]
    fn test_ticket_message_deserializes() {
        let json = r#"{"id":"msg-1","ticketId":"t-1","body":"We are looking into it.","author":"support","isStaff":true,"createdAt":"2026-04-02"}"#;
        let m: TicketMessage = serde_json::from_str(json).unwrap();
        assert_eq!(m.id, "msg-1");
        assert_eq!(m.is_staff, Some(true));
        assert_eq!(m.author.as_deref(), Some("support"));
    }

    // -----------------------------------------------------------------------
    // Notification Preferences — type tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_notification_preferences_deserializes() {
        let json = r#"{"orderFilled":true,"strategyError":false,"whaleAlert":true,"marketResolved":false,"priceAlert":true,"dailySummary":false,"marketing":false}"#;
        let np: NotificationPreferences = serde_json::from_str(json).unwrap();
        assert_eq!(np.order_filled, Some(true));
        assert_eq!(np.strategy_error, Some(false));
        assert_eq!(np.whale_alert, Some(true));
    }

    #[test]
    fn test_notification_preferences_defaults_to_none() {
        let json = r#"{}"#;
        let np: NotificationPreferences = serde_json::from_str(json).unwrap();
        assert_eq!(np.order_filled, None);
        assert_eq!(np.marketing, None);
    }

    #[test]
    fn test_update_notification_preferences_omits_none() {
        let p = UpdateNotificationPreferencesParams {
            order_filled: Some(true),
            strategy_error: None,
            whale_alert: None,
            market_resolved: None,
            price_alert: None,
            daily_summary: None,
            marketing: None,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["orderFilled"], true);
        assert!(v.get("strategyError").is_none());
    }

    // -----------------------------------------------------------------------
    // Venue Preferences — type tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_venue_preferences_deserializes() {
        let json = r#"{"defaultVenue":"polymarket","enabledVenues":["polymarket","kalshi"],"singlePlatformMode":false}"#;
        let vp: UserPreferences = serde_json::from_str(json).unwrap();
        assert_eq!(vp.default_venue.as_deref(), Some("polymarket"));
        assert_eq!(
            vp.enabled_venues.as_deref(),
            Some(&["polymarket".to_string(), "kalshi".to_string()] as &[String])
        );
        assert_eq!(vp.single_platform_mode, Some(false));
    }

    #[test]
    fn test_venue_preferences_defaults_to_none() {
        let json = r#"{}"#;
        let vp: UserPreferences = serde_json::from_str(json).unwrap();
        assert_eq!(vp.default_venue, None);
        assert_eq!(vp.enabled_venues, None);
        assert_eq!(vp.single_platform_mode, None);
    }

    #[test]
    fn test_update_venue_preferences_omits_none() {
        let p = UpdateUserPreferencesParams {
            default_venue: Some("polymarket".to_string()),
            enabled_venues: None,
            single_platform_mode: None,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["defaultVenue"], "polymarket");
        assert!(v.get("enabledVenues").is_none());
        assert!(v.get("singlePlatformMode").is_none());
    }

    // -----------------------------------------------------------------------
    // Actions Catalog — type tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_actions_schema_deserializes() {
        let json = r#"{"version":"1.0","actions":[{"name":"listMarkets","method":"GET","path":"/api/v1/markets","scope":"READ","category":"Markets"}]}"#;
        let schema: ActionsSchema = serde_json::from_str(json).unwrap();
        assert_eq!(schema.version, "1.0");
        assert_eq!(schema.actions.len(), 1);
        assert_eq!(schema.actions[0].name, "listMarkets");
        assert_eq!(schema.actions[0].method, "GET");
        assert_eq!(schema.actions[0].scope, "READ");
    }

    #[test]
    fn test_action_parameter_deserializes() {
        let json = r#"{"name":"marketId","type":"string","required":true,"in":"path","description":"Market identifier","enum":["abc","def"],"default":"abc","max":255,"min":1}"#;
        let param: ActionParameter = serde_json::from_str(json).unwrap();
        assert_eq!(param.name, "marketId");
        assert_eq!(param.param_type, "string");
        assert!(param.required);
        assert_eq!(param.param_in.as_deref(), Some("path"));
        assert_eq!(param.description.as_deref(), Some("Market identifier"));
        assert_eq!(
            param.enum_values.as_deref(),
            Some(&["abc".to_string(), "def".to_string()] as &[String])
        );
        assert_eq!(param.max, Some(255.0));
        assert_eq!(param.min, Some(1.0));
    }

    #[test]
    fn test_action_parameter_required_defaults_false() {
        let json = r#"{"name":"marketId","type":"string"}"#;
        let param: ActionParameter = serde_json::from_str(json).unwrap();
        assert_eq!(param.name, "marketId");
        assert_eq!(param.param_type, "string");
        assert!(!param.required);
    }

    #[tokio::test]
    async fn test_get_actions_dispatch_without_api_key() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0_u8; 4096];
            let n = socket.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..n]);
            assert!(
                !request.to_ascii_lowercase().contains("authorization:"),
                "get_actions request unexpectedly included Authorization header: {request}"
            );
            assert!(
                request.contains("GET /api/v1/actions HTTP/1.1"),
                "request must hit GET /api/v1/actions; got: {}",
                request.lines().next().unwrap_or("")
            );
            let body = r#"{"version":"1.0","actions":[]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                 content-type: application/json\r\n\
                 content-length: {}\r\n\
                 connection: close\r\n\
                 \r\n\
                 {}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        let client = PolyforgeClient::with_url("", format!("http://{addr}")).unwrap();
        client.get_actions().await.unwrap();
        server.await.unwrap();
    }

    // -----------------------------------------------------------------------
    // System Health — HTTP request-capture tests (POLA-3671)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_health_path_and_auth() {
        let request = capture_request(r#"{"status":"ok"}"#, |client| async move {
            client.get_health().await.map(|_| ())
        })
        .await;
        assert!(
            request.contains("GET /health HTTP/1.1"),
            "request must hit GET /health; got: {}",
            request.lines().next().unwrap_or("")
        );
        assert!(
            captured_header(&request, "Authorization").is_none(),
            "Authorization header must NOT be present for unauthenticated health check"
        );
    }

    #[tokio::test]
    async fn test_get_health_authenticated_path_and_auth() {
        let request = capture_request(r#"{"status":"operational"}"#, |client| async move {
            client.get_health_authenticated().await.map(|_| ())
        })
        .await;
        assert!(
            request.contains("GET /api/v1/status HTTP/1.1"),
            "request must hit GET /api/v1/status; got: {}",
            request.lines().next().unwrap_or("")
        );
        let auth = captured_header(&request, "Authorization")
            .expect("get_health_authenticated must include Authorization header");
        assert!(
            auth.starts_with("Bearer "),
            "Authorization must be Bearer token"
        );
    }

    #[tokio::test]
    async fn test_get_health_authenticated_deserializes_response() {
        let response_json = r#"{"status":"operational","db":{"connections":5,"status":"connected"},"redis":{"memoryUsageMb":128,"status":"connected"},"queueDepth":42}"#;
        let request = capture_request(response_json, |client| async move {
            let health = client.get_health_authenticated().await?;
            assert_eq!(health.status, "operational");
            assert_eq!(health.queue_depth, Some(42));
            assert!(health.db.is_some());
            assert!(health.redis.is_some());
            Ok(())
        })
        .await;
        assert!(request.contains("GET /api/v1/status HTTP/1.1"));
    }

    #[tokio::test]
    async fn test_get_health_dispatch_without_api_key() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0_u8; 4096];
            let n = socket.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..n]);
            assert!(
                !request.to_ascii_lowercase().contains("authorization:"),
                "get_health request unexpectedly included Authorization header: {request}"
            );
            assert!(
                request.contains("GET /health HTTP/1.1"),
                "request must hit GET /health; got: {}",
                request.lines().next().unwrap_or("")
            );
            let body = r#"{"status":"ok"}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                 content-type: application/json\r\n\
                 content-length: {}\r\n\
                 connection: close\r\n\
                 \r\n\
                 {}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        let client = PolyforgeClient::with_url("", format!("http://{addr}")).unwrap();
        client.get_health().await.unwrap();
        server.await.unwrap();
    }
    // -----------------------------------------------------------------------
    // System Health — type tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_system_health_public_deserializes() {
        let json = r#"{"status":"ok","service":"api-service","version":"2.0.0","uptime":3600}"#;
        let h: SystemHealthPublic = serde_json::from_str(json).unwrap();
        assert_eq!(h.status, "ok");
        assert_eq!(h.service.as_deref(), Some("api-service"));
        assert_eq!(h.version.as_deref(), Some("2.0.0"));
        assert_eq!(h.uptime, Some(3600));
    }

    #[test]
    fn test_system_health_authenticated_deserializes() {
        let json = r#"{"status":"operational","db":{"connections":5,"status":"connected"},"redis":{"memoryUsageMb":128,"status":"connected"},"queueDepth":42}"#;
        let h: SystemHealthAuthenticated = serde_json::from_str(json).unwrap();
        assert_eq!(h.status, "operational");
        assert!(h.db.is_some());
        assert!(h.redis.is_some());
        assert_eq!(h.queue_depth, Some(42));
    }

    #[test]
    fn test_system_health_public_defaults() {
        let json = r#"{"status":"ok"}"#;
        let h: SystemHealthPublic = serde_json::from_str(json).unwrap();
        assert_eq!(h.status, "ok");
        assert_eq!(h.service, None);
        assert_eq!(h.uptime, None);
    }

    #[test]
    fn test_ticket_category_serializes() {
        let c = TicketCategory::Billing;
        let v = serde_json::to_value(c).unwrap();
        assert_eq!(v, "BILLING");
    }

    #[test]
    fn test_ticket_priority_serializes() {
        let p = TicketPriority::Urgent;
        let v = serde_json::to_value(p).unwrap();
        assert_eq!(v, "URGENT");
    }

    // ── POLA-1841: Sports markets ─────────────────────────────────────────

    #[test]
    fn test_sports_category_deserializes() {
        let json = r#"{
            "category":"NBA","label":"Basketball — NBA",
            "seriesTickers":["KXNBAGAME","KXNBASERIES"],"marketCount":42
        }"#;
        let c: SportsCategory = serde_json::from_str(json).unwrap();
        assert_eq!(c.category, "NBA");
        assert_eq!(c.label, "Basketball — NBA");
        assert_eq!(c.series_tickers.len(), 2);
        assert_eq!(c.market_count, 42);
    }

    #[test]
    fn test_sports_category_deserializes_with_minimal_fields() {
        // Server may omit seriesTickers / marketCount on edge categories.
        let json = r#"{"category":"NHL","label":"Hockey"}"#;
        let c: SportsCategory = serde_json::from_str(json).unwrap();
        assert_eq!(c.category, "NHL");
        assert!(c.series_tickers.is_empty());
        assert_eq!(c.market_count, 0);
    }

    #[test]
    fn test_sports_combo_selection_serializes_camel_case() {
        let s = SportsComboSelection {
            market_ticker: "KXNBAGAME-25-LAL".into(),
            event_ticker: "KXNBAGAME-25".into(),
            side: "yes".into(),
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["marketTicker"], "KXNBAGAME-25-LAL");
        assert_eq!(v["eventTicker"], "KXNBAGAME-25");
        assert_eq!(v["side"], "yes");
    }

    #[test]
    fn test_sports_combo_lookup_params_serializes_camel_case() {
        let p = SportsComboLookupParams {
            collection_ticker: "KXNBACOMBO".into(),
            selected_markets: vec![SportsComboSelection {
                market_ticker: "M1".into(),
                event_ticker: "E1".into(),
                side: "no".into(),
            }],
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["collectionTicker"], "KXNBACOMBO");
        assert_eq!(v["selectedMarkets"][0]["marketTicker"], "M1");
        assert_eq!(v["selectedMarkets"][0]["side"], "no");
    }

    #[test]
    fn test_sports_paginated_value_response_deserializes() {
        // /sports/markets and /sports/events return PaginatedResponse<Record<string,unknown>>.
        let json = r#"{
            "data":[{"marketTicker":"KXNBA-1","price":0.55}],
            "total":1,"page":1,"limit":20,"totalPages":1,"hasNext":false
        }"#;
        let resp: PaginatedResponse<serde_json::Value> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.total, 1);
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0]["marketTicker"], "KXNBA-1");
    }

    // Path-construction tests — verify each new method targets the documented
    // endpoint. These mirror the existing arbitrage / profile path tests.

    #[test]
    fn test_list_sports_categories_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/sports/categories");
        assert!(url.ends_with("/api/v1/sports/categories"));
    }

    #[test]
    fn test_list_sports_markets_path_no_params() {
        // Empty params produce no query string.
        let params = ListSportsMarketsParams::default();
        let mut qp: Vec<(&str, String)> = Vec::new();
        if let Some(p) = params.page {
            qp.push(("page", p.to_string()));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        assert!(qp.is_empty());
    }

    #[test]
    fn test_list_sports_markets_path_with_filters() {
        // Confirm the param struct exposes every documented filter.
        let p = ListSportsMarketsParams {
            page: Some(2),
            limit: Some(10),
            category: Some("NBA".into()),
            search: Some("Lakers".into()),
            series_ticker: Some("KXNBAGAME".into()),
            event_ticker: Some("KXNBAGAME-25".into()),
            live_only: Some(true),
            sort: Some("closing_soon".into()),
        };
        assert_eq!(p.page, Some(2));
        assert_eq!(p.limit, Some(10));
        assert_eq!(p.category.as_deref(), Some("NBA"));
        assert_eq!(p.search.as_deref(), Some("Lakers"));
        assert_eq!(p.series_ticker.as_deref(), Some("KXNBAGAME"));
        assert_eq!(p.event_ticker.as_deref(), Some("KXNBAGAME-25"));
        assert_eq!(p.live_only, Some(true));
        assert_eq!(p.sort.as_deref(), Some("closing_soon"));
    }

    #[test]
    fn test_list_sports_events_param_struct_full() {
        let p = ListSportsEventsParams {
            page: Some(1),
            limit: Some(50),
            category: Some("NFL".into()),
            series_ticker: Some("KXNFLGAME".into()),
            status: Some("LIVE".into()),
        };
        assert_eq!(p.status.as_deref(), Some("LIVE"));
        assert_eq!(p.category.as_deref(), Some("NFL"));
    }

    #[test]
    fn test_get_sports_event_path_encodes_ticker() {
        let client = PolyforgeClient::new("k").unwrap();
        // Tickers can contain `/` or special chars in theory — confirm encoding runs.
        let path = format!("/api/v1/sports/events/{}", encode("KXNBAGAME-25/LAL"));
        let url = client.url(&path);
        assert!(url.contains("/api/v1/sports/events/KXNBAGAME-25%2FLAL"));
    }

    #[test]
    fn test_list_sports_milestones_param_struct() {
        let p = ListSportsMilestonesParams {
            page: Some(1),
            limit: Some(20),
            event_ticker: Some("KXNBAGAME-25".into()),
            status: Some("RESOLVED".into()),
        };
        assert_eq!(p.event_ticker.as_deref(), Some("KXNBAGAME-25"));
        assert_eq!(p.status.as_deref(), Some("RESOLVED"));
    }

    #[test]
    fn test_get_sports_live_data_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let path = format!("/api/v1/sports/live-data/{}", encode("ms-123"));
        let url = client.url(&path);
        assert!(url.ends_with("/api/v1/sports/live-data/ms-123"));
    }

    #[test]
    fn test_list_sports_combos_param_struct() {
        let p = ListSportsCombosParams {
            page: Some(1),
            limit: Some(10),
            series_ticker: Some("KXNBACOMBO".into()),
        };
        assert_eq!(p.series_ticker.as_deref(), Some("KXNBACOMBO"));
    }

    #[test]
    fn test_get_sports_combo_collection_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let path = format!("/api/v1/sports/combos/{}", encode("KXNBACOMBO-1"));
        let url = client.url(&path);
        assert!(url.ends_with("/api/v1/sports/combos/KXNBACOMBO-1"));
    }

    #[test]
    fn test_lookup_sports_combo_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/sports/combos/lookup");
        assert!(url.ends_with("/api/v1/sports/combos/lookup"));
    }

    #[test]
    fn test_lookup_sports_combo_result_can_be_null() {
        // The endpoint may return JSON `null` when no combo matches.
        let v: serde_json::Value = serde_json::from_str("null").unwrap();
        assert!(v.is_null());
    }

    #[test]
    fn test_lookup_sports_combo_result_deserializes() {
        let json = r#"{"eventTicker":"KXNBAGAME-25","marketTicker":"KXNBAGAME-25-LAL"}"#;
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(v["eventTicker"], "KXNBAGAME-25");
        assert_eq!(v["marketTicker"], "KXNBAGAME-25-LAL");
    }

    // ── POLA-1844: Public user profile lookups ────────────────────────────

    #[test]
    fn test_user_performance_point_deserializes() {
        let json = r#"{"date":"2026-04-01","pnl":12.5,"cumPnl":12.5}"#;
        let p: UserPerformancePoint = serde_json::from_str(json).unwrap();
        assert_eq!(p.date, "2026-04-01");
        assert_eq!(p.pnl, 12.5);
        assert_eq!(p.cum_pnl, 12.5);
    }

    #[test]
    fn test_user_data_envelope_deserializes() {
        let json = r#"{"data":[{"date":"2026-04-01","pnl":1.0,"cumPnl":1.0}]}"#;
        let env: UserDataEnvelope<UserPerformancePoint> = serde_json::from_str(json).unwrap();
        assert_eq!(env.data.len(), 1);
        assert_eq!(env.data[0].date, "2026-04-01");
    }

    #[test]
    fn test_user_strategy_summary_deserializes() {
        let json = r#"{
            "id":"s1","name":"Strat","description":"d",
            "winRate":0.5,"tradeCount":3,"priceUsdc":0.0,
            "forkCount":1,"likeCount":2,"isLiked":false
        }"#;
        let s: UserStrategySummary = serde_json::from_str(json).unwrap();
        assert_eq!(s.id, "s1");
        assert_eq!(s.win_rate, 0.5);
        assert_eq!(s.trade_count, 3);
        assert!(!s.is_liked);
    }

    #[test]
    fn test_user_activity_entry_deserializes() {
        let json = r#"{
            "id":"p1","marketQuestion":"Will it rain?","outcome":"YES",
            "side":"YES","size":100.0,"pnl":5.0,"resolvedAt":"2026-04-01T00:00:00.000Z"
        }"#;
        let a: UserActivityEntry = serde_json::from_str(json).unwrap();
        assert_eq!(a.id, "p1");
        assert_eq!(a.market_question, "Will it rain?");
        assert_eq!(a.size, 100.0);
        assert_eq!(a.resolved_at, "2026-04-01T00:00:00.000Z");
    }

    #[test]
    fn test_user_profile_badge_deserializes() {
        let json = r#"{"id":"EARLY_ADOPTER","unlockedAt":"2026-01-01T00:00:00.000Z"}"#;
        let b: UserProfileBadge = serde_json::from_str(json).unwrap();
        assert_eq!(b.id, "EARLY_ADOPTER");
        assert_eq!(b.unlocked_at, "2026-01-01T00:00:00.000Z");
    }

    #[test]
    fn test_followed_user_deserializes_with_nulls() {
        let json = r#"{
            "id":"u1","username":"alice","displayName":null,"avatarUrl":null
        }"#;
        let u: FollowedUser = serde_json::from_str(json).unwrap();
        assert_eq!(u.id, "u1");
        assert_eq!(u.username, "alice");
        assert!(u.display_name.is_none());
        assert!(u.avatar_url.is_none());
    }

    #[test]
    fn test_followed_user_deserializes_with_values() {
        let json = r#"{
            "id":"u1","username":"alice",
            "displayName":"Alice","avatarUrl":"https://x/a.png"
        }"#;
        let u: FollowedUser = serde_json::from_str(json).unwrap();
        assert_eq!(u.display_name.as_deref(), Some("Alice"));
        assert_eq!(u.avatar_url.as_deref(), Some("https://x/a.png"));
    }

    #[test]
    fn test_following_paginated_response_deserializes() {
        let json = r#"{
            "data":[{"id":"u1","username":"alice","displayName":"Alice","avatarUrl":null}],
            "total":1,"page":1,"limit":20,"totalPages":1,"hasNext":false
        }"#;
        let resp: PaginatedResponse<FollowedUser> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.total, 1);
        assert_eq!(resp.page, 1);
        assert_eq!(resp.limit, 20);
        assert_eq!(resp.total_pages, 1);
        assert!(!resp.has_next);
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0].username, "alice");
    }

    // -----------------------------------------------------------------------
    // Misc public utility endpoints (POLA-1858)
    // -----------------------------------------------------------------------

    #[test]
    fn test_accuracy_overview_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/accuracy");
        assert!(url.ends_with("/api/v1/accuracy"));
    }

    #[test]
    fn test_accuracy_leaderboard_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/accuracy/leaderboard");
        assert!(url.ends_with("/api/v1/accuracy/leaderboard"));
    }

    #[test]
    fn test_accuracy_leaderboard_entry_deserializes() {
        let json = r#"{
            "rank": 51,
            "userId": "u-1",
            "username": "alice",
            "displayName": "Alice",
            "avatarUrl": "https://example.com/avatar.png",
            "pnl": "12.50",
            "winRate": "55.0",
            "tradeCount": 42
        }"#;
        let entry: AccuracyLeaderboardEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.rank, 51);
        assert_eq!(entry.user_id, "u-1");
        assert_eq!(entry.username, "alice");
        assert_eq!(entry.display_name.as_deref(), Some("Alice"));
        assert_eq!(
            entry.avatar_url.as_deref(),
            Some("https://example.com/avatar.png")
        );
        assert_eq!(entry.pnl, "12.50");
        assert_eq!(entry.win_rate, "55.0");
        assert_eq!(entry.trade_count, 42);
    }

    #[test]
    fn test_accuracy_leaderboard_entry_deserializes_with_nulls() {
        let json = r#"{
            "rank": 1,
            "userId": "u-2",
            "username": "bob",
            "displayName": null,
            "avatarUrl": null,
            "pnl": "100.00",
            "winRate": "80.0",
            "tradeCount": 5
        }"#;
        let entry: AccuracyLeaderboardEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.rank, 1);
        assert_eq!(entry.display_name, None);
        assert_eq!(entry.avatar_url, None);
    }

    #[test]
    fn test_accuracy_leaderboard_params_default() {
        let params = AccuracyLeaderboardParams::default();
        assert!(params.period.is_none());
        assert!(params.limit.is_none());
        assert!(params.page.is_none());
        assert!(params.offset.is_none());
        assert!(params.cursor.is_none());
    }

    #[test]
    fn test_accuracy_leaderboard_params_has_offset() {
        let params = AccuracyLeaderboardParams {
            period: Some("30d".to_string()),
            limit: Some(25),
            offset: Some(50),
            ..Default::default()
        };
        assert_eq!(params.period.as_deref(), Some("30d"));
        assert_eq!(params.limit, Some(25));
        assert_eq!(params.offset, Some(50));
        assert!(params.page.is_none());
    }

    #[test]
    fn test_feed_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/feed");
        assert!(url.ends_with("/api/v1/feed"));
    }

    #[test]
    fn test_journal_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/journal");
        assert!(url.ends_with("/api/v1/journal"));
    }

    #[test]
    fn test_notifications_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/notifications");
        assert!(url.ends_with("/api/v1/notifications"));
    }

    #[test]
    fn test_referrals_me_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/referrals/me");
        assert!(url.ends_with("/api/v1/referrals/me"));
    }

    #[test]
    fn test_fees_preview_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/fees/preview");
        assert!(url.ends_with("/api/v1/fees/preview"));
    }

    #[test]
    fn test_fees_schedules_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/fees/schedules");
        assert!(url.ends_with("/api/v1/fees/schedules"));
    }

    #[test]
    fn test_market_alerts_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let path = format!("/api/v1/markets/{}/alerts", encode("market-42"));
        let url = client.url(&path);
        assert!(url.contains("/api/v1/markets/market-42/alerts"));
    }

    #[test]
    fn test_market_alert_delete_path_encodes_uuid() {
        let client = PolyforgeClient::new("k").unwrap();
        let alert_id = "8f1d2a3b-1234-4abc-9def-1234567890ab";
        let path = format!(
            "/api/v1/markets/{}/alerts/{}",
            encode("market-42"),
            encode(alert_id)
        );
        let url = client.url(&path);
        assert!(url.contains(&format!("/api/v1/markets/market-42/alerts/{}", alert_id)));
    }

    #[test]
    fn test_market_history_path_no_period() {
        let client = PolyforgeClient::new("k").unwrap();
        let path = format!("/api/v1/markets/{}/history", encode("market-42"));
        let url = client.url(&path);
        assert!(url.ends_with("/api/v1/markets/market-42/history"));
    }

    #[test]
    fn test_market_history_period_serializes() {
        assert_eq!(MarketHistoryPeriod::OneDay.as_str(), "1d");
        assert_eq!(MarketHistoryPeriod::SevenDays.as_str(), "7d");
        assert_eq!(MarketHistoryPeriod::ThirtyDays.as_str(), "30d");
        assert_eq!(MarketHistoryPeriod::NinetyDays.as_str(), "90d");
    }

    #[test]
    fn test_market_sentiment_report_path_distinct_from_news() {
        // Sanity-check that get_market_sentiment_report and the existing
        // get_market_sentiment hit different routes.
        let client = PolyforgeClient::new("k").unwrap();
        let report_path = format!("/api/v1/markets/{}/sentiment", encode("m-1"));
        let news_path = format!("/api/v1/news/sentiment/{}", encode("m-1"));
        let report_url = client.url(&report_path);
        let news_url = client.url(&news_path);
        assert_ne!(report_url, news_url);
        assert!(report_url.contains("/markets/m-1/sentiment"));
        assert!(news_url.contains("/news/sentiment/m-1"));
    }

    #[test]
    fn test_market_sentiment_report_deserializes_platform_vote_shape() {
        let json = serde_json::json!({
            "yesPercent": 60,
            "noPercent": 40,
            "totalVotes": 5,
            "userVote": {
                "direction": "BUY",
                "confidence": 0.82
            }
        });
        let report: MarketSentimentReport = serde_json::from_value(json).unwrap();
        assert_eq!(report.yes_percent, 60);
        assert_eq!(report.no_percent, 40);
        assert_eq!(report.total_votes, 5);
        let vote = report.user_vote.as_ref().unwrap();
        assert_eq!(vote.direction, "BUY");
        assert!((vote.confidence - 0.82).abs() < f64::EPSILON);
    }

    #[test]
    fn test_market_sentiment_report_deserializes_null_user_vote() {
        let json = serde_json::json!({
            "yesPercent": 67,
            "noPercent": 33,
            "totalVotes": 3,
            "userVote": null
        });
        let report: MarketSentimentReport = serde_json::from_value(json).unwrap();
        assert_eq!(report.yes_percent, 67);
        assert_eq!(report.no_percent, 33);
        assert_eq!(report.total_votes, 3);
        assert!(report.user_vote.is_none());
    }

    #[test]
    fn test_vote_market_sentiment_params_preserves_fractional_confidence() {
        let params = VoteMarketSentimentParams {
            direction: "BUY".into(),
            confidence: 0.82,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["direction"], "BUY");
        assert_eq!(json["confidence"], serde_json::json!(0.82));
    }

    #[test]
    fn test_export_personal_data_path() {
        let client = PolyforgeClient::new("k").unwrap();
        assert!(client
            .url("/api/v1/me/export")
            .ends_with("/api/v1/me/export"));
        assert!(client
            .url("/api/v1/me/export?format=csv")
            .ends_with("/api/v1/me/export?format=csv"));
    }

    #[test]
    fn test_personal_data_export_deserializes() {
        let json = serde_json::json!({
            "generatedAt": "2026-05-10T00:00:00.000Z",
            "formatVersion": "2026-05-privacy-export-v1",
            "_meta": {
                "maxRecordsPerCollection": 1000,
                "collectionsTruncated": { "orders": 1000, "positions": 850 }
            },
            "account": { "email": "user@example.com", "username": "trader1" },
            "settings": { "notificationPreferences": {} },
            "security": { "apiKeys": [], "loginHistory": [] },
            "trading": { "strategies": [], "orders": [] },
            "communications": { "notificationHistory": [], "tickets": [] },
            "social": { "follows": [], "priceAlerts": [] }
        });
        let export: PersonalDataExport = serde_json::from_value(json).unwrap();
        assert_eq!(
            export.generated_at.as_deref(),
            Some("2026-05-10T00:00:00.000Z")
        );
        assert_eq!(
            export.format_version.as_deref(),
            Some("2026-05-privacy-export-v1")
        );
        let meta = export.meta.as_ref().unwrap();
        assert_eq!(meta.max_records_per_collection, Some(1000));
        let truncated = meta.collections_truncated.as_ref().unwrap();
        assert_eq!(truncated["orders"], 1000);
        assert_eq!(truncated["positions"], 850);
        assert_eq!(export.account["email"], "user@example.com");
        assert_eq!(export.account["username"], "trader1");
    }

    #[test]
    fn test_personal_data_export_deserializes_minimal() {
        let json = serde_json::json!({
            "generatedAt": "2026-05-10T00:00:00.000Z",
            "formatVersion": "v1",
            "_meta": {},
            "account": {},
            "settings": {},
            "security": {},
            "trading": {},
            "communications": {},
            "social": {}
        });
        let export: PersonalDataExport = serde_json::from_value(json).unwrap();
        assert_eq!(
            export.generated_at.as_deref(),
            Some("2026-05-10T00:00:00.000Z")
        );
        assert!(export.meta.is_some());
    }

    #[test]
    fn test_order_journal_patch_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let path = format!("/api/v1/orders/{}/journal", encode("ord-1"));
        let url = client.url(&path);
        assert!(url.ends_with("/api/v1/orders/ord-1/journal"));
    }

    #[test]
    fn test_combo_collections_paths() {
        let client = PolyforgeClient::new("k").unwrap();
        assert!(client
            .url("/api/v1/markets/combo/collections")
            .ends_with("/api/v1/markets/combo/collections"));
        let one = format!("/api/v1/markets/combo/collections/{}", encode("KXNFL-25"));
        let url = client.url(&one);
        assert!(url.contains("/api/v1/markets/combo/collections/KXNFL-25"));
        assert!(client
            .url("/api/v1/markets/combo/lookup")
            .ends_with("/api/v1/markets/combo/lookup"));
    }

    #[test]
    fn test_correlation_categories_path() {
        let client = PolyforgeClient::new("k").unwrap();
        let url = client.url("/api/v1/analytics/correlation/categories");
        assert!(url.ends_with("/api/v1/analytics/correlation/categories"));
    }

    #[test]
    fn test_order_preview_params_serialize_camel_case() {
        let p = OrderPreviewParams {
            token_id: "tok-1".into(),
            side: PreviewSide::Buy,
            size: 100.0,
            price: 0.55,
            order_type: Some("POST_ONLY".into()),
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["tokenId"], "tok-1");
        assert_eq!(v["side"], "BUY");
        assert_eq!(v["size"], 100.0);
        assert_eq!(v["price"], 0.55);
        assert_eq!(v["orderType"], "POST_ONLY");
    }

    #[test]
    fn test_order_preview_params_omits_none_order_type() {
        let p = OrderPreviewParams {
            token_id: "tok-1".into(),
            side: PreviewSide::Sell,
            size: 50.0,
            price: 0.5,
            order_type: None,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert!(v.get("orderType").is_none());
        assert_eq!(v["side"], "SELL");
    }

    #[tokio::test]
    async fn test_preview_fees_rejects_size_below_one() {
        let client = PolyforgeClient::new("k").unwrap();
        let p = OrderPreviewParams {
            token_id: "tok-1".into(),
            side: PreviewSide::Buy,
            size: 0.5,
            price: 0.5,
            order_type: None,
        };
        let err = client.preview_fees(&p).await.unwrap_err();
        assert!(matches!(err, PolyforgeError::Validation(_)));
    }

    #[tokio::test]
    async fn test_preview_fees_rejects_price_out_of_band() {
        let client = PolyforgeClient::new("k").unwrap();
        let too_low = OrderPreviewParams {
            token_id: "tok-1".into(),
            side: PreviewSide::Buy,
            size: 10.0,
            price: 0.0005,
            order_type: None,
        };
        assert!(matches!(
            client.preview_fees(&too_low).await.unwrap_err(),
            PolyforgeError::Validation(_)
        ));
        let too_high = OrderPreviewParams {
            token_id: "tok-1".into(),
            side: PreviewSide::Buy,
            size: 10.0,
            price: 0.9995,
            order_type: None,
        };
        assert!(matches!(
            client.preview_fees(&too_high).await.unwrap_err(),
            PolyforgeError::Validation(_)
        ));
    }

    #[tokio::test]
    async fn test_preview_fees_rejects_nonfinite_inputs() {
        let client = PolyforgeClient::new("k").unwrap();
        let nan_size = OrderPreviewParams {
            token_id: "tok-1".into(),
            side: PreviewSide::Buy,
            size: f64::NAN,
            price: 0.5,
            order_type: None,
        };
        assert!(matches!(
            client.preview_fees(&nan_size).await.unwrap_err(),
            PolyforgeError::Validation(_)
        ));
        let inf_price = OrderPreviewParams {
            token_id: "tok-1".into(),
            side: PreviewSide::Buy,
            size: 10.0,
            price: f64::INFINITY,
            order_type: None,
        };
        assert!(matches!(
            client.preview_fees(&inf_price).await.unwrap_err(),
            PolyforgeError::Validation(_)
        ));
    }

    #[test]
    fn test_create_market_alert_params_serialize() {
        let p = CreateMarketAlertParams {
            outcome: MarketAlertOutcome::Yes,
            condition: MarketAlertCondition::Above,
            threshold: 0.6,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["outcome"], "YES");
        assert_eq!(v["condition"], "above");
        assert_eq!(v["threshold"], 0.6);
    }

    #[test]
    fn test_market_alert_no_outcome_serializes_uppercase() {
        let p = CreateMarketAlertParams {
            outcome: MarketAlertOutcome::No,
            condition: MarketAlertCondition::Below,
            threshold: 0.4,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["outcome"], "NO");
        assert_eq!(v["condition"], "below");
    }

    #[test]
    fn test_market_alerts_response_deserializes_data_envelope() {
        let json = serde_json::json!({
            "data": [
                {
                    "id": "alert-1",
                    "marketId": "market-1",
                    "outcome": "YES",
                    "condition": "above",
                    "threshold": 0.6,
                    "triggered": false,
                    "createdAt": "2026-05-05T00:00:00Z"
                }
            ]
        });
        let response: MarketAlertsResponse = serde_json::from_value(json).unwrap();
        assert_eq!(response.data.len(), 1);
        assert_eq!(response.data[0].id.as_deref(), Some("alert-1"));
        assert_eq!(response.data[0].market_id.as_deref(), Some("market-1"));
    }

    #[test]
    fn test_update_order_journal_params_serialize() {
        let p = UpdateOrderJournalParams {
            mood: OrderJournalMood::Confident,
            note: Some("entered at support level".into()),
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["mood"], "CONFIDENT");
        assert_eq!(v["note"], "entered at support level");
    }

    #[test]
    fn test_update_order_journal_omits_none_note() {
        let p = UpdateOrderJournalParams {
            mood: OrderJournalMood::Disciplined,
            note: None,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["mood"], "DISCIPLINED");
        assert!(v.get("note").is_none());
    }

    #[test]
    fn test_combo_lookup_params_serialize() {
        let p = ComboLookupParams {
            collection_ticker: "KXNFL-COMBO-01".into(),
            legs: vec![
                ComboLeg {
                    ticker: "KXNFL-25SEPCHIDET-CHI".into(),
                    outcome: "yes".into(),
                },
                ComboLeg {
                    ticker: "KXNFL-25SEPGBPHI-GB".into(),
                    outcome: "no".into(),
                },
            ],
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["collectionTicker"], "KXNFL-COMBO-01");
        assert_eq!(v["legs"][0]["ticker"], "KXNFL-25SEPCHIDET-CHI");
        assert_eq!(v["legs"][0]["outcome"], "yes");
        assert_eq!(v["legs"][1]["outcome"], "no");
    }

    #[test]
    fn test_journal_entry_deserializes_with_extra_fields() {
        let json = serde_json::json!({
            "id": "o1",
            "marketId": "m1",
            "mood": "CONFIDENT",
            "note": "High conviction",
            "side": "BUY",
            "outcome": "YES",
            "price": "0.55",
            "size": "100",
            "status": "CONFIRMED",
            "createdAt": "2026-05-01T00:00:00Z",
            "futureField": "ignored"
        });
        let entry: JournalEntry = serde_json::from_value(json).unwrap();
        assert_eq!(entry.id.as_deref(), Some("o1"));
        assert_eq!(entry.market_id.as_deref(), Some("m1"));
        assert_eq!(entry.mood.as_deref(), Some("CONFIDENT"));
        assert_eq!(entry.note.as_deref(), Some("High conviction"));
        assert_eq!(entry.side.as_deref(), Some("BUY"));
        assert_eq!(entry.outcome.as_deref(), Some("YES"));
        assert_eq!(entry.price.as_deref(), Some("0.55"));
        assert_eq!(entry.size.as_deref(), Some("100"));
        assert_eq!(entry.status.as_deref(), Some("CONFIRMED"));
        assert_eq!(entry.created_at.as_deref(), Some("2026-05-01T00:00:00Z"));
        assert_eq!(
            entry.extra.get("futureField").and_then(|v| v.as_str()),
            Some("ignored")
        );
    }

    #[test]
    fn test_fee_schedules_deserializes_grouped_payload() {
        let json = serde_json::json!({
            "polymarket": [
                { "category": "Politics", "role": "TAKER", "feeBps": 200, "effectiveAt": "2026-01-01T00:00:00Z" }
            ],
            "kalshi": [
                { "role": "MAKER", "feeBps": 0, "minPrice": 0.01, "maxPrice": 0.99, "effectiveAt": "2026-01-01T00:00:00Z" }
            ]
        });
        let schedules: FeeSchedules = serde_json::from_value(json).unwrap();
        assert_eq!(schedules.polymarket.len(), 1);
        assert_eq!(schedules.kalshi.len(), 1);
        assert_eq!(
            schedules.polymarket[0].category.as_deref(),
            Some("Politics")
        );
        assert_eq!(schedules.kalshi[0].fee_bps, Some(0.0));
    }

    #[test]
    fn test_my_referrals_response_deserializes() {
        let json = serde_json::json!({
            "referralCode": "ABCD1234",
            "referralLink": "https://polyforge.trade/ref/ABCD1234",
            "stats": {
                "invited": 1,
                "signedUp": 0,
                "active": 0,
                "creditsEarned": 0
            },
            "referrals": []
        });
        let info: MyReferralsResponse = serde_json::from_value(json).unwrap();
        assert_eq!(info.referral_code.as_deref(), Some("ABCD1234"));
        assert_eq!(info.stats.as_ref().unwrap().invited, 1);
    }

    #[test]
    #[allow(deprecated)]
    fn test_deprecated_referrals_alias_deserializes() {
        let json = serde_json::json!({
            "referralCode": "ABCD1234",
            "referralLink": "https://polyforge.trade/ref/ABCD1234",
            "stats": {
                "invited": 1,
                "signedUp": 0,
                "active": 0,
                "creditsEarned": 0
            },
            "referrals": []
        });
        let info: ReferralsInfo = serde_json::from_value(json).unwrap();
        assert_eq!(info.referral_code.as_deref(), Some("ABCD1234"));
        assert_eq!(info.stats.as_ref().unwrap().invited, 1);
    }

    #[test]
    #[allow(deprecated)]
    fn test_deprecated_get_notifications_alias_compiles() {
        let client = PolyforgeClient::new("k").unwrap();
        let future = client.get_notifications(None);
        drop(future);
    }

    #[test]
    fn test_correlation_categories_deserializes() {
        let json = serde_json::json!({
            "categories": ["Politics", "Sports"],
            "matrix": [[1.0, 0.5], [0.5, 1.0]],
            "updatedAt": "2026-05-01T00:00:00Z"
        });
        let cc: CategoryCorrelation = serde_json::from_value(json).unwrap();
        assert_eq!(cc.categories.len(), 2);
        assert_eq!(cc.matrix.len(), 2);
        assert!((cc.matrix[0][0] - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_max_gdpr_export_size_constant_is_500mib() {
        assert_eq!(MAX_GDPR_EXPORT_SIZE, 500 * 1024 * 1024);
    }

    #[test]
    fn test_personal_data_export_deserializes_full() {
        let json = serde_json::json!({
            "generatedAt": "2026-05-12T10:30:00Z",
            "formatVersion": "2.0",
            "_meta": {
                "collectionsTruncated": [],
                "maxRecordsPerCollection": 1000
            },
            "account": {
                "userId": "user-abc",
                "email": "user@example.com",
                "username": "trader1",
                "createdAt": "2025-01-01T00:00:00Z"
            },
            "settings": {
                "timezone": "UTC",
                "notifications": { "email": true }
            },
            "security": {
                "mfaEnabled": true,
                "lastLogin": "2026-05-12T09:00:00Z"
            },
            "trading": {
                "totalOrders": 142,
                "totalVolumeUsdc": 50000.0
            },
            "communications": {
                "tickets": 3
            },
            "social": {
                "followers": 12,
                "following": 5
            }
        });
        let export: PersonalDataExport = serde_json::from_value(json).unwrap();
        assert_eq!(export.generated_at.as_deref(), Some("2026-05-12T10:30:00Z"));
        assert_eq!(export.format_version.as_deref(), Some("2.0"));
        let meta = export.meta.unwrap();
        assert!(meta.collections_truncated.is_some());
        assert_eq!(meta.max_records_per_collection, Some(1000));
        assert_eq!(export.account["userId"], "user-abc");
        assert_eq!(export.settings["timezone"], "UTC");
        assert_eq!(export.security["mfaEnabled"], true);
        assert_eq!(export.trading["totalOrders"], 142);
        assert_eq!(export.communications["tickets"], 3);
        assert_eq!(export.social["followers"], 12);
    }

    #[test]
    fn test_personal_data_export_preserves_extra_fields() {
        let json = serde_json::json!({
            "generatedAt": "2026-05-12T10:30:00Z",
            "formatVersion": "1.0",
            "futureField": "forward-compat"
        });
        let export: PersonalDataExport = serde_json::from_value(json).unwrap();
        assert_eq!(export.extra["futureField"], "forward-compat");
    }

    #[tokio::test]
    async fn test_export_personal_data_uses_max_body_size_endpoint() {
        let resp = capture_request(
            r#"{"generatedAt":"t","formatVersion":"1"}"#,
            |client| async move { client.export_personal_data().await.map(|_| ()) },
        )
        .await;
        assert!(resp.contains("GET /api/v1/me/export HTTP/1.1"));
    }

    #[tokio::test]
    async fn test_export_personal_data_csv_uses_max_body_size_endpoint() {
        let resp = capture_request("", |client| async move {
            client.export_personal_data_csv().await.map(|_| ())
        })
        .await;
        assert!(resp.contains("GET /api/v1/me/export?format=csv HTTP/1.1"));
    }

    // --- GDPR export body-size cap enforcement (success responses) ---

    #[tokio::test]
    async fn test_handle_response_with_max_rejects_oversized_success_body() {
        let body = http::Response::builder()
            .status(200)
            .header("content-length", (MAX_GDPR_EXPORT_SIZE + 1).to_string())
            .body("")
            .unwrap();
        let resp = reqwest::Response::from(body);

        let client = PolyforgeClient::with_url("test-key", "http://localhost:3002").unwrap();
        let result: std::result::Result<serde_json::Value, _> = client
            .handle_response_with_max(resp, Some(MAX_GDPR_EXPORT_SIZE))
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            PolyforgeError::Api { code, status, .. } => {
                assert_eq!(code, "RESPONSE_BODY_TOO_LARGE");
                assert_eq!(status, 200);
            }
            other => panic!(
                "Expected Api error with RESPONSE_BODY_TOO_LARGE, got: {:?}",
                other
            ),
        }
    }

    #[tokio::test]
    async fn test_handle_response_with_max_allows_small_success_body() {
        let json = r#"{"ok":true}"#;
        let body = http::Response::builder()
            .status(200)
            .header("content-length", json.len().to_string())
            .header("content-type", "application/json")
            .body(json.to_string())
            .unwrap();
        let resp = reqwest::Response::from(body);

        let client = PolyforgeClient::with_url("test-key", "http://localhost:3002").unwrap();
        let result: std::result::Result<serde_json::Value, _> = client
            .handle_response_with_max(resp, Some(MAX_GDPR_EXPORT_SIZE))
            .await;

        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["ok"], true);
    }

    #[tokio::test]
    async fn test_handle_response_with_max_rejects_oversized_error_body() {
        let error_json = r#"{"code":"ERR","message":"boom"}"#;
        let body = http::Response::builder()
            .status(500)
            .header("content-length", (MAX_GDPR_EXPORT_SIZE + 1).to_string())
            .body(error_json.to_string())
            .unwrap();
        let resp = reqwest::Response::from(body);

        let client = PolyforgeClient::with_url("test-key", "http://localhost:3002").unwrap();
        let result: std::result::Result<serde_json::Value, _> = client
            .handle_response_with_max(resp, Some(MAX_GDPR_EXPORT_SIZE))
            .await;

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
    async fn test_handle_response_with_max_handles_204() {
        let body = http::Response::builder().status(204).body("").unwrap();
        let resp = reqwest::Response::from(body);

        let client = PolyforgeClient::with_url("test-key", "http://localhost:3002").unwrap();
        let result: std::result::Result<serde_json::Value, _> = client
            .handle_response_with_max(resp, Some(MAX_GDPR_EXPORT_SIZE))
            .await;

        assert!(result.is_ok());
    }
}
