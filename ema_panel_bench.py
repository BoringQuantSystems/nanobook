"""Time 1-D py_ema per name vs one (T, N) ema_panel pass.

Writes the same lines to stdout and ema_panel_bench.log.
"""

from __future__ import annotations

import time
from pathlib import Path

import nanobook

_LOG = Path(__file__).with_suffix(".log")
_PERIOD = 21
_SIZES = ((2000, 200), (2000, 1000))


def _panel(n_time: int, n_names: int) -> list[float]:
    close: list[float] = []
    for t in range(n_time):
        for name in range(n_names):
            close.append(100.0 + t * 0.01 + name * 0.1)
    return close


def _line(msg: str, sink: list[str]) -> None:
    print(msg)
    sink.append(msg)


def main() -> None:
    lines: list[str] = []
    warm = _panel(80, 3)
    nanobook.py_ema(warm[:80], _PERIOD)
    nanobook.py_ema_panel(warm, 3, _PERIOD)
    nanobook.py_ema_panel_by_column(warm, 3, _PERIOD)
    nanobook.py_sma(warm[:80], _PERIOD)
    nanobook.py_sma_panel(warm, 3, _PERIOD)
    nanobook.py_sma_panel_by_column(warm, 3, _PERIOD)
    for n_time, n_names in _SIZES:
        close = _panel(n_time, n_names)
        t0 = time.perf_counter()
        for name in range(n_names):
            col = [close[t * n_names + name] for t in range(n_time)]
            nanobook.py_ema(col, _PERIOD)
        ema_loop_s = time.perf_counter() - t0
        t0 = time.perf_counter()
        nanobook.py_ema_panel_by_column(close, n_names, _PERIOD)
        ema_col_s = time.perf_counter() - t0
        t0 = time.perf_counter()
        nanobook.py_ema_panel(close, n_names, _PERIOD)
        ema_outer_s = time.perf_counter() - t0
        t0 = time.perf_counter()
        for name in range(n_names):
            col = [close[t * n_names + name] for t in range(n_time)]
            nanobook.py_sma(col, _PERIOD)
        sma_loop_s = time.perf_counter() - t0
        t0 = time.perf_counter()
        nanobook.py_sma_panel_by_column(close, n_names, _PERIOD)
        sma_col_s = time.perf_counter() - t0
        t0 = time.perf_counter()
        nanobook.py_sma_panel(close, n_names, _PERIOD)
        sma_outer_s = time.perf_counter() - t0
        _line(
            f"T={n_time} N={n_names} period={_PERIOD} "
            f"py_ema_loop={ema_loop_s:.6f} s ema_by_column={ema_col_s:.6f} s ema_panel={ema_outer_s:.6f} s "
            f"py_sma_loop={sma_loop_s:.6f} s sma_by_column={sma_col_s:.6f} s sma_panel={sma_outer_s:.6f} s",
            lines,
        )
    _LOG.write_text("\n".join(lines) + "\n", encoding="utf-8")
    _line(f"wrote {_LOG}", lines)


if __name__ == "__main__":
    main()
