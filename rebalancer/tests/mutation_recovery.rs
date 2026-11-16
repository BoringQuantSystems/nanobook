//! Mutation-gap tests for write-ahead recovery: reconciliation counters and
//! the cancel-intent boundary of `determine_recovery_action`.

#![cfg(feature = "write_ahead_logging")]

mod common;

use common::LogCapture;
use nanobook_broker::Broker;
use nanobook_broker::mock::{FillMode, MockBroker};
use nanobook_rebalancer::audit::{AuditLog, Checkpoint, parse_audit_events};
use nanobook_rebalancer::error::Error;
use nanobook_rebalancer::recovery::{
    RecoveredState, RecoveryAction, reconcile_incomplete_intents, reconstruct_state,
};

/// run_started, then one order intent for AAPL and one for MSFT.
fn write_two_incomplete_intents(
    path: &std::path::Path,
    workdir: &std::path::Path,
    aapl_shares: i64,
    msft_shares: i64,
) {
    let mut log = AuditLog::open_in(path, workdir).unwrap();
    log.log_checkpoint(Checkpoint::RunStarted, 1, serde_json::json!({"t": 1}))
        .unwrap();
    for (sequence, symbol, shares) in [(2, "AAPL", aapl_shares), (3, "MSFT", msft_shares)] {
        log.log_checkpoint(
            Checkpoint::OrderIntent,
            sequence,
            serde_json::json!({
                "symbol": symbol,
                "action": "Buy",
                "shares": shares,
                "limit": 160.0,
                "client_order_id": format!("c-{symbol}"),
            }),
        )
        .unwrap();
    }
}

fn open_status(id: u64, remaining: u64) -> nanobook_broker::types::BrokerOrderStatus {
    nanobook_broker::types::BrokerOrderStatus {
        id: nanobook_broker::types::OrderId(id),
        status: nanobook_broker::types::OrderState::Submitted,
        filled_quantity: 0,
        remaining_quantity: remaining,
        avg_fill_price_cents: 0,
    }
}

#[test]
fn one_open_order_does_not_submit_two_intents_of_the_same_quantity() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.jsonl");
    write_two_incomplete_intents(&path, dir.path(), 50, 50);

    // One live 50-share order, nothing filled. Both intents ask for 50.
    // The status has no symbol, so the order cannot be given to either.
    let mut broker = MockBroker::builder().build();
    broker.connect().unwrap();
    broker.set_open_order_statuses(vec![open_status(7, 50)]);

    let (state, _) = reconstruct_state(&path).unwrap();
    let err = reconcile_incomplete_intents(&broker, &state, &path).unwrap_err();
    match err {
        Error::Recovery(message) => {
            assert!(
                message.contains("Failed to reconcile 2 incomplete intents"),
                "{message}"
            );
        }
        other => panic!("expected Recovery, got {other:?}"),
    }
    let events = parse_audit_events(&path).unwrap();
    assert_eq!(events.len(), 3);
    assert!(events.iter().all(|event| event.event != "order_submitted"));
    assert!(events.iter().all(|event| event.event != "order_failed"));
}

#[test]
fn two_open_orders_of_one_quantity_match_neither_intent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.jsonl");
    write_two_incomplete_intents(&path, dir.path(), 50, 50);

    let mut broker = MockBroker::builder().build();
    broker.connect().unwrap();
    broker.set_open_order_statuses(vec![open_status(7, 50), open_status(8, 50)]);

    let (state, _) = reconstruct_state(&path).unwrap();
    let err = reconcile_incomplete_intents(&broker, &state, &path).unwrap_err();
    match err {
        Error::Recovery(message) => {
            assert!(
                message.contains("Failed to reconcile 2 incomplete intents"),
                "{message}"
            );
        }
        other => panic!("expected Recovery, got {other:?}"),
    }
    let events = parse_audit_events(&path).unwrap();
    assert_eq!(events.len(), 3);
    assert!(events.iter().all(|event| event.event != "order_submitted"));
    assert!(events.iter().all(|event| event.event != "order_failed"));
}

#[test]
fn two_open_orders_of_different_quantities_submit_both_intents() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.jsonl");
    write_two_incomplete_intents(&path, dir.path(), 50, 25);

    let mut broker = MockBroker::builder().build();
    broker.connect().unwrap();
    broker.set_open_order_statuses(vec![open_status(7, 50), open_status(8, 25)]);

    let (state, _) = reconstruct_state(&path).unwrap();
    reconcile_incomplete_intents(&broker, &state, &path).unwrap();
    let events = parse_audit_events(&path).unwrap();
    assert_eq!(events.len(), 5);
    assert_eq!(events[3].event, "order_submitted");
    assert_eq!(
        events[3].data.get("symbol").and_then(|v| v.as_str()),
        Some("AAPL")
    );
    assert_eq!(
        events[3].data.get("ibkr_id").and_then(|v| v.as_u64()),
        Some(7)
    );
    assert_eq!(events[4].event, "order_submitted");
    assert_eq!(
        events[4].data.get("symbol").and_then(|v| v.as_str()),
        Some("MSFT")
    );
    assert_eq!(
        events[4].data.get("ibkr_id").and_then(|v| v.as_u64()),
        Some(8)
    );
}

#[test]
fn reconciliation_summary_counts_intents_missing_at_the_broker() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.jsonl");
    write_two_incomplete_intents(&path, dir.path(), 50, 50);

    let mut broker = MockBroker::builder()
        .fill_mode(FillMode::ImmediateFull)
        .build();
    broker.connect().unwrap();

    let (state, _) = reconstruct_state(&path).unwrap();
    let capture = LogCapture::default();
    capture
        .run(|| reconcile_incomplete_intents(&broker, &state, &path))
        .unwrap();

    let events = parse_audit_events(&path).unwrap();
    assert_eq!(events.len(), 5);
    assert_eq!(events[3].event, "order_failed");
    assert_eq!(events[4].event, "order_failed");
    let text = capture.text();
    assert!(
        text.contains("Broker reconciliation complete: 2 reconciled, 0 failed"),
        "log was: {text}"
    );
}

#[test]
fn cancel_result_with_the_intent_sequence_completes_the_cancel() {
    let state_with = |intent: u64, result: Option<u64>| {
        let mut state = RecoveredState::new();
        state.last_cancel_intent_sequence = Some(intent);
        state.last_cancel_result_sequence = result;
        state
    };

    // A result older than the intent, or none, leaves the cancel incomplete.
    assert_eq!(
        state_with(5, None).determine_recovery_action(),
        RecoveryAction::ManualReview
    );
    assert_eq!(
        state_with(5, Some(4)).determine_recovery_action(),
        RecoveryAction::ManualReview
    );
    // A result with the same sequence number as the intent is not older.
    assert_eq!(
        state_with(5, Some(5)).determine_recovery_action(),
        RecoveryAction::Restart
    );
    assert_eq!(
        state_with(5, Some(6)).determine_recovery_action(),
        RecoveryAction::Restart
    );
}
