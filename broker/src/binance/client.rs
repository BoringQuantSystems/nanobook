//! Binance REST API client.

use std::time::{SystemTime, UNIX_EPOCH};

use log::debug;
use reqwest::blocking::Client;

use super::auth;
use super::types::{AccountInfo, BookTicker, OrderInfo, OrderResponse};
use crate::error::BrokerError;

/// Validate that a parameter value is safe for URL query strings.
///
/// Rejects any value containing characters that could inject additional
/// query parameters (e.g., `&`, `=`, `?`, `#`, space).
fn validate_query_param(value: &str, name: &str) -> Result<(), BrokerError> {
    if value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'-')
    {
        Ok(())
    } else {
        Err(BrokerError::Order(format!(
            "invalid characters in {name}: {value:?}"
        )))
    }
}

/// Validate multiple query parameters at once.
fn validate_query_params(params: &[(&str, &str)]) -> Result<(), BrokerError> {
    for &(value, name) in params {
        validate_query_param(value, name)?;
    }
    Ok(())
}

/// Check an HTTP response status and return a formatted error on failure.
fn check_response(
    resp: reqwest::blocking::Response,
    context: &str,
    error_kind: fn(String) -> BrokerError,
) -> Result<reqwest::blocking::Response, BrokerError> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    Err(error_kind(format!("{context} returned {status}: {body}")))
}

/// Blocking Binance REST client.
///
/// `api_key` and `secret_key` are scrubbed in memory when this
/// struct drops via the [`ZeroizeOnDrop`](zeroize::ZeroizeOnDrop)
/// derive. `client` (reqwest handle) and `base_url` are marked
/// `#[zeroize(skip)]` — the reqwest handle has no `Zeroize` impl
/// and holds no secrets; the URL is public.
#[derive(zeroize::ZeroizeOnDrop)]
pub struct BinanceClient {
    #[zeroize(skip)]
    client: Client,
    api_key: String,
    secret_key: String,
    #[zeroize(skip)]
    base_url: String,
}

impl BinanceClient {
    /// Create a new Binance client.
    pub fn new(api_key: &str, secret_key: &str, testnet: bool) -> Self {
        let base_url = if testnet {
            "https://testnet.binance.vision"
        } else {
            "https://api.binance.com"
        };

        Self {
            client: Client::new(),
            api_key: api_key.to_string(),
            secret_key: secret_key.to_string(),
            base_url: base_url.to_string(),
        }
    }

    /// Test-only constructor: points the client at `base_url` (a loopback
    /// test server) and bypasses any proxy set in the environment.
    #[cfg(test)]
    pub(super) fn with_base_url(api_key: &str, secret_key: &str, base_url: &str) -> Self {
        Self {
            client: Client::builder()
                .no_proxy()
                .build()
                .expect("build test HTTP client"),
            api_key: api_key.to_string(),
            secret_key: secret_key.to_string(),
            base_url: base_url.to_string(),
        }
    }

    /// Test connectivity (GET /api/v3/ping).
    pub fn ping(&self) -> Result<(), BrokerError> {
        let url = format!("{}/api/v3/ping", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .map_err(|e| BrokerError::Connection(format!("ping failed: {e}")))?;

        check_response(resp, "ping", BrokerError::Connection)?;
        Ok(())
    }

    /// Get account information (GET /api/v3/account).
    pub fn account_info(&self) -> Result<AccountInfo, BrokerError> {
        let timestamp = current_timestamp_ms()?;
        let query = format!("timestamp={timestamp}");
        let signature = auth::sign(&query, &self.secret_key)?;
        let url = format!(
            "{}/api/v3/account?{query}&signature={signature}",
            self.base_url
        );

        let resp = self
            .client
            .get(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .map_err(|e| BrokerError::Connection(format!("account request failed: {e}")))?;

        let resp = check_response(resp, "account", BrokerError::Connection)?;
        resp.json::<AccountInfo>()
            .map_err(|e| BrokerError::Connection(format!("failed to parse account: {e}")))
    }

    /// Get every open order of the account (GET /api/v3/openOrders).
    ///
    /// `GET /api/v3/account` does not list open orders; this endpoint does.
    pub fn open_orders(&self) -> Result<Vec<OrderInfo>, BrokerError> {
        let timestamp = current_timestamp_ms()?;
        let query = format!("timestamp={timestamp}");
        let signature = auth::sign(&query, &self.secret_key)?;
        let url = format!(
            "{}/api/v3/openOrders?{query}&signature={signature}",
            self.base_url
        );

        let resp = self
            .client
            .get(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .map_err(|e| BrokerError::Connection(format!("open orders request failed: {e}")))?;

        let resp = check_response(resp, "open orders", BrokerError::Connection)?;
        resp.json::<Vec<OrderInfo>>()
            .map_err(|e| BrokerError::Connection(format!("failed to parse open orders: {e}")))
    }

    /// Submit a new order (POST /api/v3/order).
    #[allow(clippy::too_many_arguments)]
    pub fn submit_order(
        &self,
        symbol: &str,
        side: &str,
        order_type: &str,
        quantity: &str,
        price: Option<&str>,
        time_in_force: Option<&str>,
        client_order_id: Option<&str>,
    ) -> Result<OrderResponse, BrokerError> {
        validate_query_params(&[
            (symbol, "symbol"),
            (side, "side"),
            (order_type, "order_type"),
            (quantity, "quantity"),
        ])?;
        if let Some(p) = price {
            validate_query_param(p, "price")?;
        }
        if let Some(tif) = time_in_force {
            validate_query_param(tif, "timeInForce")?;
        }
        if let Some(cid) = client_order_id {
            validate_query_param(cid, "newClientOrderId")?;
        }

        let timestamp = current_timestamp_ms()?;
        let mut query = format!(
            "symbol={symbol}&side={side}&type={order_type}&quantity={quantity}&timestamp={}",
            timestamp
        );
        if let Some(p) = price {
            query.push_str(&format!("&price={p}"));
        }
        if let Some(tif) = time_in_force {
            query.push_str(&format!("&timeInForce={tif}"));
        }
        if let Some(cid) = client_order_id {
            query.push_str(&format!("&newClientOrderId={cid}"));
        }

        let signature = auth::sign(&query, &self.secret_key)?;
        let url = format!("{}/api/v3/order", self.base_url);

        debug!("Submitting Binance order: {symbol} {side} qty={quantity}");

        let resp = self
            .client
            .post(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .body(format!("{query}&signature={signature}"))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .send()
            .map_err(|e| BrokerError::Order(format!("order request failed: {e}")))?;

        let resp = check_response(resp, "order", BrokerError::Order)?;
        resp.json::<OrderResponse>()
            .map_err(|e| BrokerError::Order(format!("failed to parse order response: {e}")))
    }

    /// Get order status (GET /api/v3/order).
    pub fn order_status(&self, symbol: &str, order_id: u64) -> Result<OrderResponse, BrokerError> {
        validate_query_param(symbol, "symbol")?;
        let timestamp = current_timestamp_ms()?;
        let query = format!("symbol={symbol}&orderId={order_id}&timestamp={}", timestamp);
        let signature = auth::sign(&query, &self.secret_key)?;
        let url = format!(
            "{}/api/v3/order?{query}&signature={signature}",
            self.base_url
        );

        let resp = self
            .client
            .get(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .map_err(|e| BrokerError::Order(format!("order status request failed: {e}")))?;

        let resp = check_response(resp, "order status", BrokerError::Order)?;
        resp.json::<OrderResponse>()
            .map_err(|e| BrokerError::Order(format!("failed to parse order status: {e}")))
    }

    /// Cancel an order (DELETE /api/v3/order).
    pub fn cancel_order(&self, symbol: &str, order_id: u64) -> Result<(), BrokerError> {
        validate_query_param(symbol, "symbol")?;
        let timestamp = current_timestamp_ms()?;
        let query = format!("symbol={symbol}&orderId={order_id}&timestamp={}", timestamp);
        let signature = auth::sign(&query, &self.secret_key)?;
        let url = format!(
            "{}/api/v3/order?{query}&signature={signature}",
            self.base_url
        );

        let resp = self
            .client
            .delete(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .map_err(|e| BrokerError::Order(format!("cancel request failed: {e}")))?;

        check_response(resp, "cancel", BrokerError::Order)?;
        Ok(())
    }

    /// Get book ticker (best bid/ask) for a symbol (GET /api/v3/ticker/bookTicker).
    pub fn book_ticker(&self, symbol: &str) -> Result<BookTicker, BrokerError> {
        validate_query_param(symbol, "symbol")?;
        let url = format!("{}/api/v3/ticker/bookTicker?symbol={symbol}", self.base_url);

        let resp = self
            .client
            .get(&url)
            .send()
            .map_err(|e| BrokerError::Connection(format!("ticker request failed: {e}")))?;

        let resp = check_response(resp, "ticker", BrokerError::Connection)?;
        resp.json::<BookTicker>()
            .map_err(|e| BrokerError::Connection(format!("failed to parse ticker: {e}")))
    }
}

/// Current timestamp in milliseconds.
///
/// Returns an error if the system clock is unavailable or has gone backwards.
/// For a trading system, clock failures should be surfaced rather than silently
/// using epoch zero, as timestamps are critical for request authentication
/// and order sequencing.
fn current_timestamp_ms() -> Result<u64, BrokerError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .map_err(|e| BrokerError::Connection(format!("system clock error: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `http://127.0.0.1:<port>` of a loopback port that was just released:
    /// connecting is refused at once (a fixed port could be filtered and hang).
    fn refused_base_url() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        url
    }

    fn invalid_chars_error(result: Result<(), BrokerError>, name: &str) {
        match result {
            Err(BrokerError::Order(message)) => {
                assert!(
                    message.contains(&format!("invalid characters in {name}")),
                    "got {message}"
                );
            }
            other => panic!("expected Order error, got {other:?}"),
        }
    }

    #[test]
    fn validate_query_param_accepts_safe_values() {
        for value in ["BTCUSDT", "cid-1_a.b", "a_b", "a.b", "a-b", "0.001", ""] {
            assert!(
                validate_query_param(value, "symbol").is_ok(),
                "{value:?} must be accepted"
            );
        }
    }

    #[test]
    fn validate_query_param_rejects_injection_characters() {
        for value in ["BTC&side=SELL", "a b", "a=b", "a?b", "a#b", "a/b", "a%b"] {
            invalid_chars_error(validate_query_param(value, "symbol"), "symbol");
        }
    }

    #[test]
    fn validate_query_params_names_the_failing_parameter() {
        assert!(validate_query_params(&[("ok", "a"), ("fine-1", "b")]).is_ok());

        invalid_chars_error(validate_query_params(&[("ok", "a"), ("bad&", "b")]), "b");
    }

    #[test]
    fn current_timestamp_is_the_wall_clock_in_milliseconds() {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let stamp = current_timestamp_ms().unwrap();

        assert!(
            stamp.abs_diff(now_ms) < 5_000,
            "stamp {stamp} vs clock {now_ms}"
        );
    }

    #[test]
    fn cancel_order_validates_the_symbol_before_any_request() {
        // The server is never reached: validation fails first.
        let client = BinanceClient::with_base_url("key", "secret", &refused_base_url());

        invalid_chars_error(client.cancel_order("A&B", 1), "symbol");
    }

    #[test]
    fn ping_reports_a_connection_error_when_the_server_is_unreachable() {
        // Nothing listens on the port: the TCP connect is refused.
        let client = BinanceClient::with_base_url("key", "secret", &refused_base_url());

        assert!(matches!(client.ping(), Err(BrokerError::Connection(_))));
    }
}
