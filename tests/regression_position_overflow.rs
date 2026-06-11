//! Regression: integer overflow in Position::market_value / unrealized_pnl.
//!
//! Fuzz harness fuzz_execute_fill (added 2026-05-19) crashed at
//! `position.rs:95` with `attempt to multiply with overflow`. Cause:
//! plain `*` on i64 product of `quantity * price`. Reachable via
//! Portfolio::rebalance_simple. Fixed in v0.16.1 by switching to
//! saturating_mul throughout position.rs.
//!
//! This test asserts that Position never panics on extreme inputs.

#![cfg(feature = "portfolio")]
// Prices use dollar.cents grouping: 1_000_000_00 = $1,000,000.00.
#![allow(clippy::inconsistent_digit_grouping)]

use nanobook::Symbol;
use nanobook::portfolio::{CostModel, Portfolio, Position};

#[test]
fn market_value_does_not_panic_on_large_quantity_times_price() {
    let sym = Symbol::new("FZ0");
    let mut pos = Position::new(sym);
    // Fill that puts quantity near 2B so quantity * price overflows i64
    // for any price > ~4.6 billion cents.
    pos.apply_fill(2_000_000_000, 5_000_000_000);

    // Was previously a panic; now must return a saturated i64.
    let mv = pos.market_value(5_000_000_000);
    assert!(
        mv == i64::MAX || mv == i64::MIN,
        "expected saturation to i64 extreme, got {mv}"
    );
}

#[test]
fn unrealized_pnl_does_not_panic_on_large_inputs() {
    let sym = Symbol::new("FZ0");
    let mut pos = Position::new(sym);
    pos.apply_fill(2_000_000_000, 5_000_000_000);

    // Must not panic. Specific value depends on saturation but should be saturated.
    let _pnl = pos.unrealized_pnl(6_000_000_000);
}

#[test]
fn apply_fill_does_not_panic_on_overflow() {
    let sym = Symbol::new("FZ0");
    let mut pos = Position::new(sym);
    // Sequence that would have overflowed total_cost accumulation
    pos.apply_fill(1_000_000_000, 1_000_000_000);
    pos.apply_fill(1_000_000_000, 1_000_000_000);
    // Just assert it didn't panic
    assert!(pos.quantity > 0);
}

#[test]
fn replays_crash_artifact_330fb9() {
    // [Inference] Replay of the crashing input found by libfuzzer.
    // Artifact: fuzz/artifacts/fuzz_execute_fill/crash-330fb9097052e84d78571d06b840334680573582
    // The artifact is libfuzzer-encoded bytes; this test reconstructs the same
    // call sequence by hand to keep it independent of the fuzz harness.

    let cost_model = CostModel {
        commission_bps: 0.0,
        slippage_bps: 0.0,
        min_commission: 0,
    };
    let mut portfolio = Portfolio::new(1_000_000_000_000, cost_model);
    let sym = Symbol::new("FZ0");

    // Two large fills via rebalance_simple. Should not panic.
    portfolio.rebalance_simple(&[(sym, 1.0)], &[(sym, 1_000_000_00)]);
    portfolio.rebalance_simple(&[(sym, 1.0)], &[(sym, 100_000_000_00)]);
}

#[test]
fn total_equity_does_not_panic_on_large_holdings() {
    // Regression: src/portfolio/mod.rs:430-435 — bare `.sum()` over
    // `pos.market_value(price)` and bare `self.cash + position_value`
    // could panic when many positions accumulated to a near-i64::MAX sum,
    // OR when cash + sum exceeded i64::MAX.
    // Fix: saturating_add in the fold and the final sum.
    let cost_model = CostModel {
        commission_bps: 0.0,
        slippage_bps: 0.0,
        min_commission: 0,
    };
    let mut portfolio = Portfolio::new(i64::MAX / 2, cost_model);

    let sym_a = Symbol::new("FZ0");
    let sym_b = Symbol::new("FZ1");

    // Acquire positions with large quantity × price products
    portfolio.rebalance_simple(
        &[(sym_a, 0.5), (sym_b, 0.5)],
        &[(sym_a, 1_000_000_00), (sym_b, 1_000_000_00)],
    );

    // total_equity_from_price_map must not panic when each position
    // market_value saturates under extreme prices.
    portfolio.rebalance_simple(
        &[(sym_a, 0.5), (sym_b, 0.5)],
        &[(sym_a, 100_000_000_00), (sym_b, 100_000_000_00)],
    );

    // No panic == pass.
}

#[test]
fn rebalance_simple_does_not_panic_on_negative_extremes() {
    // Some crash inputs in fuzz/artifacts/.../crash-b794695e* and
    // crash-6bafbd45* drove rebalance through configurations where
    // intermediate i64 diffs would have underflowed. Smoke-test a
    // sequence that historically tripped this.
    let cost_model = CostModel {
        commission_bps: 100.0,
        slippage_bps: 100.0,
        min_commission: 0,
    };
    let mut portfolio = Portfolio::new(1_000_000_000_000, cost_model);
    let sym = Symbol::new("FZ0");

    // Sequence of rebalances at varying weights with extreme prices
    for (weight, price) in [
        (1.0_f64, 1_000_000_00_i64),
        (0.0, 100_000_000_00),
        (1.0, 1_000),
        (0.5, 1_000_000_000_00),
    ] {
        portfolio.rebalance_simple(&[(sym, weight)], &[(sym, price)]);
    }
}
