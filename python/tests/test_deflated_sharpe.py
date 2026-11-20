"""Deflated Sharpe Ratio against Bailey and López de Prado (2014).

Needs no third-party package, so it runs in every CI job (with and without numpy).
"""

import math
from statistics import NormalDist

import pytest

import nanobook


def test_paper_worked_example():
    """Worked example, p. 99: DSR 0.9004 (N=100), 0.9505 (N=46), 0.9505 (N=88, normal)."""
    sr = 2.5 / math.sqrt(250)
    v = 0.5 / 250
    assert nanobook.py_deflated_sharpe(sr, 100, 1250, v, -3.0, 10.0) == pytest.approx(0.9004, abs=5e-5)
    assert nanobook.py_deflated_sharpe(sr, 46, 1250, v, -3.0, 10.0) == pytest.approx(0.9505, abs=5e-5)
    assert nanobook.py_deflated_sharpe(sr, 88, 1250, v, 0.0, 3.0) == pytest.approx(0.9505, abs=5e-5)


def test_single_trial_is_psr_against_zero():
    """One trial: Phi(SR * sqrt(T - 1) / sqrt(1 + SR^2 / 2)) for normal returns.

    The Rust normal CDF (Abramowitz and Stegun 7.1.26) is accurate to about 7.5e-8,
    hence the 1e-7 tolerance against the exact CDF.
    """
    want = NormalDist().cdf(0.1 * 10.0 / math.sqrt(1.0 + 0.5 * 0.01))
    got = nanobook.py_deflated_sharpe(0.1, 1, 101, 0.0, 0.0, 3.0)
    assert got == pytest.approx(want, abs=1e-7)


def test_invalid_inputs_are_nan():
    assert math.isnan(nanobook.py_deflated_sharpe(0.1, 0, 100, 0.01, 0.0, 3.0))
    assert math.isnan(nanobook.py_deflated_sharpe(0.1, 10, 1, 0.01, 0.0, 3.0))
