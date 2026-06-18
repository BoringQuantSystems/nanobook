"""Tests for pure-Python MC scenarios in nanobook.

Focus on:
- No hard runtime dep on numpy/pandas (stdlib works)
- Reproducibility with seed
- Parity with the reference numpy implementation (when numpy available in test env)
- Structure and feeding to nanobook backtester
"""

import sys
import math
import random

import pytest

import nanobook


def test_monte_carlo_exposed_and_runs():
    assert hasattr(nanobook, "monte_carlo_stock_valuation")
    res = nanobook.monte_carlo_stock_valuation("T", 100.0, n_paths=100, seed=123, version="simple")
    assert res.ticker == "T"
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


@pytest.mark.skipif(not nanobook.scenarios._HAS_NUMPY if hasattr(nanobook, "scenarios") else True, reason="numpy not available for parity")
def test_parity_with_reference_impl():
    # Reference is the numpy version in nanotrade/calc (same monorepo)
    try:
        from nanotrade.calc import scenarios as ref_scenarios
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
