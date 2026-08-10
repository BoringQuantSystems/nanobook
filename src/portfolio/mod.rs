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
    /// Minimum order notional (cents). Orders below this are skipped instead
    /// of placed. Defaults to `0` (no minimum — every non-zero order is placed).
    #[cfg_attr(feature = "serde", serde(default))]
    min_order_value: i64,
    /// Maximum order notional (cents). Orders above this are truncated to the
    /// largest quantity (respecting `quantity_step`) whose notional fits,
    /// rather than being dropped. Defaults to `0` (unlimited).
    #[cfg_attr(feature = "serde", serde(default))]
    max_order_value: i64,
    /// No-trade band, in basis points of equity. A position is left alone
    /// until its value drifts from its target by more than this many bps.
    /// Defaults to `0.0` (any non-zero drift is corrected).
    #[cfg_attr(feature = "serde", serde(default))]
    no_trade_band_bps: f64,
    /// Hard cap on the number of orders placed in a single rebalance. When the
    /// cap binds, the orders furthest from target (largest absolute drift) are
    /// kept and the rest are dropped, ties broken by symbol. Defaults to `None`
    /// (no cap).
    #[cfg_attr(feature = "serde", serde(default))]
    max_trades_per_rebalance: Option<usize>,
    /// Maximum total absolute notional (cents) admitted in a single
    /// `rebalance_simple` call, enforced as a running sum over orders in
    /// priority order (largest drift first). Defaults to `0` (unlimited).
    #[cfg_attr(feature = "serde", serde(default))]
    max_rebalance_notional: i64,
    /// Number of orders actually executed by the most recent `rebalance_simple`
    /// call, after every filter. Not persisted: it's a transient report of the
    /// last call, not portfolio state.
    #[cfg_attr(feature = "serde", serde(skip, default))]
    last_rebalance_order_count: usize,
    /// Total absolute notional (cents) actually filled by the most recent
    /// `rebalance_simple` call. Not persisted, for the same reason as
    /// `last_rebalance_order_count`.
    #[cfg_attr(feature = "serde", serde(skip, default))]
    last_rebalance_notional: i64,
    /// Number of buy fills truncated because the account could not cover the
    /// requested quantity plus costs. Cumulative over the portfolio's life;
    /// not persisted.
    #[cfg_attr(feature = "serde", serde(skip, default))]
    trimmed_fill_count: u64,
    /// Sum, in cents of notional, of buy quantity that was requested but not
    /// filled because cash was insufficient. Cumulative; not persisted.
    #[cfg_attr(feature = "serde", serde(skip, default))]
    trimmed_shortfall_cents: i64,
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
            min_order_value: 0,
            max_order_value: 0,
            no_trade_band_bps: 0.0,
            max_trades_per_rebalance: None,
            max_rebalance_notional: 0,
            last_rebalance_order_count: 0,
            last_rebalance_notional: 0,
            trimmed_fill_count: 0,
            trimmed_shortfall_cents: 0,
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

    /// Set the minimum order notional (cents). Orders below this are skipped
    /// instead of placed. Must be non-negative; values `< 0` are a programming
    /// error (use `debug_assert`).
    pub fn set_min_order_value(&mut self, value: i64) {
        debug_assert!(value >= 0, "min_order_value must be non-negative, got {value}");
        self.min_order_value = value;
    }

    /// The current minimum order notional (cents).
    #[inline]
    pub fn min_order_value(&self) -> i64 {
        self.min_order_value
    }

    /// Set the maximum order notional (cents). Orders above this are
    /// truncated to the largest quantity (respecting `quantity_step`) whose
    /// notional fits, rather than being dropped: a real desk works a large
    /// order down over time rather than skipping the position entirely, and
    /// dropping it would silently leave the portfolio un-rebalanced with no
    /// signal that anything happened. Truncation composes with
    /// `min_order_value`: if the truncated quantity's notional then falls
    /// below the minimum, the order is skipped like any other order that
    /// doesn't clear the minimum. Must be non-negative; values `< 0` are a
    /// programming error (use `debug_assert`). `0` means unlimited (the
    /// default).
    pub fn set_max_order_value(&mut self, value: i64) {
        debug_assert!(value >= 0, "max_order_value must be non-negative, got {value}");
        self.max_order_value = value;
    }

    /// The current maximum order notional (cents). `0` means unlimited.
    #[inline]
    pub fn max_order_value(&self) -> i64 {
        self.max_order_value
    }

    /// Number of orders actually executed by the most recent
    /// [`Portfolio::rebalance_simple`] call, after every filter (no-trade
    /// band, min/max order value, trade-count cap) — including both
    /// close-loop and target-loop orders. Resets to `0` at the start of each
    /// such call, including calls that end up placing no orders.
    #[inline]
    pub fn last_rebalance_order_count(&self) -> usize {
        self.last_rebalance_order_count
    }

    /// Total absolute notional (cents) actually filled by the most recent
    /// [`Portfolio::rebalance_simple`] call, summed over close-loop and
    /// target-loop orders alike. Resets to `0` at the start of each such
    /// call.
    #[inline]
    pub fn last_rebalance_notional(&self) -> i64 {
        self.last_rebalance_notional
    }

    /// Number of buy fills that were truncated because cash could not cover
    /// the requested quantity plus costs. Cumulative over the portfolio's
    /// lifetime (not reset by rebalance).
    #[inline]
    pub fn trimmed_fill_count(&self) -> u64 {
        self.trimmed_fill_count
    }

    /// Total notional (cents) of buy quantity that was requested but not
    /// filled because cash was insufficient. Cumulative over the portfolio's
    /// lifetime (not reset by rebalance). Zero means no buy has ever been
    /// trimmed for affordability.
    #[inline]
    pub fn trimmed_shortfall_cents(&self) -> i64 {
        self.trimmed_shortfall_cents
    }

    /// Set the no-trade band, in basis points of equity. Must be non-negative
    /// and finite; other values are a programming error (use `debug_assert`).
    pub fn set_no_trade_band_bps(&mut self, bps: f64) {
        debug_assert!(
            bps.is_finite() && bps >= 0.0,
            "no_trade_band_bps must be finite and non-negative, got {bps}"
        );
        self.no_trade_band_bps = bps;
    }

    /// The current no-trade band, in basis points of equity.
    #[inline]
    pub fn no_trade_band_bps(&self) -> f64 {
        self.no_trade_band_bps
    }

    /// Set the hard cap on orders placed per rebalance. `None` removes the cap.
    /// When the cap binds, the orders furthest from target (largest absolute
    /// drift) are kept.
    pub fn set_max_trades_per_rebalance(&mut self, cap: Option<usize>) {
        self.max_trades_per_rebalance = cap;
    }

    /// The current cap on orders placed per rebalance, if any.
    #[inline]
    pub fn max_trades_per_rebalance(&self) -> Option<usize> {
        self.max_trades_per_rebalance
    }

    /// Set the maximum total absolute notional (cents) a single
    /// `rebalance_simple` call may trade. Enforced as a running sum over
    /// orders in priority order (largest drift first, ties by symbol): the
    /// first order that would push the running total past the cap is
    /// truncated to exactly the remaining budget and no further orders are
    /// admitted, so the budget is spent in full on the highest-priority
    /// correction rather than left partly unused while smaller orders are
    /// skipped in favour of it. If that truncation lands below
    /// `min_order_value`, the order is skipped like any other order that
    /// doesn't clear the minimum — trading still stops there. Must be
    /// non-negative; values `< 0` are a programming error (use
    /// `debug_assert`). `0` means unlimited (the default).
    pub fn set_max_rebalance_notional(&mut self, value: i64) {
        debug_assert!(
            value >= 0,
            "max_rebalance_notional must be non-negative, got {value}"
        );
        self.max_rebalance_notional = value;
    }

    /// The current per-rebalance notional cap (cents). `0` means unlimited.
    #[inline]
    pub fn max_rebalance_notional(&self) -> i64 {
        self.max_rebalance_notional
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
        self.last_rebalance_order_count = 0;
        self.last_rebalance_notional = 0;

        let equity = self.total_equity_from_price_map(price_map);
        if equity <= 0 {
            return;
        }

        let target_map: FxHashMap<Symbol, f64> = targets.iter().copied().collect();

        let mut planned: Vec<PlannedOrder> = Vec::new();

        // Close positions not in targets. This mirrors the pre-existing
        // behaviour exactly: a full close ignores `quantity_step` (it always
        // liquidates the whole position) and does not require a positive
        // price (a price of `<= 0` can still close a position, same as
        // before these constraints existed).
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

                let current_value = self
                    .positions
                    .get(&sym)
                    .map(|p| p.market_value(price))
                    .unwrap_or(0);
                let drift_bps = drift_bps_of(current_value, equity);
                if self.no_trade_band_bps > 0.0 && drift_bps <= self.no_trade_band_bps {
                    continue;
                }

                let mut qty = qty;
                let mut notional = notional_of(qty, price);
                if self.max_order_value > 0 && notional > self.max_order_value {
                    qty = truncate_qty_to_max_value(
                        qty,
                        price,
                        self.quantity_step,
                        self.max_order_value,
                    );
                    if qty.is_zero() {
                        continue;
                    }
                    notional = notional_of(qty, price);
                }
                if self.min_order_value > 0 && notional < self.min_order_value {
                    continue;
                }

                planned.push(PlannedOrder { symbol: sym, qty, price, drift_bps });
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

            let drift_bps = drift_bps_of(diff_value, equity);
            if self.no_trade_band_bps > 0.0 && drift_bps <= self.no_trade_band_bps {
                continue;
            }

            // Convert value difference to shares, rounded to a multiple of
            // quantity_step and truncated toward zero.
            let mut diff_qty = size_qty(diff_value, price, self.quantity_step);
            if diff_qty.is_zero() {
                continue;
            }

            let mut notional = notional_of(diff_qty, price);
            if self.max_order_value > 0 && notional > self.max_order_value {
                diff_qty = truncate_qty_to_max_value(
                    diff_qty,
                    price,
                    self.quantity_step,
                    self.max_order_value,
                );
                if diff_qty.is_zero() {
                    continue;
                }
                notional = notional_of(diff_qty, price);
            }
            if self.min_order_value > 0 && notional < self.min_order_value {
                continue;
            }

            planned.push(PlannedOrder { symbol: sym, qty: diff_qty, price, drift_bps });
        }

        // Enforce the trade-count cap and the per-rebalance notional cap
        // together, in priority order (largest absolute drift first, ties
        // broken by symbol so the result never depends on hash-map iteration
        // order). Sorting runs whenever either cap is active so admission is
        // deterministic even when the trade-count cap alone wouldn't need to
        // drop anything.
        if self.max_trades_per_rebalance.is_some() || self.max_rebalance_notional > 0 {
            planned.sort_by(|a, b| {
                b.drift_bps
                    .partial_cmp(&a.drift_bps)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.symbol.cmp(&b.symbol))
            });

            let trade_cap = self.max_trades_per_rebalance.unwrap_or(usize::MAX);
            let mut admitted: Vec<PlannedOrder> = Vec::with_capacity(planned.len().min(trade_cap));
            let mut running_notional: i64 = 0;

            for mut order in planned {
                if admitted.len() >= trade_cap {
                    break;
                }

                let notional = notional_of(order.qty, order.price);
                if self.max_rebalance_notional > 0 {
                    let remaining = self.max_rebalance_notional - running_notional;
                    if remaining <= 0 {
                        break;
                    }
                    if notional > remaining {
                        // This order alone would breach the cap: truncate it
                        // to exactly the remaining budget (see
                        // `set_max_rebalance_notional` for why truncate
                        // rather than skip-and-continue) and admit no
                        // further orders, since the budget is now spent.
                        order.qty = truncate_qty_to_max_value(
                            order.qty,
                            order.price,
                            self.quantity_step,
                            remaining,
                        );
                        if !order.qty.is_zero() {
                            let truncated_notional = notional_of(order.qty, order.price);
                            if self.min_order_value <= 0
                                || truncated_notional >= self.min_order_value
                            {
                                admitted.push(order);
                            }
                        }
                        break;
                    }
                }

                running_notional += notional;
                admitted.push(order);
            }

            planned = admitted;
        }

        self.last_rebalance_order_count = planned.len();
        self.last_rebalance_notional = planned.iter().fold(0_i64, |acc, order| {
            acc.saturating_add(notional_of(order.qty, order.price))
        });

        for order in planned {
            self.execute_fill(order.symbol, order.qty, order.price);
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
    ///
    /// Buys never spend more cash than the account holds. An unaffordable buy
    /// is truncated to the largest `quantity_step` multiple whose notional
    /// plus [`CostModel::compute_cost`] fits in `cash`. A quantity that snaps
    /// to zero is skipped entirely (no zero-notional fill that would still
    /// charge `min_commission`). Sells are never truncated — they raise cash.
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

        let mut qty = qty;

        // Affordability gate for buys. `compute_cost` is monotonic in notional
        // but has a `min_commission` floor, so `cash / price` alone overshoots:
        // solve with the cost model and snap down to `quantity_step`.
        if qty.is_positive() {
            let requested_raw = qty.raw();
            let required = buy_cash_required(requested_raw, effective_price, &self.cost_model);
            if required > self.cash {
                let affordable = largest_affordable_buy_qty(
                    requested_raw,
                    self.cash,
                    self.quantity_step,
                    effective_price,
                    &self.cost_model,
                );
                let requested_notional = notional_of(qty, effective_price);
                let filled_notional = notional_of(affordable, effective_price);
                self.trimmed_fill_count = self.trimmed_fill_count.saturating_add(1);
                self.trimmed_shortfall_cents = self
                    .trimmed_shortfall_cents
                    .saturating_add(requested_notional.saturating_sub(filled_notional));
                qty = affordable;
                if qty.is_zero() {
                    // No fill of zero-with-cost: skipping avoids charging the
                    // min_commission floor against an empty trade.
                    return;
                }
            }
        }

        // i128 intermediate: at micro-share granularity a large position
        // (e.g. 1e6 shares at $10,000) overflows i64 before normalizing by
        // Shares::SCALE, so raw_qty * price must not be computed in i64.
        let notional_i128 = (qty.raw().unsigned_abs() as i128)
            * (effective_price.unsigned_abs() as i128)
            / (Shares::SCALE as i128);
        let notional = notional_i128.clamp(0, i64::MAX as i128) as i64;
        let cost = self.cost_model.compute_cost(notional);

        let cash_delta_i128 =
            (qty.raw() as i128) * (effective_price as i128) / (Shares::SCALE as i128);
        let cash_delta = cash_delta_i128.clamp(i64::MIN as i128, i64::MAX as i128) as i64;

        // Sells are never quantity-truncated, but a tiny sell can still carry a
        // `min_commission` larger than its proceeds. If cash + proceeds cannot
        // cover the commission, skip the fill entirely rather than overdraw —
        // reducing the sell would only make the floor worse, and the buy-side
        // trim path does not apply.
        if qty.is_negative() {
            let cash_after = self.cash.saturating_sub(cash_delta.saturating_add(cost));
            if cash_after < 0 {
                return;
            }
        }

        let pos = self
            .positions
            .entry(symbol)
            .or_insert_with(|| Position::new(symbol));
        pos.apply_fill(qty, effective_price);

        self.cash = self.cash.saturating_sub(cash_delta.saturating_add(cost));
        debug_assert!(self.cash >= 0);
    }
}

/// A candidate order queued during rebalancing, before the trade-count cap
/// (if any) is applied.
struct PlannedOrder {
    symbol: Symbol,
    qty: Shares,
    price: i64,
    /// Absolute drift from target, in basis points of equity. Used to rank
    /// orders when `max_trades_per_rebalance` binds.
    drift_bps: f64,
}

/// Notional value (cents) of a fill: `|qty| * price`, in [`Shares`] micro-share
/// units. Uses an `i128` intermediate for the same overflow-safety reason as
/// `execute_fill`.
fn notional_of(qty: Shares, price: i64) -> i64 {
    let notional_i128 =
        (qty.raw().unsigned_abs() as i128) * (price.unsigned_abs() as i128) / (Shares::SCALE as i128);
    notional_i128.clamp(0, i64::MAX as i128) as i64
}

/// Cash outlay (cents) required to buy `raw_abs` micro-shares at
/// `effective_price`: notional plus [`CostModel::compute_cost`]. Uses an
/// `i128` intermediate for the same overflow-safety reason as `notional_of`.
fn buy_cash_required(raw_abs: i64, effective_price: i64, cost_model: &CostModel) -> i64 {
    if raw_abs <= 0 || effective_price <= 0 {
        return 0;
    }
    let notional_i128 =
        (raw_abs as i128) * (effective_price as i128) / (Shares::SCALE as i128);
    let notional = notional_i128.clamp(0, i64::MAX as i128) as i64;
    notional.saturating_add(cost_model.compute_cost(notional))
}

/// Largest buy quantity at or below `requested_raw` micro-shares whose
/// notional-plus-cost fits in `cash`, snapped down to a multiple of `step`.
/// Returns [`Shares::ZERO`] when even one step is unaffordable (or inputs are
/// degenerate). Binary-searches step multiples because `min_commission` makes
/// a naive `cash / price` overshoot.
fn largest_affordable_buy_qty(
    requested_raw: i64,
    cash: i64,
    step: i64,
    effective_price: i64,
    cost_model: &CostModel,
) -> Shares {
    if requested_raw <= 0 || cash <= 0 || step <= 0 || effective_price <= 0 {
        return Shares::ZERO;
    }
    if buy_cash_required(requested_raw, effective_price, cost_model) <= cash {
        return Shares::from_raw(requested_raw);
    }
    // Search over k = number of steps, k * step <= requested_raw.
    let mut lo = 0_i64;
    let mut hi = requested_raw / step;
    while lo < hi {
        let mid = lo + (hi - lo + 1) / 2;
        let raw = mid.saturating_mul(step);
        if buy_cash_required(raw, effective_price, cost_model) <= cash {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    Shares::from_raw(lo.saturating_mul(step))
}

/// Truncate `qty` (magnitude only; sign preserved) to the largest quantity,
/// snapped down to a multiple of `step`, whose notional at `price` fits
/// within `max_value` (cents). Uses an `i128` intermediate for the same
/// overflow-safety reason as `notional_of`. Floor division is conservative:
/// it never rounds the resulting notional above `max_value`. Returns
/// `Shares::ZERO` if even one `step` doesn't fit within `max_value`, or if
/// `price` is zero (no quantity has a well-defined truncated notional then).
fn truncate_qty_to_max_value(qty: Shares, price: i64, step: i64, max_value: i64) -> Shares {
    if max_value <= 0 || price == 0 || step <= 0 {
        return Shares::ZERO;
    }
    let price_abs = price.unsigned_abs() as i128;
    let max_value_i128 = max_value as i128;
    let scale = Shares::SCALE as i128;
    let raw_abs_cap = (max_value_i128 * scale) / price_abs;
    let step_i128 = step as i128;
    let snapped = (raw_abs_cap / step_i128) * step_i128;
    let snapped_i64 = snapped.clamp(0, i64::MAX as i128) as i64;
    let signed = if qty.is_negative() { -snapped_i64 } else { snapped_i64 };
    Shares::from_raw(signed)
}

/// Drift of a value difference (cents) from target, expressed in basis points
/// of equity: `|value| / equity * 10_000`. `equity` must be positive (callers
/// already return early when `equity <= 0`).
fn drift_bps_of(value: i64, equity: i64) -> f64 {
    (value.unsigned_abs() as f64 / equity as f64) * 10_000.0
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

    // === Execution constraints: min_order_value, no_trade_band_bps,
    // === max_trades_per_rebalance ===

    fn ten_symbols() -> Vec<Symbol> {
        (0..10).map(|i| Symbol::new(&format!("S{i}"))).collect()
    }

    /// THE REGRESSION THIS SUITE MUST CATCH: constant target weights, flat
    /// prices, a non-zero commission floor, and ZERO price drift must place
    /// only the initial buy — one order per name — and nothing on any later
    /// rebalance. Before `no_trade_band_bps` existed, the first rebalance's
    /// commission lowered equity, which lowered every target's value, which
    /// made every position look marginally over target on the next
    /// rebalance, triggering a "correction" that cost more commission,
    /// forever: 10 names x 36 rebalances placed 360 orders instead of 10.
    ///
    /// Checked at both whole-share and fractional (0.001-share) granularity,
    /// since whole-share rounding used to hide the defect by accident (the
    /// correction rounded to zero shares).
    #[test]
    fn constant_weights_flat_prices_place_only_initial_orders() {
        let symbols = ten_symbols();
        let weight = 0.1;
        let price = 200_00; // $200, flat across all 36 rebalances
        let targets: Vec<(Symbol, f64)> = symbols.iter().map(|s| (*s, weight)).collect();
        let prices: Vec<(Symbol, i64)> = symbols.iter().map(|s| (*s, price)).collect();

        // Two accounts: identical cost model, differing only in quantity_step
        // (whole-share default vs. fractional).
        let model = CostModel {
            commission_bps: 0.0,
            slippage_bps: 0.0,
            min_commission: 35, // $0.35/fill, matching the operator's repro
        };

        for step in [Shares::SCALE, 1_000] {
            let mut portfolio = Portfolio::with_quantity_step(10_000_00, model, step);
            portfolio.set_no_trade_band_bps(1.0); // any non-zero band stops the loop

            portfolio.rebalance_simple(&targets, &prices); // initial buy: 10 orders
            let equity_after_first = portfolio.total_equity(&prices);
            let cost_after_first = 10_000_00 - equity_after_first;
            assert_eq!(
                cost_after_first,
                10 * 35,
                "initial rebalance should place exactly 10 orders at $0.35 each, step={step}"
            );

            for _ in 0..35 {
                portfolio.rebalance_simple(&targets, &prices);
            }

            let equity_final = portfolio.total_equity(&prices);
            assert_eq!(
                equity_final, equity_after_first,
                "no further commission should be charged after the initial buy, step={step}"
            );
        }
    }

    /// Same setup with NO band set (default `0.0`), measured against the banded
    /// run above.
    ///
    /// This test used to assert that the unbanded run burned 360 × $0.35 = $126
    /// of commission, and that the band was what prevented it. That was only
    /// true while the portfolio could spend cash it did not have. Once a fill
    /// can no longer overdraw the account, both runs stop at the same $3.50 —
    /// ten opening commissions — because a cash account cannot finance a
    /// correction it cannot afford.
    ///
    /// So the band still changes how many orders are PLANNED (360 vs 45), but at
    /// a $10,000 account it no longer changes what is SPENT. Whether the band
    /// earns its keep at this size is now an open question rather than a settled
    /// one; the numbers below are pinned exactly so that any future change to
    /// either brake shows up as a failure instead of a drift.
    #[test]
    fn constant_weights_without_band_reproduces_the_feedback_loop() {
        let symbols = ten_symbols();
        let weight = 0.1;
        let price = 200_00;
        let targets: Vec<(Symbol, f64)> = symbols.iter().map(|s| (*s, weight)).collect();
        let prices: Vec<(Symbol, i64)> = symbols.iter().map(|s| (*s, price)).collect();

        let model = CostModel {
            commission_bps: 0.0,
            slippage_bps: 0.0,
            min_commission: 35,
        };
        let mut no_band = Portfolio::with_quantity_step(10_000_00, model, 1_000);
        let mut with_band = Portfolio::with_quantity_step(10_000_00, model, 1_000);
        with_band.set_no_trade_band_bps(1.0);

        let mut orders_no_band = 0usize;
        let mut orders_with_band = 0usize;
        for _ in 0..36 {
            no_band.rebalance_simple(&targets, &prices);
            with_band.rebalance_simple(&targets, &prices);
            orders_no_band += no_band.last_rebalance_order_count();
            orders_with_band += with_band.last_rebalance_order_count();
            assert!(no_band.cash() >= 0);
            assert!(with_band.cash() >= 0);
        }

        // Measured, not approximated. Without a band the portfolio still PLANS a
        // correction for all ten names on all 36 rebalances; the band suppresses
        // all but the opening buys plus a handful of corrections.
        assert_eq!(
            orders_no_band, 360,
            "unbanded: 10 names x 36 rebalances are all planned"
        );
        assert_eq!(orders_with_band, 45, "banded: openings plus a few corrections");

        // The money is the point, and here it is identical on both sides. A cash
        // account cannot finance a correction it cannot afford, so spending stops
        // at the same place with or without a band: ten opening commissions,
        // $0.35 each. The band changes how many orders are PLANNED, not what this
        // account can SPEND.
        let cost_no_band = 10_000_00 - no_band.total_equity(&prices);
        let cost_with_band = 10_000_00 - with_band.total_equity(&prices);
        assert_eq!(cost_no_band, 10 * 35, "ten opening commissions and no more");
        assert_eq!(
            cost_with_band, cost_no_band,
            "at this account size the band saves nothing once the overdraft is gone"
        );

        // Every rebalance is trimmed: the account is fully invested and the
        // targets cannot be met in full. That is the honest state, not an error.
        assert_eq!(no_band.trimmed_fill_count(), 36);
        assert_eq!(with_band.trimmed_fill_count(), 36);
    }

    // --- min_order_value ---

    #[test]
    fn min_order_value_skips_small_orders_and_places_large_ones() {
        let big = Symbol::new("BIG");
        let small = Symbol::new("SMALL");
        let price = 100_00; // $100/share
        let mut portfolio = Portfolio::new(1_000_00, CostModel::zero()); // $1,000
        // A $500 threshold: BIG's target ($500) clears it, SMALL's ($5) doesn't.
        portfolio.set_min_order_value(500_00);

        let targets = [(big, 0.5), (small, 0.005)];
        let prices = [(big, price), (small, price)];
        portfolio.rebalance_simple(&targets, &prices);

        // BIG: target_value = 500_00, price = 100_00 -> exactly 5 shares.
        assert_eq!(
            portfolio.position(&big).unwrap().quantity,
            Shares::from_whole(5)
        );
        // SMALL: order notional (5_00) is below the 500_00 threshold, so no
        // position is opened at all.
        assert!(portfolio.position(&small).is_none_or(|p| p.is_flat()));
    }

    #[test]
    fn min_order_value_default_zero_places_every_nonzero_order() {
        let portfolio_default_step = Shares::SCALE;
        let sym = Symbol::new("TINY");
        let price = 1_00; // $1/share
        let mut portfolio = Portfolio::with_quantity_step(10_00, CostModel::zero(), portfolio_default_step);
        assert_eq!(portfolio.min_order_value(), 0);

        portfolio.rebalance_simple(&[(sym, 0.5)], &[(sym, price)]);
        assert!(!portfolio.position(&sym).unwrap().is_flat());
    }

    // --- max_order_value ---

    #[test]
    fn max_order_value_truncates_rather_than_drops() {
        let sym = Symbol::new("BIG");
        let price = 100_00; // $100/share
        let mut portfolio = Portfolio::new(1_000_000_00, CostModel::zero()); // $1,000,000, whole-share step
        portfolio.set_max_order_value(30_000_00); // cap each order at $30,000

        // Target: 100% weight -> target value $1,000,000 / $100 = 10,000 shares,
        // which would exceed the cap; the order must be truncated to exactly
        // $30,000 / $100 = 300 shares, not dropped.
        portfolio.rebalance_simple(&[(sym, 1.0)], &[(sym, price)]);

        assert_eq!(
            portfolio.position(&sym).unwrap().quantity,
            Shares::from_whole(300)
        );
        assert_eq!(portfolio.last_rebalance_order_count(), 1);
        assert_eq!(portfolio.last_rebalance_notional(), 30_000_00);
    }

    #[test]
    fn max_order_value_default_zero_is_unlimited() {
        let sym = Symbol::new("BIG");
        let price = 100_00;
        let portfolio = Portfolio::new(1_000_000_00, CostModel::zero());
        assert_eq!(portfolio.max_order_value(), 0);

        let mut portfolio = portfolio;
        portfolio.rebalance_simple(&[(sym, 1.0)], &[(sym, price)]);
        assert_eq!(
            portfolio.position(&sym).unwrap().quantity,
            Shares::from_whole(10_000)
        );
    }

    /// `max_order_value` and `min_order_value` must compose sanely: when
    /// truncation lands the order's notional below the minimum, the order is
    /// skipped entirely rather than executed under-minimum.
    #[test]
    fn max_order_value_composing_with_min_order_value_truncation_below_minimum_skips() {
        let sym = Symbol::new("SMALL");
        let price = 100_00; // $100/share
        let mut portfolio =
            Portfolio::with_quantity_step(1_000_000_00, CostModel::zero(), 1_000); // 0.001-share step
        portfolio.set_max_order_value(250); // $2.50 cap: truncates to 0.025 share
        portfolio.set_min_order_value(300); // $3.00 minimum: $2.50 < $3.00

        portfolio.rebalance_simple(&[(sym, 1.0)], &[(sym, price)]);

        assert!(portfolio.position(&sym).is_none_or(|p| p.is_flat()));
        assert_eq!(portfolio.last_rebalance_order_count(), 0);
        assert_eq!(portfolio.last_rebalance_notional(), 0);
    }

    // --- last_rebalance_order_count / last_rebalance_notional ---

    /// The accessors must report exact values, reset every call, and include
    /// close-loop orders (not only target-loop orders).
    #[test]
    fn last_rebalance_stats_report_exact_values_including_close_orders() {
        let a = Symbol::new("A");
        let b = Symbol::new("B");
        let price = 100_00; // $100/share, flat throughout
        let mut portfolio = Portfolio::new(1_000_00, CostModel::zero()); // $1,000

        // Initial: both A and B opened, 5 shares ($500) each -> 2 orders,
        // $1,000 total notional.
        portfolio.rebalance_simple(&[(a, 0.5), (b, 0.5)], &[(a, price), (b, price)]);
        assert_eq!(portfolio.last_rebalance_order_count(), 2);
        assert_eq!(portfolio.last_rebalance_notional(), 1_000_00);

        // Second: only A targeted -> B is closed (close-loop order, $500) and
        // A is bought up to 100% ($500 -> $1,000, another $500 order).
        portfolio.rebalance_simple(&[(a, 1.0)], &[(a, price), (b, price)]);
        assert_eq!(portfolio.last_rebalance_order_count(), 2);
        assert_eq!(portfolio.last_rebalance_notional(), 1_000_00);

        // Third: no drift at all -> zero orders, zero notional (stats reset).
        portfolio.rebalance_simple(&[(a, 1.0)], &[(a, price)]);
        assert_eq!(portfolio.last_rebalance_order_count(), 0);
        assert_eq!(portfolio.last_rebalance_notional(), 0);
    }

    // --- no_trade_band_bps ---

    #[test]
    fn no_trade_band_leaves_small_drift_untouched_and_corrects_large_drift() {
        let a = Symbol::new("A");
        let b = Symbol::new("B");
        let price = 100_00;
        let mut portfolio = Portfolio::new(1_000_00, CostModel::zero()); // $1,000
        portfolio.set_no_trade_band_bps(500.0); // 5% band

        // Initial: 40% each leaves $200 cash so a later buy does not need an
        // overdraft (targets that sum above invested weight used to borrow
        // from cash that was not there).
        portfolio.rebalance_simple(&[(a, 0.4), (b, 0.4)], &[(a, price), (b, price)]);
        assert_eq!(portfolio.position(&a).unwrap().quantity, Shares::from_whole(4));
        assert_eq!(portfolio.position(&b).unwrap().quantity, Shares::from_whole(4));
        assert_eq!(portfolio.cash(), 200_00);

        // A drifts by 1% of equity (inside the 5% band) -> untouched.
        // B drifts by 15% of equity (outside the band) -> buys one share from cash.
        portfolio.rebalance_simple(&[(a, 0.41), (b, 0.55)], &[(a, price), (b, price)]);

        assert_eq!(
            portfolio.position(&a).unwrap().quantity,
            Shares::from_whole(4),
            "A's 1% drift is inside the 5% band and must not trade"
        );
        assert_eq!(
            portfolio.position(&b).unwrap().quantity,
            Shares::from_whole(5),
            "B's 15% drift is outside the 5% band and must be corrected"
        );
        assert!(portfolio.cash() >= 0);
    }

    #[test]
    fn no_trade_band_default_zero_corrects_any_drift() {
        let sym = Symbol::new("A");
        let price = 100_00;
        // Fractional quantity_step so a small drift doesn't get masked by
        // whole-share truncation (the band, not the step, is under test).
        let mut portfolio = Portfolio::with_quantity_step(1_000_00, CostModel::zero(), 1_000);
        assert_eq!(portfolio.no_trade_band_bps(), 0.0);

        portfolio.rebalance_simple(&[(sym, 0.5)], &[(sym, price)]);
        assert_eq!(portfolio.position(&sym).unwrap().quantity, Shares::from_whole(5));

        // A tiny 0.1% nudge still trades at the default (no band).
        portfolio.rebalance_simple(&[(sym, 0.501)], &[(sym, price)]);
        assert_ne!(portfolio.position(&sym).unwrap().quantity, Shares::from_whole(5));
    }

    // --- max_trades_per_rebalance ---

    /// With 10 positions all needing correction and a cap of 3, exactly 3
    /// orders execute and they are the 3 whose drift from target is largest
    /// — asserted by WHICH symbols traded, not just the count. Symbol S9 has
    /// the largest drift, then S8, then S7; S0..S6 must be untouched.
    #[test]
    fn max_trades_per_rebalance_keeps_largest_drift_orders() {
        let symbols = ten_symbols();
        let price = 100_00;
        let prices: Vec<(Symbol, i64)> = symbols.iter().map(|s| (*s, price)).collect();

        // Initial: 5% each leaves half the account in cash so later buys are
        // funded without an overdraft. Fractional quantity_step (0.001 share)
        // so small drifts don't truncate to zero and hide the ranking.
        let equal_targets: Vec<(Symbol, f64)> = symbols.iter().map(|s| (*s, 0.05)).collect();
        let mut portfolio = Portfolio::with_quantity_step(1_000_00, CostModel::zero(), 1_000); // $1,000
        portfolio.rebalance_simple(&equal_targets, &prices);
        for s in &symbols {
            assert_eq!(portfolio.position(s).unwrap().quantity, Shares::from_raw(500_000)); // 0.5 share
        }
        assert!(portfolio.cash() > 0);

        // New targets: S0..S6 stay put; S7/S8/S9 step up with increasing drift.
        // Absolute drifts: S9 > S8 > S7 >> S0..S6 (== 0), so a cap of 3 keeps
        // exactly those three buys.
        let new_targets: Vec<(Symbol, f64)> = symbols
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let w = if i >= 7 {
                    0.05 + 0.03 * (i as f64 - 6.0) // S7=0.08, S8=0.11, S9=0.14
                } else {
                    0.05
                };
                (*s, w)
            })
            .collect();

        portfolio.set_max_trades_per_rebalance(Some(3));
        portfolio.rebalance_simple(&new_targets, &prices);

        let traded: Vec<&str> = symbols
            .iter()
            .filter(|s| portfolio.position(s).unwrap().quantity != Shares::from_raw(500_000))
            .map(|s| s.as_str())
            .collect();
        let mut traded_sorted = traded.clone();
        traded_sorted.sort();
        assert_eq!(
            traded_sorted,
            vec!["S7", "S8", "S9"],
            "only the 3 largest-drift symbols should have traded, got {traded:?}"
        );
        assert!(portfolio.cash() >= 0);
    }

    /// Default `max_trades_per_rebalance` (`None`) places every order that
    /// clears the other constraints — no cap at all.
    #[test]
    fn max_trades_per_rebalance_default_none_places_all_orders() {
        let symbols = ten_symbols();
        let price = 100_00;
        let targets: Vec<(Symbol, f64)> = symbols.iter().map(|s| (*s, 0.1)).collect();
        let prices: Vec<(Symbol, i64)> = symbols.iter().map(|s| (*s, price)).collect();

        let mut portfolio = Portfolio::new(1_000_00, CostModel::zero());
        assert_eq!(portfolio.max_trades_per_rebalance(), None);
        portfolio.rebalance_simple(&targets, &prices);

        for s in &symbols {
            assert!(!portfolio.position(s).unwrap().is_flat());
        }
    }

    // --- max_rebalance_notional ---

    /// Shared fixture for the `max_rebalance_notional` tests below: a flat
    /// $100,000 account opening three positions in one rebalance, priced so
    /// notional equals target value exactly ($100/share). Target weights are
    /// staggered (S0 smallest, S2 largest) so the three orders have distinct
    /// drift and a deterministic priority order: S2 ($15,000) first, then S1
    /// ($10,000), then S0 ($5,000). Combined notional is $30,000.
    fn staggered_three_symbol_fixture() -> (Symbol, Symbol, Symbol, Portfolio, Vec<(Symbol, f64)>, Vec<(Symbol, i64)>) {
        let s0 = Symbol::new("S0");
        let s1 = Symbol::new("S1");
        let s2 = Symbol::new("S2");
        let price = 100_00; // $100/share
        let portfolio = Portfolio::new(100_000_00, CostModel::zero()); // $100,000
        let targets = vec![(s0, 0.05), (s1, 0.10), (s2, 0.15)];
        let prices = vec![(s0, price), (s1, price), (s2, price)];
        (s0, s1, s2, portfolio, targets, prices)
    }

    /// THE BUG ITSELF: tightening `max_order_value` to the period's remaining
    /// budget checks each order against the same $20,000 snapshot
    /// independently. Every one of S2 ($15,000), S1 ($10,000) and S0
    /// ($5,000) individually fits under $20,000, so that approach would
    /// place all three and spend $30,000 — 1.5x the cap. This test asserts
    /// the ACTUAL total spent is exactly the cap, which the old per-order
    /// approach would have failed (it would have produced $30,000, not
    /// $20,000): S2 is admitted in full ($15,000), leaving $5,000, which is
    /// exactly enough to truncate S1 to 50 shares ($5,000) and no more; S0
    /// never gets a turn.
    #[test]
    fn max_rebalance_notional_caps_total_spend_across_multiple_orders() {
        let (s0, s1, s2, mut portfolio, targets, prices) = staggered_three_symbol_fixture();
        portfolio.set_max_rebalance_notional(20_000_00);

        portfolio.rebalance_simple(&targets, &prices);

        assert_eq!(
            portfolio.last_rebalance_notional(),
            20_000_00,
            "must spend exactly the cap, not the ~$30,000 the old per-order snapshot approach would allow"
        );
        assert!(portfolio.last_rebalance_notional() <= 20_000_00);

        assert_eq!(portfolio.position(&s2).unwrap().quantity, Shares::from_whole(150));
        assert_eq!(portfolio.position(&s1).unwrap().quantity, Shares::from_whole(50));
        assert!(portfolio.position(&s0).is_none_or(|p| p.is_flat()));
    }

    /// When the cap binds, the orders admitted (fully or truncated) are the
    /// highest-drift ones, in priority order: S2 first (fully), then S1
    /// (truncated), and S0 — the smallest drift — never trades at all.
    #[test]
    fn max_rebalance_notional_preserves_drift_priority() {
        let (s0, s1, s2, mut portfolio, targets, prices) = staggered_three_symbol_fixture();
        portfolio.set_max_rebalance_notional(20_000_00);

        portfolio.rebalance_simple(&targets, &prices);

        assert!(!portfolio.position(&s2).unwrap().is_flat(), "S2 has the largest drift and must trade");
        assert!(!portfolio.position(&s1).unwrap().is_flat(), "S1 has the second-largest drift and must trade (truncated)");
        assert!(
            portfolio.position(&s0).is_none_or(|p| p.is_flat()),
            "S0 has the smallest drift and must not trade once the budget is exhausted"
        );
    }

    /// Truncation composes with `min_order_value`: when the remaining budget
    /// only leaves room to truncate an order below the minimum, that order is
    /// skipped (not placed under-minimum), and no further, smaller-priority
    /// orders are considered either — the budget is still exhausted.
    #[test]
    fn max_rebalance_notional_truncation_below_min_order_value_is_skipped() {
        let (s0, s1, s2, mut portfolio, targets, prices) = staggered_three_symbol_fixture();
        portfolio.set_max_rebalance_notional(20_000_00); // $5,000 left after S2
        portfolio.set_min_order_value(6_000_00); // truncated S1 ($5,000) falls below this

        portfolio.rebalance_simple(&targets, &prices);

        assert_eq!(
            portfolio.last_rebalance_notional(),
            15_000_00,
            "only S2's full $15,000 order should have executed"
        );
        assert_eq!(portfolio.position(&s2).unwrap().quantity, Shares::from_whole(150));
        assert!(
            portfolio.position(&s1).is_none_or(|p| p.is_flat()),
            "S1's truncated order falls below min_order_value and must be skipped"
        );
        assert!(portfolio.position(&s0).is_none_or(|p| p.is_flat()));
    }

    /// `max_rebalance_notional` composes with `max_trades_per_rebalance`:
    /// whichever binds first wins. Here the trade-count cap of 1 stops
    /// admission after S2 even though $2,000 of the $17,000 notional budget
    /// is still unused — the trade-count cap is the tighter constraint and
    /// it is respected exactly, without the notional cap reaching in to
    /// admit or truncate a second order.
    #[test]
    fn max_rebalance_notional_composes_with_max_trades_per_rebalance() {
        let (s0, s1, s2, mut portfolio, targets, prices) = staggered_three_symbol_fixture();
        portfolio.set_max_rebalance_notional(17_000_00);
        portfolio.set_max_trades_per_rebalance(Some(1));

        portfolio.rebalance_simple(&targets, &prices);

        assert_eq!(portfolio.last_rebalance_order_count(), 1);
        assert_eq!(
            portfolio.last_rebalance_notional(),
            15_000_00,
            "trade-count cap must stop admission after S2, leaving notional budget unused"
        );
        assert_eq!(portfolio.position(&s2).unwrap().quantity, Shares::from_whole(150));
        assert!(portfolio.position(&s1).is_none_or(|p| p.is_flat()));
        assert!(portfolio.position(&s0).is_none_or(|p| p.is_flat()));
    }

    /// Default `max_rebalance_notional` (`0`) is unlimited: all three orders
    /// execute in full, unchanged from behaviour before this cap existed.
    #[test]
    fn max_rebalance_notional_default_zero_is_unlimited() {
        let (s0, s1, s2, mut portfolio, targets, prices) = staggered_three_symbol_fixture();
        assert_eq!(portfolio.max_rebalance_notional(), 0);

        portfolio.rebalance_simple(&targets, &prices);

        assert_eq!(portfolio.last_rebalance_notional(), 30_000_00);
        assert_eq!(portfolio.position(&s0).unwrap().quantity, Shares::from_whole(50));
        assert_eq!(portfolio.position(&s1).unwrap().quantity, Shares::from_whole(100));
        assert_eq!(portfolio.position(&s2).unwrap().quantity, Shares::from_whole(150));
    }

    // --- determinism ---

    /// The same inputs must produce the same orders every run: guards
    /// against `FxHashMap` iteration order leaking into which positions get
    /// dropped when the trade cap binds.
    #[test]
    fn rebalance_is_deterministic_across_repeated_runs() {
        let symbols = ten_symbols();
        let price = 100_00;
        let prices: Vec<(Symbol, i64)> = symbols.iter().map(|s| (*s, price)).collect();
        let equal_targets: Vec<(Symbol, f64)> = symbols.iter().map(|s| (*s, 0.1)).collect();
        let new_targets: Vec<(Symbol, f64)> = symbols
            .iter()
            .enumerate()
            .map(|(i, s)| (*s, 0.10 + 0.01 * (i as f64 + 1.0)))
            .collect();

        fn run(
            symbols: &[Symbol],
            equal_targets: &[(Symbol, f64)],
            new_targets: &[(Symbol, f64)],
            prices: &[(Symbol, i64)],
        ) -> Vec<Shares> {
            let mut portfolio = Portfolio::with_quantity_step(1_000_00, CostModel::zero(), 1_000);
            portfolio.rebalance_simple(equal_targets, prices);
            portfolio.set_max_trades_per_rebalance(Some(3));
            portfolio.rebalance_simple(new_targets, prices);
            symbols
                .iter()
                .map(|s| portfolio.position(s).unwrap().quantity)
                .collect()
        }

        let first = run(&symbols, &equal_targets, &new_targets, &prices);
        for _ in 0..10 {
            let repeat = run(&symbols, &equal_targets, &new_targets, &prices);
            assert_eq!(first, repeat, "rebalance output must be deterministic");
        }
    }

    // === Cash affordability: buys never overdraw the account ===

    /// Operator repro (pre-fix measured values left in the comment):
    /// $1,000, 10 equal-weight names at $200, US-equities-tiered costs
    /// (`commission_bps=0.5`, `slippage_bps=2.0`, `min_commission=35`),
    /// `quantity_step=1_000`. Before the affordability gate this stabilised
    /// around cash −$5.30 / invested 100.55% after repeated rebalances — the
    /// portfolio was levered on an overdraft that does not exist at a broker.
    /// After the gate: cash stays non-negative and invested ≤ 100%.
    #[test]
    fn small_account_us_equities_never_overdraws_cash() {
        let symbols = ten_symbols();
        let weight = 0.1;
        let price = 200_00;
        let targets: Vec<(Symbol, f64)> = symbols.iter().map(|s| (*s, weight)).collect();
        let prices: Vec<(Symbol, i64)> = symbols.iter().map(|s| (*s, price)).collect();

        let model = CostModel {
            commission_bps: 0.5,
            slippage_bps: 2.0,
            min_commission: 35,
        };
        let mut portfolio = Portfolio::with_quantity_step(1_000_00, model, 1_000);

        for _ in 0..12 {
            portfolio.rebalance_simple(&targets, &prices);
            assert!(
                portfolio.cash() >= 0,
                "cash went negative: {}",
                portfolio.cash()
            );
        }

        let equity = portfolio.total_equity(&prices);
        assert!(equity > 0, "equity collapsed to {equity}");
        let invested_pct = (1.0 - portfolio.cash() as f64 / equity as f64) * 100.0;
        assert!(
            invested_pct <= 100.0,
            "invested {invested_pct:.4}% > 100% (cash={}, equity={equity})",
            portfolio.cash()
        );
        // The trim must be visible, not a silent clamp: this account cannot
        // fund ten min-commission buys at full target without cutting size.
        assert!(
            portfolio.trimmed_fill_count() > 0,
            "expected at least one buy trimmed on the $1,000 repro"
        );
        assert!(
            portfolio.trimmed_shortfall_cents() > 0,
            "expected positive shortfall cents on the $1,000 repro"
        );
    }

    /// Grid: account size × cost model × quantity_step. After every rebalance
    /// `cash() >= 0`. Every cell actually runs.
    #[test]
    fn cash_non_negative_across_account_cost_and_step_grid() {
        let symbols = ten_symbols();
        let weight = 0.1;
        let price = 200_00;
        let targets: Vec<(Symbol, f64)> = symbols.iter().map(|s| (*s, weight)).collect();
        let prices: Vec<(Symbol, i64)> = symbols.iter().map(|s| (*s, price)).collect();

        let account_sizes = [1_000_00, 10_000_00, 100_000_00, 1_000_000_00];
        let cost_models = [
            CostModel::zero(),
            // US equities (IBKR Tiered) — ADR-0003 reference parameters.
            CostModel {
                commission_bps: 0.5,
                slippage_bps: 2.0,
                min_commission: 35,
            },
            // Commission-only: bps fee, no slippage, no floor.
            CostModel {
                commission_bps: 10.0,
                slippage_bps: 0.0,
                min_commission: 0,
            },
        ];
        let steps = [Shares::SCALE, 1_000]; // whole-share, fractional 0.001

        for &cash0 in &account_sizes {
            for &model in &cost_models {
                for &step in &steps {
                    let mut portfolio = Portfolio::with_quantity_step(cash0, model, step);
                    for _ in 0..6 {
                        portfolio.rebalance_simple(&targets, &prices);
                        assert!(
                            portfolio.cash() >= 0,
                            "cash={} < 0 for cash0={cash0}, model={model:?}, step={step}",
                            portfolio.cash()
                        );
                    }
                }
            }
        }
    }

    /// Truncation picks the largest affordable step-multiple, not `cash/price`.
    /// With a $0.35 floor on a $100 account at $10/share, ten shares cost
    /// $100.35 and overshoot; nine shares cost $90.35 and fit. A naive
    /// `cash / price` would still try ten.
    #[test]
    fn affordability_picks_largest_step_multiple_under_min_commission() {
        let model = CostModel {
            commission_bps: 0.0,
            slippage_bps: 0.0,
            min_commission: 35,
        };
        let mut portfolio = Portfolio::with_quantity_step(100_00, model, Shares::SCALE);
        let sym = aapl();
        // Request the full account as a single name: size_qty wants ~10 shares.
        portfolio.rebalance_simple(&[(sym, 1.0)], &[(sym, 10_00)]);

        let pos = portfolio.position(&sym).expect("position opened");
        assert_eq!(
            pos.quantity,
            Shares::from_whole(9),
            "expected 9 shares (10 overshoots by the $0.35 floor)"
        );
        assert!(portfolio.cash() >= 0);
        // 9 * $10 + $0.35 = $90.35 → cash left $9.65.
        assert_eq!(portfolio.cash(), 100_00 - 90_00 - 35);
        assert_eq!(portfolio.trimmed_fill_count(), 1);
        // Requested ~10 shares ($100 notional) vs filled 9 ($90): $10 shortfall.
        assert_eq!(portfolio.trimmed_shortfall_cents(), 10_00);
    }

    /// A quantity that cannot fund even one step is skipped — no zero fill
    /// that would still levy `min_commission`.
    #[test]
    fn unaffordable_step_produces_no_fill_and_no_cost() {
        let model = CostModel {
            commission_bps: 0.0,
            slippage_bps: 0.0,
            min_commission: 50, // $0.50 floor
        };
        // One whole share at $100 is sizeable, but $100 cash cannot also cover
        // the $0.50 floor, so the buy must be skipped rather than filled at
        // zero-with-cost.
        let mut portfolio = Portfolio::with_quantity_step(100_00, model, Shares::SCALE);
        let sym = aapl();
        portfolio.rebalance_simple(&[(sym, 1.0)], &[(sym, 100_00)]);

        assert!(portfolio.position(&sym).is_none_or(|p| p.is_flat()));
        assert_eq!(
            portfolio.cash(),
            100_00,
            "skipped fill must not charge commission"
        );
        assert_eq!(portfolio.trimmed_fill_count(), 1);
        assert_eq!(portfolio.trimmed_shortfall_cents(), 100_00);
    }

    /// Sells raise cash and must never be truncated by the affordability gate,
    /// even when cash is already zero before the sale proceeds land.
    #[test]
    fn sells_are_never_truncated_by_affordability() {
        let mut portfolio = Portfolio::new(100_00, CostModel::zero());
        let sym = aapl();
        // Exact whole-share buy leaves cash at zero.
        portfolio.rebalance_simple(&[(sym, 1.0)], &[(sym, 100_00)]);
        let qty_before = portfolio.position(&sym).unwrap().quantity;
        assert_eq!(qty_before, Shares::from_whole(1));
        assert_eq!(portfolio.cash(), 0);
        assert_eq!(portfolio.trimmed_fill_count(), 0);

        // Full close at a higher price must liquidate every share and credit
        // the full notional — a sell is never an affordability problem.
        let closed = portfolio.close_position_at(sym, 110_00);
        assert!(closed);
        assert!(portfolio.position(&sym).unwrap().is_flat());
        assert_eq!(portfolio.cash(), 110_00);
        assert_eq!(
            portfolio.trimmed_fill_count(),
            0,
            "sells must not increment trimmed_fill_count"
        );
        assert_eq!(portfolio.trimmed_shortfall_cents(), 0);
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
