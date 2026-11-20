#![cfg(feature = "ibkr")]

use nanobook_broker::Broker;
use nanobook_broker::BrokerError;
use nanobook_broker::ibkr::orders::{broker_order_status_from_ibkr_parts, map_ibkr_order_status};
use nanobook_broker::ibkr::{ConnectionState, IbkrBroker};
use nanobook_broker::types::{OrderId, OrderState};

#[test]
fn test_open_orders_empty_when_not_connected() {
    let broker = IbkrBroker::new("127.0.0.1", 4002, 100);

    let result = broker.open_orders();

    assert!(matches!(result, Err(BrokerError::NotConnected)));
}

#[test]
fn test_ibkr_order_status_mapping() {
    assert_eq!(map_ibkr_order_status("Submitted"), OrderState::Submitted);
    assert_eq!(map_ibkr_order_status("PreSubmitted"), OrderState::Submitted);
    assert_eq!(
        map_ibkr_order_status("PendingSubmit"),
        OrderState::Submitted
    );
    assert_eq!(
        map_ibkr_order_status("PartiallyFilled"),
        OrderState::PartiallyFilled
    );
    assert_eq!(map_ibkr_order_status("Filled"), OrderState::Filled);
    assert_eq!(map_ibkr_order_status("Cancelled"), OrderState::Cancelled);
    assert_eq!(map_ibkr_order_status("ApiCancelled"), OrderState::Cancelled);
    assert_eq!(map_ibkr_order_status("Inactive"), OrderState::Rejected);
}

#[test]
fn test_ibkr_order_status_partially_filled() {
    let status = broker_order_status_from_ibkr_parts(42, "Submitted", 25.0, 75.0, 123.45).unwrap();

    assert_eq!(status.id.0, 42);
    assert_eq!(status.status, OrderState::PartiallyFilled);
    assert_eq!(status.filled_quantity, 25);
    assert_eq!(status.remaining_quantity, 75);
    assert_eq!(status.avg_fill_price_cents, 12_345);
}

#[test]
fn test_reconcile_state_requires_connection() {
    let mut broker = IbkrBroker::new("127.0.0.1", 4002, 100);

    let result = broker.reconcile_state();

    assert!(matches!(result, Err(BrokerError::NotConnected)));
}

#[test]
fn test_ibkr_order_status_without_fills_keeps_the_mapped_status() {
    // A resting order has filled 0 and remaining = the full quantity (this is
    // what `open_orders()` builds from order data). It must not read as a
    // partial fill.
    let status = broker_order_status_from_ibkr_parts(7, "Cancelled", 0.0, 100.0, 0.0).unwrap();

    assert_eq!(status.id.0, 7);
    assert_eq!(status.status, OrderState::Cancelled);
    assert_eq!(status.filled_quantity, 0);
    assert_eq!(status.remaining_quantity, 100);
    assert_eq!(status.avg_fill_price_cents, 0);
}

#[test]
fn test_ibkr_order_status_fully_filled() {
    // Nothing remaining: filled > 0 alone does not make a partial fill.
    let status = broker_order_status_from_ibkr_parts(7, "Filled", 100.0, 0.0, 185.5).unwrap();

    assert_eq!(status.id.0, 7);
    assert_eq!(status.status, OrderState::Filled);
    assert_eq!(status.filled_quantity, 100);
    assert_eq!(status.remaining_quantity, 0);
    assert_eq!(status.avg_fill_price_cents, 18_550);
}

#[test]
fn test_connect_without_gateway_fails_and_leaves_no_client() {
    // A loopback port that was just released: the TCP connect is refused at
    // once (a fixed port could be filtered and hang).
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let mut broker = IbkrBroker::new("127.0.0.1", port, 100);

    let result = broker.connect();

    match result {
        Err(BrokerError::Connection(message)) => {
            assert!(
                message.starts_with(&format!("failed to connect to 127.0.0.1:{port}")),
                "got {message}"
            );
        }
        other => panic!("expected Connection error, got {other:?}"),
    }
    assert!(broker.client().is_none());
    assert_eq!(broker.connection_state(), ConnectionState::Disconnected);
    assert!(!broker.is_connected());
}

#[test]
fn test_reconnect_requires_connection() {
    let mut broker = IbkrBroker::new("127.0.0.1", 4002, 100);

    let result = broker.reconnect();

    assert!(matches!(result, Err(BrokerError::NotConnected)));
}

#[test]
fn test_cancel_order_requires_connection() {
    let broker = IbkrBroker::new("127.0.0.1", 4002, 100);

    let result = broker.cancel_order(OrderId(7));

    assert!(matches!(result, Err(BrokerError::NotConnected)));
}
