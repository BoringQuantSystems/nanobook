"""Tests for scenario helper utilities (paths, distribution summary, vol)."""

from __future__ import annotations

import math

import pytest

import nanobook


def test_summarize_distribution_keys_and_prob():
    prices = [90.0, 100.0, 110.0, 120.0, 130.0]
    summary = nanobook.summarize_distribution(prices, current_price=100.0)
    assert summary["median"] == 110.0
    assert summary["mean"] == 110.0
    assert summary["p05"] <= summary["median"] <= summary["p95"]
    assert summary["prob_above_current"] == 0.6
    assert summary["prob_above_10pct"] == 0.4


def test_terminal_prices_to_log_return_paths_linear():
    paths = nanobook.terminal_prices_to_log_return_paths(
        [110.0, 121.0],
        current_price=100.0,
        n_periods=3,
        method="linear",
    )
    assert len(paths) == 2
    assert len(paths[0]) == 3
    assert paths[0][0] == pytest.approx(100.0)
    assert paths[0][-1] == pytest.approx(110.0)


def test_compute_annualized_vol_positive():
    returns = [0.01, -0.005, 0.02, 0.0, -0.01]
    vol = nanobook.compute_annualized_vol(returns, periods_per_year=252)
    assert vol > 0.0
    assert math.isfinite(vol)


def test_summarize_distribution_empty():
    assert nanobook.summarize_distribution([], 50.0)["median"] == 0.0