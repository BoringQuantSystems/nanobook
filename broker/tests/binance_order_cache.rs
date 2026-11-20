#![cfg(feature = "binance")]

use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use nanobook::Symbol;
use nanobook_broker::binance::{BinanceBroker, BinanceOrderCache, CachedOrder};
use nanobook_broker::{BrokerSide, OrderId, OrderState};

fn test_cache_path(name: &str) -> PathBuf {
    PathBuf::from("target").join(format!(
        "binance_order_cache_{name}_{}.json",
        std::process::id()
    ))
}

#[test]
fn test_order_cache_new() {
    let cache = BinanceOrderCache::new();
    assert!(cache.orders.is_empty());
}

#[test]
fn test_cache_order_roundtrip() {
    let broker = BinanceBroker::new("api-key", "secret-key", true);
    let order_id = OrderId(42);

    broker.cache_order(
        order_id,
        Symbol::new("BTC"),
        100,
        BrokerSide::Buy,
        Some("client-42".to_string()),
    );

    let cached = broker.get_cached_order(order_id).unwrap();
    assert_eq!(cached.symbol, Symbol::new("BTC"));
    assert_eq!(cached.quantity, 100);
    assert_eq!(cached.side, BrokerSide::Buy);
    assert_eq!(cached.status, OrderState::Submitted);
    assert_eq!(cached.binance_order_id, "42");
    assert_eq!(cached.client_order_id.as_deref(), Some("client-42"));
}

#[test]
fn test_cache_persistence() {
    let path = test_cache_path("persistence");
    let _ = fs::remove_file(&path);

    let mut cache = BinanceOrderCache::new();
    cache.orders.insert(
        OrderId(7),
        CachedOrder {
            symbol: Symbol::new("ETH"),
            quantity: 250,
            side: BrokerSide::Sell,
            status: OrderState::Submitted,
            binance_order_id: "7001".to_string(),
            client_order_id: Some("client-7".to_string()),
            submitted_at: Utc::now(),
        },
    );

    cache.save_to_disk(&path).unwrap();
    let loaded = BinanceOrderCache::load_from_disk(&path).unwrap();
    let cached = loaded.orders.get(&OrderId(7)).unwrap();

    assert_eq!(cached.symbol, Symbol::new("ETH"));
    assert_eq!(cached.quantity, 250);
    assert_eq!(cached.side, BrokerSide::Sell);
    assert_eq!(cached.status, OrderState::Submitted);
    assert_eq!(cached.binance_order_id, "7001");
    assert_eq!(cached.client_order_id.as_deref(), Some("client-7"));

    fs::remove_file(path).unwrap();
}

#[test]
fn test_cache_clear() {
    let broker = BinanceBroker::new("api-key", "secret-key", true);
    let order_id = OrderId(10);
    broker.cache_order(order_id, Symbol::new("SOL"), 12, BrokerSide::Buy, None);

    assert!(broker.get_cached_order(order_id).is_some());
    broker.clear_cache();
    assert!(broker.get_cached_order(order_id).is_none());
}

#[test]
fn test_update_cached_order_status() {
    let broker = BinanceBroker::new("api-key", "secret-key", true);
    let order_id = OrderId(11);
    broker.cache_order(order_id, Symbol::new("ADA"), 99, BrokerSide::Sell, None);

    broker.update_cached_order_status(order_id, OrderState::Filled);

    let cached = broker.get_cached_order(order_id).unwrap();
    assert_eq!(cached.status, OrderState::Filled);
}

fn cached(
    symbol: &str,
    quantity: i64,
    side: BrokerSide,
    status: OrderState,
    binance_order_id: &str,
) -> CachedOrder {
    CachedOrder {
        symbol: Symbol::new(symbol),
        quantity,
        side,
        status,
        binance_order_id: binance_order_id.to_string(),
        client_order_id: Some(format!("client-{binance_order_id}")),
        submitted_at: Utc::now(),
    }
}

#[test]
fn test_cache_persistence_covers_every_side_and_status() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("orders.json");

    let rows = [
        (1, BrokerSide::Buy, OrderState::Pending),
        (2, BrokerSide::Sell, OrderState::Submitted),
        (3, BrokerSide::Buy, OrderState::PartiallyFilled),
        (4, BrokerSide::Sell, OrderState::Filled),
        (5, BrokerSide::Buy, OrderState::Cancelled),
        (6, BrokerSide::Sell, OrderState::Rejected),
    ];
    let mut cache = BinanceOrderCache::new();
    for (id, side, status) in rows {
        cache.orders.insert(
            OrderId(id),
            cached(
                "BTC",
                10 * id as i64,
                side,
                status,
                &format!("{}", 7000 + id),
            ),
        );
    }

    cache.save_to_disk(&path).unwrap();
    let loaded = BinanceOrderCache::load_from_disk(&path).unwrap();

    assert_eq!(loaded.orders.len(), 6);
    for (id, side, status) in rows {
        let order = &loaded.orders[&OrderId(id)];
        assert_eq!(order.side, side, "order {id}");
        assert_eq!(order.status, status, "order {id}");
        assert_eq!(order.quantity, 10 * id as i64, "order {id}");
        assert_eq!(order.binance_order_id, format!("{}", 7000 + id));
    }
}

#[test]
fn test_broker_saves_and_loads_its_cache() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("broker_cache.json");

    let broker = BinanceBroker::new("api-key", "secret-key", true);
    broker.cache_order(
        OrderId(42),
        Symbol::new("BTC"),
        100,
        BrokerSide::Buy,
        Some("c-42".to_string()),
    );
    broker.update_cached_order_status(OrderId(42), OrderState::PartiallyFilled);
    broker.save_cache_to_disk(&path).unwrap();
    assert!(path.exists());

    let fresh = BinanceBroker::new("api-key", "secret-key", true);
    assert!(fresh.get_cached_order(OrderId(42)).is_none());
    fresh.load_cache_from_disk(&path).unwrap();

    let order = fresh.get_cached_order(OrderId(42)).unwrap();
    assert_eq!(order.symbol, Symbol::new("BTC"));
    assert_eq!(order.quantity, 100);
    assert_eq!(order.side, BrokerSide::Buy);
    assert_eq!(order.status, OrderState::PartiallyFilled);
    assert_eq!(order.client_order_id.as_deref(), Some("c-42"));
}
