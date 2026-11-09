//! Regression: cancelling an order must remove that order and no other.
//!
//! Orders kept the queue index they had when they were added. Filling the
//! front of the queue shifted the remaining orders, so a later cancel marked
//! the next order instead. A finished IOC order with a partial fill also
//! stayed active, so cancelling it marked whichever order sat at its stale
//! index on the book.

#![allow(clippy::inconsistent_digit_grouping)]

use nanobook::{
    Exchange, OrderId, OrderOwner, OrderStatus, Price, Quantity, Side, StpPolicy, TimeInForce,
};
use proptest::prelude::*;

const PRICE: Price = Price(100_00);

fn ask_queue(exchange: &Exchange) -> Vec<OrderId> {
    exchange
        .book()
        .asks()
        .iter_best_to_worst()
        .flat_map(|(_, level)| level.iter().collect::<Vec<_>>())
        .collect()
}

#[test]
fn cancel_after_front_fill_removes_the_named_order() {
    let mut exchange = Exchange::new();
    exchange.submit_limit(Side::Sell, PRICE, 1, TimeInForce::GTC);
    let b = exchange.submit_limit(Side::Sell, PRICE, 2, TimeInForce::GTC);
    let c = exchange.submit_limit(Side::Sell, PRICE, 4, TimeInForce::GTC);
    exchange.submit_limit(Side::Buy, PRICE, 1, TimeInForce::IOC);

    assert_eq!(exchange.cancel(b.order_id).cancelled_quantity, 2);
    assert_eq!(ask_queue(&exchange), vec![c.order_id]);

    let taker = exchange.submit_limit(Side::Buy, PRICE, 4, TimeInForce::IOC);
    assert_eq!(taker.filled_quantity, 4);
    assert_eq!(taker.trades[0].passive_order_id, c.order_id);
}

#[test]
fn cancel_after_compact_removes_the_named_order() {
    let mut exchange = Exchange::new();
    let a = exchange.submit_limit(Side::Sell, PRICE, 1, TimeInForce::GTC);
    let b = exchange.submit_limit(Side::Sell, PRICE, 2, TimeInForce::GTC);
    let c = exchange.submit_limit(Side::Sell, PRICE, 4, TimeInForce::GTC);
    exchange.cancel(a.order_id);
    exchange.compact();

    assert_eq!(exchange.cancel(b.order_id).cancelled_quantity, 2);
    assert_eq!(ask_queue(&exchange), vec![c.order_id]);
}

#[test]
fn finished_ioc_cannot_be_cancelled_and_leaves_other_orders_alone() {
    let mut exchange = Exchange::new();
    exchange.submit_limit(Side::Buy, PRICE, 10, TimeInForce::GTC);
    let ioc = exchange.submit_limit(Side::Sell, PRICE, 30, TimeInForce::IOC);
    assert_eq!(ioc.status, OrderStatus::PartiallyFilled);
    let stored = exchange.get_order(ioc.order_id).unwrap();
    assert_eq!(stored.status, OrderStatus::Cancelled);
    assert_eq!(stored.remaining_quantity, 0);
    assert_eq!(stored.filled_quantity, 10);

    let other = exchange.submit_limit(Side::Sell, PRICE, 7, TimeInForce::GTC);
    assert!(!exchange.cancel(ioc.order_id).success);
    assert_eq!(ask_queue(&exchange), vec![other.order_id]);

    let taker = exchange.submit_limit(Side::Buy, PRICE, 7, TimeInForce::IOC);
    assert_eq!(taker.filled_quantity, 7);
}

#[test]
fn partly_filled_self_trade_cancel_leaves_other_orders_alone() {
    let owner = OrderOwner(1);
    let mut exchange = Exchange::new().with_stp_policy(StpPolicy::CancelNewest);
    exchange.submit_limit(Side::Buy, PRICE, 10, TimeInForce::GTC);
    let own_bid = exchange.submit_limit_with_owner(Side::Buy, PRICE, 5, TimeInForce::GTC, owner);
    let incoming = exchange.submit_limit_with_owner(Side::Sell, PRICE, 30, TimeInForce::GTC, owner);
    assert_eq!(incoming.status, OrderStatus::PartiallyFilled);
    assert_eq!(incoming.filled_quantity, 10);
    assert!(!exchange.get_order(incoming.order_id).unwrap().is_active());

    exchange.cancel(own_bid.order_id);
    let other = exchange.submit_limit(Side::Sell, PRICE, 7, TimeInForce::GTC);
    assert!(!exchange.cancel(incoming.order_id).success);
    assert_eq!(ask_queue(&exchange), vec![other.order_id]);
}

#[derive(Clone, Debug)]
enum Op {
    Submit(Side, i64, Quantity, TimeInForce),
    Cancel(usize),
    Compact,
}

fn quantity() -> impl Strategy<Value = Quantity> {
    prop_oneof![
        4 => 1u64..=10_000,
        1 => Just(u64::MAX),
        1 => Just(u64::MAX - 1),
        1 => Just(u64::MAX / 2),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        6 => (
            prop_oneof![Just(Side::Buy), Just(Side::Sell)],
            1i64..=3,
            quantity(),
            prop_oneof![Just(TimeInForce::GTC), Just(TimeInForce::IOC)],
        )
            .prop_map(|(side, price, quantity, tif)| Op::Submit(side, price, quantity, tif)),
        3 => any::<prop::sample::Index>().prop_map(|index| Op::Cancel(index.index(1 << 16))),
        1 => Just(Op::Compact),
    ]
}

proptest! {
    /// After any mix of submits, later cancels, and compaction: every active
    /// order sits once in the level at its own price, queues stay in arrival
    /// order, each level total is exact, and every trade has a positive size.
    #[test]
    fn queues_match_active_orders(ops in prop::collection::vec(op(), 1..120)) {
        let mut exchange = Exchange::new();
        let mut submitted: Vec<OrderId> = Vec::new();

        for op in ops {
            match op {
                Op::Submit(side, price, quantity, tif) => {
                    let result = exchange.submit_limit(side, Price(price), quantity, tif);
                    prop_assert!(result.trades.iter().all(|t| t.quantity > 0));
                    submitted.push(result.order_id);
                }
                Op::Cancel(index) if !submitted.is_empty() => {
                    exchange.cancel(submitted[index % submitted.len()]);
                }
                Op::Cancel(_) => {}
                Op::Compact => exchange.compact(),
            }
        }

        let book = exchange.book();
        let mut queued = Vec::new();
        for levels in [book.bids(), book.asks()] {
            for (price, level) in levels.iter_best_to_worst() {
                prop_assert!(level.order_count() > 0);
                let ids: Vec<OrderId> = level.iter().collect();
                prop_assert!(ids.windows(2).all(|pair| pair[0].0 < pair[1].0));

                let mut sum = 0u128;
                for id in ids {
                    let order = exchange.get_order(id).unwrap();
                    prop_assert!(order.is_active(), "queued order {:?} is not active", id);
                    prop_assert_eq!(order.price, *price);
                    sum += u128::from(order.remaining_quantity);
                    queued.push(id);
                }
                prop_assert_eq!(
                    u128::from(level.total_quantity()),
                    sum.min(u128::from(u64::MAX))
                );
            }
        }

        queued.sort_by_key(|id| id.0);
        let mut active: Vec<OrderId> = submitted
            .into_iter()
            .filter(|id| exchange.get_order(*id).is_some_and(|o| o.is_active()))
            .collect();
        active.sort_by_key(|id| id.0);
        prop_assert_eq!(queued, active);
    }
}
