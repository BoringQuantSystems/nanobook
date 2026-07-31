//! Portfolio management: position tracking, cost modeling, and financial metrics.
//!
//! The portfolio layer sits on top of the LOB infrastructure. It supports two
//! execution modes:
//!
//! - **SimpleFill**: Instant execution at specified prices (for fast parameter sweeps)
//! - **LOBFill**: Route orders through actual `Exchange` matching engines (for microstructure)
//!
//! # Example
//!
//! ```
//! use nanobook::portfolio::{Portfolio, CostModel};
//! use nanobook::Symbol;
//!
//! let mut portfolio = Portfolio::new(1_000_000_00, CostModel::zero()); // $1M
//!
//! // Rebalance to 60% AAPL, 40% MSFT at current prices
//! let targets = [(Symbol::new("AAPL"), 0.6), (Symbol::new("MSFT"), 0.4)];
//! let prices = [(Symbol::new("AAPL"), 150_00), (Symbol::new("MSFT"), 300_00)];
//! portfolio.rebalance_simple(&targets, &prices);
//! portfolio.record_return(&prices);
//!
//! let snapshot = portfolio.snapshot(&prices);
//! assert_eq!(snapshot.num_positions, 2);
//! assert!(snapshot.equity > 0);
//! ```

pub mod cost_model;
pub mod metrics;
pub mod position;
pub mod strategy;
#[cfg(feature = "parallel")]
pub mod sweep;

pub use cost_model::CostModel;
pub use metrics::{Metrics, compute_metrics};
pub use position::{Position, Shares};
pub use strategy::{BacktestResult, EqualWeight, Strategy, run_backtest};

use crate::types::Symbol;
use rustc_hash::FxHashMap;

/// Serde helper for `FxHashMap<Symbol, Position>` — serializes as `Vec<(Symbol, Position)>`.
#[cfg(feature = "serde")]
mod serde_positions {
    use super::{FxHashMap, Position, Symbol};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(
        map: &FxHashMap<Symbol, Position>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut vec: Vec<(&Symbol, &Position)> = map.iter().collect();
        vec.sort_by_key(|(sym, _)| *sym);
        vec.serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<FxHashMap<Symbol, Position>, D::Error> {
        let vec: Vec<(Symbol, Position)> = Vec::deserialize(deserializer)?;
        Ok(vec.into_iter().collect())
    }
}

/// A portfolio tracking cash, positions, returns, and equity.
///
/// All monetary values (cash, equity) are in the smallest currency unit (cents).
///
/// ```
/// use nanobook::portfolio::{CostModel, Portfolio, Shares};
/// use nanobook::Symbol;
///
/// let mut portfolio = Portfolio::new(100_000_00, CostModel::zero());
/// let aapl = Symbol::new("AAPL");
///
/// portfolio.rebalance_simple(&[(aapl, 0.50)], &[(aapl, 200_00)]);
///
/// let position = portfolio.position(&aapl).unwrap();
/// assert_eq!(position.quantity, Shares::from_whole(250));
/// assert_eq!(portfolio.current_weights(&[(aapl, 200_00)])[0].1, 0.5);
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Portfolio {
    /// Cash balance (cents)
    cash: i64,
    /// Positions indexed by symbol
    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "serde_positions::serialize",
            deserialize_with = "serde_positions::deserialize"
        )
    )]
    positions: FxHashMap<Symbol, Position>,
    /// Cost model applied to each trade
    cost_model: CostModel,
    /// Series of periodic returns (for metrics computation)
    returns: Vec<f64>,
    /// Equity curve (total portfolio value at each snapshot)
    equity_curve: Vec<i64>,
    /// Previous equity for return calculation
    prev_equity: i64,
    /// Order sizing granularity, in micro-shares (see [`Shares`]). Positions are
    /// always sized to a multiple of this step. Defaults to `Shares::SCALE`
    /// (whole shares), which reproduces pre-fractional-share behaviour exactly.
    #[cfg_attr(feature = "serde", serde(default = "default_quantity_step"))]
    quantity_step: i64,
}

/// Serde default for `Portfolio::quantity_step`: whole shares, so JSON saved
/// before this field existed still loads with pre-fractional-share behaviour.
#[cfg(feature = "serde")]
fn default_quantity_step() -> i64 {
    Shares::SCALE
}

impl Portfolio {
    /// Create a new portfolio with initial cash and cost model.
    ///
    /// `initial_cash` is in cents (e.g., `1_000_000_00` = $1,000,000).
    /// Negative initial cash is a programming error (use `debug_assert`).
    ///
    /// Order sizing defaults to whole shares (`quantity_step = Shares::SCALE`).
    /// Use [`Portfolio::set_quantity_step`] or [`Portfolio::with_quantity_step`]
    /// to size at fractional-share granularity instead.
    pub fn new(initial_cash: i64, cost_model: CostModel) -> Self {
        debug_assert!(
            initial_cash >= 0,
            "initial_cash must be non-negative, got {initial_cash}"
        );
        Self {
            cash: initial_cash,
            positions: FxHashMap::default(),
            cost_model,
            returns: Vec::new(),
            equity_curve: vec![initial_cash],
            prev_equity: initial_cash,
            quantity_step: Shares::SCALE,
        }
    }

    /// Builder-style variant of [`Portfolio::new`] with an explicit sizing step
    /// (micro-shares). Useful values: `1_000_000` (whole shares, the default),
    /// `1_000` (Alpaca's 0.001-share minimum), `100` (IBKR's 0.0001-share
    /// minimum), `1` (effectively continuous).
    pub fn with_quantity_step(initial_cash: i64, cost_model: CostModel, step: i64) -> Self {
        let mut p = Self::new(initial_cash, cost_model);
        p.set_quantity_step(step);
        p
    }

    /// Set the order sizing granularity (micro-shares). Must be positive;
    /// values `<= 0` are a programming error (use `debug_assert`).
    pub fn set_quantity_step(&mut self, step: i64) {
        debug_assert!(step > 0, "quantity_step must be positive, got {step}");
        self.quantity_step = step;
    }

    /// The current order sizing granularity (micro-shares).
    #[inline]
    pub fn quantity_step(&self) -> i64 {
        self.quantity_step
    }

    // === Queries ===

    /// Current cash balance (cents).
    #[inline]
    pub fn cash(&self) -> i64 {
        self.cash
    }

    /// Get a position by symbol, if it exists.
    pub fn position(&self, symbol: &Symbol) -> Option<&Position> {
        self.positions.get(symbol)
    }

    /// Iterator over all positions.
    pub fn positions(&self) -> impl Iterator<Item = (&Symbol, &Position)> {
        self.positions.iter()
    }

    /// Total equity: cash + sum of all position market values.
    ///
    /// `prices` maps symbols to current prices (cents).
    pub fn total_equity(&self, prices: &[(Symbol, i64)]) -> i64 {
        let price_map: FxHashMap<Symbol, i64> = prices.iter().copied().collect();
        self.total_equity_from_price_map(&price_map)
    }

    /// Current portfolio weights as (symbol, weight) pairs.
    ///
    /// Weights are fractions of total equity. Cash is not included
    /// (it's implicitly `1 - sum(weights)`).
    pub fn current_weights(&self, prices: &[(Symbol, i64)]) -> Vec<(Symbol, f64)> {
        let price_map: FxHashMap<Symbol, i64> = prices.iter().copied().collect();
        let equity = self.total_equity_from_price_map(&price_map);
        self.current_weights_from_price_map(&price_map, equity)
    }

    /// The accumulated return series.
    pub fn returns(&self) -> &[f64] {
        &self.returns
    }

    /// The equity curve (one entry per `record_return` call).
    pub fn equity_curve(&self) -> &[i64] {
        &self.equity_curve
    }

    /// The cost model in use.
    pub fn cost_model(&self) -> &CostModel {
        &self.cost_model
    }

    // === Execution ===

    /// Rebalance the portfolio to target weights using simple fill (instant execution).
    ///
    /// This is the hot path for parameter sweeps. Orders execute at the provided
    /// bar prices with no market microstructure simulation.
    ///
    /// `targets`: desired (symbol, weight) pairs. Weights should sum to ≤ 1.0.
    /// `prices`: current (symbol, price_in_cents) for each symbol.
    ///
    /// Positions not in `targets` are closed. Costs are deducted from cash.
    pub fn rebalance_simple(&mut self, targets: &[(Symbol, f64)], prices: &[(Symbol, i64)]) {
        let price_map: FxHashMap<Symbol, i64> = prices.iter().copied().collect();
        self.rebalance_simple_from_price_map(targets, &price_map);
    }

    pub(crate) fn rebalance_simple_from_price_map(
        &mut self,
        targets: &[(Symbol, f64)],
        price_map: &FxHashMap<Symbol, i64>,
    ) {
        let equity = self.total_equity_from_price_map(price_map);
        if equity <= 0 {
            return;
        }

        let target_map: FxHashMap<Symbol, f64> = targets.iter().copied().collect();

        // Close positions not in targets
        let to_close: Vec<Symbol> = self
            .positions
            .keys()
            .filter(|sym| !target_map.contains_key(sym))
            .copied()
            .collect();

        for sym in to_close {
            if let Some(price) = price_map.get(&sym).copied() {
                let qty = match self.positions.get(&sym) {
                    Some(pos) if !pos.is_flat() => -pos.quantity,
                    _ => continue,
                };
                self.execute_fill(sym, qty, price);
            }
        }

        // Rebalance each target
        for &(sym, target_weight) in targets {
            let price = match price_map.get(&sym).copied() {
                Some(p) if p > 0 => p,
                _ => continue,
            };

            let current_value = self
                .positions
                .get(&sym)
                .map(|p| p.market_value(price))
                .unwrap_or(0);

            let target_value = (equity as f64 * target_weight) as i64;
            let diff_value = target_value.saturating_sub(current_value);

            // Convert value difference to shares, rounded to a multiple of
            // quantity_step and truncated toward zero.
            let diff_qty = size_qty(diff_value, price, self.quantity_step);
            if !diff_qty.is_zero() {
                self.execute_fill(sym, diff_qty, price);
            }
        }
    }

    /// Close a single symbol position at the provided price.
    ///
    /// Returns `true` if a non-flat position existed and was closed.
    pub fn close_position_at(&mut self, symbol: Symbol, price: i64) -> bool {
        if price <= 0 {
            return false;
        }

        let qty = match self.positions.get(&symbol) {
            Some(pos) if !pos.is_flat() => -pos.quantity,
            _ => return false,
        };

        self.execute_fill(symbol, qty, price);
        true
    }

    /// Rebalance the portfolio through LOB matching engines.
    ///
    /// Routes orders through actual `Exchange` instances for realistic
    /// microstructure simulation including partial fills and price impact.
    ///
    /// `targets`: desired (symbol, weight) pairs.
    /// `exchanges`: mutable reference to a `MultiExchange` containing per-symbol LOBs.
    pub fn rebalance_lob(
        &mut self,
        targets: &[(Symbol, f64)],
        exchanges: &mut crate::multi_exchange::MultiExchange,
    ) {
        // Collect current prices from exchange BBO
        let price_map: FxHashMap<Symbol, i64> = exchanges
            .symbols()
            .filter_map(|sym| {
                let ex = exchanges.get(sym)?;
                let mid = {
                    let (bid, ask) = ex.best_bid_ask();
                    match (bid, ask) {
                        (Some(b), Some(a)) => b.0.saturating_add((a.0.saturating_sub(b.0)) / 2),
                        (Some(b), None) => b.0,
                        (None, Some(a)) => a.0,
                        (None, None) => return None,
                    }
                };
                Some((*sym, mid))
            })
            .collect();
        let equity = self.total_equity_from_price_map(&price_map);
        if equity <= 0 {
            return;
        }

        let target_map: FxHashMap<Symbol, f64> = targets.iter().copied().collect();

        // Close positions not in targets
        let to_close: Vec<Symbol> = self
            .positions
            .keys()
            .filter(|sym| !target_map.contains_key(sym))
            .copied()
            .collect();

        for sym in to_close {
            // The LOB only matches whole lots (Quantity = u64 order-book units,
            // out of scope for fractional shares), so truncate to whole shares
            // regardless of `quantity_step`.
            let (qty, side) = match self.positions.get(&sym) {
                Some(pos) if !pos.is_flat() => {
                    let whole = pos.quantity.whole();
                    let side = if whole > 0 {
                        crate::Side::Sell
                    } else {
                        crate::Side::Buy
                    };
                    (whole.unsigned_abs(), side)
                }
                _ => continue,
            };
            let exchange = exchanges.get_or_create(&sym);
            let result = exchange.submit_market(side, qty);
            for trade in &result.trades {
                let fill_qty = if side == crate::Side::Sell {
                    -(trade.quantity as i64)
                } else {
                    trade.quantity as i64
                };
                self.execute_fill(sym, Shares::from_whole(fill_qty), trade.price.0);
            }
        }

        // Rebalance each target
        for &(sym, target_weight) in targets {
            let price = match price_map.get(&sym).copied() {
                Some(p) if p > 0 => p,
                _ => continue,
            };

            let current_value = self
                .positions
                .get(&sym)
                .map(|p| p.market_value(price))
                .unwrap_or(0);

            let target_value = (equity as f64 * target_weight) as i64;
            let diff_value = target_value.saturating_sub(current_value);
            let diff_qty = (diff_value / price).unsigned_abs();

            if diff_qty == 0 {
                continue;
            }

            let side = if diff_value > 0 {
                crate::Side::Buy
            } else {
                crate::Side::Sell
            };

            let exchange = exchanges.get_or_create(&sym);
            let result = exchange.submit_market(side, diff_qty);
            for trade in &result.trades {
                let fill_qty = if side == crate::Side::Buy {
                    trade.quantity as i64
                } else {
                    -(trade.quantity as i64)
                };
                self.execute_fill(sym, Shares::from_whole(fill_qty), trade.price.0);
            }
        }
    }

    /// Record a return for the current period.
    ///
    /// Call this at the end of each period (day, month, etc.) after rebalancing.
    /// `prices` are current market prices for computing equity.
    pub fn record_return(&mut self, prices: &[(Symbol, i64)]) {
        let price_map: FxHashMap<Symbol, i64> = prices.iter().copied().collect();
        self.record_return_from_price_map(&price_map);
    }

    pub(crate) fn record_return_from_price_map(&mut self, price_map: &FxHashMap<Symbol, i64>) {
        let equity = self.total_equity_from_price_map(price_map);
        if self.prev_equity > 0 {
            let ret = equity.saturating_sub(self.prev_equity) as f64 / self.prev_equity as f64;
            self.returns.push(ret);
        }
        self.equity_curve.push(equity);
        self.prev_equity = equity;
    }

    /// Take a snapshot of the portfolio state.
    pub fn snapshot(&self, prices: &[(Symbol, i64)]) -> PortfolioSnapshot {
        let price_map: FxHashMap<Symbol, i64> = prices.iter().copied().collect();
        let equity = self.total_equity_from_price_map(&price_map);
        let weights = self.current_weights_from_price_map(&price_map, equity);
        let total_realized_pnl: i64 = self
            .positions
            .values()
            .fold(0_i64, |acc, p| acc.saturating_add(p.realized_pnl));

        PortfolioSnapshot {
            cash: self.cash,
            equity,
            weights,
            num_positions: self.positions.values().filter(|p| !p.is_flat()).count(),
            total_realized_pnl,
        }
    }

    // === Persistence ===

    /// Save the portfolio to a JSON file.
    #[cfg(feature = "persistence")]
    pub fn save_json(&self, path: &std::path::Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    /// Load a portfolio from a JSON file.
    #[cfg(feature = "persistence")]
    pub fn load_json(path: &std::path::Path) -> std::io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json).map_err(std::io::Error::other)
    }

    // === Internal ===

    pub(crate) fn total_equity_from_price_map(&self, price_map: &FxHashMap<Symbol, i64>) -> i64 {
        let position_value: i64 = self.positions.iter().fold(0_i64, |acc, (sym, pos)| {
            let price = price_map.get(sym).copied().unwrap_or(0);
            acc.saturating_add(pos.market_value(price))
        });
        self.cash.saturating_add(position_value)
    }

    pub(crate) fn current_weights_from_price_map(
        &self,
        price_map: &FxHashMap<Symbol, i64>,
        equity: i64,
    ) -> Vec<(Symbol, f64)> {
        if equity == 0 {
            return Vec::new();
        }

        self.positions
            .iter()
            .filter(|(_, pos)| !pos.is_flat())
            .map(|(sym, pos)| {
                let price = price_map.get(sym).copied().unwrap_or(0);
                let mv = pos.market_value(price) as f64;
                (*sym, mv / equity as f64)
            })
            .collect()
    }

    /// Execute a fill: update position, deduct cost, adjust cash.
    ///
    /// `qty` is signed micro-shares (see [`Shares`]).
    fn execute_fill(&mut self, symbol: Symbol, qty: Shares, price: i64) {
        if qty.is_zero() {
            return;
        }
        // [Inference: rounding mode] round() = round-half-away-from-zero; a plausible default
        // but arguable. Flag: effective_price rounding mode unspecified by ADR-0003.
        let sign = if qty.is_positive() { 1.0_f64 } else { -1.0_f64 };
        let slippage_factor = 1.0 + sign * self.cost_model.slippage_bps / 10_000.0;
        let effective_price_f = price as f64 * slippage_factor;
        let effective_price = effective_price_f.round() as i64;

        // i128 intermediate: at micro-share granularity a large position
        // (e.g. 1e6 shares at $10,000) overflows i64 before normalizing by
        // Shares::SCALE, so raw_qty * price must not be computed in i64.
        let notional_i128 = (qty.raw().unsigned_abs() as i128) * (effective_price.unsigned_abs() as i128)
            / (Shares::SCALE as i128);
        let notional = notional_i128.clamp(0, i64::MAX as i128) as i64;
        let cost = self.cost_model.compute_cost(notional);

        let pos = self
            .positions
            .entry(symbol)
            .or_insert_with(|| Position::new(symbol));
        pos.apply_fill(qty, effective_price);

        let cash_delta_i128 =
            (qty.raw() as i128) * (effective_price as i128) / (Shares::SCALE as i128);
        let cash_delta = cash_delta_i128.clamp(i64::MIN as i128, i64::MAX as i128) as i64;
        self.cash = self.cash.saturating_sub(cash_delta.saturating_add(cost));
    }
}

/// Convert a target value difference (cents) at a given price (cents/share)
/// into a signed micro-share quantity ([`Shares`]), rounded down in magnitude
/// to the nearest multiple of `step` (micro-shares) — i.e. truncated toward
/// zero identically for buys (`diff_value > 0`) and sells (`diff_value < 0`).
///
/// `price` must be positive; `step` must be positive. Uses `i128`
/// intermediates so large notional values can't overflow before scaling.
fn size_qty(diff_value: i64, price: i64, step: i64) -> Shares {
    if diff_value == 0 || price <= 0 || step <= 0 {
        return Shares::ZERO;
    }
    let sign: i128 = if diff_value < 0 { -1 } else { 1 };
    let abs_value = diff_value.unsigned_abs() as i128;
    let raw_micro = abs_value * (Shares::SCALE as i128) / (price as i128);
    let snapped = (raw_micro / (step as i128)) * (step as i128);
    let signed = snapped * sign;
    Shares::from_raw(signed.clamp(i64::MIN as i128, i64::MAX as i128) as i64)
}

/// A point-in-time snapshot of portfolio state.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PortfolioSnapshot {
    /// Cash balance (cents)
    pub cash: i64,
    /// Total equity (cents)
    pub equity: i64,
    /// Current weights
    pub weights: Vec<(Symbol, f64)>,
    /// Number of non-flat positions
    pub num_positions: usize,
    /// Total realized PnL across all positions
    pub total_realized_pnl: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aapl() -> Symbol {
        Symbol::new("AAPL")
    }
    fn msft() -> Symbol {
        Symbol::new("MSFT")
    }

    #[test]
    fn new_portfolio() {
        let portfolio = Portfolio::new(1_000_000_00, CostModel::zero());
        assert_eq!(portfolio.cash(), 1_000_000_00);
        assert_eq!(portfolio.total_equity(&[]), 1_000_000_00);
    }

    #[test]
    fn simple_buy() {
        let mut portfolio = Portfolio::new(1_000_000_00, CostModel::zero());
        let targets = [(aapl(), 0.5)];
        let prices = [(aapl(), 150_00)];

        portfolio.rebalance_simple(&targets, &prices);

        let pos = portfolio.position(&aapl()).unwrap();
        assert!(pos.quantity.is_positive());
        // Should have bought ~$500,000 worth at $150 = ~3333 shares
        assert_eq!(pos.quantity, Shares::from_whole(3333));
    }

    #[test]
    fn equity_conservation_no_cost() {
        let mut portfolio = Portfolio::new(1_000_000_00, CostModel::zero());
        let prices = [(aapl(), 150_00), (msft(), 300_00)];
        let targets = [(aapl(), 0.6), (msft(), 0.4)];

        let equity_before = portfolio.total_equity(&prices);
        portfolio.rebalance_simple(&targets, &prices);
        let equity_after = portfolio.total_equity(&prices);

        // With zero cost and integer rounding, equity should be very close
        let diff = (equity_after - equity_before).abs();
        // Allow rounding error of up to 1 share per position * max price
        assert!(diff < 2 * 300_00, "equity diff too large: {diff}");
    }

    #[test]
    fn cost_model_deducts_fees() {
        let model = CostModel {
            commission_bps: 10.0,
            slippage_bps: 0.0,
            min_commission: 0,
        };
        let mut portfolio = Portfolio::new(1_000_000_00, model);
        let prices = [(aapl(), 150_00)];
        let targets = [(aapl(), 0.5)];

        portfolio.rebalance_simple(&targets, &prices);

        let equity = portfolio.total_equity(&prices);
        // Equity should be less than initial due to costs
        assert!(equity < 1_000_000_00);
    }

    #[test]
    fn rebalance_closes_unneeded_positions() {
        let mut portfolio = Portfolio::new(1_000_000_00, CostModel::zero());
        let prices = [(aapl(), 150_00), (msft(), 300_00)];

        // First: buy AAPL and MSFT
        portfolio.rebalance_simple(&[(aapl(), 0.5), (msft(), 0.5)], &prices);
        assert!(portfolio.position(&aapl()).unwrap().quantity.is_positive());
        assert!(portfolio.position(&msft()).unwrap().quantity.is_positive());

        // Second: only AAPL — MSFT should be closed
        portfolio.rebalance_simple(&[(aapl(), 0.5)], &prices);
        assert!(portfolio.position(&msft()).unwrap().is_flat());
    }

    #[test]
    fn close_position_at() {
        let mut portfolio = Portfolio::new(1_000_000_00, CostModel::zero());
        let prices = [(aapl(), 150_00)];
        portfolio.rebalance_simple(&[(aapl(), 0.8)], &prices);
        assert!(portfolio.position(&aapl()).unwrap().quantity.is_positive());

        let closed = portfolio.close_position_at(aapl(), 155_00);
        assert!(closed);
        assert!(portfolio.position(&aapl()).unwrap().is_flat());
    }

    #[test]
    fn record_return_tracks_equity() {
        let mut portfolio = Portfolio::new(100_00, CostModel::zero());
        let prices = [(aapl(), 10_00)];

        portfolio.rebalance_simple(&[(aapl(), 1.0)], &prices);

        // Price goes up 10%
        let new_prices = [(aapl(), 11_00)];
        portfolio.record_return(&new_prices);

        assert_eq!(portfolio.returns().len(), 1);
        let ret = portfolio.returns()[0];
        assert!(ret > 0.0);
    }

    #[test]
    fn snapshot() {
        let mut portfolio = Portfolio::new(1_000_000_00, CostModel::zero());
        let prices = [(aapl(), 150_00)];
        portfolio.rebalance_simple(&[(aapl(), 0.5)], &prices);

        let snap = portfolio.snapshot(&prices);
        assert_eq!(snap.num_positions, 1);
        // Equity should be close to initial (zero cost)
        assert!((snap.equity - 1_000_000_00).abs() < 300_00);
    }

    #[test]
    fn current_weights() {
        let mut portfolio = Portfolio::new(1_000_000_00, CostModel::zero());
        let prices = [(aapl(), 150_00)];
        portfolio.rebalance_simple(&[(aapl(), 0.5)], &prices);

        let weights = portfolio.current_weights(&prices);
        assert_eq!(weights.len(), 1);
        // Weight should be approximately 0.5
        assert!((weights[0].1 - 0.5).abs() < 0.01);
    }

    // === Fractional-share sizing (default quantity_step == Shares::SCALE) ===

    #[test]
    fn default_quantity_step_is_whole_shares() {
        let portfolio = Portfolio::new(1_000_000_00, CostModel::zero());
        assert_eq!(portfolio.quantity_step(), Shares::SCALE);
    }

    /// Pins the exact whole-share sizing and equity at the default
    /// `quantity_step`, so any future change to sizing that alters today's
    /// numbers is caught here — this is the "bit-identical at default" proof.
    #[test]
    fn default_step_pins_exact_quantities_and_equity() {
        let mut portfolio = Portfolio::new(1_000_000_00, CostModel::zero()); // $1,000,000
        let prices = [(aapl(), 150_00), (msft(), 300_00)];
        let targets = [(aapl(), 0.6), (msft(), 0.4)];

        portfolio.rebalance_simple(&targets, &prices);

        // target_value(AAPL) = 60_000_000 cents / 150_00 cents/share = 4000 shares exactly.
        assert_eq!(
            portfolio.position(&aapl()).unwrap().quantity,
            Shares::from_whole(4000)
        );
        // target_value(MSFT) = 40_000_000 cents / 300_00 cents/share = 1333.33 -> 1333 shares.
        assert_eq!(
            portfolio.position(&msft()).unwrap().quantity,
            Shares::from_whole(1333)
        );

        // Zero cost, no slippage, both fills exactly reversible in cash terms:
        // equity is conserved to the cent (only integer share truncation on
        // MSFT leaves a cash remainder, no value is destroyed).
        let equity = portfolio.total_equity(&prices);
        assert_eq!(equity, 1_000_000_00);
    }

    // === Fractional sizing (quantity_step < Shares::SCALE) ===

    /// With a $1,000 account and 20 equal-weight $219 names, whole-share
    /// sizing rounds every target to zero shares (50 / 219 < 1). Alpaca's
    /// fractional minimum (`quantity_step = 1_000`, 0.001 share) must let
    /// every target hold a non-zero position and invest > 99% of capital.
    #[test]
    fn fractional_step_invests_small_account() {
        let mut portfolio =
            Portfolio::with_quantity_step(1_000_00, CostModel::zero(), 1_000); // $1,000, 0.001-share step
        let price = 219_00; // $219, matches the plan's median-price figure
        let n = 20;
        let weight = 1.0 / n as f64;
        let symbols: Vec<Symbol> = (0..n)
            .map(|i| Symbol::new(&format!("SYM{i}")))
            .collect();
        let prices: Vec<(Symbol, i64)> = symbols.iter().map(|s| (*s, price)).collect();
        let targets: Vec<(Symbol, f64)> = symbols.iter().map(|s| (*s, weight)).collect();

        portfolio.rebalance_simple(&targets, &prices);

        for sym in &symbols {
            let pos = portfolio.position(sym).unwrap();
            assert!(
                !pos.is_flat(),
                "{sym} rounded to zero shares under fractional sizing"
            );
        }

        let equity = portfolio.total_equity(&prices);
        let invested_fraction = 1.0 - portfolio.cash() as f64 / equity as f64;
        assert!(
            invested_fraction > 0.99,
            "only {:.2}% of capital invested",
            invested_fraction * 100.0
        );
    }

    /// Under whole-share sizing (the pre-fractional default), the same
    /// $1,000 / 20-name setup rounds every target to zero — this is the
    /// defect the plan measured, reproduced here as the "before" baseline
    /// for the fractional-step test above.
    #[test]
    fn whole_share_step_leaves_small_account_uninvested() {
        let mut portfolio = Portfolio::new(1_000_00, CostModel::zero()); // $1,000, default step
        let price = 219_00;
        let n = 20;
        let weight = 1.0 / n as f64;
        let symbols: Vec<Symbol> = (0..n)
            .map(|i| Symbol::new(&format!("SYM{i}")))
            .collect();
        let prices: Vec<(Symbol, i64)> = symbols.iter().map(|s| (*s, price)).collect();
        let targets: Vec<(Symbol, f64)> = symbols.iter().map(|s| (*s, weight)).collect();

        portfolio.rebalance_simple(&targets, &prices);

        for sym in &symbols {
            // A target that rounds to zero shares never gets a position at
            // all (execute_fill is skipped for a zero diff_qty).
            let flat = portfolio.position(sym).is_none_or(|p| p.is_flat());
            assert!(flat, "{sym} unexpectedly holds a position");
        }
        assert_eq!(portfolio.cash(), 1_000_00);
    }

    // === Sign symmetry ===

    /// Sizing must truncate toward zero identically for a buy (positive
    /// diff_value) and a sell (negative diff_value): equal-magnitude
    /// opposite-sign inputs give equal-magnitude opposite-sign quantities.
    #[test]
    fn size_qty_truncates_toward_zero_symmetrically() {
        let price = 15_137; // deliberately not a clean divisor
        let step = 1_000; // 0.001-share granularity
        let buy = size_qty(1_000_00, price, step);
        let sell = size_qty(-1_000_00, price, step);
        assert!(buy.is_positive());
        assert!(sell.is_negative());
        assert_eq!(buy.raw(), -sell.raw());

        // Also true at the default whole-share step.
        let buy_whole = size_qty(1_000_00, price, Shares::SCALE);
        let sell_whole = size_qty(-1_000_00, price, Shares::SCALE);
        assert_eq!(buy_whole.raw(), -sell_whole.raw());
    }

    // === Overflow guard ===

    /// A position of 1,000,000 shares at $10,000/share: multiplying qty_raw
    /// (micro-shares) by price would be ~1e21 if computed naively in i64
    /// before normalizing by `Shares::SCALE` (i64::MAX is ~9.2e18). The i128
    /// intermediates in `execute_fill` and `Position::market_value` must not
    /// let this wrap.
    #[test]
    fn execute_fill_i128_guard_does_not_overflow() {
        let mut portfolio =
            Portfolio::with_quantity_step(i64::MAX, CostModel::zero(), Shares::SCALE);
        let price = 1_000_000_00; // $10,000/share, in cents
        let sym = aapl();

        // Force a target value large enough to size a 1,000,000-share position.
        let target_value = 1_000_000i64 * price; // 1e12 cents
        // Emulate rebalance_simple's internals directly via a target weight
        // computed from a controlled equity so the intended quantity is exact.
        let weight = target_value as f64 / i64::MAX as f64;
        portfolio.rebalance_simple(&[(sym, weight)], &[(sym, price)]);

        let pos = portfolio.position(&sym).unwrap();
        assert!(!pos.is_flat());
        // Naive i64 multiplication (raw_micro_shares * price) would have wrapped;
        // the i128-computed market value must be exactly qty_whole * price with
        // no sign flip or truncation artifact from wraparound.
        let mv = pos.market_value(price);
        let expected = pos.quantity.whole() as i128 * price as i128;
        assert_eq!(mv as i128, expected);
        assert!(mv > 0, "market value wrapped negative under overflow");
    }
}

#[cfg(all(test, feature = "persistence"))]
mod persistence_tests {
    use super::*;

    fn aapl() -> Symbol {
        Symbol::new("AAPL")
    }

    #[test]
    fn portfolio_json_roundtrip() {
        let mut portfolio = Portfolio::new(1_000_000_00, CostModel::zero());
        let prices = [(aapl(), 150_00)];
        portfolio.rebalance_simple(&[(aapl(), 0.5)], &prices);
        portfolio.record_return(&prices);

        let json = serde_json::to_string(&portfolio).unwrap();
        let restored: Portfolio = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.cash(), portfolio.cash());
        assert_eq!(restored.returns().len(), portfolio.returns().len());
        assert_eq!(
            restored.position(&aapl()).unwrap().quantity,
            portfolio.position(&aapl()).unwrap().quantity
        );
    }

    #[test]
    fn portfolio_save_load_file() {
        let mut portfolio = Portfolio::new(500_000_00, CostModel::zero());
        let prices = [(aapl(), 100_00)];
        portfolio.rebalance_simple(&[(aapl(), 1.0)], &prices);

        let dir = std::env::temp_dir();
        let path = dir.join("nanobook_test_portfolio.json");

        portfolio.save_json(&path).unwrap();
        let loaded = Portfolio::load_json(&path).unwrap();

        assert_eq!(loaded.cash(), portfolio.cash());
        assert_eq!(
            loaded.position(&aapl()).unwrap().quantity,
            portfolio.position(&aapl()).unwrap().quantity
        );

        // Cleanup
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn metrics_serde() {
        let returns = vec![0.01, -0.005, 0.02];
        let m = compute_metrics(&returns, 252.0, 0.0).unwrap();

        let json = serde_json::to_string(&m).unwrap();
        let restored: Metrics = serde_json::from_str(&json).unwrap();

        assert!((restored.total_return - m.total_return).abs() < 1e-10);
        assert!((restored.sharpe - m.sharpe).abs() < 1e-10);
    }
}
