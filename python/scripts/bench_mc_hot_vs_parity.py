#!/usr/bin/env python3
"""Compare native ChaCha20 hot path vs audit parity baseline (median wall time).

Hot path: ``monte_carlo_stock_valuation`` (ChaCha20 native PyO3).
Parity baseline: ``monte_carlo_stock_valuation_parity`` (pure-Python numpy audit oracle).
Optional bridge: set ``MC_NUMPY_BRIDGE=1`` for ADR-0006 NumPy-draw → Rust-math timing.

Requires a release extension build:
  VIRTUAL_ENV=.../nanotrade/.venv maturin develop --features scenarios --release
"""

from __future__ import annotations

import os
import statistics
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
# Prefer nanotrade venv (release extension); fall back to local import path.
for py in (
    ROOT / "nanotrade" / ".venv" / "bin" / "python",
    Path(sys.executable),
):
    if py.exists():
        sys.path.insert(0, str(ROOT / "nanobook" / "python"))
        break

import nanobook.scenarios as sc  # noqa: E402

PARAMS = dict(
    ticker="XYZ",
    current_price=74.0,
    version="advanced",
    n_paths=5000,
    seed=42,
    gp_growth_mean=0.16,
    multiple_mean=22.0,
    macro_shock_mean=-0.03,
)
N = 30
MIN_RATIO = 10.0


def _median_ms(fn) -> float:
    times: list[float] = []
    for _ in range(N):
        t0 = time.perf_counter()
        fn()
        times.append((time.perf_counter() - t0) * 1000.0)
    return statistics.median(times)


def main() -> int:
    if not sc._HAS_RUST_SCENARIOS:
        print("extension not built; skip")
        return 1

    hot_ms = _median_ms(lambda: sc.monte_carlo_stock_valuation(**PARAMS))
    parity_ms = _median_ms(lambda: sc.monte_carlo_stock_valuation_parity(**PARAMS))
    ratio = parity_ms / hot_ms if hot_ms > 0 else float("inf")
    print(
        f"hot_median_ms={hot_ms:.3f} parity_median_ms={parity_ms:.3f} ratio={ratio:.1f}x"
    )

    old = os.environ.get("MC_NUMPY_BRIDGE")
    try:
        os.environ["MC_NUMPY_BRIDGE"] = "1"
        import importlib

        importlib.reload(sc)
        bridge_ms = _median_ms(
            lambda: sc.monte_carlo_stock_valuation_parity(**PARAMS)
        )
    finally:
        if old is None:
            os.environ.pop("MC_NUMPY_BRIDGE", None)
        else:
            os.environ["MC_NUMPY_BRIDGE"] = old
        import importlib

        importlib.reload(sc)
    bridge_ratio = bridge_ms / hot_ms if hot_ms > 0 else float("inf")
    print(
        f"numpy_bridge_median_ms={bridge_ms:.3f} bridge_ratio={bridge_ratio:.1f}x"
    )

    if ratio < MIN_RATIO:
        print(f"WARN: parity/hot ratio {ratio:.1f}x < {MIN_RATIO:.0f}x target")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())