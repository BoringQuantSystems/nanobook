//! S1: run_with_client confirmation, counters, PID file, and min-trade filter.

use std::cell::RefCell;
use std::time::{Duration, SystemTime};

use nanobook::Symbol;
use nanobook_broker::error::BrokerError;
use nanobook_broker::ibkr::orders::{OrderOutcome, OrderResult};
use nanobook_broker::types::{Account, Position, Quote};
use nanobook_broker::{BrokerSide, ClientOrderId};
use nanobook_rebalancer::audit::{self, parse_audit_events};
use nanobook_rebalancer::broker::BrokerGateway;
use nanobook_rebalancer::config::Config;
use nanobook_rebalancer::error::{Error, Result};
use nanobook_rebalancer::execution::{RunOptions, ShutdownFlag, run_with_client};
use nanobook_rebalancer::pid_file::DEFAULT_PID_FILE;
use nanobook_rebalancer::target::TargetSpec;

struct ScriptedBroker {
    executes: RefCell<Vec<(String, BrokerSide, u64, i64)>>,
    cancels: RefCell<Vec<u64>>,
    pid_during_execute: RefCell<Option<u32>>,
    pid_path: std::path::PathBuf,
    shutdown: ShutdownFlag,
    shutdown_on_first: bool,
    outcomes: Vec<ExecOutcome>,
}

enum ExecOutcome {
    Filled,
    Partial { filled: i64, id: i32 },
    Cancelled,
    Failed,
    Err,
}

impl ScriptedBroker {
    fn new(workdir: &std::path::Path, shutdown: ShutdownFlag, outcomes: Vec<ExecOutcome>) -> Self {
        Self {
            executes: RefCell::new(Vec::new()),
            cancels: RefCell::new(Vec::new()),
            pid_during_execute: RefCell::new(None),
            pid_path: workdir.join(DEFAULT_PID_FILE),
            shutdown,
            shutdown_on_first: false,
            outcomes,
        }
    }
}

impl BrokerGateway for ScriptedBroker {
    fn account_summary(&self) -> nanobook_rebalancer::broker::BrokerResult<Account> {
        Ok(Account {
            equity_cents: 100_000_000,
            buying_power_cents: 100_000_000,
            cash_cents: 100_000_000,
            gross_position_value_cents: 0,
        })
    }

    fn positions(&self) -> nanobook_rebalancer::broker::BrokerResult<Vec<Position>> {
        Ok(Vec::new())
    }

    fn prices(
        &self,
        _symbols: &[Symbol],
    ) -> nanobook_rebalancer::broker::BrokerResult<Vec<(Symbol, i64)>> {
        Ok(Vec::new())
    }

    fn quotes(&self, symbols: &[Symbol]) -> nanobook_rebalancer::broker::BrokerResult<Vec<Quote>> {
        Ok(symbols
            .iter()
            .map(|symbol| {
                let (bid, ask, last) = if symbol.as_str() == "PENNY" {
                    (1_999, 2_001, 2_000)
                } else {
                    (9_999, 10_001, 10_000)
                };
                Quote {
                    symbol: *symbol,
                    bid_cents: bid,
                    ask_cents: ask,
                    last_cents: last,
                    volume: 0,
                    timestamp: SystemTime::now(),
                }
            })
            .collect())
    }

    fn execute_limit_order(
        &self,
        symbol: Symbol,
        side: BrokerSide,
        shares: u64,
        limit_price_cents: i64,
        _client_order_id: Option<&ClientOrderId>,
        _timeout: Duration,
    ) -> nanobook_rebalancer::broker::BrokerResult<OrderResult> {
        if let Ok(contents) = std::fs::read_to_string(&self.pid_path)
            && let Ok(pid) = contents.trim().parse::<u32>()
        {
            *self.pid_during_execute.borrow_mut() = Some(pid);
        }
        self.executes.borrow_mut().push((
            symbol.as_str().to_string(),
            side,
            shares,
            limit_price_cents,
        ));
        let index = self.executes.borrow().len() - 1;
        if self.shutdown_on_first {
            self.shutdown.request_shutdown();
        }
        let outcome = self.outcomes.get(index).unwrap_or(&ExecOutcome::Filled);
        match outcome {
            ExecOutcome::Filled => Ok(OrderResult {
                symbol,
                order_id: (index as i32) + 1,
                filled_shares: shares as i64,
                avg_fill_price: limit_price_cents as f64 / 100.0,
                commission: 0.0,
                status: OrderOutcome::Filled,
            }),
            ExecOutcome::Partial { filled, id } => Ok(OrderResult {
                symbol,
                order_id: *id,
                filled_shares: *filled,
                avg_fill_price: limit_price_cents as f64 / 100.0,
                commission: 0.0,
                status: OrderOutcome::PartialFill,
            }),
            ExecOutcome::Cancelled => Ok(OrderResult {
                symbol,
                order_id: 30,
                filled_shares: 0,
                avg_fill_price: 0.0,
                commission: 0.0,
                status: OrderOutcome::Cancelled,
            }),
            ExecOutcome::Failed => Ok(OrderResult {
                symbol,
                order_id: 40,
                filled_shares: 0,
                avg_fill_price: 0.0,
                commission: 0.0,
                status: OrderOutcome::Failed,
            }),
            ExecOutcome::Err => Err(BrokerError::Order("rej".into())),
        }
    }

    fn cancel_order(&self, order_id: u64) -> nanobook_rebalancer::broker::BrokerResult<()> {
        self.cancels.borrow_mut().push(order_id);
        Ok(())
    }
}

fn config(workdir: &std::path::Path) -> Config {
    toml::from_str(&format!(
        r#"
[connection]
host = "127.0.0.1"
port = 4002
client_id = 1
[account]
id = "DU1"
type = "cash"
[execution]
order_interval_ms = 0
limit_offset_bps = 5
order_timeout_secs = 1
max_orders_per_run = 50
quote_staleness_threshold_sec = 30
[risk]
max_position_pct = 1.0
max_leverage = 2.0
min_trade_usd = 100.0
max_trade_usd = 1000000.0
allow_short = true
max_short_pct = 1.0
[cost]
[logging]
dir = "{}"
audit_file = "audit.jsonl"
"#,
        workdir.join("logs").display()
    ))
    .unwrap()
}

fn spec_seven() -> TargetSpec {
    TargetSpec::from_json(
        r#"{
            "timestamp": "2026-02-08T15:30:00Z",
            "targets": [
                { "symbol": "T1", "weight": 0.1 },
                { "symbol": "T2", "weight": 0.1 },
                { "symbol": "T3", "weight": 0.1 },
                { "symbol": "T4", "weight": 0.1 },
                { "symbol": "T5", "weight": 0.1 },
                { "symbol": "T6", "weight": 0.1 },
                { "symbol": "PENNY", "weight": 0.00008 }
            ]
        }"#,
    )
    .unwrap()
}

fn spec_three() -> TargetSpec {
    TargetSpec::from_json(
        r#"{
            "timestamp": "2026-02-08T15:30:00Z",
            "targets": [
                { "symbol": "T1", "weight": 0.1 },
                { "symbol": "T2", "weight": 0.1 },
                { "symbol": "T3", "weight": 0.1 }
            ]
        }"#,
    )
    .unwrap()
}

fn opts(force: bool, dry_run: bool, cron_mode: bool) -> RunOptions {
    RunOptions {
        dry_run,
        force,
        target_file: "t.json".into(),
        cron_mode,
    }
}

fn t1_outcomes() -> Vec<ExecOutcome> {
    vec![
        ExecOutcome::Filled,
        ExecOutcome::Partial {
            filled: 400,
            id: 22,
        },
        ExecOutcome::Cancelled,
        ExecOutcome::Failed,
        ExecOutcome::Err,
        ExecOutcome::Filled,
    ]
}

fn assert_six_buys(executes: &[(String, BrokerSide, u64, i64)]) {
    assert_eq!(executes.len(), 6);
    for (i, (symbol, side, shares, limit)) in executes.iter().enumerate() {
        assert_eq!(symbol, &format!("T{}", i + 1));
        assert_eq!(*side, BrokerSide::Buy);
        assert_eq!(*shares, 1000);
        assert_eq!(*limit, 10_005);
    }
}

fn completed_counts(path: &std::path::Path, event: &str) -> (u64, u64, u64) {
    let events = parse_audit_events(path).unwrap();
    let e = events.iter().find(|ev| ev.event == event).unwrap();
    (
        e.data.get("submitted").and_then(|v| v.as_u64()).unwrap(),
        e.data.get("filled").and_then(|v| v.as_u64()).unwrap(),
        e.data.get("failed").and_then(|v| v.as_u64()).unwrap(),
    )
}

#[test]
fn force_cron_places_six_orders_and_skips_penny() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let spec = spec_seven();
    let shutdown = ShutdownFlag::new();
    let broker = ScriptedBroker::new(dir.path(), shutdown.clone(), t1_outcomes());
    let mut confirm = || -> Result<bool> { panic!("confirm must not be called") };
    run_with_client(
        &config,
        &spec,
        &opts(true, false, true),
        &broker,
        &shutdown,
        dir.path(),
        &mut confirm,
    )
    .unwrap();
    assert_six_buys(&broker.executes.borrow());
    assert!(
        !broker
            .executes
            .borrow()
            .iter()
            .any(|(s, _, _, _)| s == "PENNY")
    );
    assert_eq!(
        broker.pid_during_execute.borrow().as_ref(),
        Some(&std::process::id())
    );
    assert!(!dir.path().join(DEFAULT_PID_FILE).exists());
    let audit = config.audit_path();
    assert_eq!(completed_counts(&audit, "run_completed"), (6, 3, 3));
    assert_eq!(completed_counts(&audit, "cron_completed"), (6, 3, 3));
}

#[test]
fn confirm_yes_places_orders() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let spec = spec_seven();
    let shutdown = ShutdownFlag::new();
    let broker = ScriptedBroker::new(dir.path(), shutdown.clone(), t1_outcomes());
    let mut calls = 0;
    let mut confirm = || {
        calls += 1;
        Ok(true)
    };
    run_with_client(
        &config,
        &spec,
        &opts(false, false, false),
        &broker,
        &shutdown,
        dir.path(),
        &mut confirm,
    )
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(broker.executes.borrow().len(), 6);
    let events = parse_audit_events(&config.audit_path()).unwrap();
    let confirmed = events.iter().find(|e| e.event == "user_confirmed").unwrap();
    assert_eq!(
        confirmed.data.get("approved").and_then(|v| v.as_bool()),
        Some(true)
    );
}

#[test]
fn confirm_no_places_no_orders() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let spec = spec_seven();
    let shutdown = ShutdownFlag::new();
    let broker = ScriptedBroker::new(dir.path(), shutdown.clone(), t1_outcomes());
    let mut confirm = || Ok(false);
    run_with_client(
        &config,
        &spec,
        &opts(false, false, false),
        &broker,
        &shutdown,
        dir.path(),
        &mut confirm,
    )
    .unwrap();
    assert!(broker.executes.borrow().is_empty());
    let events = parse_audit_events(&config.audit_path()).unwrap();
    let confirmed = events.iter().find(|e| e.event == "user_confirmed").unwrap();
    assert_eq!(
        confirmed.data.get("approved").and_then(|v| v.as_bool()),
        Some(false)
    );
}

#[test]
fn dry_run_places_no_orders_and_skips_confirm() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let spec = spec_seven();
    let shutdown = ShutdownFlag::new();
    let broker = ScriptedBroker::new(dir.path(), shutdown.clone(), t1_outcomes());
    let mut confirm = || -> Result<bool> { panic!("confirm must not be called") };
    run_with_client(
        &config,
        &spec,
        &opts(false, true, false),
        &broker,
        &shutdown,
        dir.path(),
        &mut confirm,
    )
    .unwrap();
    assert!(broker.executes.borrow().is_empty());
}

#[test]
fn shutdown_after_partial_cancels_current_and_skips_rest() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let spec = spec_three();
    let shutdown = ShutdownFlag::new();
    let mut broker = ScriptedBroker::new(
        dir.path(),
        shutdown.clone(),
        vec![ExecOutcome::Partial {
            filled: 400,
            id: 55,
        }],
    );
    broker.shutdown_on_first = true;
    let mut confirm = || Ok(true);
    run_with_client(
        &config,
        &spec,
        &opts(true, false, false),
        &broker,
        &shutdown,
        dir.path(),
        &mut confirm,
    )
    .unwrap();
    assert_eq!(broker.executes.borrow().len(), 1);
    assert_eq!(*broker.cancels.borrow(), vec![55]);
    let events = parse_audit_events(&config.audit_path()).unwrap();
    let kill = events.iter().find(|e| e.event == "kill_completed").unwrap();
    assert_eq!(
        kill.data.get("method").and_then(|v| v.as_str()),
        Some("graceful")
    );
    assert_eq!(
        kill.data
            .get("orders_cancelled_count")
            .and_then(|v| v.as_u64()),
        Some(3)
    );
}

#[test]
fn cron_mode_refuses_a_completed_window_before_orders() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let spec = spec_three();
    let window_id = spec.window_id();
    {
        let mut audit = audit::AuditLog::open_in(&config.audit_path(), dir.path()).unwrap();
        audit::log_cron_completed(&mut audit, 42, &window_id, 1, 1, 0).unwrap();
    }
    let shutdown = ShutdownFlag::new();
    let broker = ScriptedBroker::new(dir.path(), shutdown.clone(), Vec::new());
    let mut confirm = || -> Result<bool> { panic!("confirm must not be called") };
    match run_with_client(
        &config,
        &spec,
        &opts(true, false, true),
        &broker,
        &shutdown,
        dir.path(),
        &mut confirm,
    ) {
        Err(Error::IdempotencyRejection {
            window_id: got_window,
            sequence_number,
        }) => {
            assert_eq!(got_window, window_id);
            assert_eq!(sequence_number, 42);
        }
        other => panic!("expected IdempotencyRejection, got {other:?}"),
    }
    assert!(broker.executes.borrow().is_empty());
    assert!(!dir.path().join(DEFAULT_PID_FILE).exists());
    let events = parse_audit_events(&config.audit_path()).unwrap();
    assert!(
        events
            .iter()
            .any(|event| event.event == "idempotency_rejection")
    );
    assert!(events.iter().all(|event| event.event != "run_started"));
}

#[test]
fn confirm_error_is_aborted() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let spec = spec_seven();
    let shutdown = ShutdownFlag::new();
    let broker = ScriptedBroker::new(dir.path(), shutdown.clone(), t1_outcomes());
    let mut confirm = || Err(Error::Aborted("prompt failed".into()));
    match run_with_client(
        &config,
        &spec,
        &opts(false, false, false),
        &broker,
        &shutdown,
        dir.path(),
        &mut confirm,
    ) {
        Err(Error::Aborted(m)) => assert!(m.contains("prompt failed")),
        other => panic!("expected Aborted, got {other:?}"),
    }
    assert!(broker.executes.borrow().is_empty());
}
