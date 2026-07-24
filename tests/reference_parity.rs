//! Reference-parity tests against pinned scipy / TA-Lib / quantstats
//! outputs.
//!
//! The golden fixture at `tests/parity/golden.json` is generated
//! manually by running `tests/parity/generate_golden.py` under the
//! versions pinned in `tests/parity/requirements.txt`. CI only reads
//! the fixture — it does not regenerate.
//!
//! Per-function tolerances are documented per test. Do NOT loosen a
//! tolerance to make a test pass: either the reference convention
//! differs (document it, pick a different reference) or nanobook has a
//! bug to fix.
//!
//! See `tests/parity/README.md` for the full drift policy.
//!
//! This module ships with the v0.10 "Hardening Release" as the
//! measurement substrate for every numerical fix. Per-function
//! reference comparisons live here; pure regression tests for
//! specific bugs (e.g., catastrophic cancellation) live in their own
//! test files alongside the fix that introduces them.
//!
//! Tests in this file:
//!
//! - `rsi_matches_talib`               — initial scaffolding (N10).
//! - `atr_matches_talib`               — initial scaffolding (N10).
//! - `sharpe_matches_quantstats`       — initial scaffolding (N10).
//! - `max_drawdown_matches_quantstats` — initial scaffolding (N10).
//! - `cvar_historical_matches_empirical`  — added by N2 (default method).
//! - `cvar_parametric_matches_quantstats` — added by N2 (legacy method).
//! - `sortino_matches_quantstats`         — added by N4 (ddof=0 default).
//! - `sortino_ddof1_matches_scaled_ddof0` — added by N4 (legacy path).
//!
//! Related regression tests in other files:
//!
//! - `tests/catastrophic_cancellation.rs` — Welford rolling variance
//!   (N1). Separate from this file because it has no scipy/talib/qs
//!   reference; it asserts the output is not collapsed to zero on
//!   pathological input.

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;

// --- Registry (shared with generate_golden.py) --------------------------------

#[derive(Debug, Deserialize)]
struct RegistryFile {
    indicators: Vec<RegistryEntry>,
}

#[derive(Debug, Deserialize)]
struct RegistryEntry {
    golden_key: Option<String>,
    golden_keys: Option<Vec<String>>,
    name: String,
    #[allow(dead_code)]
    talib_func: String,
    #[allow(dead_code)]
    input_type: String,
    rust_fn: String,
    rust_args: Vec<serde_json::Value>,
    tol: f64,
}

fn registry() -> RegistryFile {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "tests",
        "parity",
        "indicator_registry.json",
    ]
    .iter()
    .collect();
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "failed to read {}: {e}\n\
             Add entries to indicator_registry.json (see tests/parity/README.md)",
            path.display()
        )
    });
    serde_json::from_str(&raw).expect("indicator_registry.json is not valid JSON")
}

fn golden_keys(entry: &RegistryEntry) -> Vec<String> {
    if let Some(keys) = &entry.golden_keys {
        keys.clone()
    } else {
        vec![
            entry
                .golden_key
                .clone()
                .expect("golden_key or golden_keys required"),
        ]
    }
}

fn usize_arg(v: &serde_json::Value, label: &str) -> usize {
    v.as_u64()
        .or_else(|| v.as_f64().map(|f| f as u64))
        .expect(label) as usize
}

fn f64_arg(v: &serde_json::Value, label: &str) -> f64 {
    v.as_f64().expect(label)
}

fn run_indicator(
    entry: &RegistryEntry,
    close: &[f64],
    highs: &[f64],
    lows: &[f64],
    volume: &[f64],
) -> Vec<Vec<f64>> {
    match entry.rust_fn.as_str() {
        "sma" => {
            let period = usize_arg(&entry.rust_args[0], "sma period");
            vec![nanobook::indicators::sma(close, period)]
        }
        "ema" => {
            let period = usize_arg(&entry.rust_args[0], "ema period");
            vec![nanobook::indicators::ema(close, period)]
        }
        "rsi" => {
            let period = usize_arg(&entry.rust_args[0], "rsi period");
            vec![nanobook::indicators::rsi(close, period)]
        }
        "macd" => {
            let fast = usize_arg(&entry.rust_args[0], "macd fast");
            let slow = usize_arg(&entry.rust_args[1], "macd slow");
            let signal = usize_arg(&entry.rust_args[2], "macd signal");
            let (macd, sig, hist) = nanobook::indicators::macd(close, fast, slow, signal);
            vec![macd, sig, hist]
        }
        "bbands" => {
            let period = usize_arg(&entry.rust_args[0], "bbands period");
            let up = f64_arg(&entry.rust_args[1], "bbands nbdevup");
            let dn = f64_arg(&entry.rust_args[2], "bbands nbdevdn");
            let (upper, middle, lower) = nanobook::indicators::bbands(close, period, up, dn);
            vec![upper, middle, lower]
        }
        "atr" => {
            let period = usize_arg(&entry.rust_args[0], "atr period");
            vec![nanobook::indicators::atr(highs, lows, close, period)]
        }
        "stoch" => {
            let fk = usize_arg(&entry.rust_args[0], "stoch fastk");
            let sk = usize_arg(&entry.rust_args[1], "stoch slowk");
            let sd = usize_arg(&entry.rust_args[2], "stoch slowd");
            let (k, d) = nanobook::indicators::stoch(highs, lows, close, fk, sk, sd);
            vec![k, d]
        }
        "stochf" => {
            let fk = usize_arg(&entry.rust_args[0], "stochf fastk");
            let fd = usize_arg(&entry.rust_args[1], "stochf fastd");
            let (k, d) = nanobook::indicators::stochf(highs, lows, close, fk, fd);
            vec![k, d]
        }
        "stochrsi" => {
            let tp = usize_arg(&entry.rust_args[0], "stochrsi timeperiod");
            let fk = usize_arg(&entry.rust_args[1], "stochrsi fastk");
            let fd = usize_arg(&entry.rust_args[2], "stochrsi fastd");
            let (k, d) = nanobook::indicators::stochrsi(close, tp, fk, fd);
            vec![k, d]
        }
        "plus_di" => {
            let period = usize_arg(&entry.rust_args[0], "plus_di period");
            vec![nanobook::indicators::plus_di(highs, lows, close, period)]
        }
        "minus_di" => {
            let period = usize_arg(&entry.rust_args[0], "minus_di period");
            vec![nanobook::indicators::minus_di(highs, lows, close, period)]
        }
        "dx" => {
            let period = usize_arg(&entry.rust_args[0], "dx period");
            vec![nanobook::indicators::dx(highs, lows, close, period)]
        }
        "adx" => {
            let period = usize_arg(&entry.rust_args[0], "adx period");
            vec![nanobook::indicators::adx(highs, lows, close, period)]
        }
        "cci" => {
            let period = usize_arg(&entry.rust_args[0], "cci period");
            vec![nanobook::indicators::cci(highs, lows, close, period)]
        }
        "willr" => {
            let period = usize_arg(&entry.rust_args[0], "willr period");
            vec![nanobook::indicators::willr(highs, lows, close, period)]
        }
        "ultosc" => {
            let p1 = usize_arg(&entry.rust_args[0], "ultosc period1");
            let p2 = usize_arg(&entry.rust_args[1], "ultosc period2");
            let p3 = usize_arg(&entry.rust_args[2], "ultosc period3");
            vec![nanobook::indicators::ultosc(highs, lows, close, p1, p2, p3)]
        }
        "mom" => {
            let period = usize_arg(&entry.rust_args[0], "mom period");
            vec![nanobook::indicators::mom(close, period)]
        }
        "roc" => {
            let period = usize_arg(&entry.rust_args[0], "roc period");
            vec![nanobook::indicators::roc(close, period)]
        }
        "rocp" => {
            let period = usize_arg(&entry.rust_args[0], "rocp period");
            vec![nanobook::indicators::rocp(close, period)]
        }
        "rocr" => {
            let period = usize_arg(&entry.rust_args[0], "rocr period");
            vec![nanobook::indicators::rocr(close, period)]
        }
        "obv" => vec![nanobook::indicators::obv(close, volume)],
        "ad" => vec![nanobook::indicators::ad(highs, lows, close, volume)],
        "adosc" => {
            let fast = usize_arg(&entry.rust_args[0], "adosc fast");
            let slow = usize_arg(&entry.rust_args[1], "adosc slow");
            vec![nanobook::indicators::adosc(
                highs, lows, close, volume, fast, slow,
            )]
        }
        "natr" => {
            let period = usize_arg(&entry.rust_args[0], "natr period");
            vec![nanobook::indicators::natr(highs, lows, close, period)]
        }
        "trange" => vec![nanobook::indicators::trange(highs, lows, close)],
        other => panic!("unknown rust_fn in registry: {other}"),
    }
}

// --- Fixture loader --------------------------------------------------------

fn golden() -> Value {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "parity", "golden.json"]
        .iter()
        .collect();
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "failed to read {}: {e}\n\
             Regenerate with `uv run python tests/parity/generate_golden.py` \
             (see tests/parity/README.md)",
            path.display()
        )
    });
    serde_json::from_str(&raw).expect("golden.json is not valid JSON")
}

/// Extract a `Vec<f64>` from a JSON array of numbers. Panics if the
/// path is missing or contains a non-finite value (use `f64_nullable`
/// for indicator outputs with leading NaN).
fn f64_vec(g: &Value, path: &[&str]) -> Vec<f64> {
    let mut cur = g;
    for key in path {
        cur = cur
            .get(*key)
            .unwrap_or_else(|| panic!("golden.json missing path: {}", path.join(".")));
    }
    cur.as_array()
        .expect("not an array")
        .iter()
        .map(|v| v.as_f64().expect("non-numeric entry"))
        .collect()
}

/// Extract a `Vec<Option<f64>>` from a JSON array where `null`
/// represents NaN. Used for TA-Lib indicator outputs (first `period`
/// entries are `null`).
fn f64_nullable(g: &Value, path: &[&str]) -> Vec<Option<f64>> {
    let mut cur = g;
    for key in path {
        cur = cur
            .get(*key)
            .unwrap_or_else(|| panic!("golden.json missing path: {}", path.join(".")));
    }
    cur.as_array()
        .expect("not an array")
        .iter()
        .map(|v| {
            if v.is_null() {
                None
            } else {
                Some(v.as_f64().expect("non-numeric entry"))
            }
        })
        .collect()
}

fn f64_scalar(g: &Value, path: &[&str]) -> f64 {
    let mut cur = g;
    for key in path {
        cur = cur
            .get(*key)
            .unwrap_or_else(|| panic!("golden.json missing path: {}", path.join(".")));
    }
    cur.as_f64().expect("not a number")
}

// --- Helpers ---------------------------------------------------------------

/// Assert that two `Vec<Option<f64>>` sequences align index-for-index:
/// `None` in the reference must correspond to `NaN` in nanobook's
/// output (and vice versa), and finite values must agree within
/// `tol`.
#[track_caller]
fn assert_indicator_parity(ours: &[f64], theirs: &[Option<f64>], tol: f64, label: &str) {
    assert_eq!(
        ours.len(),
        theirs.len(),
        "{label}: length mismatch ({} vs {})",
        ours.len(),
        theirs.len()
    );
    let mut max_diff = 0.0_f64;
    let mut max_diff_idx = usize::MAX;
    for (i, (o, t)) in ours.iter().zip(theirs.iter()).enumerate() {
        match (o.is_nan(), t) {
            (true, None) => {}
            (false, Some(tv)) => {
                let diff = (o - tv).abs();
                if diff > max_diff {
                    max_diff = diff;
                    max_diff_idx = i;
                }
                assert!(
                    diff <= tol,
                    "{label}[{i}]: ours={o}, reference={tv}, diff={diff} > tol={tol}"
                );
            }
            (true, Some(tv)) => panic!(
                "{label}[{i}]: ours=NaN, reference={tv} (nanobook NaN where reference is finite)"
            ),
            (false, None) => panic!(
                "{label}[{i}]: ours={o}, reference=NaN (nanobook finite where reference is NaN)"
            ),
        }
    }
    eprintln!("{label}: max_diff={max_diff:.3e} at index {max_diff_idx} (tol={tol:.3e})");
}

// --- Scaffolding / integrity tests -----------------------------------------

#[test]
fn golden_fixture_loads() {
    let g = golden();
    // _meta.seed and _meta.n are load-bearing — any regeneration must
    // preserve them.
    assert_eq!(g["_meta"]["seed"].as_i64(), Some(42));
    assert_eq!(g["_meta"]["n"].as_i64(), Some(500));
}

#[test]
fn input_series_have_expected_length() {
    let g = golden();
    for field in ["returns", "close", "highs", "lows", "volume"] {
        let v = f64_vec(&g, &["inputs", field]);
        assert_eq!(v.len(), 500, "inputs.{field} wrong length");
    }
}

// --- TA-Lib parity: indicators (registry-driven) ---------------------------

/// Every registry entry must have a matching golden key and pass parity.
#[test]
fn talib_registry_matches_golden() {
    let g = golden();
    let close = f64_vec(&g, &["inputs", "close"]);
    let highs = f64_vec(&g, &["inputs", "highs"]);
    let lows = f64_vec(&g, &["inputs", "lows"]);
    let volume = f64_vec(&g, &["inputs", "volume"]);
    let reg = registry();

    assert!(
        !reg.indicators.is_empty(),
        "indicator_registry.json has no entries"
    );

    for entry in &reg.indicators {
        let keys = golden_keys(entry);
        let ours = run_indicator(entry, &close, &highs, &lows, &volume);
        assert_eq!(
            ours.len(),
            keys.len(),
            "{}: {} rust outputs vs {} golden keys",
            entry.name,
            ours.len(),
            keys.len()
        );

        for (key, series) in keys.iter().zip(ours.iter()) {
            let expected = f64_nullable(&g, &["talib", key]);
            assert_indicator_parity(series, &expected, entry.tol, key);
            let first_valid = expected
                .iter()
                .position(|v| v.is_some())
                .unwrap_or(usize::MAX);
            eprintln!(
                "checking {key}: first_valid_index={first_valid}, tol={:.3e}",
                entry.tol
            );
        }
    }
}

/// Golden talib keys must all be accounted for in the registry.
#[test]
fn talib_golden_keys_known() {
    let g = golden();
    let reg = registry();
    let known: std::collections::HashSet<String> =
        reg.indicators.iter().flat_map(golden_keys).collect();

    let talib_obj = g
        .get("talib")
        .and_then(|v| v.as_object())
        .expect("golden.json missing talib section");

    for key in talib_obj.keys() {
        assert!(
            known.contains(key),
            "golden talib key {key:?} has no registry entry"
        );
    }
}

// --- quantstats parity: portfolio metrics ----------------------------------

/// Annualized Sharpe (252 periods/year, rf=0) on the synthetic return
/// series must agree with quantstats.
///
/// Tolerance: 1e-9 — Sharpe is a closed-form ratio of sums, no
/// iteration or smoothing.
#[test]
fn sharpe_matches_quantstats() {
    let g = golden();
    let returns = f64_vec(&g, &["inputs", "returns"]);
    let expected = f64_scalar(&g, &["quantstats", "sharpe_annual_252"]);

    let metrics = nanobook::portfolio::metrics::compute_metrics(&returns, 252.0, 0.0)
        .expect("non-empty return series");
    let ours = metrics.sharpe;

    let diff = (ours - expected).abs();
    assert!(
        diff <= 1e-9,
        "sharpe: ours={ours}, quantstats={expected}, diff={diff}"
    );
}

/// Maximum drawdown on the synthetic return series must agree with
/// quantstats up to a sign convention.
///
/// Nanobook returns a positive fraction (0.20 = 20% drawdown);
/// quantstats returns a signed value (-0.20). Compare magnitudes.
///
/// Tolerance: 1e-9.
#[test]
fn max_drawdown_matches_quantstats() {
    let g = golden();
    let returns = f64_vec(&g, &["inputs", "returns"]);
    let expected = f64_scalar(&g, &["quantstats", "max_drawdown"]);

    let metrics = nanobook::portfolio::metrics::compute_metrics(&returns, 252.0, 0.0)
        .expect("non-empty return series");
    let ours = metrics.max_drawdown;

    let diff = (ours - expected.abs()).abs();
    assert!(
        diff <= 1e-9,
        "max_drawdown: ours={ours} (positive fraction), \
         quantstats={expected} (signed), |our - |theirs||={diff}"
    );
}

/// Historical CVaR (default in v0.10) must agree with the pure
/// empirical `mean(sorted[..ceil(n * alpha)])` formula at bit-level
/// precision. `compute_metrics.cvar_95` uses this method by default.
///
/// Tolerance: 1e-12 — both sides compute the identical operation
/// (sort, slice, mean).
#[test]
fn cvar_historical_matches_empirical() {
    use nanobook::portfolio::metrics::{CVaRMethod, cvar};

    let g = golden();
    let returns = f64_vec(&g, &["inputs", "returns"]);
    let expected = f64_scalar(&g, &["empirical", "cvar_95"]);

    // Direct API.
    let ours_direct = cvar(&returns, 0.05, CVaRMethod::Historical);
    let diff = (ours_direct - expected).abs();
    assert!(
        diff <= 1e-12,
        "cvar(Historical): ours={ours_direct}, empirical={expected}, diff={diff}"
    );

    // The Metrics struct routes through this method too.
    let metrics = nanobook::portfolio::metrics::compute_metrics(&returns, 252.0, 0.0)
        .expect("non-empty return series");
    let diff = (metrics.cvar_95 - expected).abs();
    assert!(
        diff <= 1e-12,
        "metrics.cvar_95 (Historical default): ours={}, empirical={expected}, diff={diff}",
        metrics.cvar_95
    );
}

/// ParametricNormal CVaR (legacy v0.9 behavior) must still agree with
/// quantstats's `expected_shortfall` at 1e-9 — quantstats uses the
/// same hybrid estimator.
///
/// This pins the legacy path so users who opt in via
/// `CVaRMethod::ParametricNormal` continue to get the value they had
/// before v0.10.
#[test]
fn cvar_parametric_matches_quantstats() {
    use nanobook::portfolio::metrics::{CVaRMethod, cvar};

    let g = golden();
    let returns = f64_vec(&g, &["inputs", "returns"]);
    let expected = f64_scalar(&g, &["quantstats", "cvar_95_parametric"]);

    let ours = cvar(&returns, 0.05, CVaRMethod::ParametricNormal);
    let diff = (ours - expected).abs();
    assert!(
        diff <= 1e-9,
        "cvar(ParametricNormal): ours={ours}, quantstats={expected}, diff={diff}"
    );
}

/// Annualized Sortino (ddof=0, default in v0.10) must agree with
/// `quantstats.stats.sortino` at 1e-9.
///
/// `compute_metrics.sortino` routes through `sortino(..., ddof=0)` by
/// default. The ddof=1 variant (Bessel-corrected, v0.9 behavior) is
/// not pinned here — callers who need it pass `ddof=1` explicitly and
/// can derive the expected value with `sqrt(n/(n-1))` scaling.
#[test]
fn sortino_matches_quantstats() {
    use nanobook::portfolio::metrics::sortino;

    let g = golden();
    let returns = f64_vec(&g, &["inputs", "returns"]);
    let expected = f64_scalar(&g, &["quantstats", "sortino_annual_252"]);

    // Direct API.
    let ours_direct = sortino(&returns, 0.0, 252.0, 0);
    let diff = (ours_direct - expected).abs();
    assert!(
        diff <= 1e-9,
        "sortino(ddof=0) direct: ours={ours_direct}, quantstats={expected}, diff={diff}"
    );

    // The Metrics struct routes through this method too.
    let metrics = nanobook::portfolio::metrics::compute_metrics(&returns, 252.0, 0.0)
        .expect("non-empty return series");
    let diff = (metrics.sortino - expected).abs();
    assert!(
        diff <= 1e-9,
        "metrics.sortino (ddof=0 default): ours={}, quantstats={expected}, diff={diff}",
        metrics.sortino
    );
}

/// Bessel-corrected Sortino (ddof=1, legacy v0.9 behavior) must relate
/// to the ddof=0 result by exactly `sqrt(n/(n-1))`.
///
/// This pins the opt-in legacy path at bit-level.
#[test]
fn sortino_ddof1_matches_scaled_ddof0() {
    use nanobook::portfolio::metrics::sortino;

    let g = golden();
    let returns = f64_vec(&g, &["inputs", "returns"]);
    let n = returns.len() as f64;

    // ddof=1 uses / (n-1), ddof=0 uses / n, so downside_dev ratio is
    // sqrt(n / (n-1)); Sortino is inversely proportional to downside_dev,
    // so ratio is sqrt((n-1)/n).
    let s0 = sortino(&returns, 0.0, 252.0, 0);
    let s1 = sortino(&returns, 0.0, 252.0, 1);
    let ratio = s1 / s0;
    let expected_ratio = ((n - 1.0) / n).sqrt();
    let diff = (ratio - expected_ratio).abs();
    assert!(
        diff <= 1e-12,
        "sortino ddof ratio: got s1/s0={ratio}, expected sqrt((n-1)/n)={expected_ratio}, diff={diff}"
    );
}
