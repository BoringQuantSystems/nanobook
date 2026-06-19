"""Monte Carlo terminal valuation for nanobook.

Primary path: Rust core (`src/scenarios.rs`) via PyO3 when the extension is
built with the `scenarios` feature and seed is `int` or `None`. NumPy draws
normals; Rust applies closed-form terminal math (ADR-0006).

Fallback: pure-Python implementation (stdlib + optional NumPy) for
`random.Random` seeds, no-extension builds, and stdlib-only environments.

Reference oracle: nanotrade/calc/scenarios.py (parity fixtures in
`tests/reference/scenarios_parity.json`).

Key properties:
- Explicit seed for full reproducibility.
- Two models: "simple" (classic GBM) and "advanced" (multi-driver).
- Returns a rich MonteCarloResult with terminal prices (list) and summary.
- Can generate price paths for use with nanobook's deterministic backtester.
"""

from __future__ import annotations

import math
import random
from dataclasses import dataclass
from typing import Literal, Any

# Try optional numpy for acceleration (dev/test only for parity & speed)
try:
    import numpy as np  # type: ignore

    _HAS_NUMPY = True
except ImportError:
    np = None  # type: ignore
    _HAS_NUMPY = False

# Rust-backed path (numpy RNG bridge inside extension; int/None seeds only)
try:
    from nanobook.nanobook import monte_carlo_stock_valuation as _rust_monte_carlo

    _HAS_RUST_SCENARIOS = True
except Exception:  # pragma: no cover
    _rust_monte_carlo = None  # type: ignore
    _HAS_RUST_SCENARIOS = False


ModelVersion = Literal["simple", "advanced"]


@dataclass
class ValuationParams:
    """Advanced multi-driver parameters (annualized log-return contributions)."""

    gp_growth_mean: float = 0.16
    gp_growth_sd: float = 0.06
    margin_boost_mean: float = 0.02
    margin_boost_sd: float = 0.03
    multiple_mean: float = 22.0
    multiple_sd: float = 3.5
    macro_shock_mean: float = -0.03
    macro_shock_sd: float = 0.11
    bear_skew_factor: float = 0.04


@dataclass
class MonteCarloResult:
    """Result of a Monte Carlo valuation run.

    terminal_prices: list of simulated terminal prices (length = n_paths)
    summary: dict with keys like 'median_price', 'implied_median_annual_return', etc.

    Methods provide convenient access (prob_above, to_price_paths, etc.).
    """

    ticker: str
    method: str
    horizon_years: float
    current_price: float
    terminal_prices: list[float]
    summary: dict[str, Any]

    @property
    def median_price(self) -> float:
        return float(self.summary["median_price"])

    @property
    def mean_price(self) -> float:
        return float(self.summary["mean_price"])

    @property
    def implied_median_annual_return(self) -> float:
        return float(self.summary["implied_median_annual_return"])

    @property
    def p10_price(self) -> float:
        return float(self.summary["p10"])

    @property
    def p90_price(self) -> float:
        return float(self.summary["p90"])

    def prob_above(self, level: float) -> float:
        n = len(self.terminal_prices)
        if n == 0:
            return 0.0
        count = sum(1 for p in self.terminal_prices if p > level)
        return count / n

    def quantile(self, q: float) -> float:
        if not self.terminal_prices:
            return 0.0
        s = sorted(self.terminal_prices)
        idx = max(0, min(len(s) - 1, int(q * (len(s) - 1))))
        return float(s[idx])

    def as_log_returns(self) -> list[float]:
        if self.current_price <= 0:
            return [0.0] * len(self.terminal_prices)
        return [math.log(p / self.current_price) for p in self.terminal_prices]

    def to_price_paths(
        self,
        n_periods: int,
        *,
        seed: int | random.Random | None = None,
        method: Literal["linear", "terminal_only"] = "linear",
    ) -> list[list[float]]:
        """Generate crude intra-horizon price paths from the terminals.

        Returns list of lists (n_paths x n_periods).
        """
        if n_periods < 1:
            raise ValueError("n_periods must be >= 1")
        paths: list[list[float]] = []
        _ = _make_pure_rng(seed)  # accepted for future stochastic variants
        for term in self.terminal_prices:
            if method == "terminal_only":
                p = [self.current_price] * (n_periods - 1) + [term]
                paths.append(p)
                continue
            # Linear in log space
            if self.current_price <= 0 or term <= 0:
                paths.append([self.current_price] * n_periods)
                continue
            log0 = math.log(self.current_price)
            logt = math.log(term)
            step = (logt - log0) / (n_periods - 1) if n_periods > 1 else 0.0
            row = [math.exp(log0 + i * step) for i in range(n_periods)]
            paths.append(row)
        return paths

    def to_summary_dict(self) -> dict[str, Any]:
        return dict(self.summary)

    def __repr__(self) -> str:
        return (
            f"MonteCarloResult(ticker={self.ticker!r}, method={self.method!r}, "
            f"n_paths={len(self.terminal_prices)}, median_price={self.median_price})"
        )


def _validate_mc_inputs(
    current_price: float, n_paths: int, horizon: float, annual_vol: float
) -> None:
    if current_price <= 0 or not math.isfinite(current_price):
        raise ValueError(
            f"current_price must be positive and finite, got {current_price}"
        )
    if n_paths < 0:
        raise ValueError(f"n_paths must be >= 0, got {n_paths}")
    if horizon <= 0 or not math.isfinite(horizon):
        raise ValueError(f"horizon must be positive and finite, got {horizon}")
    if annual_vol < 0 or not math.isfinite(annual_vol):
        raise ValueError(
            f"annual_vol must be non-negative and finite, got {annual_vol}"
        )


def _make_pure_rng(seed: int | random.Random | None) -> random.Random:
    """Return a random.Random instance. Pure stdlib."""
    if isinstance(seed, random.Random):
        return seed
    rng = random.Random()
    if seed is not None:
        rng.seed(seed)
    return rng


def _get_rng(seed: int | random.Random | None) -> Any:
    """Best RNG (np.Generator if avail+seed int, else random.Random)."""  # noqa: E501
    if _HAS_NUMPY and np is not None:
        if isinstance(seed, (int, type(None))):
            return np.random.default_rng(seed)
        if isinstance(seed, random.Random):
            # fall back to pure for explicit Random instance
            return seed
        return np.random.default_rng(seed)
    return _make_pure_rng(seed)


def _pure_mean(xs: list[float]) -> float:
    if not xs:
        return 0.0
    return sum(xs) / len(xs)


def _pure_median(xs: list[float]) -> float:
    if not xs:
        return 0.0
    s = sorted(xs)
    n = len(s)
    mid = n // 2
    if n % 2 == 1:
        return float(s[mid])
    return (s[mid - 1] + s[mid]) / 2


def _pure_percentile(xs: list[float], q: float) -> float:
    """Percentile (nearest-rank + linear interp to match np for parity)."""  # noqa: E501
    if not xs:
        return 0.0
    s = sorted(xs)
    n = len(s)
    if q <= 0:
        return float(s[0])
    if q >= 1:
        return float(s[-1])
    # position
    pos = q * (n - 1)
    i = int(pos)
    # linear interpolation for better match to np.percentile default
    frac = pos - i
    if i + 1 >= n:
        return float(s[-1])
    return float(s[i] + (s[i + 1] - s[i]) * frac)


def _pure_prob_above(xs: list[float], level: float) -> float:
    if not xs:
        return 0.0
    return sum(1 for x in xs if x > level) / len(xs)


def _normal_box_muller(rng: random.Random, mu: float, sigma: float) -> float:
    """Explicit Box-Muller normal draw (two ``random()`` calls per sample).

    Fallback when you need predictable uniform consumption (no ``gauss`` spare
    cache). Prefer :func:`_normal` for MC paths — it uses ``Random.gauss``.
    """
    if sigma == 0.0:
        return mu
    u1 = rng.random()
    u2 = rng.random()
    z = math.sqrt(-2.0 * math.log(max(u1, 1e-12))) * math.cos(2 * math.pi * u2)
    return mu + sigma * z


def _normal(rng: random.Random | Any, mu: float, sigma: float) -> float:
    """Draw one sample from ``N(mu, sigma)``.

    * ``numpy.random.Generator`` (e.g. ``default_rng``): delegates to
      ``rng.normal(mu, sigma)`` — same stream as the reference MC impl.
    * ``random.Random``: uses ``rng.gauss(mu, sigma)`` (cached Box-Muller).

    Reproducibility: same seed + same engine → bit-identical sequences.
    With ``default_rng(seed)``, sequential :func:`_normal` calls match
    ``rng.normal(mu, sigma)`` one draw at a time (verified in tests).
    With ``random.Random(seed)``, sequences are stable across runs but use
    MT19937, not PCG64 — integer seeds do **not** match numpy streams.

    For explicit two-uniform Box-Muller without the gauss spare cache, use
    :func:`_normal_box_muller`.
    """
    if sigma == 0.0:
        return mu
    if hasattr(rng, "normal") and not isinstance(rng, random.Random):
        return float(rng.normal(mu, sigma))
    return rng.gauss(mu, sigma)


def compute_annualized_vol(
    returns: list[float] | Any, periods_per_year: int = 252
) -> float:
    """Pure stdlib annualized vol. Accepts list or (if numpy) array-like."""
    if _HAS_NUMPY and hasattr(returns, "drop_nulls"):
        # compatibility with pl.Series from reference
        arr = [float(x) for x in returns.drop_nulls().to_list() if x == x]
    elif _HAS_NUMPY and np is not None and hasattr(returns, "__array__"):
        arr = [float(x) for x in np.asarray(returns).ravel() if math.isfinite(x)]
    else:
        arr = [float(x) for x in returns if math.isfinite(float(x))]
    n = len(arr)
    if n < 2:
        return 0.30
    mean = sum(arr) / n
    var = sum((x - mean) ** 2 for x in arr) / (n - 1)
    vol = math.sqrt(var)
    return vol * (periods_per_year**0.5)


def simple_gbm_terminal(
    current_price: float,
    n_paths: int,
    horizon: float,
    expected_annual_return: float,
    annual_vol: float,
    rng: random.Random,
) -> list[float]:
    if current_price <= 0 or n_paths <= 0 or horizon <= 0 or annual_vol < 0:
        raise ValueError("Invalid GBM parameters")
    drift = (expected_annual_return - 0.5 * annual_vol**2) * horizon
    sigma = annual_vol * math.sqrt(horizon)
    prices = []
    for _ in range(n_paths):
        z = _normal(rng, 0.0, 1.0)
        prices.append(current_price * math.exp(drift + sigma * z))
    return prices


def advanced_multi_driver_terminal(
    current_price: float,
    n_paths: int,
    horizon: float,
    params: ValuationParams,
    rng: random.Random,
) -> list[float]:
    if current_price <= 0 or n_paths <= 0 or horizon <= 0:
        raise ValueError("Invalid price/horizon")
    p = params
    prices = []
    for _ in range(n_paths):
        gp = _normal(rng, p.gp_growth_mean, p.gp_growth_sd)
        marg = _normal(rng, p.margin_boost_mean, p.margin_boost_sd)
        mult = max(16.0, min(28.0, _normal(rng, p.multiple_mean, p.multiple_sd)))
        shock = _normal(rng, p.macro_shock_mean, p.macro_shock_sd) - abs(
            _normal(rng, 0.0, p.bear_skew_factor)
        )
        total_ret = (gp * 0.8) + (marg * 2.0) + ((mult / 20.0 - 1.0) * 0.6) + shock
        prices.append(current_price * math.exp(total_ret * horizon))
    return prices


def monte_carlo_stock_valuation(
    ticker: str,
    current_price: float,
    *,
    version: ModelVersion = "advanced",
    n_paths: int = 5000,
    horizon: float = 1.0,
    seed: int | random.Random | None = 42,
    expected_annual_return: float = 0.18,
    annual_vol: float = 0.38,
    gp_growth_mean: float = 0.16,
    gp_growth_sd: float = 0.06,
    margin_boost_mean: float = 0.02,
    margin_boost_sd: float = 0.03,
    multiple_mean: float = 22.0,
    multiple_sd: float = 3.5,
    macro_shock_mean: float = -0.03,
    macro_shock_sd: float = 0.11,
    bear_skew_factor: float = 0.04,
    hurdle_rate: float = 0.08,
    bull_price: float | None = None,
    bear_price: float | None = None,
) -> MonteCarloResult:
    """Run Monte Carlo terminal valuation (Rust when available, else pure-Python).

    Delegates to the Rust core via PyO3 when the extension is built with the
    ``scenarios`` feature and ``seed`` is ``int`` or ``None``. Falls back to the
    pure-Python path for ``random.Random`` seeds and no-extension builds.

    See ``nanotrade/calc/scenarios.py`` and ADR-0006 for full semantics.
    """
    _validate_mc_inputs(current_price, n_paths, horizon, annual_vol)

    if (
        _HAS_RUST_SCENARIOS
        and _rust_monte_carlo is not None
        and (seed is None or isinstance(seed, int))
    ):
        rust_res = _rust_monte_carlo(
            ticker,
            current_price,
            version=version,
            n_paths=n_paths,
            horizon=horizon,
            seed=seed,
            expected_annual_return=expected_annual_return,
            annual_vol=annual_vol,
            gp_growth_mean=gp_growth_mean,
            gp_growth_sd=gp_growth_sd,
            margin_boost_mean=margin_boost_mean,
            margin_boost_sd=margin_boost_sd,
            multiple_mean=multiple_mean,
            multiple_sd=multiple_sd,
            macro_shock_mean=macro_shock_mean,
            macro_shock_sd=macro_shock_sd,
            bear_skew_factor=bear_skew_factor,
            hurdle_rate=hurdle_rate,
            bull_price=bull_price,
            bear_price=bear_price,
        )
        summary = dict(rust_res.summary)
        summary["ticker"] = rust_res.ticker
        summary["method"] = rust_res.method
        return MonteCarloResult(
            ticker=rust_res.ticker,
            method=rust_res.method,
            horizon_years=rust_res.horizon_years,
            current_price=rust_res.current_price,
            terminal_prices=list(rust_res.terminal_prices),
            summary=summary,
        )

    rng = _get_rng(seed)

    use_np = _HAS_NUMPY and hasattr(rng, "normal")  # numpy Generator

    if version == "simple":
        if use_np:
            drift = (expected_annual_return - 0.5 * annual_vol**2) * horizon
            diffusion = annual_vol * np.sqrt(horizon) * rng.standard_normal(n_paths)
            prices = (current_price * np.exp(drift + diffusion)).tolist()
        else:
            prices = simple_gbm_terminal(
                current_price, n_paths, horizon, expected_annual_return, annual_vol, rng
            )
        method = "Simple GBM"
    else:
        params = ValuationParams(
            gp_growth_mean=gp_growth_mean,
            gp_growth_sd=gp_growth_sd,
            margin_boost_mean=margin_boost_mean,
            margin_boost_sd=margin_boost_sd,
            multiple_mean=multiple_mean,
            multiple_sd=multiple_sd,
            macro_shock_mean=macro_shock_mean,
            macro_shock_sd=macro_shock_sd,
            bear_skew_factor=bear_skew_factor,
        )
        if use_np:
            p = params
            gp = rng.normal(p.gp_growth_mean, p.gp_growth_sd, n_paths)
            marg = rng.normal(p.margin_boost_mean, p.margin_boost_sd, n_paths)
            mult = np.clip(
                rng.normal(p.multiple_mean, p.multiple_sd, n_paths), 16.0, 28.0
            )
            shock = rng.normal(p.macro_shock_mean, p.macro_shock_sd, n_paths) - np.abs(
                rng.normal(0.0, p.bear_skew_factor, n_paths)
            )
            total_ret = (gp * 0.8) + (marg * 2.0) + ((mult / 20.0 - 1.0) * 0.6) + shock
            prices = (current_price * np.exp(total_ret * horizon)).tolist()
        else:
            prices = advanced_multi_driver_terminal(
                current_price, n_paths, horizon, params, rng
            )
        method = "Advanced Multi-Driver"

    bull = bull_price if bull_price is not None else current_price * 1.10
    bear = bear_price if bear_price is not None else current_price * 0.81
    hurdle_mult = 1.0 + hurdle_rate

    med = _pure_median(prices)
    mn = _pure_mean(prices)

    row = {
        "ticker": ticker,
        "method": method,
        "n_paths": n_paths,
        "horizon_years": horizon,
        "median_price": round(med, 2),
        "mean_price": round(mn, 2),
        "mean_minus_median": round(mn - med, 2),
        "pct_above_hurdle": round(
            _pure_prob_above(prices, current_price * hurdle_mult) * 100, 1
        ),
        "pct_above_bull": round(_pure_prob_above(prices, bull) * 100, 1),
        "pct_below_bear": round(1 - _pure_prob_above(prices, bear) * 100, 1),
        "p10": round(_pure_percentile(prices, 0.10), 2),
        "p90": round(_pure_percentile(prices, 0.90), 2),
        "implied_median_annual_return": round(
            (med / current_price) ** (1.0 / horizon) - 1.0, 4
        ),
        "current_price": current_price,
        "hurdle_rate": hurdle_rate,
    }

    return MonteCarloResult(
        ticker=ticker,
        method=method,
        horizon_years=horizon,
        current_price=current_price,
        terminal_prices=prices,
        summary=row,
    )


def calibrate_from_fundamentals(
    ticker: str,
    *,
    current_price: float,
    hist_vol: float | None = None,
    expected_annual_return: float | None = None,
    gp_growth: float | None = None,
    margin_expansion: float | None = None,
    fwd_multiple: float | None = None,
    macro_drag: float | None = None,
) -> dict[str, float]:
    """Pure stdlib calibrate stub."""
    out: dict[str, float] = {
        "expected_annual_return": expected_annual_return or 0.15,
        "annual_vol": hist_vol or 0.35,
        "gp_growth_mean": gp_growth or 0.12,
        "margin_boost_mean": margin_expansion or 0.01,
        "multiple_mean": fwd_multiple or 20.0,
        "macro_shock_mean": macro_drag or -0.02,
    }
    out.setdefault("gp_growth_sd", 0.06)
    out.setdefault("margin_boost_sd", 0.03)
    out.setdefault("multiple_sd", 3.0)
    out.setdefault("macro_shock_sd", 0.10)
    out.setdefault("bear_skew_factor", 0.04)
    return out


def terminal_prices_to_log_return_paths(
    terminal_prices: list[float],
    current_price: float,
    n_periods: int,
    *,
    seed: int | random.Random | None = None,
    method: Literal["linear", "terminal_only"] = "linear",
) -> list[list[float]]:
    """Pure version of path generator."""
    if n_periods < 1:
        raise ValueError("n_periods must be >= 1")
    out: list[list[float]] = []
    for term in terminal_prices:
        if method == "terminal_only":
            out.append([current_price] * (n_periods - 1) + [term])
            continue
        if current_price <= 0 or term <= 0:
            out.append([current_price] * n_periods)
            continue
        log0 = math.log(current_price)
        logt = math.log(term)
        step = (logt - log0) / max(1, n_periods - 1)
        row = [math.exp(log0 + i * step) for i in range(n_periods)]
        out.append(row)
    return out


def summarize_distribution(
    prices: list[float], current_price: float
) -> dict[str, float]:
    if not prices:
        return {
            "median": 0.0,
            "mean": 0.0,
            "p05": 0.0,
            "p95": 0.0,
            "prob_above_current": 0.0,
        }
    return {
        "median": _pure_median(prices),
        "mean": _pure_mean(prices),
        "p05": _pure_percentile(prices, 0.05),
        "p95": _pure_percentile(prices, 0.95),
        "prob_above_current": _pure_prob_above(prices, current_price),
        "prob_above_10pct": _pure_prob_above(prices, current_price * 1.10),
    }
