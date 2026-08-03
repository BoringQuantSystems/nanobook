//! Fast backtest bridge: simulate portfolio returns from a pre-computed weight schedule.
//!
//! Python computes the weight schedule (factor models, signals, etc.),
//! Rust handles the inner simulation loop (rebalance, track positions, compute returns).

use std::collections::{HashMap, HashSet, hash_map::Entry};

use crate::portfolio::metrics::{
    DrawdownEvent, Metrics, compute_metrics, drawdown_series, rolling_sharpe,
};
use crate::portfolio::{CostModel, Portfolio};
use crate::types::Symbol;

/// Optional stop simulation configuration.
#[derive(Clone, Debug, Default)]
pub struct BacktestStopConfig {
    /// Fixed stop distance as fraction of entry price (e.g. 0.10 = 10%).
    pub fixed_stop_pct: Option<f64>,
    /// Trailing stop distance as fraction from watermark (e.g. 0.05 = 5%).
    pub trailing_stop_pct: Option<f64>,
    /// ATR multiple for adaptive trailing stop.
    pub atr_multiple: Option<f64>,
    /// Rolling period for ATR approximation (absolute close-to-close changes).
    pub atr_period: usize,
}

impl BacktestStopConfig {
    fn sanitized(&self) -> Option<Self> {
        let fixed = sanitize_pct(self.fixed_stop_pct);
        let trailing = sanitize_pct(self.trailing_stop_pct);
        let atr_multiple = sanitize_positive(self.atr_multiple);
        let atr_period = self.atr_period.max(1);

        if fixed.is_none() && trailing.is_none() && atr_multiple.is_none() {
            return None;
        }

        Some(Self {
            fixed_stop_pct: fixed,
            trailing_stop_pct: trailing,
            atr_multiple,
            atr_period,
        })
    }
}

/// Backtest options for v0.9 API surface.
#[derive(Clone, Debug, Default)]
pub struct BacktestBridgeOptions {
    /// Optional stop simulation configuration.
    pub stop_cfg: Option<BacktestStopConfig>,
    /// Order sizing granularity, in micro-shares (see [`crate::portfolio::Shares`]).
    /// `None` keeps the default: whole shares (`Shares::SCALE`), so existing
    /// callers are unaffected. Set e.g. `Some(1_000)` for Alpaca's 0.001-share
    /// minimum.
    pub quantity_step: Option<i64>,
    /// Minimum order notional (cents). `None` keeps the default: `0`, i.e. no
    /// minimum. See [`crate::portfolio::Portfolio::set_min_order_value`].
    pub min_order_value: Option<i64>,
    /// No-trade band, in basis points of equity. `None` keeps the default:
    /// `0.0`, i.e. no band. See
    /// [`crate::portfolio::Portfolio::set_no_trade_band_bps`].
    pub no_trade_band_bps: Option<f64>,
    /// Hard cap on orders placed per rebalance. `None` keeps the default: no
    /// cap. See [`crate::portfolio::Portfolio::set_max_trades_per_rebalance`].
    pub max_trades_per_rebalance: Option<usize>,
    /// Maximum order notional (cents). `None` keeps the default: `0`, i.e. no
    /// maximum. See [`crate::portfolio::Portfolio::set_max_order_value`].
    pub max_order_value: Option<i64>,
    /// Hard cap on orders placed across all rebalances that share the same
    /// `period_day_ordinal`. `None` keeps the default: no cap. Inert (no-op)
    /// if `period_day_ordinal` is not supplied, even when this is `Some`.
    pub max_trades_per_day: Option<usize>,
    /// Hard cap on orders placed across all rebalances that share the same
    /// `period_month_ordinal`. `None` keeps the default: no cap. Inert
    /// (no-op) if `period_month_ordinal` is not supplied, even when this is
    /// `Some`.
    pub max_trades_per_month: Option<usize>,
    /// Maximum total absolute notional (cents) traded across all rebalances
    /// that share the same `period_month_ordinal`. `None` keeps the default:
    /// no cap. Inert (no-op) if `period_month_ordinal` is not supplied, even
    /// when this is `Some`.
    pub max_traded_value_per_month: Option<i64>,
    /// Per-period day ordinal, parallel to `weight_schedule`/`price_schedule`
    /// (same length, or the bridge returns an empty result). A monotonically
    /// non-decreasing integer identifying which calendar day a period falls
    /// on — e.g. `date.toordinal()`. This crate has no calendar of its own,
    /// so the caller supplies it; consecutive periods sharing a day is normal
    /// for a 24/7 venue. `None` disables every day-scoped budget.
    pub period_day_ordinal: Option<Vec<i64>>,
    /// Per-period month ordinal, parallel to `weight_schedule`/`price_schedule`
    /// (same length, or the bridge returns an empty result). A monotonically
    /// non-decreasing integer identifying which calendar month a period falls
    /// on — e.g. `year * 12 + month`. `None` disables every month-scoped
    /// budget.
    pub period_month_ordinal: Option<Vec<i64>>,
}

#[derive(Clone, Debug, Copy)]
pub struct BarPrices {
    pub open: i64,
    pub high: i64,
    pub low: i64,
    pub close: i64,
}

#[derive(Clone, Debug, Copy, PartialEq)]
pub enum FillPolicy {
    SignalBarClose,
    NextBarOpen,
    NextBarTypical,
}

/// Stop event emitted by stop-aware backtest simulation.
#[derive(Clone, Debug)]
pub struct BacktestStopEvent {
    /// Period index where the stop triggered.
    pub period_index: usize,
    /// Symbol that was exited.
    pub symbol: Symbol,
    /// Stop threshold that was breached.
    pub trigger_price: i64,
    /// Executed exit price.
    pub exit_price: i64,
    /// Trigger reason: `fixed`, `trailing`, `atr`.
    pub reason: &'static str,
}

/// Trade lifecycle detected from target-weight transitions.
#[derive(Debug, Clone, PartialEq)]
pub struct AttributionTrade {
    pub symbol: Symbol,
    pub entry_index: usize,
    pub exit_index: Option<usize>,
    pub entry_weight: f64,
    pub exit_weight: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttributionResult {
    pub contributions: Vec<Vec<(Symbol, f64)>>,
    pub cumulative_contributions: Vec<Vec<(Symbol, f64)>>,
    pub trades: Vec<AttributionTrade>,
}

/// Aggregate trade lifecycle counts for reporting.
#[derive(Debug, Clone, PartialEq)]
pub struct TradeAnalytics {
    pub trade_count: usize,
    pub open_trade_count: usize,
    pub closed_trade_count: usize,
}

#[derive(Debug, Clone)]
pub struct TearSheet {
    pub monthly_returns: Vec<Vec<f64>>,
    pub rolling_sharpe: Vec<f64>,
    pub drawdown_events: Vec<DrawdownEvent>,
    pub trade_analytics: TradeAnalytics,
}

pub struct BacktestBridgeResult {
    /// Per-period returns.
    pub returns: Vec<f64>,
    /// Equity curve (one entry per date + initial equity).
    pub equity_curve: Vec<i64>,
    /// Final portfolio state.
    pub final_cash: i64,
    /// Computed metrics (None if no returns).
    pub metrics: Option<Metrics>,
    /// Per-period holdings as (symbol, weight).
    pub holdings: Vec<Vec<(Symbol, f64)>>,
    /// Per-period per-symbol close-to-close returns.
    pub symbol_returns: Vec<Vec<(Symbol, f64)>>,
    /// Stop-trigger events (empty when stop simulation disabled or no triggers).
    pub stop_events: Vec<BacktestStopEvent>,
    /// Rebalance indices skipped when the fill policy needs the next bar.
    pub skipped_rebalances: Vec<usize>,
}

/// Simulate portfolio returns from a pre-computed weight schedule.
///
/// Compatibility wrapper (v0.7/v0.8 behavior): stop simulation disabled.
pub fn backtest_weights(
    weight_schedule: &[Vec<(Symbol, f64)>],
    price_schedule: &[Vec<(Symbol, BarPrices)>],
    initial_cash_cents: i64,
    cost_model: CostModel,
    fill_policy: FillPolicy,
    periods_per_year: f64,
    risk_free: f64,
) -> BacktestBridgeResult {
    backtest_weights_with_options(
        weight_schedule,
        price_schedule,
        initial_cash_cents,
        cost_model,
        fill_policy,
        periods_per_year,
        risk_free,
        BacktestBridgeOptions::default(),
    )
}

/// Simulate portfolio returns from a pre-computed weight schedule with optional v0.9 features.
///
/// Returns an empty result (no returns, no metrics) for invalid inputs:
/// mismatched schedule lengths, non-positive cash, NaN/Inf weights,
/// or negative prices.
#[allow(clippy::too_many_arguments)]
pub fn backtest_weights_with_options(
    weight_schedule: &[Vec<(Symbol, f64)>],
    price_schedule: &[Vec<(Symbol, BarPrices)>],
    initial_cash_cents: i64,
    cost_model: CostModel,
    fill_policy: FillPolicy,
    periods_per_year: f64,
    risk_free: f64,
    options: BacktestBridgeOptions,
) -> BacktestBridgeResult {
    if !valid_inputs(weight_schedule, price_schedule, initial_cash_cents) {
        return empty_result(initial_cash_cents);
    }
    if let Some(days) = &options.period_day_ordinal
        && days.len() != weight_schedule.len()
    {
        return empty_result(initial_cash_cents);
    }
    if let Some(months) = &options.period_month_ordinal
        && months.len() != weight_schedule.len()
    {
        return empty_result(initial_cash_cents);
    }

    let stop_cfg = options
        .stop_cfg
        .as_ref()
        .and_then(BacktestStopConfig::sanitized);

    let mut portfolio = Portfolio::new(initial_cash_cents, cost_model);
    if let Some(step) = options.quantity_step {
        portfolio.set_quantity_step(step);
    }
    if let Some(value) = options.min_order_value {
        portfolio.set_min_order_value(value);
    }
    if let Some(value) = options.max_order_value {
        portfolio.set_max_order_value(value);
    }
    if let Some(bps) = options.no_trade_band_bps {
        portfolio.set_no_trade_band_bps(bps);
    }
    if options.max_trades_per_rebalance.is_some() {
        portfolio.set_max_trades_per_rebalance(options.max_trades_per_rebalance);
    }

    // Windowed (day/month) budgets: the bridge owns the calendar (via the
    // caller-supplied ordinals) since Portfolio has no concept of dates.
    // Every field here defaults to inert so unset options reproduce the
    // exact behaviour above with no per-period overriding at all.
    let base_max_trades_per_rebalance = options.max_trades_per_rebalance;
    let base_max_order_value = options.max_order_value.unwrap_or(0);
    let has_day_budget =
        options.max_trades_per_day.is_some() && options.period_day_ordinal.is_some();
    let has_month_trade_budget =
        options.max_trades_per_month.is_some() && options.period_month_ordinal.is_some();
    let has_month_value_budget =
        options.max_traded_value_per_month.is_some() && options.period_month_ordinal.is_some();
    let windowed_budgets_active = has_day_budget || has_month_trade_budget || has_month_value_budget;

    let mut trades_used_today: usize = 0;
    let mut trades_used_month: usize = 0;
    let mut value_used_month: i64 = 0;
    let mut prev_day_ordinal: Option<i64> = None;
    let mut prev_month_ordinal: Option<i64> = None;

    let mut equity_curve = Vec::with_capacity(weight_schedule.len() + 1);
    equity_curve.push(initial_cash_cents);

    let mut holdings = Vec::with_capacity(weight_schedule.len());
    let mut symbol_returns = Vec::with_capacity(weight_schedule.len());
    let mut stop_events = Vec::new();
    let mut skipped_rebalances = Vec::new();

    let mut prev_prices: HashMap<Symbol, i64> = HashMap::new();
    let mut stop_trackers: HashMap<Symbol, StopTracker> = HashMap::new();

    for (period_index, (weights, bars)) in weight_schedule
        .iter()
        .zip(price_schedule.iter())
        .enumerate()
    {
        let close_prices: Vec<(Symbol, i64)> = bars
            .iter()
            .map(|&(symbol, bar_prices)| (symbol, bar_prices.close))
            .collect();
        let price_map: HashMap<Symbol, i64> = close_prices.iter().copied().collect();

        let mut period_symbol_returns = Vec::with_capacity(bars.len());
        for &(sym, bp) in bars {
            let px = bp.close;
            let ret = prev_prices
                .get(&sym)
                .copied()
                .and_then(|p0| {
                    if p0 > 0 && px > 0 {
                        Some((px - p0) as f64 / p0 as f64)
                    } else {
                        None
                    }
                })
                .unwrap_or(f64::NAN);
            period_symbol_returns.push((sym, ret));
        }
        period_symbol_returns.sort_by_key(|(sym, _)| *sym);
        symbol_returns.push(period_symbol_returns);

        if windowed_budgets_active {
            if let Some(days) = &options.period_day_ordinal {
                let day = days[period_index];
                if prev_day_ordinal != Some(day) {
                    trades_used_today = 0;
                    prev_day_ordinal = Some(day);
                }
            }
            if let Some(months) = &options.period_month_ordinal {
                let month = months[period_index];
                if prev_month_ordinal != Some(month) {
                    trades_used_month = 0;
                    value_used_month = 0;
                    prev_month_ordinal = Some(month);
                }
            }

            let mut cap_candidates: Vec<usize> = Vec::new();
            if let Some(base) = base_max_trades_per_rebalance {
                cap_candidates.push(base);
            }
            if has_day_budget {
                let max_day = options.max_trades_per_day.expect("has_day_budget implies Some");
                cap_candidates.push(max_day.saturating_sub(trades_used_today));
            }
            if has_month_trade_budget {
                let max_month = options
                    .max_trades_per_month
                    .expect("has_month_trade_budget implies Some");
                cap_candidates.push(max_month.saturating_sub(trades_used_month));
            }

            let mut effective_max_order_value = base_max_order_value;
            if has_month_value_budget {
                let month_value_budget = options
                    .max_traded_value_per_month
                    .expect("has_month_value_budget implies Some");
                let remaining = month_value_budget.saturating_sub(value_used_month);
                if remaining <= 0 {
                    // Value budget exhausted for the month: no more trading
                    // until the next month ordinal resets it.
                    cap_candidates.push(0);
                } else {
                    effective_max_order_value = if base_max_order_value > 0 {
                        base_max_order_value.min(remaining)
                    } else {
                        remaining
                    };
                }
            }

            portfolio.set_max_trades_per_rebalance(cap_candidates.into_iter().min());
            portfolio.set_max_order_value(effective_max_order_value);
        }

        if let Some(fill_prices) = fill_prices_for_period(price_schedule, period_index, fill_policy)
        {
            portfolio.rebalance_simple(weights, &fill_prices);
            if windowed_budgets_active {
                let orders = portfolio.last_rebalance_order_count();
                trades_used_today += orders;
                trades_used_month += orders;
                value_used_month =
                    value_used_month.saturating_add(portfolio.last_rebalance_notional());
            }
        } else {
            skipped_rebalances.push(period_index);
        }

        // Optional stop simulation runs after target rebalance on each bar.
        if let Some(cfg) = stop_cfg.as_ref() {
            apply_stop_cfg(
                &mut portfolio,
                &price_map,
                period_index,
                cfg,
                &mut stop_trackers,
                &mut stop_events,
            );
        }

        // Record return for this period.
        portfolio.record_return(&close_prices);

        // Track holdings and equity.
        let mut period_holdings = portfolio.current_weights(&close_prices);
        period_holdings.sort_by_key(|(sym, _)| *sym);
        holdings.push(period_holdings);

        let equity = portfolio.total_equity(&close_prices);
        equity_curve.push(equity);

        prev_prices = price_map;
    }

    let returns = portfolio.returns().to_vec();
    let metrics = compute_metrics(&returns, periods_per_year, risk_free);

    BacktestBridgeResult {
        returns,
        equity_curve,
        final_cash: portfolio.cash(),
        metrics,
        holdings,
        symbol_returns,
        stop_events,
        skipped_rebalances,
    }
}

fn fill_prices_for_period(
    price_schedule: &[Vec<(Symbol, BarPrices)>],
    period_index: usize,
    fill_policy: FillPolicy,
) -> Option<Vec<(Symbol, i64)>> {
    match fill_policy {
        FillPolicy::SignalBarClose => Some(
            price_schedule[period_index]
                .iter()
                .map(|&(symbol, bar_prices)| (symbol, bar_prices.close))
                .collect(),
        ),
        FillPolicy::NextBarOpen => {
            let next = price_schedule.get(period_index + 1)?;
            Some(
                next.iter()
                    .map(|&(symbol, bar_prices)| (symbol, bar_prices.open))
                    .collect(),
            )
        }
        FillPolicy::NextBarTypical => {
            let next = price_schedule.get(period_index + 1)?;
            Some(
                next.iter()
                    .map(|&(symbol, bar_prices)| {
                        (
                            symbol,
                            (bar_prices.high + bar_prices.low + bar_prices.close) / 3,
                        )
                    })
                    .collect(),
            )
        }
    }
}

/// Decompose a weight/return schedule into per-symbol contributions and trades.
///
/// Each period contribution is `weight * period_return` for that symbol. Cumulative
/// contribution is a simple running sum per symbol. Trade events are derived from
/// target-weight transitions: zero→non-zero opens a trade, non-zero→zero closes it.
pub fn tear_sheet(
    result: &BacktestBridgeResult,
    rolling_window: usize,
    periods_per_year: usize,
) -> TearSheet {
    let attribution = decompose_backtest(&result.holdings, &result.symbol_returns);
    let equity: Vec<f64> = result
        .equity_curve
        .iter()
        .map(|value| *value as f64)
        .collect();
    TearSheet {
        monthly_returns: monthly_return_matrix(&result.returns, 21),
        rolling_sharpe: rolling_sharpe(&result.returns, rolling_window, periods_per_year),
        drawdown_events: drawdown_series(&equity),
        trade_analytics: TradeAnalytics {
            trade_count: attribution.trades.len(),
            open_trade_count: attribution
                .trades
                .iter()
                .filter(|trade| trade.exit_index.is_none())
                .count(),
            closed_trade_count: attribution
                .trades
                .iter()
                .filter(|trade| trade.exit_index.is_some())
                .count(),
        },
    }
}

fn monthly_return_matrix(returns: &[f64], periods_per_month: usize) -> Vec<Vec<f64>> {
    if periods_per_month == 0 {
        return Vec::new();
    }
    returns
        .chunks(periods_per_month)
        .map(|chunk| chunk.iter().fold(1.0, |acc, value| acc * (1.0 + value)) - 1.0)
        .collect::<Vec<_>>()
        .chunks(12)
        .map(|year| year.to_vec())
        .collect()
}

pub fn decompose_backtest(
    weight_schedule: &[Vec<(Symbol, f64)>],
    return_schedule: &[Vec<(Symbol, f64)>],
) -> AttributionResult {
    if weight_schedule.len() != return_schedule.len() {
        return AttributionResult {
            contributions: Vec::new(),
            cumulative_contributions: Vec::new(),
            trades: Vec::new(),
        };
    }

    let mut cumulative: HashMap<Symbol, f64> = HashMap::new();
    let mut previous_weights: HashMap<Symbol, f64> = HashMap::new();
    let mut open_trades: HashMap<Symbol, (usize, f64)> = HashMap::new();
    let mut contributions = Vec::with_capacity(weight_schedule.len());
    let mut cumulative_contributions = Vec::with_capacity(weight_schedule.len());
    let mut trades = Vec::new();

    for (period_index, (weights, returns)) in
        weight_schedule.iter().zip(return_schedule).enumerate()
    {
        let weight_map: HashMap<Symbol, f64> = weights.iter().copied().collect();
        let return_map: HashMap<Symbol, f64> = returns.iter().copied().collect();
        let mut symbols: Vec<Symbol> = weight_map
            .keys()
            .chain(return_map.keys())
            .chain(previous_weights.keys())
            .copied()
            .collect();
        symbols.sort_unstable();
        symbols.dedup();

        let mut period_contrib = Vec::new();
        let mut period_cumulative = Vec::new();

        for symbol in symbols {
            let weight = weight_map
                .get(&symbol)
                .copied()
                .filter(|value| value.is_finite())
                .unwrap_or(0.0);
            let previous = previous_weights
                .get(&symbol)
                .copied()
                .filter(|value| value.is_finite())
                .unwrap_or(0.0);

            if previous == 0.0 && weight != 0.0 {
                open_trades.insert(symbol, (period_index, weight));
            } else if previous != 0.0
                && weight == 0.0
                && let Some((entry_index, entry_weight)) = open_trades.remove(&symbol)
            {
                trades.push(AttributionTrade {
                    symbol,
                    entry_index,
                    exit_index: Some(period_index),
                    entry_weight,
                    exit_weight: previous,
                });
            }

            let period_return = return_map
                .get(&symbol)
                .copied()
                .filter(|value| value.is_finite())
                .unwrap_or(0.0);
            let contribution = weight * period_return;
            if contribution != 0.0 || weight != 0.0 || previous != 0.0 {
                let running = cumulative.entry(symbol).or_insert(0.0);
                *running += contribution;
                period_contrib.push((symbol, contribution));
                period_cumulative.push((symbol, *running));
            }
        }

        period_contrib.sort_by_key(|(symbol, _)| *symbol);
        period_cumulative.sort_by_key(|(symbol, _)| *symbol);
        contributions.push(period_contrib);
        cumulative_contributions.push(period_cumulative);
        previous_weights = weight_map;
    }

    for (symbol, (entry_index, entry_weight)) in open_trades {
        let exit_weight = previous_weights
            .get(&symbol)
            .copied()
            .unwrap_or(entry_weight);
        trades.push(AttributionTrade {
            symbol,
            entry_index,
            exit_index: None,
            entry_weight,
            exit_weight,
        });
    }
    trades.sort_by_key(|trade| (trade.entry_index, trade.symbol));

    AttributionResult {
        contributions,
        cumulative_contributions,
        trades,
    }
}

fn valid_inputs(
    weight_schedule: &[Vec<(Symbol, f64)>],
    price_schedule: &[Vec<(Symbol, BarPrices)>],
    initial_cash_cents: i64,
) -> bool {
    if weight_schedule.len() != price_schedule.len() {
        return false;
    }
    if initial_cash_cents <= 0 {
        return false;
    }
    for (weights, prices) in weight_schedule.iter().zip(price_schedule.iter()) {
        for &(_, w) in weights {
            if !w.is_finite() {
                return false;
            }
        }
        for &(_, bp) in prices {
            if bp.open < 0 || bp.high < 0 || bp.low < 0 || bp.close < 0 {
                return false;
            }
            if bp.high < bp.low {
                return false;
            }
        }
    }

    true
}

fn empty_result(initial_cash_cents: i64) -> BacktestBridgeResult {
    BacktestBridgeResult {
        returns: Vec::new(),
        equity_curve: vec![initial_cash_cents],
        final_cash: initial_cash_cents,
        metrics: None,
        holdings: Vec::new(),
        symbol_returns: Vec::new(),
        stop_events: Vec::new(),
        skipped_rebalances: Vec::new(),
    }
}

#[derive(Clone, Debug)]
struct StopTracker {
    side: i8, // +1 long, -1 short
    entry_price: i64,
    reference_price: i64,
    last_price: i64,
    abs_changes: Vec<i64>,
}

impl StopTracker {
    fn new(entry_price: i64, side: i8) -> Self {
        Self {
            side,
            entry_price,
            reference_price: entry_price,
            last_price: entry_price,
            abs_changes: Vec::new(),
        }
    }

    fn update(&mut self, price: i64, atr_period: usize) {
        if price <= 0 {
            return;
        }

        let delta = (price - self.last_price).abs();
        self.abs_changes.push(delta);
        let keep = atr_period.max(1) * 6;
        if self.abs_changes.len() > keep {
            let drop_n = self.abs_changes.len() - keep;
            self.abs_changes.drain(..drop_n);
        }

        self.last_price = price;
        if self.side > 0 {
            self.reference_price = self.reference_price.max(price);
        } else {
            self.reference_price = self.reference_price.min(price);
        }
    }

    fn atr(&self, atr_period: usize) -> Option<f64> {
        if self.abs_changes.is_empty() {
            return None;
        }

        let k = atr_period.max(1).min(self.abs_changes.len());
        // Safe: k is bounded by [1, abs_changes.len()], so len() - k >= 0
        let tail = &self.abs_changes[self.abs_changes.len() - k..];
        let mean = tail.iter().map(|x| *x as f64).sum::<f64>() / k as f64;
        Some(mean)
    }
}

fn apply_stop_cfg(
    portfolio: &mut Portfolio,
    price_map: &HashMap<Symbol, i64>,
    period_index: usize,
    cfg: &BacktestStopConfig,
    trackers: &mut HashMap<Symbol, StopTracker>,
    stop_events: &mut Vec<BacktestStopEvent>,
) {
    let open_positions: Vec<(Symbol, i64, i64)> = portfolio
        .positions()
        .filter_map(|(sym, pos)| {
            if pos.is_flat() {
                return None;
            }
            let px = price_map.get(sym).copied()?;
            if px <= 0 {
                return None;
            }
            Some((*sym, pos.quantity.raw(), px))
        })
        .collect();

    let open_symbols: HashSet<Symbol> = open_positions.iter().map(|(s, _, _)| *s).collect();
    trackers.retain(|sym, _| open_symbols.contains(sym));

    for (sym, qty, price) in open_positions {
        let side = if qty >= 0 { 1 } else { -1 };

        let tracker = match trackers.entry(sym) {
            Entry::Occupied(mut entry) => {
                if entry.get().side != side {
                    entry.insert(StopTracker::new(price, side));
                } else {
                    entry.get_mut().update(price, cfg.atr_period);
                }
                entry.into_mut()
            }
            Entry::Vacant(entry) => entry.insert(StopTracker::new(price, side)),
        };

        let Some((stop_level, reason)) = effective_stop_level(cfg, tracker) else {
            continue;
        };

        let breached = if side > 0 {
            price <= stop_level
        } else {
            price >= stop_level
        };

        if breached {
            let closed = portfolio.close_position_at(sym, price);
            if closed {
                stop_events.push(BacktestStopEvent {
                    period_index,
                    symbol: sym,
                    trigger_price: stop_level,
                    exit_price: price,
                    reason,
                });
                trackers.remove(&sym);
            }
        }
    }
}

fn effective_stop_level(
    cfg: &BacktestStopConfig,
    tracker: &StopTracker,
) -> Option<(i64, &'static str)> {
    let mut candidates = Vec::new();

    if let Some(p) = cfg.fixed_stop_pct {
        let level = if tracker.side > 0 {
            (tracker.entry_price as f64 * (1.0 - p)).round() as i64
        } else {
            (tracker.entry_price as f64 * (1.0 + p)).round() as i64
        }
        .max(1);
        candidates.push((level, "fixed"));
    }

    if let Some(p) = cfg.trailing_stop_pct {
        let level = if tracker.side > 0 {
            (tracker.reference_price as f64 * (1.0 - p)).round() as i64
        } else {
            (tracker.reference_price as f64 * (1.0 + p)).round() as i64
        }
        .max(1);
        candidates.push((level, "trailing"));
    }

    if let Some(mult) = cfg.atr_multiple
        && let Some(atr) = tracker.atr(cfg.atr_period)
    {
        let level = if tracker.side > 0 {
            (tracker.reference_price as f64 - mult * atr).round() as i64
        } else {
            (tracker.reference_price as f64 + mult * atr).round() as i64
        }
        .max(1);
        candidates.push((level, "atr"));
    }

    if candidates.is_empty() {
        return None;
    }

    if tracker.side > 0 {
        candidates.into_iter().max_by_key(|(level, _)| *level)
    } else {
        candidates.into_iter().min_by_key(|(level, _)| *level)
    }
}

fn sanitize_pct(v: Option<f64>) -> Option<f64> {
    v.filter(|x| x.is_finite() && *x > 0.0 && *x < 1.0)
}

fn sanitize_positive(v: Option<f64>) -> Option<f64> {
    v.filter(|x| x.is_finite() && *x > 0.0)
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
    fn bar(p: i64) -> BarPrices {
        BarPrices {
            open: p,
            high: p,
            low: p,
            close: p,
        }
    }

    /// `BacktestBridgeOptions.quantity_step` must actually reach the
    /// `Portfolio` built inside the loop: an account too small to afford one
    /// whole share can only take a position at all if sized at fractional
    /// granularity.
    #[test]
    fn quantity_step_option_enables_fractional_sizing() {
        let weights = vec![vec![(aapl(), 1.0)]];
        let prices = vec![vec![(aapl(), bar(50_00))]]; // $50/share

        // Default (whole shares): $10 can't afford a single $50 share.
        let whole_result = backtest_weights_with_options(
            &weights,
            &prices,
            10_00, // $10
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
            BacktestBridgeOptions::default(),
        );
        assert_eq!(whole_result.final_cash, 10_00);

        // quantity_step = 1 (continuous, 1e-6 share) sizes a fractional
        // position instead of leaving the account entirely in cash.
        let fractional_result = backtest_weights_with_options(
            &weights,
            &prices,
            10_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
            BacktestBridgeOptions {
                quantity_step: Some(1),
                ..Default::default()
            },
        );
        assert!(fractional_result.final_cash < 10_00);
    }

    // === Windowed (day/month) execution budgets ===

    fn sym(i: usize) -> Symbol {
        Symbol::new(&format!("S{i}"))
    }

    /// `max_trades_per_day` must bind ACROSS multiple rebalances that share
    /// the same day ordinal — the crypto case, where consecutive periods can
    /// land on the same calendar day. Asserts exactly which symbols traded
    /// on the second rebalance of the day, not just a count.
    #[test]
    fn max_trades_per_day_binds_across_multiple_rebalances_same_day() {
        let price = 100_00; // $100/share, flat
        let symbols: Vec<Symbol> = (0..5).map(sym).collect();
        let bars: Vec<(Symbol, BarPrices)> = symbols.iter().map(|s| (*s, bar(price))).collect();
        let prices = vec![bars.clone(), bars];

        // Period 0: open S0, S1 (2 orders). Period 1 (same day): also target
        // S2, S3, S4 (3 more orders needed), but the day's budget is 3 total
        // and 2 are already spent, leaving room for exactly 1.
        let weights = vec![
            vec![(symbols[0], 0.2), (symbols[1], 0.2)],
            vec![
                (symbols[0], 0.2),
                (symbols[1], 0.2),
                (symbols[2], 0.2),
                (symbols[3], 0.2),
                (symbols[4], 0.2),
            ],
        ];

        let options = BacktestBridgeOptions {
            max_trades_per_day: Some(3),
            period_day_ordinal: Some(vec![1, 1]), // both periods, same day
            ..Default::default()
        };

        let result = backtest_weights_with_options(
            &weights,
            &prices,
            1_000_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
            options,
        );

        // After period 0: S0 and S1 only.
        let held0: Vec<&str> = result.holdings[0].iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(held0, vec!["S0", "S1"]);

        // After period 1: only 1 of {S2, S3, S4} can trade (budget: 3 - 2 = 1
        // left). Ties broken by symbol ascending (Portfolio's own rule), so
        // S2 is the one that gets in; S3 and S4 stay unopened.
        let held1: Vec<&str> = result.holdings[1].iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(held1, vec!["S0", "S1", "S2"]);
    }

    /// `max_trades_per_month` must bind across periods sharing a month
    /// ordinal and RESET when the ordinal changes.
    #[test]
    fn max_trades_per_month_binds_across_periods_and_resets_at_month_boundary() {
        let price = 100_00;
        let symbols: Vec<Symbol> = (0..4).map(sym).collect();
        let bars: Vec<(Symbol, BarPrices)> = symbols.iter().map(|s| (*s, bar(price))).collect();
        let prices = vec![bars.clone(), bars.clone(), bars.clone(), bars];

        // Introduce one new symbol per period: A (0), then +B (1), then +C
        // (2, blocked by the exhausted monthly budget), then +C+D once the
        // month resets (3).
        let weights = vec![
            vec![(symbols[0], 0.25)],
            vec![(symbols[0], 0.25), (symbols[1], 0.25)],
            vec![(symbols[0], 0.25), (symbols[1], 0.25), (symbols[2], 0.25)],
            vec![
                (symbols[0], 0.25),
                (symbols[1], 0.25),
                (symbols[2], 0.25),
                (symbols[3], 0.25),
            ],
        ];

        let options = BacktestBridgeOptions {
            max_trades_per_month: Some(2),
            period_month_ordinal: Some(vec![1, 1, 1, 2]), // periods 0-2 in month 1, period 3 in month 2
            ..Default::default()
        };

        let result = backtest_weights_with_options(
            &weights,
            &prices,
            1_000_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
            options,
        );

        // Period 0: 1 trade (A). Period 1: 1 trade (B) -> month budget (2)
        // fully spent. Period 2: C is blocked, budget exhausted for the month.
        let held2: Vec<&str> = result.holdings[2].iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(held2, vec!["S0", "S1"]);

        // Period 3: new month ordinal (2) resets the budget to 2, enough for
        // both the still-pending C and the new D.
        let held3: Vec<&str> = result.holdings[3].iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(held3, vec!["S0", "S1", "S2", "S3"]);
    }

    /// `max_traded_value_per_month` exhausting mid-month must stop trading
    /// for the rest of that month, including corrective orders on positions
    /// left short by an earlier truncation.
    #[test]
    fn max_traded_value_per_month_exhausted_mid_month_stops_further_trading() {
        let price = 100_00; // $100/share
        let a = sym(0);
        let b = sym(1);
        let c = sym(2);
        let bars = |syms: &[Symbol]| -> Vec<(Symbol, BarPrices)> {
            syms.iter().map(|s| (*s, bar(price))).collect()
        };
        let prices = vec![
            bars(&[a]),
            bars(&[a, b]),
            bars(&[a, b, c]),
        ];

        // $100,000 account, 0.6% target weight -> $600 per fully-funded
        // position (6 shares at $100).
        let weights = vec![
            vec![(a, 0.006)],
            vec![(a, 0.006), (b, 0.006)],
            vec![(a, 0.006), (b, 0.006), (c, 0.006)],
        ];

        let options = BacktestBridgeOptions {
            max_traded_value_per_month: Some(1_000_00), // $1,000/month
            period_month_ordinal: Some(vec![1, 1, 1]),  // all 3 periods, same month
            ..Default::default()
        };

        let result = backtest_weights_with_options(
            &weights,
            &prices,
            100_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
            options,
        );

        // Period 0: A opened at the full $600 -> $400 of the month's $1,000
        // budget remains.
        // Period 1: B would need $600 but only $400 remains, so it is
        // tightened via max_order_value to exactly $400 (4 shares) -> the
        // month's budget is now fully spent ($600 + $400 = $1,000).
        let b_weight_after_1 = result.holdings[1]
            .iter()
            .find(|(s, _)| *s == b)
            .map(|(_, w)| *w)
            .expect("B should hold a truncated position");
        // 4 shares * $100 = $400 out of $100,000 equity = 0.004, not the
        // requested 0.006 -- proof the order was truncated, not skipped.
        assert!(
            (b_weight_after_1 - 0.004).abs() < 1e-9,
            "expected B truncated to 0.004 weight ($400), got {b_weight_after_1}"
        );

        // Period 2: the month's value budget is fully exhausted, so the
        // effective trade cap is 0 for this period -- C is never opened, AND
        // B is NOT corrected up to its 0.006 target even though it is still
        // under-target from the period-1 truncation.
        let held2: Vec<&str> = result.holdings[2].iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(held2, vec!["S0", "S1"], "C must not open once the month's value budget is exhausted");
        let b_weight_after_2 = result.holdings[2]
            .iter()
            .find(|(s, _)| *s == b)
            .map(|(_, w)| *w)
            .expect("B still holds its truncated position");
        assert!(
            (b_weight_after_2 - b_weight_after_1).abs() < 1e-9,
            "B must not be corrected once the month's value budget is exhausted: {b_weight_after_1} -> {b_weight_after_2}"
        );
    }

    /// Mismatched ordinal array length must be treated as an invalid input
    /// (empty result), the same as a mismatched weight/price schedule length.
    #[test]
    fn mismatched_ordinal_length_returns_empty_result() {
        let weights = vec![vec![(aapl(), 1.0)]; 2];
        let prices = vec![vec![(aapl(), bar(100_00))]; 2];

        let options = BacktestBridgeOptions {
            max_trades_per_day: Some(1),
            period_day_ordinal: Some(vec![1]), // length 1, schedule length 2
            ..Default::default()
        };

        let result = backtest_weights_with_options(
            &weights,
            &prices,
            1_000_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
            options,
        );

        assert!(result.returns.is_empty());
        assert!(result.holdings.is_empty());
        assert_eq!(result.final_cash, 1_000_000_00);
    }

    /// All five new options default to `None`/unset, so a backtest that
    /// doesn't touch them must reproduce byte-identical numbers to the
    /// pre-existing baseline: no per-period overriding happens at all.
    #[test]
    fn windowed_budget_options_default_to_unset_and_preserve_baseline_behavior() {
        let weights = vec![vec![(aapl(), 1.0)]];
        let prices = vec![vec![(aapl(), bar(150_00))]];

        let result = backtest_weights_with_options(
            &weights,
            &prices,
            1_000_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
            BacktestBridgeOptions {
                max_order_value: None,
                max_trades_per_day: None,
                max_trades_per_month: None,
                max_traded_value_per_month: None,
                period_day_ordinal: None,
                period_month_ordinal: None,
                ..Default::default()
            },
        );

        // 1,000,000_00 / 150_00 = 6666.67 -> 6666 whole shares at the
        // default quantity_step; $100 remains in cash. Pinned exact numbers.
        assert_eq!(result.final_cash, 100_00);
        assert_eq!(result.equity_curve, vec![1_000_000_00, 1_000_000_00]);

        let default_result = backtest_weights(
            &weights,
            &prices,
            1_000_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
        );
        assert_eq!(default_result.final_cash, result.final_cash);
        assert_eq!(default_result.equity_curve, result.equity_curve);
        assert_eq!(default_result.holdings, result.holdings);
    }

    #[test]
    fn signal_bar_close_parity_with_degenerate_ohlc() {
        let weights = vec![vec![(aapl(), 1.0)]; 3];
        let close_prices = [100_00i64, 110_00, 99_00];
        let prices: Vec<Vec<(Symbol, BarPrices)>> = close_prices
            .iter()
            .map(|&p| vec![(aapl(), bar(p))])
            .collect();
        let result = backtest_weights(
            &weights,
            &prices,
            1_000_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
        );
        assert_eq!(result.equity_curve.len(), 4);
        assert!(result.skipped_rebalances.is_empty());
        assert!(result.equity_curve[2] > result.equity_curve[0]);
    }

    #[test]
    fn next_bar_open_fills_at_open_t_plus_1() {
        let n = 4usize;
        let closes = [100_00i64, 101_00, 102_00, 103_00];
        let opens = [99_00i64, 100_00 + 100, 101_00 + 100, 102_00 + 100];
        let prices: Vec<Vec<(Symbol, BarPrices)>> = (0..n)
            .map(|i| {
                vec![(
                    aapl(),
                    BarPrices {
                        open: opens[i],
                        high: opens[i].max(closes[i]),
                        low: opens[i].min(closes[i]),
                        close: closes[i],
                    },
                )]
            })
            .collect();
        let weights = vec![vec![(aapl(), 1.0)]; n];
        let result = backtest_weights(
            &weights,
            &prices,
            1_000_000_00,
            CostModel::zero(),
            FillPolicy::NextBarOpen,
            252.0,
            0.0,
        );
        assert!(result.skipped_rebalances.contains(&(n - 1)));
        assert_eq!(result.equity_curve.len(), n + 1);
    }

    #[test]
    fn next_bar_typical_fill_price_is_hlc3() {
        let base = 100_00i64;
        let h1 = base * 102 / 100;
        let l1 = base * 99 / 100;
        let c1 = base;
        let prices = vec![
            vec![(aapl(), bar(base))],
            vec![(
                aapl(),
                BarPrices {
                    open: c1,
                    high: h1,
                    low: l1,
                    close: c1,
                },
            )],
        ];
        let weights = vec![vec![(aapl(), 1.0)]; 2];
        let result = backtest_weights(
            &weights,
            &prices,
            1_000_000_00,
            CostModel::zero(),
            FillPolicy::NextBarTypical,
            252.0,
            0.0,
        );
        assert!(result.skipped_rebalances.contains(&1));
        assert_eq!(result.equity_curve.len(), 3);
    }

    #[test]
    fn last_bar_skip_with_next_bar_open() {
        let n = 3usize;
        let p = 100_00i64;
        let prices: Vec<Vec<(Symbol, BarPrices)>> =
            (0..n).map(|_| vec![(aapl(), bar(p))]).collect();
        let weights: Vec<Vec<(Symbol, f64)>> = (0..n).map(|_| vec![(aapl(), 1.0)]).collect();
        let result = backtest_weights(
            &weights,
            &prices,
            1_000_000_00,
            CostModel::zero(),
            FillPolicy::NextBarOpen,
            252.0,
            0.0,
        );
        assert!(result.skipped_rebalances.contains(&(n - 1)));
        assert_eq!(result.equity_curve.len(), n + 1);
        assert_eq!(result.returns.len(), n);
    }

    #[test]
    fn tear_sheet_contains_reporting_payload() {
        let weights = vec![vec![(aapl(), 1.0)]; 24];
        let prices: Vec<Vec<(Symbol, BarPrices)>> = (0..24)
            .map(|i| vec![(aapl(), bar(100_00 + i as i64 * 100))])
            .collect();
        let result = backtest_weights(
            &weights,
            &prices,
            1_000_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
        );

        let sheet = tear_sheet(&result, 5, 252);

        assert_eq!(sheet.monthly_returns.len(), 1);
        assert_eq!(sheet.monthly_returns[0].len(), 2);
        assert_eq!(sheet.rolling_sharpe.len(), result.returns.len());
        assert!(sheet.trade_analytics.trade_count >= 1);
    }

    #[test]
    fn monthly_return_matrix_compounds_chunks() {
        let matrix = monthly_return_matrix(&[0.1, 0.1, -0.1], 2);
        assert_eq!(matrix.len(), 1);
        assert!((matrix[0][0] - 0.21).abs() < 1e-12);
        assert!((matrix[0][1] + 0.1).abs() < 1e-12);
    }

    #[test]
    fn decompose_backtest_computes_contribution_and_cumulative_sum() {
        let weights = vec![
            vec![(aapl(), 0.6), (msft(), 0.4)],
            vec![(aapl(), 0.5), (msft(), 0.5)],
        ];
        let returns = vec![
            vec![(aapl(), 0.10), (msft(), -0.05)],
            vec![(aapl(), 0.02), (msft(), 0.04)],
        ];

        let result = decompose_backtest(&weights, &returns);

        assert_eq!(
            result.contributions[0],
            vec![(aapl(), 0.06), (msft(), -0.020000000000000004)]
        );
        assert_eq!(
            result.contributions[1],
            vec![(aapl(), 0.01), (msft(), 0.02)]
        );
        assert!((result.cumulative_contributions[1][0].1 - 0.07).abs() < 1e-12);
        assert!((result.cumulative_contributions[1][1].1 - 0.0).abs() < 1e-12);
    }

    #[test]
    fn decompose_backtest_detects_entries_exits_and_open_trades() {
        let weights = vec![
            vec![(aapl(), 1.0)],
            vec![(aapl(), 0.5), (msft(), 0.5)],
            vec![(msft(), 1.0)],
        ];
        let returns = vec![
            vec![(aapl(), 0.01)],
            vec![(aapl(), 0.01), (msft(), 0.02)],
            vec![(msft(), 0.03)],
        ];

        let result = decompose_backtest(&weights, &returns);

        assert!(result.trades.iter().any(|trade| {
            trade.symbol == aapl()
                && trade.entry_index == 0
                && trade.exit_index == Some(2)
                && trade.entry_weight == 1.0
                && trade.exit_weight == 0.5
        }));
        assert!(result.trades.iter().any(|trade| {
            trade.symbol == msft() && trade.entry_index == 1 && trade.exit_index.is_none()
        }));
    }

    #[test]
    fn decompose_backtest_rejects_mismatched_lengths_with_empty_result() {
        let result = decompose_backtest(&[vec![(aapl(), 1.0)]], &[]);
        assert!(result.contributions.is_empty());
        assert!(result.cumulative_contributions.is_empty());
        assert!(result.trades.is_empty());
    }

    #[test]
    fn decompose_backtest_integrates_with_backtest_weights_outputs() {
        let weights = vec![
            vec![(aapl(), 1.0)],
            vec![(aapl(), 1.0)],
            vec![(aapl(), 1.0)],
        ];
        let prices = vec![
            vec![(aapl(), bar(100_00))],
            vec![(aapl(), bar(110_00))],
            vec![(aapl(), bar(99_00))],
        ];
        let backtest = backtest_weights(
            &weights,
            &prices,
            1_000_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
        );

        let attribution = decompose_backtest(&backtest.holdings, &backtest.symbol_returns);

        for (period, contributions) in attribution.contributions.iter().enumerate() {
            let summed: f64 = contributions.iter().map(|(_, value)| value).sum();
            assert!((summed - backtest.returns[period]).abs() < 1e-12);
        }
    }

    #[test]
    fn decompose_backtest_treats_non_finite_returns_as_zero() {
        let weights = vec![vec![(aapl(), 1.0)]];
        let returns = vec![vec![(aapl(), f64::NAN)]];

        let result = decompose_backtest(&weights, &returns);

        assert_eq!(result.contributions, vec![vec![(aapl(), 0.0)]]);
        assert_eq!(result.cumulative_contributions, vec![vec![(aapl(), 0.0)]]);
    }

    #[test]
    fn basic_two_period_backtest() {
        let weights = vec![
            vec![(aapl(), 0.5), (msft(), 0.5)],
            vec![(aapl(), 0.3), (msft(), 0.7)],
        ];
        let prices = vec![
            vec![(aapl(), bar(150_00)), (msft(), bar(300_00))],
            vec![(aapl(), bar(155_00)), (msft(), bar(310_00))],
        ];

        let result = backtest_weights(
            &weights,
            &prices,
            1_000_000_00,
            CostModel {
                commission_bps: 10.0,
                slippage_bps: 0.0,
                min_commission: 0,
            },
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
        );

        assert_eq!(result.returns.len(), 2);
        assert_eq!(result.equity_curve.len(), 3); // initial + 2 periods
        assert!(result.metrics.is_some());
        assert_eq!(result.holdings.len(), 2);
        assert_eq!(result.symbol_returns.len(), 2);
    }

    #[test]
    fn zero_cost_preserves_equity() {
        let weights = vec![vec![(aapl(), 0.5)]];
        let prices = vec![vec![(aapl(), bar(100_00))]];

        let result = backtest_weights(
            &weights,
            &prices,
            1_000_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
        );

        // With zero cost and no price movement, equity should be ~initial
        let final_eq = *result
            .equity_curve
            .last()
            .expect("equity curve has one point");
        assert!((final_eq - 1_000_000_00).abs() < 200_00); // rounding tolerance
    }

    #[test]
    fn empty_schedule() {
        let result = backtest_weights(
            &[],
            &[],
            1_000_000_00,
            CostModel {
                commission_bps: 10.0,
                slippage_bps: 0.0,
                min_commission: 0,
            },
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
        );
        assert!(result.returns.is_empty());
        assert!(result.metrics.is_none());
        assert_eq!(result.equity_curve.len(), 1);
        assert!(result.holdings.is_empty());
        assert!(result.symbol_returns.is_empty());
    }

    #[test]
    fn fixed_stop_triggers_exit() {
        let weights = vec![vec![(aapl(), 1.0)], vec![(aapl(), 1.0)]];
        let prices = vec![vec![(aapl(), bar(100_00))], vec![(aapl(), bar(85_00))]];

        let options = BacktestBridgeOptions {
            stop_cfg: Some(BacktestStopConfig {
                fixed_stop_pct: Some(0.10),
                trailing_stop_pct: None,
                atr_multiple: None,
                atr_period: 14,
            }),
            ..Default::default()
        };

        let result = backtest_weights_with_options(
            &weights,
            &prices,
            100_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
            options,
        );

        assert_eq!(result.stop_events.len(), 1);
        assert_eq!(result.stop_events[0].reason, "fixed");
        assert_eq!(result.stop_events[0].period_index, 1);
        assert_eq!(result.stop_events[0].trigger_price, 90_00);
        assert_eq!(result.stop_events[0].exit_price, 85_00);
        assert!(result.holdings[1].is_empty());
    }

    #[test]
    fn trailing_stop_emits_event() {
        let weights = vec![
            vec![(aapl(), 1.0)],
            vec![(aapl(), 1.0)],
            vec![(aapl(), 1.0)],
        ];
        let prices = vec![
            vec![(aapl(), bar(100_00))],
            vec![(aapl(), bar(110_00))],
            vec![(aapl(), bar(95_00))],
        ];

        let options = BacktestBridgeOptions {
            stop_cfg: Some(BacktestStopConfig {
                fixed_stop_pct: None,
                trailing_stop_pct: Some(0.10),
                atr_multiple: None,
                atr_period: 14,
            }),
            ..Default::default()
        };

        let result = backtest_weights_with_options(
            &weights,
            &prices,
            100_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
            options,
        );

        assert!(!result.stop_events.is_empty());
        assert_eq!(result.stop_events[0].reason, "trailing");
    }

    #[test]
    fn first_breach_triggers_once_per_position_lifecycle() {
        let weights = vec![
            vec![(aapl(), 1.0)],
            vec![(aapl(), 1.0)],
            vec![(aapl(), 1.0)],
        ];
        let prices = vec![
            vec![(aapl(), bar(100_00))],
            vec![(aapl(), bar(90_00))], // fixed 10% stop breaches here
            vec![(aapl(), bar(89_00))], // reopened, new stop basis, no second trigger
        ];

        let options = BacktestBridgeOptions {
            stop_cfg: Some(BacktestStopConfig {
                fixed_stop_pct: Some(0.10),
                trailing_stop_pct: None,
                atr_multiple: None,
                atr_period: 14,
            }),
            ..Default::default()
        };

        let result = backtest_weights_with_options(
            &weights,
            &prices,
            100_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
            options,
        );

        assert_eq!(result.stop_events.len(), 1);
        assert_eq!(result.stop_events[0].period_index, 1);
        assert_eq!(result.stop_events[0].reason, "fixed");
    }

    #[test]
    fn tighter_stop_reason_is_reported_when_multiple_rules_enabled() {
        let weights = vec![
            vec![(aapl(), 1.0)],
            vec![(aapl(), 1.0)],
            vec![(aapl(), 1.0)],
        ];
        let prices = vec![
            vec![(aapl(), bar(100_00))],
            vec![(aapl(), bar(110_00))], // updates trailing reference
            vec![(aapl(), bar(103_00))], // breaches trailing(104.5) but not fixed(90)
        ];

        let options = BacktestBridgeOptions {
            stop_cfg: Some(BacktestStopConfig {
                fixed_stop_pct: Some(0.10),
                trailing_stop_pct: Some(0.05),
                atr_multiple: None,
                atr_period: 14,
            }),
            ..Default::default()
        };

        let result = backtest_weights_with_options(
            &weights,
            &prices,
            100_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
            options,
        );

        assert_eq!(result.stop_events.len(), 1);
        assert_eq!(result.stop_events[0].reason, "trailing");
        assert_eq!(result.stop_events[0].trigger_price, 104_50);
    }

    #[test]
    fn atr_stop_triggers_on_high_volatility() {
        let weights = vec![
            vec![(aapl(), 1.0)],
            vec![(aapl(), 1.0)],
            vec![(aapl(), 1.0)],
            vec![(aapl(), 1.0)],
        ];
        // High volatility: 100 -> 110 -> 95 -> 85 (large moves)
        let prices = vec![
            vec![(aapl(), bar(100_00))],
            vec![(aapl(), bar(110_00))],
            vec![(aapl(), bar(95_00))],
            vec![(aapl(), bar(85_00))],
        ];

        let options = BacktestBridgeOptions {
            stop_cfg: Some(BacktestStopConfig {
                fixed_stop_pct: None,
                trailing_stop_pct: None,
                atr_multiple: Some(2.0), // 2x ATR stop
                atr_period: 3,
            }),
            ..Default::default()
        };

        let result = backtest_weights_with_options(
            &weights,
            &prices,
            100_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
            options,
        );

        // Should trigger on high volatility
        assert!(!result.stop_events.is_empty());
        assert_eq!(result.stop_events[0].reason, "atr");
    }

    #[test]
    fn short_position_fixed_stop_triggers_on_rise() {
        let weights = vec![vec![(aapl(), -1.0)], vec![(aapl(), -1.0)]];
        // Short position: stop triggers when price rises
        let prices = vec![vec![(aapl(), bar(100_00))], vec![(aapl(), bar(115_00))]];

        let options = BacktestBridgeOptions {
            stop_cfg: Some(BacktestStopConfig {
                fixed_stop_pct: Some(0.10), // 10% stop
                trailing_stop_pct: None,
                atr_multiple: None,
                atr_period: 14,
            }),
            ..Default::default()
        };

        let result = backtest_weights_with_options(
            &weights,
            &prices,
            100_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
            options,
        );

        assert_eq!(result.stop_events.len(), 1);
        assert_eq!(result.stop_events[0].reason, "fixed");
        assert_eq!(result.stop_events[0].trigger_price, 110_00); // 100 * 1.10
        assert_eq!(result.stop_events[0].exit_price, 115_00);
    }

    #[test]
    fn short_position_trailing_stop_adjusts_downward() {
        let weights = vec![
            vec![(aapl(), -1.0)],
            vec![(aapl(), -1.0)],
            vec![(aapl(), -1.0)],
        ];
        // Short: trailing stop moves down as price falls, then triggers on a rebound.
        let prices = vec![
            vec![(aapl(), bar(100_00))],
            vec![(aapl(), bar(90_00))], // profit, trailing stop adjusts down to 94.50
            vec![(aapl(), bar(98_00))], // rebounds through adjusted stop
        ];

        let options = BacktestBridgeOptions {
            stop_cfg: Some(BacktestStopConfig {
                fixed_stop_pct: None,
                trailing_stop_pct: Some(0.05),
                atr_multiple: None,
                atr_period: 14,
            }),
            ..Default::default()
        };

        let result = backtest_weights_with_options(
            &weights,
            &prices,
            100_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
            options,
        );

        assert_eq!(result.stop_events.len(), 1);
        assert_eq!(result.stop_events[0].reason, "trailing");
        assert_eq!(result.stop_events[0].trigger_price, 94_50);
        assert_eq!(result.stop_events[0].exit_price, 98_00);
    }

    #[test]
    fn multiple_symbols_independent_stops() {
        let weights = vec![
            vec![(aapl(), 0.5), (msft(), 0.5)],
            vec![(aapl(), 0.5), (msft(), 0.5)],
        ];
        // AAPL drops 15% (triggers 10% stop), MSFT drops 5% (no trigger)
        let prices = vec![
            vec![(aapl(), bar(100_00)), (msft(), bar(100_00))],
            vec![(aapl(), bar(85_00)), (msft(), bar(95_00))],
        ];

        let options = BacktestBridgeOptions {
            stop_cfg: Some(BacktestStopConfig {
                fixed_stop_pct: Some(0.10),
                trailing_stop_pct: None,
                atr_multiple: None,
                atr_period: 14,
            }),
            ..Default::default()
        };

        let result = backtest_weights_with_options(
            &weights,
            &prices,
            100_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
            options,
        );

        assert_eq!(result.stop_events.len(), 1);
        assert_eq!(result.stop_events[0].symbol, aapl());
        assert!(result.holdings[1].iter().all(|(sym, _)| *sym != aapl()));
    }

    #[test]
    fn position_flip_resets_stop_tracker() {
        let weights = vec![
            vec![(aapl(), 1.0)],  // long
            vec![(aapl(), -1.0)], // flip to short
            vec![(aapl(), -1.0)],
        ];
        let prices = vec![
            vec![(aapl(), bar(100_00))],
            vec![(aapl(), bar(95_00))],
            vec![(aapl(), bar(110_00))], // short stop would trigger at 105
        ];

        let options = BacktestBridgeOptions {
            stop_cfg: Some(BacktestStopConfig {
                fixed_stop_pct: Some(0.10),
                trailing_stop_pct: None,
                atr_multiple: None,
                atr_period: 14,
            }),
            ..Default::default()
        };

        let result = backtest_weights_with_options(
            &weights,
            &prices,
            100_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
            options,
        );

        // Should trigger on short position after flip
        assert_eq!(result.stop_events.len(), 1);
        assert_eq!(result.stop_events[0].period_index, 2);
    }

    #[test]
    fn stop_loss_with_low_volatility_no_atr_trigger() {
        let weights = vec![
            vec![(aapl(), 1.0)],
            vec![(aapl(), 1.0)],
            vec![(aapl(), 1.0)],
            vec![(aapl(), 1.0)],
        ];
        // Low volatility: small price moves
        let prices = vec![
            vec![(aapl(), bar(100_00))],
            vec![(aapl(), bar(101_00))],
            vec![(aapl(), bar(102_00))],
            vec![(aapl(), bar(101_50))],
        ];

        let options = BacktestBridgeOptions {
            stop_cfg: Some(BacktestStopConfig {
                fixed_stop_pct: None,
                trailing_stop_pct: None,
                atr_multiple: Some(3.0), // high multiple but low volatility
                atr_period: 3,
            }),
            ..Default::default()
        };

        let result = backtest_weights_with_options(
            &weights,
            &prices,
            100_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
            options,
        );

        // Should not trigger - low volatility keeps ATR small
        assert!(result.stop_events.is_empty());
    }

    #[test]
    fn stop_loss_with_rebalance_keeps_tracking() {
        let weights = vec![
            vec![(aapl(), 0.8), (msft(), 0.2)],
            vec![(aapl(), 0.6), (msft(), 0.4)], // rebalance
            vec![(aapl(), 0.6), (msft(), 0.4)],
        ];
        let prices = vec![
            vec![(aapl(), bar(100_00)), (msft(), bar(100_00))],
            vec![(aapl(), bar(95_00)), (msft(), bar(95_00))], // both drop
            vec![(aapl(), bar(85_00)), (msft(), bar(95_00))], // AAPL triggers
        ];

        let options = BacktestBridgeOptions {
            stop_cfg: Some(BacktestStopConfig {
                fixed_stop_pct: Some(0.10),
                trailing_stop_pct: None,
                atr_multiple: None,
                atr_period: 14,
            }),
            ..Default::default()
        };

        let result = backtest_weights_with_options(
            &weights,
            &prices,
            100_000_00,
            CostModel::zero(),
            FillPolicy::SignalBarClose,
            252.0,
            0.0,
            options,
        );

        // AAPL should trigger, MSFT should continue
        assert_eq!(result.stop_events.len(), 1);
        assert_eq!(result.stop_events[0].symbol, aapl());
        assert!(result.holdings[2].iter().any(|(sym, _)| *sym == msft()));
    }
}
