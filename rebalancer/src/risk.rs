//! Pre-trade risk checks.
//!
//! Validates a set of rebalance orders against risk limits before execution.

use nanobook::Symbol;
use nanobook_broker::{Account, BrokerSide};
use rustc_hash::{FxHashMap, FxHashSet};

use nanobook_risk::{RiskConfig as SharedRiskConfig, RiskEngine as RiskEngineImpl};

use crate::config::RiskConfig;
use crate::diff::{Action, RebalanceOrder};

pub use nanobook_risk::report::{RiskCheck, RiskReport, RiskStatus};

/// Convert rebalancer actions to broker-side direction.
fn action_to_side(action: Action) -> BrokerSide {
    match action {
        Action::Buy | Action::BuyCover => BrokerSide::Buy,
        Action::Sell | Action::SellShort => BrokerSide::Sell,
    }
}

/// Convert rebalancer risk config into nanobook-risk config.
fn adapt_config(config: &RiskConfig) -> SharedRiskConfig {
    SharedRiskConfig {
        max_position_pct: config.max_position_pct,
        max_leverage: config.max_leverage,
        min_trade_usd: config.min_trade_usd,
        max_trade_usd: config.max_trade_usd,
        allow_short: config.allow_short,
        max_short_pct: config.max_short_pct,
        // Rebalancer config doesn't expose these yet; 0 = disabled.
        max_order_value_cents: 0,
        max_batch_value_cents: 0,
        ..SharedRiskConfig::default()
    }
}

fn validation_failure(detail: impl Into<String>) -> RiskReport {
    RiskReport {
        checks: vec![RiskCheck {
            name: "Order validation",
            status: RiskStatus::Fail,
            detail: detail.into(),
        }],
    }
}

/// Run all pre-trade risk checks.
///
/// # Arguments
/// - `orders`: The computed rebalance orders
/// - `equity_cents`: Total account equity
/// - `targets`: Target (symbol, weight) pairs
/// - `prices`: Current market prices (symbol, price_cents)
/// - `current_qty`: Current positions (symbol → quantity) including the effect of orders
/// - `config`: Risk configuration
pub fn check_risk(
    orders: &[RebalanceOrder],
    equity_cents: i64,
    targets: &[(Symbol, f64)],
    prices: &[(Symbol, i64)],
    current_qty: &FxHashMap<Symbol, i64>,
    config: &RiskConfig,
) -> RiskReport {
    let engine = match RiskEngineImpl::new(adapt_config(config)) {
        Ok(e) => e,
        Err(e) => return validation_failure(e.to_string()),
    };
    let account = Account {
        equity_cents,
        buying_power_cents: equity_cents,
        cash_cents: equity_cents,
        gross_position_value_cents: 0,
    };

    let price_map: FxHashMap<Symbol, i64> = prices.iter().copied().collect();
    let mut broker_orders: Vec<(Symbol, BrokerSide, u64, i64)> =
        Vec::with_capacity(orders.len() + current_qty.len());
    let mut symbols_with_orders: FxHashSet<Symbol> = FxHashSet::default();

    for order in orders {
        let shares = match u64::try_from(order.shares) {
            Ok(v) => v,
            Err(_) => {
                return validation_failure(format!("invalid share quantity for {order:?}"));
            }
        };

        broker_orders.push((
            order.symbol,
            action_to_side(order.action),
            shares,
            order.limit_price_cents,
        ));
        symbols_with_orders.insert(order.symbol);
    }

    // Include unchanged current positions by carrying a zero-quantity seed with a known price.
    // This keeps leverage/short exposure checks consistent with pre-trade behavior.
    for (symbol, qty) in current_qty.iter().filter(|(_, qty)| **qty != 0) {
        if symbols_with_orders.contains(symbol) {
            continue;
        }
        let price = price_map.get(symbol).copied().unwrap_or(0);
        let side = if *qty >= 0 {
            BrokerSide::Buy
        } else {
            BrokerSide::Sell
        };
        broker_orders.push((*symbol, side, 0, price));
    }

    let current_positions: Vec<(Symbol, i64)> =
        current_qty.iter().map(|(sym, qty)| (*sym, *qty)).collect();

    engine.check_batch(&broker_orders, &account, &current_positions, targets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{Action, RebalanceOrder};

    fn default_risk_config() -> RiskConfig {
        RiskConfig {
            max_position_pct: 0.40,
            max_leverage: 1.5,
            min_trade_usd: 100.0,
            max_trade_usd: 100_000.0,
            allow_short: true,
            max_short_pct: 0.30,
        }
    }

    fn aapl() -> Symbol {
        Symbol::new("AAPL")
    }
    fn spy() -> Symbol {
        Symbol::new("SPY")
    }
    fn msft() -> Symbol {
        Symbol::new("MSFT")
    }

    #[test]
    fn all_pass_simple() {
        let orders = vec![RebalanceOrder {
            symbol: aapl(),
            action: Action::Buy,
            shares: 100,
            limit_price_cents: 18_500,
            notional_cents: 1_850_000,
            description: "open",
        }];

        let targets = vec![(aapl(), 0.30)];
        let prices = vec![(aapl(), 18_500)];
        let current: FxHashMap<Symbol, i64> = FxHashMap::default();

        let report = check_risk(
            &orders,
            10_000_000, // $100K
            &targets,
            &prices,
            &current,
            &default_risk_config(),
        );

        assert!(!report.has_failures());
    }

    #[test]
    fn fail_max_position() {
        let orders = vec![RebalanceOrder {
            symbol: aapl(),
            action: Action::Buy,
            shares: 500,
            limit_price_cents: 18_500,
            notional_cents: 9_250_000,
            description: "open",
        }];

        let targets = vec![(aapl(), 0.50)]; // 50% > 40% limit
        let prices = vec![(aapl(), 18_500)];
        let current: FxHashMap<Symbol, i64> = FxHashMap::default();

        let report = check_risk(
            &orders,
            10_000_000,
            &targets,
            &prices,
            &current,
            &default_risk_config(),
        );

        assert!(report.has_failures());
    }

    #[test]
    fn fail_short_not_allowed() {
        let mut config = default_risk_config();
        config.allow_short = false;

        let orders = vec![RebalanceOrder {
            symbol: spy(),
            action: Action::SellShort,
            shares: 50,
            limit_price_cents: 43_000,
            notional_cents: 2_150_000,
            description: "open short",
        }];

        let targets = vec![(spy(), -0.10)];
        let prices = vec![(spy(), 43_000)];
        let current: FxHashMap<Symbol, i64> = FxHashMap::default();

        let report = check_risk(&orders, 10_000_000, &targets, &prices, &current, &config);

        assert!(report.has_failures());
    }

    #[test]
    fn warn_max_trade_size() {
        let orders = vec![RebalanceOrder {
            symbol: aapl(),
            action: Action::Buy,
            shares: 1000,
            limit_price_cents: 18_500,
            notional_cents: 18_500_000, // $185K > $100K max
            description: "open",
        }];

        let targets = vec![(aapl(), 0.30)];
        let prices = vec![(aapl(), 18_500)];
        let current: FxHashMap<Symbol, i64> = FxHashMap::default();

        let report = check_risk(
            &orders,
            100_000_000,
            &targets,
            &prices,
            &current,
            &default_risk_config(),
        );

        assert!(report.has_warnings());
        assert!(!report.has_failures());
    }

    #[test]
    fn display_report() {
        let report = RiskReport {
            checks: vec![RiskCheck {
                name: "Test",
                status: RiskStatus::Pass,
                detail: "ok".into(),
            }],
        };
        let s = format!("{report}");
        assert!(s.contains("[PASS]"));
    }

    #[test]
    fn invalid_adapted_fields_fail_order_validation() {
        let cases = [
            {
                let mut cfg = default_risk_config();
                cfg.max_leverage = 0.5;
                cfg
            },
            {
                let mut cfg = default_risk_config();
                cfg.min_trade_usd = -1.0;
                cfg
            },
            {
                let mut cfg = default_risk_config();
                cfg.max_trade_usd = -1.0;
                cfg
            },
            {
                let mut cfg = default_risk_config();
                cfg.max_short_pct = -0.1;
                cfg
            },
        ];
        for cfg in cases {
            let report = check_risk(&[], 10_000_000, &[], &[], &FxHashMap::default(), &cfg);
            assert_eq!(report.checks.len(), 1);
            assert_eq!(report.checks[0].name, "Order validation");
            assert_eq!(report.checks[0].status, RiskStatus::Fail);
        }
    }

    #[test]
    fn disabled_batch_cap_does_not_fail_large_order() {
        let orders = vec![RebalanceOrder {
            symbol: aapl(),
            action: Action::Buy,
            shares: 10_000,
            limit_price_cents: 18_500,
            notional_cents: 185_000_000,
            description: "open",
        }];
        let cfg = RiskConfig {
            max_position_pct: 0.25,
            max_leverage: 1.5,
            min_trade_usd: 100.0,
            max_trade_usd: 10_000_000.0,
            allow_short: true,
            max_short_pct: 0.30,
        };
        let report = check_risk(
            &orders,
            1_000_000_000,
            &[(aapl(), 0.185)],
            &[(aapl(), 18_500)],
            &FxHashMap::default(),
            &cfg,
        );
        assert!(!report.has_failures());
        assert!(!report.has_warnings());
    }

    #[test]
    fn held_short_is_seeded_as_sell() {
        let mut cfg = default_risk_config();
        cfg.allow_short = false;
        let orders = vec![RebalanceOrder {
            symbol: aapl(),
            action: Action::Buy,
            shares: 10,
            limit_price_cents: 18_500,
            notional_cents: 185_000,
            description: "open",
        }];
        let mut current = FxHashMap::default();
        current.insert(spy(), -50);
        let report = check_risk(
            &orders,
            100_000_000,
            &[(aapl(), 0.1)],
            &[(aapl(), 18_500), (spy(), 43_000)],
            &current,
            &cfg,
        );
        assert!(
            report
                .checks
                .iter()
                .any(|c| c.name == "Short selling" && c.status == RiskStatus::Fail)
        );
    }

    #[test]
    fn held_long_is_seeded_into_leverage() {
        let orders = vec![RebalanceOrder {
            symbol: msft(),
            action: Action::Buy,
            shares: 1,
            limit_price_cents: 100,
            notional_cents: 100,
            description: "open",
        }];
        let mut current = FxHashMap::default();
        current.insert(aapl(), 1000);
        let report = check_risk(
            &orders,
            10_000_000,
            &[(msft(), 0.001)],
            &[(msft(), 100), (aapl(), 18_500)],
            &current,
            &default_risk_config(),
        );
        assert!(
            report
                .checks
                .iter()
                .any(|c| c.name == "Leverage" && c.status == RiskStatus::Fail)
        );
    }
}
