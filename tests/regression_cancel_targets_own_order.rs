//! Regression: cancelling an order must remove that order and no other.
//!
//! Orders kept the queue index they had when they were added. Filling the
//! front of the queue shifted the remaining orders, so a later cancel marked
//! the next order instead. A finished IOC order with a partial fill also
//! stayed active, so cancelling it marked whichever order sat at its stale
//! index on the book.

#![allow(clippy::inconsistent_digit_grouping)]

use nanobook::{
    Exchange, Order, OrderId, OrderOwner, OrderStatus, Price, PriceLevels, Quantity, Side,
    StpPolicy, SubmitResult, TimeInForce,
};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

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
    SubmitLimit {
        side: Side,
        price: i64,
        quantity: Quantity,
        tif: TimeInForce,
        owner: Option<OrderOwner>,
    },
    SubmitMarket {
        side: Side,
        quantity: Quantity,
    },
    Modify {
        index: usize,
        price: i64,
        quantity: Quantity,
    },
    StopMarket {
        side: Side,
        stop_price: i64,
        quantity: Quantity,
    },
    StopLimit {
        side: Side,
        stop_price: i64,
        limit_price: i64,
        quantity: Quantity,
        tif: TimeInForce,
    },
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

fn side() -> impl Strategy<Value = Side> {
    prop_oneof![Just(Side::Buy), Just(Side::Sell)]
}

fn tif() -> impl Strategy<Value = TimeInForce> {
    prop_oneof![
        Just(TimeInForce::GTC),
        Just(TimeInForce::IOC),
        Just(TimeInForce::FOK),
    ]
}

fn owner() -> impl Strategy<Value = Option<OrderOwner>> {
    prop_oneof![
        Just(None),
        Just(Some(OrderOwner(1))),
        Just(Some(OrderOwner(2))),
    ]
}

fn price() -> impl Strategy<Value = i64> {
    1i64..=3
}

fn earlier_index() -> impl Strategy<Value = usize> {
    any::<prop::sample::Index>().prop_map(|index| index.index(1 << 16))
}

fn stp_policy() -> impl Strategy<Value = StpPolicy> {
    prop_oneof![
        Just(StpPolicy::Off),
        Just(StpPolicy::CancelNewest),
        Just(StpPolicy::CancelOldest),
        Just(StpPolicy::DecrementAndCancel),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (side(), price(), quantity(), tif(), owner()).prop_map(
            |(side, price, quantity, tif, owner)| Op::SubmitLimit {
                side,
                price,
                quantity,
                tif,
                owner,
            },
        ),
        2 => (side(), quantity()).prop_map(|(side, quantity)| Op::SubmitMarket { side, quantity }),
        2 => (earlier_index(), price(), quantity()).prop_map(|(index, price, quantity)| {
            Op::Modify {
                index,
                price,
                quantity,
            }
        }),
        1 => (side(), price(), quantity()).prop_map(|(side, stop_price, quantity)| Op::StopMarket {
            side,
            stop_price,
            quantity,
        }),
        1 => (side(), price(), price(), quantity(), tif()).prop_map(
            |(side, stop_price, limit_price, quantity, tif)| Op::StopLimit {
                side,
                stop_price,
                limit_price,
                quantity,
                tif,
            },
        ),
        3 => earlier_index().prop_map(Op::Cancel),
        1 => Just(Op::Compact),
    ]
}

fn note_ids(exchange: &Exchange, ids: &mut Vec<OrderId>, next: &mut u64) {
    let end = exchange.book().peek_next_order_id().0;
    while *next < end {
        ids.push(OrderId(*next));
        *next += 1;
    }
}

fn assert_submit(result: &SubmitResult, quantity: Quantity) -> Result<(), TestCaseError> {
    prop_assert_eq!(
        u128::from(result.filled_quantity)
            + u128::from(result.resting_quantity)
            + u128::from(result.cancelled_quantity),
        u128::from(quantity)
    );
    Ok(())
}

fn trade_price_ok(side: Side, limit: Price, trade_price: Price) -> bool {
    match side {
        Side::Buy => trade_price <= limit,
        Side::Sell => trade_price >= limit,
    }
}

fn assert_new_trades(exchange: &Exchange, from: usize) -> Result<(), TestCaseError> {
    for trade in &exchange.trades()[from..] {
        prop_assert!(trade.quantity > 0, "trade {:?} has no size", trade.id);
        prop_assert!(
            exchange.get_order(trade.aggressor_order_id).is_some(),
            "trade {:?} aggressor is not stored",
            trade.id
        );
        let aggressor = exchange.get_order(trade.aggressor_order_id).unwrap();
        // Market orders carry Price::MAX or Price::MIN. Those prices are not limits.
        if aggressor.price == Price::MAX || aggressor.price == Price::MIN {
            continue;
        }
        prop_assert!(
            trade_price_ok(aggressor.side, aggressor.price, trade.price),
            "trade {:?} price {} is outside {:?} limit {}",
            trade.id,
            trade.price.0,
            aggressor.side,
            aggressor.price.0
        );
    }
    Ok(())
}

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

fn assert_fok_rejection_is_inert(
    exchange: &Exchange,
    result: &SubmitResult,
    bids_before: &[(i64, Vec<u64>)],
    asks_before: &[(i64, Vec<u64>)],
    orders_before: &[Option<Order>],
) -> Result<(), TestCaseError> {
    prop_assert!(result.trades.is_empty());
    prop_assert_eq!(result.filled_quantity, 0);
    prop_assert!(exchange.get_order(result.order_id).is_none());
    prop_assert_eq!(level_ids(exchange.book().bids()), bids_before);
    prop_assert_eq!(level_ids(exchange.book().asks()), asks_before);
    let orders_after = order_slots(exchange);
    prop_assert_eq!(orders_after.len(), orders_before.len() + 1);
    prop_assert_eq!(&orders_after[..orders_before.len()], orders_before);
    prop_assert!(orders_after[orders_before.len()].is_none());
    Ok(())
}

fn assert_book(exchange: &Exchange) -> Result<(), TestCaseError> {
    if let (Some(bid), Some(ask)) = exchange.best_bid_ask() {
        prop_assert!(bid < ask, "crossed book bid {} ask {}", bid.0, ask.0);
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
                prop_assert_eq!(
                    u128::from(order.filled_quantity) + u128::from(order.remaining_quantity),
                    u128::from(order.original_quantity),
                    "active order {:?} lost quantity",
                    id
                );
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
    // A pending stop is not a book order. A triggered stop's child order is.
    // This scan keeps every id, including stop ids.
    let next = book.peek_next_order_id().0;
    let mut active: Vec<OrderId> = (1..next)
        .map(OrderId)
        .filter(|id| {
            exchange
                .get_order(*id)
                .is_some_and(|order| order.is_active())
        })
        .collect();
    active.sort_by_key(|id| id.0);
    prop_assert_eq!(queued, active);
    Ok(())
}

proptest! {
    /// After every limit, market, modify, stop, cancel, and compact: the book
    /// is not crossed, each submit conserves quantity, each limit trade is at
    /// or inside its limit, and every active order is queued once at its price.
    #[test]
    fn queues_match_active_orders(
        (policy, ops) in (stp_policy(), prop::collection::vec(op(), 1..120))
    ) {
        let mut exchange = Exchange::new().with_stp_policy(policy);
        let mut ids: Vec<OrderId> = Vec::new();
        let mut next_id = 1u64;

        for op in ops {
            let trades_before = exchange.trades().len();
            match op {
                Op::SubmitLimit {
                    side,
                    price,
                    quantity,
                    tif,
                    owner,
                } => {
                    let fok_before = (tif == TimeInForce::FOK).then(|| {
                        (
                            level_ids(exchange.book().bids()),
                            level_ids(exchange.book().asks()),
                            order_slots(&exchange),
                        )
                    });
                    let result = match owner {
                        Some(owner) => exchange.submit_limit_with_owner(
                            side,
                            Price(price),
                            quantity,
                            tif,
                            owner,
                        ),
                        None => exchange.submit_limit(side, Price(price), quantity, tif),
                    };
                    if let Some((bids_before, asks_before, orders_before)) = fok_before
                        && result.trades.is_empty()
                    {
                        assert_fok_rejection_is_inert(
                            &exchange,
                            &result,
                            &bids_before,
                            &asks_before,
                            &orders_before,
                        )?;
                    }
                    assert_submit(&result, quantity)?;
                }
                Op::SubmitMarket { side, quantity } => {
                    let result = exchange.submit_market(side, quantity);
                    assert_submit(&result, quantity)?;
                }
                Op::Modify {
                    index,
                    price,
                    quantity,
                } if !ids.is_empty() => {
                    exchange.modify(ids[index % ids.len()], Price(price), quantity);
                }
                Op::Modify { .. } => {}
                Op::StopMarket {
                    side,
                    stop_price,
                    quantity,
                } => {
                    exchange.submit_stop_market(side, Price(stop_price), quantity);
                }
                Op::StopLimit {
                    side,
                    stop_price,
                    limit_price,
                    quantity,
                    tif,
                } => {
                    exchange.submit_stop_limit(
                        side,
                        Price(stop_price),
                        Price(limit_price),
                        quantity,
                        tif,
                    );
                }
                Op::Cancel(index) if !ids.is_empty() => {
                    exchange.cancel(ids[index % ids.len()]);
                }
                Op::Cancel(_) => {}
                Op::Compact => exchange.compact(),
            }
            note_ids(&exchange, &mut ids, &mut next_id);
            assert_new_trades(&exchange, trades_before)?;
            assert_book(&exchange)?;
        }
    }
}
