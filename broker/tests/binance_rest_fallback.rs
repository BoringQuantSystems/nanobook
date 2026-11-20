//! Tests for Binance REST fallback polling mode (F-bin2 Phase 4).

#[cfg(feature = "binance")]
mod binance_rest_fallback_tests {
    use std::time::{Duration, SystemTime};

    use nanobook::Symbol;
    use nanobook_broker::BrokerError;
    use nanobook_broker::binance::types::{AccountInfo, BalanceInfo, OrderInfo};
    use nanobook_broker::binance::{BinanceBroker, ConnectionMode};
    use nanobook_broker::types::{BrokerSide, OrderId, OrderState};

    fn account_info_fixture() -> AccountInfo {
        AccountInfo {
            balances: vec![
                BalanceInfo {
                    asset: "BTC".to_string(),
                    free: "1.00000000".to_string(),
                    locked: "0.50000000".to_string(),
                },
                BalanceInfo {
                    asset: "USDT".to_string(),
                    free: "100.00".to_string(),
                    locked: "0.00".to_string(),
                },
            ],
            positions: Vec::new(),
            open_orders: vec![OrderInfo {
                symbol: "BTCUSDT".to_string(),
                order_id: 42,
                status: "NEW".to_string(),
                side: "BUY".to_string(),
                orig_qty: "7".to_string(),
                executed_qty: "0".to_string(),
            }],
            can_trade: true,
        }
    }

    #[test]
    fn test_connection_mode_enum() {
        let broker = BinanceBroker::new("api-key", "secret-key", true)
            .with_connection_mode(ConnectionMode::Rest);

        assert_eq!(broker.connection_mode(), ConnectionMode::Rest);
        assert_ne!(ConnectionMode::WebSocket, ConnectionMode::Auto);
    }

    #[test]
    fn test_rest_polling_updates_state() {
        let broker = BinanceBroker::new("api-key", "secret-key", true)
            .with_connection_mode(ConnectionMode::Rest);

        let info = account_info_fixture();
        broker.update_state_from_account_info(&info).unwrap();
        broker.update_order_cache_from_open_orders(&info.open_orders);

        let positions = broker.cached_positions();
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0].symbol, Symbol::try_new("BTC").unwrap());
        assert_eq!(positions[0].quantity, 150_000_000);

        let cached_order = broker.get_cached_order(OrderId(42)).unwrap();
        assert_eq!(cached_order.symbol, Symbol::try_new("BTC").unwrap());
        assert_eq!(cached_order.quantity, 7);
        assert_eq!(cached_order.side, BrokerSide::Buy);
        assert_eq!(cached_order.status, OrderState::Submitted);
        assert_eq!(cached_order.binance_order_id, "42");
    }

    #[test]
    fn test_mode_switching() {
        let mut broker = BinanceBroker::new("api-key", "secret-key", true);

        assert_eq!(broker.connection_mode(), ConnectionMode::Auto);

        broker.switch_to_rest_mode();
        assert_eq!(broker.connection_mode(), ConnectionMode::Rest);

        broker.switch_to_websocket_mode();
        assert_eq!(broker.connection_mode(), ConnectionMode::WebSocket);
    }

    #[test]
    fn test_polling_interval_enforcement() {
        let broker = BinanceBroker::new("api-key", "secret-key", true);

        assert!(broker.should_poll(None));

        let now = SystemTime::now();
        assert!(!broker.should_poll(Some(now)));

        let stale = now - Duration::from_millis(5001);
        assert!(broker.should_poll(Some(stale)));

        broker.set_last_account_poll(Some(now));
        broker.set_last_orders_poll(Some(stale));
        assert_eq!(broker.last_account_poll(), Some(now));
        assert_eq!(broker.last_orders_poll(), Some(stale));
    }

    #[test]
    fn test_mode_configuration() {
        let rest = BinanceBroker::new("api-key", "secret-key", true)
            .with_connection_mode(ConnectionMode::Rest);
        let websocket = BinanceBroker::new("api-key", "secret-key", true)
            .with_connection_mode(ConnectionMode::WebSocket);
        let auto = BinanceBroker::new("api-key", "secret-key", true);

        assert_eq!(rest.connection_mode(), ConnectionMode::Rest);
        assert_eq!(websocket.connection_mode(), ConnectionMode::WebSocket);
        assert_eq!(auto.connection_mode(), ConnectionMode::Auto);
    }

    #[test]
    fn test_poll_account_info_polls_when_due_and_skips_when_recent() {
        let broker = BinanceBroker::new("api-key", "secret-key", true);

        // No poll recorded yet: a poll is due, and it needs a client.
        assert!(matches!(
            broker.poll_account_info(),
            Err(BrokerError::NotConnected)
        ));

        // Polled just now: nothing is due, so no client is needed and the
        // stamp stays as it was.
        let now = SystemTime::now();
        broker.set_last_account_poll(Some(now));
        broker.poll_account_info().unwrap();
        assert_eq!(broker.last_account_poll(), Some(now));
    }

    #[test]
    fn test_poll_open_orders_polls_when_due_and_skips_when_recent() {
        let broker = BinanceBroker::new("api-key", "secret-key", true);

        assert!(matches!(
            broker.poll_open_orders(),
            Err(BrokerError::NotConnected)
        ));

        let now = SystemTime::now();
        broker.set_last_orders_poll(Some(now));
        broker.poll_open_orders().unwrap();
        assert_eq!(broker.last_orders_poll(), Some(now));
    }

    #[test]
    fn test_open_order_side_and_status_mapping() {
        let order = |order_id: u64, symbol: &str, side: &str, status: &str, qty: &str| OrderInfo {
            symbol: symbol.to_string(),
            order_id,
            status: status.to_string(),
            side: side.to_string(),
            orig_qty: qty.to_string(),
            executed_qty: "0".to_string(),
        };
        let info = AccountInfo {
            balances: Vec::new(),
            positions: Vec::new(),
            open_orders: vec![
                order(1, "BTCUSDT", "SELL", "NEW", "3"),
                order(2, "ETHUSDT", "BUY", "PARTIALLY_FILLED", "5"),
                order(3, "SOLUSDT", "BUY", "FILLED", "1"),
                order(4, "ADAUSDT", "BUY", "CANCELED", "1"),
                order(5, "DOGEUSDT", "BUY", "REJECTED", "1"),
                order(6, "XRPUSDT", "BUY", "EXPIRED", "1"),
            ],
            can_trade: true,
        };
        let broker = BinanceBroker::new("api-key", "secret-key", true);

        broker.update_order_cache_from_open_orders(&info.open_orders);

        let expected = [
            (1, "BTC", BrokerSide::Sell, OrderState::Submitted, 3),
            (2, "ETH", BrokerSide::Buy, OrderState::PartiallyFilled, 5),
            (3, "SOL", BrokerSide::Buy, OrderState::Filled, 1),
            (4, "ADA", BrokerSide::Buy, OrderState::Cancelled, 1),
            (5, "DOGE", BrokerSide::Buy, OrderState::Rejected, 1),
            (6, "XRP", BrokerSide::Buy, OrderState::Cancelled, 1),
        ];
        for (id, symbol, side, status, quantity) in expected {
            let cached = broker.get_cached_order(OrderId(id)).unwrap();
            assert_eq!(cached.symbol, Symbol::new(symbol), "order {id}");
            assert_eq!(cached.side, side, "order {id}");
            assert_eq!(cached.status, status, "order {id}");
            assert_eq!(cached.quantity, quantity, "order {id}");
        }
    }
}
