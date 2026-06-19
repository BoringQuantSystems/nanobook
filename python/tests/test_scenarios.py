"""Tests for pure-Python MC scenarios in nanobook.

Focus on:
- No hard runtime dep on numpy/pandas (stdlib works)
- Reproducibility with seed
- Parity with the reference numpy implementation (when numpy available in test env)
- Structure and feeding to nanobook backtester
"""

from __future__ import annotations

import math
import random
import subprocess
import sys
from pathlib import Path

import pytest

import nanobook


def test_rust_scenarios_path_active_when_extension_built():
    """Int/None seeds delegate to the Rust extension (numpy RNG bridge)."""
    import nanobook.scenarios as sc

    assert sc._HAS_RUST_SCENARIOS, "extension should be built with scenarios feature in dev/CI"
    res = nanobook.monte_carlo_stock_valuation("T", 100.0, n_paths=10, seed=42, version="simple")
    assert type(res).__name__ == "MonteCarloResult"
    assert isinstance(res.terminal_prices, list)


def test_monte_carlo_exposed_and_runs():
    assert hasattr(nanobook, "monte_carlo_stock_valuation")
    res = nanobook.monte_carlo_stock_valuation("T", 100.0, n_paths=100, seed=123, version="simple")
    assert res.ticker == "T"
    assert "MonteCarloResult" in repr(res)
    assert len(res.terminal_prices) == 100
    assert res.median_price > 0
    assert 0 < res.implied_median_annual_return < 1
    assert isinstance(res.summary, dict)


def test_reproducibility_same_seed():
    r1 = nanobook.monte_carlo_stock_valuation("A", 50.0, n_paths=200, seed=42, version="advanced")
    r2 = nanobook.monte_carlo_stock_valuation("A", 50.0, n_paths=200, seed=42, version="advanced")
    assert r1.terminal_prices == r2.terminal_prices
    assert r1.median_price == r2.median_price


def test_simple_vs_advanced_differ():
    r_s = nanobook.monte_carlo_stock_valuation("X", 74.0, n_paths=1000, seed=99, version="simple")
    r_a = nanobook.monte_carlo_stock_valuation("X", 74.0, n_paths=1000, seed=99, version="advanced")
    # They use different models so results differ (unless by chance)
    assert abs(r_s.median_price - r_a.median_price) > 1e-6 or r_s.method != r_a.method


@pytest.mark.skipif(not getattr(nanobook, "scenarios", None) or not getattr(nanobook.scenarios, "_HAS_NUMPY", False), reason="numpy not available for parity")
def test_parity_with_reference_impl():
    # Reference is the numpy version in nanotrade/calc (same monorepo)
    try:
        sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "nanotrade"))
        from calc import scenarios as ref_scenarios
    except Exception:
        pytest.skip("reference nanotrade/calc/scenarios not importable in this env")

    params = dict(
        ticker="XYZ",
        current_price=74.0,
        version="advanced",
        n_paths=2000,
        seed=42,
        gp_growth_mean=0.16,
        multiple_mean=22.0,
        macro_shock_mean=-0.03,
    )
    ref_res = ref_scenarios.monte_carlo_stock_valuation(**params)
    our_res = nanobook.monte_carlo_stock_valuation(**params)

    # When numpy accel is used, results should match closely
    assert abs(our_res.median_price - ref_res.median_price) < 0.5
    assert abs(our_res.implied_median_annual_return - ref_res.implied_median_annual_return) < 0.01
    # For the exact seed + np path we expect very close (the reference used np)
    ref_med = ref_res.median_price
    our_med = our_res.median_price
    assert math.isclose(our_med, ref_med, rel_tol=1e-3, abs_tol=0.2), f"median mismatch {our_med} vs ref {ref_med}"


def test_to_price_paths_usable_with_nanobook():
    res = nanobook.monte_carlo_stock_valuation("P", 100.0, n_paths=5, seed=7, version="simple")
    paths = res.to_price_paths(4)  # 4 periods
    assert len(paths) == 5
    assert len(paths[0]) == 4
    # Can be turned into cents schedule for nanobook (smoke)
    # Just check it doesn't crash if we try a tiny backtest
    # (full integration tested in e2e elsewhere)
    cents0 = int(round(paths[0][-1] * 100))
    assert cents0 > 0


def test_calibrate_and_summary():
    drivers = nanobook.calibrate_from_fundamentals("Z", current_price=80.0, hist_vol=0.4)
    assert "annual_vol" in drivers
    res = nanobook.monte_carlo_stock_valuation("Z", 80.0, **drivers, n_paths=50, seed=1)
    d = res.to_summary_dict()
    assert "implied_median_annual_return" in d
    assert "p10" in d


def test_edge_case_zero_paths_returns_empty_summary():
    res = nanobook.monte_carlo_stock_valuation("E", 100.0, n_paths=0, seed=1)
    assert res.terminal_prices == []
    assert res.median_price == 0.0


def test_edge_case_invalid_price_raises():
    with pytest.raises(ValueError, match="current_price"):
        nanobook.monte_carlo_stock_valuation("E", 0.0, n_paths=10, seed=1)


def test_edge_case_negative_paths_raises():
    with pytest.raises((ValueError, TypeError)):
        nanobook.monte_carlo_stock_valuation("E", 100.0, n_paths=-1, seed=1)


def test_pure_stdlib_path_without_numpy(monkeypatch):
    """Scenarios must run when numpy is unavailable (stdlib-only path)."""
    import nanobook.scenarios as sc

    monkeypatch.setattr(sc, "_HAS_NUMPY", False)
    monkeypatch.setattr(sc, "np", None)
    rng = sc._make_pure_rng(42)
    res = sc.monte_carlo_stock_valuation(
        "PURE",
        74.0,
        version="advanced",
        n_paths=30,
        seed=rng,
    )
    assert len(res.terminal_prices) == 30
    assert all(p > 0 for p in res.terminal_prices)
    assert res.median_price > 0


def test_e2e_scenario_paths_into_backtest_weights():
    """End-to-end: MC terminals -> price schedule -> nanobook.backtest_weights."""
    res = nanobook.monte_carlo_stock_valuation(
        "XYZ",
        74.0,
        version="advanced",
        n_paths=3,
        seed=42,
        gp_growth_mean=0.16,
        multiple_mean=22.0,
        macro_shock_mean=-0.03,
    )
    paths = res.to_price_paths(4, method="linear")
    price_schedule = []
    for path in paths[:1]:
        for price in path:
            cents = int(round(price * 100))
            price_schedule.append(
                [("XYZ", nanobook.BarPrices(cents, cents, cents, cents))]
            )
    weight_schedule = [[("XYZ", 1.0)]] * len(price_schedule)
    result = nanobook.backtest_weights(
        weight_schedule=weight_schedule,
        price_schedule=price_schedule,
        initial_cash=1_000_000_00,
        cost_model=nanobook.CostModel.zero(),
        fill_policy=nanobook.FillPolicy.NextBarOpen,
    )
    assert len(result["equity_curve"]) == len(price_schedule) + 1
    assert result["equity_curve"][-1] > 0


_NO_NUMPY_SNIPPET = """
import importlib
import sys
sys.modules.pop("numpy", None)
sc = importlib.import_module("nanobook.scenarios")
sc._HAS_NUMPY = False
sc.np = None
res = sc.monte_carlo_stock_valuation("N", 50.0, n_paths=10, seed=sc._make_pure_rng(7), version="simple")
assert len(res.terminal_prices) == 10
assert all(p > 0 for p in res.terminal_prices)
print("ok")
"""


def test_scenarios_importable_in_subprocess_without_numpy():
    """Smoke: fresh interpreter with numpy blocked still runs pure scenarios."""
    env = {**dict(**__import__("os").environ), "PYTHONPATH": str(Path(__file__).resolve().parents[1])}
    proc = subprocess.run(
        [sys.executable, "-c", _NO_NUMPY_SNIPPET],
        env=env,
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    assert proc.returncode == 0, proc.stderr or proc.stdout
    assert "ok" in proc.stdout
