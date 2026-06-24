# Plan: Pure-Python Monte Carlo Scenarios in nanobook (minimal deps)

> **Historical (2026-06-19):** Core computation moved to Rust (`src/scenarios.rs`,
> feature `scenarios`). This document remains as the original stdlib-first design
> record. See `docs/plans/rust-monte-carlo-port.md` and ADR 0006 for the current
> architecture. Python `scenarios.py` is now a thin shim with pure-Python fallback.

## Vision & Goals
- Consolidate the working Monte Carlo terminal valuation / scenario generation logic into the nanobook Python package as **pure Python** code.
- Minimal external runtime dependencies (ideally stdlib only: `random`, `math`, `dataclasses`, `typing`, `statistics`).
- Keep full reproducibility and exact behavioral parity with the current numpy/pandas working version in nanotrade/calc/scenarios.py (easy to reproduce because we have it).
- Expose via `import nanobook; nanobook.monte_carlo_stock_valuation(...)` and rich `MonteCarloResult`.
- Support optional numpy acceleration for speed if user has it installed (graceful fallback).
- Make it available for forecasts, strategy conviction, sizing views, and generating price paths for nanobook's own deterministic backtester.
- Preserve nanobook's core identity (execution kernel) while adding this as a first-class "research helper" in the Python surface (similar to how walkforward, indicators, etc. are exposed).
- All changes respect the sharp Python-strategy / Rust-execution boundary.
- Long-term: source of truth lives in nanobook; nanotrade/calc can re-export or depend lightly.

## Constraints & Principles (from project docs)
- Pure Python in the installed `nanobook` package (see nanobook/python/nanobook/__init__.py and maturin config).
- No new runtime deps in pyproject.toml (dev deps ok for tests).
- Reproducible with explicit seed (use `random.Random(seed)` not global).
- Self-documenting, with detailed docstrings.
- Full test coverage including parity against numpy reference impl.
- Follows nanobook's ubiquitous language, docs style, and CI (ruff, pytest, etc.).
- Beads will be used for implementation via ultracode / supervisor later.
- Do not break existing nanobook users or the Rust core.
- Support both "simple" GBM and "advanced" multi-driver models exactly.
- Return structure compatible with feeding `terminal_prices` into nanobook backtest price schedules.
- Calibration helper remains (improved for stdlib).

## Current State (reference for reproduction)
- Working impl: nanotrade/calc/scenarios.py (MonteCarloResult dataclass, simple/advanced, calibrate, to_price_paths, pure stats).
- Original proposal in user query with numpy/pandas.
- nanobook Python: thin pure-py wrapper + heavy Rust via .so (see nanobook/python/nanobook/__init__.py, pyproject.toml uses maturin, no runtime py deps).
- Existing pure-py patterns in nanobook/python: aliases, some test helpers use math/random.
- Determinism focus in nanobook but MC is intentionally stochastic (seeded, documented as research tool).

## High-Level Approach
Implement as a new pure-Python submodule `nanobook.scenarios` (or `nanobook.montecarlo`).
- File: nanobook/python/nanobook/scenarios.py (included automatically).
- Expose in __init__.py with clean names.
- Pure stdlib core + optional numpy fast-path.
- Update .pyi for static types.
- Add tests under tests/ (new test_scenarios.py + parity).
- Update README, add example.
- Ensure maturin/python packaging includes .py without Rust changes.
- Add to capabilities() list.

## Detailed Multi-Phase, Multi-Step Plan (for beads)

### Phase 0: Foundation & Exploration (Investigation & Design)
0.1 Audit nanobook Python packaging and pure-py extension points.
0.2 Extract and freeze exact behavior/spec from working numpy version (input/output contracts, edge cases, math).
0.3 Design pure-Python equivalent (no pandas: return dataclass + ndarray-like or list; use dicts for summary).
0.4 Decide on dep strategy: stdlib primary, try: import numpy as np except: np = None ; use fast path when available.
0.5 Design API surface (keep close to current working + original proposal).
0.6 Decide on error types (simple Exception or mirror calc style but standalone).
0.7 Document positioning vs nanobook core (research helper, not execution).
0.8 Create initial beads for this phase.

### Phase 1: Core Pure-Python Implementation
1.1 Implement seeded RNG wrapper using random.Random (support int seed or Random instance).
1.2 Implement normal sampling (random.gauss(mu, sigma) or Box-Muller for purity if needed).
1.3 Port simple GBM terminal: drift + diffusion.
1.4 Port advanced multi-driver: gp, marg, mult (clip), shock with bear_skew, weighted total_ret, exp.
1.5 Implement pure stats: mean, median (sorted), percentile (manual or statistics), prob_above (count).
1.6 Build MonteCarloResult dataclass (pure, no polars/numpy hard req; methods for implied_return, quantiles, to_price_paths, etc.).
1.7 Implement calibrate_from_fundamentals as pure dict-returning stub (expand docs).
1.8 Implement terminal_prices_to_log_return_paths and summarize_distribution pure.
1.9 Make summary return a simple object or dict (no pl.DataFrame hard dep).
1.10 Add __version__ or model version tracking inside results if useful.
1.11 Internal: make all paths use lists or arrays only when numpy present.

### Phase 2: Packaging, Exports & Surface
2.1 Add scenarios.py to nanobook/python/nanobook/ .
2.2 Update nanobook/__init__.py : from .scenarios import * ; add aliases like monte_carlo_stock_valuation = ... ; extend capabilities().
2.3 Update nanobook.pyi with full signatures and MonteCarloResult class stub.
2.4 Ensure pyproject.toml / maturin include pure .py (python-source="." should cover).
2.5 Add to __all__ properly.
2.6 Add optional numpy fast-path inside functions (if np: use np.random.default_rng ... else: pure).
2.7 Add deprecation or compatibility notes if overlapping with nanotrade later.

### Phase 3: Integration Helpers & Usability
3.1 Ensure to_price_paths / schedule helpers work with nanobook's BarPrices / price_schedule format (pure py examples).
3.2 Add build_forward_metrics or similar (for StrategyReport style usage, pure).
3.3 Add convenience: scenario_expected_returns for batch.
3.4 Make MonteCarloResult have .to_dict(), .as_pandas() if pandas optional, etc.
3.5 Add usage examples that feed directly into nanobook.backtest_weights (price schedules from terminals).
3.6 Support horizon >1 and multi-period considerations (document limitations).
3.7 Add seed handling that is cross-call reproducible.

### Phase 4: Tests & Parity (Critical for "easy to reproduce")
4.1 Create tests/test_scenarios.py .
4.2 Basic unit tests: seeds produce same output, simple vs advanced differ, edge cases (N=0, bad prices, zero vol, etc.).
4.3 Property tests (hypothesis) for invariants (positive prices, median between p10/p90, etc.).
4.4 Parity / golden tests: run identical params with numpy version (from nanotrade or vendored reference) and assert exact match on summary + sorted paths within float tol.
4.5 Test optional numpy path vs pure (when numpy present in test env).
4.6 Test no external deps: run in clean venv with only nanobook wheel (if possible in CI).
4.7 Repro tests for the XYZ calibration example (median ~88).
4.8 Test feeding generated paths into nanobook backtest (end-to-end).
4.9 Add to existing reference/ or property/ test structure if fits.
4.10 CI matrix: test with/without numpy in dev.

### Phase 5: Documentation & Examples
5.1 Update nanobook/python/README.md with full section on scenarios (like optimizers/GARCH).
5.2 Add usage in main nanobook README if appropriate (cross-ref).
5.3 Create or extend examples/ with scenario + backtest usage (pure py script).
5.4 Update nanobook.pyi docs.
5.5 Add docstring examples that match working version.
5.6 Document minimal-deps story + when to use numpy accel.
5.7 Update CHANGELOG.md in nanobook.
5.8 Add to pyi public API list.

### Phase 6: Polish, Robustness & Edge Cases
6.1 Input validation (prices >0, n_paths>0, horizon>0, sds>=0, etc.) with clear errors.
6.2 Float safety (inf, nan handling, under/overflow).
6.3 Performance: pure py acceptable for 5k-50k paths; benchmark vs numpy.
6.4 Make dataclass frozen or immutable where sensible.
6.5 Add __repr__, equality, etc. for MonteCarloResult.
6.6 Handle large N gracefully (memory for paths list).
6.7 Thread-safety notes for RNG (document).
6.8 Optional: support correlated drivers in future (but not in v1).

### Phase 7: Cross-Repo & Downstream
7.1 Plan (but do not yet execute) sync with nanotrade/calc: make calc import from nanobook when available, or thin wrapper.
7.2 Update any references in goals, VISION, CONTEXT if needed for nanobook scope.
7.3 Ensure nanobook python tests pass (add scenarios to test matrix).
7.4 Verify wheel build includes pure py and import works post-install.
7.5 Add to "capabilities" probing pattern in docs.

### Phase 8: Release & Maintenance
8.1 Bump version in pyproject.toml / Cargo (follow nanobook SEMVER).
8.2 Full test suite + reference parity.
8.3 Update docs.rs / sphinx if any, but mostly README.
8.4 Add beads for future enhancements (full path sim, correlation, calibration from data, sensitivity).
8.5 Post-release: monitor for float diff reports between pure and numpy.
8.6 Long-term: consider moving more research helpers here if pattern succeeds.

### Non-Goals (for this plan)
- Rust port of MC (keep discussion separate).
- Adding runtime numpy/pandas as hard dep.
- Changing nanobook core Rust (no randomness in LOB/portfolio).
- Full multi-horizon stochastic paths in v1 (terminal focus).
- Automatic lake calibration in this pass (stub + manual params).

### Acceptance Criteria (overall)
- `pip install nanobook` (or editable) allows `from nanobook import monte_carlo_stock_valuation` with zero extra imports.
- Exact same numbers as working numpy version for same seeds/params (within documented tol).
- Runs in clean Python with no numpy/pandas.
- Full docs + tests + examples.
- No breakage to existing nanobook API or builds.
- Beads cover every step above with dependencies.

### Risks & Mitigations
- Float reproducibility across platforms: mitigate with sorted paths + documented tolerance + seed tests.
- Scope creep into "research stack": mitigate with clear docs "this is a helper, execution remains deterministic".
- Packaging the .py correctly under maturin: test build early.
- Users expecting pandas df: provide .to_pandas() optional method.

Use this plan to generate beads via the standard conversion prompt (detailed, self-contained, with tests in every relevant bead).

## References
- Original MC proposal (user message).
- Working impl: nanotrade/calc/scenarios.py
- nanobook Python: pyproject.toml, __init__.py, tests/
- nanobook philosophy: README determinism sections, CONTEXT.md
- Beads process: beads-workflow skill

This plan is deliberately deep and granular for ultracode / supervisor-coder execution later.