//! Mutation-gap tests for audit.rs: checkpoint-order warnings and the
//! `log_*` convenience writers.

mod common;

use common::LogCapture;
use nanobook::Symbol;
use nanobook_rebalancer::audit::{
    AuditLog, parse_audit_events, parse_audit_events_from_str, validate_checkpoints_from_parsed,
};
use nanobook_rebalancer::diff::{Action, RebalanceOrder};

/// Build a parsed audit log from event names. Event `i` gets sequence number
/// `i + 1`. `diff_computed` carries one order so the later checkpoints are
/// required.
fn events(names: &[&str]) -> Vec<nanobook_rebalancer::audit::AuditEvent> {
    let mut text = String::new();
    for (i, name) in names.iter().enumerate() {
        let extra = if *name == "diff_computed" {
            r#","orders":[{"symbol":"AAPL","action":"BUY","shares":1,"limit":1.0,"description":"open"}]"#
        } else {
            ""
        };
        text.push_str(&format!(
            r#"{{"event":"{name}","ts":"2026-01-01T00:00:00Z","sequence_number":{}{extra}}}"#,
            i + 1
        ));
        text.push('\n');
    }
    parse_audit_events_from_str(&text).unwrap()
}

#[cfg(feature = "write_ahead_logging")]
const POSITIONS: &[&str] = &["positions_intent", "positions_result"];
#[cfg(not(feature = "write_ahead_logging"))]
const POSITIONS: &[&str] = &["positions_fetched"];

/// run_started, positions, diff_computed, risk_check_passed, then `tail`.
fn run_with_tail(tail: &[&str]) -> Vec<nanobook_rebalancer::audit::AuditEvent> {
    let mut names = vec!["run_started"];
    names.extend_from_slice(POSITIONS);
    names.extend_from_slice(&["diff_computed", "risk_check_passed"]);
    names.extend_from_slice(tail);
    events(&names)
}

fn validate_and_capture(log: &[nanobook_rebalancer::audit::AuditEvent]) -> String {
    let capture = LogCapture::default();
    capture
        .run(|| validate_checkpoints_from_parsed(log))
        .unwrap();
    capture.text()
}

const ORDER_WARNING: &str = "Found OrderIntent checkpoint without OrderSubmitted or OrderFailed";

#[test]
fn order_intent_without_followup_warns_even_when_submissions_precede_it() {
    // An order_submitted directly before the intent, and none after it.
    // The warning must look only at events after the first order_intent.
    let text = validate_and_capture(&run_with_tail(&["order_submitted", "order_intent"]));
    assert!(text.contains(ORDER_WARNING), "log was: {text}");

    // The plain case: an intent and nothing after it.
    let text = validate_and_capture(&run_with_tail(&["order_intent"]));
    assert!(text.contains(ORDER_WARNING), "log was: {text}");
}

#[test]
fn order_intent_with_submitted_or_failed_followup_does_not_warn() {
    for followup in ["order_submitted", "order_failed"] {
        let text = validate_and_capture(&run_with_tail(&["order_intent", followup]));
        assert!(
            !text.contains(ORDER_WARNING),
            "{followup} after the intent still warned: {text}"
        );
    }
}

#[cfg(feature = "write_ahead_logging")]
mod wal_checkpoints {
    use super::*;

    /// A write-ahead log in which every intent has its result.
    fn complete_log() -> Vec<nanobook_rebalancer::audit::AuditEvent> {
        events(&[
            "run_started",
            "account_summary_intent",
            "account_summary_result",
            "positions_intent",
            "positions_result",
            "quotes_intent",
            "quotes_result",
            "diff_computed",
            "risk_check_passed",
            "order_intent",
            "order_submitted",
            "cancel_intent",
            "cancel_result",
            "order_filled",
            "run_completed",
        ])
    }

    #[test]
    fn complete_intent_result_pairs_do_not_warn() {
        let text = validate_and_capture(&complete_log());
        for message in [
            "without PositionsResult",
            "without QuotesResult",
            "without AccountSummaryResult",
            "without CancelResult",
            "without OrderSubmitted or OrderFailed",
        ] {
            assert!(!text.contains(message), "unexpected {message:?} in: {text}");
        }
    }

    #[test]
    fn logs_without_quotes_account_or_cancel_events_do_not_warn_about_them() {
        let log = events(&[
            "run_started",
            "positions_intent",
            "positions_result",
            "diff_computed",
            "risk_check_passed",
            "order_intent",
            "order_submitted",
        ]);
        let text = validate_and_capture(&log);
        for message in [
            "without PositionsResult",
            "without QuotesResult",
            "without AccountSummaryResult",
            "without CancelResult",
            "without OrderSubmitted or OrderFailed",
        ] {
            assert!(!text.contains(message), "unexpected {message:?} in: {text}");
        }
    }

    #[test]
    fn incomplete_quotes_account_and_cancel_intents_warn() {
        let log = events(&[
            "run_started",
            "account_summary_intent",
            "positions_intent",
            "positions_result",
            "quotes_intent",
            "diff_computed",
            "risk_check_passed",
            "order_intent",
            "order_submitted",
            "cancel_intent",
        ]);
        let text = validate_and_capture(&log);
        assert!(
            text.contains("Found QuotesIntent checkpoint without QuotesResult"),
            "log was: {text}"
        );
        assert!(
            text.contains("Found AccountSummaryIntent checkpoint without AccountSummaryResult"),
            "log was: {text}"
        );
        assert!(
            text.contains("Found CancelIntent checkpoint without CancelResult"),
            "log was: {text}"
        );
        assert!(!text.contains("without PositionsResult"), "log was: {text}");
        assert!(
            !text.contains("without OrderSubmitted or OrderFailed"),
            "log was: {text}"
        );
    }
}

fn buy_order() -> RebalanceOrder {
    RebalanceOrder {
        symbol: Symbol::new("AAPL"),
        action: Action::Buy,
        shares: 10,
        limit_price_cents: 18_502,
        notional_cents: 185_020,
        description: "open",
    }
}

#[test]
fn log_order_submitted_writes_the_event_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.jsonl");
    let mut audit = AuditLog::open_in(&path, dir.path()).unwrap();

    nanobook_rebalancer::audit::log_order_submitted(&mut audit, &buy_order(), 4242).unwrap();

    let events = parse_audit_events(&path).unwrap();
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.event, "order_submitted");
    assert_eq!(event.checkpoint, None);
    assert_eq!(event.data["symbol"], "AAPL");
    assert_eq!(event.data["action"], "BUY");
    assert_eq!(event.data["shares"], 10);
    assert_eq!(event.data["limit"], 185.02);
    assert_eq!(event.data["ibkr_id"], 4242);
}

#[cfg(feature = "write_ahead_logging")]
#[test]
fn account_summary_and_cancel_writers_write_the_event_fields() {
    use chrono::{TimeZone, Utc};
    use nanobook_rebalancer::audit::{
        log_account_summary_intent, log_account_summary_result, log_cancel_intent,
        log_cancel_result,
    };

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.jsonl");
    let mut audit = AuditLog::open_in(&path, dir.path()).unwrap();
    let when = Utc.with_ymd_and_hms(2026, 3, 4, 5, 6, 7).unwrap();

    log_account_summary_intent(&mut audit, when, "scope-1").unwrap();
    log_account_summary_result(&mut audit, 123_456_789, 98_765).unwrap();
    log_cancel_intent(&mut audit, 77, "operator", when).unwrap();
    log_cancel_result(&mut audit, 77, false, Some("no such order")).unwrap();
    log_cancel_result(&mut audit, 78, true, None).unwrap();

    let events = parse_audit_events(&path).unwrap();
    assert_eq!(
        events.iter().map(|e| e.event.as_str()).collect::<Vec<_>>(),
        [
            "account_summary_intent",
            "account_summary_result",
            "cancel_intent",
            "cancel_result",
            "cancel_result",
        ]
    );
    assert!(events.iter().all(|e| e.checkpoint.is_none()));

    assert_eq!(events[0].data["timestamp"], when.to_rfc3339());
    assert_eq!(events[0].data["target_spec_reference"], "scope-1");

    assert_eq!(events[1].data["equity"], 1_234_567.89);
    assert_eq!(events[1].data["cash"], 987.65);

    assert_eq!(events[2].data["order_id"], 77);
    assert_eq!(events[2].data["cancellation_reason"], "operator");
    assert_eq!(events[2].data["timestamp"], when.to_rfc3339());

    assert_eq!(events[3].data["order_id"], 77);
    assert_eq!(events[3].data["success"], false);
    assert_eq!(events[3].data["error_message"], "no such order");

    assert_eq!(events[4].data["order_id"], 78);
    assert_eq!(events[4].data["success"], true);
    assert!(events[4].data["error_message"].is_null());
}
