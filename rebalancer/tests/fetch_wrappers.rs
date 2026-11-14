//! G16 WAL fetch wrappers and T2 non-WAL wrappers.

use std::cell::Cell;
use std::time::{Duration, SystemTime};

use nanobook::Symbol;
use nanobook_broker::error::BrokerError;
use nanobook_broker::ibkr::orders::{OrderOutcome, OrderResult};
use nanobook_broker::types::{Account, Position, Quote};
use nanobook_broker::{BrokerSide, ClientOrderId};
use nanobook_rebalancer::audit::AuditLog;
use nanobook_rebalancer::broker::BrokerGateway;
use nanobook_rebalancer::diff::{Action, RebalanceOrder};
use nanobook_rebalancer::error::Error;
use nanobook_rebalancer::execution::{
    cancel_order_with_write_ahead, execute_order_with_write_ahead,
    fetch_positions_with_write_ahead, fetch_quotes_with_write_ahead,
};

struct FetchStub {
    fail_connection: bool,
    cancel_ok: bool,
    cancel_calls: Cell<usize>,
    execute_calls: Cell<usize>,
    execute_ok: bool,
}

impl FetchStub {
    fn ok() -> Self {
        Self {
            fail_connection: false,
            cancel_ok: true,
            cancel_calls: Cell::new(0),
            execute_calls: Cell::new(0),
            execute_ok: true,
        }
    }

    fn connection_fail() -> Self {
        Self {
            fail_connection: true,
            cancel_ok: true,
            cancel_calls: Cell::new(0),
            execute_calls: Cell::new(0),
            execute_ok: true,
        }
    }
}

fn sample_positions() -> Vec<Position> {
    vec![
        Position {
            symbol: Symbol::new("AAPL"),
            quantity: 100,
            avg_cost_cents: 15_000,
            market_value_cents: 0,
            unrealized_pnl_cents: 0,
        },
        Position {
            symbol: Symbol::new("MSFT"),
            quantity: -30,
            avg_cost_cents: 40_000,
            market_value_cents: 0,
            unrealized_pnl_cents: 0,
        },
    ]
}

fn sample_quotes() -> Vec<Quote> {
    vec![
        Quote {
            symbol: Symbol::new("AAPL"),
            bid_cents: 14_990,
            ask_cents: 15_010,
            last_cents: 15_000,
            volume: 0,
            timestamp: SystemTime::now(),
        },
        Quote {
            symbol: Symbol::new("MSFT"),
            bid_cents: 39_990,
            ask_cents: 40_010,
            last_cents: 40_000,
            volume: 0,
            timestamp: SystemTime::now(),
        },
    ]
}

impl BrokerGateway for FetchStub {
    fn account_summary(&self) -> nanobook_rebalancer::broker::BrokerResult<Account> {
        if self.fail_connection {
            return Err(BrokerError::Connection("down".into()));
        }
        Ok(Account {
            equity_cents: 100_000_000,
            buying_power_cents: 0,
            cash_cents: 100_000_000,
            gross_position_value_cents: 0,
        })
    }

    fn positions(&self) -> nanobook_rebalancer::broker::BrokerResult<Vec<Position>> {
        if self.fail_connection {
            return Err(BrokerError::Connection("down".into()));
        }
        Ok(sample_positions())
    }

    fn prices(
        &self,
        _symbols: &[Symbol],
    ) -> nanobook_rebalancer::broker::BrokerResult<Vec<(Symbol, i64)>> {
        Ok(Vec::new())
    }

    fn quotes(&self, _symbols: &[Symbol]) -> nanobook_rebalancer::broker::BrokerResult<Vec<Quote>> {
        if self.fail_connection {
            return Err(BrokerError::Connection("down".into()));
        }
        Ok(sample_quotes())
    }

    fn execute_limit_order(
        &self,
        symbol: Symbol,
        _side: BrokerSide,
        shares: u64,
        _limit_price_cents: i64,
        _client_order_id: Option<&ClientOrderId>,
        _timeout: Duration,
    ) -> nanobook_rebalancer::broker::BrokerResult<OrderResult> {
        self.execute_calls.set(self.execute_calls.get() + 1);
        if !self.execute_ok {
            return Err(BrokerError::Order("boom".into()));
        }
        Ok(OrderResult {
            symbol,
            order_id: 1,
            filled_shares: shares as i64,
            avg_fill_price: 1.0,
            commission: 0.0,
            status: OrderOutcome::Filled,
        })
    }

    fn cancel_order(&self, _order_id: u64) -> nanobook_rebalancer::broker::BrokerResult<()> {
        self.cancel_calls.set(self.cancel_calls.get() + 1);
        if self.cancel_ok {
            Ok(())
        } else {
            Err(BrokerError::Other("boom".into()))
        }
    }
}

#[test]
fn fetch_wrappers_return_stub_positions_and_quotes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.jsonl");
    let mut audit = AuditLog::open_in(&path, dir.path()).unwrap();
    let stub = FetchStub::ok();
    let mut seq = 100u64;
    let positions =
        fetch_positions_with_write_ahead(&stub, &mut audit, &mut seq, "ref", 100_000_000).unwrap();
    assert_eq!(positions.len(), 2);
    assert_eq!(positions[1].quantity, -30);

    #[cfg(feature = "write_ahead_logging")]
    {
        assert_eq!(seq, 102);
        let events = nanobook_rebalancer::audit::parse_audit_events(&path).unwrap();
        assert_eq!(events[0].event, "positions_intent");
        assert_eq!(events[0].sequence_number, Some(101));
        assert_eq!(events[1].event, "positions_result");
        assert_eq!(events[1].sequence_number, Some(102));
        assert_eq!(
            events[1].data["positions"][1]
                .get("qty")
                .and_then(|v| v.as_i64()),
            Some(-30)
        );
        assert_eq!(
            events[1].data.get("equity").and_then(|v| v.as_f64()),
            Some(1_000_000.0)
        );
    }

    let symbols = [Symbol::new("AAPL"), Symbol::new("MSFT")];
    let quotes =
        fetch_quotes_with_write_ahead(&stub, &mut audit, &mut seq, "ref", &symbols, 30).unwrap();
    assert_eq!(quotes.len(), 2);
    assert_eq!(quotes[1].ask_cents, 40_010);

    #[cfg(feature = "write_ahead_logging")]
    {
        assert_eq!(seq, 104);
        let events = nanobook_rebalancer::audit::parse_audit_events(&path).unwrap();
        let intent = events.iter().find(|e| e.event == "quotes_intent").unwrap();
        assert_eq!(
            intent
                .data
                .get("symbols")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                }),
            Some(vec!["AAPL".into(), "MSFT".into()])
        );
        assert_eq!(
            intent
                .data
                .get("staleness_threshold_sec")
                .and_then(|v| v.as_u64()),
            Some(30)
        );
        assert!(events.iter().any(|e| e.event == "quotes_result"));
    }
}

#[test]
fn fetch_wrappers_map_connection_errors() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.jsonl");
    let mut audit = AuditLog::open_in(&path, dir.path()).unwrap();
    let stub = FetchStub::connection_fail();
    let mut seq = 1u64;
    match fetch_positions_with_write_ahead(&stub, &mut audit, &mut seq, "ref", 0) {
        Err(Error::Connection(_)) => {}
        other => panic!("expected Connection, got {other:?}"),
    }
}

#[test]
fn cancel_wrapper_maps_broker_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.jsonl");
    let mut audit = AuditLog::open_in(&path, dir.path()).unwrap();
    let mut fail = FetchStub::ok();
    fail.cancel_ok = false;
    let mut seq = 1u64;
    match cancel_order_with_write_ahead(&fail, &mut audit, &mut seq, 42, "r") {
        Err(Error::Order(m)) => assert!(m.starts_with("cancel failed for order 42")),
        other => panic!("expected Order, got {other:?}"),
    }

    let ok = FetchStub::ok();
    cancel_order_with_write_ahead(&ok, &mut audit, &mut seq, 42, "r").unwrap();
    assert_eq!(ok.cancel_calls.get(), 1);
}

#[test]
fn execute_wrapper_maps_order_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.jsonl");
    let mut audit = AuditLog::open_in(&path, dir.path()).unwrap();
    let mut stub = FetchStub::ok();
    stub.execute_ok = false;
    let order = RebalanceOrder {
        symbol: Symbol::new("AAPL"),
        action: Action::Buy,
        shares: 10,
        limit_price_cents: 18_500,
        notional_cents: 185_000,
        description: "open",
    };
    let cid = ClientOrderId::derive("scope", "AAPL", BrokerSide::Buy, 10);
    match execute_order_with_write_ahead(
        &stub,
        &mut audit,
        &order,
        &cid,
        Duration::from_secs(1),
        1,
        "ref",
    ) {
        Err(Error::Order(_)) => {}
        other => panic!("expected Order, got {other:?}"),
    }
    assert_eq!(stub.execute_calls.get(), 1);
}
