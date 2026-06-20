# ADR 0007: ChaCha20 native hot path for Monte Carlo scenarios

**Status:** Accepted  
**Date:** 2026-06-19  
**Supersedes (partial):** ADR-0006 hot-path RNG only — parity bridge remains for audit

## Context

ADR-0006 wired the Python MC path through NumPy `default_rng` → Rust terminal math to
preserve bit-exact frozen parity fixtures. That bridge copies draws and is ~20× slower
than native ChaCha20 already implemented in `nanobook/src/scenarios.rs`.

Production callers (`calc.engine`, research notebooks) need speed; CI needs the frozen
oracle unchanged.

## Decision

1. **Hot path:** `nanobook.scenarios.monte_carlo_stock_valuation` delegates to PyO3
   `monte_carlo_stock_valuation_native` (ChaCha20 + `rand_distr::Normal`) when the
   extension is built and `seed` is `int` or `None`.
2. **Audit path:** `monte_carlo_stock_valuation_parity` keeps the ADR-0006 NumPy bridge.
   `MC_AUDIT_MODE=1` forces the public entry point onto parity.
3. **Frozen fixtures:** `tests/reference/scenarios_parity.json` tests call `_parity` only.
   Hot path uses statistical equivalence gates (median / tails / binned L1).
4. **Reproducibility:** integer seeds → bitwise-identical native reruns; `seed=None`
   uses `nondeterministic_mc_seed()` (thread_rng) on the native path.

## Alternatives considered

- Keep NumPy bridge as default — rejected: leaves ~20× speed on the table.
- Regenerate frozen fixtures for ChaCha20 — rejected: breaks historical audit trail.

## Consequences

- `nanotrade/calc/scenarios.py` shim picks up speed with no code changes.
- Wheels still require NumPy for the parity PyO3 path (audit / CI).
- Statistical tests required alongside frozen parity for hot-path regression safety.
- **Lazy summary deferred:** `.summary` stays eager; Phase 3.2 skipped to avoid extra getter API surface. Zero-copy `terminal_prices` (PyArray1) satisfies the trim phase.

## Evidence

- `nanobook/src/scenarios.rs` — `monte_carlo_stock_valuation`, `native_mc_reproducible`
- `nanobook/python/tests/test_scenarios_native_hot_path.py`
- `nanobook/benches/scenarios.rs`