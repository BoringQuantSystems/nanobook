//! G24: audit convenience writers.

use nanobook::Symbol;
use nanobook_broker::ibkr::orders::{OrderOutcome, OrderResult};
use nanobook_rebalancer::audit::{
    AuditLog, log_cron_completed, log_cron_start, log_diff, log_diff_checkpoint,
    log_idempotency_rejection, log_order_filled, log_order_filled_checkpoint,
    log_order_submitted_checkpoint, log_positions, log_risk_check,
    log_risk_check_passed_checkpoint, log_run_completed, log_run_completed_checkpoint,
    log_run_started, log_run_started_checkpoint, parse_audit_events,
};
use nanobook_rebalancer::diff::{Action, CurrentPosition, RebalanceOrder};
use nanobook_rebalancer::kill::verify_no_dangling_orders;
use nanobook_rebalancer::risk::{RiskCheck, RiskReport, RiskStatus};

fn order() -> RebalanceOrder {
    RebalanceOrder {
        symbol: Symbol::new("AAPL"),
        action: Action::Buy,
        shares: 10,
        limit_price_cents: 18_502,
        notional_cents: 185_020,
        description: "open",
    }
}

fn find<'a>(
    events: &'a [nanobook_rebalancer::audit::AuditEvent],
    name: &str,
) -> &'a nanobook_rebalancer::audit::AuditEvent {
    events.iter().find(|e| e.event == name).unwrap()
}

#[test]
fn convenience_writers_round_trip_audit_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.jsonl");
    let mut a = AuditLog::open_in(&path, dir.path()).unwrap();
    let order = order();

    log_cron_completed(&mut a, 1_700_000_001, "w1", 3, 2, 1).unwrap();
    let events = parse_audit_events(&path).unwrap();
    let cron = events.last().unwrap();
    assert_eq!(cron.event, "cron_completed");
    assert_eq!(cron.sequence_number, Some(1_700_000_001));
    assert_eq!(cron.window_id.as_deref(), Some("w1"));
    assert_eq!(cron.data.get("submitted").and_then(|v| v.as_u64()), Some(3));
    assert_eq!(cron.data.get("filled").and_then(|v| v.as_u64()), Some(2));
    assert_eq!(cron.data.get("failed").and_then(|v| v.as_u64()), Some(1));
    assert_eq!(
        a.check_window_already_complete("w1").unwrap(),
        Some(1_700_000_001)
    );

    log_cron_start(&mut a, 1_700_000_000, "w1").unwrap();
    log_idempotency_rejection(&mut a, "w1", 1_700_000_001).unwrap();

    log_order_submitted_checkpoint(&mut a, 8, &order, 4242).unwrap();
    let dangling = verify_no_dangling_orders(&path).unwrap();
    assert_eq!(dangling.len(), 1);
    assert_eq!(dangling[0].ibkr_id, 4242);
    assert_eq!(dangling[0].symbol, "AAPL");

    let result = OrderResult {
        symbol: Symbol::new("AAPL"),
        order_id: 4242,
        filled_shares: 10,
        avg_fill_price: 185.01,
        commission: 1.0,
        status: OrderOutcome::PartialFill,
    };
    log_order_filled(&mut a, &result).unwrap();
    assert!(verify_no_dangling_orders(&path).unwrap().is_empty());

    log_order_filled_checkpoint(&mut a, 12, &result).unwrap();
    log_run_started(&mut a, "t.json", "DU1").unwrap();
    log_run_started_checkpoint(&mut a, 7, "t.json", "DU1").unwrap();
    log_run_completed(&mut a, 2, 1, 1).unwrap();
    log_run_completed_checkpoint(&mut a, 13, 2, 1, 1).unwrap();
    log_positions(
        &mut a,
        &[CurrentPosition {
            symbol: Symbol::new("AAPL"),
            quantity: 100,
            avg_cost_cents: 15_025,
        }],
        100_000_000,
    )
    .unwrap();
    log_diff(&mut a, std::slice::from_ref(&order)).unwrap();
    log_diff_checkpoint(&mut a, 9, std::slice::from_ref(&order)).unwrap();
    let report = RiskReport {
        checks: vec![
            RiskCheck {
                name: "Max position",
                status: RiskStatus::Pass,
                detail: "ok".into(),
            },
            RiskCheck {
                name: "Leverage",
                status: RiskStatus::Fail,
                detail: "bad".into(),
            },
        ],
    };
    log_risk_check(&mut a, &report).unwrap();
    log_risk_check_passed_checkpoint(&mut a, 11, &report).unwrap();

    let events = parse_audit_events(&path).unwrap();
    let start = events.iter().find(|e| e.event == "cron_start").unwrap();
    assert_eq!(start.sequence_number, Some(1_700_000_000));
    assert_eq!(start.window_id.as_deref(), Some("w1"));
    let rejection = find(&events, "idempotency_rejection");
    assert_eq!(rejection.sequence_number, Some(1_700_000_001));
    assert_eq!(rejection.window_id.as_deref(), Some("w1"));

    let filled = events
        .iter()
        .find(|e| e.event == "order_filled" && e.checkpoint.is_none())
        .unwrap();
    assert_eq!(
        filled.data.get("symbol").and_then(|v| v.as_str()),
        Some("AAPL")
    );
    assert_eq!(
        filled.data.get("ibkr_id").and_then(|v| v.as_i64()),
        Some(4242)
    );
    assert_eq!(filled.data.get("filled").and_then(|v| v.as_i64()), Some(10));
    assert_eq!(
        filled.data.get("avg_price").and_then(|v| v.as_f64()),
        Some(185.01)
    );
    assert_eq!(
        filled.data.get("commission").and_then(|v| v.as_f64()),
        Some(1.0)
    );
    assert_eq!(
        filled.data.get("status").and_then(|v| v.as_str()),
        Some("PartialFill")
    );
    let filled_cp = events
        .iter()
        .find(|e| e.event == "order_filled" && e.sequence_number == Some(12))
        .unwrap();
    assert_eq!(filled_cp.checkpoint.as_deref(), Some("OrderFilled"));

    let run_started = events
        .iter()
        .find(|e| e.event == "run_started" && e.checkpoint.is_none())
        .unwrap();
    assert_eq!(
        run_started.data.get("target_file").and_then(|v| v.as_str()),
        Some("t.json")
    );
    assert_eq!(
        run_started.data.get("account").and_then(|v| v.as_str()),
        Some("DU1")
    );
    let run_started_cp = events
        .iter()
        .find(|e| e.event == "run_started" && e.sequence_number == Some(7))
        .unwrap();
    assert_eq!(run_started_cp.checkpoint.as_deref(), Some("RunStarted"));

    let completed = events
        .iter()
        .find(|e| e.event == "run_completed" && e.checkpoint.is_none())
        .unwrap();
    assert_eq!(
        completed.data.get("submitted").and_then(|v| v.as_u64()),
        Some(2)
    );
    assert_eq!(
        completed.data.get("filled").and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        completed.data.get("failed").and_then(|v| v.as_u64()),
        Some(1)
    );
    let completed_cp = events
        .iter()
        .find(|e| e.event == "run_completed" && e.sequence_number == Some(13))
        .unwrap();
    assert_eq!(completed_cp.checkpoint.as_deref(), Some("RunCompleted"));

    let positions = find(&events, "positions_fetched");
    let pos0 = &positions.data["positions"][0];
    assert_eq!(pos0.get("symbol").and_then(|v| v.as_str()), Some("AAPL"));
    assert_eq!(pos0.get("qty").and_then(|v| v.as_i64()), Some(100));
    assert_eq!(pos0.get("avg_cost").and_then(|v| v.as_f64()), Some(150.25));
    assert_eq!(
        positions.data.get("equity").and_then(|v| v.as_f64()),
        Some(1_000_000.0)
    );

    let diff = events
        .iter()
        .find(|e| e.event == "diff_computed" && e.checkpoint.is_none())
        .unwrap();
    let d0 = &diff.data["orders"][0];
    assert_eq!(d0.get("symbol").and_then(|v| v.as_str()), Some("AAPL"));
    assert_eq!(d0.get("action").and_then(|v| v.as_str()), Some("BUY"));
    assert_eq!(d0.get("shares").and_then(|v| v.as_i64()), Some(10));
    assert_eq!(d0.get("limit").and_then(|v| v.as_f64()), Some(185.02));
    assert_eq!(d0.get("description").and_then(|v| v.as_str()), Some("open"));
    let diff_cp = events
        .iter()
        .find(|e| e.event == "diff_computed" && e.sequence_number == Some(9))
        .unwrap();
    assert_eq!(diff_cp.checkpoint.as_deref(), Some("DiffComputed"));

    let risk = events.iter().find(|e| e.event == "risk_check").unwrap();
    assert_eq!(
        risk.data.get("passed").and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        risk.data["checks"][1].get("name").and_then(|v| v.as_str()),
        Some("Leverage")
    );
    assert_eq!(
        risk.data["checks"][1]
            .get("status")
            .and_then(|v| v.as_str()),
        Some("FAIL")
    );
    assert_eq!(
        risk.data["checks"][1]
            .get("detail")
            .and_then(|v| v.as_str()),
        Some("bad")
    );
    let risk_cp = events
        .iter()
        .find(|e| e.event == "risk_check_passed")
        .unwrap();
    assert_eq!(risk_cp.sequence_number, Some(11));
    assert_eq!(risk_cp.checkpoint.as_deref(), Some("RiskCheckPassed"));
    assert_eq!(risk_cp.data["checks"].as_array().unwrap().len(), 2);
}
