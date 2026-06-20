# Deep Plan: Comprehensive TA-Lib Compatible Indicators in nanobook (Revised)

**Status:** Revised draft (post-ultrathink socratic analysis, 2026-06-17)  
**Owner:** Ricardo + agents  
**Motivation:** nanotrade's Strategy Spec v2 (manifest `features` + `filters` + `composition`) and stop logic depend on parameterized, latest-value technical indicators computed per-symbol at decision time. Current implementation is a tiny curated set (SMA/EMA/RSI/MACD/BB/ATR) implemented as pure-Rust TA-Lib drop-ins. Expanding safely enables richer, auditable, reproducible strategies without pulling in heavy Python TA libraries at runtime or risking lookahead.

**Key insight from socratic revision:** "All 200+" is neither feasible nor desirable. TA-Lib contains a lot of noise for systematic daily-bar quant work (60+ fragile candlestick patterns, many math transforms better done in Polars). The right target is **high-signal, low-maintenance core** (~25-40 functions) that directly power current and near-term manifest-driven strategies. Validation must be obsessive because any divergence = non-reproducible backtest vs live = capital risk.

**Principles (strengthened):**
- Pure Rust, zero runtime external TA deps.
- Bit-for-bit (or fp-tol) match to TA-Lib where we claim compatibility.
- Validation-first + incremental (one coherent group per PR).
- Domain-first prioritization (what nanotrade/strategies actually use or will use soon).
- Explicit "not supported / do it in data layer" for the rest.
- Careful: every change must preserve existing behavior and have parity evidence.

---

## 1. Foundations (Socratic Layer 1)

**What makes porting/maintaining a large TA library in a trading kernel hard and high-stakes?**

- **Fidelity tension**: TA-Lib has very specific (sometimes quirky) conventions: Wilder's vs classic EMA, population std (ddof=0), exact lookback NaN counts, first-value seeding for MACD, handling of flat series. Any deviation breaks "drop-in" promise and introduces silent lookahead or non-reproducible results in nanotrade backtests.
- **Reproducibility is load-bearing**: Strategies are versioned manifests. A backtest run today must produce identical signals/weights as the same manifest run in 6 months or in production. External Python libs at runtime introduce env drift.
- **Pareto + domain filter**: Most quant edge in daily systems comes from a small number of trend/momentum/volatility constructs. Candles are mostly overfitting traps. Math ops belong in the data prep layer (nanolake Polars).
- **Maintenance tax**: Each function is a commitment — unit tests, lookback docs, Python exposure, manifest registry update, parity golden, CI pain (system ta-lib for regen).
- **Risk of overreach**: Chasing "all 200" leads to half-baked implementations, bloat, and distraction from higher-leverage work (recovery, broker truth, stops ratchet, etc.).

Core principle: Implement only what delivers immediate, measurable value to manifest strategies, and make every one bulletproof on parity.

---

## 2. Frameworks & Mechanisms (Layer 2)

**What structures and trade-offs apply?**

**Prioritization taxonomy** (applied):
1. **Already in use** (highest): SMA/EMA/RSI/MACD/BB/ATR (and their v1 factor siblings like bollinger_pctb, atr_14).
2. **High-leverage for v2 manifests**: STOCH (fast/slow), ADX family (ADX, +DI/-DI), MOM/ROC variants, CCI, WILLR, ULTOSC, OBV/AD (volume), NATR/TRANGE.
3. **Useful overlap/price**: DEMA/TEMA/WMA, MIDPOINT/MEDPRICE, SAR (if trend following), TYPPRICE/WCLPRICE.
4. **Lower or defer**: Most math transforms, cycles, pattern recognition (60+), esoteric.

**Parity & validation mechanism** (must be data-driven):
- Golden fixture (inputs + talib outputs) in `tests/parity/golden.json`.
- Two test layers:
  - Rust: `reference_parity.rs` (fast, no C lib at runtime).
  - Python: `python/tests/reference/test_ref_indicators.py` (uses real talib for cross-check).
- Generator must support arbitrary functions easily.
- NaN handling + exact leading-NaN count must match.
- Tolerance: start 1e-6 (current for some), tighten to 1e-9/1e-10 where Python tests already achieve (see test_ref_indicators ATOL 1e-10).

**API & integration mechanism**:
- Core: pure funcs taking slices → full output series (NaN padded).
- nanotrade wrapper (indicators.py): per-symbol latest-value extraction for manifests (this is the "compute on top of data" the user asked about earlier).
- For multi-input (e.g. ADX needs H/L/C): pass full price DataFrame slice or separate high/low/close lists.
- Discoverability: add `list_indicators()` or a static registry so manifests and docs can be validated.
- Volume requirement: some indicators need volume → plan must decide whether to change per-symbol price list to richer struct early.

**Trade-off dimensions**:
- Completeness vs signal-to-noise.
- Batch series API (current, good for backtests) vs potential streaming (not now).
- Pure price vs OHLCV (many momentum are price-only; volume ones are powerful for confirmation).
- Maintenance burden vs flexibility (better a solid 30 than a buggy 150).

---

## 3. Expert Lens (Layer 3)

What would a world-class quant systems or numerical library expert prioritize?

- **Relevance filter ruthlessly**: In a daily-bar long-only or crypto system, the 80/20 are moving averages (with variants), RSI-family momentum, MACD, Bollinger (for mean-reversion or vol), ATR (for stops/risk), STOCH/ADX for regime, basic volume accumulation. Everything else is either data-prep (math) or high-dimensional overfitting (most patterns).
- **Numerical hygiene first**: Match TA-Lib's exact seeding, smoothing, and NaN rules even when "better" alternatives exist. Any divergence will be blamed on the backtest/live mismatch.
- **Test the contract, not just the number**: Tests must cover insufficient history (return None / NaN count), flat prices (RSI → 0 or 100 per TA-Lib), monotonic series, lookback length exactly.
- **Expose the primitive, compose in nanotrade**: Keep nanobook returning full series. Let nanotrade/calc decide "latest" or "whole series for factor". This keeps the kernel lean.
- **Non-obvious factors**:
  - The current per-symbol list extraction in `calc/indicators.py` is simple but will need generalization for volume or OHLC indicators.
  - Golden generation must be reproducible (seeded, pinned versions) — Docker or explicit "how to regen" is mandatory.
  - Once you have 20+, users will ask for "why not X?" — have a documented "why this one and not that" + easy contribution path.
  - Performance: these are called per rebalance date per symbol in research, and once per symbol in pre_market. Keep them O(N) and fast.
  - Future: if patterns are ever wanted, they are usually boolean per bar, not smooth numeric series — different API shape.

Expert would also say: treat this as "feature-complete core for this system" rather than "TA-Lib port". The goal is powerful manifests, not library completeness.

---

## 4. Applied Synthesis — Revised Plan

**Major revisions from original (based on socratic layers)**:

- Scope tightened dramatically: Target **~25-35 high-value functions** (not 40-60 or "all 200"). Explicit prioritized list below. Patterns and most math are explicitly out for this effort.
- Harness must be made extensible **in Phase 0** before any new functions.
- Add explicit integration work for nanotrade early (generalize the per-symbol extraction, update compute_indicator match, add registry).
- Add "registry / discoverability" as a first-class deliverable.
- Stronger emphasis on volume/OHLC path (decide in Phase 0/1).
- Each phase ends with "nanotrade can use it in a manifest + end-to-end test".
- Add "stop and evaluate" gate after Phase 2.
- Include concrete first-function list and a mapping table.
- Explicitly call out that current nanotrade usage (from strategies/research/*.jsonc) is the north star for prioritization.
- Add performance + maintenance cost tracking.
- Create the plan as living doc + pair with a real ADR.

### Revised Prioritized Function List (25-35 target)

**Group A — Already used / core (implement or harden first)**
- sma, ema (harden + variants)
- rsi
- macd + macd_signal + macd_hist (already strong)
- bbands (upper/middle/lower/pctb)
- atr (already used in stops)

**Group B — High value for v2 manifests (Phase 1-2)**
- stoch (k, d), stochf, stochrsi
- adx, plus_di, minus_di, dx
- mom, roc, rocp, rocr
- cci, willr, ultosc
- obv, ad, adosc (volume)
- natr, trange

**Group C — Useful overlap / price (Phase 2-3)**
- dema, tema, wma, trima
- midpoint, medprice, typprice, wclprice
- sar (if trend systems want it)
- apo, ppo (macd relatives)

**Explicitly deferred (documented "not planned in this effort")**
- Most CDL* patterns
- HT_* cycles
- Most math (SIN, COS, TANH, etc.) — use Polars
- LINEARREG, STDDEV, etc. if not heavily requested

### Revised Phases (tighter, with gates)

**Phase 0: Foundation & Harness (mandatory before new code)**
- Audit & publish coverage matrix + "why these and not others" rationale (based on current strategy jsonc + common quant practice).
- Refactor generator + golden schema to be function-registry driven (name + params → talib call + output dict).
- Make `reference_parity.rs` data-driven (loop over registry of {name, talib_key, args, tol}).
- Update Python ref tests to use same registry where possible.
- Add `nanobook.indicators.list_supported()` (or equivalent) returning name + docstring + parity status.
- Create ADR in nanobook/docs/adr/ (next number) summarizing decisions.
- Add first end-to-end manifest test in nanotrade using an existing indicator to prove the loop.
- **Gate**: Existing parity + all strategy backtests still pass. Adding a 5th indicator is now <30 min of work + regen.

**Phase 1: Harden Core + First High-Value Momentum (deliver usable expansion)**
- Harden existing + add 1-2 easy overlap if missing (WMA, DEMA?).
- Add STOCH family + ADX family + CCI/WILLR (biggest bang for v2 filters).
- Extend generator + golden + both test layers for these.
- Update `nanotrade/calc/indicators.py` match + add examples in strategy-manifest.md.
- Add at least one new v2 manifest in research/ using a new indicator + test it end-to-end.
- **Gate**: New indicators pass full parity (Rust + Python). Can be used in a real manifest and produce non-null signals.

**Phase 2: Volume + More Momentum/Vol (complete the 80/20)**
- OBV, AD, ADOSC.
- Remaining high ones from list (MOM/ROC, ULTOSC, NATR, etc.).
- Decide and implement OHLCV path if volume indicators are wanted (update data passing in features.py or make indicators accept richer input).
- Full docs update.
- **Gate**: Volume indicators work in manifests. Performance microbench on large universe shows no regression.

**Phase 3: Polish, Docs, Advanced Overlap (optional)**
- Remaining overlap (TEMA etc.).
- Full public docs in nanobook + update nanotrade manifest docs with "recommended for X use case".
- Performance work if needed.
- Optional: simple CLI `nanobook indicators list --parity`.

**Phase 4+ (future, on demand)**
- Math transforms only if manifests need them.
- Patterns: only if a concrete strategy use-case appears (different API shape recommended).

**Per-function implementation checklist (use for every one)**
1. Rust impl in indicators.rs matching TA-Lib (use helpers, add comments with ta-lib source line if possible).
2. Unit tests: lookback NaN count, monotonic, flat, insufficient, bounds.
3. Python py_ + high-level wrapper + .pyi.
4. Add to generator + (manually) regenerate golden.json + commit new data + versions.
5. Add/adjust Rust `*_matches_talib` test.
6. Add/adjust Python reference test.
7. Wire in nanotrade/calc/indicators.py (or generalize the dispatcher).
8. Document in manifest.md + add example usage.
9. Add at least one manifest or test that exercises it.
10. Update coverage matrix.

### Validation & CI (strengthened)

- Golden is the source of truth for "what TA-Lib produces".
- Every new indicator must have entry before the Rust test is considered passing.
- Tolerances: inherit from existing (1e-10 where Python tests use it; 1e-6 for Rust parity). Document per-function if different.
- CI: main parity uses golden (no ta-lib needed). Python ref tests are dev-only / manual or docker.
- Regeneration: must be reproducible; document exact command + prerequisites. Consider adding a Docker snippet.

### Integration with nanotrade (new explicit work)

- Generalize `compute_indicator` / `_series` to support volume/OHLC when needed (don't break existing price-only calls).
- Make indicator names discoverable (sync with nanobook list_supported or duplicate small registry).
- Update `calc/SPECS.md` and strategy-manifest.md with new names + when to use (filter vs feature vs factor).
- Add test that a v2 manifest using a Phase 1 indicator produces a non-empty selected set and correct stop levels.
- Ensure `universe.screening.lookback_days` is sufficient in examples.

### Risks & Mitigations (revised for realism)

- **Scope creep / maintenance explosion**: Strict prioritized list + "stop after Phase 2 unless strong demand" gate. Each function costs ongoing test + doc maintenance.
- **Volume path complexity**: Decide early in Phase 0/1 whether volume indicators are in scope. If yes, design a clean richer input type once.
- **Numerical surprises**: Golden + two test layers. Never claim parity without the test passing on the golden data.
- **nanotrade lookback / data window**: Manifests control lookback; new indicators with longer natural periods (e.g. 200 sma) must be tested with realistic screening.lookback_days.
- **"Why not X?" user questions**: Published rationale + "contribute one" path.
- **TA-Lib itself has edge-case quirks**: Document them (e.g. flat price RSI behavior) rather than "fix".
- **Performance on very large universes**: Current per-symbol Python loops are acceptable for research; if live pre_market becomes bottleneck, we can move more logic into Rust later.

### Effort & Sequencing (realistic)

- Phase 0: 2-4 days (harness is the foundation).
- Phase 1 (harden + 5-8 high value): 4-7 days.
- Phase 2 (volume + complete 80/20): 4-6 days.
- Polish + docs + integration tests: 2-3 days.
- Total for solid, usable expansion: 3-5 weeks of focused, reviewable work in small PRs.

**First recommended slice (after plan/ADR approval)**:
1. Phase 0 harness work + coverage matrix.
2. Add STOCH + one ADX (biggest user-visible win).
3. Wire + manifest example + end-to-end test.

### Success Criteria (measurable)

- 15-25 new high-value indicators with full parity (Rust golden + Python ref).
- At least 3 new indicators exercised in real strategy manifests in the repo.
- `list_supported()` or equivalent exists and is accurate.
- Updated strategy-manifest.md shows clear usage and "recommended for" guidance.
- Zero regressions on existing indicators, backtests, or live paths.
- Plan + ADR merged; coverage matrix published in nanobook docs.

### Open Questions (to resolve before Phase 0 code)

- Exact first 10 functions after the current set? (Use current research manifests + common additions like STOCH/ADX/CCI as input.)
- Volume support: design the input shape now or defer?
- Registry: pure Rust table or also Python-side?
- Do we want a "nanobook.indicators" high-level namespace that returns latest value directly (to reduce wrapper churn in nanotrade)?

### Next Actions

1. Review this revised plan (focus on scope, harness changes, integration work, gates).
2. Create the ADR in nanobook (summarizing key decisions from this socratic process).
3. Execute Phase 0 (harness first — this is the highest-leverage "careful" step).
4. Track the entire epic using beads in the orchestrator repo (see new section 11).
5. After Phase 1 slice, re-evaluate demand before continuing.

---

## 11. Beads-Based Cross-Repo Orchestration Model (boringQuant + nanobook:dev)

This section expands the plan to address the specific execution model requested: **keep all planning, tracking, and beads in the boringQuant orchestrator repo, while performing implementation commits inside the nanobook checkout on its `dev` branch**. Only after full testing of a slice (or the epic) do we PR into `nanobook:main`. The submodule pointer in boringQuant is then bumped as a coordination commit.

### Why This Model Fits
- **Central orchestration**: boringQuant is the meta-repo. All large efforts (beads, epics, VISION alignment, cross-repo dependencies) live here for visibility and dependency tracking.
- **Clean history in target repo**: nanobook:main stays stable. Long-running indicator work lives on `dev` (or a dedicated `feature/ta-lib-indicators` that feeds `dev`).
- **"Orchestrate from here"**: Beads, progress, cross-references, and integration testing coordination happen in boringQuant. Code changes/PRs happen where the code lives (nanobook).
- **Fits existing patterns**: The project already uses sibling checkouts + submodules, `bq-` prefixed beads, and `plan.md` / goals-ledger for orchestration.

### Bead Management (in boringQuant)
- All beads for this effort live in `.beads/issues.jsonl` in the root of boringQuant.
- Prefix convention: `bq-nb-ta-ind-<slug>` (e.g., `bq-nb-ta-ind-phase0-harness`, `bq-nb-ta-ind-stoch-adx`).
- Each bead records:
  - Description and acceptance criteria.
  - Link to relevant nanobook:dev commit hash (or PR).
  - Dependencies on other beads.
  - Status (using `br` CLI).
- Use the `beads-workflow` skill / `br` tool from within boringQuant to create, update, and query.
- High-level epic bead: `bq-nb-ta-ind-epic` (or similar) that contains child beads for phases/slices.
- Beads can reference nanotrade integration work (e.g., wiring new indicators into `calc/indicators.py` or manifest tests) even if the bulk code is in nanobook.

Example bead creation flow (run from boringQuant root):
```bash
br create bq-nb-ta-ind-phase0-harness "Refactor parity harness and bring existing indicators under golden" --depends-on ...
# Then edit .beads/issues.jsonl or use br update for details, nanobook-dev-commit: <hash>
```

### Branching & Commit Strategy
1. **nanobook side (implementation repo)**:
   - Work in the nanobook sibling checkout (or `cd nanobook` if using submodule).
   - Create / work on `dev` branch (or `feature/ta-lib-indicators` that is periodically merged to `dev`).
   - All Rust implementation, Python bindings, tests, docs, parity updates, etc., are committed here.
   - Commits should reference the boringQuant bead when possible (e.g., `refs bq-nb-ta-ind-xxx` in commit message).
   - Use the existing nanobook contribution guidelines (small focused commits, parity tests required).

2. **boringQuant side (orchestration + coordination)**:
   - Never do implementation commits here for nanobook code.
   - Only coordination commits: updating the nanobook submodule pointer after a successful PR to nanobook:main, or updating beads/docs that live in the orchestrator.
   - Track the current working nanobook dev commit in the relevant bead.

3. **PR flow**:
   - When a slice (or full phase) is ready and tested:
     - In nanobook: `dev` → open PR to `nanobook:main`.
     - Required checks: unit tests, parity (golden + reference), any new indicators exercised.
   - After merge:
     - In boringQuant: update the submodule (`git submodule update --init nanobook` or direct commit of the new pointer).
     - Commit message: "chore(submodule): bump nanobook to <commit> (refs bq-nb-ta-ind-xxx)".
     - This commit can close or advance the corresponding bead(s).

### Testing Strategy (Cross-Repo)
- **Unit + parity**: Entirely inside nanobook (on its dev branch). Must pass before PR.
- **Integration with nanotrade**:
  - Preferred: sibling checkout model (nanotrade configured to use local `../nanobook/python` via uv sources, as already done in pyproject.toml).
  - Temporarily point nanotrade's nanobook dependency to the current nanobook:dev commit hash for testing a slice.
  - Run nanotrade tests that exercise indicators (`tests/calc/`, manifest tests, strategy v2 tests, stops computation).
  - Full pre_market / backtest flows using a test manifest that includes the new indicator.
- **End-to-end validation**:
  - After nanobook:dev has the slice, create a test manifest in nanotrade/strategies/research/ that uses it.
  - Verify signal generation, weights, stops, no lookahead, parity with any reference.
  - Record results in the boringQuant bead.
- **No direct commits to nanobook:main** until the slice passes the above.

### Coordination & Synchronization
- Use beads in boringQuant as the single source of truth for status.
- In nanobook commits/PR descriptions: always include `refs bq-nb-ta-ind-xxx`.
- When a PR lands in nanobook:main, the boringQuant bead owner (or automation) updates the submodule pointer in a coordination commit.
- If using submodules during active work: occasionally push the dev pointer, but prefer editable local paths for day-to-day.
- After full epic:
  - Merge/final PR in nanobook:dev → main.
  - Bump submodule in boringQuant:main.
  - Close the epic bead.

### Updated Phasing with Beads / Branches
Every phase now explicitly includes:
- Create/update bead(s) in boringQuant.
- Work exclusively on nanobook:dev.
- Test slice (unit + nanotrade integration).
- PR nanobook:dev → main.
- Bump pointer + advance bead in boringQuant.

**Example for Phase 0**:
- Bead: `bq-nb-ta-ind-phase0-harness`
- All harness + registry + golden work done on nanobook:dev.
- No new indicators yet.
- PR to nanobook:main only after harness is proven (can add a 5th indicator with <30min effort + tests).
- Then bump in boringQuant.

### Benefits
- Orchestration stays centralized in the meta-repo.
- nanobook history remains clean (dev branch for the "careful op").
- Clear separation: planning here, implementation there.
- Easy to track cross-repo dependencies via beads.
- Matches existing development patterns (sibling checkouts, `bq-` beads).

### Risks & Mitigations Specific to This Model
- **Submodule / pointer drift**: Mitigate by making pointer bumps part of bead completion. Use `git submodule status` checks in CI or scripts.
- **Testing friction**: Document the exact uv editable + sibling checkout commands in the plan / bead. Prefer this over repeated submodule bumps during development.
- **Long-lived dev branch**: Keep slices small so dev can be merged frequently. Rebase or merge main into dev regularly.
- **Coordination overhead**: Beads + explicit `refs bq-...` in commits reduce it. One owner per bead.
- **nanotrade depending on unmerged code**: Use local paths for testing; only bump submodule after PR. Never require nanotrade to depend on nanobook:dev in its main branch.
- **Branch naming**: Standardize on `dev` for the accumulation branch in nanobook (or document the exact name).

### How Beads Will Look (Example)
In `.beads/issues.jsonl` (boringQuant):
```json
{
  "id": "bq-nb-ta-ind-phase0-harness",
  "title": "Phase 0: Parity harness + registry + existing indicators under golden",
  "status": "in_progress",
  "nanobook_dev_commit": "abc1234",
  "nanobook_pr": "https://github.com/BoringQuantSystems/nanobook/pull/XXX",
  "depends_on": [],
  ...
}
```

### Updated Next Actions (incorporating this model)
1. ...
4. Create epic bead `bq-nb-ta-ind-epic` in boringQuant.
5. In nanobook checkout: ensure `dev` branch exists or create feature branch feeding dev.
6. Start Phase 0 work exclusively on nanobook:dev.
7. Use boringQuant beads to track and orchestrate.

This model allows us to orchestrate the careful, multi-slice expansion from the central repo while keeping implementation commits and the final PR cleanly inside the nanobook repository.

The rest of the plan (scope, phases, validation, etc.) remains as previously revised, with this workflow overlaid on every phase.

**References** (key files explored):
- `nanobook/src/indicators.rs`
- `nanobook/python/src/indicators.rs`
- `nanobook/tests/parity/` (golden, generator, reference_parity.rs)
- `nanobook/python/tests/reference/test_ref_indicators.py`
- `nanotrade/calc/indicators.py`, `factors/technical.py`, `stops.py`
- `nanotrade/docs/strategy-manifest.md` + research manifests
- `nanobook/tests/parity/README.md`

This revised plan is tighter, more evidence-based, and even more incremental than the previous version. It treats the expansion as the careful operation it is.

---

**Key Takeaways from Socratic Process**
- Prioritize ruthlessly by actual manifest usage + signal, not library completeness.
- Harness extensibility is the real foundation — do it first.
- Integration with nanotrade's per-symbol latest-value model must be designed, not assumed.
- Validation is non-negotiable for trust; two layers + golden is the mechanism.
- "Careful" means small slices, explicit gates, published rationale, and stopping points.

The plan is now ready for review or Phase 0 execution. Which part do you want to deepen or start?