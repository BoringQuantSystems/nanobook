"""Reference tests: nanobook indicators vs TA-Lib.

Validates that nanobook's Rust indicators produce numerically
identical results to TA-Lib's C implementation.

Dev dependencies: ta-lib (requires C library: `brew install ta-lib`)
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import pytest

try:
    import talib

    HAS_TALIB = True
except ImportError:
    HAS_TALIB = False

import nanobook

pytestmark = pytest.mark.skipif(not HAS_TALIB, reason="ta-lib not installed")

ATOL = 1e-10
REGISTRY_PATH = Path(__file__).resolve().parents[3] / "tests" / "parity" / "indicator_registry.json"


def load_registry() -> list[dict]:
    data = json.loads(REGISTRY_PATH.read_text())
    return data["indicators"]


def call_talib(
    entry: dict,
    close: np.ndarray,
    highs: np.ndarray,
    lows: np.ndarray,
    volume: np.ndarray,
):
    func = getattr(talib, entry["talib_func"])
    args = dict(entry.get("talib_args", {}))
    if entry["input_type"] == "close":
        return func(close, **args)
    if entry["input_type"] == "ohlc":
        return func(highs, lows, close, **args)
    if entry["input_type"] == "close_volume":
        return func(close, volume, **args)
    if entry["input_type"] == "ohlcv":
        return func(highs, lows, close, volume, **args)
    raise ValueError(f"unknown input_type: {entry['input_type']}")


def call_nanobook(entry: dict, close: list, highs: list, lows: list, volume: list):
    rust_fn = entry["rust_fn"]
    args = entry["rust_args"]
    if rust_fn == "sma":
        return nanobook.py_sma(close, args[0])
    if rust_fn == "ema":
        return nanobook.py_ema(close, args[0])
    if rust_fn == "rsi":
        return nanobook.py_rsi(close, args[0])
    if rust_fn == "macd":
        return nanobook.py_macd(close, args[0], args[1], args[2])
    if rust_fn == "bbands":
        return nanobook.py_bbands(close, args[0], args[1], args[2])
    if rust_fn == "atr":
        return nanobook.py_atr(highs, lows, close, args[0])
    if rust_fn == "stoch":
        return nanobook.py_stoch(highs, lows, close, args[0], args[1], args[2])
    if rust_fn == "stochf":
        return nanobook.py_stochf(highs, lows, close, args[0], args[1])
    if rust_fn == "stochrsi":
        return nanobook.py_stochrsi(close, args[0], args[1], args[2])
    if rust_fn == "plus_di":
        return nanobook.py_plus_di(highs, lows, close, args[0])
    if rust_fn == "minus_di":
        return nanobook.py_minus_di(highs, lows, close, args[0])
    if rust_fn == "dx":
        return nanobook.py_dx(highs, lows, close, args[0])
    if rust_fn == "adx":
        return nanobook.py_adx(highs, lows, close, args[0])
    if rust_fn == "cci":
        return nanobook.py_cci(highs, lows, close, args[0])
    if rust_fn == "willr":
        return nanobook.py_willr(highs, lows, close, args[0])
    if rust_fn == "ultosc":
        return nanobook.py_ultosc(highs, lows, close, args[0], args[1], args[2])
    if rust_fn == "mom":
        return nanobook.py_mom(close, args[0])
    if rust_fn == "roc":
        return nanobook.py_roc(close, args[0])
    if rust_fn == "rocp":
        return nanobook.py_rocp(close, args[0])
    if rust_fn == "rocr":
        return nanobook.py_rocr(close, args[0])
    if rust_fn == "obv":
        return nanobook.py_obv(close, volume)
    if rust_fn == "ad":
        return nanobook.py_ad(highs, lows, close, volume)
    if rust_fn == "adosc":
        return nanobook.py_adosc(highs, lows, close, volume, args[0], args[1])
    if rust_fn == "natr":
        return nanobook.py_natr(highs, lows, close, args[0])
    if rust_fn == "trange":
        return nanobook.py_trange(highs, lows, close)
    raise ValueError(f"unknown rust_fn: {rust_fn}")


def assert_series_parity(ref, got, label: str) -> None:
    ref_arr = np.asarray(ref, dtype=float)
    got_arr = np.asarray(got, dtype=float)
    valid = ~np.isnan(ref_arr)
    np.testing.assert_allclose(
        got_arr[valid], ref_arr[valid], atol=ATOL, err_msg=f"{label} mismatch"
    )
    assert int(np.isnan(ref_arr).sum()) == int(np.isnan(got_arr).sum()), (
        f"{label}: NaN count ref={np.isnan(ref_arr).sum()} "
        f"got={np.isnan(got_arr).sum()}"
    )


@pytest.mark.parametrize("entry", load_registry(), ids=lambda e: e.get("golden_key") or e["name"])
class TestRegistryParity:
    """Golden-registry entries must match TA-Lib on synthetic OHLC."""

    def test_matches_talib(self, entry, random_close, random_ohlc, random_volume):
        high, low, close = random_ohlc
        ref = call_talib(entry, close, high, low, random_volume)
        got = call_nanobook(
            entry,
            close.tolist(),
            high.tolist(),
            low.tolist(),
            random_volume.tolist(),
        )

        if "golden_keys" in entry:
            assert isinstance(ref, tuple)
            assert isinstance(got, tuple)
            for key, ref_series, got_series in zip(
                entry["golden_keys"], ref, got, strict=True
            ):
                assert_series_parity(ref_series, got_series, key)
        else:
            assert_series_parity(ref, got, entry["golden_key"])


class TestListSupported:
    def test_lists_group_a(self):
        supported = nanobook.list_supported_indicators()
        names = {item["name"] for item in supported}
        assert names == {
            "sma", "ema", "rsi", "macd", "bbands", "atr",
            "stoch", "stochf", "stochrsi",
            "adx", "plus_di", "minus_di", "dx",
            "cci", "willr", "ultosc",
            "mom", "roc", "rocp", "rocr",
            "obv", "ad", "adosc", "natr", "trange",
        }
        assert all(item["has_parity"] for item in supported)


class TestRSIEdgeCases:
    def test_monotonic_up(self):
        close = np.arange(1.0, 101.0)
        ref = talib.RSI(close, timeperiod=14)
        got = nanobook.py_rsi(close.tolist(), 14)
        valid = ~np.isnan(ref)
        np.testing.assert_allclose(np.array(got)[valid], ref[valid], atol=ATOL)
        assert got[-1] > 99.0

    def test_monotonic_down(self):
        close = np.arange(100.0, 0.0, -1.0)
        ref = talib.RSI(close, timeperiod=14)
        got = nanobook.py_rsi(close.tolist(), 14)
        valid = ~np.isnan(ref)
        np.testing.assert_allclose(np.array(got)[valid], ref[valid], atol=ATOL)

    def test_constant_price(self):
        close = np.full(100, 50.0)
        ref = talib.RSI(close, timeperiod=14)
        got = nanobook.py_rsi(close.tolist(), 14)
        valid = ~np.isnan(ref)
        np.testing.assert_allclose(np.array(got)[valid], ref[valid], atol=ATOL)


class TestBBandsOrdering:
    def test_ordering(self, random_close):
        upper, middle, lower = nanobook.py_bbands(
            random_close.tolist(), 20, 2.0, 2.0
        )
        for i in range(19, len(upper)):
            if not np.isnan(upper[i]):
                assert lower[i] <= middle[i] <= upper[i], f"ordering violated at {i}"