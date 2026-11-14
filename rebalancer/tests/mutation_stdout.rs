//! Mutation-gap tests for output that only reaches standard output:
//! the portfolio and plan tables of a run, and the guidance of `recover`.
//! Each test re-runs itself in a child process with `--nocapture` to read the
//! output (see `common::run_child`).

mod common;

use std::time::{Duration, SystemTime};

use common::{child_scenario, run_child};
use nanobook::Symbol;
use nanobook_broker::error::BrokerError;
use nanobook_broker::ibkr::orders::OrderResult;
use nanobook_broker::types::{Account, Position, Quote};
use nanobook_broker::{BrokerSide, ClientOrderId};
use nanobook_rebalancer::broker::{BrokerGateway, BrokerResult};
use nanobook_rebalancer::config::Config;
use nanobook_rebalancer::execution::{CronMode, RunOptions, ShutdownFlag, run_with_client};
use nanobook_rebalancer::recovery::run_recover;
use nanobook_rebalancer::target::TargetSpec;

fn config(logs_dir: &std::path::Path) -> Config {
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
        logs_dir.display()
    ))
    .unwrap()
}

fn spec() -> TargetSpec {
    TargetSpec::from_json(
        r#"{
            "timestamp": "2026-02-08T15:30:00Z",
            "targets": [{ "symbol": "AAPL", "weight": 0.1 }]
        }"#,
    )
    .unwrap()
}

/// Holds 10 AAPL at an average cost of $150.00; every quote has mid $100.00.
struct StubBroker {
    equity_cents: i64,
}

impl BrokerGateway for StubBroker {
    fn account_summary(&self) -> BrokerResult<Account> {
        Ok(Account {
            equity_cents: self.equity_cents,
            buying_power_cents: self.equity_cents,
            cash_cents: self.equity_cents,
            gross_position_value_cents: 0,
        })
    }

    fn positions(&self) -> BrokerResult<Vec<Position>> {
        Ok(vec![Position {
            symbol: Symbol::new("AAPL"),
            quantity: 10,
            avg_cost_cents: 15_000,
            market_value_cents: 0,
            unrealized_pnl_cents: 0,
        }])
    }

    fn prices(&self, _symbols: &[Symbol]) -> BrokerResult<Vec<(Symbol, i64)>> {
        Ok(Vec::new())
    }

    fn quotes(&self, symbols: &[Symbol]) -> BrokerResult<Vec<Quote>> {
        Ok(symbols
            .iter()
            .map(|symbol| Quote {
                symbol: *symbol,
                bid_cents: 9_999,
                ask_cents: 10_001,
                last_cents: 10_000,
                volume: 0,
                timestamp: SystemTime::now(),
            })
            .collect())
    }

    fn execute_limit_order(
        &self,
        _symbol: Symbol,
        _side: BrokerSide,
        _shares: u64,
        _limit_price_cents: i64,
        _client_order_id: Option<&ClientOrderId>,
        _timeout: Duration,
    ) -> BrokerResult<OrderResult> {
        Err(BrokerError::Order("dry run only".into()))
    }
}

fn child_display(scenario: &str) {
    let equity_cents = match scenario {
        "equity_100k" => 10_000_000,
        "equity_zero" => 0,
        other => panic!("unknown scenario {other}"),
    };
    let dir = tempfile::tempdir().unwrap();
    let options = RunOptions {
        dry_run: true,
        force: true,
        target_file: "t.json".into(),
        cron_mode: false,
    };
    // The result is not part of what is checked here; the run may stop at the
    // risk check.
    let _ = run_with_client(
        &config(&dir.path().join("logs")),
        &spec(),
        &options,
        &StubBroker { equity_cents },
        &ShutdownFlag::new(),
        dir.path(),
        &mut || Ok(false),
    );
}

#[test]
fn run_prints_portfolio_weights_and_the_plan() {
    if let Some(scenario) = child_scenario() {
        child_display(&scenario);
        return;
    }

    // 10 shares x $150.00 average cost = $1500.00 = 1.5 % of $100000.00.
    let out = run_child("run_prints_portfolio_weights_and_the_plan", "equity_100k");
    assert!(out.contains("CURRENT PORTFOLIO:"), "stdout:\n{out}");
    let line = out
        .lines()
        .find(|l| l.contains("AAPL") && l.contains(" avg = $"))
        .unwrap_or_else(|| panic!("no position line in:\n{out}"));
    assert!(
        line.contains("@ $  150.00 avg = $   1500.00  (1.5%)"),
        "{line}"
    );

    // The plan table: 90 shares to reach $10000 from $1000 at mid $100.00.
    assert!(out.contains("REBALANCE ORDERS:"), "stdout:\n{out}");
    let order = out
        .lines()
        .find(|l| l.contains("BUY") && l.contains("(increase)"))
        .unwrap_or_else(|| panic!("no order line in:\n{out}"));
    assert!(order.contains("$   100.05 $    9000.00"), "{order}");
    assert!(out.contains("Est. cost: $"), "stdout:\n{out}");
}

#[test]
fn run_prints_zero_weight_when_equity_is_zero() {
    if let Some(scenario) = child_scenario() {
        child_display(&scenario);
        return;
    }

    let out = run_child("run_prints_zero_weight_when_equity_is_zero", "equity_zero");
    let line = out
        .lines()
        .find(|l| l.contains("AAPL") && l.contains(" avg = $"))
        .unwrap_or_else(|| panic!("no position line in:\n{out}"));
    assert!(line.ends_with("avg = $   1500.00  (0.0%)"), "{line}");
}

#[test]
fn cron_mode_reports_enabled() {
    assert!(CronMode::new(1_700_000_000).is_enabled());
}

fn child_recover(scenario: &str) {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let audit_path = config.audit_path();
    let dry_run = scenario.ends_with("_dry");
    match scenario {
        "restart" | "restart_dry" => {
            std::fs::write(&audit_path, "").unwrap();
            let _ = run_recover(&config, &spec(), dry_run, None);
        }
        #[cfg(feature = "write_ahead_logging")]
        "reconcile_not_needed" | "reconcile_needed" => child_recover_with_broker(&config, scenario),
        other => panic!("unknown scenario {other}"),
    }
}

#[cfg(feature = "write_ahead_logging")]
fn child_recover_with_broker(config: &Config, scenario: &str) {
    use nanobook_broker::Broker;
    use nanobook_broker::mock::{FillMode, MockBroker};
    use nanobook_rebalancer::audit::{AuditLog, Checkpoint};

    let dir = config.audit_path().parent().unwrap().to_path_buf();
    {
        let mut log = AuditLog::open_in(&config.audit_path(), &dir).unwrap();
        log.log_checkpoint(Checkpoint::RunStarted, 1, serde_json::json!({"t": 1}))
            .unwrap();
        match scenario {
            // Orders known only from the diff carry no client order id, so
            // none of them is an incomplete intent.
            "reconcile_not_needed" => log
                .log_checkpoint(
                    Checkpoint::DiffComputed,
                    2,
                    serde_json::json!({"orders": [{
                        "symbol": "AAPL", "action": "BUY", "shares": 50, "limit": 160.0
                    }]}),
                )
                .unwrap(),
            "reconcile_needed" => log
                .log_checkpoint(
                    Checkpoint::OrderIntent,
                    2,
                    serde_json::json!({
                        "symbol": "AAPL", "action": "Buy", "shares": 50, "limit": 160.0,
                        "client_order_id": "c1"
                    }),
                )
                .unwrap(),
            _ => unreachable!(),
        }
    }
    let mut broker = MockBroker::builder()
        .fill_mode(FillMode::ImmediateFull)
        .build();
    broker.connect().unwrap();
    let _ = run_recover(config, &spec(), true, Some(&broker));
}

#[test]
fn recover_prints_run_hint_only_when_not_a_dry_run() {
    if let Some(scenario) = child_scenario() {
        child_recover(&scenario);
        return;
    }

    let real = run_child("recover_prints_run_hint_only_when_not_a_dry_run", "restart");
    assert!(
        real.contains("Safe to restart the entire rebalance from the beginning."),
        "stdout:\n{real}"
    );
    assert!(
        real.contains("Run: rebalancer run <target.json>"),
        "stdout:\n{real}"
    );
    assert!(!real.contains("Dry Run"), "stdout:\n{real}");

    let dry = run_child(
        "recover_prints_run_hint_only_when_not_a_dry_run",
        "restart_dry",
    );
    assert!(
        dry.contains("Safe to restart the entire rebalance from the beginning."),
        "stdout:\n{dry}"
    );
    assert!(!dry.contains("Run: rebalancer run"), "stdout:\n{dry}");
    assert!(
        dry.contains("=== Dry Run - No Action Taken ==="),
        "stdout:\n{dry}"
    );
}

#[cfg(feature = "write_ahead_logging")]
#[test]
fn recover_reconciles_only_incomplete_intents() {
    if let Some(scenario) = child_scenario() {
        child_recover(&scenario);
        return;
    }

    let not_needed = run_child(
        "recover_reconciles_only_incomplete_intents",
        "reconcile_not_needed",
    );
    assert!(
        not_needed.contains("=== Broker State Comparison ==="),
        "stdout:\n{not_needed}"
    );
    assert!(
        !not_needed.contains("=== Broker Reconciliation ==="),
        "stdout:\n{not_needed}"
    );

    let needed = run_child(
        "recover_reconciles_only_incomplete_intents",
        "reconcile_needed",
    );
    assert!(
        needed.contains("=== Broker Reconciliation ==="),
        "stdout:\n{needed}"
    );
    assert!(
        needed.contains("Broker reconciliation completed successfully."),
        "stdout:\n{needed}"
    );
}
