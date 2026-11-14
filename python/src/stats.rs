use nanobook::stats;
use pyo3::prelude::*;

/// Compute Spearman rank correlation with two-tailed p-value.
///
/// Drop-in replacement for ``scipy.stats.spearmanr(x, y)``.
/// Uses average-rank tie-breaking, matching scipy's default.
///
/// Args:
///     x: First variable (list of floats).
///     y: Second variable (list of floats, same length as x).
///
/// Returns:
///     Tuple of (correlation, p_value). Returns (NaN, NaN) if len < 3.
///
/// Example::
///
///     corr, p = nanobook.py_spearman(scores, returns)
///
#[pyfunction]
pub fn py_spearman(x: Vec<f64>, y: Vec<f64>) -> (f64, f64) {
    stats::spearman(&x, &y)
}

/// Compute quintile spread (top quintile mean - bottom quintile mean).
///
/// Sorts by ``scores``, splits into ``n_quantiles`` groups, returns the
/// difference between the top group's mean return and the bottom group's.
///
/// Args:
///     scores: Factor scores (list of floats).
///     returns: Realized returns (list of floats, same length as scores).
///     n_quantiles: Number of groups (default 5).
///
/// Returns:
///     Float: top_mean - bottom_mean. NaN if inputs are invalid.
///
/// Example::
///
///     spread = nanobook.py_quintile_spread(scores, returns, 5)
///
#[pyfunction]
#[pyo3(signature = (scores, returns, n_quantiles=5))]
pub fn py_quintile_spread(scores: Vec<f64>, returns: Vec<f64>, n_quantiles: usize) -> f64 {
    stats::quintile_spread(&scores, &returns, n_quantiles)
}

/// Compute the Deflated Sharpe Ratio (Bailey and López de Prado, 2014).
///
/// The probability that the true Sharpe ratio is above zero after correcting the
/// observed one for selection across ``n_trials`` strategies and for non-normal
/// returns. With one trial it is the probabilistic Sharpe ratio against zero.
///
/// Args:
///     sharpe: Observed Sharpe ratio, per period (not annualized).
///     n_trials: Number of independent strategy trials (N).
///     n_observations: Number of returns behind ``sharpe`` (T), at least 2.
///     trials_sharpe_variance: Variance of the per-period Sharpe ratios across
///         the trials (V).
///     skewness: Skewness of the returns.
///     kurtosis: Raw kurtosis of the returns (3 for normal returns).
///
/// Returns:
///     Float: probability in [0, 1]. NaN for invalid inputs.
///
/// Example (the paper's worked example, p. 99; returns about 0.9004)::
///
///     dsr = nanobook.py_deflated_sharpe(2.5 / 250**0.5, 100, 1250, 0.5 / 250, -3.0, 10.0)
///
#[pyfunction]
pub fn py_deflated_sharpe(
    sharpe: f64,
    n_trials: usize,
    n_observations: usize,
    trials_sharpe_variance: f64,
    skewness: f64,
    kurtosis: f64,
) -> f64 {
    stats::deflated_sharpe(
        sharpe,
        n_trials,
        n_observations,
        trials_sharpe_variance,
        skewness,
        kurtosis,
    )
}
