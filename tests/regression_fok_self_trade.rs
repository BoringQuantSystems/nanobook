//! A fill-or-kill order counts only size it can trade.
//! A short order is rejected. The book does not change.

#![allow(clippy::inconsistent_digit_grouping)]

use nanobook::{
    Exchange, Order, OrderId, OrderOwner, OrderStatus, Price, PriceLevels, Side, StpPolicy,
    TimeInForce,
};

const PRICE: Price = Price(100_00);

fn level_ids(levels: &PriceLevels) -> Vec<(i64, Vec<u64>)> {
    levels
        .iter_best_to_worst()
        .map(|(price, level)| (price.0, level.iter().map(|id| id.0).collect()))
        .collect()
}

fn order_slots(exchange: &Exchange) -> Vec<Option<Order>> {
    let end = exchange.book().peek_next_order_id().0;
    (1..end)
        .map(|id| exchange.get_order(OrderId(id)).cloned())
        .collect()
}

struct BookView {
    bids: Vec<(i64, Vec<u64>)>,
    asks: Vec<(i64, Vec<u64>)>,
    orders: Vec<Option<Order>>,
}

fn book_view(exchange: &Exchange) -> BookView {
    BookView {
        bids: level_ids(exchange.book().bids()),
        asks: level_ids(exchange.book().asks()),
        orders: order_slots(exchange),
    }
}

fn assert_unchanged(exchange: &Exchange, before: &BookView, fok_id: OrderId) {
    assert!(exchange.get_order(fok_id).is_none());
    let after = book_view(exchange);
    assert_eq!(after.bids, before.bids);
    assert_eq!(after.asks, before.asks);
    assert_eq!(after.orders.len(), before.orders.len() + 1);
    assert_eq!(&after.orders[..before.orders.len()], &before.orders[..]);
    assert!(after.orders[before.orders.len()].is_none());
}

fn assert_own_fok_rejected(policy: StpPolicy) {
    let owner = OrderOwner(1);
    let mut exchange = Exchange::new().with_stp_policy(policy);
    let resting = exchange.submit_limit_with_owner(Side::Sell, PRICE, 10, TimeInForce::GTC, owner);
    let before = book_view(&exchange);
    let fok = exchange.submit_limit_with_owner(Side::Buy, PRICE, 10, TimeInForce::FOK, owner);

    assert_eq!(fok.status, OrderStatus::Cancelled);
    assert_eq!(fok.filled_quantity, 0);
    assert_eq!(fok.cancelled_quantity, 10);
    assert!(fok.trades.is_empty());
    assert_unchanged(&exchange, &before, fok.order_id);

    let stored = exchange.get_order(resting.order_id).unwrap();
    assert_eq!(stored.status, OrderStatus::New);
    assert_eq!(stored.remaining_quantity, 10);
}

#[test]
fn fok_cancel_newest_rejects_own_liquidity() {
    assert_own_fok_rejected(StpPolicy::CancelNewest);
}

#[test]
fn fok_cancel_oldest_rejects_own_liquidity() {
    assert_own_fok_rejected(StpPolicy::CancelOldest);
}

#[test]
fn fok_decrement_rejects_own_liquidity() {
    assert_own_fok_rejected(StpPolicy::DecrementAndCancel);
}

#[test]
fn fok_decrement_stops_when_own_resting_is_larger() {
    let owner = OrderOwner(1);
    let mut exchange = Exchange::new().with_stp_policy(StpPolicy::DecrementAndCancel);
    let resting = exchange.submit_limit_with_owner(Side::Sell, PRICE, 15, TimeInForce::GTC, owner);
    let before = book_view(&exchange);
    let fok = exchange.submit_limit_with_owner(Side::Buy, PRICE, 10, TimeInForce::FOK, owner);

    assert!(fok.trades.is_empty());
    assert_unchanged(&exchange, &before, fok.order_id);
    let stored = exchange.get_order(resting.order_id).unwrap();
    assert_eq!(stored.remaining_quantity, 15);
    assert_eq!(stored.status, OrderStatus::New);
}

#[test]
fn fok_off_fills_own_sell() {
    let owner = OrderOwner(1);
    let mut exchange = Exchange::new().with_stp_policy(StpPolicy::Off);
    let resting = exchange.submit_limit_with_owner(Side::Sell, PRICE, 10, TimeInForce::GTC, owner);
    let fok = exchange.submit_limit_with_owner(Side::Buy, PRICE, 10, TimeInForce::FOK, owner);

    assert_eq!(fok.status, OrderStatus::Filled);
    assert_eq!(fok.filled_quantity, 10);
    assert_eq!(fok.trades.len(), 1);
    assert_eq!(fok.trades[0].passive_order_id, resting.order_id);
    assert_eq!(fok.trades[0].quantity, 10);
    let stored = exchange.get_order(resting.order_id).unwrap();
    assert_eq!(stored.status, OrderStatus::Filled);
    assert_eq!(stored.remaining_quantity, 0);
}

#[test]
fn fok_fills_the_other_owner_ahead_of_its_own() {
    let own = OrderOwner(1);
    let other = OrderOwner(2);
    let mut exchange = Exchange::new().with_stp_policy(StpPolicy::CancelNewest);
    let ahead = exchange.submit_limit_with_owner(Side::Sell, PRICE, 10, TimeInForce::GTC, other);
    let behind = exchange.submit_limit_with_owner(Side::Sell, PRICE, 10, TimeInForce::GTC, own);
    let fok = exchange.submit_limit_with_owner(Side::Buy, PRICE, 10, TimeInForce::FOK, own);

    assert_eq!(fok.status, OrderStatus::Filled);
    assert_eq!(fok.filled_quantity, 10);
    assert_eq!(fok.trades.len(), 1);
    assert_eq!(fok.trades[0].passive_order_id, ahead.order_id);
    assert_eq!(fok.trades[0].quantity, 10);
    let own_sell = exchange.get_order(behind.order_id).unwrap();
    assert_eq!(own_sell.status, OrderStatus::New);
    assert_eq!(own_sell.remaining_quantity, 10);
    assert!(exchange.get_order(fok.order_id).is_some());
}

#[test]
fn fok_is_not_partly_filled_ahead_of_its_own_order() {
    // Another owner's 4 rests ahead of the same owner's 10. The old check
    // counted 14, the matcher traded 4, then self-trade prevention cancelled
    // the rest: a fill-or-kill order was partly filled.
    let mut exchange = Exchange::new().with_stp_policy(StpPolicy::CancelNewest);
    exchange.submit_limit(Side::Sell, PRICE, 4, TimeInForce::GTC);
    exchange.submit_limit_with_owner(Side::Sell, PRICE, 10, TimeInForce::GTC, OrderOwner(1));
    let before = book_view(&exchange);

    let fok =
        exchange.submit_limit_with_owner(Side::Buy, PRICE, 10, TimeInForce::FOK, OrderOwner(1));
    assert!(
        fok.trades.is_empty(),
        "fill-or-kill traded {:?}",
        fok.trades
    );
    assert_eq!(fok.filled_quantity, 0);
    assert_unchanged(&exchange, &before, fok.order_id);
}
