//! Technical analysis indicators.
//!
//! Drop-in replacements for TA-Lib's RSI, MACD, Bollinger Bands, and ATR.
//! All functions use the same algorithms and conventions as TA-Lib so that
//! outputs are numerically identical (within floating-point tolerance).
//!
//! # Conventions
//!
//! - Input slices are `&[f64]` (closing prices, or OHLC for ATR).
//! - Output `Vec<f64>` has the same length as input; elements within the
//!   lookback period are filled with `f64::NAN`.
//! - **Wilder's smoothing** (RSI, ATR): `alpha = 1/period`, NOT `2/(period+1)`.
//! - **Standard EMA** (MACD): `alpha = 2/(period+1)`.
//!
//! # References
//!
//! - TA-Lib source: `ta_RSI.c`, `ta_MACD.c`, `ta_BBANDS.c`, `ta_ATR.c`
//!   <https://github.com/TA-Lib/ta-lib/tree/main/src/ta_func>

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Standard exponential moving average (alpha = 2/(period+1)).
///
/// Used by MACD (fast EMA, slow EMA, signal line).
pub fn ema(values: &[f64], period: usize) -> Vec<f64> {
    let n = values.len();
    let mut out = vec![f64::NAN; n];
    if n < period || period == 0 {
        return out;
    }

    // Seed: simple average of first `period` values
    let seed: f64 = values[..period].iter().sum::<f64>() / period as f64;
    out[period - 1] = seed;

    let multiplier = 2.0 / (period as f64 + 1.0);
    for i in period..n {
        out[i] = (values[i] - out[i - 1]) * multiplier + out[i - 1];
    }
    out
}

/// Simple moving average.
pub fn sma(values: &[f64], period: usize) -> Vec<f64> {
    let n = values.len();
    let mut out = vec![f64::NAN; n];
    if n < period || period == 0 {
        return out;
    }

    let mut window_sum: f64 = values[..period].iter().sum();
    out[period - 1] = window_sum / period as f64;

    for i in period..n {
        window_sum += values[i] - values[i - period];
        out[i] = window_sum / period as f64;
    }
    out
}

/// Population standard deviation (ddof=0) over a rolling window.
///
/// Returns NaN for the lookback period.
///
/// # Numerical notes
///
/// Earlier implementations used an O(1) sliding state with the formula
/// `sum_sq / k - mean^2`. This formula suffers catastrophic cancellation
/// on high-mean, low-variance series (e.g., a $1000 stock with sub-cent
/// moves): both terms are large and nearly equal, so their difference
/// loses most of its precision to rounding. The `.max(0.0)` guard then
/// silently clamps the (now slightly negative) cancelled variance to
/// zero, so `rolling_std_pop` returned exactly 0 — and Bollinger bands
/// collapsed to the middle band.
///
/// This rewrite recomputes Welford per window, O(window) per step.
fn rolling_std_pop(values: &[f64], period: usize) -> Vec<f64> {
    let n = values.len();
    let mut out = vec![f64::NAN; n];
    if n < period || period == 0 {
        return out;
    }

    let k = period as f64;
    for i in (period - 1)..n {
        let slice = &values[i + 1 - period..=i];
        let (_mean, m2) = crate::stats::welford_mean_m2(slice);
        out[i] = (m2 / k).max(0.0).sqrt();
    }
    out
}

// ---------------------------------------------------------------------------
// Public indicators
// ---------------------------------------------------------------------------

/// Compute RSI value from average gain/loss (TA-Lib convention).
///
/// - Both zero (flat price) returns 0.0.
/// - Zero loss (always up) returns 100.0.
/// - Otherwise: 100 - 100/(1 + gain/loss).
fn rsi_from_avgs(avg_gain: f64, avg_loss: f64) -> f64 {
    if avg_gain == 0.0 && avg_loss == 0.0 {
        0.0
    } else if avg_loss == 0.0 {
        100.0
    } else {
        100.0 - 100.0 / (1.0 + avg_gain / avg_loss)
    }
}

/// Relative Strength Index (Wilder's smoothing).
///
/// Matches TA-Lib `ta_RSI.c` behavior:
/// - Lookback: first `period` elements are NaN.
/// - When all gains are zero (flat price), returns 0.0 (not 50.0).
/// - When all losses are zero (always up), returns 100.0.
///
/// # Insufficient data
///
/// Returns a vector of `close.len()` NaN values when
/// `close.len() <= period` or `period == 0`. At least `period + 1` prices
/// are required to produce the first non-NaN RSI value (one extra point
/// to compute the `period` price changes used for the initial
/// gain/loss averages). Matches TA-Lib.
///
/// # Arguments
///
/// * `close` — Closing prices.
/// * `period` — Lookback period (typically 14).
///
/// # Example
///
/// ```
/// use nanobook::indicators::rsi;
///
/// let close = vec![44.0, 44.25, 44.50, 43.75, 44.50, 44.25, 43.50,
///                  44.00, 44.50, 43.25, 43.00, 43.50, 44.00, 44.50,
///                  44.25, 44.00, 43.50, 43.75, 44.00, 43.25];
/// let result = rsi(&close, 14);
/// assert!(result[13].is_nan());  // lookback period
/// assert!(!result[14].is_nan()); // first valid RSI
/// ```
pub fn rsi(close: &[f64], period: usize) -> Vec<f64> {
    let n = close.len();
    let mut out = vec![f64::NAN; n];
    if n <= period || period == 0 {
        return out;
    }

    // Seed with simple average over first `period` changes (indices 1..=period)
    let mut avg_gain = 0.0_f64;
    let mut avg_loss = 0.0_f64;
    for i in 1..=period {
        let diff = close[i] - close[i - 1];
        if diff > 0.0 {
            avg_gain += diff;
        } else {
            avg_loss -= diff;
        }
    }
    avg_gain /= period as f64;
    avg_loss /= period as f64;

    // First RSI value
    out[period] = rsi_from_avgs(avg_gain, avg_loss);

    // Subsequent values with Wilder's smoothing
    for i in (period + 1)..n {
        let diff = close[i] - close[i - 1];
        let gain = if diff > 0.0 { diff } else { 0.0 };
        let loss = if diff < 0.0 { -diff } else { 0.0 };
        avg_gain = (avg_gain * (period as f64 - 1.0) + gain) / period as f64;
        avg_loss = (avg_loss * (period as f64 - 1.0) + loss) / period as f64;

        out[i] = rsi_from_avgs(avg_gain, avg_loss);
    }

    out
}

/// Moving Average Convergence Divergence (MACD).
///
/// Matches TA-Lib `ta_MACD.c` behavior:
/// - Fast/slow lines use standard EMA (alpha = 2/(period+1)).
/// - Signal line is EMA of the MACD line.
/// - Histogram = MACD line − signal line.
///
/// Returns `(macd_line, signal_line, histogram)`.
///
/// NaN is filled for the lookback period: `slow_period + signal_period - 2` elements.
///
/// # Arguments
///
/// * `close` — Closing prices.
/// * `fast_period` — Fast EMA period (typically 12).
/// * `slow_period` — Slow EMA period (typically 26).
/// * `signal_period` — Signal line EMA period (typically 9).
pub fn macd(
    close: &[f64],
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = close.len();
    let nan_vec = || vec![f64::NAN; n];

    if n < slow_period
        || fast_period == 0
        || slow_period == 0
        || signal_period == 0
        || fast_period >= slow_period
    {
        return (nan_vec(), nan_vec(), nan_vec());
    }

    // TA-Lib aligns both EMAs so they first produce a value at index slow_period-1.
    // The fast EMA is seeded from close[slow_period-fast_period..slow_period],
    // NOT from close[0..fast_period]. This ensures both EMAs start from the same bar.
    let offset = slow_period - fast_period;
    let fast_ema = ema(&close[offset..], fast_period);
    let slow_ema = ema(close, slow_period);

    // MACD line = fast EMA - slow EMA (internally valid from slow_period - 1).
    let slow_first = slow_period - 1;
    let mut macd_internal = vec![f64::NAN; n];
    for i in slow_first..n {
        let fi = i - offset;
        if !fast_ema[fi].is_nan() && !slow_ema[i].is_nan() {
            macd_internal[i] = fast_ema[fi] - slow_ema[i];
        }
    }

    // Signal line = EMA of the MACD line (seeded from slow_first).
    let signal_raw = ema(&macd_internal[slow_first..], signal_period);

    // TA-Lib exposes macd/signal/histogram only once the signal EMA is
    // warm: lookback = slow_period + signal_period - 2 (first index =
    // slow_first + signal_period - 1).
    let output_first = slow_first + signal_period - 1;

    let mut macd_line = vec![f64::NAN; n];
    let mut signal_line = vec![f64::NAN; n];
    let mut histogram = vec![f64::NAN; n];

    for (j, &sig) in signal_raw.iter().enumerate() {
        let i = slow_first + j;
        if i >= output_first && !macd_internal[i].is_nan() && !sig.is_nan() {
            macd_line[i] = macd_internal[i];
            signal_line[i] = sig;
            histogram[i] = macd_internal[i] - sig;
        }
    }

    (macd_line, signal_line, histogram)
}

/// Bollinger Bands (SMA +/- k * population standard deviation).
///
/// Matches TA-Lib `ta_BBANDS.c` behavior:
/// - Middle band = SMA.
/// - Upper band = SMA + num_std_up * stddev.
/// - Lower band = SMA - num_std_dn * stddev.
/// - Uses **population** standard deviation (ddof=0), matching TA-Lib.
///
/// Returns `(upper, middle, lower)`.
///
/// # Arguments
///
/// * `close` — Closing prices.
/// * `period` — SMA/stddev period (typically 20).
/// * `num_std_up` — Number of standard deviations above SMA (typically 2.0).
/// * `num_std_dn` — Number of standard deviations below SMA (typically 2.0).
///
/// # Zero-width bands
///
/// If `num_std_up == 0.0` the upper band equals the middle band
/// (SMA) exactly; likewise for `num_std_dn == 0.0` and the lower band.
/// No warning or error is emitted — this is a supported configuration
/// for callers who want a plain SMA returned alongside only one band, or
/// a bare SMA via `bbands(..., 0.0, 0.0)`.
pub fn bbands(
    close: &[f64],
    period: usize,
    num_std_up: f64,
    num_std_dn: f64,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = close.len();
    let middle = sma(close, period);
    let std = rolling_std_pop(close, period);

    let mut upper = vec![f64::NAN; n];
    let mut lower = vec![f64::NAN; n];

    for i in 0..n {
        if !middle[i].is_nan() {
            upper[i] = middle[i] + num_std_up * std[i];
            lower[i] = middle[i] - num_std_dn * std[i];
        }
    }

    (upper, middle, lower)
}

/// Explicit Bollinger Bands alias with the canonical public name.
pub fn bollinger(
    close: &[f64],
    period: usize,
    num_std_up: f64,
    num_std_dn: f64,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    bbands(close, period, num_std_up, num_std_dn)
}

/// Average True Range (Wilder's smoothing of True Range).
///
/// Matches TA-Lib `ta_ATR.c` behavior:
/// - True Range = max(H-L, |H-C_prev|, |L-C_prev|).
/// - First ATR value = simple average of first `period` True Range values.
/// - Subsequent values use Wilder's smoothing (alpha = 1/period).
///
/// # Arguments
///
/// * `high` — High prices.
/// * `low` — Low prices.
/// * `close` — Closing prices.
/// * `period` — Lookback period (typically 14).
pub fn wilder_atr(high: &[f64], low: &[f64], close: &[f64], period: usize) -> Vec<f64> {
    let n = high.len();
    if n != low.len() || n != close.len() {
        return vec![f64::NAN; n];
    }
    if n <= period || period == 0 {
        return vec![f64::NAN; n];
    }

    // Compute True Range series
    let mut tr = vec![0.0_f64; n];
    tr[0] = high[0] - low[0]; // First bar: just H-L (no previous close)
    for i in 1..n {
        let hl = high[i] - low[i];
        let hc = (high[i] - close[i - 1]).abs();
        let lc = (low[i] - close[i - 1]).abs();
        tr[i] = hl.max(hc).max(lc);
    }

    // Apply Wilder's smoothing to True Range (starting from index 1)
    // ATR lookback is `period` bars of True Range (from index 1 onward)
    let mut out = vec![f64::NAN; n];

    // Seed: simple average of first `period` True Range values (starting from index 1)
    let seed: f64 = tr[1..=period].iter().sum::<f64>() / period as f64;
    out[period] = seed;

    // Wilder's recursive smoothing
    for i in (period + 1)..n {
        out[i] = (out[i - 1] * (period as f64 - 1.0) + tr[i]) / period as f64;
    }

    out
}

/// Backward-compatible alias for Wilder ATR.
pub fn atr(high: &[f64], low: &[f64], close: &[f64], period: usize) -> Vec<f64> {
    wilder_atr(high, low, close, period)
}

/// Raw stochastic %K: `(close - LL) / (HH - LL) * 100` over `period`.
fn stoch_raw_k(close: f64, high_window: &[f64], low_window: &[f64]) -> f64 {
    let hh = high_window.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let ll = low_window.iter().copied().fold(f64::INFINITY, f64::min);
    let denom = hh - ll;
    if denom == 0.0 {
        50.0
    } else {
        (close - ll) / denom * 100.0
    }
}

/// Stochastic oscillator (slow %K and %D).
///
/// Matches TA-Lib `STOCH` with SMA smoothing (`matype=0`). Both outputs
/// are exposed only once `%D` is warm: lookback =
/// `(fastk + slowk + slowd) - 3` leading NaNs.
pub fn stoch(
    high: &[f64],
    low: &[f64],
    close: &[f64],
    fastk_period: usize,
    slowk_period: usize,
    slowd_period: usize,
) -> (Vec<f64>, Vec<f64>) {
    let n = close.len();
    let nan_pair = || (vec![f64::NAN; n], vec![f64::NAN; n]);
    if n == 0
        || fastk_period == 0
        || slowk_period == 0
        || slowd_period == 0
        || n < fastk_period
    {
        return nan_pair();
    }

    let mut raw_k = vec![f64::NAN; n];
    for i in (fastk_period - 1)..n {
        let hw = &high[i + 1 - fastk_period..=i];
        let lw = &low[i + 1 - fastk_period..=i];
        raw_k[i] = stoch_raw_k(close[i], hw, lw);
    }

    let mut slow_k = vec![f64::NAN; n];
    let k_smooth_start = fastk_period - 1 + slowk_period - 1;
    for i in k_smooth_start..n {
        let w = &raw_k[i + 1 - slowk_period..=i];
        slow_k[i] = w.iter().sum::<f64>() / slowk_period as f64;
    }

    let mut slow_d = vec![f64::NAN; n];
    let d_start = k_smooth_start + slowd_period - 1;
    for i in d_start..n {
        let w = &slow_k[i + 1 - slowd_period..=i];
        slow_d[i] = w.iter().sum::<f64>() / slowd_period as f64;
    }

    let output_first = fastk_period - 1 + slowk_period - 1 + slowd_period - 1;
    let mut out_k = vec![f64::NAN; n];
    let mut out_d = vec![f64::NAN; n];
    for i in output_first..n {
        out_k[i] = slow_k[i];
        out_d[i] = slow_d[i];
    }
    (out_k, out_d)
}

/// Fast stochastic oscillator (%K and %D).
///
/// Matches TA-Lib `STOCHF` with SMA `%D`. Outputs align once `%D` is warm.
pub fn stochf(
    high: &[f64],
    low: &[f64],
    close: &[f64],
    fastk_period: usize,
    fastd_period: usize,
) -> (Vec<f64>, Vec<f64>) {
    let n = close.len();
    let nan_pair = || (vec![f64::NAN; n], vec![f64::NAN; n]);
    if n == 0 || fastk_period == 0 || fastd_period == 0 || n < fastk_period {
        return nan_pair();
    }

    let mut raw_k = vec![f64::NAN; n];
    for i in (fastk_period - 1)..n {
        let hw = &high[i + 1 - fastk_period..=i];
        let lw = &low[i + 1 - fastk_period..=i];
        raw_k[i] = stoch_raw_k(close[i], hw, lw);
    }

    let mut fast_d = vec![f64::NAN; n];
    let d_start = fastk_period - 1 + fastd_period - 1;
    for i in d_start..n {
        let w = &raw_k[i + 1 - fastd_period..=i];
        fast_d[i] = w.iter().sum::<f64>() / fastd_period as f64;
    }

    let output_first = d_start;
    let mut out_k = vec![f64::NAN; n];
    let mut out_d = vec![f64::NAN; n];
    for i in output_first..n {
        out_k[i] = raw_k[i];
        out_d[i] = fast_d[i];
    }
    (out_k, out_d)
}

/// Stochastic RSI (%K and %D of RSI).
///
/// Matches TA-Lib `STOCHRSI` with SMA `%D`.
pub fn stochrsi(
    close: &[f64],
    timeperiod: usize,
    fastk_period: usize,
    fastd_period: usize,
) -> (Vec<f64>, Vec<f64>) {
    let rsi_series = rsi(close, timeperiod);
    // TA-Lib RSI(period) first finite value is at index `period`.
    stoch_on_series(&rsi_series, timeperiod, fastk_period, fastd_period)
}

/// One-period directional movement (+DM1, -DM1) per TA-Lib rules.
fn dm1(prev_high: f64, prev_low: f64, high: f64, low: f64) -> (f64, f64) {
    let diff_p = high - prev_high;
    let diff_m = prev_low - low;
    let plus = if diff_p > 0.0 && diff_p > diff_m { diff_p } else { 0.0 };
    let minus = if diff_m > 0.0 && diff_p < diff_m { diff_m } else { 0.0 };
    (plus, minus)
}

/// True range for one bar (H-L, |H-C_prev|, |L-C_prev|).
fn tr1(high: f64, low: f64, prev_close: f64) -> f64 {
    let hl = high - low;
    let hc = (high - prev_close).abs();
    let lc = (low - prev_close).abs();
    hl.max(hc).max(lc)
}

/// Directional index from smoothed +DI and -DI.
fn dx_from_di(plus_di: f64, minus_di: f64) -> f64 {
    let sum = plus_di + minus_di;
    if sum == 0.0 {
        0.0
    } else {
        100.0 * (plus_di - minus_di).abs() / sum
    }
}

/// Plus Directional Indicator (Wilder DM + TR smoothing).
///
/// Matches TA-Lib `PLUS_DI`. First finite value at index `period`.
pub fn plus_di(high: &[f64], low: &[f64], close: &[f64], period: usize) -> Vec<f64> {
    let n = high.len();
    let mut out = vec![f64::NAN; n];
    if n == 0 || period == 0 || high.len() != low.len() || high.len() != close.len() || n <= period
    {
        return out;
    }

    let mut today = 0usize;
    let mut prev_high = high[today];
    let mut prev_low = low[today];
    let mut prev_close = close[today];
    let mut prev_plus_dm = 0.0_f64;
    let mut prev_tr = 0.0_f64;

    for _ in 0..(period - 1) {
        today += 1;
        let (plus_dm, _) = dm1(prev_high, prev_low, high[today], low[today]);
        prev_high = high[today];
        prev_low = low[today];
        prev_plus_dm += plus_dm;
        prev_tr += tr1(prev_high, prev_low, prev_close);
        prev_close = close[today];
    }

    today += 1;
    let (plus_dm, _) = dm1(prev_high, prev_low, high[today], low[today]);
    prev_high = high[today];
    prev_low = low[today];
    prev_plus_dm -= prev_plus_dm / period as f64;
    prev_plus_dm += plus_dm;
    let tr = tr1(prev_high, prev_low, prev_close);
    prev_tr = prev_tr - prev_tr / period as f64 + tr;
    prev_close = close[today];
    out[today] = if prev_tr == 0.0 {
        0.0
    } else {
        100.0 * prev_plus_dm / prev_tr
    };

    while today + 1 < n {
        today += 1;
        let (plus_dm, _) = dm1(prev_high, prev_low, high[today], low[today]);
        prev_high = high[today];
        prev_low = low[today];
        prev_plus_dm -= prev_plus_dm / period as f64;
        prev_plus_dm += plus_dm;
        let tr = tr1(prev_high, prev_low, prev_close);
        prev_tr = prev_tr - prev_tr / period as f64 + tr;
        prev_close = close[today];
        out[today] = if prev_tr == 0.0 {
            0.0
        } else {
            100.0 * prev_plus_dm / prev_tr
        };
    }

    out
}

/// Minus Directional Indicator (Wilder DM + TR smoothing).
///
/// Matches TA-Lib `MINUS_DI`. First finite value at index `period`.
pub fn minus_di(high: &[f64], low: &[f64], close: &[f64], period: usize) -> Vec<f64> {
    let n = high.len();
    let mut out = vec![f64::NAN; n];
    if n == 0 || period == 0 || high.len() != low.len() || high.len() != close.len() || n <= period
    {
        return out;
    }

    let mut today = 0usize;
    let mut prev_high = high[today];
    let mut prev_low = low[today];
    let mut prev_close = close[today];
    let mut prev_minus_dm = 0.0_f64;
    let mut prev_tr = 0.0_f64;

    for _ in 0..(period - 1) {
        today += 1;
        let (_, minus_dm) = dm1(prev_high, prev_low, high[today], low[today]);
        prev_high = high[today];
        prev_low = low[today];
        prev_minus_dm += minus_dm;
        prev_tr += tr1(prev_high, prev_low, prev_close);
        prev_close = close[today];
    }

    today += 1;
    let (_, minus_dm) = dm1(prev_high, prev_low, high[today], low[today]);
    prev_high = high[today];
    prev_low = low[today];
    prev_minus_dm -= prev_minus_dm / period as f64;
    prev_minus_dm += minus_dm;
    let tr = tr1(prev_high, prev_low, prev_close);
    prev_tr = prev_tr - prev_tr / period as f64 + tr;
    prev_close = close[today];
    out[today] = if prev_tr == 0.0 {
        0.0
    } else {
        100.0 * prev_minus_dm / prev_tr
    };

    while today + 1 < n {
        today += 1;
        let (_, minus_dm) = dm1(prev_high, prev_low, high[today], low[today]);
        prev_high = high[today];
        prev_low = low[today];
        prev_minus_dm -= prev_minus_dm / period as f64;
        prev_minus_dm += minus_dm;
        let tr = tr1(prev_high, prev_low, prev_close);
        prev_tr = prev_tr - prev_tr / period as f64 + tr;
        prev_close = close[today];
        out[today] = if prev_tr == 0.0 {
            0.0
        } else {
            100.0 * prev_minus_dm / prev_tr
        };
    }

    out
}

/// Directional Movement Index.
///
/// Matches TA-Lib `DX`. First finite value at index `period`.
pub fn dx(high: &[f64], low: &[f64], close: &[f64], period: usize) -> Vec<f64> {
    let n = high.len();
    let mut out = vec![f64::NAN; n];
    if n == 0 || period == 0 || high.len() != low.len() || high.len() != close.len() || n <= period
    {
        return out;
    }

    let mut today = 0usize;
    let mut prev_high = high[today];
    let mut prev_low = low[today];
    let mut prev_close = close[today];
    let mut prev_plus_dm = 0.0_f64;
    let mut prev_minus_dm = 0.0_f64;
    let mut prev_tr = 0.0_f64;

    for _ in 0..(period - 1) {
        today += 1;
        let (plus_dm, minus_dm) = dm1(prev_high, prev_low, high[today], low[today]);
        prev_high = high[today];
        prev_low = low[today];
        prev_plus_dm += plus_dm;
        prev_minus_dm += minus_dm;
        prev_tr += tr1(prev_high, prev_low, prev_close);
        prev_close = close[today];
    }

    today += 1;
    let (plus_dm, minus_dm) = dm1(prev_high, prev_low, high[today], low[today]);
    prev_high = high[today];
    prev_low = low[today];
    prev_plus_dm -= prev_plus_dm / period as f64;
    prev_plus_dm += plus_dm;
    prev_minus_dm -= prev_minus_dm / period as f64;
    prev_minus_dm += minus_dm;
    let tr = tr1(prev_high, prev_low, prev_close);
    prev_tr = prev_tr - prev_tr / period as f64 + tr;
    prev_close = close[today];
    if prev_tr != 0.0 {
        let plus_di = 100.0 * prev_plus_dm / prev_tr;
        let minus_di = 100.0 * prev_minus_dm / prev_tr;
        out[today] = dx_from_di(plus_di, minus_di);
    } else {
        out[today] = 0.0;
    }

    while today + 1 < n {
        today += 1;
        let (plus_dm, minus_dm) = dm1(prev_high, prev_low, high[today], low[today]);
        prev_high = high[today];
        prev_low = low[today];
        prev_plus_dm -= prev_plus_dm / period as f64;
        prev_plus_dm += plus_dm;
        prev_minus_dm -= prev_minus_dm / period as f64;
        prev_minus_dm += minus_dm;
        let tr = tr1(prev_high, prev_low, prev_close);
        prev_tr = prev_tr - prev_tr / period as f64 + tr;
        prev_close = close[today];
        if prev_tr != 0.0 {
            let plus_di = 100.0 * prev_plus_dm / prev_tr;
            let minus_di = 100.0 * prev_minus_dm / prev_tr;
            out[today] = dx_from_di(plus_di, minus_di);
        } else {
            out[today] = 0.0;
        }
    }

    out
}

/// Average Directional Movement Index.
///
/// Matches TA-Lib `ADX`. First finite value at index `2 * period - 1`.
pub fn adx(high: &[f64], low: &[f64], close: &[f64], period: usize) -> Vec<f64> {
    let n = high.len();
    let mut out = vec![f64::NAN; n];
    if n == 0
        || period < 2
        || high.len() != low.len()
        || high.len() != close.len()
        || n <= 2 * period - 1
    {
        return out;
    }

    let lookback = 2 * period - 1;
    let mut today = 0usize;
    let mut prev_high = high[today];
    let mut prev_low = low[today];
    let mut prev_close = close[today];
    let mut prev_plus_dm = 0.0_f64;
    let mut prev_minus_dm = 0.0_f64;
    let mut prev_tr = 0.0_f64;

    for _ in 0..(period - 1) {
        today += 1;
        let (plus_dm, minus_dm) = dm1(prev_high, prev_low, high[today], low[today]);
        prev_high = high[today];
        prev_low = low[today];
        prev_plus_dm += plus_dm;
        prev_minus_dm += minus_dm;
        prev_tr += tr1(prev_high, prev_low, prev_close);
        prev_close = close[today];
    }

    let mut sum_dx = 0.0_f64;
    for _ in 0..period {
        today += 1;
        let (plus_dm, minus_dm) = dm1(prev_high, prev_low, high[today], low[today]);
        prev_high = high[today];
        prev_low = low[today];
        prev_minus_dm -= prev_minus_dm / period as f64;
        prev_minus_dm += minus_dm;
        prev_plus_dm -= prev_plus_dm / period as f64;
        prev_plus_dm += plus_dm;
        let tr = tr1(prev_high, prev_low, prev_close);
        prev_tr = prev_tr - prev_tr / period as f64 + tr;
        prev_close = close[today];
        if prev_tr != 0.0 {
            let plus_di = 100.0 * prev_plus_dm / prev_tr;
            let minus_di = 100.0 * prev_minus_dm / prev_tr;
            sum_dx += dx_from_di(plus_di, minus_di);
        }
    }

    let mut prev_adx = sum_dx / period as f64;
    out[today] = prev_adx;

    while today + 1 < n {
        today += 1;
        let (plus_dm, minus_dm) = dm1(prev_high, prev_low, high[today], low[today]);
        prev_high = high[today];
        prev_low = low[today];
        prev_minus_dm -= prev_minus_dm / period as f64;
        prev_minus_dm += minus_dm;
        prev_plus_dm -= prev_plus_dm / period as f64;
        prev_plus_dm += plus_dm;
        let tr = tr1(prev_high, prev_low, prev_close);
        prev_tr = prev_tr - prev_tr / period as f64 + tr;
        prev_close = close[today];
        if prev_tr != 0.0 {
            let plus_di = 100.0 * prev_plus_dm / prev_tr;
            let minus_di = 100.0 * prev_minus_dm / prev_tr;
            let dx = dx_from_di(plus_di, minus_di);
            prev_adx = (prev_adx * (period as f64 - 1.0) + dx) / period as f64;
        }
        out[today] = prev_adx;
    }

    // TA-Lib lookback is 2*period-1; ensure leading NaNs through lookback-1.
    for v in out.iter_mut().take(lookback) {
        *v = f64::NAN;
    }

    out
}

/// Typical price for CCI / ULTOSC.
fn typical_price(high: f64, low: f64, close: f64) -> f64 {
    (high + low + close) / 3.0
}

/// Commodity Channel Index.
///
/// Matches TA-Lib `CCI`. First finite value at index `period - 1`.
pub fn cci(high: &[f64], low: &[f64], close: &[f64], period: usize) -> Vec<f64> {
    let n = close.len();
    let mut out = vec![f64::NAN; n];
    if n < period || period < 2 || high.len() != low.len() || high.len() != close.len() {
        return out;
    }

    for i in (period - 1)..n {
        let start = i + 1 - period;
        let mut sum = 0.0_f64;
        let mut tp_vals = Vec::with_capacity(period);
        for j in start..=i {
            let tp = typical_price(high[j], low[j], close[j]);
            tp_vals.push(tp);
            sum += tp;
        }
        let avg = sum / period as f64;
        let last_tp = tp_vals[period - 1];
        let mean_dev: f64 = tp_vals.iter().map(|v| (v - avg).abs()).sum();
        let diff = last_tp - avg;
        out[i] = if diff != 0.0 && mean_dev != 0.0 {
            diff / (0.015 * (mean_dev / period as f64))
        } else {
            0.0
        };
    }

    out
}

/// Williams' %R.
///
/// Matches TA-Lib `WILLR`. First finite value at index `period - 1`.
pub fn willr(high: &[f64], low: &[f64], close: &[f64], period: usize) -> Vec<f64> {
    let n = close.len();
    let mut out = vec![f64::NAN; n];
    if n < period || period < 2 || high.len() != low.len() || high.len() != close.len() {
        return out;
    }

    for i in (period - 1)..n {
        let start = i + 1 - period;
        let highest = high[start..=i]
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        let lowest = low[start..=i]
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let denom = highest - lowest;
        out[i] = if denom == 0.0 {
            0.0
        } else {
            -100.0 * (highest - close[i]) / denom
        };
    }

    out
}

/// Ultimate Oscillator terms for one bar.
fn ultosc_terms(high: f64, low: f64, close: f64, prev_close: f64) -> (f64, f64) {
    let true_low = low.min(prev_close);
    let close_minus_true_low = close - true_low;
    let mut true_range = high - low;
    let hc = (prev_close - high).abs();
    if hc > true_range {
        true_range = hc;
    }
    let lc = (prev_close - low).abs();
    if lc > true_range {
        true_range = lc;
    }
    (close_minus_true_low, true_range)
}

/// Ultimate Oscillator.
///
/// Matches TA-Lib `ULTOSC` with default periods 7/14/28. First finite
/// value at index `max(period1, period2, period3)`.
pub fn ultosc(
    high: &[f64],
    low: &[f64],
    close: &[f64],
    period1: usize,
    period2: usize,
    period3: usize,
) -> Vec<f64> {
    let n = close.len();
    let mut out = vec![f64::NAN; n];
    if n == 0 || period1 == 0 || period2 == 0 || period3 == 0 {
        return out;
    }
    if high.len() != low.len() || high.len() != close.len() {
        return out;
    }

    let mut periods = [period1, period2, period3];
    periods.sort_unstable();
    let p_short = periods[0];
    let p_mid = periods[1];
    let p_long = periods[2];
    let lookback = p_long;
    if n <= lookback {
        return out;
    }

    let mut a1 = 0.0_f64;
    let mut b1 = 0.0_f64;
    let mut a2 = 0.0_f64;
    let mut b2 = 0.0_f64;
    let mut a3 = 0.0_f64;
    let mut b3 = 0.0_f64;

    let start = lookback;
    for i in (start - p_long + 1)..start {
        let (bp, tr) = ultosc_terms(high[i], low[i], close[i], close[i - 1]);
        a3 += bp;
        b3 += tr;
        if i >= start - p_mid + 1 {
            a2 += bp;
            b2 += tr;
        }
        if i >= start - p_short + 1 {
            a1 += bp;
            b1 += tr;
        }
    }

    let mut trailing1 = start - p_short + 1;
    let mut trailing2 = start - p_mid + 1;
    let mut trailing3 = start - p_long + 1;

    for today in start..n {
        let (bp, tr) = ultosc_terms(high[today], low[today], close[today], close[today - 1]);
        a1 += bp;
        a2 += bp;
        a3 += bp;
        b1 += tr;
        b2 += tr;
        b3 += tr;

        let mut output = 0.0_f64;
        if b1 != 0.0 {
            output += 4.0 * (a1 / b1);
        }
        if b2 != 0.0 {
            output += 2.0 * (a2 / b2);
        }
        if b3 != 0.0 {
            output += a3 / b3;
        }
        out[today] = 100.0 * (output / 7.0);

        let (bp1, tr1) =
            ultosc_terms(high[trailing1], low[trailing1], close[trailing1], close[trailing1 - 1]);
        a1 -= bp1;
        b1 -= tr1;
        let (bp2, tr2) =
            ultosc_terms(high[trailing2], low[trailing2], close[trailing2], close[trailing2 - 1]);
        a2 -= bp2;
        b2 -= tr2;
        let (bp3, tr3) =
            ultosc_terms(high[trailing3], low[trailing3], close[trailing3], close[trailing3 - 1]);
        a3 -= bp3;
        b3 -= tr3;

        trailing1 += 1;
        trailing2 += 1;
        trailing3 += 1;
    }

    out
}

/// Momentum: `close - close[period]`.
///
/// Matches TA-Lib `MOM`. First finite value at index `period`.
pub fn mom(close: &[f64], period: usize) -> Vec<f64> {
    let n = close.len();
    let mut out = vec![f64::NAN; n];
    if n <= period || period == 0 {
        return out;
    }
    for i in period..n {
        out[i] = close[i] - close[i - period];
    }
    out
}

/// Rate of change (percent): `100 * (close - close[period]) / close[period]`.
///
/// Matches TA-Lib `ROC`.
pub fn roc(close: &[f64], period: usize) -> Vec<f64> {
    let n = close.len();
    let mut out = vec![f64::NAN; n];
    if n <= period || period == 0 {
        return out;
    }
    for i in period..n {
        let prev = close[i - period];
        out[i] = if prev == 0.0 {
            0.0
        } else {
            100.0 * (close[i] - prev) / prev
        };
    }
    out
}

/// Rate of change (ratio): `(close - close[period]) / close[period]`.
///
/// Matches TA-Lib `ROCP`.
pub fn rocp(close: &[f64], period: usize) -> Vec<f64> {
    let n = close.len();
    let mut out = vec![f64::NAN; n];
    if n <= period || period == 0 {
        return out;
    }
    for i in period..n {
        let prev = close[i - period];
        out[i] = if prev == 0.0 {
            0.0
        } else {
            (close[i] - prev) / prev
        };
    }
    out
}

/// Rate of change ratio: `close / close[period]`.
///
/// Matches TA-Lib `ROCR`.
pub fn rocr(close: &[f64], period: usize) -> Vec<f64> {
    let n = close.len();
    let mut out = vec![f64::NAN; n];
    if n <= period || period == 0 {
        return out;
    }
    for i in period..n {
        let prev = close[i - period];
        out[i] = if prev == 0.0 {
            0.0
        } else {
            close[i] / prev
        };
    }
    out
}

/// Chaikin money-flow contribution for one bar.
fn ad_money_flow(high: f64, low: f64, close: f64, volume: f64) -> f64 {
    let hl = high - low;
    if hl > 0.0 {
        (((close - low) - (high - close)) / hl) * volume
    } else {
        0.0
    }
}

/// On Balance Volume.
///
/// Matches TA-Lib `OBV`: seeds with `volume[0]`, then adds/subtracts volume on
/// close up/down moves.
pub fn obv(close: &[f64], volume: &[f64]) -> Vec<f64> {
    let n = close.len();
    if n == 0 || volume.len() != n {
        return Vec::new();
    }

    let mut out = vec![0.0; n];
    let mut prev_obv = volume[0];
    let mut prev_close = close[0];
    out[0] = prev_obv;

    for i in 1..n {
        if close[i] > prev_close {
            prev_obv += volume[i];
        } else if close[i] < prev_close {
            prev_obv -= volume[i];
        }
        out[i] = prev_obv;
        prev_close = close[i];
    }
    out
}

/// Chaikin Accumulation/Distribution Line.
///
/// Matches TA-Lib `AD`.
pub fn ad(high: &[f64], low: &[f64], close: &[f64], volume: &[f64]) -> Vec<f64> {
    let n = close.len();
    if n == 0 || high.len() != n || low.len() != n || volume.len() != n {
        return Vec::new();
    }

    let mut out = vec![0.0; n];
    let mut cum = 0.0;
    for i in 0..n {
        cum += ad_money_flow(high[i], low[i], close[i], volume[i]);
        out[i] = cum;
    }
    out
}

/// Chaikin A/D Oscillator: `EMA(fast, AD) - EMA(slow, AD)`.
///
/// Matches TA-Lib `ADOSC` (EMA applied to the cumulative AD series).
pub fn adosc(
    high: &[f64],
    low: &[f64],
    close: &[f64],
    volume: &[f64],
    fast_period: usize,
    slow_period: usize,
) -> Vec<f64> {
    let n = close.len();
    let mut out = vec![f64::NAN; n];
    if n == 0
        || high.len() != n
        || low.len() != n
        || volume.len() != n
        || fast_period < 2
        || slow_period < 2
    {
        return out;
    }

    let slowest = fast_period.max(slow_period);
    let lookback = slowest - 1;
    if n <= lookback {
        return out;
    }

    let fast_k = 2.0 / (fast_period as f64 + 1.0);
    let one_minus_fast_k = 1.0 - fast_k;
    let slow_k = 2.0 / (slow_period as f64 + 1.0);
    let one_minus_slow_k = 1.0 - slow_k;

    let start_idx = lookback;
    let mut today = 0usize;
    let mut ad_cum = 0.0_f64;

    ad_cum += ad_money_flow(high[today], low[today], close[today], volume[today]);
    today += 1;
    let mut fast_ema = ad_cum;
    let mut slow_ema = ad_cum;

    while today < start_idx {
        ad_cum += ad_money_flow(high[today], low[today], close[today], volume[today]);
        today += 1;
        fast_ema = fast_k * ad_cum + one_minus_fast_k * fast_ema;
        slow_ema = slow_k * ad_cum + one_minus_slow_k * slow_ema;
    }

    let mut out_idx = start_idx;
    while today < n {
        ad_cum += ad_money_flow(high[today], low[today], close[today], volume[today]);
        today += 1;
        fast_ema = fast_k * ad_cum + one_minus_fast_k * fast_ema;
        slow_ema = slow_k * ad_cum + one_minus_slow_k * slow_ema;
        out[out_idx] = fast_ema - slow_ema;
        out_idx += 1;
    }

    out
}

/// True Range (unsmoothed).
///
/// Matches TA-Lib `TRANGE`: index 0 is NaN; valid from index 1.
pub fn trange(high: &[f64], low: &[f64], close: &[f64]) -> Vec<f64> {
    let n = high.len();
    let mut out = vec![f64::NAN; n];
    if n == 0 || low.len() != n || close.len() != n {
        return out;
    }
    if n < 2 {
        return out;
    }

    for i in 1..n {
        let hl = high[i] - low[i];
        let hc = (high[i] - close[i - 1]).abs();
        let lc = (low[i] - close[i - 1]).abs();
        out[i] = hl.max(hc).max(lc);
    }
    out
}

/// Wilder-smoothed ATR from a precomputed TRANGE series.
fn wilder_atr_from_trange(tr: &[f64], period: usize) -> Vec<f64> {
    let n = tr.len();
    let mut out = vec![f64::NAN; n];
    if n <= period || period == 0 {
        return out;
    }

    let seed: f64 = tr[1..=period].iter().sum::<f64>() / period as f64;
    out[period] = seed;

    for i in (period + 1)..n {
        out[i] = (out[i - 1] * (period as f64 - 1.0) + tr[i]) / period as f64;
    }
    out
}

/// Normalized Average True Range: `ATR / close * 100`.
///
/// Matches TA-Lib `NATR`.
pub fn natr(high: &[f64], low: &[f64], close: &[f64], period: usize) -> Vec<f64> {
    let n = high.len();
    let mut out = vec![f64::NAN; n];
    if n == 0 || low.len() != n || close.len() != n || period == 0 {
        return out;
    }

    let tr = trange(high, low, close);
    let atr = wilder_atr_from_trange(&tr, period);

    for i in 0..n {
        if !atr[i].is_nan() {
            out[i] = if close[i] == 0.0 {
                0.0
            } else {
                atr[i] / close[i] * 100.0
            };
        }
    }
    out
}

/// Apply fast stochastic smoothing to an existing series (used by STOCHRSI).
fn stoch_on_series(
    series: &[f64],
    series_first: usize,
    fastk_period: usize,
    fastd_period: usize,
) -> (Vec<f64>, Vec<f64>) {
    let n = series.len();
    let nan_pair = || (vec![f64::NAN; n], vec![f64::NAN; n]);
    if n == 0 || fastk_period == 0 || fastd_period == 0 {
        return nan_pair();
    }

    let mut raw_k = vec![f64::NAN; n];
    let raw_start = series_first + fastk_period - 1;
    for i in raw_start..n {
        let w = &series[i + 1 - fastk_period..=i];
        if w.iter().any(|v| v.is_nan()) {
            continue;
        }
        let hh = w.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let ll = w.iter().copied().fold(f64::INFINITY, f64::min);
        let denom = hh - ll;
        raw_k[i] = if denom == 0.0 {
            50.0
        } else {
            (series[i] - ll) / denom * 100.0
        };
    }

    let mut fast_d = vec![f64::NAN; n];
    let d_start = raw_start + fastd_period - 1;
    for i in d_start..n {
        let w = &raw_k[i + 1 - fastd_period..=i];
        if w.iter().any(|v| v.is_nan()) {
            continue;
        }
        fast_d[i] = w.iter().sum::<f64>() / fastd_period as f64;
    }

    let output_first = series_first + fastk_period - 1 + fastd_period - 1;
    let mut out_k = vec![f64::NAN; n];
    let mut out_d = vec![f64::NAN; n];
    for i in output_first..n {
        out_k[i] = raw_k[i];
        out_d[i] = fast_d[i];
    }
    (out_k, out_d)
}

// ---------------------------------------------------------------------------
// Discoverability (mirrors tests/parity/indicator_registry.json)
// ---------------------------------------------------------------------------

/// Metadata for a supported technical indicator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndicatorMeta {
    pub name: &'static str,
    pub category: &'static str,
    pub input_type: &'static str,
    pub rust_fn: &'static str,
    pub has_parity: bool,
}

/// Return every indicator with golden parity coverage (Group A today).
pub fn list_supported() -> &'static [IndicatorMeta] {
    const SUPPORTED: &[IndicatorMeta] = &[
        IndicatorMeta {
            name: "sma",
            category: "overlap",
            input_type: "close",
            rust_fn: "sma",
            has_parity: true,
        },
        IndicatorMeta {
            name: "ema",
            category: "overlap",
            input_type: "close",
            rust_fn: "ema",
            has_parity: true,
        },
        IndicatorMeta {
            name: "rsi",
            category: "momentum",
            input_type: "close",
            rust_fn: "rsi",
            has_parity: true,
        },
        IndicatorMeta {
            name: "macd",
            category: "momentum",
            input_type: "close",
            rust_fn: "macd",
            has_parity: true,
        },
        IndicatorMeta {
            name: "bbands",
            category: "overlap",
            input_type: "close",
            rust_fn: "bbands",
            has_parity: true,
        },
        IndicatorMeta {
            name: "atr",
            category: "volatility",
            input_type: "ohlc",
            rust_fn: "atr",
            has_parity: true,
        },
        IndicatorMeta {
            name: "stoch",
            category: "momentum",
            input_type: "ohlc",
            rust_fn: "stoch",
            has_parity: true,
        },
        IndicatorMeta {
            name: "stochf",
            category: "momentum",
            input_type: "ohlc",
            rust_fn: "stochf",
            has_parity: true,
        },
        IndicatorMeta {
            name: "stochrsi",
            category: "momentum",
            input_type: "close",
            rust_fn: "stochrsi",
            has_parity: true,
        },
        IndicatorMeta {
            name: "adx",
            category: "momentum",
            input_type: "ohlc",
            rust_fn: "adx",
            has_parity: true,
        },
        IndicatorMeta {
            name: "plus_di",
            category: "momentum",
            input_type: "ohlc",
            rust_fn: "plus_di",
            has_parity: true,
        },
        IndicatorMeta {
            name: "minus_di",
            category: "momentum",
            input_type: "ohlc",
            rust_fn: "minus_di",
            has_parity: true,
        },
        IndicatorMeta {
            name: "dx",
            category: "momentum",
            input_type: "ohlc",
            rust_fn: "dx",
            has_parity: true,
        },
        IndicatorMeta {
            name: "cci",
            category: "momentum",
            input_type: "ohlc",
            rust_fn: "cci",
            has_parity: true,
        },
        IndicatorMeta {
            name: "willr",
            category: "momentum",
            input_type: "ohlc",
            rust_fn: "willr",
            has_parity: true,
        },
        IndicatorMeta {
            name: "ultosc",
            category: "momentum",
            input_type: "ohlc",
            rust_fn: "ultosc",
            has_parity: true,
        },
        IndicatorMeta {
            name: "mom",
            category: "momentum",
            input_type: "close",
            rust_fn: "mom",
            has_parity: true,
        },
        IndicatorMeta {
            name: "roc",
            category: "momentum",
            input_type: "close",
            rust_fn: "roc",
            has_parity: true,
        },
        IndicatorMeta {
            name: "rocp",
            category: "momentum",
            input_type: "close",
            rust_fn: "rocp",
            has_parity: true,
        },
        IndicatorMeta {
            name: "rocr",
            category: "momentum",
            input_type: "close",
            rust_fn: "rocr",
            has_parity: true,
        },
        IndicatorMeta {
            name: "obv",
            category: "volume",
            input_type: "close_volume",
            rust_fn: "obv",
            has_parity: true,
        },
        IndicatorMeta {
            name: "ad",
            category: "volume",
            input_type: "ohlcv",
            rust_fn: "ad",
            has_parity: true,
        },
        IndicatorMeta {
            name: "adosc",
            category: "volume",
            input_type: "ohlcv",
            rust_fn: "adosc",
            has_parity: true,
        },
        IndicatorMeta {
            name: "natr",
            category: "volatility",
            input_type: "ohlc",
            rust_fn: "natr",
            has_parity: true,
        },
        IndicatorMeta {
            name: "trange",
            category: "volatility",
            input_type: "ohlc",
            rust_fn: "trange",
            has_parity: true,
        },
    ];
    SUPPORTED
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sma_basic() {
        let result = sma(&[1.0, 2.0, 3.0, 4.0], 2);
        assert!(result[0].is_nan());
        assert_eq!(result[1], 1.5);
        assert_eq!(result[2], 2.5);
        assert_eq!(result[3], 3.5);
    }

    #[test]
    fn ema_basic() {
        let result = ema(&[1.0, 2.0, 3.0, 4.0], 2);
        assert!(result[0].is_nan());
        assert_eq!(result[1], 1.5);
        assert!((result[2] - 2.5).abs() < 1e-12);
        assert!((result[3] - 3.5).abs() < 1e-12);
    }

    #[test]
    fn bollinger_alias_matches_bbands() {
        let close = [1.0, 2.0, 3.0, 4.0, 5.0];
        let bb = bbands(&close, 3, 2.0, 2.0);
        let alias = bollinger(&close, 3, 2.0, 2.0);
        for (left, right) in [(&bb.0, &alias.0), (&bb.1, &alias.1), (&bb.2, &alias.2)] {
            for (a, b) in left.iter().zip(right.iter()) {
                assert!(a == b || (a.is_nan() && b.is_nan()));
            }
        }
    }

    #[test]
    fn wilder_atr_alias_matches_atr() {
        let high = [11.0, 12.0, 13.0, 14.0];
        let low = [9.0, 10.0, 11.0, 12.0];
        let close = [10.0, 11.0, 12.0, 13.0];
        let wilder = wilder_atr(&high, &low, &close, 2);
        let alias = atr(&high, &low, &close, 2);
        for (a, b) in wilder.iter().zip(alias.iter()) {
            assert!(a == b || (a.is_nan() && b.is_nan()));
        }
    }

    #[test]
    fn rsi_monotonic_up() {
        let close: Vec<f64> = (1..=100).map(|x| x as f64).collect();
        let result = rsi(&close, 14);
        // All gains, no losses → RSI should be 100
        let last = result.last().unwrap();
        assert!((*last - 100.0).abs() < 1e-10);
    }

    #[test]
    fn rsi_monotonic_down() {
        let close: Vec<f64> = (1..=100).rev().map(|x| x as f64).collect();
        let result = rsi(&close, 14);
        // All losses, no gains → RSI should be 0
        let last = result.last().unwrap();
        assert!(last.abs() < 1e-10);
    }

    #[test]
    fn rsi_constant_price() {
        let close = vec![100.0; 50];
        let result = rsi(&close, 14);
        // Flat price: TA-Lib returns 0.0
        let last = result.last().unwrap();
        assert!(
            last.abs() < 1e-10,
            "expected 0.0 for flat price, got {last}"
        );
    }

    #[test]
    fn rsi_bounds() {
        let close = vec![
            44.0, 44.25, 44.50, 43.75, 44.50, 44.25, 43.50, 44.0, 44.50, 43.25, 43.0, 43.50, 44.0,
            44.50, 44.25, 44.0, 43.50, 43.75, 44.0, 43.25,
        ];
        let result = rsi(&close, 14);
        for (i, &v) in result.iter().enumerate() {
            if !v.is_nan() {
                assert!(
                    (0.0..=100.0).contains(&v),
                    "RSI out of bounds at index {i}: {v}"
                );
            }
        }
    }

    #[test]
    fn rsi_lookback_nan() {
        let close: Vec<f64> = (1..=30).map(|x| x as f64).collect();
        let result = rsi(&close, 14);
        // First 14 elements should be NaN (indices 0..14)
        for (i, v) in result.iter().take(14).enumerate() {
            assert!(v.is_nan(), "expected NaN at index {i}");
        }
        assert!(!result[14].is_nan(), "expected valid RSI at index 14");
    }

    #[test]
    fn macd_basic() {
        let close: Vec<f64> = (1..=50).map(|x| x as f64).collect();
        let (macd_line, signal, histogram) = macd(&close, 12, 26, 9);
        assert_eq!(macd_line.len(), 50);
        assert_eq!(signal.len(), 50);
        assert_eq!(histogram.len(), 50);
        // MACD of uptrend should be positive
        let last_macd = macd_line.last().unwrap();
        assert!(!last_macd.is_nan());
        assert!(*last_macd > 0.0);
    }

    #[test]
    fn bbands_basic() {
        let close: Vec<f64> = (1..=30).map(|x| x as f64).collect();
        let (upper, middle, lower) = bbands(&close, 20, 2.0, 2.0);
        assert_eq!(upper.len(), 30);

        // Check ordering: lower < middle < upper
        for i in 19..30 {
            assert!(
                lower[i] < middle[i] && middle[i] < upper[i],
                "band ordering violated at index {i}"
            );
        }
    }

    #[test]
    fn bbands_constant_price() {
        let close = vec![100.0; 30];
        let (upper, middle, lower) = bbands(&close, 20, 2.0, 2.0);
        // Constant price: std = 0, so upper == middle == lower
        let last = close.len() - 1;
        assert!((upper[last] - 100.0).abs() < 1e-10);
        assert!((middle[last] - 100.0).abs() < 1e-10);
        assert!((lower[last] - 100.0).abs() < 1e-10);
    }

    #[test]
    fn atr_basic() {
        // Simple case: constant range
        let high = vec![102.0; 20];
        let low = vec![98.0; 20];
        let close = vec![100.0; 20];
        let result = atr(&high, &low, &close, 14);

        // True range is always 4.0, so ATR should converge to 4.0
        let last = result.last().unwrap();
        assert!((*last - 4.0).abs() < 0.1, "expected ATR ~4.0, got {last}");
    }

    #[test]
    fn atr_lookback_nan() {
        let high = vec![102.0; 20];
        let low = vec![98.0; 20];
        let close = vec![100.0; 20];
        let result = atr(&high, &low, &close, 14);
        // First 14 elements should be NaN (indices 0..14)
        for (i, v) in result.iter().take(14).enumerate() {
            assert!(v.is_nan(), "expected NaN at index {i}");
        }
        assert!(!result[14].is_nan(), "expected valid ATR at index 14");
    }

    #[test]
    fn empty_input() {
        let empty: Vec<f64> = vec![];
        assert!(rsi(&empty, 14).is_empty());
        let (m, s, h) = macd(&empty, 12, 26, 9);
        assert!(m.is_empty() && s.is_empty() && h.is_empty());
        let (u, mid, l) = bbands(&empty, 20, 2.0, 2.0);
        assert!(u.is_empty() && mid.is_empty() && l.is_empty());
        assert!(atr(&empty, &empty, &empty, 14).is_empty());
    }

    #[test]
    fn insufficient_data() {
        let short = vec![1.0, 2.0, 3.0];
        let result = rsi(&short, 14);
        assert!(result.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn obv_flat_price_unchanged() {
        let close = vec![10.0; 5];
        let volume = vec![100.0, 200.0, 300.0, 400.0, 500.0];
        let result = obv(&close, &volume);
        assert_eq!(result[0], 100.0);
        for i in 1..5 {
            assert_eq!(result[i], result[i - 1]);
        }
    }

    #[test]
    fn ad_zero_range_bar_unchanged() {
        let high = vec![10.0, 11.0];
        let low = vec![10.0, 10.0];
        let close = vec![10.0, 10.5];
        let volume = vec![1000.0, 2000.0];
        let result = ad(&high, &low, &close, &volume);
        assert_eq!(result[0], 0.0);
        assert_eq!(result[1], 0.0);
    }

    #[test]
    fn adosc_lookback_nan() {
        let high: Vec<f64> = (1..=20).map(|x| x as f64 + 1.0).collect();
        let low: Vec<f64> = (1..=20).map(|x| x as f64 - 1.0).collect();
        let close: Vec<f64> = (1..=20).map(|x| x as f64).collect();
        let volume = vec![1000.0; 20];
        let result = adosc(&high, &low, &close, &volume, 3, 10);
        for v in result.iter().take(9) {
            assert!(v.is_nan());
        }
        assert!(!result[9].is_nan());
    }

    #[test]
    fn trange_first_bar_nan() {
        let high = vec![11.0, 12.0, 13.0];
        let low = vec![9.0, 10.0, 11.0];
        let close = vec![10.0, 11.0, 12.0];
        let result = trange(&high, &low, &close);
        assert!(result[0].is_nan());
        assert!(!result[1].is_nan());
        assert!(result[1] > 0.0);
    }

    #[test]
    fn natr_positive_on_synthetic() {
        let high: Vec<f64> = (1..=30).map(|x| x as f64 + 0.5).collect();
        let low: Vec<f64> = (1..=30).map(|x| x as f64 - 0.5).collect();
        let close: Vec<f64> = (1..=30).map(|x| x as f64).collect();
        let result = natr(&high, &low, &close, 14);
        assert!(result.iter().take(14).all(|v| v.is_nan()));
        assert!(result[14] > 0.0);
    }
}
