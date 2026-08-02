use nanobook::portfolio::{CostModel, Portfolio};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::metrics::PyMetrics;
use crate::multi::PyMultiExchange;
use crate::position::PyPosition;
use crate::types::parse_symbol;

/// Transaction cost model.
///
/// Args:
///     commission_bps: Commission in basis points (1 bps = 0.01%)
///     slippage_bps: Slippage estimate in basis points
///     min_commission: Minimum commission per fill in cents
///
/// Example::
///
///     model = CostModel(commission_bps=10.0, slippage_bps=5.0, min_commission=100)
///     zero = CostModel.zero()
///
#[pyclass(name = "CostModel", from_py_object)]
#[derive(Clone)]
pub struct PyCostModel {
    pub inner: CostModel,
}

#[pymethods]
impl PyCostModel {
    #[new]
    #[pyo3(signature = (commission_bps=0.0, slippage_bps=0.0, min_commission=0))]
    fn new(commission_bps: f64, slippage_bps: f64, min_commission: i64) -> Self {
        Self {
            inner: CostModel {
                commission_bps,
                slippage_bps,
                min_commission,
            },
        }
    }

    /// Create a zero-cost model.
    #[staticmethod]
    fn zero() -> Self {
        Self {
            inner: CostModel::zero(),
        }
    }

    /// Compute cost for a trade with the given notional value (cents).
    fn compute_cost(&self, notional: i64) -> i64 {
        self.inner.compute_cost(notional)
    }

    fn __repr__(&self) -> String {
        format!(
            "CostModel(commission_bps={}, slippage_bps={}, min_commission={})",
            self.inner.commission_bps, self.inner.slippage_bps, self.inner.min_commission
        )
    }
}

/// Portfolio: tracks cash, positions, and returns.
///
/// Args:
///     initial_cash: Starting cash in cents (e.g., 1_000_000_00 = $1M)
///     cost_model: A CostModel instance
///     quantity_step: Optional order sizing granularity, in micro-shares
///         (1 share = 1_000_000 units). Positions are always sized to a
///         multiple of this step. Defaults to whole shares. Four useful
///         values:
///
///         - ``1_000_000`` — whole shares (the default)
///         - ``1_000`` — 0.001 share (Alpaca's fractional minimum)
///         - ``100`` — 0.0001 share (IBKR's fractional minimum)
///         - ``1`` — 0.000001 share (effectively continuous)
///
///     min_order_value: Optional minimum order notional in cents. An order
///         whose notional is below this is skipped instead of placed.
///         Defaults to ``0`` (no minimum).
///     no_trade_band_bps: Optional no-trade band, in basis points of equity.
///         A position is left alone until it drifts from its target weight
///         by more than this many bps. Defaults to ``0.0`` (no band).
///     max_trades_per_rebalance: Optional hard cap on the number of orders
///         placed in a single rebalance. When the cap binds, the orders
///         furthest from target (largest absolute drift) are kept and the
///         rest are dropped, ties broken by symbol. Defaults to ``None``
///         (no cap).
///
/// Example::
///
///     portfolio = Portfolio(1_000_000_00, CostModel.zero())
///     portfolio.rebalance_simple([("AAPL", 0.6)], [("AAPL", 15000)])
///
///     # Fractional-share account sized to Alpaca's 0.001-share minimum:
///     fractional = Portfolio(1_000_00, CostModel.zero(), quantity_step=1_000)
///     fractional.rebalance_simple([("AAPL", 0.6)], [("AAPL", 15000)])
///
#[pyclass(name = "Portfolio", from_py_object)]
#[derive(Clone)]
pub struct PyPortfolio {
    pub inner: Portfolio,
}

impl PyPortfolio {
    pub fn from_portfolio(inner: Portfolio) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPortfolio {
    #[new]
    #[pyo3(signature = (initial_cash, cost_model, quantity_step=None, min_order_value=None, no_trade_band_bps=None, max_trades_per_rebalance=None))]
    fn new(
        initial_cash: i64,
        cost_model: &PyCostModel,
        quantity_step: Option<i64>,
        min_order_value: Option<i64>,
        no_trade_band_bps: Option<f64>,
        max_trades_per_rebalance: Option<usize>,
    ) -> PyResult<Self> {
        let mut inner = Portfolio::new(initial_cash, cost_model.inner);
        if let Some(step) = quantity_step {
            validate_quantity_step(step)?;
            inner.set_quantity_step(step);
        }
        if let Some(value) = min_order_value {
            validate_min_order_value(value)?;
            inner.set_min_order_value(value);
        }
        if let Some(bps) = no_trade_band_bps {
            validate_no_trade_band_bps(bps)?;
            inner.set_no_trade_band_bps(bps);
        }
        if let Some(cap) = max_trades_per_rebalance {
            inner.set_max_trades_per_rebalance(Some(cap));
        }
        Ok(Self { inner })
    }

    /// Order sizing granularity, in micro-shares (1 share = 1_000_000 units).
    /// See the class docstring for useful values.
    #[getter]
    fn quantity_step(&self) -> i64 {
        self.inner.quantity_step()
    }

    /// Set the order sizing granularity, in micro-shares. Must be positive.
    #[setter]
    fn set_quantity_step(&mut self, step: i64) -> PyResult<()> {
        validate_quantity_step(step)?;
        self.inner.set_quantity_step(step);
        Ok(())
    }

    /// Set the order sizing granularity from a fractional share size (e.g.
    /// ``0.001``) instead of raw micro-share units. The fraction is rounded
    /// to the nearest micro-share (`round(fraction * 1_000_000)`) and must
    /// round to a positive number of micro-shares.
    fn set_quantity_step_fraction(&mut self, fraction: f64) -> PyResult<()> {
        if !fraction.is_finite() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "quantity_step fraction must be finite",
            ));
        }
        let step = (fraction * nanobook::portfolio::Shares::SCALE as f64).round() as i64;
        validate_quantity_step(step)?;
        self.inner.set_quantity_step(step);
        Ok(())
    }

    /// Minimum order notional in cents. An order below this is skipped
    /// instead of placed.
    #[getter]
    fn min_order_value(&self) -> i64 {
        self.inner.min_order_value()
    }

    /// Set the minimum order notional, in cents. Must be non-negative.
    #[setter]
    fn set_min_order_value(&mut self, value: i64) -> PyResult<()> {
        validate_min_order_value(value)?;
        self.inner.set_min_order_value(value);
        Ok(())
    }

    /// No-trade band, in basis points of equity. A position is left alone
    /// until it drifts from its target weight by more than this many bps.
    #[getter]
    fn no_trade_band_bps(&self) -> f64 {
        self.inner.no_trade_band_bps()
    }

    /// Set the no-trade band, in basis points of equity. Must be finite and
    /// non-negative.
    #[setter]
    fn set_no_trade_band_bps(&mut self, bps: f64) -> PyResult<()> {
        validate_no_trade_band_bps(bps)?;
        self.inner.set_no_trade_band_bps(bps);
        Ok(())
    }

    /// Hard cap on the number of orders placed in a single rebalance, or
    /// ``None`` for no cap. When the cap binds, the orders furthest from
    /// target (largest absolute drift) are kept.
    #[getter]
    fn max_trades_per_rebalance(&self) -> Option<usize> {
        self.inner.max_trades_per_rebalance()
    }

    /// Set the hard cap on orders placed per rebalance. Pass ``None`` to
    /// remove the cap.
    #[setter]
    fn set_max_trades_per_rebalance(&mut self, cap: Option<usize>) {
        self.inner.set_max_trades_per_rebalance(cap);
    }

    /// Current cash balance in cents.
    #[getter]
    fn cash(&self) -> i64 {
        self.inner.cash()
    }

    /// Get a position by symbol.
    fn position(&self, symbol: &str) -> PyResult<Option<PyPosition>> {
        let sym = parse_symbol(symbol)?;
        Ok(self
            .inner
            .position(&sym)
            .map(|p| PyPosition { inner: p.clone() }))
    }

    /// Get all positions as a dict {symbol: Position}.
    fn positions(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let dict = PyDict::new(py);
        for (sym, pos) in self.inner.positions() {
            dict.set_item(sym.to_string(), PyPosition { inner: pos.clone() })?;
        }
        Ok(dict.into_any().unbind())
    }

    /// Total equity (cash + position values) given current prices.
    ///
    /// Args:
    ///     prices: List of (symbol, price_in_cents) tuples
    fn total_equity(&self, prices: Vec<(String, i64)>) -> PyResult<i64> {
        let prices = parse_price_list(&prices)?;
        Ok(self.inner.total_equity(&prices))
    }

    /// Current portfolio weights.
    ///
    /// Returns list of (symbol, weight) tuples.
    fn current_weights(&self, prices: Vec<(String, i64)>) -> PyResult<Vec<(String, f64)>> {
        let prices = parse_price_list(&prices)?;
        Ok(self
            .inner
            .current_weights(&prices)
            .into_iter()
            .map(|(sym, w)| (sym.as_str().to_string(), w))
            .collect())
    }

    /// Get the return series.
    fn returns(&self) -> Vec<f64> {
        self.inner.returns().to_vec()
    }

    /// Get the equity curve.
    fn equity_curve(&self) -> Vec<i64> {
        self.inner.equity_curve().to_vec()
    }

    /// Rebalance to target weights using simple fill (instant execution).
    ///
    /// Args:
    ///     targets: List of (symbol, weight) tuples. Weights should sum to <= 1.0.
    ///     prices: List of (symbol, price_in_cents) tuples.
    fn rebalance_simple(
        &mut self,
        targets: Vec<(String, f64)>,
        prices: Vec<(String, i64)>,
    ) -> PyResult<()> {
        let targets = parse_target_list(&targets)?;
        let prices = parse_price_list(&prices)?;
        self.inner.rebalance_simple(&targets, &prices);
        Ok(())
    }

    /// Rebalance through LOB matching engines.
    fn rebalance_lob(
        &mut self,
        targets: Vec<(String, f64)>,
        exchanges: &mut PyMultiExchange,
    ) -> PyResult<()> {
        let targets = parse_target_list(&targets)?;
        self.inner.rebalance_lob(&targets, &mut exchanges.inner);
        Ok(())
    }

    /// Record a return for the current period.
    fn record_return(&mut self, prices: Vec<(String, i64)>) -> PyResult<()> {
        let prices = parse_price_list(&prices)?;
        self.inner.record_return(&prices);
        Ok(())
    }

    /// Take a portfolio snapshot.
    fn snapshot(&self, py: Python<'_>, prices: Vec<(String, i64)>) -> PyResult<Py<PyAny>> {
        let prices = parse_price_list(&prices)?;
        let snap = self.inner.snapshot(&prices);

        let dict = PyDict::new(py);
        dict.set_item("cash", snap.cash)?;
        dict.set_item("equity", snap.equity)?;
        dict.set_item("num_positions", snap.num_positions)?;
        dict.set_item("total_realized_pnl", snap.total_realized_pnl)?;

        let weights = PyDict::new(py);
        for (sym, w) in snap.weights {
            weights.set_item(sym.to_string(), w)?;
        }
        dict.set_item("weights", weights)?;

        Ok(dict.into_any().unbind())
    }

    /// Compute metrics from the recorded return series.
    ///
    /// Args:
    ///     periods_per_year: Annualization factor (252 for daily, 12 for monthly)
    ///     risk_free: Risk-free rate per period
    fn compute_metrics(&self, periods_per_year: f64, risk_free: f64) -> Option<PyMetrics> {
        nanobook::portfolio::compute_metrics(self.inner.returns(), periods_per_year, risk_free)
            .map(PyMetrics::from)
    }

    /// Save portfolio state to a JSON file.
    fn save_json(&self, path: &str) -> PyResult<()> {
        self.inner
            .save_json(std::path::Path::new(path))
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))
    }

    /// Load portfolio state from a JSON file.
    #[staticmethod]
    fn load_json(path: &str) -> PyResult<Self> {
        let inner = Portfolio::load_json(std::path::Path::new(path))
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "Portfolio(cash=${:.2}, returns={})",
            self.inner.cash() as f64 / 100.0,
            self.inner.returns().len()
        )
    }
}

/// Validate a `quantity_step` value (micro-shares): must be positive.
fn validate_quantity_step(step: i64) -> PyResult<()> {
    if step <= 0 {
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "quantity_step must be positive, got {step}"
        )));
    }
    Ok(())
}

/// Validate a `min_order_value` (cents): must be non-negative.
fn validate_min_order_value(value: i64) -> PyResult<()> {
    if value < 0 {
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "min_order_value must be non-negative, got {value}"
        )));
    }
    Ok(())
}

/// Validate a `no_trade_band_bps` value: must be finite and non-negative.
fn validate_no_trade_band_bps(bps: f64) -> PyResult<()> {
    if !bps.is_finite() || bps < 0.0 {
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "no_trade_band_bps must be finite and non-negative, got {bps}"
        )));
    }
    Ok(())
}

/// Parse Python list of (str, i64) into Vec<(Symbol, i64)>.
fn parse_price_list(prices: &[(String, i64)]) -> PyResult<Vec<(nanobook::Symbol, i64)>> {
    prices
        .iter()
        .map(|(s, p)| Ok((parse_symbol(s)?, *p)))
        .collect()
}

/// Parse Python list of (str, f64) into Vec<(Symbol, f64)>.
fn parse_target_list(targets: &[(String, f64)]) -> PyResult<Vec<(nanobook::Symbol, f64)>> {
    targets
        .iter()
        .map(|(s, w)| Ok((parse_symbol(s)?, *w)))
        .collect()
}
