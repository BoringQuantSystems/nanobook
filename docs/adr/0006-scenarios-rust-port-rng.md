# ADR 0006: Rust Monte Carlo scenarios — RNG bridge strategy

**Status:** Accepted  
**Date:** 2026-06-19

## Context

Porting `nanobook.scenarios` to Rust requires passing frozen parity tests that pin
NumPy `default_rng` (PCG64) draw order and summary statistics at tight tolerance.

## Decision

1. Feature flag: `scenarios` on `nanobook` and `nanobook-python` (default-on in wheels).
2. **Rust owns** validation, model math, summary stats, and `MonteCarloResult` assembly.
3. **NumPy owns RNG** in the PyO3 path: `numpy.random.default_rng(seed)` draws are
   converted to `Vec<f64>` and fed into Rust. This preserves bit-exact parity with
   existing fixtures without vendoring PCG64 + ziggurat tables.
4. Python `scenarios.py` delegates when the extension is built and `seed` is `int` or
   `None`; `random.Random` seeds and no-numpy environments keep the pure-Python fallback.
5. `rand` / `rand_chacha` / `rand_distr` are wired for future native-Rust-only callers;
   they are not used on the Python hot path today.

## Consequences

- Frozen `scenarios_parity.json` tests pass unchanged.
- Rust-native API can later add ChaCha20 for cross-platform reproducibility outside Python.
- Wheels require NumPy at runtime for the Rust MC path (already a dev/test dependency).