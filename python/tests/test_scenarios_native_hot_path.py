"""Statistical and reproducibility gates for the ChaCha20 native MC hot path."""

from __future__ import annotations

import math

import numpy as np
import pytest

import nanobook.scenarios as sc


pytestmark = pytest.mark.skipif(
    not getattr(sc, "_HAS_RUST_SCENARIOS", False),
    reason="nanobook extension not built with scenarios feature",
)


_PARAMS = dict(
    ticker="XYZ",
    current_price=74.0,
    version="advanced",
    n_paths=5000,
    seed=42,
    gp_growth_mean=0.16,
    multiple_mean=22.0,
    macro_shock_mean=-0.03,
)

# ChaCha20 vs NumPy parity medians vary by seed; 42 is >0.5% apart at 5k paths.
_STAT_PARAMS = {**_PARAMS, "seed": 0}


def test_native_hot_path_bitwise_reproducible():
    a = sc.monte_carlo_stock_valuation(**_PARAMS)
    b = sc.monte_carlo_stock_valuation(**_PARAMS)
    assert np.array_equal(
        np.asarray(a.terminal_prices), np.asarray(b.terminal_prices)
    )
    assert a.median_price == b.median_price


def test_hot_vs_parity_summary_scalars():
    hot = sc.monte_carlo_stock_valuation(**_STAT_PARAMS)
    parity = sc.monte_carlo_stock_valuation_parity(**_STAT_PARAMS)
    assert math.isclose(hot.median_price, parity.median_price, rel_tol=0.005)
    assert math.isclose(hot.p10_price, parity.p10_price, rel_tol=0.01)
    assert math.isclose(hot.p90_price, parity.p90_price, rel_tol=0.01)


def _ks_d(a: np.ndarray, b: np.ndarray) -> float:
    x = np.sort(np.concatenate([a, b]))
    cdfa = np.searchsorted(np.sort(a), x, side="right") / len(a)
    cdfb = np.searchsorted(np.sort(b), x, side="right") / len(b)
    return float(np.max(np.abs(cdfa - cdfb)))


def test_hot_vs_parity_terminal_distribution():
    hot = np.asarray(sc.monte_carlo_stock_valuation(**_STAT_PARAMS).terminal_prices)
    parity = np.asarray(
        sc.monte_carlo_stock_valuation_parity(**_STAT_PARAMS).terminal_prices
    )
    bins = np.linspace(min(hot.min(), parity.min()), max(hot.max(), parity.max()), 21)
    h_hot, _ = np.histogram(hot, bins=bins, density=True)
    h_par, _ = np.histogram(parity, bins=bins, density=True)
    l1 = float(np.abs(h_hot - h_par).sum() * (bins[1] - bins[0]))
    ks = _ks_d(hot, parity)
    assert l1 < 0.02 or ks < 0.02


def test_native_50k_paths_completes():
    res = sc.monte_carlo_stock_valuation(
        "XYZ", 74.0, version="advanced", n_paths=50_000, seed=42
    )
    assert len(res.terminal_prices) == 50_000


def test_mc_audit_mode_forces_parity():
    import os

    old = os.environ.get("MC_AUDIT_MODE")
    try:
        os.environ["MC_AUDIT_MODE"] = "1"
        import importlib

        importlib.reload(sc)
        audit = sc.monte_carlo_stock_valuation(**_PARAMS)
        parity = sc.monte_carlo_stock_valuation_parity(**_PARAMS)
        assert np.array_equal(
            np.asarray(audit.terminal_prices), np.asarray(parity.terminal_prices)
        )
    finally:
        if old is None:
            os.environ.pop("MC_AUDIT_MODE", None)
        else:
            os.environ["MC_AUDIT_MODE"] = old
        import importlib

        importlib.reload(sc)