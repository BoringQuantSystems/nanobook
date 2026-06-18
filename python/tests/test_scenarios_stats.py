"""Parity tests for pure-stdlib percentile/median vs numpy."""

from __future__ import annotations

import pytest

from nanobook.scenarios import _pure_median, _pure_percentile

np = pytest.importorskip("numpy")


@pytest.mark.parametrize(
    "values",
    [
        [1.0, 2.0, 3.0, 4.0, 5.0],
        [10.0, 20.0, 30.0, 40.0, 50.0, 60.0],
        [0.5, 1.5, 2.5, 3.5],
        [100.0],
        [3.0, 3.0, 3.0, 3.0],
    ],
)
def test_pure_median_matches_numpy(values):
    assert _pure_median(values) == pytest.approx(float(np.median(values)))


@pytest.mark.parametrize("q", [0.0, 0.05, 0.25, 0.5, 0.75, 0.95, 1.0])
def test_pure_percentile_matches_numpy_default(q):
    values = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]
    expected = float(np.percentile(values, q * 100))
    assert _pure_percentile(values, q) == pytest.approx(expected, rel=1e-9, abs=1e-9)


def test_pure_percentile_empty():
    assert _pure_percentile([], 0.5) == 0.0
    assert _pure_median([]) == 0.0