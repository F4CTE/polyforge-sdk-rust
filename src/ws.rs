use std::collections::BTreeMap;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use url::Url;

use crate::errors::{PolyforgeError, Result};

/// Default path for the Polyforge API gateway WebSocket.
pub const DEFAULT_WS_PATH: &str = "/ws";

type GatewayStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Authentication and reconnect options for [`PolyforgeClient::connect_ws`](crate::PolyforgeClient::connect_ws).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsConnectOptions {
    /// JWT to send as the deprecated `?token=` gateway query parameter.
    ///
    /// The platform also accepts a `pf_token` cookie, but this Rust SDK does not
    /// manage browser sessions. Prefer passing an explicit short-lived JWT here.
    pub token: Option<String>,
    /// WebSocket path. Defaults to `/ws`.
    pub path: String,
    /// Reconnect policy used by [`GatewayWsClient::reconnect`] and
    /// [`GatewayWsClient::next_message_with_reconnect`].
    pub reconnect: WsReconnectOptions,
}

impl Default for WsConnectOptions {
    fn default() -> Self {
        Self {
            token: None,
            path: DEFAULT_WS_PATH.to_string(),
            reconnect: WsReconnectOptions::default(),
        }
    }
}

impl WsConnectOptions {
    /// Build options that authenticate with an explicit gateway JWT.
    pub fn with_token(token: impl Into<String>) -> Self {
        Self {
            token: Some(token.into()),
            ..Default::default()
        }
    }
}

/// Bounded reconnect policy for gateway WebSocket clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WsReconnectOptions {
    /// Maximum number of reconnect attempts before returning the last error.
    pub max_attempts: u32,
    /// Initial delay before the first retry.
    pub initial_delay: Duration,
    /// Maximum delay between retries.
    pub max_delay: Duration,
}

impl Default for WsReconnectOptions {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(250),
            max_delay: Duration::from_secs(5),
        }
    }
}

/// Client messages accepted by the Polyforge `/ws` gateway.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WsClientMessage {
    Ping,
    SubscribePrices {
        #[serde(rename = "tokenIds")]
        token_ids: Vec<String>,
    },
    UnsubscribePrices {
        #[serde(rename = "tokenIds")]
        token_ids: Vec<String>,
    },
    SubscribeWhales {
        #[serde(rename = "minSize", skip_serializing_if = "Option::is_none")]
        min_size: Option<f64>,
    },
    UnsubscribeWhales,
}

/// Typed `PRICE_UPDATE` payload from the gateway.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WsPriceUpdate {
    pub token_id: String,
    pub price: f64,
    #[serde(default)]
    pub timestamp: Option<serde_json::Value>,
}

/// Typed server messages emitted by the Polyforge `/ws` gateway.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum WsServerMessage {
    AuthOk {
        data: serde_json::Value,
        timestamp: Option<serde_json::Value>,
    },
    Pong {
        data: serde_json::Value,
        timestamp: Option<serde_json::Value>,
    },
    PriceUpdate {
        data: WsPriceUpdate,
        timestamp: Option<serde_json::Value>,
    },
    WhaleTrade {
        data: serde_json::Value,
        timestamp: Option<serde_json::Value>,
    },
    NewsSignal {
        data: serde_json::Value,
        timestamp: Option<serde_json::Value>,
    },
    MarketSettlement {
        data: serde_json::Value,
        timestamp: Option<serde_json::Value>,
    },
    Broadcast {
        event_type: String,
        data: serde_json::Value,
        timestamp: Option<serde_json::Value>,
        extra: BTreeMap<String, serde_json::Value>,
    },
}

#[derive(Debug, Deserialize)]
struct WsEnvelope {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    data: serde_json::Value,
    #[serde(default)]
    timestamp: Option<serde_json::Value>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

impl<'de> Deserialize<'de> for WsServerMessage {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let envelope = WsEnvelope::deserialize(deserializer)?;
        let message = match envelope.event_type.as_str() {
            "AUTH_OK" => WsServerMessage::AuthOk {
                data: envelope.data,
                timestamp: envelope.timestamp,
            },
            "PONG" => WsServerMessage::Pong {
                data: envelope.data,
                timestamp: envelope.timestamp,
            },
            "PRICE_UPDATE" => WsServerMessage::PriceUpdate {
                data: serde_json::from_value(envelope.data).map_err(serde::de::Error::custom)?,
                timestamp: envelope.timestamp,
            },
            "WHALE_TRADE" => WsServerMessage::WhaleTrade {
                data: envelope.data,
                timestamp: envelope.timestamp,
            },
            "NEWS_SIGNAL" => WsServerMessage::NewsSignal {
                data: envelope.data,
                timestamp: envelope.timestamp,
            },
            "MARKET_SETTLEMENT" => WsServerMessage::MarketSettlement {
                data: envelope.data,
                timestamp: envelope.timestamp,
            },
            other => WsServerMessage::Broadcast {
                event_type: other.to_string(),
                data: envelope.data,
                timestamp: envelope.timestamp,
                extra: envelope.extra,
            },
        };
        Ok(message)
    }
}

/// Open Polyforge `/ws` gateway connection.
pub struct GatewayWsClient {
    url: Url,
    stream: Option<GatewayStream>,
    reconnect: WsReconnectOptions,
}

impl std::fmt::Debug for GatewayWsClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut redacted = self.url.clone();
        if redacted.query_pairs().any(|(key, _)| key == "token") {
            redacted.set_query(Some("token=[REDACTED]"));
        }
        f.debug_struct("GatewayWsClient")
            .field("url", &redacted.as_str())
            .field("connected", &self.stream.is_some())
            .field("reconnect", &self.reconnect)
            .finish()
    }
}

impl GatewayWsClient {
    /// Connect to the gateway URL.
    pub async fn connect(url: Url, reconnect: WsReconnectOptions) -> Result<Self> {
        let (stream, _) = connect_async(url.as_str()).await?;
        Ok(Self {
            url,
            stream: Some(stream),
            reconnect,
        })
    }

    /// Reconnect using the configured URL and retry policy.
    pub async fn reconnect(&mut self) -> Result<()> {
        self.close().await?;
        let mut delay = self.reconnect.initial_delay;
        let attempts = self.reconnect.max_attempts.max(1);
        let mut last_error = None;

        for attempt in 0..attempts {
            match connect_async(self.url.as_str()).await {
                Ok((stream, _)) => {
                    self.stream = Some(stream);
                    return Ok(());
                }
                Err(err) => {
                    last_error = Some(err);
                    if attempt + 1 < attempts {
                        tokio::time::sleep(delay).await;
                        delay = next_delay(delay, self.reconnect.max_delay);
                    }
                }
            }
        }

        Err(PolyforgeError::WebSocket(
            last_error.expect("at least one reconnect attempt"),
        ))
    }

    /// Send one typed client message.
    pub async fn send(&mut self, message: WsClientMessage) -> Result<()> {
        let payload = serde_json::to_string(&message)?;
        let stream = self.stream_mut()?;
        stream.send(Message::Text(payload.into())).await?;
        Ok(())
    }

    /// Send `PING`.
    pub async fn ping(&mut self) -> Result<()> {
        self.send(WsClientMessage::Ping).await
    }

    /// Subscribe to price updates for one or more token IDs.
    pub async fn subscribe_prices<I, S>(&mut self, token_ids: I) -> Result<()>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.send(WsClientMessage::SubscribePrices {
            token_ids: token_ids.into_iter().map(Into::into).collect(),
        })
        .await
    }

    /// Unsubscribe from price updates for one or more token IDs.
    pub async fn unsubscribe_prices<I, S>(&mut self, token_ids: I) -> Result<()>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.send(WsClientMessage::UnsubscribePrices {
            token_ids: token_ids.into_iter().map(Into::into).collect(),
        })
        .await
    }

    /// Subscribe to whale-trade broadcasts.
    pub async fn subscribe_whales(&mut self, min_size: Option<f64>) -> Result<()> {
        self.send(WsClientMessage::SubscribeWhales { min_size })
            .await
    }

    /// Unsubscribe from whale-trade broadcasts.
    pub async fn unsubscribe_whales(&mut self) -> Result<()> {
        self.send(WsClientMessage::UnsubscribeWhales).await
    }

    /// Receive and parse the next gateway message.
    ///
    /// Returns `Ok(None)` when the server closes the stream cleanly.
    pub async fn next_message(&mut self) -> Result<Option<WsServerMessage>> {
        loop {
            let stream = self.stream_mut()?;
            match stream.next().await {
                Some(Ok(Message::Text(text))) => return Ok(Some(serde_json::from_str(&text)?)),
                Some(Ok(Message::Binary(bytes))) => {
                    return Ok(Some(serde_json::from_slice(&bytes)?));
                }
                Some(Ok(Message::Ping(payload))) => {
                    stream.send(Message::Pong(payload)).await?;
                }
                Some(Ok(Message::Pong(_))) => {}
                Some(Ok(Message::Close(_))) | None => {
                    self.stream = None;
                    return Ok(None);
                }
                Some(Ok(_)) => {}
                Some(Err(err)) => {
                    self.stream = None;
                    return Err(PolyforgeError::WebSocket(err));
                }
            }
        }
    }

    /// Receive the next gateway message, reconnecting once according to the
    /// configured retry policy when the current stream closes or errors.
    pub async fn next_message_with_reconnect(&mut self) -> Result<Option<WsServerMessage>> {
        match self.next_message().await {
            Ok(Some(message)) => Ok(Some(message)),
            Ok(None) | Err(PolyforgeError::WebSocket(_)) => {
                self.reconnect().await?;
                self.next_message().await
            }
            Err(err) => Err(err),
        }
    }

    /// Close the current connection. Calling this on an already-closed client is
    /// a no-op.
    pub async fn close(&mut self) -> Result<()> {
        if let Some(mut stream) = self.stream.take() {
            stream.close(None).await?;
        }
        Ok(())
    }

    fn stream_mut(&mut self) -> Result<&mut GatewayStream> {
        self.stream
            .as_mut()
            .ok_or_else(|| PolyforgeError::Validation("WebSocket is not connected".into()))
    }
}

/// Build the gateway WebSocket URL from an API base URL and connection options.
pub fn build_ws_url(base_url: &str, options: &WsConnectOptions) -> Result<Url> {
    let mut url = Url::parse(base_url)
        .map_err(|e| PolyforgeError::Validation(format!("Malformed base URL: {e}")))?;

    match url.scheme() {
        "https" => url
            .set_scheme("wss")
            .map_err(|_| PolyforgeError::Validation("Unable to set WebSocket scheme".into()))?,
        "http" => url
            .set_scheme("ws")
            .map_err(|_| PolyforgeError::Validation("Unable to set WebSocket scheme".into()))?,
        other => {
            return Err(PolyforgeError::Validation(format!(
                "Unsupported WebSocket base URL scheme \"{other}\""
            )));
        }
    }

    let path = if options.path.starts_with('/') {
        options.path.clone()
    } else {
        format!("/{}", options.path)
    };
    url.set_path(&path);
    url.set_fragment(None);
    url.set_query(None);

    if let Some(token) = options.token.as_deref() {
        if token.is_empty() {
            return Err(PolyforgeError::Validation(
                "WebSocket token must not be empty".into(),
            ));
        }
        url.query_pairs_mut().append_pair("token", token);
    }

    Ok(url)
}

fn next_delay(current: Duration, max: Duration) -> Duration {
    current.saturating_mul(2).min(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_wss_url_with_token_query() {
        let options = WsConnectOptions::with_token("jwt.value");
        let url = build_ws_url("https://api.polyforge.app/api/v1", &options).unwrap();

        assert_eq!(url.as_str(), "wss://api.polyforge.app/ws?token=jwt.value");
    }

    #[test]
    fn builds_ws_url_for_local_http() {
        let options = WsConnectOptions {
            path: "ws".into(),
            ..Default::default()
        };
        let url = build_ws_url("http://localhost:3002", &options).unwrap();

        assert_eq!(url.as_str(), "ws://localhost:3002/ws");
    }

    #[test]
    fn rejects_empty_token() {
        let options = WsConnectOptions {
            token: Some(String::new()),
            ..Default::default()
        };
        assert!(matches!(
            build_ws_url("https://api.polyforge.app", &options),
            Err(PolyforgeError::Validation(_))
        ));
    }

    #[test]
    fn serializes_subscribe_payloads() {
        let prices = serde_json::to_value(WsClientMessage::SubscribePrices {
            token_ids: vec!["token-a".into(), "token-b".into()],
        })
        .unwrap();
        assert_eq!(
            prices,
            serde_json::json!({
                "type": "SUBSCRIBE_PRICES",
                "tokenIds": ["token-a", "token-b"]
            })
        );

        let whales = serde_json::to_value(WsClientMessage::SubscribeWhales {
            min_size: Some(1_000.0),
        })
        .unwrap();
        assert_eq!(
            whales,
            serde_json::json!({
                "type": "SUBSCRIBE_WHALES",
                "minSize": 1000.0
            })
        );
    }

    #[test]
    fn serializes_unsubscribe_payloads() {
        let prices = serde_json::to_value(WsClientMessage::UnsubscribePrices {
            token_ids: vec!["token-a".into()],
        })
        .unwrap();
        assert_eq!(
            prices,
            serde_json::json!({
                "type": "UNSUBSCRIBE_PRICES",
                "tokenIds": ["token-a"]
            })
        );

        let whales = serde_json::to_value(WsClientMessage::UnsubscribeWhales).unwrap();
        assert_eq!(
            whales,
            serde_json::json!({
                "type": "UNSUBSCRIBE_WHALES"
            })
        );
    }

    #[test]
    fn parses_price_update() {
        let message: WsServerMessage = serde_json::from_value(serde_json::json!({
            "type": "PRICE_UPDATE",
            "data": {
                "tokenId": "token-a",
                "price": 0.42,
                "timestamp": "2026-06-05T12:00:00Z"
            },
            "timestamp": 1710000000000_u64
        }))
        .unwrap();

        match message {
            WsServerMessage::PriceUpdate { data, timestamp } => {
                assert_eq!(data.token_id, "token-a");
                assert_eq!(data.price, 0.42);
                assert_eq!(data.timestamp.unwrap(), "2026-06-05T12:00:00Z");
                assert_eq!(timestamp.unwrap(), 1710000000000_u64);
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn parses_broadcast_envelope() {
        let message: WsServerMessage = serde_json::from_value(serde_json::json!({
            "type": "CUSTOM_EVENT",
            "data": { "ok": true },
            "timestamp": "now",
            "traceId": "abc"
        }))
        .unwrap();

        match message {
            WsServerMessage::Broadcast {
                event_type,
                data,
                timestamp,
                extra,
            } => {
                assert_eq!(event_type, "CUSTOM_EVENT");
                assert_eq!(data["ok"], true);
                assert_eq!(timestamp.unwrap(), "now");
                assert_eq!(extra["traceId"], "abc");
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn parses_named_broadcast_envelopes() {
        let news: WsServerMessage = serde_json::from_value(serde_json::json!({
            "type": "NEWS_SIGNAL",
            "data": { "marketId": "m-1" },
            "timestamp": "now"
        }))
        .unwrap();
        assert!(matches!(news, WsServerMessage::NewsSignal { .. }));

        let settlement: WsServerMessage = serde_json::from_value(serde_json::json!({
            "type": "MARKET_SETTLEMENT",
            "data": { "marketId": "m-1", "settled": true },
            "timestamp": "now"
        }))
        .unwrap();
        assert!(matches!(
            settlement,
            WsServerMessage::MarketSettlement { .. }
        ));
    }

    #[test]
    fn reconnect_delay_is_bounded() {
        assert_eq!(
            next_delay(Duration::from_millis(250), Duration::from_secs(5)),
            Duration::from_millis(500)
        );
        assert_eq!(
            next_delay(Duration::from_secs(4), Duration::from_secs(5)),
            Duration::from_secs(5)
        );
    }

    #[tokio::test]
    async fn disconnected_client_returns_validation_error() {
        let mut client = GatewayWsClient {
            url: Url::parse("ws://127.0.0.1:9/ws").unwrap(),
            stream: None,
            reconnect: WsReconnectOptions::default(),
        };

        assert!(matches!(
            client.next_message().await,
            Err(PolyforgeError::Validation(_))
        ));
    }

    #[tokio::test]
    async fn reconnect_returns_last_transport_error_after_attempts() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let mut client = GatewayWsClient {
            url: Url::parse(&format!("ws://{addr}/ws")).unwrap(),
            stream: None,
            reconnect: WsReconnectOptions {
                max_attempts: 1,
                initial_delay: Duration::ZERO,
                max_delay: Duration::ZERO,
            },
        };

        assert!(matches!(
            client.reconnect().await,
            Err(PolyforgeError::WebSocket(_))
        ));
    }
}
