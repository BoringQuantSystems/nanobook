"""Panel EMA last value matches 1-D py_ema on each name."""

from __future__ import annotations

import math

import nanobook


def _last_finite(xs: list[float]) -> float | None:
    for value in reversed(xs):
        if math.isfinite(value):
            return value
    return None


def test_py_ema_panel_last_matches_py_ema() -> None:
    n_time = 80
    n_names = 7
    close = []
    for t in range(n_time):
        for name in range(n_names):
            close.append(100.0 + t * 0.1 + name)
    for period in (8, 12, 21, 26, 50):
        panel = nanobook.py_ema_panel(close, n_names, period)
        for name in range(n_names):
            col = [close[t * n_names + name] for t in range(n_time)]
            one_d = nanobook.py_ema(col, period)
            panel_col = [panel[t * n_names + name] for t in range(n_time)]
            a = _last_finite(panel_col)
            b = _last_finite(one_d)
            assert a is not None and b is not None
            assert abs(a - b) <= 1e-6


def test_py_sma_panel_last_matches_py_sma() -> None:
    n_time = 80
    n_names = 7
    close = []
    for t in range(n_time):
        for name in range(n_names):
            close.append(100.0 + t * 0.1 + name)
    for period in (8, 12, 21, 26, 50):
        panel = nanobook.py_sma_panel(close, n_names, period)
        for name in range(n_names):
            col = [close[t * n_names + name] for t in range(n_time)]
            one_d = nanobook.py_sma(col, period)
            panel_col = [panel[t * n_names + name] for t in range(n_time)]
            a = _last_finite(panel_col)
            b = _last_finite(one_d)
            assert a is not None and b is not None
            assert abs(a - b) <= 1e-6


def _close_panel(n_time: int = 80, n_names: int = 5) -> tuple[int, int, list[float]]:
    close: list[float] = []
    for t in range(n_time):
        for name in range(n_names):
            close.append(100.0 + t * 0.1 + name)
    return n_time, n_names, close


def test_py_rsi_panel_last_matches_py_rsi() -> None:
    n_time, n_names, close = _close_panel()
    for period in (8, 14, 21):
        panel = nanobook.py_rsi_panel(close, n_names, period)
        for name in range(n_names):
            col = [close[t * n_names + name] for t in range(n_time)]
            a = _last_finite([panel[t * n_names + name] for t in range(n_time)])
            b = _last_finite(nanobook.py_rsi(col, period))
            assert a is not None and b is not None
            assert abs(a - b) <= 1e-6


def test_py_macd_panel_last_matches_py_macd() -> None:
    n_time, n_names, close = _close_panel()
    pm, ps, ph = nanobook.py_macd_panel(close, n_names, 12, 26, 9)
    for name in range(n_names):
        col = [close[t * n_names + name] for t in range(n_time)]
        m, s, h = nanobook.py_macd(col, 12, 26, 9)
        for panel, one_d in (
            ([pm[t * n_names + name] for t in range(n_time)], m),
            ([ps[t * n_names + name] for t in range(n_time)], s),
            ([ph[t * n_names + name] for t in range(n_time)], h),
        ):
            a = _last_finite(panel)
            b = _last_finite(one_d)
            assert a is not None and b is not None
            assert abs(a - b) <= 1e-6


def test_py_bbands_panel_last_matches_py_bbands() -> None:
    n_time, n_names, close = _close_panel()
    pu, pm, pl = nanobook.py_bbands_panel(close, n_names, 20, 2.0, 2.0)
    for name in range(n_names):
        col = [close[t * n_names + name] for t in range(n_time)]
        u, m, l = nanobook.py_bbands(col, 20, 2.0, 2.0)
        for panel, one_d in (
            ([pu[t * n_names + name] for t in range(n_time)], u),
            ([pm[t * n_names + name] for t in range(n_time)], m),
            ([pl[t * n_names + name] for t in range(n_time)], l),
        ):
            a = _last_finite(panel)
            b = _last_finite(one_d)
            assert a is not None and b is not None
            assert abs(a - b) <= 1e-6


def test_py_atr_panel_last_matches_py_atr() -> None:
    n_time, n_names, close = _close_panel()
    high = [c + 1.0 for c in close]
    low = [c - 1.0 for c in close]
    panel = nanobook.py_atr_panel(high, low, close, n_names, 14)
    for name in range(n_names):
        c = [close[t * n_names + name] for t in range(n_time)]
        h = [high[t * n_names + name] for t in range(n_time)]
        lo = [low[t * n_names + name] for t in range(n_time)]
        a = _last_finite([panel[t * n_names + name] for t in range(n_time)])
        b = _last_finite(nanobook.py_atr(h, lo, c, 14))
        assert a is not None and b is not None
        assert abs(a - b) <= 1e-6



def test_sma_panel_and_ema_panel_wrappers_match_py_functions() -> None:
    n_time, n_names, close = _close_panel()
    for period in (8, 21):
        for wrapper, raw in (
            (nanobook.sma_panel, nanobook.py_sma_panel),
            (nanobook.ema_panel, nanobook.py_ema_panel),
        ):
            got = wrapper(close, n_names, period)
            want = raw(close, n_names, period)
            assert len(got) == n_time * n_names
            assert _last_finite(got) is not None
            for x, y in zip(got, want, strict=True):
                assert x == y or (math.isnan(x) and math.isnan(y))
