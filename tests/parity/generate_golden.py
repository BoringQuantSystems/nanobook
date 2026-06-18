"""Reference-parity golden fixture generator.

Produces tests/parity/golden.json from scipy, TA-Lib, and quantstats.

This script is run MANUALLY, not in CI. The generated JSON is
checked into the repository and read-only from the Rust test side.
Regenerate only when reference library versions in
tests/parity/requirements.txt are deliberately bumped.

When adding indicator X:
  1. Append an entry to tests/parity/indicator_registry.json
  2. Run this generator
  3. reference_parity.rs and test_ref_indicators.py auto-cover the entry

Usage:
    uv pip install -r tests/parity/requirements.txt
    uv run python tests/parity/generate_golden.py [--verbose]

System prerequisites:
    macOS:  brew install ta-lib
    Ubuntu: apt-get install libta-lib-dev
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

import numpy as np
import quantstats as qs
import scipy.stats as sps
import talib

SEED = 42
N = 500
RETURN_SCALE = 0.01

PARITY_DIR = Path(__file__).parent
REGISTRY_PATH = PARITY_DIR / "indicator_registry.json"


def to_jsonable(value: Any) -> Any:
    """Convert numpy scalars / arrays to JSON-compatible primitives.

    NaN and +/-Inf become None (JSON null) so the Rust side can
    distinguish "not yet computed" (first `period - 1` indicator
    values) from real finite outputs.
    """
    if isinstance(value, (float, np.floating)):
        f = float(value)
        if not np.isfinite(f):
            return None
        return f
    if isinstance(value, (int, np.integer)):
        return int(value)
    if isinstance(value, (list, np.ndarray)):
        return [to_jsonable(v) for v in value]
    return value


def load_registry() -> list[dict[str, Any]]:
    data = json.loads(REGISTRY_PATH.read_text())
    return data["indicators"]


def call_talib(
    entry: dict[str, Any],
    close: np.ndarray,
    highs: np.ndarray,
    lows: np.ndarray,
    volume: np.ndarray,
    verbose: bool,
) -> dict[str, list[Any]]:
    func_name = entry["talib_func"]
    func = getattr(talib, func_name)
    args = dict(entry.get("talib_args", {}))
    input_type = entry["input_type"]

    if input_type == "close":
        result = func(close, **args)
    elif input_type == "ohlc":
        result = func(highs, lows, close, **args)
    elif input_type == "close_volume":
        result = func(close, volume, **args)
    elif input_type == "ohlcv":
        result = func(highs, lows, close, volume, **args)
    else:
        raise ValueError(f"unknown input_type {input_type!r} for {entry['name']}")

    if "golden_keys" in entry:
        keys = entry["golden_keys"]
        if not isinstance(result, tuple):
            raise TypeError(f"{func_name} expected tuple output, got {type(result)}")
        if len(result) != len(keys):
            raise ValueError(
                f"{func_name}: {len(result)} outputs vs {len(keys)} golden_keys"
            )
        out = {key: to_jsonable(arr) for key, arr in zip(keys, result, strict=True)}
    else:
        key = entry["golden_key"]
        out = {key: to_jsonable(result)}

    if verbose:
        for key, arr in out.items():
            finite = sum(1 for v in arr if v is not None)
            print(f"  generated talib/{key}: {finite}/{len(arr)} finite values")

    return out


def generate_talib_section(
    close: np.ndarray,
    highs: np.ndarray,
    lows: np.ndarray,
    volume: np.ndarray,
    verbose: bool,
) -> dict[str, list[Any]]:
    registry = load_registry()
    talib_out: dict[str, list[Any]] = {}
    if verbose:
        print(f"Generating {len(registry)} registry entries...")
    for entry in registry:
        label = entry.get("golden_key") or ",".join(entry["golden_keys"])
        if verbose:
            print(f"- {entry['name']} ({label})")
        talib_out.update(call_talib(entry, close, highs, lows, volume, verbose))
    return talib_out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--verbose", "-v", action="store_true", help="Print per-key generation stats"
    )
    args = parser.parse_args()

    # Seeded inputs. NEVER change SEED or N without a deliberate
    # decision; every regenerated value depends on them.
    rng = np.random.default_rng(SEED)
    returns = rng.standard_normal(N) * RETURN_SCALE

    # Synthetic OHLC series derived from the same returns. High/low
    # bands are small perturbations around close so that ATR has
    # non-trivial signal.
    close = 100.0 * np.cumprod(1.0 + returns)
    highs = close * (1.0 + 0.002 * rng.random(N))
    lows = close * (1.0 - 0.002 * rng.random(N))
    volume = rng.integers(1000, 10000, N).astype(float)

    # --- scipy.stats ---
    spearman_self = sps.spearmanr(returns, returns).statistic
    shuffled = np.roll(returns, 7)
    spearman_shuffled = sps.spearmanr(returns, shuffled).statistic

    # --- TA-Lib (registry-driven) ---
    talib_section = generate_talib_section(close, highs, lows, volume, args.verbose)

    # --- quantstats ---
    import pandas as pd

    idx = pd.date_range("2023-01-01", periods=N, freq="B")
    returns_series = pd.Series(returns, index=idx)
    qs_sharpe = qs.stats.sharpe(returns_series, rf=0.0, periods=252, annualize=True)
    qs_sortino = qs.stats.sortino(returns_series, rf=0.0, periods=252, annualize=True)
    qs_max_dd = qs.stats.max_drawdown(returns_series)
    qs_cvar_95_parametric = qs.stats.expected_shortfall(
        returns_series, confidence=0.95
    )

    alpha = 0.05
    sorted_returns = np.sort(returns)
    tail_n = int(np.ceil(N * alpha))
    empirical_cvar_95 = float(sorted_returns[:tail_n].mean())

    import scipy

    versions = {
        "numpy": np.__version__,
        "scipy": scipy.__version__,
        "talib": getattr(talib, "__version__", "unknown"),
        "quantstats": getattr(qs, "__version__", "unknown"),
        "pandas": pd.__version__,
    }

    out = {
        "_meta": {
            "seed": SEED,
            "n": N,
            "return_scale": RETURN_SCALE,
            "versions": versions,
            "registry": str(REGISTRY_PATH.name),
            "note": (
                "Regenerate only when requirements.txt is deliberately "
                "bumped. See tests/parity/README.md."
            ),
        },
        "inputs": {
            "returns": to_jsonable(returns),
            "close": to_jsonable(close),
            "highs": to_jsonable(highs),
            "lows": to_jsonable(lows),
            "volume": to_jsonable(volume),
        },
        "scipy": {
            "spearman_self_correlation": to_jsonable(spearman_self),
            "spearman_shuffled_correlation": to_jsonable(spearman_shuffled),
        },
        "talib": talib_section,
        "quantstats": {
            "sharpe_annual_252": to_jsonable(qs_sharpe),
            "sortino_annual_252": to_jsonable(qs_sortino),
            "max_drawdown": to_jsonable(qs_max_dd),
            "cvar_95_parametric": to_jsonable(qs_cvar_95_parametric),
        },
        "empirical": {
            "cvar_95": empirical_cvar_95,
        },
    }

    path = PARITY_DIR / "golden.json"
    path.write_text(json.dumps(out, indent=2) + "\n")

    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    print(f"Wrote {path}")
    print(f"sha256: {digest}")
    print(f"Reference versions: {versions}")
    print(f"TA-Lib keys: {sorted(talib_section.keys())}")
    return 0


if __name__ == "__main__":
    sys.exit(main())