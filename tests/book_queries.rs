//! Exact answers of the read-only book queries and the history and policy setters.
//!
//! cargo-mutants (0.20.0 run) showed these could return wrong values without any
//! test noticing: most tests check trades and order status, not the queries
//! themselves. Each test pins one query to an exact value.

#![allow(clippy::inconsistent_digit_grouping)] // prices in cents: 100_00 = 100.00

use nanobook::{Exchange, OrderId, OrderOwner, OrderStatus, Price, Side, StpPolicy, TimeInForce};

/// Bid 100.00 x 10, ask 101.00 x 20 (a second bid at 99.00 x 5 behind it).
fn two_sided() -> Exchange {
    let mut ex = Exchange::new();
    ex.submit_limit(Side::Buy, Price(100_00), 10, TimeInForce::GTC);
    ex.submit_limit(Side::Buy, Price(99_00), 5, TimeInForce::GTC);
    ex.submit_limit(Side::Sell, Price(101_00), 20, TimeInForce::GTC);
    ex
}

#[test]
fn best_bid_ask_and_spread_are_exact() {
    let ex = two_sided();
    assert_eq!(
        ex.best_bid_ask(),
        (Some(Price(100_00)), Some(Price(101_00)))
    );
    assert_eq!(
        ex.book().best_bid_ask(),
        (Some(Price(100_00)), Some(Price(101_00)))
    );
    assert_eq!(ex.spread(), Some(1_00));
}

#[test]
fn one_sided_and_empty_books_report_missing_sides() {
    let mut ex = Exchange::new();
    assert_eq!(ex.best_bid_ask(), (None, None));
    assert_eq!(ex.spread(), None);

    ex.submit_limit(Side::Buy, Price(100_00), 10, TimeInForce::GTC);
    assert_eq!(ex.best_bid_ask(), (Some(Price(100_00)), None));
    assert_eq!(ex.spread(), None);

    let mut ex = Exchange::new();
    ex.submit_limit(Side::Sell, Price(101_00), 10, TimeInForce::GTC);
    assert_eq!(ex.best_bid_ask(), (None, Some(Price(101_00))));
    assert_eq!(ex.book().best_bid_ask(), (None, Some(Price(101_00))));
}

#[test]
fn full_book_lists_every_level_best_first() {
    let snap = two_sided().full_book();
    let bids: Vec<_> = snap
        .bids
        .iter()
        .map(|l| (l.price, l.quantity, l.order_count))
        .collect();
    let asks: Vec<_> = snap
        .asks
        .iter()
        .map(|l| (l.price, l.quantity, l.order_count))
        .collect();
    assert_eq!(bids, vec![(Price(100_00), 10, 1), (Price(99_00), 5, 1)]);
    assert_eq!(asks, vec![(Price(101_00), 20, 1)]);
}

#[test]
fn contains_order_is_false_for_unknown_ids() {
    let mut ex = Exchange::new();
    let id = ex
        .submit_limit(Side::Buy, Price(100_00), 10, TimeInForce::GTC)
        .order_id;
    assert!(ex.book().contains_order(id));
    assert!(!ex.book().contains_order(OrderId(id.0 + 1_000)));
}

#[test]
fn stp_policy_setter_takes_effect() {
    let mut ex = Exchange::new();
    assert_eq!(ex.stp_policy(), StpPolicy::Off);
    ex.set_stp_policy(StpPolicy::CancelNewest);
    assert_eq!(ex.stp_policy(), StpPolicy::CancelNewest);

    // The setter changes matching: the same owner no longer trades with itself.
    let owner = OrderOwner(7);
    ex.submit_limit_with_owner(Side::Sell, Price(100_00), 10, TimeInForce::GTC, owner);
    let buy = ex.submit_limit_with_owner(Side::Buy, Price(100_00), 10, TimeInForce::GTC, owner);
    assert!(ex.trades().is_empty());
    assert_eq!(
        ex.get_order(buy.order_id).unwrap().status,
        OrderStatus::Cancelled
    );
}

#[test]
fn clear_order_history_returns_the_number_removed() {
    let mut ex = Exchange::new();
    let resting = ex
        .submit_limit(Side::Buy, Price(99_00), 10, TimeInForce::GTC)
        .order_id;
    let filled = ex
        .submit_limit(Side::Sell, Price(101_00), 5, TimeInForce::GTC)
        .order_id;
    ex.submit_limit(Side::Buy, Price(101_00), 5, TimeInForce::GTC); // fills `filled` and itself
    let cancelled = ex
        .submit_limit(Side::Sell, Price(102_00), 5, TimeInForce::GTC)
        .order_id;
    ex.cancel(cancelled);

    // Four orders: one active, three finished (two filled, one cancelled).
    assert_eq!(ex.clear_order_history(), 3);
    assert!(ex.get_order(resting).is_some());
    assert!(ex.get_order(filled).is_none());
    assert_eq!(ex.clear_order_history(), 0);
}

#[test]
fn book_mut_and_stop_book_reach_the_exchange_state() {
    let mut ex = Exchange::new();
    let id = ex
        .submit_limit(Side::Buy, Price(100_00), 10, TimeInForce::GTC)
        .order_id;
    assert_eq!(ex.book_mut().cancel_order(id), Some(10));
    assert_eq!(ex.best_bid_ask(), (None, None));

    ex.submit_stop_market(Side::Buy, Price(105_00), 10);
    assert_eq!(ex.stop_book().pending_count(), 1);
    assert_eq!(ex.pending_stop_count(), 1);
}
