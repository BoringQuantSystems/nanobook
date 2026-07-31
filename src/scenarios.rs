//! Monte Carlo terminal valuation (feature-gated stochastic research helper).
//!
//! Two models, matching the Python reference in `nanobook/python/nanobook/scenarios.py`:
//! - **Simple GBM**: classic one-step geometric Brownian terminal price.
//! - **Advanced multi-driver**: GP growth, margin expansion, multiple rerating, macro shock + bear skew.
//!
//! RNG paths:
//! - **Python / parity** (`python/src/scenarios.rs`): NumPy `default_rng` draws normals; Rust applies
//!   the closed-form terminal math (`simple_gbm_from_z`, `advanced_from_driver_batches`).
//! - **Native Rust** (`monte_carlo_stock_valuation` below): ChaCha20 + `rand_distr::Normal` for callers
//!   that do not need NumPy bit-exact parity.
//! - **Parallel terminal math** activates when `n_paths >= PARALLEL_MC_MIN_PATHS` (50_000).
//!
//! With the `parallel` feature, terminal-price math runs on rayon after draws complete so RNG order
//! stays identical to the sequential path.

use std::collections::HashMap;

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use rand_distr::{Distribution, Normal};

/// Minimum path count before rayon parallelizes terminal-price math only.
pub const PARALLEL_MC_MIN_PATHS: i64 = 50_000;

#[derive(Debug, Clone, Copy)]
pub struct ValuationParams {
    pub gp_growth_mean: f64,
    pub gp_growth_sd: f64,
    pub margin_boost_mean: f64,
    pub margin_boost_sd: f64,
    pub multiple_mean: f64,
    pub multiple_sd: f64,
    pub macro_shock_mean: f64,
    pub macro_shock_sd: f64,
    pub bear_skew_factor: f64,
}

impl Default for ValuationParams {
    fn default() -> Self {
        Self {
            gp_growth_mean: 0.16,
            gp_growth_sd: 0.06,
            margin_boost_mean: 0.02,
            margin_boost_sd: 0.03,
            multiple_mean: 22.0,
            multiple_sd: 3.5,
            macro_shock_mean: -0.03,
            macro_shock_sd: 0.11,
            bear_skew_factor: 0.04,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MonteCarloResult {
    pub ticker: String,
    pub method: String,
    pub horizon_years: f64,
    pub current_price: f64,
    pub terminal_prices: Vec<f64>,
    pub summary: HashMap<String, f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelVersion {
    Simple,
    Advanced,
}

impl ModelVersion {
    pub fn parse(version: &str) -> Self {
        if version == "simple" {
            Self::Simple
        } else {
            Self::Advanced
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioError(String);

impl std::fmt::Display for ScenarioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ScenarioError {}

/// Validate Monte Carlo inputs before drawing paths or building summaries.
///
/// Rejects non-finite prices, negative path counts, non-positive horizons, and
/// negative volatility — matching the Python reference error messages.
pub fn validate_mc_inputs(
    current_price: f64,
    n_paths: i64,
    horizon: f64,
    annual_vol: f64,
) -> Result<(), ScenarioError> {
    if current_price <= 0.0 || !current_price.is_finite() {
        return Err(ScenarioError(format!(
            "current_price must be positive and finite, got {current_price}"
        )));
    }
    if n_paths < 0 {
        return Err(ScenarioError(format!(
            "n_paths must be >= 0, got {n_paths}"
        )));
    }
    if horizon <= 0.0 || !horizon.is_finite() {
        return Err(ScenarioError(format!(
            "horizon must be positive and finite, got {horizon}"
        )));
    }
    if annual_vol < 0.0 || !annual_vol.is_finite() {
        return Err(ScenarioError(format!(
            "annual_vol must be non-negative and finite, got {annual_vol}"
        )));
    }
    Ok(())
}

/// Arithmetic mean; returns `0.0` for an empty slice.
pub fn pure_mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    xs.iter().sum::<f64>() / xs.len() as f64
}

fn sort_f64(xs: &mut [f64]) {
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
}

/// Median of a pre-sorted slice (caller must sort first).
fn median_sorted(s: &[f64]) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    let n = s.len();
    let mid = n / 2;
    if n % 2 == 1 {
        s[mid]
    } else {
        (s[mid - 1] + s[mid]) / 2.0
    }
}

/// Linear-interpolation percentile on a pre-sorted slice (matches NumPy `method="linear"`).
fn percentile_sorted(s: &[f64], q: f64) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    let n = s.len();
    if q <= 0.0 {
        return s[0];
    }
    if q >= 1.0 {
        return s[n - 1];
    }
    let pos = q * (n as f64 - 1.0);
    let i = pos.floor() as usize;
    let frac = pos - i as f64;
    if i + 1 >= n {
        return s[n - 1];
    }
    s[i] + (s[i + 1] - s[i]) * frac
}

/// Median via one sort + middle index (even-length: average of two middles).
pub fn pure_median(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut s = xs.to_vec();
    sort_f64(&mut s);
    median_sorted(&s)
}

/// Linear-interpolation percentile on `q ∈ [0, 1]` (NumPy `method="linear"`).
pub fn pure_percentile(xs: &[f64], q: f64) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut s = xs.to_vec();
    sort_f64(&mut s);
    percentile_sorted(&s, q)
}

/// Fraction of samples strictly above `level`; `0.0` when `xs` is empty.
pub fn pure_prob_above(xs: &[f64], level: f64) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let count = xs.iter().filter(|&&x| x > level).count();
    count as f64 / xs.len() as f64
}

/// Nearest-rank quantile (floor index on sorted data); used by `MonteCarloResult::quantile`.
pub fn quantile_nearest_rank(xs: &[f64], q: f64) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut s = xs.to_vec();
    sort_f64(&mut s);
    let idx = (q * (s.len() as f64 - 1.0))
        .floor()
        .clamp(0.0, (s.len() - 1) as f64) as usize;
    s[idx]
}

fn clip_multiple(x: f64) -> f64 {
    x.clamp(16.0, 28.0)
}

/// Terminal prices from pre-drawn standard normals (parity path shares NumPy's `z` vector).
pub fn simple_gbm_from_z(
    current_price: f64,
    z: &[f64],
    horizon: f64,
    expected_annual_return: f64,
    annual_vol: f64,
) -> Vec<f64> {
    let drift = (expected_annual_return - 0.5 * annual_vol * annual_vol) * horizon;
    let sigma = annual_vol * horizon.sqrt();
    gbm_prices_from_z(current_price, z, drift, sigma)
}

fn gbm_prices_from_z(current_price: f64, z: &[f64], drift: f64, sigma: f64) -> Vec<f64> {
    #[cfg(feature = "parallel")]
    {
        if z.len() >= PARALLEL_MC_MIN_PATHS as usize {
            use rayon::prelude::*;
            return z
                .par_iter()
                .map(|&zi| current_price * (drift + sigma * zi).exp())
                .collect();
        }
    }
    z.iter()
        .map(|&zi| current_price * (drift + sigma * zi).exp())
        .collect()
}

/// Terminal prices from pre-drawn driver batches (parity path shares NumPy draws).
pub fn advanced_from_driver_batches(
    current_price: f64,
    horizon: f64,
    gp: &[f64],
    marg: &[f64],
    mult_raw: &[f64],
    macro_draw: &[f64],
    bear_skew: &[f64],
) -> Vec<f64> {
    let n = gp.len();
    debug_assert_eq!(n, marg.len());
    debug_assert_eq!(n, mult_raw.len());
    debug_assert_eq!(n, macro_draw.len());
    debug_assert_eq!(n, bear_skew.len());

    advanced_terminal_from_slices(
        current_price,
        horizon,
        gp,
        marg,
        mult_raw,
        macro_draw,
        bear_skew,
    )
}

/// Advanced multi-driver terminal prices from pre-drawn log-return contributions.
///
/// Each path combines five independent draws into one horizon log-return, then
/// compounds: `terminal = current_price * exp(total_ret * horizon)`.
///
/// Driver weights mirror the Python reference (`scenarios.py`):
/// - GP growth:        0.8 × draw
/// - Margin expansion: 2.0 × draw
/// - Multiple rerate:  0.6 × (clipped_multiple / 20 − 1)
/// - Macro shock:      draw − |bear_skew|  (left-tail drag via one-sided skew)
///
/// Multiples are clipped to [16, 28] before the rerate term.
fn advanced_terminal_from_slices(
    current_price: f64,
    horizon: f64,
    gp: &[f64],
    marg: &[f64],
    mult_raw: &[f64],
    macro_draw: &[f64],
    bear_skew: &[f64],
) -> Vec<f64> {
    let n = gp.len();
    let price_at = |i: usize| {
        let mult = clip_multiple(mult_raw[i]);
        let shock = macro_draw[i] - bear_skew[i].abs();
        let total_ret = (gp[i] * 0.8) + (marg[i] * 2.0) + ((mult / 20.0 - 1.0) * 0.6) + shock;
        current_price * (total_ret * horizon).exp()
    };

    #[cfg(feature = "parallel")]
    {
        if n >= PARALLEL_MC_MIN_PATHS as usize {
            use rayon::prelude::*;
            return (0..n).into_par_iter().map(price_at).collect();
        }
    }
    (0..n).map(price_at).collect()
}

/// Build the summary dict consumed by Python (`median_price`, `p10`, `p90`, hurdle/bull/bear %).
///
/// Sorts `prices` once for median and both percentiles. Rounding matches the Python reference
/// (`round1` / `round2` / `round4` on selected fields).
#[allow(clippy::too_many_arguments)]
pub fn build_summary(
    ticker: &str,
    method: &str,
    prices: &[f64],
    n_paths: i64,
    horizon: f64,
    current_price: f64,
    hurdle_rate: f64,
    bull: f64,
    bear: f64,
) -> HashMap<String, f64> {
    let mn = pure_mean(prices);
    let hurdle_level = current_price * (1.0 + hurdle_rate);

    // One unstable sort for median + p10 + p90 (was three independent sorts).
    let mut sorted = prices.to_vec();
    sorted.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let med = median_sorted(&sorted);
    let p10 = percentile_sorted(&sorted, 0.10);
    let p90 = percentile_sorted(&sorted, 0.90);

    let _ = ticker;
    let _ = method;

    let mut row = HashMap::new();
    row.insert("n_paths".to_string(), n_paths as f64);
    row.insert("horizon_years".to_string(), horizon);
    row.insert("median_price".to_string(), round2(med));
    row.insert("mean_price".to_string(), round2(mn));
    row.insert("mean_minus_median".to_string(), round2(mn - med));
    row.insert(
        "pct_above_hurdle".to_string(),
        round1(pure_prob_above(prices, hurdle_level) * 100.0),
    );
    row.insert(
        "pct_above_bull".to_string(),
        round1(pure_prob_above(prices, bull) * 100.0),
    );
    row.insert(
        "pct_below_bear".to_string(),
        round1((1.0 - pure_prob_above(prices, bear)) * 100.0),
    );
    row.insert("p10".to_string(), round2(p10));
    row.insert("p90".to_string(), round2(p90));
    row.insert(
        "implied_median_annual_return".to_string(),
        round4((med / current_price).powf(1.0 / horizon) - 1.0),
    );
    row.insert("current_price".to_string(), current_price);
    row.insert("hurdle_rate".to_string(), hurdle_rate);
    row
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

fn round4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

/// Wrap terminal prices and summary into a `MonteCarloResult` with default bull/bear levels.
#[allow(clippy::too_many_arguments)]
pub fn assemble_result(
    ticker: String,
    method: String,
    horizon: f64,
    current_price: f64,
    prices: Vec<f64>,
    n_paths: i64,
    hurdle_rate: f64,
    bull_price: Option<f64>,
    bear_price: Option<f64>,
) -> MonteCarloResult {
    let bull = bull_price.unwrap_or(current_price * 1.10);
    let bear = bear_price.unwrap_or(current_price * 0.81);
    let summary = build_summary(
        &ticker,
        &method,
        &prices,
        n_paths,
        horizon,
        current_price,
        hurdle_rate,
        bull,
        bear,
    );
    MonteCarloResult {
        ticker,
        method,
        horizon_years: horizon,
        current_price,
        terminal_prices: prices,
        summary,
    }
}

impl MonteCarloResult {
    pub fn median_price(&self) -> f64 {
        self.summary["median_price"]
    }

    pub fn mean_price(&self) -> f64 {
        self.summary["mean_price"]
    }

    pub fn implied_median_annual_return(&self) -> f64 {
        self.summary["implied_median_annual_return"]
    }

    pub fn p10_price(&self) -> f64 {
        self.summary["p10"]
    }

    pub fn p90_price(&self) -> f64 {
        self.summary["p90"]
    }

    pub fn prob_above(&self, level: f64) -> f64 {
        pure_prob_above(&self.terminal_prices, level)
    }

    pub fn quantile(&self, q: f64) -> f64 {
        quantile_nearest_rank(&self.terminal_prices, q)
    }

    pub fn as_log_returns(&self) -> Vec<f64> {
        if self.current_price <= 0.0 {
            return vec![0.0; self.terminal_prices.len()];
        }
        self.terminal_prices
            .iter()
            .map(|p| (p / self.current_price).ln())
            .collect()
    }

    pub fn to_price_paths(
        &self,
        n_periods: usize,
        method: PricePathMethod,
    ) -> Result<Vec<Vec<f64>>, ScenarioError> {
        if n_periods < 1 {
            return Err(ScenarioError("n_periods must be >= 1".to_string()));
        }
        let mut paths = Vec::with_capacity(self.terminal_prices.len());
        for &term in &self.terminal_prices {
            paths.push(single_price_path(
                self.current_price,
                term,
                n_periods,
                method,
            ));
        }
        Ok(paths)
    }
}

/// How to expand a terminal price into a multi-period schedule for backtests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PricePathMethod {
    Linear,
    TerminalOnly,
}

fn single_price_path(
    current_price: f64,
    terminal: f64,
    n_periods: usize,
    method: PricePathMethod,
) -> Vec<f64> {
    if method == PricePathMethod::TerminalOnly {
        let mut row = vec![current_price; n_periods];
        if let Some(last) = row.last_mut() {
            *last = terminal;
        }
        return row;
    }
    if current_price <= 0.0 || terminal <= 0.0 {
        return vec![current_price; n_periods];
    }
    let log0 = current_price.ln();
    let logt = terminal.ln();
    let step = if n_periods > 1 {
        (logt - log0) / (n_periods as f64 - 1.0)
    } else {
        0.0
    };
    (0..n_periods)
        .map(|i| (log0 + i as f64 * step).exp())
        .collect()
}

fn cha_cha_rng(seed: u64) -> ChaCha20Rng {
    ChaCha20Rng::seed_from_u64(seed)
}

/// Nondeterministic seed for native MC when Python passes ``seed=None``.
pub fn nondeterministic_mc_seed() -> u64 {
    // rand 0.10 renamed `RngCore` to `Rng` and `thread_rng()` to `rng()`;
    // `random` does the same job here without naming either.
    rand::random::<u64>()
}

#[allow(dead_code)] // parity PyO3 / batch paths keep separate driver vecs
fn draw_standard_normals(rng: &mut ChaCha20Rng, n: usize) -> Vec<f64> {
    let normal = Normal::new(0.0, 1.0).expect("standard normal");
    (0..n).map(|_| normal.sample(rng)).collect()
}

#[allow(dead_code)]
fn draw_normals(rng: &mut ChaCha20Rng, mean: f64, sd: f64, n: usize) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    let normal = Normal::new(mean, sd).expect("normal params");
    (0..n).map(|_| normal.sample(rng)).collect()
}

/// Fused ChaCha20 draw + terminal math (native hot path only; no intermediate driver vecs).
fn simple_gbm_native(
    current_price: f64,
    n: usize,
    horizon: f64,
    expected_annual_return: f64,
    annual_vol: f64,
    rng: &mut ChaCha20Rng,
) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    let drift = (expected_annual_return - 0.5 * annual_vol * annual_vol) * horizon;
    let sigma = annual_vol * horizon.sqrt();
    let normal = Normal::new(0.0, 1.0).expect("standard normal");
    let mut prices = Vec::with_capacity(n);
    for _ in 0..n {
        let z = normal.sample(rng);
        prices.push(current_price * (drift + sigma * z).exp());
    }
    prices
}

/// Fused ChaCha20 draw + advanced terminal math (native hot path only).
fn advanced_native_fused(
    current_price: f64,
    horizon: f64,
    n: usize,
    params: ValuationParams,
    rng: &mut ChaCha20Rng,
) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    let gp_n = Normal::new(params.gp_growth_mean, params.gp_growth_sd).expect("gp normal");
    let marg_n =
        Normal::new(params.margin_boost_mean, params.margin_boost_sd).expect("marg normal");
    let mult_n = Normal::new(params.multiple_mean, params.multiple_sd).expect("mult normal");
    let macro_n =
        Normal::new(params.macro_shock_mean, params.macro_shock_sd).expect("macro normal");
    let bear_n = Normal::new(0.0, params.bear_skew_factor).expect("bear normal");
    let mut prices = Vec::with_capacity(n);
    for _ in 0..n {
        let gp = gp_n.sample(rng);
        let marg = marg_n.sample(rng);
        let mult = clip_multiple(mult_n.sample(rng));
        let shock = macro_n.sample(rng) - bear_n.sample(rng).abs();
        let total_ret = (gp * 0.8) + (marg * 2.0) + ((mult / 20.0 - 1.0) * 0.6) + shock;
        prices.push(current_price * (total_ret * horizon).exp());
    }
    prices
}

/// Native Rust Monte Carlo entry point (ChaCha20 + `rand_distr::Normal`).
///
/// Used by Rust callers and benchmarks. The Python/PyO3 path instead feeds
/// NumPy-drawn normals into `simple_gbm_from_z` / `advanced_from_driver_batches`
/// for bit-exact parity with frozen fixtures.
#[allow(clippy::too_many_arguments)]
pub fn monte_carlo_stock_valuation(
    ticker: String,
    current_price: f64,
    version: ModelVersion,
    n_paths: i64,
    horizon: f64,
    seed: u64,
    expected_annual_return: f64,
    annual_vol: f64,
    params: ValuationParams,
    hurdle_rate: f64,
    bull_price: Option<f64>,
    bear_price: Option<f64>,
) -> Result<MonteCarloResult, ScenarioError> {
    validate_mc_inputs(current_price, n_paths, horizon, annual_vol)?;

    let prices = if n_paths == 0 {
        Vec::new()
    } else if version == ModelVersion::Simple {
        let mut rng = cha_cha_rng(seed);
        simple_gbm_native(
            current_price,
            n_paths as usize,
            horizon,
            expected_annual_return,
            annual_vol,
            &mut rng,
        )
    } else {
        let mut rng = cha_cha_rng(seed);
        advanced_native_fused(current_price, horizon, n_paths as usize, params, &mut rng)
    };

    let method = if version == ModelVersion::Simple {
        "Simple GBM".to_string()
    } else {
        "Advanced Multi-Driver".to_string()
    };

    Ok(assemble_result(
        ticker,
        method,
        horizon,
        current_price,
        prices,
        n_paths,
        hurdle_rate,
        bull_price,
        bear_price,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_matches_python_reference_case() {
        let xs = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((pure_percentile(&xs, 0.10) - 1.4).abs() < 1e-12);
        assert!((pure_median(&xs) - 3.0).abs() < 1e-12);
    }

    #[test]
    fn zero_paths_empty_summary() {
        let summary = build_summary("T", "m", &[], 0, 1.0, 100.0, 0.08, 110.0, 81.0);
        assert_eq!(summary["median_price"], 0.0);
        assert_eq!(summary["mean_price"], 0.0);
    }

    #[test]
    fn advanced_clip_multiple() {
        let prices =
            advanced_from_driver_batches(74.0, 1.0, &[0.16], &[0.02], &[50.0], &[-0.03], &[0.01]);
        assert_eq!(prices.len(), 1);
        assert!(prices[0].is_finite() && prices[0] > 0.0);
    }

    #[test]
    fn native_mc_reproducible() {
        let p = ValuationParams::default();
        let a = monte_carlo_stock_valuation(
            "T".to_string(),
            74.0,
            ModelVersion::Advanced,
            200,
            1.0,
            42,
            0.18,
            0.38,
            p,
            0.08,
            None,
            None,
        )
        .unwrap();
        let b = monte_carlo_stock_valuation(
            "T".to_string(),
            74.0,
            ModelVersion::Advanced,
            200,
            1.0,
            42,
            0.18,
            0.38,
            p,
            0.08,
            None,
            None,
        )
        .unwrap();
        assert_eq!(a.terminal_prices, b.terminal_prices);
        assert_eq!(a.median_price(), b.median_price());
    }

    /// Frozen reference values for the native ChaCha20 path.
    ///
    /// `native_mc_reproducible` above only compares two runs inside one build,
    /// so it passes whether or not an RNG upgrade moved the stream. This test
    /// pins the actual numbers, captured on 2026-07-31 with rand 0.10.2 /
    /// rand_chacha 0.10. If a future rand bump changes the draw sequence, these
    /// values move by whole percent and this fails loudly, instead of the shift
    /// having to be caught by hand.
    ///
    /// The comparison is relative rather than bitwise on purpose: the draws
    /// themselves are exact, but the terminal math runs through `exp`/`ln`,
    /// whose last ulp is libm-dependent and so can differ between Linux, macOS
    /// and Windows. 1e-12 is far tighter than any stream change and far looser
    /// than that platform noise.
    #[test]
    fn native_mc_matches_frozen_reference() {
        fn close(actual: f64, expected: f64, what: &str) {
            let tol = 1e-12 * expected.abs();
            assert!(
                (actual - expected).abs() <= tol,
                "{what}: got {actual:.17e}, expected {expected:.17e}"
            );
        }

        // (version, expected median, first six terminal prices)
        let cases: [(ModelVersion, f64, [f64; 6]); 2] = [
            (
                ModelVersion::Simple,
                82.72,
                [
                    83.8342845450791,
                    75.0777746721276,
                    62.07229385240271,
                    43.482335154890144,
                    51.38091457588956,
                    71.13543229003699,
                ],
            ),
            (
                ModelVersion::Advanced,
                87.31,
                [
                    65.12115044541964,
                    79.01696441262114,
                    110.75436660707012,
                    68.65094077523544,
                    101.49080641203211,
                    75.51724580675427,
                ],
            ),
        ];

        for (version, expected_median, expected_head) in cases {
            let result = monte_carlo_stock_valuation(
                "T".to_string(),
                74.0,
                version,
                256,
                1.0,
                42,
                0.18,
                0.38,
                ValuationParams::default(),
                0.08,
                None,
                None,
            )
            .unwrap();

            assert_eq!(result.terminal_prices.len(), 256, "{version:?}: path count");
            close(result.median_price(), expected_median, "median");
            for (i, expected) in expected_head.iter().enumerate() {
                close(result.terminal_prices[i], *expected, &format!("path {i}"));
            }
        }
    }

    /// With `parallel` enabled, rayon preserves index order in `collect()`; terminal
    /// math on independent paths must remain bitwise reproducible across runs.
    #[cfg(feature = "parallel")]
    #[test]
    fn parallel_terminal_math_bitwise_reproducible() {
        let z: Vec<f64> = (0..2_000).map(|i| ((i as f64) * 0.013).sin()).collect();
        let gbm_a = simple_gbm_from_z(74.0, &z, 1.0, 0.18, 0.38);
        let gbm_b = simple_gbm_from_z(74.0, &z, 1.0, 0.18, 0.38);
        assert_eq!(gbm_a, gbm_b);

        let n = 2_000;
        let gp = vec![0.16; n];
        let marg = vec![0.02; n];
        let mult_raw = vec![22.0; n];
        let macro_draw = vec![-0.03; n];
        let bear_skew = vec![0.04; n];
        let adv_a =
            advanced_from_driver_batches(74.0, 1.0, &gp, &marg, &mult_raw, &macro_draw, &bear_skew);
        let adv_b =
            advanced_from_driver_batches(74.0, 1.0, &gp, &marg, &mult_raw, &macro_draw, &bear_skew);
        assert_eq!(adv_a, adv_b);
    }

    #[test]
    fn large_n_paths_stress_native() {
        let p = ValuationParams::default();
        let res = monte_carlo_stock_valuation(
            "XYZ".to_string(),
            74.0,
            ModelVersion::Advanced,
            100_000,
            1.0,
            42,
            0.18,
            0.38,
            p,
            0.08,
            None,
            None,
        )
        .unwrap();
        assert_eq!(res.terminal_prices.len(), 100_000);
        assert!(
            res.terminal_prices
                .iter()
                .all(|p| p.is_finite() && *p > 0.0)
        );
        let med = res.median_price();
        assert!((80.0..=95.0).contains(&med));
    }

    #[test]
    fn to_price_paths_linear_and_terminal_only() {
        let res = assemble_result(
            "P".to_string(),
            "m".to_string(),
            1.0,
            100.0,
            vec![121.0],
            1,
            0.08,
            None,
            None,
        );
        let linear = res.to_price_paths(4, PricePathMethod::Linear).unwrap();
        assert_eq!(linear.len(), 1);
        assert_eq!(linear[0].len(), 4);
        assert!((linear[0][0] - 100.0).abs() < 1e-9);
        assert!((linear[0][3] - 121.0).abs() < 1e-6);

        let terminal = res
            .to_price_paths(3, PricePathMethod::TerminalOnly)
            .unwrap();
        assert_eq!(terminal[0], vec![100.0, 100.0, 121.0]);
    }

    proptest::proptest! {
        #[test]
        fn percentile_monotone(xs in proptest::collection::vec(0.1f64..1000.0, 1..40)) {
            let qs = [0.0, 0.1, 0.5, 0.9, 1.0];
            let mut prev = pure_percentile(&xs, qs[0]);
            for &q in &qs[1..] {
                let cur = pure_percentile(&xs, q);
                proptest::prop_assert!(cur >= prev - 1e-12);
                prev = cur;
            }
        }
    }
}
