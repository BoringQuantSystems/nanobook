//! Regression: resting quantities whose sum exceeds `u64::MAX`.
//!
//! A level kept its total in a saturating `u64`. After the total saturated,
//! removing the largest order drove it to zero, the level counted as empty
//! and was dropped, and the remaining orders stayed active but could never
//! match. Sums across levels and snapshots also overflowed, which made a
//! fill-or-kill feasibility check wrong in release builds.

#![allow(clippy::inconsistent_digit_grouping)]

use nanobook::{Exchange, OrderId, Price, Side, TimeInForce, Trade, TradeId};

const PRICE: Price = Price(100_00);

fn rest_two_asks_at_one_price() -> (Exchange, OrderId, OrderId) {
    let mut exchange = Exchange::new();
    let large = exchange.submit_limit(Side::Sell, PRICE, u64::MAX, TimeInForce::GTC);
    let small = exchange.submit_limit(Side::Sell, PRICE, 5, TimeInForce::GTC);
    (exchange, large.order_id, small.order_id)
}

#[test]
fn filling_the_large_order_keeps_the_small_order_matchable() {
    let (mut exchange, _, small) = rest_two_asks_at_one_price();
    assert_eq!(exchange.book().asks().total_quantity(), u64::MAX);

    let taker = exchange.submit_limit(Side::Buy, PRICE, u64::MAX, TimeInForce::IOC);
    assert_eq!(taker.filled_quantity, u64::MAX);
    assert_eq!(exchange.best_ask(), Some(PRICE));
    assert_eq!(exchange.book().asks().total_quantity(), 5);

    let second = exchange.submit_limit(Side::Buy, PRICE, 5, TimeInForce::IOC);
    assert_eq!(second.filled_quantity, 5);
    assert_eq!(exchange.best_ask(), None);
    assert!(!exchange.get_order(small).unwrap().is_active());
}

#[test]
fn cancelling_the_large_order_keeps_the_small_order_matchable() {
    let (mut exchange, large, _) = rest_two_asks_at_one_price();

    assert_eq!(exchange.cancel(large).cancelled_quantity, u64::MAX);
    assert_eq!(exchange.best_ask(), Some(PRICE));
    assert_eq!(exchange.book().asks().total_quantity(), 5);

    let taker = exchange.submit_limit(Side::Buy, PRICE, 5, TimeInForce::IOC);
    assert_eq!(taker.filled_quantity, 5);
}

#[test]
fn partial_fill_of_a_saturated_level_keeps_the_exact_remainder() {
    let (mut exchange, _, _) = rest_two_asks_at_one_price();

    let taker = exchange.submit_limit(Side::Buy, PRICE, 10, TimeInForce::IOC);
    assert_eq!(taker.filled_quantity, 10);
    // True remainder: u64::MAX - 10 + 5.
    assert_eq!(exchange.book().asks().total_quantity(), u64::MAX - 5);
}

#[test]
fn fill_or_kill_sees_liquidity_above_the_u64_range() {
    let mut exchange = Exchange::new();
    exchange.submit_limit(Side::Sell, Price(100_00), u64::MAX, TimeInForce::GTC);
    exchange.submit_limit(Side::Sell, Price(101_00), 5, TimeInForce::GTC);

    let taker = exchange.submit_limit(Side::Buy, Price(101_00), u64::MAX, TimeInForce::FOK);
    assert_eq!(taker.filled_quantity, u64::MAX);
}

#[test]
fn totals_across_levels_are_capped() {
    let mut exchange = Exchange::new();
    exchange.submit_limit(Side::Buy, Price(100_00), u64::MAX, TimeInForce::GTC);
    exchange.submit_limit(Side::Buy, Price(99_00), u64::MAX, TimeInForce::GTC);

    assert_eq!(exchange.book().bids().total_quantity(), u64::MAX);
    assert_eq!(
        exchange.book().bids().quantity_at_or_better(Price(99_00)),
        u64::MAX
    );

    let snapshot = exchange.depth(10);
    assert_eq!(snapshot.total_bid_quantity(), u64::MAX);
    assert_eq!(snapshot.imbalance(), Some(1.0));
}

#[test]
fn imbalance_uses_exact_side_sums() {
    let mut exchange = Exchange::new();
    exchange.submit_limit(Side::Buy, Price(100_00), u64::MAX, TimeInForce::GTC);
    exchange.submit_limit(Side::Buy, Price(99_00), u64::MAX, TimeInForce::GTC);
    exchange.submit_limit(Side::Sell, Price(101_00), u64::MAX, TimeInForce::GTC);

    // Bids hold twice the asks: (2 - 1) / (2 + 1).
    let imbalance = exchange.depth(10).imbalance().unwrap();
    assert!(
        (imbalance - 1.0 / 3.0).abs() < 1e-12,
        "imbalance {imbalance}"
    );
}

#[test]
fn imbalance_handles_both_sides_at_the_cap() {
    let mut exchange = Exchange::new();
    exchange.submit_limit(Side::Buy, Price(99_00), u64::MAX, TimeInForce::GTC);
    exchange.submit_limit(Side::Sell, Price(100_00), u64::MAX, TimeInForce::GTC);

    assert_eq!(exchange.depth(10).imbalance(), Some(0.0));
}

#[test]
fn vwap_is_none_when_the_quantity_sum_overflows() {
    let trade = |id| {
        Trade::new(
            TradeId(id),
            Price(0),
            u64::MAX,
            OrderId(id),
            OrderId(id + 10),
            Side::Buy,
            id,
        )
    };
    assert_eq!(Trade::vwap(&[trade(1), trade(2)]), None);
}
