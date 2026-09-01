"""Which element quantile() actually returns.

nanobook's Python layer is at 100% line coverage, and two things about this
one-line index calculation could still be changed without a test failing:

    idx = max(0, min(len(s) - 1, int(q * (len(s) - 1))))

Changing `q * (len(s) - 1)` to `q * len(s)` shifts the answer by one element
for most inputs, and widening the upper clamp turns an out-of-range quantile
into an IndexError. Both survived the whole suite.

This matters because p10_price and p90_price are reported numbers. A quantile
off by one element is a wrong figure that still looks entirely plausible.
"""

from __future__ import annotations

import pytest

from nanobook.scenarios import MonteCarloResult


def _result(prices: list[float]) -> MonteCarloResult:
    return MonteCarloResult(
        ticker="TEST",
        method="Simple GBM",
        horizon_years=1.0,
        current_price=100.0,
        terminal_prices=prices,
        summary={},
    )


# Ten values, so the index arithmetic is visible: q * (n - 1) uses 9, and the
# wrong version q * n would use 10.
_TEN = [float(10 * i) for i in range(1, 11)]  # 10.0 .. 100.0


@pytest.mark.parametrize(
    ("q", "expected"),
    [
        (0.0, 10.0),  # int(0.0 * 9) = 0
        (0.1, 10.0),  # int(0.9)     = 0
        (0.2, 20.0),  # int(1.8)     = 1
        (0.5, 50.0),  # int(4.5)     = 4, NOT 5
        (0.9, 90.0),  # int(8.1)     = 8
        (1.0, 100.0),  # int(9.0)    = 9, the last element
    ],
)
def test_the_quantile_index_is_scaled_by_one_less_than_the_count(
    q: float, expected: float
) -> None:
    """The scale is len - 1, so q = 1.0 lands exactly on the last element.

    Scaling by len instead would put q = 0.5 on the sixth of ten values rather
    than the fifth, and would run off the end at q = 1.0.
    """
    assert _result(_TEN).quantile(q) == expected


def test_the_midpoint_of_an_even_list_takes_the_lower_of_the_two() -> None:
    """This is the case that separates the two scalings.

    With ten values, q = 0.5 gives index 4 and not 5. The function picks an
    actual observation rather than interpolating between the middle pair, so
    the answer is always a price that really occurred.
    """
    assert _result(_TEN).quantile(0.5) == 50.0
    assert _result(_TEN).quantile(0.5) != 60.0


def test_a_quantile_above_one_is_clamped_to_the_last_element() -> None:
    """An out-of-range quantile is clamped, not an error.

    Without the upper clamp this raises IndexError. That would turn a caller's
    bad argument into a crash inside a reporting path, rather than the nearest
    sensible answer.
    """
    assert _result(_TEN).quantile(1.5) == 100.0
    assert _result(_TEN).quantile(99.0) == 100.0


def test_a_negative_quantile_is_clamped_to_the_first_element() -> None:
    """The lower clamp, which stops a negative index reading from the end.

    Python would happily return s[-2] for a negative index, so this would give
    a high price for a quantile below zero — wrong in the worst direction.
    """
    assert _result(_TEN).quantile(-0.5) == 10.0
    assert _result(_TEN).quantile(-99.0) == 10.0


def test_the_input_is_sorted_before_indexing() -> None:
    """Terminal prices arrive in simulation order, not in size order.

    Indexing them unsorted would return whichever path happened to run in that
    position, which is a random number rather than a quantile.
    """
    shuffled = [70.0, 10.0, 100.0, 40.0, 20.0, 90.0, 30.0, 60.0, 80.0, 50.0]

    assert _result(shuffled).quantile(0.0) == 10.0
    assert _result(shuffled).quantile(1.0) == 100.0
    assert _result(shuffled).quantile(0.5) == 50.0


def test_a_single_price_is_returned_for_every_quantile() -> None:
    """With one observation the scale is zero, so every quantile is that value.

    This is the edge where len - 1 is 0 and the multiplication cannot separate
    the quantiles at all. It must not divide by zero or run off the list.
    """
    one = _result([42.0])

    for q in (0.0, 0.1, 0.5, 0.9, 1.0):
        assert one.quantile(q) == 42.0


def test_an_empty_result_has_no_quantile() -> None:
    assert _result([]).quantile(0.5) == 0.0


def test_probability_above_a_level_excludes_the_level_itself() -> None:
    """Above means strictly above. A price equal to the level has not passed it.

    With four prices at 10, 20, 30 and 40, the chance of ending above 20 is one
    half, not three quarters. Counting the equal one would overstate every
    probability the report shows.
    """
    result = _result([10.0, 20.0, 30.0, 40.0])

    assert result.prob_above(20.0) == 0.5
    assert result.prob_above(5.0) == 1.0
    assert result.prob_above(40.0) == 0.0


def test_probability_above_an_empty_result_is_zero() -> None:
    assert _result([]).prob_above(10.0) == 0.0
