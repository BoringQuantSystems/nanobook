"""Property-based tests for pure-Python MC scenarios."""

from __future__ import annotations

import math

import pytest

try:
    from hypothesis import given, settings, strategies as st

    HAS_HYPOTHESIS = True
except ImportError:
    HAS_HYPOTHESIS = False

import nanobook

pytestmark = pytest.mark.skipif(not HAS_HYPOTHESIS, reason="hypothesis not installed")


@given(
    current_price=st.floats(min_value=1.0, max_value=500.0, allow_nan=False, allow_infinity=False),
    n_paths=st.integers(min_value=10, max_value=200),
    seed=st.integers(min_value=0, max_value=10_000),
)
@settings(max_examples=100)
def test_terminal_prices_positive(current_price, n_paths, seed):
    res = nanobook.monte_carlo_stock_valuation(
        "T",
        current_price,
        n_paths=n_paths,
        seed=seed,
        version="simple",
    )
    assert len(res.terminal_prices) == n_paths
    assert all(p > 0.0 and math.isfinite(p) for p in res.terminal_prices)


@given(
    current_price=st.floats(min_value=5.0, max_value=200.0, allow_nan=False, allow_infinity=False),
    n_paths=st.integers(min_value=20, max_value=150),
    seed=st.integers(min_value=0, max_value=5000),
)
@settings(max_examples=80)
def test_median_between_p10_and_p90(current_price, n_paths, seed):
    res = nanobook.monte_carlo_stock_valuation(
        "T",
        current_price,
        n_paths=n_paths,
        seed=seed,
        version="advanced",
    )
    assert res.p10_price <= res.median_price + 1e-9
    assert res.median_price <= res.p90_price + 1e-9


@given(seed=st.integers(min_value=0, max_value=9999))
@settings(max_examples=50)
def test_same_seed_same_output(seed):
    kwargs = dict(
        ticker="R",
        current_price=88.0,
        n_paths=40,
        version="advanced",
        seed=seed,
    )
    r1 = nanobook.monte_carlo_stock_valuation(**kwargs)
    r2 = nanobook.monte_carlo_stock_valuation(**kwargs)
    assert r1.terminal_prices == r2.terminal_prices
    assert r1.summary == r2.summary


@given(
    current_price=st.floats(min_value=10.0, max_value=300.0, allow_nan=False, allow_infinity=False),
    expected_return=st.floats(min_value=0.0, max_value=0.5, allow_nan=False, allow_infinity=False),
)
@settings(max_examples=60)
def test_zero_vol_simple_is_deterministic(current_price, expected_return):
    res = nanobook.monte_carlo_stock_valuation(
        "Z",
        current_price,
        version="simple",
        n_paths=25,
        seed=3,
        annual_vol=0.0,
        expected_annual_return=expected_return,
    )
    assert all(math.isclose(p, res.terminal_prices[0], rel_tol=1e-9) for p in res.terminal_prices)
    assert res.terminal_prices[0] > 0.0