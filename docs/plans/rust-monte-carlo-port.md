# Plan: Full Rust Port of Monte Carlo Scenarios (nanobook)

**Date:** 2026-06-19  
**Goal:** Move the core Monte Carlo terminal valuation / scenario generation logic from pure-Python (`nanobook/python/nanobook/scenarios.py`) into the Rust core of nanobook.  
**Success Criteria:** 
- Rust implementation behind a feature flag (`scenarios`).
- Python surface continues to work (delegates to Rust when available).
- With equivalent seeding, results match the current Python reference within documented floating-point tolerance on a comprehensive parity suite.
- Existing tests (including frozen parity) continue to pass or are updated cleanly.
- No impact on nanobook's deterministic execution guarantees for non-scenario code.

## Background & Motivation

The current implementation lives in pure Python (stdlib + optional numpy) for "minimal external dependencies." It was added to make scenario analysis (simple GBM and the advanced multi-driver model) available inside the nanobook Python package.

Reasons to port to Rust now:
- Consistency with the rest of nanobook's numeric surface (GARCH, realized volatility, optimizers, metrics — all in Rust).
- Performance for large `n_paths`, many tickers, or when generating full multi-period paths.
- Ability to call the generator directly from Rust (e.g. inside parallel sweeps or custom backtest harnesses).
- Stronger control over reproducibility (explicit, portable RNG).
- Reduce divergence risk between "analysis" and "execution" layers.
- Long-term: make the Rust version the canonical implementation.

Challenges (must be addressed in the plan):
- nanobook's core philosophy is **deterministic** ("No randomness anywhere" for replayable execution). MC is intentionally stochastic → must be feature-gated and clearly separated.
- Exact bit-for-bit reproducibility between Python `random`/`numpy` and Rust `rand` is difficult. We will target "semantically equivalent for the same seed + same model" with explicit tolerance.
- Adding `rand` dependency (and `rand_distr` or equivalent) must not bloat the default build.
- Python callers currently get nice objects (`MonteCarloResult` with properties). We must preserve or improve the Python UX.

## Scope

**In scope:**
- Port of `monte_carlo_stock_valuation` (both "simple" and "advanced" modes).
- `ValuationParams`.
- `MonteCarloResult` (or equivalent struct + PyClass).
- All statistics (mean, median, percentiles, prob_above, implied returns).
- `to_price_paths` (linear / terminal_only).
- Seeded RNG surface that is usable from both Rust and Python.
- Calibration stub (can stay thin in Python or move).
- Full parity testing against the existing reference.
- Feature flag integration (`scenarios`).
- Python binding + thin shim in `scenarios.py`.
- Documentation, capabilities, examples, CI.

**Out of scope (for v1 of this port):**
- Correlated multi-asset drivers (future).
- Full stochastic path simulation inside the LOB/portfolio (future, big).
- Replacing the existing Python parity fixtures immediately (we will run dual for a transition period).
- Making it on-by-default (opt-in via feature).

## High-Level Architecture

```
nanobook (Rust)
  +-- features = ["scenarios"]
       +-- src/scenarios.rs          # core logic + MonteCarloResult
       +-- uses rand + rand_chacha (or equivalent)

nanobook-python
  +-- features = ["scenarios"] (propagates)
       +-- python/src/scenarios.rs   # PyO3 binding (PyMonteCarloResult, pyfunction)
       +-- python/nanobook/scenarios.py  # thin wrapper (prefers Rust)

Tests
  +-- reference/scenarios_parity.json (frozen from current Python or new Rust)
  +-- Dual-run parity tests (Python ref vs Rust)
  +-- Rust proptest for the models
```

### RNG Strategy (Critical for Validation)

We will use a high-quality, portable, seedable RNG from the `rand` ecosystem so that:
- Inside Rust, results are 100% reproducible.
- We can still validate against the Python oracle using statistical + quantile matching (not requiring identical bitstreams).

Recommended crates (feature-gated):
- `rand = "0.8"`
- `rand_chacha = "0.3"` (ChaCha20 — good speed + excellent cross-platform reproducibility)
- `rand_distr = "0.4"` for `Normal`

Seeding strategy:
- Public API takes `seed: u64`.
- Internally: `ChaCha20Rng::seed_from_u64(seed)`.
- Provide a way to get a deterministic sequence of normals for a given seed.
- Document that the underlying stream will **not** be bit-identical to Python's `random.Random` or `numpy.random.default_rng` for the same integer seed. Validation will rely on the Python pipeline as oracle + property checks.

For maximum validation power during development we can also implement a Box-Muller on top of a simple portable uniform (for exact algorithm matching in tests), but the production path will use the high-quality `rand_distr::Normal`.

We will keep the Python implementation's `_normal` logic as reference for the "how the model is supposed to be sampled".

### Floating Point, Reproducibility & Validation Policy (Revised for High Confidence)

Because we have the full Python pipeline (nanotrade/calc reference implementation + the existing nanobook Python test harness, parity generator, and frozen fixtures), we will treat the Python version as the **oracle** during the port and for ongoing validation.

**Validation layers (we will implement all of them):**
1. **Unit + property tests** in Rust (math primitives, distribution invariants, edge cases, reproducibility for a fixed seed + fixed RNG engine).
2. **Differential parity on frozen cases**: For every case in `scenarios_parity.json` (and new ones we generate), run both the Python reference (via the thin shim or direct import from nanotrade/calc) and the Rust path. Assert on:
   - Summary fields (median, mean, p10, p90, implied return) within tight tolerance.
   - Sorted terminal prices match within tolerance on quantiles.
3. **Statistical equivalence on large samples**: Draw large n_paths in both and compare moments, empirical CDF at several points, or use simple KS-style checks.
4. **Reproducibility**: Same seed inside Rust always gives identical output (across runs, threads when using parallel feature).
5. **Cross-check with numpy when available**: The Python side can use numpy; we compare Rust output to numpy-driven Python output on identical logical seeds where possible.
6. **Large-scale stress**: 100k+ paths, many tickers, extreme parameters — ensure no panics, reasonable memory, and statistical sanity.
7. **End-to-end**: Generate paths in Rust, feed them into nanobook's Rust backtester, compare portfolio outcomes against paths generated by the Python reference.

**Tolerance policy** (will be tuned during audit):
- Summary scalars: `rel_tol=1e-9, abs_tol=1e-6` or tighter for median/implied return.
- Full price lists: compare sorted + selected quantiles (p1, p5, p10, p25, p50, p75, p90, p95, p99).
- Document explicitly: "The Rust implementation is the fast, rich canonical version. Python reference is used for validation. Small numerical differences are expected due to different RNGs and floating-point evaluation order."

**Richness in Rust (leverage being in Rust)**:
- Keep (and expand) the rich `MonteCarloResult` API.
- Add Rust-only or faster niceties: `quantiles(&[f64]) -> Vec<f64>`, empirical VaR / CVaR / expected shortfall, `summary_stats()` with more moments/skew/kurtosis, optional parallel generation (behind `parallel` feature using rayon), memory-efficient streaming stats for very large n_paths, ability to generate paths with full intra-step diffusion if desired later.
- Use `Vec<f64>` or (behind optional dep) `ndarray` for speed when users want array interop.
- Better numerical stability (use `f64` carefully, avoid unnecessary allocations in hot loops).
- Make generation embarrassingly parallel when the feature is enabled.

This gives us something that is **at least as rich as the Python version and significantly faster** for real workloads.

## Detailed Phase Plan

### Phase 0: Audit & Design (Do this first)

1. Fully audit the current Python implementation:
   - Read `scenarios.py` completely (all math, edge cases, `_normal`, percentile interpolation, validation).
   - Read all test files: `test_scenarios*.py`, property, reference.
   - Analyze the parity JSON and generator script.
   - Document every public behavior, default values, error conditions, and the exact weighting in the advanced model.

2. Decide & document:
   - Exact feature name (`scenarios` recommended).
   - RNG crate + seeding API.
   - Rust public API (struct `MonteCarloResult`, free functions or methods).
   - How the Python `MonteCarloResult` will be produced (native PyClass vs dict + array).
   - Tolerance policy for parity tests.
   - Whether the pure-Python implementation stays as fallback forever or is deprecated after transition.

3. Update `Cargo.toml` (nanobook + python) design.
4. Write or update an ADR if the feature flag + randomness decision is significant.

**Deliverable:** Updated or new `docs/plans/rust-monte-carlo-port.md` (this document) with decisions locked.

### Phase 1: Rust Core Implementation

1. Add dependencies under the feature:
   ```toml
   [dependencies]
   rand = { version = "0.8", optional = true }
   rand_chacha = { version = "0.3", optional = true }
   rand_distr = { version = "0.4", optional = true }  # for Normal
   ```

2. Create `src/scenarios.rs`:
   - `pub struct ValuationParams { ... }` (mirror Python).
   - `pub struct MonteCarloResult { ticker: ..., method: ..., terminal_prices: Vec<f64>, ... }`
   - Implement `median_price()`, `mean_price()`, `implied_median_annual_return()`, `p10()`, `p90()`, `prob_above()`, `quantile()`, `as_log_returns()`, `to_price_paths()`.
   - `fn simple_gbm_terminal(...) -> Vec<f64>`
   - `fn advanced_multi_driver_terminal(...) -> Vec<f64>`
   - Seeded RNG wrapper: `pub fn monte_carlo_stock_valuation(..., seed: u64, ...) -> MonteCarloResult`
   - Internal `_normal` using the chosen RNG + distribution.
   - Input validation (same errors/messages as Python where reasonable).
   - Pure stats functions (no extra crates).

3. Wire into `src/lib.rs` (behind `#[cfg(feature = "scenarios")]`).

4. Add to `Cargo.toml` features:
   ```toml
   scenarios = ["dep:rand", "dep:rand_chacha", "dep:rand_distr"]
   ```

**Acceptance:** `cargo test --features scenarios` compiles and basic unit tests pass. No effect on default build.

### Phase 2: Python Bindings & Surface

1. In `python/src/` create or extend for scenarios (behind `#[cfg(feature = "scenarios")]`):
   - `PyMonteCarloResult` (PyClass mirroring the struct, with properties and methods).
   - `#[pyfunction] fn monte_carlo_stock_valuation(...)`

2. Update `python/src/lib.rs`:
   - Add to `capabilities()` the new strings: `"monte_carlo_stock_valuation"`, `"scenario_terminal_paths"`, etc.
   - Conditionally expose the module/functions.

3. Update `python/Cargo.toml`:
   - Propagate feature: `scenarios = ["nanobook/scenarios"]`

4. Update `python/nanobook/scenarios.py`:
   - Try to import the Rust version first.
   - If available, delegate.
   - Keep pure-Python implementation as fallback (or mark deprecated).
   - Maintain exact same public API and `MonteCarloResult` shape for callers.

5. Update `nanobook.pyi`.

**Acceptance:** From Python, `import nanobook; nanobook.monte_carlo_stock_valuation(...)` works when the crate is built with the feature, and produces a usable object.

### Phase 3: Validation Using the Full Python Pipeline + Richness in Rust (Be Sure)

**Core principle**: We have the entire working Python pipeline (the original reference in `nanotrade/calc/scenarios.py`, the generator `python/scripts/generate_scenarios_parity.py`, frozen `scenarios_parity.json`, reference tests, and property tests). We will use **Python as the trusted oracle** to validate the Rust implementation at every step. This gives us extremely high confidence ("we need to be sure").

**Multi-layer validation strategy (all mandatory):**
- **Oracle differential testing**: Extend the existing harness so that for every parameter set (the current ~dozen cases + many more we can generate programmatically from the Python side), we compute:
  - Using the Python reference (via `nanotrade.calc.scenarios` or the nanobook shim when numpy is present).
  - Using the new Rust path (through the PyO3 binding).
  - Assert on summary fields + selected quantiles + basic distribution properties with tight tolerances.
- **Re-generate & dual golden**: Keep the ability to run the generator against the Python oracle. Once the Rust version is solid on the existing cases, optionally promote a "Rust-generated" parity file as the new canonical (with Python still runnable for cross-check).
- **Statistical property validation** (in Rust + cross):
  - Mean of terminals should be close to the theoretical expectation for the model.
  - Median behavior, tail behavior (p10/p90), positive skew in advanced model, etc.
  - Use `proptest` + larger Monte Carlo checks.
- **Reproducibility in Rust**: Dedicated tests that the same `seed` + same parameters always produce identical `terminal_prices` vector (bitwise) inside Rust.
- **End-to-end consumption test**: Generate paths with Rust MC → feed into nanobook's Rust `backtest_weights` / portfolio simulator. Do the same with Python-generated paths. Compare final equity, returns, etc. (within model noise).
- **Stress / large N**: 100k–1M paths, extreme parameters. Ensure performance is good and no numerical blow-ups.
- **No-numpy / pure path cross-check**: Make sure the Rust results are reasonable even when the Python side is forced to stdlib-only mode.

**Leveraging "we are in Rust" for richness + speed**:
Because the implementation is native Rust (behind the feature), we can (and should) make it **better** than the Python version:
- **Faster**:
  - Use rayon (`parallel` feature) to generate paths in parallel when enabled.
  - Avoid Python object overhead in hot loops.
  - Use `Vec<f64>` efficiently; optional `ndarray` feature for zero-copy interop with numpy users.
  - Streaming / online statistics so we don't always need to materialize millions of prices just to get quantiles.
- **Richer**:
  - `MonteCarloResult` can expose many more methods cheaply: arbitrary `quantiles(&[f64])`, `var(alpha)`, `cvar(alpha)` / expected shortfall, `skewness()`, `kurtosis()`, empirical CDF at point, `to_histogram(bins)`, etc.
  - Easy to add "rich" options in the future: full intra-period Brownian bridges for path generation, antithetic variates, control variates, etc.
  - Better diagnostics (e.g. effective sample size, convergence checks on running stats).
  - Direct Rust API for people doing large scenario analysis inside Rust code (no GIL, no Python at all).
- Keep the Python-facing `MonteCarloResult` API backward-compatible (or a superset) so existing users (nanotrade etc.) see almost no change.

**Implementation order for validation**:
1. Get basic Rust math + one mode working.
2. Wire Python binding.
3. Run the full existing parity cases via the Python generator + compare (fail the build if outside tol).
4. Expand the case list programmatically from Python.
5. Add the statistical + e2e layers.
6. Only then declare the port solid.

This approach gives us the best of both worlds: the trusted Python pipeline validates us, while we deliver something that is richer and much faster because it is written in Rust.

### Phase 4: Packaging, Docs, Polish

1. Update all documentation:
   - `README.md` (both root and python/)
   - Add note about the `scenarios` feature.
   - Mention reproducibility characteristics and tolerance.
   - Update the existing `plan-pure-python-mc-scenarios.md` or mark it as historical.

2. Add to public API baselines if needed.

3. Update `CHANGELOG.md`.

4. Decide + document the future of the pure-Python code:
   - Option A: Keep as always-available fallback.
   - Option B: Only available when `scenarios` feature not compiled (for no-rand builds).
   - Option C: Deprecate after one release.

5. Add to capabilities probing docs/examples.

6. Optional but nice: benchmark large `n_paths` comparing pure-Py vs Rust.

### Phase 5: Integration & Rollout

1. Update the Python package build to enable the feature by default in the published wheel (or keep opt-in? Recommend enabling it).

2. In `nanotrade/calc/scenarios.py` (or wherever it is used), switch to using `nanobook.monte_carlo_stock_valuation` when available.

3. Add a deprecation path if the old pure-Py module in nanotrade is removed.

4. Ensure that using scenarios does not accidentally pull randomness into deterministic backtest replay paths (the generator is separate from execution).

### Phase 6: Beads & Execution

Break the above into granular, ordered beads with dependencies.

Example high-level beads:
- `bq-rust-mc-audit-spec`
- `bq-rust-mc-design-rng-feature-api`
- `bq-rust-mc-impl-core`
- `bq-rust-mc-py-binding`
- `bq-rust-mc-parity-harness`
- `bq-rust-mc-dual-test-ci`
- `bq-rust-mc-docs-polish`
- etc.

Use the beads-workflow (`br`) to create them after locking this plan.

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Float divergence makes "same results" claim impossible | Clear tolerance + sorted-list comparison + documentation that exact match is not promised |
| Adding rand crate increases compile time / binary size | Feature-gated; document impact |
| Breaks nanobook "deterministic" brand | Strong separation in docs + feature flag + name the module `scenarios` not core |
| Reproducibility across platforms/Python versions | Choose ChaCha20 + test on CI (linux + mac + windows if possible) |
| Maintenance burden of two implementations | Plan to make Rust canonical; keep Python thin or remove after transition period |

## Open Questions (to resolve in Phase 0)

1. Exact RNG crate + version?
2. Should `seed` be `u64` in Rust and `int` in Python (with wrapping)?
3. Do we expose a way to get the internal RNG state (probably not)?
4. Tolerance values — decide after first dual runs on the parity cases.
5. Default: enable `scenarios` feature in published `nanobook` wheels?

## Next Immediate Actions

1. Read full current Python implementation + all tests + parity data (done in this planning pass).
2. Lock decisions on RNG and feature name.
3. Write/update this plan as the source of truth.
4. Create beads graph.
5. Start with audit bead.

---

This plan is designed to be executed methodically, with heavy emphasis on verification ("check if we get the same python results") as requested.