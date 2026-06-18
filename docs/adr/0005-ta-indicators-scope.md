# ADR-0005: ta-indicators-scope

- **Date:** 2026-06-18
- **Status:** Accepted

## Context

nanobook shipped with a tiny indicator surface (sma, ema, rsi, macd, bbands, atr). nanotrade Strategy Spec v2 manifests need richer boolean filters and feature columns (stoch, adx, volume confirmation) without a runtime dependency on the TA-Lib C library or Python `talib` in the backtest path. Golden parity against pinned TA-Lib outputs is load-bearing for reproducibility.

## Decision

1. **Scope:** Implement a curated set of ~25–35 high-signal TA-Lib functions for equity daily-bar strategy work. Full classification coverage of all 158 `talib.get_functions()` entries lives in `docs/ta-lib-full-coverage-matrix.md`; unimplemented functions are explicitly deferred with rationale.

2. **Numeric conventions (must match TA-Lib):**
   - Wilder's smoothing for RSI, ATR, ADX family: `alpha = 1/period`
   - Standard EMA for MACD, ADOSC: `alpha = 2/(period+1)`
   - Bollinger bands: population stddev (`ddof=0`)
   - Leading NaN counts follow TA-Lib lookback (e.g. TRANGE valid from index 1)

3. **Harness:** `tests/parity/indicator_registry.json` is the single source for golden generation (`generate_golden.py`), Rust parity (`reference_parity.rs`), and Python reference tests. Adding an indicator is mechanical: Rust fn → py bind → registry entry → regen golden.

4. **Discoverability:** `indicators::list_supported()` / `py_list_supported_indicators()` is the public contract; `has_parity: true` marks golden-backed entries.

5. **Input model:** Close-only, OHLC, and OHLCV (+ `close_volume` for OBV) input types in the registry. nanotrade `compute_indicator` fetches series via `_ohlc_series`; volume indicators require a `volume` column in prices.

6. **Cross-repo workflow:** Beads and orchestration in boringQuant; implementation on `nanobook:dev`; nanotrade tests via sibling editable install; PR to nanobook main only after integration green; boringQuant submodule bump post-merge.

## Alternatives Considered

1. **Implement all 158 TA-Lib functions** — Rejected: candlestick patterns and math transforms add maintenance without improving v2 manifest expressiveness.
2. **Depend on external TA-Lib at runtime** — Rejected: C library drift, deployment friction, no in-kernel backtest path.
3. **Implement indicators in nanotrade Python** — Rejected: duplicates logic; breaks single-source-of-truth for backtest and research parity.

## Consequences

- New indicators are fast to add (<30 min mechanical path) once registry entry exists.
- Deferred TA-Lib functions remain visible in the matrix; no silent gaps.
- Volume/OHLC indicators require lake price schema with `volume` present.

## Evidence

- Registry: `tests/parity/indicator_registry.json` (35 golden keys, 25 unique functions)
- Classification matrix: `docs/ta-lib-full-coverage-matrix.md` (158/158 rows)
- Parity gates: `cargo test --test reference_parity`, `pytest tests/reference/test_ref_indicators.py`
- Epic beads: `bq-nb-ta-ind-epic-ngrb` in boringQuant `.beads/issues.jsonl`