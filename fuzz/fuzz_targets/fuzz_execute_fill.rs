#![no_main]
//! Fuzz target for Portfolio::execute_fill (v0.16.0+).
//!
//! `execute_fill` is private; it is driven indirectly via
//! `Portfolio::rebalance_simple`, which is the only stable public entry point
//! that calls it.  Every rebalance call exercises the slippage-as-price-impact
//! path added in v0.16.0.
//!
//! Invariants checked on every run:
//! 1. No panic. Any panic from execute_fill or compute_cost fails the run.
//! 2. compute_cost(notional) >= 0 for a set of probe notionals.
//!
//! NOT run in CI. Use `cargo +nightly fuzz run fuzz_execute_fill` locally.

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use nanobook::portfolio::{CostModel, Portfolio};
use nanobook::Symbol;

/// Bounded CostModel inputs — wider than realistic but tight enough to avoid
/// trivially exhausting i64 cash via degenerate slippage_bps values.
#[derive(Debug, Arbitrary)]
struct FuzzCostModel {
    commission_bps: f64,
    slippage_bps: f64,
    min_commission: i64,
}

impl FuzzCostModel {
    /// Convert into a valid CostModel, applying bounds that the public API
    /// otherwise lets the caller violate.
    fn to_cost_model(&self) -> Option<CostModel> {
        // Reject NaN / inf bps values — the caller contract assumes finite.
        if !self.commission_bps.is_finite() || !self.slippage_bps.is_finite() {
            return None;
        }
        // Reject negative bps (would be silly but fuzzer would explore).
        if self.commission_bps < 0.0 || self.slippage_bps < 0.0 {
            return None;
        }
        // Cap bps at a generous 100_000 (1000% — already absurd). Above this,
        // slippage as price impact would overflow even the largest realistic
        // price. Fuzzer should explore the meaningful range, not bug-by-overflow.
        if self.commission_bps > 100_000.0 || self.slippage_bps > 100_000.0 {
            return None;
        }
        // min_commission must be non-negative.
        if self.min_commission < 0 {
            return None;
        }
        Some(CostModel {
            commission_bps: self.commission_bps,
            slippage_bps: self.slippage_bps,
            min_commission: self.min_commission,
        })
    }
}

#[derive(Debug, Arbitrary)]
struct FuzzFillInput {
    initial_cash: i64,
    cost_model: FuzzCostModel,
    fills: Vec<FuzzFill>,
}

#[derive(Debug, Arbitrary)]
struct FuzzFill {
    symbol_id: u8, // pick from a small pool of symbols
    weight: u8,    // target weight as 0-100 (clamped to 0.0-1.0)
    price: i64,
}

fuzz_target!(|input: FuzzFillInput| {
    // Reject inputs that bypass the documented API contract.
    let cost_model = match input.cost_model.to_cost_model() {
        Some(m) => m,
        None => return,
    };

    // Bound initial cash to a realistic range. Fuzzer can still explore
    // extreme but representable values within this range.
    if input.initial_cash < 0 || input.initial_cash > 1_000_000_000_000 {
        return; // up to $10B
    }

    let mut portfolio = Portfolio::new(input.initial_cash, cost_model);

    for fill in input.fills.iter().take(50) {
        // Bound price to representable values: $0.01 — $1M per share (cents)
        if fill.price <= 0 || fill.price > 1_000_000_00 {
            continue;
        }

        // Construct symbol from a small pool (FZ0..FZ9) to allow position
        // accumulation and closure across iterations.
        let sym_name = format!("FZ{}", fill.symbol_id % 10);
        let symbol = Symbol::new(sym_name.as_str());

        // Convert u8 weight to a 0.0-1.0 float (0 = 0%, 100 = 100%).
        let target_weight = (fill.weight.min(100) as f64) / 100.0;

        let targets = [(symbol, target_weight)];
        let prices = [(symbol, fill.price)];

        // [Inference] execute_fill is private; rebalance_simple is the only
        // stable public path that exercises it (confirmed by reading mod.rs).
        // Invariant: must not panic on bounded inputs.
        portfolio.rebalance_simple(&targets, &prices);
    }

    // Invariant: compute_cost(notional) is non-negative for any notional we
    // can construct without overflow. Probe a few sample notionals.
    for &notional in &[0_i64, 1, 100_00, 1_000_000_00] {
        let cost = portfolio.cost_model().compute_cost(notional);
        assert!(
            cost >= 0,
            "compute_cost returned negative: notional={notional}, cost={cost}"
        );
    }
});
