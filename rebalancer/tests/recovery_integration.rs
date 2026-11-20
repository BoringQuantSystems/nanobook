//! Integration tests for F9 crash recovery.

use nanobook::Symbol;
use nanobook_broker::mock::{FillMode, MockBroker};
use nanobook_broker::{Broker, BrokerOrder, BrokerOrderType, BrokerSide};
#[cfg(feature = "write_ahead_logging")]
use nanobook_rebalancer::audit::parse_audit_events;
use nanobook_rebalancer::audit::{AuditLog, log_positions_checkpoint};
use nanobook_rebalancer::config::Config;
use nanobook_rebalancer::diff::CurrentPosition;
#[cfg(feature = "write_ahead_logging")]
use nanobook_rebalancer::error::Error;
use nanobook_rebalancer::recovery::{RecoveryAction, reconstruct_state, run_recover};
use nanobook_rebalancer::target::TargetSpec;
use tempfile::tempdir;

#[test]
fn roundtrip_position_through_audit_log_preserves_avg_cost() {
    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("roundtrip_audit.jsonl");
    let workdir = dir.path();

    let positions = vec![CurrentPosition {
        symbol: Symbol::new("AAPL"),
        quantity: 100,
        avg_cost_cents: 15_000,
    }];

    {
        let mut log = AuditLog::open_in(&audit_path, workdir).unwrap();
        log_positions_checkpoint(&mut log, 1, &positions, 1_000_000_00).unwrap();
    }

    let (state, _action) = nanobook_rebalancer::recovery::reconstruct_state(&audit_path).unwrap();
    assert_eq!(state.positions.len(), 1);
    assert_eq!(
        state.positions[0].avg_cost_cents, 15_000,
        "avg_cost_cents roundtrip failed"
    );
    assert_eq!(
        state.positions[0].quantity, 100,
        "quantity roundtrip failed"
    );
}

/// Test recovery from crash at each checkpoint.
#[test]
fn test_recovery_from_run_started() {
    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let workdir = dir.path();

    // Simulate a crash at run_started checkpoint
    {
        let mut log = AuditLog::open_in(&audit_path, workdir).unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::RunStarted,
            1,
            serde_json::json!({"target": "test"}),
        )
        .unwrap();
    }

    // Recover state
    let (state, action) = reconstruct_state(&audit_path).unwrap();

    assert_eq!(
        state.checkpoint,
        nanobook_rebalancer::audit::Checkpoint::RunStarted
    );
    assert_eq!(state.sequence_number, 1);
    assert!(!state.run_completed);
    assert_eq!(action, RecoveryAction::Restart);
}

#[test]
fn test_recovery_from_positions_fetched() {
    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let workdir = dir.path();

    // Simulate a crash at positions_fetched checkpoint
    {
        let mut log = AuditLog::open_in(&audit_path, workdir).unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::RunStarted,
            1,
            serde_json::json!({"target": "test"}),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::PositionsFetched,
            2,
            serde_json::json!({
                "positions": [{
                    "symbol": "AAPL",
                    "qty": 100,
                    "avg_cost": 150.0
                }]
            }),
        )
        .unwrap();
    }

    // Recover state
    let (state, action) = reconstruct_state(&audit_path).unwrap();

    assert_eq!(
        state.checkpoint,
        nanobook_rebalancer::audit::Checkpoint::PositionsFetched
    );
    assert_eq!(state.sequence_number, 2);
    assert_eq!(state.positions.len(), 1);
    assert_eq!(state.positions[0].symbol.as_str(), "AAPL");
    assert_eq!(state.positions[0].quantity, 100);
    assert!(!state.run_completed);
    assert_eq!(action, RecoveryAction::Restart);
}

#[test]
fn test_recovery_from_diff_computed() {
    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let workdir = dir.path();

    // Simulate a crash at diff_computed checkpoint
    {
        let mut log = AuditLog::open_in(&audit_path, workdir).unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::RunStarted,
            1,
            serde_json::json!({"target": "test"}),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::PositionsFetched,
            2,
            serde_json::json!({
                "positions": [{
                    "symbol": "AAPL",
                    "qty": 100,
                    "avg_cost": 150.0
                }]
            }),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::DiffComputed,
            3,
            serde_json::json!({
                "orders": [{
                    "symbol": "AAPL",
                    "action": "Buy",
                    "shares": 50,
                    "limit": 160.0,
                    "description": "test"
                }]
            }),
        )
        .unwrap();
    }

    // Recover state
    let (state, action) = reconstruct_state(&audit_path).unwrap();

    assert_eq!(
        state.checkpoint,
        nanobook_rebalancer::audit::Checkpoint::DiffComputed
    );
    assert_eq!(state.sequence_number, 3);
    assert_eq!(state.orders.len(), 1);
    assert_eq!(state.orders[0].symbol.as_str(), "AAPL");
    assert_eq!(state.orders[0].shares, 50);
    assert!(!state.orders[0].submitted);
    assert!(!state.run_completed);
    assert_eq!(action, RecoveryAction::Restart);
}

#[test]
fn test_recovery_from_order_submitted() {
    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let workdir = dir.path();

    // Simulate a crash at order_submitted checkpoint
    {
        let mut log = AuditLog::open_in(&audit_path, workdir).unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::RunStarted,
            1,
            serde_json::json!({"target": "test"}),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::PositionsFetched,
            2,
            serde_json::json!({
                "positions": [{
                    "symbol": "AAPL",
                    "qty": 100,
                    "avg_cost": 150.0
                }]
            }),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::DiffComputed,
            3,
            serde_json::json!({
                "orders": [{
                    "symbol": "AAPL",
                    "action": "Buy",
                    "shares": 50,
                    "limit": 160.0,
                    "description": "test"
                }]
            }),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::OrderSubmitted,
            4,
            serde_json::json!({
                "symbol": "AAPL",
                "action": "Buy",
                "ibkr_id": 12345
            }),
        )
        .unwrap();
    }

    // Recover state
    let (state, action) = reconstruct_state(&audit_path).unwrap();

    assert_eq!(
        state.checkpoint,
        nanobook_rebalancer::audit::Checkpoint::OrderSubmitted
    );
    assert_eq!(state.sequence_number, 4);
    assert_eq!(state.orders.len(), 1);
    assert_eq!(state.orders[0].ibkr_id, 12345);
    assert!(state.orders[0].submitted);
    assert!(!state.orders[0].filled);
    assert!(!state.run_completed);
    assert_eq!(action, RecoveryAction::ManualReview);
}

#[test]
fn test_recovery_from_order_filled() {
    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let workdir = dir.path();

    // Simulate a crash at order_filled checkpoint
    {
        let mut log = AuditLog::open_in(&audit_path, workdir).unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::RunStarted,
            1,
            serde_json::json!({"target": "test"}),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::PositionsFetched,
            2,
            serde_json::json!({
                "positions": [{
                    "symbol": "AAPL",
                    "qty": 100,
                    "avg_cost": 150.0
                }]
            }),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::DiffComputed,
            3,
            serde_json::json!({
                "orders": [{
                    "symbol": "AAPL",
                    "action": "Buy",
                    "shares": 50,
                    "limit": 160.0,
                    "description": "test"
                }]
            }),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::OrderSubmitted,
            4,
            serde_json::json!({
                "symbol": "AAPL",
                "action": "Buy",
                "ibkr_id": 12345
            }),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::OrderFilled,
            5,
            serde_json::json!({
                "symbol": "AAPL",
                "ibkr_id": 12345,
                "filled": 50,
                "avg_price": 155.0,
                "commission": 1.0,
                "status": "Filled"
            }),
        )
        .unwrap();
    }

    // Recover state
    let (state, action) = reconstruct_state(&audit_path).unwrap();

    assert_eq!(
        state.checkpoint,
        nanobook_rebalancer::audit::Checkpoint::OrderFilled
    );
    assert_eq!(state.sequence_number, 5);
    assert_eq!(state.orders.len(), 1);
    assert!(state.orders[0].submitted);
    assert!(state.orders[0].filled);
    assert!(!state.run_completed);
    assert_eq!(action, RecoveryAction::Restart);
}

#[test]
fn test_recovery_from_run_completed() {
    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let workdir = dir.path();

    // Simulate a crash at run_completed checkpoint
    {
        let mut log = AuditLog::open_in(&audit_path, workdir).unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::RunStarted,
            1,
            serde_json::json!({"target": "test"}),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::RunCompleted,
            2,
            serde_json::json!({
                "submitted": 1,
                "filled": 1,
                "failed": 0
            }),
        )
        .unwrap();
    }

    // Recover state
    let (state, action) = reconstruct_state(&audit_path).unwrap();

    assert_eq!(
        state.checkpoint,
        nanobook_rebalancer::audit::Checkpoint::RunCompleted
    );
    assert_eq!(state.sequence_number, 2);
    assert!(state.run_completed);
    assert_eq!(action, RecoveryAction::Restart);
}

#[test]
fn test_checkpoint_coverage_all_checkpoints() {
    let checkpoints = vec![
        nanobook_rebalancer::audit::Checkpoint::RunStarted,
        nanobook_rebalancer::audit::Checkpoint::PositionsFetched,
        nanobook_rebalancer::audit::Checkpoint::DiffComputed,
        nanobook_rebalancer::audit::Checkpoint::RiskCheckPassed,
        nanobook_rebalancer::audit::Checkpoint::OrderIntent,
        nanobook_rebalancer::audit::Checkpoint::OrderSubmitted,
        nanobook_rebalancer::audit::Checkpoint::OrderFailed,
        nanobook_rebalancer::audit::Checkpoint::OrderFilled,
        nanobook_rebalancer::audit::Checkpoint::RunCompleted,
    ];

    for (i, checkpoint) in checkpoints.iter().enumerate() {
        let dir = tempfile::tempdir().unwrap();
        let audit_path = dir.path().join("audit.jsonl");
        let workdir = dir.path();

        // Simulate crash at this checkpoint
        {
            let mut log = AuditLog::open_in(&audit_path, workdir).unwrap();
            log.log_checkpoint(
                nanobook_rebalancer::audit::Checkpoint::RunStarted,
                1,
                serde_json::json!({"target": "test"}),
            )
            .unwrap();

            // Add the specific checkpoint
            log.log_checkpoint(
                *checkpoint,
                (i + 2) as u64,
                serde_json::json!({"test": "data"}),
            )
            .unwrap();
        }

        // Verify recovery works
        let (state, _action) = reconstruct_state(&audit_path).unwrap();
        assert_eq!(
            state.checkpoint, *checkpoint,
            "Checkpoint mismatch for {:?}",
            checkpoint
        );
        assert_eq!(state.sequence_number, (i + 2) as u64);
    }
}

/// Test recovery with broker state comparison.
#[test]
fn test_recovery_with_broker_state_comparison() {
    let dir = tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let workdir = dir.path();
    let config_path = dir.path().join("config.toml");
    let target_path = dir.path().join("target.json");

    // Create a minimal config
    std::fs::write(
        &config_path,
        format!(
            r#"
[account]
id = "U1234567"
type = "cash"

[connection]
host = "127.0.0.1"
port = 7497
client_id = 1

[logging]
# The path is written as a TOML string value, escaped as needed: a Windows temp
# path such as C:\Users\RUNNER~1\AppData\Local\Temp\... has backslashes, and a
# user name may hold an apostrophe.
dir = {}
audit_file = "audit.jsonl"

[execution]
limit_offset_bps = 10
quote_staleness_threshold_sec = 30
order_timeout_secs = 60
order_interval_ms = 1000
max_orders_per_run = 100

[risk]
max_position_pct = 0.25
max_leverage = 1.0
min_trade_usd = 100.0

[cost]
commission_per_share = 0.005
commission_min = 1.0
slippage_bps = 5
"#,
            toml::Value::String(dir.path().display().to_string())
        ),
    )
    .unwrap();

    // Create a minimal target
    std::fs::write(
        &target_path,
        r#"
{
    "timestamp": "2026-02-08T15:30:00Z",
    "targets": [
        {"symbol": "AAPL", "weight": 0.5}
    ]
}
"#,
    )
    .unwrap();

    // Simulate a crash at order_submitted checkpoint
    {
        let mut log = AuditLog::open_in(&audit_path, workdir).unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::RunStarted,
            1,
            serde_json::json!({"target": "test"}),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::PositionsFetched,
            2,
            serde_json::json!({
                "positions": [{
                    "symbol": "AAPL",
                    "qty": 100,
                    "avg_cost": 150.0
                }]
            }),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::DiffComputed,
            3,
            serde_json::json!({
                "orders": [{
                    "symbol": "AAPL",
                    "action": "Buy",
                    "shares": 50,
                    "limit": 160.0,
                    "description": "test"
                }]
            }),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::OrderSubmitted,
            4,
            serde_json::json!({
                "symbol": "AAPL",
                "action": "Buy",
                "ibkr_id": 12345
            }),
        )
        .unwrap();
    }

    // Create MockBroker with an orphan order
    let mut broker = MockBroker::builder()
        .fill_mode(FillMode::ImmediatePartial(0.5))
        .with_position(Symbol::new("AAPL"), 100, 150_00)
        .build();
    broker.connect().unwrap();

    // Submit an order to create an open order in the broker (orphan)
    let order = BrokerOrder {
        symbol: Symbol::new("AAPL"),
        side: BrokerSide::Buy,
        quantity: 25,
        order_type: BrokerOrderType::Market,
        client_order_id: None,
    };
    broker.submit_order(&order).unwrap();

    // Load config and target
    let config = Config::load(&config_path).unwrap();
    let spec = TargetSpec::load(&target_path).unwrap();

    // Run recovery with broker
    let result = run_recover(&config, &spec, true, Some(&broker as &dyn Broker));

    // Recovery should return error for ManualReview action (expected)
    // The important part is that broker state comparison was performed
    assert!(result.is_err());
}

#[cfg(feature = "write_ahead_logging")]
#[test]
fn test_broker_reconciliation_incomplete_intent_found() {
    use nanobook_rebalancer::recovery::reconcile_incomplete_intents;
    use std::fs;

    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let workdir = dir.path();

    // Simulate a crash at order_intent checkpoint (incomplete intent)
    {
        let mut log = AuditLog::open_in(&audit_path, workdir).unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::RunStarted,
            1,
            serde_json::json!({"target": "test"}),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::OrderIntent,
            2,
            serde_json::json!({
                "symbol": "AAPL",
                "action": "Buy",
                "shares": 50,
                "limit": 160.0,
                "client_order_id": "test_client_123",
            }),
        )
        .unwrap();
    }

    // 50 shares open, none filled. The mock fill mode is based on 100 shares.
    let mut broker = MockBroker::builder()
        .with_position(Symbol::new("AAPL"), 100, 150_00)
        .build();
    broker.connect().unwrap();
    broker.set_open_order_statuses(vec![nanobook_broker::types::BrokerOrderStatus {
        id: nanobook_broker::types::OrderId(1),
        status: nanobook_broker::types::OrderState::Submitted,
        filled_quantity: 0,
        remaining_quantity: 50,
        avg_fill_price_cents: 0,
    }]);

    // Reconstruct state
    let (state, _) = reconstruct_state(&audit_path).unwrap();

    // Run reconciliation (this should find the order and append OrderSubmitted)
    let result = reconcile_incomplete_intents(&broker, &state, &audit_path);

    // Reconciliation should succeed
    assert!(result.is_ok());

    // Verify audit log was updated with OrderSubmitted event
    let audit_contents = fs::read_to_string(&audit_path).unwrap();
    assert!(audit_contents.contains("order_submitted"));
    assert!(audit_contents.contains("reconciled"));
}

#[cfg(feature = "write_ahead_logging")]
#[test]
fn test_broker_reconciliation_incomplete_intent_not_found() {
    use nanobook_rebalancer::recovery::reconcile_incomplete_intents;
    use std::fs;

    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let workdir = dir.path();

    // Simulate a crash at order_intent checkpoint (incomplete intent)
    {
        let mut log = AuditLog::open_in(&audit_path, workdir).unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::RunStarted,
            1,
            serde_json::json!({"target": "test"}),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::OrderIntent,
            2,
            serde_json::json!({
                "symbol": "AAPL",
                "action": "Buy",
                "shares": 50,
                "limit": 160.0,
                "client_order_id": "test_client_123",
            }),
        )
        .unwrap();
    }

    // Create MockBroker without the order (order was never submitted)
    let mut broker = MockBroker::builder()
        .fill_mode(FillMode::ImmediateFull)
        .with_position(Symbol::new("AAPL"), 100, 150_00)
        .build();
    broker.connect().unwrap();

    // Reconstruct state
    let (state, _) = reconstruct_state(&audit_path).unwrap();

    // Run reconciliation (this should not find the order and append OrderFailed)
    let result = reconcile_incomplete_intents(&broker, &state, &audit_path);

    // Reconciliation should succeed
    if let Err(e) = result {
        panic!("Reconciliation failed: {:?}", e);
    }

    // Verify audit log was updated with OrderFailed event
    let audit_contents = fs::read_to_string(&audit_path).unwrap();
    assert!(audit_contents.contains("order_failed"));
    assert!(audit_contents.contains("not_found_at_broker"));
    assert!(audit_contents.contains("reconciled"));
}

#[cfg(feature = "write_ahead_logging")]
#[test]
fn test_broker_reconciliation_mixed_intents() {
    use nanobook_rebalancer::recovery::reconcile_incomplete_intents;

    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let workdir = dir.path();

    // Simulate a crash with mixed intents (some submitted, some not)
    {
        let mut log = AuditLog::open_in(&audit_path, workdir).unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::RunStarted,
            1,
            serde_json::json!({"target": "test"}),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::OrderIntent,
            2,
            serde_json::json!({
                "symbol": "AAPL",
                "action": "Buy",
                "shares": 50,
                "limit": 160.0,
                "client_order_id": "client_aapl_123",
            }),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::OrderIntent,
            3,
            serde_json::json!({
                "symbol": "MSFT",
                "action": "Sell",
                "shares": 25,
                "limit": 310.0,
                "client_order_id": "client_msft_456",
            }),
        )
        .unwrap();
        // AAPL was submitted, MSFT was not
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::OrderSubmitted,
            4,
            serde_json::json!({
                "symbol": "AAPL",
                "ibkr_id": 54321,
            }),
        )
        .unwrap();
    }

    // Create MockBroker with only the AAPL order (no orders for MSFT): a
    // 50-share order, 25 filled and 25 still open.
    let mut broker = MockBroker::builder()
        .with_position(Symbol::new("AAPL"), 100, 150_00)
        .with_position(Symbol::new("MSFT"), 50, 300_00)
        .build();
    broker.connect().unwrap();
    broker.set_open_order_statuses(vec![nanobook_broker::types::BrokerOrderStatus {
        id: nanobook_broker::types::OrderId(1),
        status: nanobook_broker::types::OrderState::PartiallyFilled,
        filled_quantity: 25,
        remaining_quantity: 25,
        avg_fill_price_cents: 0,
    }]);

    // Reconstruct state
    let (state, _) = reconstruct_state(&audit_path).unwrap();

    // Run reconciliation
    let result = reconcile_incomplete_intents(&broker, &state, &audit_path);

    // Reconciliation should succeed (only MSFT is incomplete)
    assert!(result.is_ok());

    // MSFT (25 shares) is incomplete and no open order started at 25 shares,
    // so it is marked failed. The 25 filled plus 25 open order is a 50-share order.
    let events = nanobook_rebalancer::audit::parse_audit_events(&audit_path).unwrap();
    let last = events.last().unwrap();
    assert_eq!(last.event, "order_failed");
    assert_eq!(
        last.data.get("symbol").and_then(|v| v.as_str()),
        Some("MSFT")
    );
    assert_eq!(
        last.data.get("reason").and_then(|v| v.as_str()),
        Some("not_found_at_broker")
    );
}

#[cfg(feature = "write_ahead_logging")]
#[test]
fn test_recovery_with_golden_fixture_intent_only() {
    use std::path::PathBuf;

    // Use the golden fixture for intent-only scenario
    let fixture_path = PathBuf::from("tests/fixtures/recovery_intent_only.jsonl");

    // Reconstruct state from fixture
    let (state, action) = reconstruct_state(&fixture_path).unwrap();

    // Verify state
    assert_eq!(
        state.checkpoint,
        nanobook_rebalancer::audit::Checkpoint::OrderIntent
    );
    assert_eq!(state.orders.len(), 1);
    assert_eq!(
        state.orders[0].client_order_id,
        Some("client-123".to_string())
    );
    assert!(!state.orders[0].submitted);
    assert!(!state.orders[0].failed);

    // Verify recovery action
    #[cfg(feature = "write_ahead_logging")]
    assert_eq!(action, RecoveryAction::Resume);
    #[cfg(not(feature = "write_ahead_logging"))]
    assert_eq!(action, RecoveryAction::ManualReview);
}

#[cfg(feature = "write_ahead_logging")]
#[test]
fn test_recovery_with_golden_fixture_resolved_success() {
    use std::path::PathBuf;

    // Use the golden fixture for resolved success scenario
    let fixture_path = PathBuf::from("tests/fixtures/recovery_intent_resolved_success.jsonl");

    // Reconstruct state from fixture
    let (state, _action) = reconstruct_state(&fixture_path).unwrap();

    // Verify state
    assert_eq!(
        state.checkpoint,
        nanobook_rebalancer::audit::Checkpoint::OrderSubmitted
    );
    assert_eq!(state.orders.len(), 1);
    assert!(state.orders[0].submitted);
    assert_eq!(state.orders[0].ibkr_id, 54321);
}

#[cfg(feature = "write_ahead_logging")]
#[test]
fn test_recovery_with_golden_fixture_resolved_failure() {
    use std::path::PathBuf;

    // Use the golden fixture for resolved failure scenario
    let fixture_path = PathBuf::from("tests/fixtures/recovery_intent_resolved_failure.jsonl");

    // Reconstruct state from fixture
    let (state, action) = reconstruct_state(&fixture_path).unwrap();

    // Verify state
    assert_eq!(
        state.checkpoint,
        nanobook_rebalancer::audit::Checkpoint::OrderFailed
    );
    assert_eq!(state.orders.len(), 1);
    assert!(state.orders[0].failed);
    assert_eq!(
        state.orders[0].failure_reason,
        Some("not_found_at_broker".to_string())
    );

    // Should be safe to restart since order failed
    assert_eq!(action, RecoveryAction::Restart);
}

#[cfg(feature = "write_ahead_logging")]
#[test]
fn test_recovery_with_golden_fixture_mixed_intents() {
    use std::path::PathBuf;

    // Use the golden fixture for mixed intents scenario
    let fixture_path = PathBuf::from("tests/fixtures/recovery_mixed_intents.jsonl");

    // Reconstruct state from fixture
    let (state, action) = reconstruct_state(&fixture_path).unwrap();

    // Verify state
    assert_eq!(state.orders.len(), 2);
    assert!(state.orders[0].submitted); // AAPL was submitted
    assert!(state.orders[1].failed); // MSFT failed

    // Should require manual review due to unfilled submitted order
    assert_eq!(action, RecoveryAction::ManualReview);
}

#[cfg(feature = "write_ahead_logging")]
fn log_run_started_and_intents(audit_path: &std::path::Path, workdir: &std::path::Path) {
    let mut log = AuditLog::open_in(audit_path, workdir).unwrap();
    log.log_checkpoint(
        nanobook_rebalancer::audit::Checkpoint::RunStarted,
        1,
        serde_json::json!({"target": "test"}),
    )
    .unwrap();
    log.log_checkpoint(
        nanobook_rebalancer::audit::Checkpoint::OrderIntent,
        2,
        serde_json::json!({
            "symbol": "AAPL",
            "action": "Buy",
            "shares": 50,
            "limit": 160.0,
            "client_order_id": "a",
        }),
    )
    .unwrap();
    log.log_checkpoint(
        nanobook_rebalancer::audit::Checkpoint::OrderIntent,
        3,
        serde_json::json!({
            "symbol": "MSFT",
            "action": "Buy",
            "shares": 25,
            "limit": 310.0,
            "client_order_id": "b",
        }),
    )
    .unwrap();
}

#[cfg(feature = "write_ahead_logging")]
#[test]
fn reconcile_skips_already_submitted_intents() {
    use nanobook_rebalancer::recovery::reconcile_incomplete_intents;

    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    {
        let mut log = AuditLog::open_in(&audit_path, dir.path()).unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::RunStarted,
            1,
            serde_json::json!({"target": "test"}),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::DiffComputed,
            2,
            serde_json::json!({
                "orders": [
                    {"symbol": "AAPL", "action": "Buy", "shares": 50, "limit": 160.0},
                    {"symbol": "MSFT", "action": "Buy", "shares": 25, "limit": 310.0}
                ]
            }),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::OrderIntent,
            3,
            serde_json::json!({
                "symbol": "AAPL",
                "action": "Buy",
                "shares": 50,
                "limit": 160.0,
                "client_order_id": "a",
            }),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::OrderSubmitted,
            4,
            serde_json::json!({
                "symbol": "AAPL",
                "ibkr_id": 111,
            }),
        )
        .unwrap();
    }
    let mut broker = MockBroker::builder().build();
    broker.connect().unwrap();
    let (state, _) = reconstruct_state(&audit_path).unwrap();
    reconcile_incomplete_intents(&broker, &state, &audit_path).unwrap();
    assert_eq!(parse_audit_events(&audit_path).unwrap().len(), 4);
}

#[cfg(feature = "write_ahead_logging")]
#[test]
fn reconcile_appends_submitted_and_failed_in_sequence() {
    use nanobook_rebalancer::recovery::reconcile_incomplete_intents;

    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    log_run_started_and_intents(&audit_path, dir.path());
    let mut broker = MockBroker::builder().build();
    broker.connect().unwrap();
    // AAPL's 50 shares are still open. MSFT's 25 shares are not.
    broker.set_open_order_statuses(vec![nanobook_broker::types::BrokerOrderStatus {
        id: nanobook_broker::types::OrderId(1),
        status: nanobook_broker::types::OrderState::Submitted,
        filled_quantity: 0,
        remaining_quantity: 50,
        avg_fill_price_cents: 16_000,
    }]);
    let (state, _) = reconstruct_state(&audit_path).unwrap();
    reconcile_incomplete_intents(&broker, &state, &audit_path).unwrap();
    let events = parse_audit_events(&audit_path).unwrap();
    assert_eq!(
        events[3..]
            .iter()
            .map(|e| e.sequence_number.unwrap())
            .collect::<Vec<_>>(),
        vec![4, 5]
    );
    assert_eq!(events[3].event, "order_submitted");
    assert_eq!(
        events[3].data.get("symbol").and_then(|v| v.as_str()),
        Some("AAPL")
    );
    assert_eq!(events[4].event, "order_failed");
    assert_eq!(
        events[4].data.get("symbol").and_then(|v| v.as_str()),
        Some("MSFT")
    );
}

#[cfg(feature = "write_ahead_logging")]
#[test]
fn reconcile_appends_failed_when_broker_has_no_open_orders() {
    use nanobook_rebalancer::recovery::reconcile_incomplete_intents;

    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    log_run_started_and_intents(&audit_path, dir.path());
    let mut broker = MockBroker::builder()
        .fill_mode(FillMode::ImmediateFull)
        .build();
    broker.connect().unwrap();
    let (state, _) = reconstruct_state(&audit_path).unwrap();
    reconcile_incomplete_intents(&broker, &state, &audit_path).unwrap();
    let events = parse_audit_events(&audit_path).unwrap();
    assert_eq!(
        events[3..]
            .iter()
            .map(|e| e.sequence_number.unwrap())
            .collect::<Vec<_>>(),
        vec![4, 5]
    );
    assert_eq!(events[3].event, "order_failed");
    assert_eq!(events[4].event, "order_failed");
}

#[cfg(feature = "write_ahead_logging")]
struct FailOpenBroker {
    calls: std::cell::Cell<usize>,
}

#[cfg(feature = "write_ahead_logging")]
impl Broker for FailOpenBroker {
    fn connect(&mut self) -> std::result::Result<(), nanobook_broker::error::BrokerError> {
        Ok(())
    }
    fn disconnect(&mut self) -> std::result::Result<(), nanobook_broker::error::BrokerError> {
        Ok(())
    }
    fn positions(
        &self,
    ) -> std::result::Result<
        Vec<nanobook_broker::types::Position>,
        nanobook_broker::error::BrokerError,
    > {
        Ok(Vec::new())
    }
    fn account(
        &self,
    ) -> std::result::Result<nanobook_broker::types::Account, nanobook_broker::error::BrokerError>
    {
        Ok(nanobook_broker::types::Account {
            equity_cents: 0,
            buying_power_cents: 0,
            cash_cents: 0,
            gross_position_value_cents: 0,
        })
    }
    fn submit_order(
        &self,
        _order: &BrokerOrder,
    ) -> std::result::Result<nanobook_broker::types::OrderId, nanobook_broker::error::BrokerError>
    {
        Ok(nanobook_broker::types::OrderId(1))
    }
    fn order_status(
        &self,
        id: nanobook_broker::types::OrderId,
    ) -> std::result::Result<
        nanobook_broker::types::BrokerOrderStatus,
        nanobook_broker::error::BrokerError,
    > {
        Ok(nanobook_broker::types::BrokerOrderStatus {
            id,
            status: nanobook_broker::types::OrderState::Submitted,
            filled_quantity: 0,
            remaining_quantity: 1,
            avg_fill_price_cents: 0,
        })
    }
    fn open_orders(
        &self,
    ) -> std::result::Result<
        Vec<nanobook_broker::types::BrokerOrderStatus>,
        nanobook_broker::error::BrokerError,
    > {
        self.calls.set(self.calls.get() + 1);
        Err(nanobook_broker::error::BrokerError::Connection(
            "open orders unavailable".into(),
        ))
    }
    fn cancel_order(
        &self,
        _id: nanobook_broker::types::OrderId,
    ) -> std::result::Result<(), nanobook_broker::error::BrokerError> {
        Ok(())
    }
    fn quote(
        &self,
        symbol: &Symbol,
    ) -> std::result::Result<nanobook_broker::types::Quote, nanobook_broker::error::BrokerError>
    {
        Ok(nanobook_broker::types::Quote {
            symbol: *symbol,
            bid_cents: 0,
            ask_cents: 0,
            last_cents: 0,
            volume: 0,
            timestamp: std::time::SystemTime::now(),
        })
    }
}

#[cfg(feature = "write_ahead_logging")]
#[test]
fn reconcile_incomplete_intents_counts_broker_failures() {
    use nanobook_rebalancer::recovery::reconcile_incomplete_intents;

    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    {
        let mut log = AuditLog::open_in(&audit_path, dir.path()).unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::RunStarted,
            1,
            serde_json::json!({"target": "test"}),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::OrderIntent,
            2,
            serde_json::json!({
                "symbol": "AAPL",
                "action": "Buy",
                "shares": 50,
                "limit": 160.0,
                "client_order_id": "a",
            }),
        )
        .unwrap();
    }
    let broker = FailOpenBroker {
        calls: std::cell::Cell::new(0),
    };
    let (state, _) = reconstruct_state(&audit_path).unwrap();
    match reconcile_incomplete_intents(&broker, &state, &audit_path) {
        Err(Error::Recovery(m)) => {
            assert!(m.contains("Failed to reconcile 1 incomplete intents"));
        }
        other => panic!("expected Recovery, got {other:?}"),
    }
    assert_eq!(parse_audit_events(&audit_path).unwrap().len(), 2);
    assert_eq!(broker.calls.get(), 5);
}

#[cfg(feature = "write_ahead_logging")]
#[test]
fn run_recover_reconciles_incomplete_intent() {
    let dir = tempfile::tempdir().unwrap();
    let config: Config = toml::from_str(&format!(
        r#"
[connection]
host = "127.0.0.1"
port = 4002
client_id = 1
[account]
id = "DU1"
type = "cash"
[execution]
[risk]
[cost]
[logging]
dir = {}
audit_file = "audit.jsonl"
"#,
        toml::Value::String(dir.path().display().to_string())
    ))
    .unwrap();
    let spec = TargetSpec::from_json(
        r#"{
            "timestamp": "2026-02-08T15:30:00Z",
            "targets": [{ "symbol": "AAPL", "weight": 0.5 }]
        }"#,
    )
    .unwrap();
    {
        let mut log = AuditLog::open_in(&config.audit_path(), dir.path()).unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::RunStarted,
            1,
            serde_json::json!({"target": "test"}),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::OrderIntent,
            2,
            serde_json::json!({
                "symbol": "AAPL",
                "action": "Buy",
                "shares": 50,
                "limit": 160.0,
                "client_order_id": "c",
            }),
        )
        .unwrap();
    }
    let mut broker = MockBroker::builder().build();
    broker.connect().unwrap();
    run_recover(&config, &spec, false, Some(&broker)).unwrap();
    let events = parse_audit_events(&config.audit_path()).unwrap();
    assert_eq!(events.len(), 3);
    let last = events.last().unwrap();
    assert_eq!(last.event, "order_failed");
    assert_eq!(
        last.data.get("reason").and_then(|v| v.as_str()),
        Some("not_found_at_broker")
    );
    assert_eq!(
        last.data.get("reconciled").and_then(|v| v.as_bool()),
        Some(true)
    );
}

#[cfg(feature = "write_ahead_logging")]
#[test]
fn run_recover_dry_run_does_not_write_the_audit() {
    let dir = tempfile::tempdir().unwrap();
    let config: Config = toml::from_str(&format!(
        r#"
[connection]
host = "127.0.0.1"
port = 4002
client_id = 1
[account]
id = "DU1"
type = "cash"
[execution]
[risk]
[cost]
[logging]
dir = {}
audit_file = "audit.jsonl"
"#,
        toml::Value::String(dir.path().display().to_string())
    ))
    .unwrap();
    let spec = TargetSpec::from_json(
        r#"{
            "timestamp": "2026-02-08T15:30:00Z",
            "targets": [{ "symbol": "AAPL", "weight": 0.5 }]
        }"#,
    )
    .unwrap();
    {
        let mut log = AuditLog::open_in(&config.audit_path(), dir.path()).unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::RunStarted,
            1,
            serde_json::json!({"target": "test"}),
        )
        .unwrap();
        log.log_checkpoint(
            nanobook_rebalancer::audit::Checkpoint::OrderIntent,
            2,
            serde_json::json!({
                "symbol": "AAPL",
                "action": "Buy",
                "shares": 50,
                "limit": 160.0,
                "client_order_id": "c",
            }),
        )
        .unwrap();
    }
    let before = std::fs::read(config.audit_path()).unwrap();
    let mut broker = MockBroker::builder().build();
    broker.connect().unwrap();
    run_recover(&config, &spec, true, Some(&broker)).unwrap();
    assert_eq!(std::fs::read(config.audit_path()).unwrap(), before);
}
