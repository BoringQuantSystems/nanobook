//! PyO3 bindings for Monte Carlo scenario generation.
//!
//! Architecture (ADR-0007): Python hot path calls native ChaCha20 (`monte_carlo_stock_valuation_native`).
//! Audit / frozen parity uses NumPy `default_rng` draws (`monte_carlo_stock_valuation_parity`, ADR-0006).

use nanobook::scenarios::{
    ModelVersion, MonteCarloResult, ValuationParams, advanced_from_driver_batches, assemble_result,
    monte_carlo_stock_valuation as native_mc, nondeterministic_mc_seed, simple_gbm_from_z,
    validate_mc_inputs,
};
use numpy::PyArray1;
use pyo3::buffer::PyBuffer;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyDict, PyModule};

fn vec_from_numpy(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
    let numpy = PyModule::import(py, "numpy")?;
    let arr = numpy
        .getattr("asarray")?
        .call1((obj,))?
        .call_method1("astype", ("float64",))?
        .call_method0("ravel")?;
    let buffer = PyBuffer::<f64>::get(&arr)?;
    let mut out = vec![0.0; buffer.item_count()];
    buffer.copy_to_slice(py, &mut out)?;
    Ok(out)
}

fn numpy_rng(py: Python<'_>, seed: Option<i64>) -> PyResult<Bound<'_, PyAny>> {
    let numpy = PyModule::import(py, "numpy")?;
    let random = numpy.getattr("random")?;
    match seed {
        Some(s) => random.call_method1("default_rng", (s,)),
        None => random.call_method0("default_rng"),
    }
}

#[pyclass(name = "MonteCarloResult")]
pub struct PyMonteCarloResult {
    inner: MonteCarloResult,
}

#[pymethods]
impl PyMonteCarloResult {
    #[getter]
    fn ticker(&self) -> &str {
        &self.inner.ticker
    }

    #[getter]
    fn method(&self) -> &str {
        &self.inner.method
    }

    #[getter]
    fn horizon_years(&self) -> f64 {
        self.inner.horizon_years
    }

    #[getter]
    fn current_price(&self) -> f64 {
        self.inner.current_price
    }

    #[getter]
    fn terminal_prices<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_vec(py, self.inner.terminal_prices.clone())
    }

    #[getter]
    fn summary<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        d.set_item("ticker", &self.inner.ticker)?;
        d.set_item("method", &self.inner.method)?;
        for (k, v) in &self.inner.summary {
            d.set_item(k, v)?;
        }
        Ok(d)
    }

    #[getter]
    fn median_price(&self) -> f64 {
        self.inner.median_price()
    }

    #[getter]
    fn mean_price(&self) -> f64 {
        self.inner.mean_price()
    }

    #[getter]
    fn implied_median_annual_return(&self) -> f64 {
        self.inner.implied_median_annual_return()
    }

    #[getter]
    fn p10_price(&self) -> f64 {
        self.inner.p10_price()
    }

    #[getter]
    fn p90_price(&self) -> f64 {
        self.inner.p90_price()
    }

    fn prob_above(&self, level: f64) -> f64 {
        self.inner.prob_above(level)
    }

    fn quantile(&self, q: f64) -> f64 {
        self.inner.quantile(q)
    }

    fn as_log_returns(&self) -> Vec<f64> {
        self.inner.as_log_returns()
    }

    fn to_summary_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        self.summary(py)
    }

    fn __repr__(&self) -> String {
        format!(
            "MonteCarloResult(ticker={:?}, method={:?}, n_paths={}, median_price={})",
            self.inner.ticker,
            self.inner.method,
            self.inner.terminal_prices.len(),
            self.inner.median_price()
        )
    }
}

#[pyfunction]
#[pyo3(signature = (
    ticker,
    current_price,
    *,
    version = "advanced",
    n_paths = 5000,
    horizon = 1.0,
    seed = 42,
    expected_annual_return = 0.18,
    annual_vol = 0.38,
    gp_growth_mean = 0.16,
    gp_growth_sd = 0.06,
    margin_boost_mean = 0.02,
    margin_boost_sd = 0.03,
    multiple_mean = 22.0,
    multiple_sd = 3.5,
    macro_shock_mean = -0.03,
    macro_shock_sd = 0.11,
    bear_skew_factor = 0.04,
    hurdle_rate = 0.08,
    bull_price = None,
    bear_price = None,
))]
#[allow(clippy::too_many_arguments)]
pub fn monte_carlo_stock_valuation_parity(
    py: Python<'_>,
    ticker: String,
    current_price: f64,
    version: &str,
    n_paths: i64,
    horizon: f64,
    seed: Option<i64>,
    expected_annual_return: f64,
    annual_vol: f64,
    gp_growth_mean: f64,
    gp_growth_sd: f64,
    margin_boost_mean: f64,
    margin_boost_sd: f64,
    multiple_mean: f64,
    multiple_sd: f64,
    macro_shock_mean: f64,
    macro_shock_sd: f64,
    bear_skew_factor: f64,
    hurdle_rate: f64,
    bull_price: Option<f64>,
    bear_price: Option<f64>,
) -> PyResult<PyMonteCarloResult> {
    validate_mc_inputs(current_price, n_paths, horizon, annual_vol)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;

    let model = ModelVersion::parse(version);
    let rng = numpy_rng(py, seed)?;

    let prices = if n_paths == 0 {
        Vec::new()
    } else if model == ModelVersion::Simple {
        let z = vec_from_numpy(py, &rng.call_method1("standard_normal", (n_paths,))?)?;
        simple_gbm_from_z(
            current_price,
            &z,
            horizon,
            expected_annual_return,
            annual_vol,
        )
    } else {
        let p = ValuationParams {
            gp_growth_mean,
            gp_growth_sd,
            margin_boost_mean,
            margin_boost_sd,
            multiple_mean,
            multiple_sd,
            macro_shock_mean,
            macro_shock_sd,
            bear_skew_factor,
        };
        let gp = vec_from_numpy(
            py,
            &rng.call_method1("normal", (p.gp_growth_mean, p.gp_growth_sd, n_paths))?,
        )?;
        let marg = vec_from_numpy(
            py,
            &rng.call_method1("normal", (p.margin_boost_mean, p.margin_boost_sd, n_paths))?,
        )?;
        let mult_raw = vec_from_numpy(
            py,
            &rng.call_method1("normal", (p.multiple_mean, p.multiple_sd, n_paths))?,
        )?;
        let macro_draw = vec_from_numpy(
            py,
            &rng.call_method1("normal", (p.macro_shock_mean, p.macro_shock_sd, n_paths))?,
        )?;
        let bear_skew = vec_from_numpy(
            py,
            &rng.call_method1("normal", (0.0, p.bear_skew_factor, n_paths))?,
        )?;
        advanced_from_driver_batches(
            current_price,
            horizon,
            &gp,
            &marg,
            &mult_raw,
            &macro_draw,
            &bear_skew,
        )
    };

    let method = if model == ModelVersion::Simple {
        "Simple GBM".to_string()
    } else {
        "Advanced Multi-Driver".to_string()
    };

    let inner = assemble_result(
        ticker,
        method,
        horizon,
        current_price,
        prices,
        n_paths,
        hurdle_rate,
        bull_price,
        bear_price,
    );

    Ok(PyMonteCarloResult { inner })
}

#[pyfunction]
#[pyo3(signature = (
    ticker,
    current_price,
    *,
    version = "advanced",
    n_paths = 5000,
    horizon = 1.0,
    seed = 42,
    expected_annual_return = 0.18,
    annual_vol = 0.38,
    gp_growth_mean = 0.16,
    gp_growth_sd = 0.06,
    margin_boost_mean = 0.02,
    margin_boost_sd = 0.03,
    multiple_mean = 22.0,
    multiple_sd = 3.5,
    macro_shock_mean = -0.03,
    macro_shock_sd = 0.11,
    bear_skew_factor = 0.04,
    hurdle_rate = 0.08,
    bull_price = None,
    bear_price = None,
))]
#[allow(clippy::too_many_arguments)]
pub fn monte_carlo_stock_valuation_native(
    ticker: String,
    current_price: f64,
    version: &str,
    n_paths: i64,
    horizon: f64,
    seed: Option<i64>,
    expected_annual_return: f64,
    annual_vol: f64,
    gp_growth_mean: f64,
    gp_growth_sd: f64,
    margin_boost_mean: f64,
    margin_boost_sd: f64,
    multiple_mean: f64,
    multiple_sd: f64,
    macro_shock_mean: f64,
    macro_shock_sd: f64,
    bear_skew_factor: f64,
    hurdle_rate: f64,
    bull_price: Option<f64>,
    bear_price: Option<f64>,
) -> PyResult<PyMonteCarloResult> {
    let model = ModelVersion::parse(version);
    let seed_u64 = seed
        .map(|s| s as u64)
        .unwrap_or_else(nondeterministic_mc_seed);
    let params = ValuationParams {
        gp_growth_mean,
        gp_growth_sd,
        margin_boost_mean,
        margin_boost_sd,
        multiple_mean,
        multiple_sd,
        macro_shock_mean,
        macro_shock_sd,
        bear_skew_factor,
    };
    let inner = native_mc(
        ticker,
        current_price,
        model,
        n_paths,
        horizon,
        seed_u64,
        expected_annual_return,
        annual_vol,
        params,
        hurdle_rate,
        bull_price,
        bear_price,
    )
    .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(PyMonteCarloResult { inner })
}
