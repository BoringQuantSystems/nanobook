//! Mock broker for testing — implements the `Broker` trait with configurable behavior.
//!
//! Use this in integration tests to simulate broker responses without network calls.
//!
//! ```
//! use nanobook_broker::mock::{MockBroker, FillMode};
//! use nanobook::Symbol;
//!
//! let broker = MockBroker::builder()
//!     .fill_mode(FillMode::ImmediateFull)
//!     .with_position(Symbol::new("AAPL"), 100, 150_00)
//!     .with_account(1_000_000_00, 500_000_00)
//!     .build();
//! ```

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use nanobook::Symbol;

use crate::Broker;
use crate::error::BrokerError;
use crate::types::*;

/// How the mock broker handles submitted orders.
#[derive(Clone, Debug)]
pub enum FillMode {
    /// Orders are immediately fully filled at the limit price (or mid for market).
    ImmediateFull,
    /// Orders are partially filled (the given fraction, e.g., 0.5 = 50%).
    ImmediatePartial(f64),
    /// All orders are rejected.
    Reject,
}

/// A recorded order submission for assertion in tests.
#[derive(Clone, Debug)]
pub struct RecordedOrder {
    pub symbol: Symbol,
    pub side: BrokerSide,
    pub quantity: u64,
    pub order_type: String,
    pub client_order_id: Option<ClientOrderId>,
}

/// Builder for `MockBroker`.
pub struct MockBrokerBuilder {
    fill_mode: FillMode,
    positions: Vec<Position>,
    quotes: Vec<(Symbol, Quote)>,
    equity_cents: i64,
    cash_cents: i64,
}

impl MockBrokerBuilder {
    pub fn fill_mode(mut self, mode: FillMode) -> Self {
        self.fill_mode = mode;
        self
    }

    pub fn with_position(mut self, symbol: Symbol, quantity: i64, avg_cost_cents: i64) -> Self {
        let market_value = quantity * avg_cost_cents;
        self.positions.push(Position {
            symbol,
            quantity,
            avg_cost_cents,
            market_value_cents: market_value,
            unrealized_pnl_cents: 0,
        });
        self
    }

    pub fn with_quote(mut self, symbol: Symbol, bid: i64, ask: i64) -> Self {
        self.quotes.push((
            symbol,
            Quote {
                symbol,
                bid_cents: bid,
                ask_cents: ask,
                last_cents: (bid + ask) / 2,
                volume: 0,
                timestamp: std::time::SystemTime::now(),
            },
        ));
        self
    }

    pub fn with_account(mut self, equity_cents: i64, cash_cents: i64) -> Self {
        self.equity_cents = equity_cents;
        self.cash_cents = cash_cents;
        self
    }

    pub fn build(self) -> MockBroker {
        MockBroker {
            connected: false,
            fill_mode: self.fill_mode,
            positions: self.positions,
            quotes: self.quotes,
            equity_cents: self.equity_cents,
            cash_cents: self.cash_cents,
            next_order_id: AtomicU64::new(1),
            submitted_orders: Mutex::new(Vec::new()),
            open_orders: Mutex::new(Vec::new()),
            open_order_statuses: Mutex::new(None),
        }
    }
}

/// A mock broker that records submitted orders and returns configurable responses.
pub struct MockBroker {
    connected: bool,
    fill_mode: FillMode,
    positions: Vec<Position>,
    quotes: Vec<(Symbol, Quote)>,
    equity_cents: i64,
    cash_cents: i64,
    next_order_id: AtomicU64,
    submitted_orders: Mutex<Vec<RecordedOrder>>,
    open_orders: Mutex<Vec<OrderId>>,
    /// When set, `open_orders` returns these rows instead of `fill_mode`.
    open_order_statuses: Mutex<Option<Vec<BrokerOrderStatus>>>,
}

impl MockBroker {
    pub fn builder() -> MockBrokerBuilder {
        MockBrokerBuilder {
            fill_mode: FillMode::ImmediateFull,
            positions: Vec::new(),
            quotes: Vec::new(),
            equity_cents: 100_000_000,
            cash_cents: 100_000_000,
        }
    }

    /// Get all orders that were submitted (for assertion in tests).
    pub fn submitted_orders(&self) -> Vec<RecordedOrder> {
        self.submitted_orders
            .lock()
            .expect("submitted_orders mutex poisoned")
            .clone()
    }

    /// Make `open_orders` return `orders`.
    ///
    /// `FillMode` builds every row from 100 shares. Call this when the
    /// caller needs an explicit filled quantity and remaining quantity.
    pub fn set_open_order_statuses(&self, orders: Vec<BrokerOrderStatus>) {
        *self
            .open_order_statuses
            .lock()
            .expect("open_order_statuses mutex poisoned") = Some(orders);
    }

    fn fill_status(&self) -> (OrderState, u64, u64) {
        match &self.fill_mode {
            FillMode::ImmediateFull => (OrderState::Filled, 100, 0),
            FillMode::ImmediatePartial(frac) => {
                let filled = (100.0 * frac) as u64;
                (OrderState::PartiallyFilled, filled, 100 - filled)
            }
            FillMode::Reject => (OrderState::Rejected, 0, 0),
        }
    }
}

impl Broker for MockBroker {
    fn connect(&mut self) -> Result<(), BrokerError> {
        self.connected = true;
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), BrokerError> {
        self.connected = false;
        Ok(())
    }

    fn positions(&self) -> Result<Vec<Position>, BrokerError> {
        if !self.connected {
            return Err(BrokerError::NotConnected);
        }
        Ok(self.positions.clone())
    }

    fn account(&self) -> Result<Account, BrokerError> {
        if !self.connected {
            return Err(BrokerError::NotConnected);
        }
        let gross = self
            .positions
            .iter()
            .map(|p| p.market_value_cents.abs())
            .sum();
        Ok(Account {
            equity_cents: self.equity_cents,
            buying_power_cents: self.cash_cents,
            cash_cents: self.cash_cents,
            gross_position_value_cents: gross,
        })
    }

    fn submit_order(&self, order: &BrokerOrder) -> Result<OrderId, BrokerError> {
        if !self.connected {
            return Err(BrokerError::NotConnected);
        }

        // Record the order
        self.submitted_orders
            .lock()
            .expect("submitted_orders mutex poisoned")
            .push(RecordedOrder {
                symbol: order.symbol,
                side: order.side,
                quantity: order.quantity,
                order_type: format!("{:?}", order.order_type),
                client_order_id: order.client_order_id.clone(),
            });

        match &self.fill_mode {
            FillMode::Reject => Err(BrokerError::Order("mock: order rejected".into())),
            _ => {
                let id = OrderId(self.next_order_id.fetch_add(1, Ordering::Relaxed));
                // Add to open orders list
                self.open_orders
                    .lock()
                    .expect("open_orders mutex poisoned")
                    .push(id);
                Ok(id)
            }
        }
    }

    fn order_status(&self, id: OrderId) -> Result<BrokerOrderStatus, BrokerError> {
        if !self.connected {
            return Err(BrokerError::NotConnected);
        }

        let (status, filled, remaining) = self.fill_status();

        Ok(BrokerOrderStatus {
            id,
            status,
            filled_quantity: filled,
            remaining_quantity: remaining,
            avg_fill_price_cents: 0,
        })
    }

    fn open_orders(&self) -> Result<Vec<BrokerOrderStatus>, BrokerError> {
        if !self.connected {
            return Err(BrokerError::NotConnected);
        }

        if let Some(orders) = self
            .open_order_statuses
            .lock()
            .expect("open_order_statuses mutex poisoned")
            .clone()
        {
            return Ok(orders);
        }

        let open_ids = self.open_orders.lock().expect("open_orders mutex poisoned");
        let (status, filled, remaining) = self.fill_status();

        Ok(open_ids
            .iter()
            .copied()
            .map(|id| BrokerOrderStatus {
                id,
                status,
                filled_quantity: filled,
                remaining_quantity: remaining,
                avg_fill_price_cents: 0,
            })
            .collect())
    }

    fn cancel_order(&self, id: OrderId) -> Result<(), BrokerError> {
        if !self.connected {
            return Err(BrokerError::NotConnected);
        }
        // Remove from open orders list
        self.open_orders
            .lock()
            .expect("open_orders mutex poisoned")
            .retain(|&oid| oid != id);
        // Rows set by `set_open_order_statuses` are open orders too.
        if let Some(rows) = self
            .open_order_statuses
            .lock()
            .expect("open_order_statuses mutex poisoned")
            .as_mut()
        {
            rows.retain(|row| row.id != id);
        }
        Ok(())
    }

    fn quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError> {
        if !self.connected {
            return Err(BrokerError::NotConnected);
        }
        self.quotes
            .iter()
            .find(|(s, _)| s == symbol)
            .map(|(_, q)| q.clone())
            .ok_or_else(|| BrokerError::InvalidSymbol(symbol.as_str().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanobook::Price;

    fn aapl() -> Symbol {
        Symbol::new("AAPL")
    }

    #[test]
    fn builder_basic() {
        let mut broker = MockBroker::builder()
            .with_position(aapl(), 100, 15_000)
            .with_account(100_000_000, 50_000_000)
            .with_quote(aapl(), 14_950, 15_050)
            .build();

        broker.connect().unwrap();

        let positions = broker.positions().unwrap();
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].symbol, aapl());
        assert_eq!(positions[0].quantity, 100);

        let account = broker.account().unwrap();
        assert_eq!(account.equity_cents, 100_000_000);

        let quote = broker.quote(&aapl()).unwrap();
        assert_eq!(quote.bid_cents, 14_950);
        assert_eq!(quote.ask_cents, 15_050);
    }

    #[test]
    fn not_connected_errors() {
        let broker = MockBroker::builder().build();
        assert!(broker.positions().is_err());
        assert!(broker.account().is_err());
    }

    #[test]
    fn submit_records_orders() {
        let mut broker = MockBroker::builder().build();
        broker.connect().unwrap();

        let order = BrokerOrder {
            symbol: aapl(),
            side: BrokerSide::Buy,
            quantity: 50,
            order_type: BrokerOrderType::Limit(Price(15_000)),
            client_order_id: None,
        };

        let id = broker.submit_order(&order).unwrap();
        assert_eq!(id, OrderId(1));

        let recorded = broker.submitted_orders();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].symbol, aapl());
        assert_eq!(recorded[0].quantity, 50);
    }

    #[test]
    fn reject_mode() {
        let mut broker = MockBroker::builder().fill_mode(FillMode::Reject).build();
        broker.connect().unwrap();

        let order = BrokerOrder {
            symbol: aapl(),
            side: BrokerSide::Buy,
            quantity: 50,
            order_type: BrokerOrderType::Market,
            client_order_id: None,
        };

        assert!(broker.submit_order(&order).is_err());
    }

    #[test]
    fn partial_fill_status() {
        let mut broker = MockBroker::builder()
            .fill_mode(FillMode::ImmediatePartial(0.5))
            .build();
        broker.connect().unwrap();

        let status = broker.order_status(OrderId(1)).unwrap();
        assert_eq!(status.status, OrderState::PartiallyFilled);
        assert_eq!(status.filled_quantity, 50);
    }

    #[test]
    fn order_ids_are_monotonic() {
        let mut broker = MockBroker::builder().build();
        broker.connect().unwrap();

        let order = BrokerOrder {
            symbol: aapl(),
            side: BrokerSide::Buy,
            quantity: 1,
            order_type: BrokerOrderType::Market,
            client_order_id: None,
        };

        let first = broker.submit_order(&order).unwrap();
        let second = broker.submit_order(&order).unwrap();
        let third = broker.submit_order(&order).unwrap();

        assert_eq!(first, OrderId(1));
        assert_eq!(second, OrderId(2));
        assert_eq!(third, OrderId(3));
    }

    /// A 100-share order: the mock builds every fill status from 100 shares.
    fn market_order() -> BrokerOrder {
        BrokerOrder {
            symbol: aapl(),
            side: BrokerSide::Buy,
            quantity: 100,
            order_type: BrokerOrderType::Market,
            client_order_id: None,
        }
    }

    #[test]
    fn with_quote_last_is_the_mid_of_bid_and_ask() {
        let mut broker = MockBroker::builder()
            .with_quote(aapl(), 14_950, 15_050)
            .build();
        broker.connect().unwrap();

        let quote = broker.quote(&aapl()).unwrap();

        assert_eq!(quote.bid_cents, 14_950);
        assert_eq!(quote.ask_cents, 15_050);
        assert_eq!(quote.last_cents, 15_000);
    }

    #[test]
    fn partial_fill_remainder_is_the_unfilled_part_of_100() {
        let mut broker = MockBroker::builder()
            .fill_mode(FillMode::ImmediatePartial(0.5))
            .build();
        broker.connect().unwrap();
        let id = broker.submit_order(&market_order()).unwrap();

        let status = broker.order_status(id).unwrap();
        assert_eq!(status.filled_quantity, 50);
        assert_eq!(status.remaining_quantity, 50);

        let open = broker.open_orders().unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, id);
        assert_eq!(open[0].status, OrderState::PartiallyFilled);
        assert_eq!(open[0].filled_quantity, 50);
        assert_eq!(open[0].remaining_quantity, 50);
    }

    #[test]
    fn open_orders_lists_every_submitted_order() {
        let mut broker = MockBroker::builder().build();
        assert!(matches!(
            broker.open_orders(),
            Err(BrokerError::NotConnected)
        ));

        broker.connect().unwrap();
        assert!(broker.open_orders().unwrap().is_empty());
        broker.submit_order(&market_order()).unwrap();
        broker.submit_order(&market_order()).unwrap();

        let open = broker.open_orders().unwrap();
        let ids: Vec<OrderId> = open.iter().map(|o| o.id).collect();
        assert_eq!(ids, vec![OrderId(1), OrderId(2)]);
        for order in &open {
            assert_eq!(order.status, OrderState::Filled);
            assert_eq!(order.filled_quantity, 100);
            assert_eq!(order.remaining_quantity, 0);
        }
    }

    #[test]
    fn cancel_order_removes_only_that_order() {
        let mut broker = MockBroker::builder().build();
        assert!(matches!(
            broker.cancel_order(OrderId(1)),
            Err(BrokerError::NotConnected)
        ));

        broker.connect().unwrap();
        broker.submit_order(&market_order()).unwrap();
        broker.submit_order(&market_order()).unwrap();

        broker.cancel_order(OrderId(1)).unwrap();

        let ids: Vec<OrderId> = broker.open_orders().unwrap().iter().map(|o| o.id).collect();
        assert_eq!(ids, vec![OrderId(2)]);
    }

    #[test]
    fn cancel_order_removes_a_configured_row() {
        let mut broker = MockBroker::builder().build();
        broker.connect().unwrap();
        let row = |id: u64| BrokerOrderStatus {
            id: OrderId(id),
            status: OrderState::Submitted,
            filled_quantity: 0,
            remaining_quantity: 50,
            avg_fill_price_cents: 0,
        };
        broker.set_open_order_statuses(vec![row(7), row(8)]);

        broker.cancel_order(OrderId(7)).unwrap();

        let ids: Vec<OrderId> = broker.open_orders().unwrap().iter().map(|o| o.id).collect();
        assert_eq!(ids, vec![OrderId(8)]);
    }

    #[test]
    fn set_open_order_statuses_replaces_the_rows_open_orders_returns() {
        let mut broker = MockBroker::builder().build();
        broker.connect().unwrap();
        broker.submit_order(&market_order()).unwrap();

        // 25 of 80 shares filled; FillMode alone would report 100/0 rows.
        broker.set_open_order_statuses(vec![BrokerOrderStatus {
            id: OrderId(7),
            status: OrderState::PartiallyFilled,
            filled_quantity: 25,
            remaining_quantity: 55,
            avg_fill_price_cents: 12_345,
        }]);

        let open = broker.open_orders().unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, OrderId(7));
        assert_eq!(open[0].status, OrderState::PartiallyFilled);
        assert_eq!(open[0].filled_quantity, 25);
        assert_eq!(open[0].remaining_quantity, 55);
        assert_eq!(open[0].avg_fill_price_cents, 12_345);
    }
}
