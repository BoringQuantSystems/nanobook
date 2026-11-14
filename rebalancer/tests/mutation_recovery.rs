//! Mutation-gap tests for write-ahead recovery: reconciliation counters and
//! the cancel-intent boundary of `determine_recovery_action`.

#![cfg(feature = "write_ahead_logging")]

mod common;

use common::LogCapture;
use nanobook::Symbol;
use nanobook_broker::Broker;
use nanobook_broker::mock::{FillMode, MockBroker};
use nanobook_broker::{BrokerOrder, BrokerOrderType, BrokerSide};
use nanobook_rebalancer::audit::{AuditLog, Checkpoint, parse_audit_events};
use nanobook_rebalancer::recovery::{
    RecoveredState, RecoveryAction, reconcile_incomplete_intents, reconstruct_state,
};

/// run_started, then one 50-share order intent for AAPL and one for MSFT.
fn write_two_incomplete_intents(path: &std::path::Path, workdir: &std::path::Path) {
    let mut log = AuditLog::open_in(path, workdir).unwrap();
    log.log_checkpoint(Checkpoint::RunStarted, 1, serde_json::json!({"t": 1}))
        .unwrap();
    for (sequence, symbol) in [(2, "AAPL"), (3, "MSFT")] {
        log.log_checkpoint(
            Checkpoint::OrderIntent,
            sequence,
            serde_json::json!({
                "symbol": symbol,
                "action": "Buy",
                "shares": 50,
                "limit": 160.0,
                "client_order_id": format!("c-{symbol}"),
            }),
        )
        .unwrap();
    }
}

#[test]
fn reconciliation_summary_counts_intents_found_at_the_broker() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.jsonl");
    write_two_incomplete_intents(&path, dir.path());

    // The broker holds one open order with 50 shares remaining.
    let mut broker = MockBroker::builder()
        .fill_mode(FillMode::ImmediatePartial(0.5))
        .build();
    broker.connect().unwrap();
    broker
        .submit_order(&BrokerOrder {
            symbol: Symbol::new("AAPL"),
            side: BrokerSide::Buy,
            quantity: 50,
            order_type: BrokerOrderType::Market,
            client_order_id: None,
        })
        .unwrap();

    let (state, _) = reconstruct_state(&path).unwrap();
    let capture = LogCapture::default();
    capture
        .run(|| reconcile_incomplete_intents(&broker, &state, &path))
        .unwrap();

    let events = parse_audit_events(&path).unwrap();
    assert_eq!(events.len(), 5);
    assert_eq!(events[3].event, "order_submitted");
    assert_eq!(events[3].sequence_number, Some(4));
    assert_eq!(events[4].event, "order_submitted");
    assert_eq!(events[4].sequence_number, Some(5));
    let text = capture.text();
    assert!(
        text.contains("Broker reconciliation complete: 2 reconciled, 0 failed"),
        "log was: {text}"
    );
}

#[test]
fn reconciliation_summary_counts_intents_missing_at_the_broker() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.jsonl");
    write_two_incomplete_intents(&path, dir.path());

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
