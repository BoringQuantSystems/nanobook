"""Frozen parity tests against the nanobook pure-Python audit reference."""

from __future__ import annotations

import json
import math
from pathlib import Path

import pytest

import nanobook

_FIXTURE = Path(__file__).with_name("scenarios_parity.json")


def _load_cases():
    data = json.loads(_FIXTURE.read_text())
    return data["cases"]


@pytest.mark.parametrize("case", _load_cases(), ids=lambda c: c["name"])
def test_numpy_path_matches_frozen_reference(case):
    params = dict(case["params"])
    res = nanobook.monte_carlo_stock_valuation_parity(**params)
    expected = case["summary"]
    assert math.isclose(res.median_price, expected["median_price"], rel_tol=1e-9, abs_tol=1e-6)
    assert math.isclose(res.mean_price, expected["mean_price"], rel_tol=1e-9, abs_tol=1e-6)
    assert math.isclose(
        res.implied_median_annual_return,
        expected["implied_median_annual_return"],
        rel_tol=1e-9,
        abs_tol=1e-6,
    )
    assert math.isclose(res.p10_price, expected["p10"], rel_tol=1e-9, abs_tol=1e-6)
    assert math.isclose(res.p90_price, expected["p90"], rel_tol=1e-9, abs_tol=1e-6)
    got_sorted = [round(float(x), 6) for x in sorted(res.terminal_prices)]
    assert got_sorted == case["terminal_prices_sorted"]


def test_xyz_advanced_repro_median_band():
    """Plan repro: XYZ advanced calibration median in a stable band (~88 on full 5k paths)."""
    res = nanobook.monte_carlo_stock_valuation_parity(
        "XYZ",
        74.0,
        version="advanced",
        n_paths=200,
        seed=42,
        gp_growth_mean=0.16,
        multiple_mean=22.0,
        macro_shock_mean=-0.03,
    )
    assert 80.0 <= res.median_price <= 95.0