//! IBKR connection, position fetching, market data, and account summary.
//!
//! # Disconnect and Reconnect Handling (F3 Failure Mode)
//!
//! This module implements reconnect logic for handling connection losses during order execution.
//!
//! When a disconnect occurs:
//! 1. The `IbkrClient::reconnect` method re-establishes the connection to TWS/Gateway
//! 2. On reconnect, the callback deduplication cache is cleared to avoid stale entries
//! 3. Current positions are queried via `positions()` API to get ground truth
//! 4. The positions are returned to the caller for reconciliation
//!
//! The reconnect process ensures that:
//! - Any partial fills that occurred during disconnect are detected via position comparison
//! - The callback cache is cleared to prevent false duplicate detection after reconnect
//! - Ground truth from positions API is used for reconciliation (not local state)
//!
//! See `orders::reconcile_partial_fill` for the reconciliation logic that uses the positions
//! returned by `reconnect`.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use ibapi::accounts::types::AccountGroup;
use ibapi::accounts::{AccountSummaryResult, PositionUpdate};
use ibapi::client::blocking::Client;
use ibapi::contracts::Contract;
use ibapi::market_data::realtime::{TickType, TickTypes};
use ibapi::orders::Orders;
use log::{debug, info, warn};
use nanobook::Symbol;

use crate::error::BrokerError;
use crate::parse::parse_f64_or_warn;
use crate::types::{
    Account, BestQuote, BrokerOrder, BrokerOrderStatus, BrokerSide, OrderId, Position, Quote,
    f64_cents_checked,
};

use super::market_data::best_quote_from_quote;
use super::orders::{
    self, broker_order_status_from_ibkr_parts, broker_order_status_from_order_data,
};

/// Locally tracked order submitted by this IBKR client instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedOrder {
    pub order_id: i32,
    pub symbol: Symbol,
    pub quantity: i64,
    pub side: String,
    pub status: String,
    pub submitted_at: Instant,
}

/// Deduplication key for order-status callbacks.
///
/// TWS may send duplicate OrderStatus callbacks for the same fill event.
/// We deduplicate based on the combination of order ID, status string,
/// and filled quantity to ensure position changes only once per unique fill.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrderCallbackKey {
    pub order_id: i32,
    pub status: String,
    pub filled_quantity: i64, // Converted from f64 for hashing
}

/// Wraps the ibapi blocking client with convenience methods.
///
/// Unlike `BinanceBroker`, `IbkrClient` holds no credentials —
/// IBKR authentication happens at the TWS/Gateway socket layer
/// via `(host, port, client_id)` rather than via API-key strings
/// in process memory. There is nothing this struct owns that
/// benefits from `ZeroizeOnDrop`, so the derive is deliberately
/// absent. If a future refactor adds a secret-bearing field,
/// re-evaluate: ibapi's `Client` does not implement `Zeroize` and
/// would need `#[zeroize(skip)]`.
pub struct IbkrClient {
    client: Client,
    best_quotes: Mutex<HashMap<Symbol, BestQuote>>,
    order_cache: Mutex<HashMap<i32, CachedOrder>>,
    /// Deduplication cache for order-status callbacks to handle TWS duplicates.
    /// Key: (order_id, status, filled_quantity), Value: timestamp when first seen.
    /// TTL: 5 minutes to prevent unbounded growth.
    callback_cache: Mutex<HashMap<OrderCallbackKey, Instant>>,
}

impl IbkrClient {
    /// Connect to IB Gateway/TWS.
    pub fn connect(host: &str, port: u16, client_id: i32) -> Result<Self, BrokerError> {
        let address = format!("{host}:{port}");
        info!("Connecting to IB Gateway at {address}...");

        let client = Client::connect(&address, client_id)
            .map_err(|e| BrokerError::Connection(format!("failed to connect to {address}: {e}")))?;

        info!("Connected (client_id={client_id})");
        Ok(Self {
            client,
            best_quotes: Mutex::new(HashMap::new()),
            order_cache: Mutex::new(HashMap::new()),
            callback_cache: Mutex::new(HashMap::new()),
        })
    }

    /// Reconnect to IB Gateway/TWS after a disconnect.
    ///
    /// This method re-establishes the connection and queries current positions
    /// to detect any partial fills that may have occurred during the disconnect.
    ///
    /// The reconnection process:
    /// 1. Establishes a new connection to TWS/Gateway
    /// 2. Queries current positions via positions() API
    /// 3. Returns the current positions for reconciliation
    ///
    /// # Arguments
    /// * `host` - TWS/Gateway host address
    /// * `port` - TWS/Gateway port
    /// * `client_id` - Client ID for the new connection
    ///
    /// # Returns
    /// * `Ok(Vec<Position>)` - Current positions after reconnect (for reconciliation)
    /// * `Err(BrokerError)` - If reconnection or position query fails
    pub fn reconnect(
        &mut self,
        host: &str,
        port: u16,
        client_id: i32,
    ) -> Result<Vec<Position>, BrokerError> {
        let address = format!("{host}:{port}");
        info!("Reconnecting to IB Gateway at {address}...");

        // Establish new connection
        let client = Client::connect(&address, client_id).map_err(|e| {
            BrokerError::Connection(format!("failed to reconnect to {address}: {e}"))
        })?;

        info!("Reconnected (client_id={client_id})");
        self.client = client;

        // Clear local caches on reconnect to avoid stale callback/order state.
        {
            let mut cache = self
                .callback_cache
                .lock()
                .map_err(|_| BrokerError::Other("callback cache poisoned".into()))?;
            cache.clear();
        }
        self.clear_order_cache()?;

        // Query current positions to detect partial fills during disconnect
        info!("Querying positions after reconnect for reconciliation...");
        let positions = self.positions()?;

        info!(
            "Reconnect complete: {} positions fetched for reconciliation",
            positions.len()
        );

        Ok(positions)
    }

    /// Get the underlying ibapi client (for order submission).
    pub fn inner(&self) -> &Client {
        &self.client
    }

    /// Submit an order using the current best-quote cache when available.
    pub fn submit_order(&self, order: &BrokerOrder) -> Result<OrderId, BrokerError> {
        let best_quote = self.cached_best_quote(&order.symbol)?;
        let order_id = orders::submit_order(&self.client, order, best_quote.as_ref())?;
        let side = match order.side {
            BrokerSide::Buy => "Buy",
            BrokerSide::Sell => "Sell",
        };
        self.cache_order(
            order_id.0 as i32,
            order.symbol,
            order.quantity as i64,
            side.to_string(),
        )?;
        Ok(order_id)
    }

    pub fn cache_order(
        &self,
        order_id: i32,
        symbol: Symbol,
        quantity: i64,
        side: String,
    ) -> Result<(), BrokerError> {
        let mut cache = self
            .order_cache
            .lock()
            .map_err(|_| BrokerError::Other("order cache poisoned".into()))?;
        cache.insert(
            order_id,
            CachedOrder {
                order_id,
                symbol,
                quantity,
                side,
                status: "Submitted".to_string(),
                submitted_at: Instant::now(),
            },
        );
        Ok(())
    }

    pub fn update_cached_order_status(
        &self,
        order_id: i32,
        status: String,
    ) -> Result<(), BrokerError> {
        let mut cache = self
            .order_cache
            .lock()
            .map_err(|_| BrokerError::Other("order cache poisoned".into()))?;
        if let Some(order) = cache.get_mut(&order_id) {
            order.status = status;
        }
        Ok(())
    }

    pub fn get_cached_order(&self, order_id: i32) -> Result<Option<CachedOrder>, BrokerError> {
        let cache = self
            .order_cache
            .lock()
            .map_err(|_| BrokerError::Other("order cache poisoned".into()))?;
        Ok(cache.get(&order_id).cloned())
    }

    pub fn cached_orders(&self) -> Result<Vec<CachedOrder>, BrokerError> {
        let cache = self
            .order_cache
            .lock()
            .map_err(|_| BrokerError::Other("order cache poisoned".into()))?;
        Ok(cache.values().cloned().collect())
    }

    pub fn clear_order_cache(&self) -> Result<(), BrokerError> {
        let mut cache = self
            .order_cache
            .lock()
            .map_err(|_| BrokerError::Other("order cache poisoned".into()))?;
        cache.clear();
        Ok(())
    }

    fn cached_best_quote(&self, symbol: &Symbol) -> Result<Option<BestQuote>, BrokerError> {
        let quotes = self
            .best_quotes
            .lock()
            .map_err(|_| BrokerError::Other("best quote cache poisoned".into()))?;
        Ok(quotes.get(symbol).copied())
    }

    fn cache_best_quote(&self, symbol: Symbol, quote: BestQuote) -> Result<(), BrokerError> {
        let mut quotes = self
            .best_quotes
            .lock()
            .map_err(|_| BrokerError::Other("best quote cache poisoned".into()))?;
        quotes.insert(symbol, quote);
        Ok(())
    }

    /// Check if an order-status callback is a duplicate.
    ///
    /// Returns true if this exact (order_id, status, filled_quantity) combination
    /// has been seen within the TTL window (5 minutes).
    pub fn is_duplicate_callback(&self, order_id: i32, status: &str, filled_quantity: f64) -> bool {
        let key = OrderCallbackKey {
            order_id,
            status: status.to_string(),
            filled_quantity: filled_quantity.round() as i64,
        };

        let mut cache = self
            .callback_cache
            .lock()
            .map_err(|_| BrokerError::Other("callback cache poisoned".into()));

        if let Ok(ref mut cache) = cache {
            // Clean up expired entries (TTL: 5 minutes)
            let ttl = Duration::from_secs(300);
            cache.retain(|_, timestamp| timestamp.elapsed() < ttl);

            // Check if this is a duplicate
            if cache.contains_key(&key) {
                return true;
            }
        }

        false
    }

    /// Record that an order-status callback has been processed.
    pub fn record_processed_callback(
        &self,
        order_id: i32,
        status: &str,
        filled_quantity: f64,
    ) -> Result<(), BrokerError> {
        let key = OrderCallbackKey {
            order_id,
            status: status.to_string(),
            filled_quantity: filled_quantity.round() as i64,
        };

        let mut cache = self
            .callback_cache
            .lock()
            .map_err(|_| BrokerError::Other("callback cache poisoned".into()))?;

        // Clean up expired entries before inserting (TTL: 5 minutes)
        let ttl = Duration::from_secs(300);
        cache.retain(|_, timestamp| timestamp.elapsed() < ttl);

        cache.insert(key, Instant::now());
        Ok(())
    }

    /// Get a reference to the deduplication cache for order-status callbacks.
    ///
    /// This is used by `execute_limit_order` to detect and skip duplicate
    /// callbacks from TWS.
    pub fn dedup_cache(&self) -> &Mutex<HashMap<OrderCallbackKey, Instant>> {
        &self.callback_cache
    }

    /// Fetch current positions from IBKR.
    pub fn positions(&self) -> Result<Vec<Position>, BrokerError> {
        let subscription = self
            .client
            .positions()
            .map_err(|e| BrokerError::Connection(format!("failed to request positions: {e}")))?;

        positions_from_updates(subscription)
    }

    /// Fetch all open orders known to TWS/Gateway.
    pub fn open_orders(&self) -> Result<Vec<BrokerOrderStatus>, BrokerError> {
        let subscription = self
            .client
            .all_open_orders()
            .map_err(|e| BrokerError::Connection(format!("failed to request open orders: {e}")))?;

        let mut statuses = HashMap::new();
        for update in subscription {
            match update {
                Orders::OrderData(order_data) => {
                    let status = broker_order_status_from_order_data(&order_data)?;
                    statuses.insert(status.id.0, status);
                }
                Orders::OrderStatus(order_status) => {
                    let status = broker_order_status_from_ibkr_parts(
                        order_status.order_id,
                        &order_status.status,
                        order_status.filled,
                        order_status.remaining,
                        order_status.average_fill_price,
                    )?;
                    self.update_cached_order_status(
                        order_status.order_id,
                        order_status.status.clone(),
                    )?;
                    statuses.insert(status.id.0, status);
                }
                Orders::Notice(notice) => {
                    debug!("IBKR open-orders notice: {notice:?}");
                }
            }
        }

        Ok(statuses.into_values().collect())
    }

    /// Fetch account summary (equity, cash, buying power).
    pub fn account_summary(&self) -> Result<Account, BrokerError> {
        let group = AccountGroup("All".to_string());
        let tags = &["NetLiquidation", "TotalCashValue", "BuyingPower"];

        let subscription = self.client.account_summary(&group, tags).map_err(|e| {
            BrokerError::Connection(format!("failed to request account summary: {e}"))
        })?;

        account_from_summaries(subscription)
    }

    /// Fetch a live quote for a symbol.
    pub fn quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError> {
        let contract = Contract::stock(symbol.as_str()).build();
        let subscription = self
            .client
            .market_data(&contract)
            .snapshot()
            .subscribe()
            .map_err(|e| BrokerError::Connection(format!("market data request failed: {e}")))?;

        let quote = quote_from_ticks(symbol, subscription)?;

        if let Some(best_quote) = best_quote_from_quote(&quote) {
            self.cache_best_quote(*symbol, best_quote)?;
        }

        Ok(quote)
    }

    /// Fetch bid/ask midpoint price for a symbol, in cents.
    pub fn mid_price(&self, symbol: &Symbol) -> Result<i64, BrokerError> {
        let q = self.quote(symbol)?;
        mid_cents(&q)
    }

    /// Fetch live prices (bid/ask midpoint) for a set of symbols.
    pub fn prices(&self, symbols: &[Symbol]) -> Result<Vec<(Symbol, i64)>, BrokerError> {
        let mut prices = Vec::with_capacity(symbols.len());
        for &sym in symbols {
            let mid = self.mid_price(&sym)?;
            debug!("{}: ${:.2}", sym, mid as f64 / 100.0);
            prices.push((sym, mid));
        }
        Ok(prices)
    }
}

/// Fold a position stream into broker positions (symbols over 8 bytes are skipped).
fn positions_from_updates(
    updates: impl IntoIterator<Item = PositionUpdate>,
) -> Result<Vec<Position>, BrokerError> {
    let mut positions = Vec::new();
    for update in updates {
        match update {
            PositionUpdate::Position(pos) => {
                let symbol_str = pos.contract.symbol.to_string();
                if let Some(sym) = Symbol::try_new(&symbol_str) {
                    let qty = pos.position as i64;
                    let avg_cost_cents =
                        f64_cents_checked(pos.average_cost, "ibkr position.average_cost")?;
                    debug!(
                        "Position: {} qty={} avg_cost={:.2}",
                        sym, qty, pos.average_cost
                    );
                    positions.push(Position {
                        symbol: sym,
                        quantity: qty,
                        avg_cost_cents,
                        market_value_cents: qty.abs() * avg_cost_cents, // approximate
                        unrealized_pnl_cents: 0, // would need live prices for exact value
                    });
                } else {
                    warn!("Skipping symbol '{symbol_str}' (> 8 bytes)");
                }
            }
            PositionUpdate::PositionEnd => break,
        }
    }

    // Demoted to debug in S7: the position count is a coarse
    // signal of account activity and shouldn't appear in
    // aggregated info-level logs.
    debug!("Fetched {} positions", positions.len());
    Ok(positions)
}

/// Build the account from an account-summary stream.
fn account_from_summaries(
    summaries: impl IntoIterator<Item = AccountSummaryResult>,
) -> Result<Account, BrokerError> {
    let mut equity = 0.0_f64;
    let mut cash = 0.0_f64;
    let mut buying_power = 0.0_f64;

    for result in summaries {
        match result {
            AccountSummaryResult::Summary(s) => {
                debug!("Account: {}={} {}", s.tag, s.value, s.currency);
                // Each known tag routes to its own destination AND its
                // own `field` label for the parse-failure warning, so
                // a malformed NetLiquidation vs. BuyingPower surfaces
                // distinguishably in logs.
                match s.tag.as_str() {
                    "NetLiquidation" => {
                        equity = parse_f64_or_warn(&s.value, "ibkr account.NetLiquidation");
                    }
                    "TotalCashValue" => {
                        cash = parse_f64_or_warn(&s.value, "ibkr account.TotalCashValue");
                    }
                    "BuyingPower" => {
                        buying_power = parse_f64_or_warn(&s.value, "ibkr account.BuyingPower");
                    }
                    _ => {}
                }
            }
            AccountSummaryResult::End => break,
        }
    }

    let account = Account {
        equity_cents: f64_cents_checked(equity, "ibkr NetLiquidation")?,
        cash_cents: f64_cents_checked(cash, "ibkr TotalCashValue")?,
        buying_power_cents: f64_cents_checked(buying_power, "ibkr BuyingPower")?,
        gross_position_value_cents: f64_cents_checked(equity - cash, "ibkr gross_position_value")?,
    };

    // Demoted to debug in S7: equity/cash/buying-power are
    // financial PII and must not appear in aggregated info-level
    // logs. Set `RUST_LOG=debug` locally if you need to see them
    // while debugging.
    debug!(
        "Account: equity=${:.2}, cash=${:.2}, buying_power=${:.2}",
        equity, cash, buying_power
    );

    Ok(account)
}

/// Build a quote from a snapshot tick stream.
fn quote_from_ticks(
    symbol: &Symbol,
    ticks: impl IntoIterator<Item = TickTypes>,
) -> Result<Quote, BrokerError> {
    let mut bid = None;
    let mut ask = None;
    let mut last = None;

    for tick in ticks {
        match tick {
            TickTypes::Price(price_tick) => match price_tick.tick_type {
                TickType::Bid => bid = Some(price_tick.price),
                TickType::Ask => ask = Some(price_tick.price),
                TickType::Last => last = Some(price_tick.price),
                _ => {}
            },
            TickTypes::PriceSize(ps) => match ps.price_tick_type {
                TickType::Bid => bid = Some(ps.price),
                TickType::Ask => ask = Some(ps.price),
                TickType::Last => last = Some(ps.price),
                _ => {}
            },
            TickTypes::SnapshotEnd => break,
            _ => {}
        }
    }

    let bid_cents = bid
        .map(|b| f64_cents_checked(b, "ibkr tick.bid"))
        .transpose()?
        .unwrap_or(0);
    let ask_cents = ask
        .map(|a| f64_cents_checked(a, "ibkr tick.ask"))
        .transpose()?
        .unwrap_or(0);
    let last_cents = last
        .map(|l| f64_cents_checked(l, "ibkr tick.last"))
        .transpose()?
        .unwrap_or(0);

    // Require at least one valid price
    if bid_cents <= 0 && ask_cents <= 0 && last_cents <= 0 {
        return Err(BrokerError::Connection("no valid price received".into()));
    }

    Ok(Quote {
        symbol: *symbol,
        bid_cents,
        ask_cents,
        last_cents,
        volume: 0, // snapshot doesn't provide volume
        timestamp: std::time::SystemTime::now(),
    })
}

/// Bid/ask midpoint in cents, falling back to one side, then to the last price.
fn mid_cents(q: &Quote) -> Result<i64, BrokerError> {
    let mid = match (q.bid_cents, q.ask_cents) {
        (b, a) if b > 0 && a > 0 => b + (a - b) / 2,
        (b, _) if b > 0 => b,
        (_, a) if a > 0 => a,
        _ => q.last_cents,
    };
    if mid <= 0 {
        return Err(BrokerError::Connection("no valid price received".into()));
    }
    Ok(mid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ibapi::accounts::{AccountSummary, Position as IbPosition};
    use ibapi::market_data::realtime::{TickPrice, TickPriceSize};
    use std::time::SystemTime;

    fn summary(tag: &str, value: &str) -> AccountSummaryResult {
        AccountSummaryResult::Summary(AccountSummary {
            tag: tag.to_string(),
            value: value.to_string(),
            currency: "USD".to_string(),
            ..Default::default()
        })
    }

    #[test]
    fn account_routes_each_tag_and_derives_gross_value() {
        let account = account_from_summaries(vec![
            summary("NetLiquidation", "100000.00"),
            summary("TotalCashValue", "40000.00"),
            summary("BuyingPower", "250000.00"),
            summary("AvailableFunds", "1.00"),
            AccountSummaryResult::End,
            summary("NetLiquidation", "1.00"),
        ])
        .unwrap();

        assert_eq!(account.equity_cents, 10_000_000);
        assert_eq!(account.cash_cents, 4_000_000);
        assert_eq!(account.buying_power_cents, 25_000_000);
        assert_eq!(account.gross_position_value_cents, 6_000_000);
    }

    fn aapl() -> Symbol {
        Symbol::new("AAPL")
    }

    fn price(tick_type: TickType, price: f64) -> TickTypes {
        TickTypes::Price(TickPrice {
            tick_type,
            price,
            ..Default::default()
        })
    }

    fn price_size(tick_type: TickType, price: f64) -> TickTypes {
        TickTypes::PriceSize(TickPriceSize {
            price_tick_type: tick_type,
            price,
            ..Default::default()
        })
    }

    fn cents(quote: &Quote) -> (i64, i64, i64) {
        (quote.bid_cents, quote.ask_cents, quote.last_cents)
    }

    #[test]
    fn quote_reads_bid_ask_last_from_price_ticks() {
        let quote = quote_from_ticks(
            &aapl(),
            vec![
                price(TickType::Bid, 184.95),
                price(TickType::Ask, 185.05),
                price(TickType::Last, 185.00),
                TickTypes::SnapshotEnd,
            ],
        )
        .unwrap();

        assert_eq!(quote.symbol, aapl());
        assert_eq!(cents(&quote), (18_495, 18_505, 18_500));
        assert_eq!(quote.volume, 0);
        assert!(quote.timestamp <= SystemTime::now());
    }

    #[test]
    fn quote_reads_bid_ask_last_from_price_size_ticks() {
        let quote = quote_from_ticks(
            &aapl(),
            vec![
                price_size(TickType::Bid, 184.95),
                price_size(TickType::Ask, 185.05),
                price_size(TickType::Last, 185.00),
                TickTypes::SnapshotEnd,
            ],
        )
        .unwrap();

        assert_eq!(cents(&quote), (18_495, 18_505, 18_500));
    }

    #[test]
    fn quote_ignores_ticks_after_the_snapshot_end() {
        let result = quote_from_ticks(
            &aapl(),
            vec![TickTypes::SnapshotEnd, price(TickType::Bid, 99.0)],
        );

        assert!(matches!(
            result,
            Err(BrokerError::Connection(m)) if m == "no valid price received"
        ));
    }

    #[test]
    fn quote_accepts_a_single_valid_side() {
        let bid = quote_from_ticks(&aapl(), vec![price(TickType::Bid, 184.95)]).unwrap();
        assert_eq!(cents(&bid), (18_495, 0, 0));

        let ask = quote_from_ticks(&aapl(), vec![price(TickType::Ask, 185.05)]).unwrap();
        assert_eq!(cents(&ask), (0, 18_505, 0));

        let last = quote_from_ticks(&aapl(), vec![price(TickType::Last, 185.0)]).unwrap();
        assert_eq!(cents(&last), (0, 0, 18_500));
    }

    #[test]
    fn quote_without_a_positive_price_is_an_error() {
        for ticks in [
            vec![TickTypes::SnapshotEnd],
            vec![price(TickType::Bid, 0.0)],
            vec![price(TickType::Bid, 0.0), price(TickType::Ask, 0.0)],
        ] {
            let result = quote_from_ticks(&aapl(), ticks);

            assert!(matches!(
                result,
                Err(BrokerError::Connection(m)) if m == "no valid price received"
            ));
        }
    }

    #[test]
    fn positions_convert_cost_and_value_and_skip_long_symbols() {
        let held = |symbol: &str, position: f64, average_cost: f64| {
            PositionUpdate::Position(IbPosition {
                contract: Contract::stock(symbol).build(),
                position,
                average_cost,
                ..Default::default()
            })
        };

        let positions = positions_from_updates(vec![
            held("AAPL", -10.0, 185.0),
            held("TOOLONGSYM", 5.0, 1.0),
            PositionUpdate::PositionEnd,
            held("MSFT", 1.0, 1.0),
        ])
        .unwrap();

        assert_eq!(positions.len(), 1);
        let p = &positions[0];
        assert_eq!(p.symbol, aapl());
        assert_eq!(p.quantity, -10);
        assert_eq!(p.avg_cost_cents, 18_500);
        assert_eq!(p.market_value_cents, 185_000);
        assert_eq!(p.unrealized_pnl_cents, 0);
    }

    fn quote_of(bid_cents: i64, ask_cents: i64, last_cents: i64) -> Quote {
        Quote {
            symbol: aapl(),
            bid_cents,
            ask_cents,
            last_cents,
            volume: 0,
            timestamp: SystemTime::now(),
        }
    }

    #[test]
    fn mid_is_the_midpoint_then_one_side_then_last() {
        let mid = |b, a, l| mid_cents(&quote_of(b, a, l)).unwrap();

        assert_eq!(mid(18_495, 18_505, 18_400), 18_500);
        assert_eq!(mid(100, 103, 50), 101);
        assert_eq!(mid(18_495, 0, 18_400), 18_495);
        assert_eq!(mid(0, 18_505, 18_400), 18_505);
        assert_eq!(mid(0, 0, 18_400), 18_400);
    }

    #[test]
    fn mid_without_a_positive_price_is_an_error() {
        for (b, a, l) in [(0, 0, 0), (0, 0, -5)] {
            assert!(matches!(
                mid_cents(&quote_of(b, a, l)),
                Err(BrokerError::Connection(m)) if m == "no valid price received"
            ));
        }
    }
}
