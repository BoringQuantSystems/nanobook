//! Binance WebSocket user-data stream client.

use std::error::Error;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio::time::{Instant, sleep};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

/// Current Binance WebSocket connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connected,
    Disconnected,
    Reconnecting,
}

/// Parsed Binance user-data WebSocket event.
#[derive(Debug, Clone, PartialEq)]
pub enum BinanceWebSocketEvent {
    AccountUpdate(AccountUpdate),
    ExecutionReport(ExecutionReport),
    Other(Value),
}

/// Minimal account update event for Phase 1.
#[derive(Debug, Clone, PartialEq)]
pub struct AccountUpdate {
    pub event_time: Option<u64>,
    pub raw: Value,
}

/// Minimal execution report event for Phase 1.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionReport {
    pub event_time: Option<u64>,
    pub symbol: Option<String>,
    pub order_id: Option<u64>,
    pub client_order_id: Option<String>,
    pub order_status: Option<String>,
    pub raw: Value,
}

/// Binance WebSocket client.
pub struct BinanceWebSocket {
    api_key: String,
    secret_key: String,
    testnet: bool,
    connected: Arc<AtomicBool>,
    state: ConnectionState,
    client: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    events: Vec<BinanceWebSocketEvent>,
    // Heartbeat mechanism
    last_heartbeat: Arc<Mutex<Option<Instant>>>,
    heartbeat_interval: Duration,
    // Auto-reconnect mechanism
    max_reconnect_attempts: u32,
    reconnect_attempts: u32,
    /// Test-only: connect here instead of the Binance stream.
    #[cfg(test)]
    endpoint_override: Option<String>,
}

impl BinanceWebSocket {
    /// Create a disconnected Binance WebSocket client.
    pub fn new(api_key: &str, secret_key: &str, testnet: bool) -> Self {
        Self {
            api_key: api_key.to_string(),
            secret_key: secret_key.to_string(),
            testnet,
            connected: Arc::new(AtomicBool::new(false)),
            state: ConnectionState::Disconnected,
            client: None,
            events: Vec::new(),
            last_heartbeat: Arc::new(Mutex::new(None)),
            heartbeat_interval: Duration::from_secs(10),
            max_reconnect_attempts: 5,
            reconnect_attempts: 0,
            #[cfg(test)]
            endpoint_override: None,
        }
    }

    /// Returns true when the underlying socket is currently connected.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Return the tracked connection state.
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Return parsed events buffered by the most recent read/subscribe call.
    pub fn events(&self) -> &[BinanceWebSocketEvent] {
        &self.events
    }

    /// Connect to the Binance WebSocket endpoint.
    pub async fn connect(&mut self) -> Result<(), Box<dyn Error>> {
        self.state = ConnectionState::Reconnecting;
        let endpoint = self.endpoint().to_string();
        let (stream, _) = connect_async(endpoint).await.inspect_err(|_| {
            self.connected.store(false, Ordering::SeqCst);
            self.state = ConnectionState::Disconnected;
        })?;
        // Reset only on success: reconnect_with_backoff counts failed connects.
        self.reconnect_attempts = 0;

        self.client = Some(stream);
        self.connected.store(true, Ordering::SeqCst);
        self.state = ConnectionState::Connected;
        self.update_heartbeat();
        Ok(())
    }

    /// Subscribe to user-data updates and parse one available text message.
    ///
    /// Binance user-data streams normally require a REST-created listen key
    /// and a `/ws/<listenKey>` URL. Phase 1 keeps authentication deliberately
    /// minimal and sends a simple subscription frame so tests and future mock
    /// servers can exercise the connection path without live credentials.
    pub async fn subscribe_user_data(&mut self) -> Result<(), Box<dyn Error>> {
        // Check heartbeat before processing
        if !self.check_heartbeat() {
            self.state = ConnectionState::Disconnected;
            return Err("Heartbeat timeout detected".into());
        }

        let client = self
            .client
            .as_mut()
            .ok_or("Binance WebSocket is not connected")?;

        let subscription = serde_json::json!({
            "method": "SUBSCRIBE",
            "params": ["!userData"],
            "id": 1,
        });
        // tokio-tungstenite 0.30 carries text as `Utf8Bytes`, not `String`.
        client
            .send(Message::Text(subscription.to_string().into()))
            .await?;

        if let Some(message) = client.next().await
            && let Some(event) = Self::parse_frame(message?)?
        {
            self.events.push(event);
            // Update heartbeat on successful message processing
            self.update_heartbeat();
        }

        Ok(())
    }

    /// Close the WebSocket connection and reset connection state.
    pub async fn disconnect(&mut self) {
        if let Some(mut client) = self.client.take() {
            let _ = client.close(None).await;
        }
        self.connected.store(false, Ordering::SeqCst);
        self.state = ConnectionState::Disconnected;
        *self
            .last_heartbeat
            .lock()
            .expect("heartbeat mutex poisoned") = None;
        self.reconnect_attempts = 0;
    }

    /// Send a ping message to the server.
    pub async fn send_ping(&mut self) -> Result<(), Box<dyn Error>> {
        let client = self
            .client
            .as_mut()
            .ok_or("Binance WebSocket is not connected")?;

        client.send(Message::Ping(Vec::new().into())).await?;
        self.update_heartbeat();
        Ok(())
    }

    /// Attempt to reconnect with exponential backoff.
    ///
    /// Uses exponential backoff: 1s, 2s, 4s, 8s, 16s (max).
    /// Returns error if all attempts fail.
    pub async fn reconnect_with_backoff(&mut self) -> Result<(), Box<dyn Error>> {
        self.reconnect_attempts = 0;

        while self.reconnect_attempts < self.max_reconnect_attempts {
            self.reconnect_attempts += 1;
            self.state = ConnectionState::Reconnecting;

            // Calculate backoff delay: 2^(attempt-1) seconds, capped at 16s
            let backoff_secs = 2u64.pow(self.reconnect_attempts.saturating_sub(1)).min(16);
            let backoff = Duration::from_secs(backoff_secs);

            // Wait for backoff
            sleep(backoff).await;

            // Attempt to reconnect
            if let Ok(()) = self.connect().await {
                return Ok(());
            }
        }

        // All attempts failed
        self.state = ConnectionState::Disconnected;
        Err(format!(
            "Failed to reconnect after {} attempts",
            self.max_reconnect_attempts
        )
        .into())
    }

    /// Check if heartbeat has timed out.
    ///
    /// Returns true if the heartbeat is still valid (no timeout),
    /// false if a timeout has occurred.
    pub fn check_heartbeat(&self) -> bool {
        let last_heartbeat = self
            .last_heartbeat
            .lock()
            .expect("heartbeat mutex poisoned");
        if let Some(last) = *last_heartbeat {
            last.elapsed() < self.heartbeat_interval
        } else {
            false
        }
    }

    /// Update the last heartbeat timestamp to now.
    pub fn update_heartbeat(&self) {
        *self
            .last_heartbeat
            .lock()
            .expect("heartbeat mutex poisoned") = Some(Instant::now());
    }

    /// Get the current number of reconnect attempts.
    pub fn reconnect_attempts(&self) -> u32 {
        self.reconnect_attempts
    }

    /// Get the maximum number of reconnect attempts.
    pub fn max_reconnect_attempts(&self) -> u32 {
        self.max_reconnect_attempts
    }

    /// Set the maximum number of reconnect attempts (for testing).
    pub fn set_max_reconnect_attempts(&mut self, max: u32) {
        self.max_reconnect_attempts = max;
    }

    /// Set the heartbeat interval (for testing).
    pub fn set_heartbeat_interval(&mut self, interval: Duration) {
        self.heartbeat_interval = interval;
    }

    fn endpoint(&self) -> &str {
        let _ = (&self.api_key, &self.secret_key);
        #[cfg(test)]
        if let Some(endpoint) = &self.endpoint_override {
            return endpoint;
        }
        if self.testnet {
            "wss://stream.testnet.binance.vision/ws"
        } else {
            "wss://stream.binance.com:9443/ws"
        }
    }

    fn parse_frame(message: Message) -> Result<Option<BinanceWebSocketEvent>, Box<dyn Error>> {
        match message {
            Message::Text(text) => Self::parse_text_message(&text).map(Some),
            Message::Binary(bytes) => {
                // `bytes` is a `Bytes` as of tokio-tungstenite 0.30; borrow it
                // rather than copying into an owned String on every frame.
                let text = std::str::from_utf8(&bytes)?;
                Self::parse_text_message(text).map(Some)
            }
            Message::Ping(_) | Message::Pong(_) => Ok(None),
            Message::Close(_) => Ok(None),
            Message::Frame(_) => Ok(None),
        }
    }

    fn parse_text_message(text: &str) -> Result<BinanceWebSocketEvent, Box<dyn Error>> {
        let value: Value = serde_json::from_str(text)?;
        let event_type = value.get("e").and_then(Value::as_str);
        let event_time = value.get("E").and_then(Value::as_u64);

        match event_type {
            Some("outboundAccountPosition") | Some("balanceUpdate") => {
                Ok(BinanceWebSocketEvent::AccountUpdate(AccountUpdate {
                    event_time,
                    raw: value,
                }))
            }
            Some("executionReport") => {
                Ok(BinanceWebSocketEvent::ExecutionReport(ExecutionReport {
                    event_time,
                    symbol: value
                        .get("s")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    order_id: value.get("i").and_then(Value::as_u64),
                    client_order_id: value
                        .get("c")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    order_status: value
                        .get("X")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    raw: value,
                }))
            }
            _ => Ok(BinanceWebSocketEvent::Other(value)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXECUTION_REPORT: &str =
        r#"{"e":"executionReport","E":5,"s":"BTCUSDT","i":42,"c":"cid-1","X":"FILLED"}"#;

    fn execution_report_event() -> BinanceWebSocketEvent {
        BinanceWebSocketEvent::ExecutionReport(ExecutionReport {
            event_time: Some(5),
            symbol: Some("BTCUSDT".to_string()),
            order_id: Some(42),
            client_order_id: Some("cid-1".to_string()),
            order_status: Some("FILLED".to_string()),
            raw: serde_json::from_str(EXECUTION_REPORT).unwrap(),
        })
    }

    #[test]
    fn execution_report_fields_are_extracted() {
        let event = BinanceWebSocket::parse_text_message(EXECUTION_REPORT).unwrap();

        assert_eq!(event, execution_report_event());
    }

    #[test]
    fn account_events_become_account_updates() {
        for kind in ["outboundAccountPosition", "balanceUpdate"] {
            let text = format!(r#"{{"e":"{kind}","E":7}}"#);

            let event = BinanceWebSocket::parse_text_message(&text).unwrap();

            assert_eq!(
                event,
                BinanceWebSocketEvent::AccountUpdate(AccountUpdate {
                    event_time: Some(7),
                    raw: serde_json::from_str(&text).unwrap(),
                }),
                "{kind}"
            );
        }
    }

    #[test]
    fn unknown_events_stay_untyped() {
        let event = BinanceWebSocket::parse_text_message(r#"{"e":"other"}"#).unwrap();

        assert_eq!(
            event,
            BinanceWebSocketEvent::Other(serde_json::json!({"e": "other"}))
        );
    }

    #[test]
    fn frames_carry_events_only_for_text_and_binary() {
        let text = BinanceWebSocket::parse_frame(Message::Text(EXECUTION_REPORT.into())).unwrap();
        assert_eq!(text, Some(execution_report_event()));

        let binary = BinanceWebSocket::parse_frame(Message::Binary(
            EXECUTION_REPORT.as_bytes().to_vec().into(),
        ))
        .unwrap();
        assert_eq!(binary, Some(execution_report_event()));

        for message in [
            Message::Ping(Vec::new().into()),
            Message::Pong(Vec::new().into()),
            Message::Close(None),
        ] {
            assert_eq!(BinanceWebSocket::parse_frame(message).unwrap(), None);
        }
    }

    #[test]
    fn endpoint_is_the_production_stream() {
        let ws = BinanceWebSocket::new("key", "secret", false);

        assert_eq!(ws.endpoint(), "wss://stream.binance.com:9443/ws");
    }

    #[test]
    fn endpoint_follows_testnet() {
        // Testnet keys do not work on the production stream, and a testnet
        // session must never see production events.
        let ws = BinanceWebSocket::new("key", "secret", true);

        assert_eq!(ws.endpoint(), "wss://stream.testnet.binance.vision/ws");
    }

    #[test]
    fn is_connected_follows_the_connected_flag() {
        let ws = BinanceWebSocket::new("key", "secret", true);
        assert!(!ws.is_connected());

        ws.connected.store(true, Ordering::SeqCst);
        assert!(ws.is_connected());
    }

    #[test]
    fn events_returns_the_buffered_events() {
        let mut ws = BinanceWebSocket::new("key", "secret", true);
        assert!(ws.events().is_empty());

        ws.events.push(execution_report_event());
        assert_eq!(ws.events(), &[execution_report_event()]);
    }

    #[test]
    fn reconnect_attempts_reports_the_counter() {
        let mut ws = BinanceWebSocket::new("key", "secret", true);
        ws.reconnect_attempts = 3;

        assert_eq!(ws.reconnect_attempts(), 3);
    }

    #[tokio::test]
    async fn reconnect_gives_up_after_the_attempt_limit() {
        // A port that was free a moment ago refuses the connection at once.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let mut ws = BinanceWebSocket::new("key", "secret", true);
        ws.endpoint_override = Some(format!("ws://127.0.0.1:{port}/ws"));
        ws.set_max_reconnect_attempts(2);

        // Real time (a paused clock jumps ahead while a socket connects):
        // backoff 1 s + 2 s. Before the fix, each failed connect reset the
        // counter and the loop never ended.
        let result = tokio::time::timeout(Duration::from_secs(30), ws.reconnect_with_backoff())
            .await
            .expect("reconnect loop must stop after its attempt limit");

        assert_eq!(
            result.unwrap_err().to_string(),
            "Failed to reconnect after 2 attempts"
        );
        assert_eq!(ws.reconnect_attempts(), 2);
        assert_eq!(ws.state(), ConnectionState::Disconnected);
    }

    #[tokio::test(start_paused = true)]
    async fn heartbeat_times_out_when_the_interval_has_elapsed_exactly() {
        let mut ws = BinanceWebSocket::new("key", "secret", true);
        ws.set_heartbeat_interval(Duration::from_secs(10));
        ws.update_heartbeat();

        tokio::time::advance(Duration::from_millis(9_999)).await;
        assert!(ws.check_heartbeat());

        // Elapsed time equal to the interval is a timeout.
        tokio::time::advance(Duration::from_millis(1)).await;
        assert!(!ws.check_heartbeat());
    }

    #[tokio::test]
    async fn subscribe_checks_the_heartbeat_before_the_connection() {
        let mut ws = BinanceWebSocket::new("key", "secret", true);

        // No heartbeat yet: reported as a timeout.
        let err = ws.subscribe_user_data().await.unwrap_err();
        assert_eq!(err.to_string(), "Heartbeat timeout detected");
        assert_eq!(ws.state(), ConnectionState::Disconnected);

        // Fresh heartbeat but no socket.
        ws.update_heartbeat();
        let err = ws.subscribe_user_data().await.unwrap_err();
        assert_eq!(err.to_string(), "Binance WebSocket is not connected");
    }

    #[tokio::test]
    async fn reconnect_with_zero_attempts_fails_at_once_without_dialling() {
        let mut ws = BinanceWebSocket::new("key", "secret", true);
        ws.set_max_reconnect_attempts(0);

        // One attempt would sleep 1 s first; the call must return before that.
        let result = tokio::time::timeout(Duration::from_millis(500), ws.reconnect_with_backoff())
            .await
            .expect("zero attempts must not sleep or dial");

        assert_eq!(
            result.unwrap_err().to_string(),
            "Failed to reconnect after 0 attempts"
        );
        assert_eq!(ws.reconnect_attempts(), 0);
        assert_eq!(ws.state(), ConnectionState::Disconnected);
    }
}
