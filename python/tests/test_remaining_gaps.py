"""Tests for the last uncovered lines in the Python layer.

Three groups: the thin wrappers in ``nanobook/__init__.py`` that forward to
Rust, the numpy-absent fallback in ``scenarios``, and the guards that only a
specific shape of input reaches.
"""

from __future__ import annotations

import builtins
import importlib
import math
import sys

import pytest

import nanobook
from nanobook import scenarios


def _bar(p: int):
    return nanobook.BarPrices(open=p, high=p, low=p, close=p)


def _backtest():
    return nanobook.backtest_weights(
        weight_schedule=[[("AAPL", 1.0)], [("AAPL", 1.0)], [("AAPL", 1.0)]],
        price_schedule=[
            [("AAPL", _bar(100_00))],
            [("AAPL", _bar(102_00))],
            [("AAPL", _bar(101_00))],
        ],
        initial_cash=100_000_00,
        cost_model=nanobook.CostModel.zero(),
        fill_policy=nanobook.FillPolicy.SignalBarClose,
    )


# ── the thin wrappers over Rust ────────────────────────────────────


def test_decompose_backtest_wrapper_returns_the_three_curves() -> None:
    """One symbol held at full weight for two periods then sold."""
    weights = [[("AAA", 1.0)], [("AAA", 1.0)], [("AAA", 0.0)]]
    returns = [[("AAA", 0.0)], [("AAA", 0.10)], [("AAA", 0.05)]]

    result = nanobook.decompose_backtest(weights, returns)

    assert sorted(result) == ["contributions", "cumulative_contributions", "trades"]
    # One (symbol, value) pair per period.
    assert result["contributions"] == [
        [("AAA", pytest.approx(0.0))],
        [("AAA", pytest.approx(0.10))],
        [("AAA", pytest.approx(0.0))],
    ]
    # The cumulative curve is the running sum, so it holds 0.10 once the
    # position is sold rather than dropping back to zero.
    assert result["cumulative_contributions"] == [
        [("AAA", pytest.approx(0.0))],
        [("AAA", pytest.approx(0.10))],
        [("AAA", pytest.approx(0.10))],
    ]


def test_tear_sheet_wrapper_returns_the_four_report_sections() -> None:
    result = nanobook.tear_sheet(_backtest(), rolling_window=2, periods_per_year=252)

    assert sorted(result) == [
        "drawdown_events",
        "monthly_returns",
        "rolling_sharpe",
        "trade_analytics",
    ]


def _six_period_backtest():
    prices = (100_00, 102_00, 101_00, 104_00, 103_00, 106_00)
    return nanobook.backtest_weights(
        weight_schedule=[[("AAPL", 1.0)]] * 6,
        price_schedule=[[("AAPL", _bar(p))] for p in prices],
        initial_cash=100_000_00,
        cost_model=nanobook.CostModel.zero(),
        fill_policy=nanobook.FillPolicy.SignalBarClose,
    )


def test_tear_sheet_wrapper_passes_the_rolling_window_through() -> None:
    """The rolling Sharpe needs a full window before it can report, so a
    window of w leaves exactly w - 1 leading NaN. That count is the proof the
    window argument arrives, and arrives in the right position.
    """
    backtest = _six_period_backtest()

    for window in (2, 3, 4):
        rolling = nanobook.tear_sheet(
            backtest, rolling_window=window, periods_per_year=252
        )["rolling_sharpe"]
        leading_nan = next(i for i, v in enumerate(rolling) if not math.isnan(v))
        assert leading_nan == window - 1
        assert len(rolling) == 6


def test_tear_sheet_wrapper_passes_periods_per_year_through() -> None:
    """Sharpe is annualized by sqrt(periods_per_year), so quartering the
    periods halves the figure. That pins the second argument separately from
    the window, which the leading-NaN count pins.
    """
    backtest = _six_period_backtest()

    yearly = nanobook.tear_sheet(backtest, rolling_window=2, periods_per_year=252)
    quarterly = nanobook.tear_sheet(backtest, rolling_window=2, periods_per_year=63)

    assert yearly["rolling_sharpe"][1] == pytest.approx(
        quarterly["rolling_sharpe"][1] * math.sqrt(252 / 63)
    )


def test_walkforward_wrapper_splits_into_the_requested_window_count() -> None:
    returns = [0.01, -0.02, 0.03, 0.005, -0.01] * 20

    windows = nanobook.walkforward(returns, None, 3, 0.7, 252.0, 0.0)

    assert len(windows) == 3
    for window in windows:
        assert window["train_start"] < window["train_end"] <= window["test_start"]
        assert window["test_start"] < window["test_end"]
        assert "train_metrics" in window
        assert "test_metrics" in window


def test_walkforward_wrapper_honours_the_train_fraction() -> None:
    """A larger train_pct gives a longer training span in the first window."""
    returns = [0.01, -0.02, 0.03, 0.005, -0.01] * 20

    lean = nanobook.walkforward(returns, None, 3, 0.5, 252.0, 0.0)[0]
    rich = nanobook.walkforward(returns, None, 3, 0.9, 252.0, 0.0)[0]

    lean_span = lean["train_end"] - lean["train_start"]
    rich_span = rich["train_end"] - rich["train_start"]
    assert rich_span > lean_span


# ── _pure_percentile: the single-element interpolation guard ───────


def test_pure_percentile_on_a_single_value_returns_that_value() -> None:
    """With one element, q * (n - 1) is 0 and i + 1 equals n, so the
    interpolation step has no neighbour to reach for and falls back to the
    last (only) element.
    """
    assert scenarios._pure_percentile([7.5], 0.5) == pytest.approx(7.5)
    assert scenarios._pure_percentile([7.5], 0.25) == pytest.approx(7.5)
    assert scenarios._pure_percentile([7.5], 0.99) == pytest.approx(7.5)


def test_pure_percentile_interpolates_between_two_neighbours() -> None:
    """Four values, q = 0.5: pos = 1.5, so the answer sits halfway between
    the second and third sorted values.
    """
    assert scenarios._pure_percentile([4.0, 1.0, 3.0, 2.0], 0.5) == pytest.approx(2.5)


# ── _get_rng: the numpy generator branch ───────────────────────────


def test_get_rng_passes_a_seed_sequence_to_numpy() -> None:
    """A SeedSequence is neither an int, None, nor random.Random, so it goes
    to numpy's default_rng rather than the pure-Python path.
    """
    np = pytest.importorskip("numpy")
    seed_sequence = np.random.SeedSequence(1234)

    rng = scenarios._get_rng(seed_sequence)

    assert isinstance(rng, np.random.Generator)


def test_get_rng_returns_a_random_random_untouched() -> None:
    import random

    source = random.Random(7)

    assert scenarios._get_rng(source) is source


# ── compute_annualized_vol: the polars-like drop_nulls branch ──────


class _SeriesLike:
    """Stands in for a polars Series: it has drop_nulls() and to_list()."""

    def __init__(self, values: list[float | None]) -> None:
        self._values = values

    def drop_nulls(self) -> _SeriesLike:
        return _SeriesLike([v for v in self._values if v is not None])

    def to_list(self) -> list[float]:
        return list(self._values)


def test_annualized_vol_accepts_an_object_with_drop_nulls() -> None:
    """The drop_nulls branch must give the same answer as the plain list."""
    values = [0.01, -0.02, 0.03, 0.005, -0.01]

    from_series = scenarios.compute_annualized_vol(_SeriesLike(values))
    from_list = scenarios.compute_annualized_vol(values)

    assert from_series == pytest.approx(from_list)
    assert from_series > 0.0


def test_annualized_vol_drops_nulls_before_computing() -> None:
    """Nulls are removed, so the answer matches the same data without them."""
    values = [0.01, -0.02, 0.03, 0.005, -0.01]

    with_nulls = scenarios.compute_annualized_vol(_SeriesLike([0.01, None, -0.02, 0.03, None, 0.005, -0.01]))

    assert with_nulls == pytest.approx(scenarios.compute_annualized_vol(values))


# ── the numpy-absent guards ────────────────────────────────────────


def test_numpy_batch_loop_refuses_to_run_when_numpy_is_absent(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The batch oracle needs numpy. Without it, it must say so instead of
    failing later with an attribute error on None.
    """
    monkeypatch.setattr(scenarios, "np", None)
    params = scenarios.ValuationParams()

    with pytest.raises(RuntimeError, match="numpy required for parity audit path"):
        scenarios._advanced_numpy_batch_loop(100.0, 10, 1.0, params, object())


def test_module_falls_back_to_pure_python_when_numpy_cannot_be_imported(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Reload the module with the numpy import blocked. The module must still
    import, with np set to None and the numpy flag off.
    """
    real_import = builtins.__import__

    def _block_numpy(name, *args, **kwargs):
        if name == "numpy" or name.startswith("numpy."):
            raise ImportError("numpy is blocked for this test")
        return real_import(name, *args, **kwargs)

    monkeypatch.setattr(builtins, "__import__", _block_numpy)
    monkeypatch.delitem(sys.modules, "numpy", raising=False)

    reloaded = importlib.reload(scenarios)
    try:
        assert reloaded.np is None
        assert reloaded._HAS_NUMPY is False
        # The pure-Python path still answers.
        assert reloaded._pure_percentile([1.0, 2.0, 3.0, 4.0], 0.5) == pytest.approx(2.5)
    finally:
        monkeypatch.undo()
        importlib.reload(scenarios)

    # The restored module has numpy back.
    assert scenarios._HAS_NUMPY is True


# ── the Rust bridge branch of the parity entry point ───────────────


def test_parity_takes_the_rust_bridge_when_the_bridge_flag_is_on(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """With MC_NUMPY_BRIDGE on, an int seed routes to the Rust parity call.
    The result must still be a MonteCarloResult with a sane price spread.
    """
    if not scenarios._HAS_RUST_SCENARIOS or scenarios._rust_monte_carlo_parity is None:
        pytest.skip("Rust scenarios extension not built")

    monkeypatch.setattr(scenarios, "_MC_NUMPY_BRIDGE", True)

    result = scenarios.monte_carlo_stock_valuation_parity(
        "AAPL", 100.0, n_paths=200, seed=42
    )

    assert isinstance(result, scenarios.MonteCarloResult)
    assert result.ticker == "AAPL"
    assert result.current_price == pytest.approx(100.0)
    assert result.p10_price <= result.median_price <= result.p90_price
    assert math.isfinite(result.mean_price)
    assert result.p10_price > 0.0
    assert len(result.terminal_prices) == 200


def test_parity_takes_the_pure_python_path_when_the_bridge_flag_is_off(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(scenarios, "_MC_NUMPY_BRIDGE", False)

    result = scenarios.monte_carlo_stock_valuation_parity(
        "AAPL", 100.0, n_paths=200, seed=42
    )

    assert isinstance(result, scenarios.MonteCarloResult)
    assert result.p10_price <= result.median_price <= result.p90_price
    assert len(result.terminal_prices) == 200
