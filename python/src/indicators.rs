use nanobook::indicators;
use pyo3::prelude::*;

/// Compute simple moving average.
#[pyfunction]
#[pyo3(signature = (close, period))]
pub fn py_sma(close: Vec<f64>, period: usize) -> Vec<f64> {
    indicators::sma(&close, period)
}

/// Compute exponential moving average.
#[pyfunction]
#[pyo3(signature = (close, period))]
pub fn py_ema(close: Vec<f64>, period: usize) -> Vec<f64> {
    indicators::ema(&close, period)
}

/// Compute RSI (Relative Strength Index) using Wilder's smoothing.
///
/// Drop-in replacement for ``talib.RSI(close, timeperiod)``.
///
/// Args:
///     close: List of closing prices.
///     period: Lookback period (default 14).
///
/// Returns:
///     List of RSI values. NaN for the lookback period.
///
/// Example::
///
///     rsi = nanobook.py_rsi([44.0, 44.25, 44.5, ...], 14)
///
#[pyfunction]
#[pyo3(signature = (close, period=14))]
pub fn py_rsi(close: Vec<f64>, period: usize) -> Vec<f64> {
    indicators::rsi(&close, period)
}

/// Compute MACD (Moving Average Convergence Divergence).
///
/// Drop-in replacement for ``talib.MACD(close, fast, slow, signal)``.
///
/// Args:
///     close: List of closing prices.
///     fast_period: Fast EMA period (default 12).
///     slow_period: Slow EMA period (default 26).
///     signal_period: Signal line EMA period (default 9).
///
/// Returns:
///     Tuple of (macd_line, signal_line, histogram).
///
/// Example::
///
///     macd, signal, hist = nanobook.py_macd(closes, 12, 26, 9)
///
#[pyfunction]
#[pyo3(signature = (close, fast_period=12, slow_period=26, signal_period=9))]
pub fn py_macd(
    close: Vec<f64>,
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    indicators::macd(&close, fast_period, slow_period, signal_period)
}

/// Compute Bollinger Bands (SMA +/- k * standard deviation).
///
/// Drop-in replacement for ``talib.BBANDS(close, period, nbdevup, nbdevdn)``.
///
/// Args:
///     close: List of closing prices.
///     period: SMA/stddev period (default 20).
///     num_std_up: Standard deviations above SMA (default 2.0).
///     num_std_dn: Standard deviations below SMA (default 2.0).
///
/// Returns:
///     Tuple of (upper_band, middle_band, lower_band).
///
/// Example::
///
///     upper, middle, lower = nanobook.py_bbands(closes, 20, 2.0, 2.0)
///
#[pyfunction]
#[pyo3(signature = (close, period=20, num_std_up=2.0, num_std_dn=2.0))]
pub fn py_bbands(
    close: Vec<f64>,
    period: usize,
    num_std_up: f64,
    num_std_dn: f64,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    indicators::bbands(&close, period, num_std_up, num_std_dn)
}

/// Explicit Bollinger Bands alias.
#[pyfunction]
#[pyo3(signature = (close, period=20, num_std_up=2.0, num_std_dn=2.0))]
pub fn py_bollinger(
    close: Vec<f64>,
    period: usize,
    num_std_up: f64,
    num_std_dn: f64,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    indicators::bollinger(&close, period, num_std_up, num_std_dn)
}

/// Compute ATR (Average True Range) using Wilder's smoothing.
///
/// Drop-in replacement for ``talib.ATR(high, low, close, timeperiod)``.
///
/// Args:
///     high: List of high prices.
///     low: List of low prices.
///     close: List of closing prices.
///     period: Lookback period (default 14).
///
/// Returns:
///     List of ATR values. NaN for the lookback period.
///
/// Example::
///
///     atr = nanobook.py_atr(highs, lows, closes, 14)
///
#[pyfunction]
#[pyo3(signature = (high, low, close, period=14))]
pub fn py_atr(high: Vec<f64>, low: Vec<f64>, close: Vec<f64>, period: usize) -> Vec<f64> {
    indicators::wilder_atr(&high, &low, &close, period)
}

/// Explicit Wilder ATR alias for callers distinguishing ATR variants.
#[pyfunction]
#[pyo3(signature = (high, low, close, period=14))]
pub fn py_wilder_atr(high: Vec<f64>, low: Vec<f64>, close: Vec<f64>, period: usize) -> Vec<f64> {
    indicators::wilder_atr(&high, &low, &close, period)
}

/// Stochastic oscillator (slow %K and %D).
#[pyfunction]
#[pyo3(signature = (high, low, close, fastk_period=5, slowk_period=3, slowd_period=3))]
pub fn py_stoch(
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    fastk_period: usize,
    slowk_period: usize,
    slowd_period: usize,
) -> (Vec<f64>, Vec<f64>) {
    indicators::stoch(
        &high,
        &low,
        &close,
        fastk_period,
        slowk_period,
        slowd_period,
    )
}

/// Fast stochastic oscillator.
#[pyfunction]
#[pyo3(signature = (high, low, close, fastk_period=5, fastd_period=3))]
pub fn py_stochf(
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    fastk_period: usize,
    fastd_period: usize,
) -> (Vec<f64>, Vec<f64>) {
    indicators::stochf(&high, &low, &close, fastk_period, fastd_period)
}

/// Plus Directional Indicator.
#[pyfunction]
#[pyo3(signature = (high, low, close, period=14))]
pub fn py_plus_di(high: Vec<f64>, low: Vec<f64>, close: Vec<f64>, period: usize) -> Vec<f64> {
    indicators::plus_di(&high, &low, &close, period)
}

/// Minus Directional Indicator.
#[pyfunction]
#[pyo3(signature = (high, low, close, period=14))]
pub fn py_minus_di(high: Vec<f64>, low: Vec<f64>, close: Vec<f64>, period: usize) -> Vec<f64> {
    indicators::minus_di(&high, &low, &close, period)
}

/// Directional Movement Index.
#[pyfunction]
#[pyo3(signature = (high, low, close, period=14))]
pub fn py_dx(high: Vec<f64>, low: Vec<f64>, close: Vec<f64>, period: usize) -> Vec<f64> {
    indicators::dx(&high, &low, &close, period)
}

/// Average Directional Movement Index.
#[pyfunction]
#[pyo3(signature = (high, low, close, period=14))]
pub fn py_adx(high: Vec<f64>, low: Vec<f64>, close: Vec<f64>, period: usize) -> Vec<f64> {
    indicators::adx(&high, &low, &close, period)
}

/// Commodity Channel Index.
#[pyfunction]
#[pyo3(signature = (high, low, close, period=14))]
pub fn py_cci(high: Vec<f64>, low: Vec<f64>, close: Vec<f64>, period: usize) -> Vec<f64> {
    indicators::cci(&high, &low, &close, period)
}

/// Williams' %R.
#[pyfunction]
#[pyo3(signature = (high, low, close, period=14))]
pub fn py_willr(high: Vec<f64>, low: Vec<f64>, close: Vec<f64>, period: usize) -> Vec<f64> {
    indicators::willr(&high, &low, &close, period)
}

/// Ultimate Oscillator.
#[pyfunction]
#[pyo3(signature = (high, low, close, period1=7, period2=14, period3=28))]
pub fn py_ultosc(
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    period1: usize,
    period2: usize,
    period3: usize,
) -> Vec<f64> {
    indicators::ultosc(&high, &low, &close, period1, period2, period3)
}

/// Momentum.
#[pyfunction]
#[pyo3(signature = (close, period=10))]
pub fn py_mom(close: Vec<f64>, period: usize) -> Vec<f64> {
    indicators::mom(&close, period)
}

/// Rate of change (percent).
#[pyfunction]
#[pyo3(signature = (close, period=10))]
pub fn py_roc(close: Vec<f64>, period: usize) -> Vec<f64> {
    indicators::roc(&close, period)
}

/// Rate of change (ratio).
#[pyfunction]
#[pyo3(signature = (close, period=10))]
pub fn py_rocp(close: Vec<f64>, period: usize) -> Vec<f64> {
    indicators::rocp(&close, period)
}

/// Rate of change multiplier.
#[pyfunction]
#[pyo3(signature = (close, period=10))]
pub fn py_rocr(close: Vec<f64>, period: usize) -> Vec<f64> {
    indicators::rocr(&close, period)
}

/// On Balance Volume.
#[pyfunction]
pub fn py_obv(close: Vec<f64>, volume: Vec<f64>) -> Vec<f64> {
    indicators::obv(&close, &volume)
}

/// Chaikin Accumulation/Distribution Line.
#[pyfunction]
pub fn py_ad(high: Vec<f64>, low: Vec<f64>, close: Vec<f64>, volume: Vec<f64>) -> Vec<f64> {
    indicators::ad(&high, &low, &close, &volume)
}

/// Chaikin A/D Oscillator.
#[pyfunction]
#[pyo3(signature = (high, low, close, volume, fast_period=3, slow_period=10))]
pub fn py_adosc(
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    volume: Vec<f64>,
    fast_period: usize,
    slow_period: usize,
) -> Vec<f64> {
    indicators::adosc(&high, &low, &close, &volume, fast_period, slow_period)
}

/// Normalized Average True Range.
#[pyfunction]
#[pyo3(signature = (high, low, close, period=14))]
pub fn py_natr(high: Vec<f64>, low: Vec<f64>, close: Vec<f64>, period: usize) -> Vec<f64> {
    indicators::natr(&high, &low, &close, period)
}

/// True Range (unsmoothed).
#[pyfunction]
pub fn py_trange(high: Vec<f64>, low: Vec<f64>, close: Vec<f64>) -> Vec<f64> {
    indicators::trange(&high, &low, &close)
}

/// Stochastic RSI.
#[pyfunction]
#[pyo3(signature = (close, timeperiod=14, fastk_period=5, fastd_period=3))]
pub fn py_stochrsi(
    close: Vec<f64>,
    timeperiod: usize,
    fastk_period: usize,
    fastd_period: usize,
) -> (Vec<f64>, Vec<f64>) {
    indicators::stochrsi(&close, timeperiod, fastk_period, fastd_period)
}

/// List supported technical indicators with parity metadata.
#[pyfunction]
pub fn py_list_supported_indicators(py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
    use pyo3::types::PyDict;

    indicators::list_supported()
        .iter()
        .map(|meta| {
            let dict = PyDict::new(py);
            dict.set_item("name", meta.name)?;
            dict.set_item("category", meta.category)?;
            dict.set_item("input_type", meta.input_type)?;
            dict.set_item("rust_fn", meta.rust_fn)?;
            dict.set_item("has_parity", meta.has_parity)?;
            Ok(dict.into())
        })
        .collect()
}
