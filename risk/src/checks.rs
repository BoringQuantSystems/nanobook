//! Individual risk check implementations.

use nanobook::Symbol;
use nanobook_broker::{Account, BrokerSide};
use rustc_hash::FxHashMap;

use crate::config::RiskConfig;
use crate::report::{RiskCheck, RiskReport, RiskStatus};

/// Returns `"<="` if the check passed, `">"` if it failed or warned.
fn cmp_symbol(status: RiskStatus) -> &'static str {
    if status == RiskStatus::Pass {
        "<="
    } else {
        ">"
    }
}

/// Ratio of `numerator / equity`, or `INFINITY` when equity is non-positive.
fn ratio_or_inf(numerator: i64, equity: i64) -> f64 {
    if equity > 0 {
        numerator as f64 / equity as f64
    } else {
        f64::INFINITY
    }
}

/// Run all risk checks for a batch of orders.
pub fn check_batch(
    config: &RiskConfig,
    orders: &[(Symbol, BrokerSide, u64, i64)],
    account: &Account,
    current_positions: &[(Symbol, i64)],
    target_weights: &[(Symbol, f64)],
) -> RiskReport {
    let equity = account.equity_cents;
    let target_map: FxHashMap<Symbol, f64> = target_weights.iter().copied().collect();
    let mut checks = Vec::new();

    // 1. Max position check — no single target weight > max_position_pct
    let max_pos = config.max_position_pct;
    let mut worst_pos = 0.0_f64;
    let mut worst_sym = Symbol::new("?");
    for &(sym, weight) in target_weights {
        let abs_w = weight.abs();
        if abs_w > worst_pos {
            worst_pos = abs_w;
            worst_sym = sym;
        }
    }
    let pos_status = if worst_pos > max_pos {
        RiskStatus::Fail
    } else {
        RiskStatus::Pass
    };
    checks.push(RiskCheck {
        name: "Max position",
        status: pos_status,
        detail: format!(
            "{:.1}% ({}) {} {:.1}% limit",
            worst_pos * 100.0,
            worst_sym.as_str(),
            cmp_symbol(pos_status),
            max_pos * 100.0,
        ),
    });

    // 2. Leverage check — post-trade gross exposure / equity
    let mut post_qty: FxHashMap<Symbol, i64> = current_positions.iter().copied().collect();
    let mut price_map: FxHashMap<Symbol, i64> = FxHashMap::default();
    for &(sym, side, qty, price) in orders {
        let sign = match side {
            BrokerSide::Buy => 1_i64,
            BrokerSide::Sell => -1,
        };
        let qty_i64 = i64::try_from(qty).unwrap_or(i64::MAX);
        *post_qty.entry(sym).or_insert(0) += sign.saturating_mul(qty_i64);
        price_map.insert(sym, price);
    }

    // Compute exposure: sum of |qty| * |price| for each position.
    // When `filter` is None, includes all positions (gross); when Some(f),
    // includes only positions where qty satisfies f.
    let exposure = |filter: Option<fn(&i64) -> bool>| -> i64 {
        post_qty
            .iter()
            .filter(|(_, qty)| filter.is_none_or(|f| f(qty)))
            .map(|(sym, qty)| {
                let price = price_map.get(sym).copied().unwrap_or(0).saturating_abs();
                qty.saturating_abs().saturating_mul(price)
            })
            .fold(0_i64, |acc, v| acc.saturating_add(v))
    };

    let gross_exposure = exposure(None);
    let leverage = ratio_or_inf(gross_exposure, equity);
    let lev_status = if leverage > config.max_leverage {
        RiskStatus::Fail
    } else {
        RiskStatus::Pass
    };
    checks.push(RiskCheck {
        name: "Leverage",
        status: lev_status,
        detail: format!(
            "{:.2}x {} {:.2}x limit",
            leverage,
            cmp_symbol(lev_status),
            config.max_leverage,
        ),
    });

    // 3. Short exposure check
    let short_exposure = exposure(Some(|qty: &i64| *qty < 0));
    let short_pct = ratio_or_inf(short_exposure, equity);

    let has_shorts = orders
        .iter()
        .any(|(_, side, _, _)| *side == BrokerSide::Sell)
        && post_qty.values().any(|q| *q < 0);

    if has_shorts && !config.allow_short {
        checks.push(RiskCheck {
            name: "Short selling",
            status: RiskStatus::Fail,
            detail: "short selling not allowed".into(),
        });
    } else {
        let short_status = if short_pct > config.max_short_pct {
            RiskStatus::Fail
        } else {
            RiskStatus::Pass
        };
        checks.push(RiskCheck {
            name: "Short exposure",
            status: short_status,
            detail: format!(
                "{:.1}% {} {:.1}% limit",
                short_pct * 100.0,
                cmp_symbol(short_status),
                config.max_short_pct * 100.0,
            ),
        });
    }

    // 4. Max order value in cents
    let mut batch_value = 0_i64;
    for &(sym, _side, qty, price) in orders {
        let qty_i64 = i64::try_from(qty).unwrap_or(i64::MAX);
        let notional = qty_i64.saturating_mul(price.saturating_abs());
        batch_value = batch_value.saturating_add(notional);

        let max_order = config.max_order_value_cents;
        if max_order > 0 && notional > max_order {
            checks.push(RiskCheck {
                name: "Max order value",
                status: RiskStatus::Fail,
                detail: format!(
                    "{}: ${:.0} > ${:.0} max_order_value_cents",
                    sym,
                    notional as f64 / 100.0,
                    max_order as f64 / 100.0,
                ),
            });
        }
    }

    // 5. Max rebalance batch value in cents
    let max_batch = config.max_batch_value_cents;
    if max_batch > 0 && batch_value > max_batch {
        checks.push(RiskCheck {
            name: "Max batch value",
            status: RiskStatus::Fail,
            detail: format!(
                "${:.0} > ${:.0} max_batch_value_cents",
                batch_value as f64 / 100.0,
                max_batch as f64 / 100.0,
            ),
        });
    }

    // 6. Max trade size — warn if any trade > max_trade_usd
    let max_cents = (config.max_trade_usd * 100.0) as i64;
    for &(sym, _side, qty, price) in orders {
        let qty_i64 = i64::try_from(qty).unwrap_or(i64::MAX);
        let notional = qty_i64.saturating_mul(price.saturating_abs());
        if notional > max_cents {
            checks.push(RiskCheck {
                name: "Max trade size",
                status: RiskStatus::Warn,
                detail: format!(
                    "{}: ${:.0} > ${:.0} max_trade_usd",
                    sym,
                    notional as f64 / 100.0,
                    config.max_trade_usd,
                ),
            });
        }
    }

    // 7. Order count
    checks.push(RiskCheck {
        name: "Order count",
        status: RiskStatus::Pass,
        detail: format!("{} orders", orders.len()),
    });

    // 8. Target weights sanity
    let long_sum: f64 = target_map.values().filter(|w| **w > 0.0).sum();
    let short_sum: f64 = target_map
        .values()
        .filter(|w| **w < 0.0)
        .map(|w| w.abs())
        .sum();
    checks.push(RiskCheck {
        name: "Weight allocation",
        status: RiskStatus::Pass,
        detail: format!(
            "{:.1}% long, {:.1}% short, {:.1}% cash",
            long_sum * 100.0,
            short_sum * 100.0,
            (1.0 - long_sum) * 100.0,
        ),
    });

    RiskReport { checks }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aapl() -> Symbol {
        Symbol::new("AAPL")
    }
    fn spy() -> Symbol {
        Symbol::new("SPY")
    }

    fn default_config() -> RiskConfig {
        RiskConfig {
            max_position_pct: 0.40,
            max_leverage: 1.5,
            allow_short: true,
            max_short_pct: 0.30,
            max_trade_usd: 100_000.0,
            ..RiskConfig::default()
        }
    }

    fn account(equity: i64) -> Account {
        Account {
            equity_cents: equity,
            buying_power_cents: equity,
            cash_cents: equity,
            gross_position_value_cents: 0,
        }
    }

    #[test]
    fn all_pass_simple() {
        let orders = vec![(aapl(), BrokerSide::Buy, 100_u64, 18_500_i64)];
        let targets = vec![(aapl(), 0.30)];
        let report = check_batch(
            &default_config(),
            &orders,
            &account(10_000_000),
            &[],
            &targets,
        );
        assert!(!report.has_failures());
    }

    #[test]
    fn fail_max_position() {
        let orders = vec![(aapl(), BrokerSide::Buy, 500, 18_500)];
        let targets = vec![(aapl(), 0.50)]; // 50% > 40%
        let report = check_batch(
            &default_config(),
            &orders,
            &account(10_000_000),
            &[],
            &targets,
        );
        assert!(report.has_failures());
    }

    #[test]
    fn fail_short_not_allowed() {
        let mut config = default_config();
        config.allow_short = false;

        let orders = vec![(spy(), BrokerSide::Sell, 50, 43_000)];
        let targets = vec![(spy(), -0.10)];
        let report = check_batch(&config, &orders, &account(10_000_000), &[], &targets);
        assert!(report.has_failures());
    }

    #[test]
    fn warn_large_trade() {
        let orders = vec![(aapl(), BrokerSide::Buy, 1000, 18_500)];
        let targets = vec![(aapl(), 0.30)];
        let mut config = default_config();
        config.max_order_value_cents = 20_000_000; // keep order-level hard cap above max trade warning
        let report = check_batch(&config, &orders, &account(100_000_000), &[], &targets);
        assert!(report.has_warnings());
        assert!(!report.has_failures());
    }

    #[test]
    fn fails_max_order_value() {
        let orders = vec![(aapl(), BrokerSide::Buy, 120, 15_000)];
        let targets = vec![(aapl(), 0.30)];
        let mut config = default_config();
        config.max_order_value_cents = 10_000; // $100

        let report = check_batch(&config, &orders, &account(10_000_000), &[], &targets);

        assert!(report.has_failures());
    }

    #[test]
    fn passes_max_order_value_boundary() {
        let orders = vec![(aapl(), BrokerSide::Buy, 2, 5_000)];
        let targets = vec![(aapl(), 0.30)];
        let mut config = default_config();
        config.max_order_value_cents = 10_000; // $100

        let report = check_batch(&config, &orders, &account(10_000_000), &[], &targets);

        assert!(!report.has_failures());
        assert!(!report.checks.iter().any(|c| c.name == "Max order value"));
    }

    #[test]
    fn boundary_checks_include_failure_messages() {
        let orders = vec![(aapl(), BrokerSide::Buy, 3, 5_000)];
        let targets = vec![(aapl(), 0.30)];
        let mut config = default_config();
        config.max_order_value_cents = 10_000; // $100

        let report = check_batch(&config, &orders, &account(10_000_000), &[], &targets);

        assert!(report.has_failures());
        let order_limit = report
            .checks
            .iter()
            .find(|c| c.name == "Max order value")
            .unwrap();
        assert_eq!(order_limit.status, RiskStatus::Fail);
        assert!(
            order_limit
                .detail
                .contains("$150 > $100 max_order_value_cents")
        );
    }

    #[test]
    fn fails_max_batch_value() {
        let orders = vec![
            (aapl(), BrokerSide::Buy, 40, 18_500),
            (spy(), BrokerSide::Sell, 40, 18_500),
        ];
        let targets = vec![(aapl(), 0.30), (spy(), -0.30)];
        let mut config = default_config();
        config.max_batch_value_cents = 10_000; // $100

        let report = check_batch(&config, &orders, &account(10_000_000), &[], &targets);

        assert!(report.has_failures());
    }

    #[test]
    fn passes_max_batch_value_boundary() {
        let orders = vec![
            (aapl(), BrokerSide::Buy, 4, 2500),
            (spy(), BrokerSide::Sell, 4, 0),
        ];
        let targets = vec![(aapl(), 0.30), (spy(), -0.30)];
        let mut config = default_config();
        config.max_batch_value_cents = 10_000; // $100

        let report = check_batch(&config, &orders, &account(10_000_000), &[], &targets);

        assert!(!report.has_failures());
        assert!(!report.checks.iter().any(|c| c.name == "Max batch value"));
    }

    #[test]
    fn fails_batch_value_with_report_name() {
        let orders = vec![
            (aapl(), BrokerSide::Buy, 25, 500),
            (spy(), BrokerSide::Sell, 25, 500),
        ];
        let targets = vec![(aapl(), 0.30), (spy(), -0.30)];
        let mut config = default_config();
        config.max_batch_value_cents = 10_000; // $100

        let report = check_batch(&config, &orders, &account(10_000_000), &[], &targets);

        assert!(report.has_failures());
        let batch_limit = report
            .checks
            .iter()
            .find(|c| c.name == "Max batch value")
            .unwrap();
        assert_eq!(batch_limit.status, RiskStatus::Fail);
        assert!(
            batch_limit
                .detail
                .contains("$250 > $100 max_batch_value_cents")
        );
    }

    fn find<'a>(report: &'a RiskReport, name: &str) -> Option<&'a RiskCheck> {
        report.checks.iter().find(|c| c.name == name)
    }

    fn named<'a>(report: &'a RiskReport, name: &str) -> &'a RiskCheck {
        find(report, name).unwrap_or_else(|| panic!("no check named {name:?} in {report}"))
    }

    // ---- Leverage check (cargo-mutants 0.20.0: never failed in any test) ----

    #[test]
    fn leverage_above_limit_fails() {
        // gross 500 * 18_500 = 9_250_000 over equity 5_000_000 = 1.85x > 1.5x
        let orders = vec![(aapl(), BrokerSide::Buy, 500_u64, 18_500_i64)];
        let targets = vec![(aapl(), 0.40)];
        let report = check_batch(
            &default_config(),
            &orders,
            &account(5_000_000),
            &[],
            &targets,
        );
        let lev = named(&report, "Leverage");
        assert_eq!(lev.status, RiskStatus::Fail);
        assert!(lev.detail.contains("1.85x > 1.50x limit"), "{}", lev.detail);
    }

    #[test]
    fn leverage_exactly_at_limit_passes() {
        // gross 1000 * 15_000 = 15_000_000 over equity 10_000_000 = 1.50x
        let mut config = default_config();
        config.max_order_value_cents = 0; // disabled, so only leverage is under test
        let orders = vec![(aapl(), BrokerSide::Buy, 1000_u64, 15_000_i64)];
        let report = check_batch(&config, &orders, &account(10_000_000), &[], &[]);
        let lev = named(&report, "Leverage");
        assert_eq!(lev.status, RiskStatus::Pass);
        assert!(
            lev.detail.contains("1.50x <= 1.50x limit"),
            "{}",
            lev.detail
        );
    }

    #[test]
    fn zero_equity_fails_leverage_and_short_exposure() {
        // ratio_or_inf returns infinity for equity <= 0, also for 0 / 0
        let report = check_batch(&default_config(), &[], &account(0), &[], &[]);
        let lev = named(&report, "Leverage");
        assert_eq!(lev.status, RiskStatus::Fail);
        assert!(lev.detail.contains("infx > 1.50x limit"), "{}", lev.detail);
        assert_eq!(named(&report, "Short exposure").status, RiskStatus::Fail);
    }

    // ---- Short-selling logic ----

    #[test]
    fn selling_down_a_long_is_not_short_selling() {
        let mut config = default_config();
        config.allow_short = false;
        let current = vec![(aapl(), 100_i64)];
        // Sell 50 leaves 50 long; Sell 100 leaves exactly flat.
        for qty in [50_u64, 100] {
            let orders = vec![(aapl(), BrokerSide::Sell, qty, 18_500_i64)];
            let report = check_batch(&config, &orders, &account(10_000_000), &current, &[]);
            assert!(!report.has_failures(), "qty {qty}: {report}");
            assert!(find(&report, "Short selling").is_none(), "qty {qty}");
            assert_eq!(named(&report, "Short exposure").status, RiskStatus::Pass);
        }
    }

    #[test]
    fn short_exposure_counts_only_short_positions() {
        // Only SPY is short: 50 * 43_000 = 2_150_000 over 10_000_000 = 21.5%.
        // The long AAPL (18.5%) must not be counted, and the short must be.
        let mut config = default_config();
        config.max_short_pct = 0.20;
        let orders = vec![
            (aapl(), BrokerSide::Buy, 100_u64, 18_500_i64),
            (spy(), BrokerSide::Sell, 50, 43_000),
        ];
        let report = check_batch(&config, &orders, &account(10_000_000), &[], &[]);
        let short = named(&report, "Short exposure");
        assert_eq!(short.status, RiskStatus::Fail);
        assert!(
            short.detail.contains("21.5% > 20.0% limit"),
            "{}",
            short.detail
        );
    }

    #[test]
    fn short_exposure_boundary() {
        let config = default_config(); // max_short_pct 0.30
        // exactly 3_000_000 / 10_000_000 = 30.0%: passes
        let at = vec![(spy(), BrokerSide::Sell, 100_u64, 30_000_i64)];
        let report = check_batch(&config, &at, &account(10_000_000), &[], &[]);
        let short = named(&report, "Short exposure");
        assert_eq!(short.status, RiskStatus::Pass);
        assert!(
            short.detail.contains("30.0% <= 30.0% limit"),
            "{}",
            short.detail
        );
        // 3_030_000 / 10_000_000 = 30.3%: fails
        let over = vec![(spy(), BrokerSide::Sell, 101_u64, 30_000_i64)];
        let report = check_batch(&config, &over, &account(10_000_000), &[], &[]);
        assert_eq!(named(&report, "Short exposure").status, RiskStatus::Fail);
    }

    // ---- Limits at their boundary and disabled at 0 ----

    #[test]
    fn target_weight_exactly_at_max_position_passes() {
        let targets = vec![(aapl(), 0.40)]; // default_config max_position_pct is 0.40
        let report = check_batch(&default_config(), &[], &account(10_000_000), &[], &targets);
        assert_eq!(named(&report, "Max position").status, RiskStatus::Pass);
    }

    #[test]
    fn zero_order_and_batch_limits_mean_no_limit() {
        let mut config = default_config();
        config.max_order_value_cents = 0;
        config.max_batch_value_cents = 0;
        let orders = vec![(aapl(), BrokerSide::Buy, 1000_u64, 18_500_i64)];
        let report = check_batch(&config, &orders, &account(100_000_000), &[], &[]);
        assert!(find(&report, "Max order value").is_none());
        assert!(find(&report, "Max batch value").is_none());
    }

    #[test]
    fn max_trade_size_warning_threshold() {
        let mut config = default_config();
        config.max_trade_usd = 1000.0; // 100_000 cents
        // notional exactly 100_000 cents: no warning
        let at = vec![(aapl(), BrokerSide::Buy, 10_u64, 10_000_i64)];
        let report = check_batch(&config, &at, &account(100_000_000), &[], &[]);
        assert!(find(&report, "Max trade size").is_none());
        // notional 100_001 cents: one warning
        let over = vec![(aapl(), BrokerSide::Buy, 1_u64, 100_001_i64)];
        let report = check_batch(&config, &over, &account(100_000_000), &[], &[]);
        assert_eq!(named(&report, "Max trade size").status, RiskStatus::Warn);
    }

    // ---- Detail strings: worst-weight symbol and weight allocation ----

    #[test]
    fn max_position_detail_names_the_first_of_tied_weights() {
        let tied = vec![(aapl(), 0.20), (spy(), 0.20)];
        let report = check_batch(&default_config(), &[], &account(10_000_000), &[], &tied);
        assert_eq!(
            named(&report, "Max position").detail,
            "20.0% (AAPL) <= 40.0% limit"
        );

        // A zero weight is not larger than the starting value 0.0, so the
        // placeholder symbol stays.
        let zero = vec![(aapl(), 0.0)];
        let report = check_batch(&default_config(), &[], &account(10_000_000), &[], &zero);
        assert_eq!(
            named(&report, "Max position").detail,
            "0.0% (?) <= 40.0% limit"
        );
    }

    #[test]
    fn weight_allocation_splits_long_short_and_cash() {
        let msft = Symbol::new("MSFT");
        let targets = vec![(aapl(), 0.50), (spy(), -0.20), (msft, 0.0)];
        let report = check_batch(&default_config(), &[], &account(10_000_000), &[], &targets);
        let check = named(&report, "Weight allocation");
        assert_eq!(check.status, RiskStatus::Pass);
        assert_eq!(check.detail, "50.0% long, 20.0% short, 50.0% cash");
    }
}
