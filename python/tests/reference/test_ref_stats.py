"""Reference tests: nanobook stats vs SciPy.

Validates Spearman correlation and quintile spread against scipy/numpy.

Dev dependencies: scipy, numpy
"""

import numpy as np
import pytest

try:
    from scipy import stats

    HAS_SCIPY = True
except ImportError:
    HAS_SCIPY = False

import nanobook

pytestmark = pytest.mark.skipif(not HAS_SCIPY, reason="scipy not installed")


class TestSpearmanReference:
    """Validate nanobook Spearman against SciPy."""

    ATOL = 1e-10

    def test_random_data(self, rng):
        x = rng.standard_normal(100).tolist()
        y = rng.standard_normal(100).tolist()
        ref_corr, ref_p = stats.spearmanr(x, y)
        got_corr, got_p = nanobook.py_spearman(x, y)
        assert abs(got_corr - ref_corr) < self.ATOL
        assert abs(got_p - ref_p) < self.ATOL

    def test_perfect_positive(self):
        x = list(range(50))
        y = list(range(50))
        ref_corr, ref_p = stats.spearmanr(x, y)
        got_corr, got_p = nanobook.py_spearman(
            [float(v) for v in x], [float(v) for v in y]
        )
        assert abs(got_corr - 1.0) < 1e-10
        assert got_p < 1e-10

    def test_perfect_negative(self):
        x = list(range(50))
        y = list(range(49, -1, -1))
        got_corr, got_p = nanobook.py_spearman(
            [float(v) for v in x], [float(v) for v in y]
        )
        assert abs(got_corr - (-1.0)) < 1e-10

    def test_tied_values(self):
        x = [1.0, 1.0, 2.0, 2.0, 3.0]
        y = [5.0, 4.0, 3.0, 2.0, 1.0]
        ref_corr, ref_p = stats.spearmanr(x, y)
        got_corr, got_p = nanobook.py_spearman(x, y)
        assert abs(got_corr - ref_corr) < self.ATOL
        assert abs(got_p - ref_p) < 1e-6  # p-value tolerance slightly wider

    def test_small_n(self):
        x = [1.0, 2.0, 3.0]
        y = [3.0, 1.0, 2.0]
        ref_corr, ref_p = stats.spearmanr(x, y)
        got_corr, got_p = nanobook.py_spearman(x, y)
        assert abs(got_corr - ref_corr) < self.ATOL


class TestQuintileSpreadReference:
    """Validate quintile spread against manual numpy computation."""

    def test_known_spread(self):
        scores = [float(i) for i in range(100)]
        returns = [float(i) * 0.001 for i in range(100)]
        got = nanobook.py_quintile_spread(scores, returns, 5)

        # Manual: bottom 20 = 0..19, top 20 = 80..99
        bottom = np.mean([i * 0.001 for i in range(20)])
        top = np.mean([i * 0.001 for i in range(80, 100)])
        expected = top - bottom
        assert abs(got - expected) < 1e-12

    def test_inverse_scores(self):
        scores = [float(i) for i in range(100)]
        returns = [float(99 - i) * 0.001 for i in range(100)]
        got = nanobook.py_quintile_spread(scores, returns, 5)
        assert got < 0.0  # inverse → negative spread


class TestDeflatedSharpe:
    """Deflated Sharpe Ratio against the worked example of Bailey and
    López de Prado (2014), p. 99."""

    def test_paper_example(self):
        sr = 2.5 / 250**0.5
        v = 0.5 / 250
        assert nanobook.py_deflated_sharpe(sr, 100, 1250, v, -3.0, 10.0) == pytest.approx(0.9004, abs=5e-5)
        assert nanobook.py_deflated_sharpe(sr, 46, 1250, v, -3.0, 10.0) == pytest.approx(0.9505, abs=5e-5)
        assert nanobook.py_deflated_sharpe(sr, 88, 1250, v, 0.0, 3.0) == pytest.approx(0.9505, abs=5e-5)

    def test_single_trial_is_psr_against_zero(self):
        from statistics import NormalDist
        want = NormalDist().cdf(0.1 * 10.0 / (1.0 + 0.5 * 0.01) ** 0.5)
        got = nanobook.py_deflated_sharpe(0.1, 1, 101, 0.0, 0.0, 3.0)
        assert got == pytest.approx(want, abs=1e-12)

    def test_invalid_inputs(self):
        assert np.isnan(nanobook.py_deflated_sharpe(0.1, 0, 100, 0.01, 0.0, 3.0))
        assert np.isnan(nanobook.py_deflated_sharpe(0.1, 10, 1, 0.01, 0.0, 3.0))
