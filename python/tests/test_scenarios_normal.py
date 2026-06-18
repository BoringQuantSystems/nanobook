"""Tests for scenarios._normal sampler and reproducibility guarantees."""

from __future__ import annotations

import random

import numpy as np
import pytest

from nanobook.scenarios import (
    _make_pure_rng,
    _normal,
    _normal_box_muller,
    monte_carlo_stock_valuation,
)

# MC advanced model: five normals per path (path-major order on pure path).
_MC_ADVANCED_NORMAL_PARAMS = (
    (0.16, 0.06),
    (0.02, 0.03),
    (22.0, 3.5),
    (-0.03, 0.11),
    (0.0, 0.04),
)


def _mc_advanced_normal_sequence(rng, n_paths: int) -> list[float]:
    """Replay the pure-path advanced driver normal draw order."""
    out: list[float] = []
    for _ in range(n_paths):
        for mu, sigma in _MC_ADVANCED_NORMAL_PARAMS:
            out.append(_normal(rng, mu, sigma))
    return out


def test_normal_pure_random_reproducible():
    seed = 42
    seq_a = [_normal(random.Random(seed), 1.0, 0.5) for _ in range(50)]
    seq_b = [_normal(random.Random(seed), 1.0, 0.5) for _ in range(50)]
    assert seq_a == seq_b


def test_normal_pure_random_matches_gauss():
    assert _normal(random.Random(7), 2.5, 0.3) == random.Random(7).gauss(2.5, 0.3)


def test_normal_sigma_zero_returns_mu():
    rng = random.Random(0)
    assert _normal(rng, 3.14, 0.0) == 3.14


def test_normal_numpy_default_rng_reproducible():
    seed = 42
    rng_a = np.random.default_rng(seed)
    rng_b = np.random.default_rng(seed)
    seq_a = [_normal(rng_a, 0.16, 0.06) for _ in range(50)]
    seq_b = [_normal(rng_b, 0.16, 0.06) for _ in range(50)]
    assert seq_a == seq_b


def test_normal_numpy_matches_direct_normal_calls():
    seed = 99
    helper_rng = np.random.default_rng(seed)
    direct_rng = np.random.default_rng(seed)
    via_helper = [_normal(helper_rng, 0.18, 0.38) for _ in range(30)]
    via_direct = [float(direct_rng.normal(0.18, 0.38)) for _ in range(30)]
    assert via_helper == via_direct


def test_normal_numpy_mc_advanced_draw_order_reproducible():
    """Within MC use: advanced pure-path draw order is seed-stable with default_rng."""
    seed = 42
    n_paths = 8
    seq_a = _mc_advanced_normal_sequence(np.random.default_rng(seed), n_paths)
    seq_b = _mc_advanced_normal_sequence(np.random.default_rng(seed), n_paths)
    assert seq_a == seq_b


def test_normal_pure_and_numpy_streams_differ():
    """Document MT19937 vs PCG64: same integer seed, different streams."""
    seed = 42
    py_seq = [_normal(random.Random(seed), 0.0, 1.0) for _ in range(10)]
    np_seq = [_normal(np.random.default_rng(seed), 0.0, 1.0) for _ in range(10)]
    assert py_seq != np_seq


def test_normal_box_muller_fallback_reproducible():
    seed = 11
    seq_a = [_normal_box_muller(random.Random(seed), 0.0, 1.0) for _ in range(20)]
    seq_b = [_normal_box_muller(random.Random(seed), 0.0, 1.0) for _ in range(20)]
    assert seq_a == seq_b


def test_monte_carlo_pure_path_reproducible():
    base = dict(
        ticker="TEST",
        current_price=100.0,
        n_paths=25,
        version="advanced",
    )
    r1 = monte_carlo_stock_valuation(**base, seed=_make_pure_rng(42))
    r2 = monte_carlo_stock_valuation(**base, seed=_make_pure_rng(42))
    assert r1.terminal_prices == r2.terminal_prices
    assert r1.summary == r2.summary


def test_monte_carlo_numpy_path_reproducible():
    kwargs = dict(
        ticker="TEST",
        current_price=100.0,
        n_paths=25,
        seed=42,
        version="simple",
    )
    r1 = monte_carlo_stock_valuation(**kwargs)
    r2 = monte_carlo_stock_valuation(**kwargs)
    assert r1.terminal_prices == r2.terminal_prices


def test_monte_carlo_numpy_matches_default_rng_reference():
    """Numpy MC fast path matches hand-rolled default_rng simple GBM."""
    seed = 42
    n_paths = 10
    current_price = 100.0
    horizon = 1.0
    expected_annual_return = 0.18
    annual_vol = 0.38

    res = monte_carlo_stock_valuation(
        "REF",
        current_price,
        version="simple",
        n_paths=n_paths,
        horizon=horizon,
        seed=seed,
        expected_annual_return=expected_annual_return,
        annual_vol=annual_vol,
    )

    rng = np.random.default_rng(seed)
    drift = (expected_annual_return - 0.5 * annual_vol**2) * horizon
    diffusion = annual_vol * np.sqrt(horizon) * rng.standard_normal(n_paths)
    expected = (current_price * np.exp(drift + diffusion)).tolist()
    assert res.terminal_prices == expected