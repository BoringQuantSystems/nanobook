# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **CI now lints the whole workspace.** `cargo clippy` ran without
  `--workspace`, so only the root crate was checked and broker, risk and
  rebalancer accumulated 141 unchecked findings. Clearing them was mostly
  mechanical: 46 were the `100_00` = $100.00 digit-grouping convention, which
  the root crate had declared crate-wide but the other two had not — now set
  once per crate in `[lints.clippy]`, which reaches integration tests as well
  (each file under `tests/` is its own crate root, so a `lib.rs` attribute
  cannot). Another 38 were machine-applied by `cargo clippy --fix`. The
  remainder were dead-code warnings on shared test mocks: `mock_binance.rs`
  and `mock_tws.rs` are each pulled into four test binaries, so a method one
  binary drives looks dead to the other three.
- **Public API baselines regenerated** (`docs/public-api/`) — they were eight
  releases out of date. Nothing was removed; the growth is `scenarios` and
  `indicators` arriving in v0.17.0, plus a rendering change in newer
  `cargo-public-api`. Regeneration is now a step in `RELEASING.md`, and
  `docs/public-api/README.md` records the tool version, since the output
  format is not stable across releases of it.
- **`rebalancer kill` states plainly that it does nothing on Windows.** It
  already refused there, but with a bare "not fully supported"; it now names
  the manual steps (`taskkill`, then cancel open orders at the broker) so an
  operator is not left assuming an emergency stop happened.
  `docs/operations/kill-switch.md` marks live execution on Windows unsupported.
- **`ibapi` major bumps are now ignored by dependabot**, with the reasoning in
  `.github/dependabot.yml`. 3.x removes the arm that detects IBKR disconnects
  mid-execution; it needs its own change driven by the mock-TWS drills rather
  than arriving as a weekly PR. Minor and patch bumps still come through.

### Added

- **Frozen reference test for the native ChaCha20 Monte Carlo path**
  (`native_mc_matches_frozen_reference`). The existing reproducibility test
  compares two runs inside one build, so it passes whether or not an RNG
  upgrade moves the stream — the `rand` 0.8 → 0.10 bump in 0.17.1 had to be
  checked by hand. The comparison is relative to 1e-12 rather than bitwise,
  because the terminal math goes through `exp`/`ln` whose last ulp is
  libm-dependent across platforms; any real stream change moves the numbers by
  whole percent.

## [0.17.2] - 2026-07-26 - Release-gate fix

0.17.1 reached PyPI and the GitHub release but never reached crates.io: the
publish gate ran the workspace test suite and one test failed on the runner.
This release carries the same contents as 0.17.1 plus the fix, and is the
version to use. Crate versions: `nanobook` 0.17.2, `nanobook-broker` 0.8.1,
`nanobook-risk` 0.6.4, `nanobook-rebalancer` 0.8.4.

### Changed

- **MSRV raised from 1.86 to 1.88** across all four crates. The 1.86 claim was
  already false for `nanobook-broker`, `nanobook-risk` and
  `nanobook-rebalancer`: the 0.17.1 dependency sweep pulled `time` to 0.3.54
  (via `ibapi` and `tracing-appender`), which requires 1.88. Declaring 1.88
  everywhere keeps one number across the workspace. Cargo's resolver is
  MSRV-aware, so a consumer on an older toolchain resolves to an earlier
  version rather than failing to build.
- Nested `if let` blocks in `level`, `price_levels`, `stop`, `exchange`,
  `backtest_bridge` and two examples are now let-chains, which 1.88 permits.
  Machine-applied and semantically identical; the suite is unchanged at 1134
  passing.

### Fixed

- **`nanobook-rebalancer` did not compile on Windows.** The `#[cfg(windows)]`
  arm of the kill switch opened with `use windows::Win32::System::Console::…`
  for symbols it never referenced, and the `windows` crate was never declared
  as a dependency. The import is removed; the arm already returned "Kill
  switch not fully supported on Windows yet" unconditionally, so behaviour is
  unchanged. Every published version of this crate has been broken on Windows.
- **`test_reconciliation_blocks_submission` no longer calls the live Binance
  API.** It opened a real connection as setup, so it failed with
  `451 Unavailable For Legal Reasons` on runners in regions Binance
  restricts. `submit_order` checks the reconciliation flag before it requires
  a client, so the connection was never needed for the assertion; the test
  still fails if that check order is ever reversed.
- **CI ran `cargo test --all-features` without `--workspace`**, so it only
  ever tested the root crate — the broker, risk and rebalancer suites were
  invisible to it apart from three explicitly-named failure-injection drills.
  That is why a test making live network calls survived unnoticed. The test
  job now runs the whole workspace.

## [0.17.1] - 2026-07-26 - Dependency sweep and MSRV 1.86

**Superseded by 0.17.2.** Published to PyPI and GitHub releases only; the
crates.io publish did not run. See 0.17.2 for the reason.

A maintenance release. No public API changed in any crate, and no numerical
output moved. The one behavioural change is the TLS trust store used by the
optional Binance HTTPS client, which is why `nanobook-broker` takes a minor
bump while everything else is a patch.

**Breaking (nanobook-broker 0.7.2 → 0.8.0):**

- **The `rustls` TLS backend now uses the OS trust store, not a bundled one.**
  `reqwest` 0.13 renamed its `rustls-tls` feature to `rustls` and changed what
  it pulls in: `rustls-platform-verifier` (system trust anchors) and `aws-lc-rs`
  replace the previously vendored `webpki-roots` and `ring`. The
  `nanobook-broker` feature names (`rustls`, `native-tls`) are unchanged, so no
  manifest edit is needed — but a minimal container image that ships no CA
  bundle will now fail the TLS handshake to Binance where it previously
  succeeded. Install `ca-certificates`, or select `native-tls` instead. Only
  affects builds with the `binance` feature enabled; `nanobook-risk` and
  `nanobook-rebalancer` do not enable it.

### Changed

- **MSRV raised from 1.85 to 1.86.** `criterion` 0.8 requires rustc 1.86, and
  holding the benchmark harness three majors back to preserve 1.85 was the worse
  trade. The library itself uses no 1.86-only features, so this is a floor
  change rather than a code change; let-chains still need 1.88 and remain
  written out longhand. Cargo's resolver has been MSRV-aware since 1.84, so a
  toolchain still on 1.85 resolves to 0.17.0 rather than failing to build.
- **Dependencies brought current** — GitHub Actions: `checkout` 7.0.1,
  `setup-python` 7.0.0, `setup-uv` 9.0.0, `action-gh-release` 3.0.2,
  `gh-action-pypi-publish` 1.14.1. Cargo: `serde` 1.0.229, `criterion` 0.8,
  `toml` 1.1, `dialoguer` 0.12, `nix` 0.31, `signal-hook` 0.4,
  `tokio-tungstenite` 0.30, `reqwest` 0.13, and `sha2` 0.11 with `hmac` 0.13
  (the digest 0.11 generation — `new_from_slice` moved from `Mac` to
  `KeyInit`). Binance request signing is unchanged; the known-signature vector
  still matches.
- **`rand` 0.8 → 0.10** (with `rand_chacha` 0.3 → 0.10 and `rand_distr` 0.4 →
  0.6). The only affected call site is the `seed=None` helper in
  `scenarios`; seeded Monte Carlo output was compared before and after the
  upgrade for a fixed seed and is bit-identical to 17 significant digits.
- **Patch versions**: `nanobook` and `nanobook-python` to 0.17.1,
  `nanobook-risk` to 0.6.3, `nanobook-rebalancer` to 0.8.3. `ibapi` stays on
  the 2.x line deliberately; the 3.x migration changes disconnect-detection
  semantics in live order placement and is scoped as its own change.

## [0.17.0] - 2026-07-25 - TA-Lib indicators + Monte Carlo scenarios

### Added

- **Technical-analysis indicators (TA-Lib parity)** — 25 curated indicators implemented in
  pure Rust with golden parity against pinned TA-Lib outputs: trend and momentum (`rsi`,
  `macd`, `stoch`, `stochf`, `stochrsi`, `willr`, `cci`, `roc`, `rocp`, `rocr`, `mom`,
  `ultosc`), the directional family (`adx`, `dx`, `plus_di`, `minus_di`), volatility (`atr`,
  `natr`, `trange`, `bbands`), and volume (`obv`, `ad`, `adosc`). Wilder smoothing for the
  RSI/ATR/ADX family and population standard deviation for Bollinger bands. A JSON indicator
  registry drives golden generation alongside the Rust and Python parity tests, and
  `list_supported()` exposes the contract (`has_parity` marks golden-backed entries). No
  runtime dependency on the TA-Lib C library; the remaining functions are classified and
  explicitly deferred in `docs/ta-lib-full-coverage-matrix.md`. See
  `docs/adr/0005-ta-indicators-scope.md`.

- **Native ChaCha20 Monte Carlo hot path** — the default Python `monte_carlo_stock_valuation`
  now draws with native Rust ChaCha20 and no NumPy RNG round-trip; the NumPy-bridge path is
  retained as `monte_carlo_stock_valuation_parity` for frozen audit and CI. Terminal prices
  return zero-copy as a NumPy array, and runs of 50k paths or more parallelise. See
  `docs/adr/0007-scenarios-chacha20-hot-path.md`.

- **Rust Monte Carlo scenarios** — Core terminal valuation (simple GBM and advanced
  multi-driver) lives in `src/scenarios.rs` behind the `scenarios` feature. The
  Python extension draws with NumPy PCG64 and runs math/stats in Rust for frozen
  parity (`rel_tol=1e-9`). A native ChaCha20 Rust API is available for Rust-only
  callers. Pure-Python fallback remains for `random.Random` seeds and no-numpy
  environments. See `docs/adr/0006-scenarios-rust-port-rng.md`.

- **Pure-Python Monte Carlo scenarios** — `nanobook.scenarios` exposes
  `monte_carlo_stock_valuation`, `MonteCarloResult`, and helpers with stdlib-only
  runtime (optional numpy acceleration when installed). Includes frozen numpy parity
  fixtures, hypothesis property tests, and an e2e path into `backtest_weights`.
  See `python/examples/scenario_backtest.py`.

- **IBKR resting STOP / STOP_LIMIT orders** — `IbkrBroker.submit_order` accepts a
  new optional `stop_price_cents` and routes `order_type="stop"` / `"stop_limit"`
  through ibapi's `stop` / `stop_limit` order builders (`aux_price` = stop trigger,
  `limit_price` = limit for stop-limit). Previously only market/limit were encodable.
  Additive and backward-compatible: existing market/limit calls are unchanged and the
  new parameter is last and optional. Enables broker-resting protective stops.

### Security

- **Dependency advisories cleared** — upgraded `pyo3` and `numpy` 0.27 → 0.29
  (RUSTSEC-2026-0176, out-of-bounds read in `PyList`/`PyTuple` `nth`/`nth_back`;
  RUSTSEC-2026-0177, missing `Sync` bound on `PyCFunction::new_closure`); bumped
  `quinn-proto` to 0.11.16 (RUSTSEC-2026-0185, remote memory exhaustion) and
  `crossbeam-epoch` to 0.9.20 (RUSTSEC-2026-0204, invalid pointer dereference).
  The pyo3 bump adds explicit `from_py_object` opt-in to the `Clone`-deriving
  `#[pyclass]` types; behaviour is unchanged. Also cleared the two remaining
  unsoundness advisories by bumping `anyhow` to 1.0.104 (RUSTSEC-2026-0190,
  `Error::downcast_mut`) and `memmap2` to 0.9.11 (RUSTSEC-2026-0186, unchecked
  pointer offset). `cargo audit` and `cargo deny check` are both clean.

### Fixed

- **Python `__version__` drift** — the binding hardcoded `0.9.1` while the wheel
  shipped as `0.16.2`. It is now taken from the crate manifest via
  `env!("CARGO_PKG_VERSION")`, and `test_version` asserts it against the
  installed distribution metadata so the two cannot diverge again.
- **CI toolchain and lints** — pinned `dtolnay/rust-toolchain` to the known-good
  stable SHA, applied `cargo fmt --all`, and resolved `clippy -D warnings`
  (manual memcpy, range/counter loops) in the indicators module. Bumped the
  pinned GitHub Actions (setup-python, upload-artifact, setup-uv, codecov).

## [0.16.2] - 2026-06-11 - MSRV fix + repository metadata

Patch release. No behavior changes.

### Fixed

- **MSRV regression**: v0.16.1 did not compile on Rust 1.85 despite
  declaring `rust-version = "1.85"` — `src/backtest_bridge.rs` used a
  let-chain, stabilized only in Rust 1.88. Rewritten as nested `if let`.
- `examples/itch-replay/download.sh` failed on a fresh checkout (resolved
  the `data/` path by `cd`-ing into it before creating it).

### Changed

- Repository moved to the BoringQuantSystems organization; all
  `repository`/`homepage` metadata and links updated.
- CI: GitHub Actions pinned to commit SHAs, Dependabot enabled for
  actions, `ocaml/setup-ocaml` bumped to v3, uv smoke jobs create a venv
  before installing dependencies.

## [0.16.1] - 2026-05-20 - Position arithmetic overflow fix

Patch release. Replaces bare integer arithmetic in `Position` with
saturating variants, matching the pattern already used in
`Portfolio::execute_fill`. Found by `fuzz_execute_fill` (added in v0.16.0
as part of the property-based testing pass after the v0.16.0 ship).

### Fixed

- **`Position::market_value` overflow** (`src/portfolio/position.rs:95`):
  `self.quantity * price` previously panicked on `i64` overflow.
  Replaced with `saturating_mul`. Reachable via the public
  `Portfolio::rebalance_simple` API; crash artifact preserved at
  `fuzz/artifacts/fuzz_execute_fill/crash-330fb9097052e84d78571d06b840334680573582`.
- **`Position::unrealized_pnl` overflow** (`position.rs:104`): same root
  cause; both the subtraction and multiplication now saturate.
- **`Position::apply_fill` overflow** (lines 55, 58, 69, 80, 87): all
  intermediate products/sums switched to saturating variants. Real-world
  trigger requires extreme quantity × price products; bug was latent
  pre-v0.16.0 but only surfaced after the fuzz harness was added.
- **`Portfolio::total_equity_from_price_map` overflow** (`src/portfolio/mod.rs:430-435`):
  bare `.sum()` over `pos.market_value(price)` and bare `self.cash + position_value`
  could panic when many large positions accumulated. Replaced with `saturating_*`.
  Found by `fuzz_execute_fill` re-run after the position.rs fix landed (crash artifact
  `crash-6bafbd4544cfff56e4ac3e2d68e18ebeecf6c2d7`).
- **`Portfolio::rebalance_simple` arithmetic** (`src/portfolio/mod.rs:238, 285, 346, 385`):
  defensive `saturating_*` replacements at five other i64 arithmetic sites in
  the rebalance / equity / price-mid paths. Each was a latent overflow risk
  reachable through the public API under extreme inputs.
- **`Portfolio::snapshot` `total_realized_pnl` overflow** (`src/portfolio/mod.rs:397`)
  [Inference]: bare `.sum()` over `pos.realized_pnl` fields — same pattern as the
  `total_equity_from_price_map` bug; replaced with `saturating_add` fold. Not in the
  original fuzz crash path but reachable from the same extreme-input scenarios.

### Added

- **`tests/regression_position_overflow.rs`**: integration tests that
  assert each previously-crashing input sequence now returns a saturated
  i64 instead of panicking. Replays the libfuzzer crash artifact by hand.

## [0.16.0] - 2026-05-19 - Backtest fill realism

Breaking-change release driven by an audit of the portfolio backtester (`nanotrade/docs/audit/2026-05-19-nanobook-fill-model.md`). The Python binding `backtest_weights` is replaced in-place per ADR-0004; the only Python caller is nanotrade, which bumps to 0.6.0 in lockstep.

### Added

- **`BarPrices` struct** (`src/backtest_bridge.rs`): `{ open, high, low, close: i64 }` replaces the single-price `i64` previously used in `price_schedule`. Enables fill policies that need bars other than close. See `docs/adr/0001-backtest-bar-prices.md`.
- **`FillPolicy` enum** (`src/backtest_bridge.rs`): `SignalBarClose` (parity / true MoC), `NextBarOpen` (default — realistic EOD fill), `NextBarTypical` ((H+L+C)/3 of t+1). Final-bar rebalances under `NextBarOpen`/`NextBarTypical` are skipped with a returned diagnostic rather than silently falling back. See `docs/adr/0002-fill-policy.md`.
- **`BacktestBridgeResult.skipped_rebalances: Vec<usize>`**: indices in the schedule that the simulator could not fill (e.g. last-bar `NextBarOpen` with no `t+1`). Empty when nothing was skipped.
- **Decision records**: `docs/adr/0001-backtest-bar-prices.md`, `0002-fill-policy.md`, `0003-cost-model-semantics.md`, `0004-binding-breaking-change.md`.
- **Test coverage**: parity test (`signal_bar_close_parity_with_degenerate_ohlc`), fill-policy proofs (`next_bar_open_fills_at_open_t_plus_1`, `next_bar_typical_fill_price_is_hlc3`), last-bar edge case (`last_bar_skip_with_next_bar_open`), CostModel parity (`parity_no_slippage_matches_old_formula`), and matching Python tests in `python/tests/test_fill_policy.py`.

### Changed

- **BREAKING: `CostModel` struct** (`src/portfolio/cost_model.rs`): three fields with corrected types and semantics.
  - `commission_bps: f64` (was `u32`) — fractional bps allowed; commission billed as cash charge subject to `min_commission` floor.
  - `slippage_bps: f64` (was `u32`) — now applied as **price impact** in `Portfolio::execute_fill` (fill price adjusted before recording the position), no longer collapsed into the cash charge.
  - `min_commission: i64` (renamed from `min_trade_fee`) — floor on commission, preserves the prior `max()` semantics with an unambiguous name.
- **BREAKING: Python binding `backtest_weights`** (`python/src/backtest_bridge.rs`): replaces `cost_bps: u32` with `cost_model: CostModel`; adds `fill_policy: FillPolicy`; changes `price_schedule` element type from `(str, i64)` to `(str, BarPrices)`. Returned dict gains `skipped_rebalances`.
- **`compute_cost` formula**: `max(|notional| * commission_bps / 10_000, min_commission)`. Slippage is no longer summed in; it lives in `execute_fill`.

### Fixed

- **Slippage attribution**: Previously, slippage was deducted from cash alongside commission, leaving per-position cost basis unchanged. After this release, position cost basis reflects slippage (matching TradingView's `strategy(slippage=N)` semantics and live broker microstructure).
- **Hardwired `slippage_bps=0`, `min_trade_fee=0` in Python binding**: the old `backtest_weights` PyFunction silently zeroed two `CostModel` fields. Half-implemented feature now fully exposed.

### Migration

- nanotrade 0.6.0 bumps the `nanobook` pin to `>=0.16,<0.17` and rewrites `calc/nb_backtest.py:run_backtest_nb` to call the new binding. No other Python callers exist.

## [0.15.1] - 2026-05-17 - Ops Hardening & Optimization

This patch release bundles the post-v0.15 reliability work, Python cleanup, and the simplify/optimization pass across the workspace. It keeps behavior stable while making rebalancer execution safer and several hot paths leaner.

### Added

- **HRP optimizer**: Added a Hierarchical Risk Parity optimizer following López de Prado 2016, with review follow-ups for critical and lower-priority issues.
- **Portfolio research/reporting APIs**: Added backtest attribution decomposition, standard indicator APIs, walk-forward analysis, drawdown analytics, OHLC realized-volatility estimators, and tear-sheet reporting payloads.
- **Rebalancer production hardening**: Added write-ahead audit checkpoints for orders, positions, quotes, and account/cancel flows; broker-state reconciliation during recovery; feature-flagged rollout support; crash-injection validation; and complete checkpoint coverage fixtures.
- **Operational safety tooling**: Added graceful shutdown, guaranteed/two-phase kill-switch workflows, forceful open-order cancellation, startup validators, tracing spans/log output, and paper-trading preflight/runner/reporting artifacts.

### Changed

- **Patch versions**: Bumped `nanobook` and `nanobook-python` to 0.15.1. Internal workspace crates with code changes are patch-bumped as `nanobook-broker` 0.7.1, `nanobook-risk` 0.6.1, and `nanobook-rebalancer` 0.8.1.
- **Python package metadata**: Brought `python/pyproject.toml` forward to 0.15.1 so Python packaging matches the v0.15 patch line.
- **Live-evidence accounting**: Kept remaining paper-live evidence requirements blocked on real IBKR paper-trading evidence instead of claiming completion prematurely.

### Fixed

- **Complete deprecation cleanup**: Removed Python wrapper functions for `garch_forecast`, `optimize_cvar`, and `optimize_cdar` that were accidentally left in place during the v0.15.0 release. The Rust implementations were removed but the Python wrappers remained, causing runtime failures. Users should use the replacement functions: `garch_ewma_forecast`, `inverse_cvar_weights`, and `inverse_cdar_weights`.
- **Binance and verification cleanup**: Improved Binance client error handling and fixed verification failures around Binance signing tests, backtest stop semantics, formatting, and clippy warnings.
- **Write-ahead recovery edge cases**: Fixed recovery edge cases found during the write-ahead rollout and extended coverage for incomplete intents, retries, broker reconciliation, and audit-log parsing.

### Performance

- **Portfolio price-map reuse**: Reduced redundant portfolio price-map rebuilds and reused one price map per `run_backtest` bar. Criterion evidence improved the portfolio benchmark from roughly 396 µs to 353 µs, then to about 291 µs in the follow-up pass.
- **Workspace simplify/optimization pass**: Simplified or optimized each code area with small commits: broker mock fill status handling, rebalancer quote pricing and symbol collection, risk position-map construction, Python symbol-schedule parsing/profile generation, examples report helpers, sanitizer iteration, and LOB price-map construction.

### Testing

- **Stop loss integration tests**: Added 10 comprehensive integration tests for stop loss functionality, covering ATR-based stops, short position behavior, multiple symbols, config sanitization, position flips, and interactions with rebalancing. Total backtest_bridge tests increased from 8 to 18.
- **Stop loss test cleanup**: Removed 6 low-quality stop-loss tests with ambiguous or incorrect behavior (mixed concerns, unclear validation semantics, implementation detail tests).
- **Release verification**: Current release gate passes `cargo fmt --all -- --check`, `cargo check --workspace --all-features`, `cargo test --workspace --all-features`, `cargo clippy --workspace --all-features -- -W clippy::perf -W clippy::complexity`, and Python byte-compilation for `python/`, `examples/`, and `scripts/`.

## [0.15.0] - 2026-05-13 - Documentation & Infrastructure

This release focuses on documentation completeness, infrastructure hardening, and release process improvements. No new user-facing features — all changes are either documentation, CI improvements, or cleanup of deprecated APIs.

### Removed

- Removed the deprecated `garch_forecast`, `optimize_cvar`, and `optimize_cdar`
  Rust APIs, along with their Python exports. Use `garch_ewma_forecast`,
  `inverse_cvar_weights`, and `inverse_cdar_weights` instead.

### Added

**Documentation:**
- **`docs/staying-0x.md`**: Central argument why nanobook stays pre-1.0, covering solo maintenance constraint, evolving abstractions, learning phase, and fork-at-tag path. Conditions to revisit 1.0: ≥3 maintainers, ≥6 months stable schema, demonstrated users.
- **`SEMVER.md`**: Explicit pre-1.0 versioning policy (0.x means minor versions MAY break), public API scope, breaking change categories, and practical guidance for users.
- **`docs/api-surface-audit.md`**: Comprehensive inventory of all public items across 5 crates, identifying operational items exposed publicly and Python deprecation patterns.
- **`docs/public-api/`**: cargo-public-api baselines for all workspace crates (nanobook, nanobook-broker, nanobook-risk, nanobook-rebalancer, nanobook-python). These are documentation and review aids, not a 1.0 stability contract.
- **Rustdoc examples**: Added rustdoc examples to core APIs (OrderBook, Exchange, Order, Trade, Portfolio, RiskEngine, Broker trait) and broker/risk modules.

**Infrastructure:**
- **`docs/event-log-schema.md`**: Formalized schema versioning policy with breaking change rules, CI requirements, field stability classification, and migration path for schema changes.
- **CI matrix row**: Added Rust 1.85 (MSRV) testing to CI matrix to ensure MSRV compliance.

### Changed

**Breaking (nanobook 0.14.0 → 0.15.0):**
- Removed deprecated v0.9.3 APIs: `garch_forecast`, `optimize_cvar`, `optimize_cdar`

**Breaking (nanobook-broker 0.6.0 → 0.7.0):**
- Audit-driven breaking change to align public API surface with operational needs

**Breaking (nanobook-risk 0.5.0 → 0.6.0):**
- Audit-driven breaking change — first substantive change since v0.10

**Breaking (nanobook-rebalancer 0.7.0 → 0.8.0):**
- Audit-driven breaking change to align public API surface with operational needs

**Breaking (nanobook-python 0.14.0 → 0.15.0):**
- Follows upstream breaking changes from nanobook and workspace crates

### Fixed

- **Python library name collision**: Renamed python library from "nanobook" to "nanobook_python" in python/Cargo.toml to resolve rustdoc documentation collision with the main nanobook crate.

### Implementation Notes

- **Per-crate semver**: This release uses per-crate semantic versioning, not a unified workspace bump. Synchronizing all crates to a single 0.15.0 would be marketing, not semver — risk has had effectively one change in seven releases.
- **No 1.0 tag**: nanobook remains pre-1.0 per docs/staying-0x.md.
- **Public API baselines**: The cargo-public-api baselines in docs/public-api/ are documentation and review aids, not a 1.0 stability contract. During 0.x, breaking changes remain allowed but should be visible in baseline diffs and called out in CHANGELOG.md.

## [0.14.0] - 2026-05-13 - OCaml Oracle

This release adds an OCaml reference implementation of the limit-order-book engine for differential testing against the Rust implementation. The oracle serves dual purposes: (1) detecting wrong-but-consistent bugs that fuzzing misses, and (2) signaling commitment to correctness via dual-language implementation. Both engines produce byte-identical output on a comprehensive golden corpus of 18 LOB edge cases.

### Added

**OCaml Oracle (oracle-ocaml/):**
- **`lib/price.ml`**: Int64 Price.t newtype (cents, matches Rust)
- **`lib/side.ml`**: Side = Buy | Sell
- **`lib/order.ml`**: Order variant with Limit/Market, TIF GTC/IOC/FOK, optional owner, STP policy (Off, CancelNewest, CancelOldest, DecrementAndCancel)
- **`lib/book.ml`**: Sorted association lists for price levels (O(n) insert/remove/find, correctness-focused)
- **`lib/matching.ml`**: Exhaustive pattern matching on every event variant, STP variants explicitly cased, FOK no-match/partial-cross behavior
- **`lib/replay.ml`**: Event replay with JSONL I/O, order lifecycle management
- **`lib/json.ml`**: JSON serialization/deserialization with schema_version support
- **`bin/replay_bin.ml`**: CLI binary for JSONL event log → JSONL trades conversion
- **`test/oracle_ocaml_tests.ml`**: 10 unit tests covering core invariants and order lifecycle
- **`bench/bench.ml`**: Performance benchmarks (throughput, market sweep, large book, cancellation)
- **Golden corpus**: 18 hand-curated LOB edge cases in `test/corpus/` (simple-cross, no-cross, market-order-sweep, fok variants, ioc, fifo, cancels, owner, stp policies, min/max prices)

**Documentation:**
- **`docs/event-log-schema.md`**: Formalized schema with schema_version field (optional), added owner and stp_policy fields, documented STP policy behavior
- **`docs/ocaml-oracle-v0.15-summary.md`**: Comprehensive implementation summary (CLI module loading fix, infinite loop fix, FOK partial-cross bug fix, order owner support, STP policy implementation)
- **`docs/solutions/oracle-design.md`**: Oracle design document explaining dual purpose (technical bug-finding + Jane Street signaling), triage protocol for divergence, and implementation invariants
- **`oracle-ocaml/test/corpus/README.md`**: Test case catalog

**CI:**
- **`.github/workflows/oracle.yml`**: OCaml CI job that installs OCaml 5.4 via opam-installer (cached), builds oracle, runs golden corpus, verifies byte-identical output between Rust and OCaml engines
- **Sanity-check job**: Confirms `cargo add nanobook` does NOT pull OCaml (oracle is CI-only, zero Python-surface change)

### Changed

- **Event-log schema**: Added optional `schema_version` field for future versioning, added `owner` field (optional int) for order ownership, added `stp_policy` field (Off, CancelNewest, CancelOldest, DecrementAndCancel) for self-trade prevention

### Fixed

- **CLI module loading**: Fixed runtime "Unbound module Price" errors by renaming library from `oracle` to `oracle_lib` and adding `(wrapped false)` to lib/dune
- **Infinite loop in matching**: Fixed `Matching.match_order` while loop that ran forever when no liquidity or prices didn't cross (added `continue_matching` flag and 1000-iteration safety limit)
- **FOK partial-cross bug**: Fixed FOK orders allowing partial fills (added `calculate_available_liquidity` to check liquidity before matching)
- **STP CancelNewest terminal state**: Fixed CancelNewest calling `Order.fill` before `Order.cancel` causing terminal state error

### Implementation Notes

- **Independence**: OCaml oracle written from spec only, NOT by reading Rust source
- **Type safety**: OCaml's type system and exhaustive pattern matching prevent entire classes of bugs
- **Minimal dependencies**: Uses stdlib-only (~800 LOC target), no Base/Core
- **CI-only**: OCaml oracle is repo-internal tooling, NOT published to crates.io
- **Python surface unchanged**: Zero Python API changes in this release

## [0.13.0] - 2026-05-13 - Ops Hardening

This release completes the v0.13 ops-hardening program with failure-injection testing for 9 failure modes (F1-F9) covering IBKR and Binance broker adapters, warm-restart recovery, cron idempotency, and kill-switch safety. All failure modes are now validated with end-to-end integration tests and documented operational procedures.

### Added

**Failure Mode Testing (F-series):**
- **F6 (IBKR TWS restart drill)**: Connection state tracking, heartbeat mechanism, exponential backoff reconnect (1s, 2s, 4s, 8s, 16s max, 5 attempts), open orders query via `all_open_orders()`, local order cache, state reconciliation with discrepancy detection (orphan orders, missing orders, status mismatches, position mismatches), reconciliation safety checks blocking submission when discrepancies detected, 30s target for full reconnect+reconcile cycle, 23 integration tests in `broker/tests/ibkr_*.rs`
- **F-bin1 (Binance idempotency proof)**: Order cache with JSON persistence, UUID-based client order ID generation (`nanobook-{16-char-uuid}-{sequence_number}`), duplicate detection based on client_order_id, audit log integration in JSONL format, sequence-based double-fire detection, 37 integration tests in `broker/tests/binance_*.rs`
- **F-bin2 (Binance reconnect drill)**: WebSocket implementation with connection state tracking, heartbeat mechanism (10s default), exponential backoff reconnect, account info query with position/order comparison, REST API fallback with 5s polling interval, connection mode enum (WebSocket, Rest, Auto), 24 integration tests in `broker/tests/binance_*.rs`
- **F9 (process crash mid-rebalance + warm restart)**: Audit-log → state reconstruction protocol in `rebalancer/src/recovery.rs`, `run_recover()` subcommand with reconstructed state output, broker state comparison for discrepancy detection, 5 integration tests in `rebalancer/tests/recovery.rs`
- **F8 (kill switch)**: `--kill` subcommand that cancels all open orders via broker, safety checks (confirmation prompt, dry-run mode), 4 integration tests in `rebalancer/tests/kill.rs`
- **F7 (cron double-fire)**: `--cron-mode` flag with idempotency checks, sequence-based audit log collision detection, 3 integration tests in `rebalancer/tests/cron.rs`
- **F5 (clock skew)**: Clock skew detection between strategy host and exchange via `clock_skew_ms()` in broker adapters, 2 integration tests
- **F4 (stale market data)**: Stale market data detection via timestamp comparison, 2 integration tests
- **F3 (partial fill + disconnect)**: Partial fill handling followed by disconnect simulation, state reconciliation verification, 2 integration tests
- **F2 (cancel reject race)**: Cancel reject handling with fill race detection, 2 integration tests
- **F1 (duplicate order-status callbacks)**: Duplicate callback deduplication, 2 integration tests

**Operational Documentation:**
- **`docs/operations/rebalancer-ops-hardening.md`**: Consolidated learnings from operational failure modes, categorizing existing defenses, newly hardened gaps, and operator takeaways
- **`docs/operations/warm-restart.md`**: Audit-log → state reconstruction protocol with worked examples for operators, documenting what to do after a crash (how to read audit log, confirm position state matches broker view, when to manually intervene)

**CI:**
- **`failure-injection` job**: GitHub Actions job running all failure-injection tests (IBKR F6, Binance F-bin1, Binance F-bin2) on every PR

### Changed

**Breaking (nanobook-rebalancer 0.6.0 → 0.7.0):**
- **`--cron-mode` flag**: New flag for cron-scheduled idempotent execution, adds sequence-based collision detection to prevent double-fire
- **`--kill` subcommand**: New subcommand that cancels all open orders via broker with safety checks (confirmation prompt, dry-run mode)

**Breaking (nanobook-broker 0.5.0 → 0.6.0):**
- **Binance WebSocket**: Added WebSocket connection mode with automatic fallback to REST polling
- **Connection mode enum**: New `ConnectionMode` (WebSocket, Rest, Auto) with `connection_mode` field on `BinanceBroker`
- **REST polling**: Added `poll_account_info()` and `poll_open_orders()` methods with 5s interval enforcement

### Fixed

- **IBKR reconnect logic**: Fixed connection state tracking to properly detect disconnects and trigger reconnect with backoff
- **Binance idempotency**: Fixed duplicate order detection via client_order_id cache and audit log sequence checks
- **State reconciliation**: Fixed discrepancy detection to properly identify orphan orders, missing orders, status mismatches, and position mismatches
- **Clock skew detection**: Fixed timestamp comparison logic to detect skew between strategy host and exchange
- **Stale data detection**: Fixed market data staleness checks with configurable thresholds

### Performance

- **Reconnect target**: IBKR and Binance reconnect drills target 30s for full reconnect+reconcile cycle (measured via integration tests)
- **WebSocket vs REST**: WebSocket mode provides real-time updates; REST fallback ensures reliability when WebSocket unavailable; Auto mode selects best of both
- **Audit log performance**: JSONL audit log with sequence-based collision detection adds minimal overhead to order submission

## [0.12.0] - 2026-05-12 - Backtest + Positioning

This release adds a momentum backtest case study demonstrating nanobook's portfolio simulator with parity validation against vectorbt, and completes the competitive positioning in the README.

### Added

- **`examples/momentum-backtest/report.py`**: HTML report generator with equity curve, drawdown plots, and performance metrics (Sharpe, Sortino, max drawdown)
- **`examples/momentum-backtest/COMPARISON.md`**: Line-by-line comparison of nanobook vs vectorbt covering signal generation, execution models, valuation approaches, and cost modeling
- **`docs/solutions/portfolio-sim-parity-learnings.md`**: Learnings from parity check including snapshot timing, API usage, unit conversion, architectural differences, and cost model implementation differences
- **`examples/momentum-backtest/strategy.py --output`**: JSON export flag for report generation integration
- **`examples/momentum-backtest/vectorbt_parity.py --cost-bps`**: Cost model parity check support with configurable transaction costs (default 5 bps)
- **CI momentum-backtest-smoke job**: GitHub Actions job that runs backtest on cached price data with zero costs for validation

### Changed

- **README competitive positioning**: Competitive table already present and meets v0.12 requirements (honest positioning with strengths/weaknesses)
- **examples/momentum-backtest/requirements.txt**: Added matplotlib>=3.7.0 for report plotting

### Performance

- **Parity validation**: 0.0818% max difference vs vectorbt for 2020-2022-11 (zero cost)
- **Cost model parity**: Expected differences when costs enabled due to fundamentally different cost implementations (nanobook: separate commission per share + slippage per leg; vectorbt: percentage-of-trade-value fees + percentage slippage)
- **Epsilon adjustment**: Parity check epsilon adjusted from 0.1% to 1% for cost-enabled comparisons to account for expected cost model differences
- **Known limitation**: 0.4-2.0% differences for 2022-12+ due to architectural differences (snapshot-based vs continuous valuation)
- **Strategy**: Cross-sectional momentum on S&P 100 (12-month lookback, top/bottom decile, equal-weight, monthly rebalance)

## [0.11.0] - 2026-05-12 - Replay

This release introduces a reproducible ITCH replay harness for measuring end-to-end parsing and order-book update performance on real NASDAQ TotalView-ITCH data. The focus is on honest performance measurement with proper warmup exclusion and full reproducibility documentation.

### Added

- **`examples/itch-replay/` replay harness**: End-to-end ITCH replay example that downloads NASDAQ TotalView-ITCH data, slices to a 1-minute window, runs deterministic replay, and generates performance reports
- **`examples/itch-replay/report.py`**: HTML report generator with latency histograms, message-rate timeline, spread distribution, and book reconstruction snapshots
- **`REPRODUCIBILITY.md`**: Comprehensive reproducibility guide with exact data sources, command sequences, reference environment specifications, measured performance numbers, and verification methodology
- **`docs/solutions/itch-replay-learnings.md`**: First entry in docs/solutions/ capturing learnings from ITCH replay implementation (warmup methodology, JSON corruption handling, performance characteristics)
- **`--warmup N` flag**: Exclude first N events from latency measurements (default N=1000) to avoid warmup inflation in performance numbers
- **`--snapshot-every N` flag**: Gate book snapshot generation to every Nth event (default N=1000) to reduce JSONL output size
- **CI smoke job**: GitHub Actions `examples-smoke` job that downloads ITCH data, runs replay, generates report, and validates output on every push

### Changed

- **README performance section**: Updated with measured end-to-end ITCH replay performance (p50=83ns/p95=125ns/p99=250ns parse, p50=208ns/p95=833ns/p99=3,000ns book-update on Apple M1 Pro, 16GB RAM, N=974,288 events, warmup excluded)
- **README methodology footnotes**: Added footnote ¹ for end-to-end ITCH replay (measured on real data) and footnote ² for kernel microbenchmarks (synthetic) to clearly distinguish measurement methodologies
- **README report path**: Updated from `replay-smoke` to `replay-v2` to reflect the new warmup-aware measurement methodology

### Fixed

- **Null-safety filters in report.py**: Added `isinstance(value, int) and not isinstance(value, bool)` checks to exclude boolean values from latency calculations (Python's `isinstance(True, int)` returns `True`)
- **JSON file corruption**: Documented incomplete JSON line issue on replay interrupt and added atomic file writes as candidate future work

## [0.10.0] - 2026-04-22 - Hardening Release

This release focuses on security hardening, supply-chain
improvements, test-rigor uplift, and honesty in the published
performance claims. No new user-facing features — every change
is either a safety improvement, a defense-in-depth layer, or a
correctness regression test.

**Workspace version bumps:**
- `nanobook` 0.9.3 → 0.10.0
- `nanobook-broker` 0.4.0 → 0.5.0 (breaking: default TLS flip,
  `Trade::notional` signature)
- `nanobook-risk` 0.4.1 → 0.5.0 (breaking: `RiskEngine::new`
  returns `Result`)
- `nanobook-rebalancer` 0.5.0 → 0.6.0 (breaking: audit-path
  sandboxing rejects out-of-workdir configs)

**Scope of work in this release:**
- **N-series (numerical / correctness)**: N1-N19. Welford
  variance, CVaR/Sortino defaults, FOK ghost-id contract, STP
  policy, trace-relative ridge, `periods_per_year` guard,
  `project_simplex` fallibility, `min_variance` convergence
  diagnostics, reference-parity golden fixture, and a
  deny_unknown_fields audit.
- **S-series (security)**: S1-S9. Rustls default, NaN/overflow-
  safe f64 conversions, ITCH `io::Error` in place of `unwrap`,
  checked price × quantity, `RiskEngine::new` fallibility,
  broker parse-or-warn consolidation, PII log demotion +
  `0o600` audit file, audit path sandboxing, zeroize-on-drop
  for broker credentials.
- **I-series (infra / supply chain)**: I1-I3. CI permissions
  lockdown + pinned tool versions, cargo-fuzz harness for
  matching + ITCH, cargo-mutants baseline at 89.76 %.
- **D-series (delivery)**: D0, D1, D2. Pre-tag bench
  comparison, this release commit, README honesty-revise.

For per-item migration notes and detailed rationale, see the
consolidated entries below.

### Changed (Breaking, security)

- **`nanobook-rebalancer` audit path sandboxing (S8)**:
  `AuditLog::open` now refuses to write to a path that
  canonicalizes outside the current working directory.
  `canonicalize` is symlink-aware, so a `./logs → /tmp/shared`
  symlink or a `../elsewhere` traversal is rejected with a new
  `Error::AuditPathOutsideWorkdir { path }` variant. The primary
  defense is against symlink-assisted audit-data exfiltration.
  - `AuditLog::open_in(path, workdir)` is a new variant that
    lets callers specify an explicit workdir (used by tests and
    intended for a future `--workdir` CLI flag).
  - Configs with an absolute `logging.dir` outside CWD will
    error at audit open. Migration: move the audit directory
    under CWD, or use a process-level workdir switch such as
    `cd <config-root> && rebalancer …`.

- **`nanobook_risk::RiskEngine::new` (S5)**: Signature changes
  from `-> Self` (panicking) to `-> Result<Self, RiskError>`.
  The old implementation panicked on invalid config — bad for
  Python callers (kills the interpreter with a stack trace) and
  for any code that loads configs from files. New
  `RiskError::InvalidConfig(String)` carries the offending-field
  message suitable for display to a user. Migration: callers
  with static configs use `.expect("config")`; callers with
  user-supplied configs propagate the error.
  - `rebalancer::check_risk` routes the error into the existing
    `RiskReport` failing-check channel (signature unchanged).
  - `python.RiskEngine.__init__` now raises `ValueError` on
    invalid config instead of panicking the interpreter.

- **`Trade::notional` (S4)**: Signature changes from
  `-> i64` to `-> Result<i64, ValidationError>`. The old method
  silently wrapped on `price.0 * (quantity as i64)` overflow,
  turning a large positive product into a negative `i64` (via
  two's-complement wrap) and propagating a financially-absurd
  value into P&L and risk accounting. The new implementation uses
  `checked_mul`; overflow becomes a new
  `ValidationError::NotionalOverflow { price, quantity }` variant
  that carries the offending operands.
- **`Trade::vwap`**: Signature unchanged (`-> Option<Price>`),
  but the internal notional sum is now checked at both the
  per-trade product and the running-sum stage. Any overflow
  returns `None` instead of wrapping silently.
- **Out of scope for this commit** (flagged for a follow-up S
  item): `src/portfolio/position.rs:95,104` have analogous
  `quantity * price` patterns that are not yet checked.

### Changed (Docs)

- **README performance claim (D2)**: The "120 ns submit /
  8M ops/sec" figures were aspirational — v0.9.3 itself
  measured 131 ns, and v0.10.0 measures ~155 ns after N8's
  self-trade prevention added the `Order::owner` field and
  a per-trade STP branch. The README now reports the measured
  v0.10.0 numbers (~155 ns, ~6M ops/sec) with links to
  `benches/README.md` (methodology) and
  `benches/v0.10-comparison.md` (delta). `cargo doc
  --all-features --no-deps -p nanobook -p nanobook-broker
  -p nanobook-risk -p nanobook-rebalancer` runs clean under
  `-D rustdoc::broken_intra_doc_links`; the python binding
  is excluded because its `nanobook` lib name collides with
  the main crate on docs output — tracked as a follow-up.

### Added

- **`benches/v0.10-comparison.md` (D0)**: Pre-tag benchmark
  delta against the v0.9.3 release commit (`bc4c48f`). Nine
  benchmarks crossed the plan's +5 % regression threshold;
  the largest is `submit_no_match` at +18 %, attributable to
  N8's Order-struct growth (`owner: Option<OrderOwner>`,
  +8 bytes) and the per-trade STP branch in the matcher. The
  README's "120 ns submit" claim was already aspirational in
  v0.9.3 (measured 131 ns then, 155 ns now) — scheduled for
  honesty revision under D2. Six benchmarks showed apparent
  large speedups (30 to 56 %) that the report attributes to
  noisy baseline measurement; flagged for re-bench on a
  quiet machine.

- **`fuzz/mutants-baseline.md` (I3)**: First mutation-testing
  baseline for the matching engine using `cargo-mutants` v27.0.0
  against `src/matching.rs`, `src/exchange.rs`, `src/level.rs`.
  Kill rate **89.76 %** (114 / 127 testable mutants) — clears the
  plan's ≥85 % bar. Six regression tests added this commit closed
  7 of the 20 original survivors; the 13 remaining are all
  accessor-equivalent mutations documented in the baseline
  report. Reproduce with
  `cargo mutants -p nanobook --file <file> --timeout 60
  --jobs 4 --all-features`.

- **`fuzz/` cargo-fuzz harness (I2)**: Two libFuzzer targets in a
  new nightly-only workspace isolated from the main workspace via
  `exclude = ["fuzz"]`.
  - `fuzz_submit` drives a fresh `Exchange` with arbitrary
    `SubmitLimit`, `SubmitMarket`, `Cancel`, and `Modify`
    actions; asserts no panic, book never crossed, and order
    IDs strictly monotonic with submission order (including
    FOK-rejected ghost IDs per N7).
  - `fuzz_itch` drains arbitrary bytes through `ItchParser` up
    to 64 messages per input; asserts the S3 "never panic on
    malformed input" contract under coverage-guided
    exploration.
  - Both targets run clean for 50k iterations each in local
    smoke testing; designed for long local soaks and NOT
    CI-gated. `fuzz/README.md` documents manual invocation.

### Fixed (Security)

- **GitHub Actions hardening (I1)**:
  - `ci.yml` and `release.yml` now declare
    `permissions: contents: read` at workflow scope; jobs that
    need elevated permissions (e.g., `release` needing
    `contents: write`) override at job scope. Least-privilege
    by default, regardless of the org's baseline `GITHUB_TOKEN`
    policy.
  - Tool installs are pinned:
    `cargo install cargo-deny --version 0.19.4 --locked`,
    `cargo install cargo-audit --version 0.22.1 --locked`,
    `cargo install cargo-llvm-cov --version 0.8.5 --locked`.
    Supply-chain attacks via a compromised publisher account
    can no longer slip into CI via a floating version.
  - `release.yml`'s `cargo publish || true` is replaced with a
    version-gated helper that skips only when crates.io already
    reports the current version, and surfaces all other errors
    (network, auth, malformed manifest).
  - `wheels.yml` now declares `attestations: true` on the PyPI
    publish step for readability; the default was already
    `true` under trusted publishing, but making it explicit
    signals intent.

- **`nanobook-broker` credential scrubbing (S9)**:
  `BinanceBroker` and its internal `BinanceClient` now derive
  [`zeroize::ZeroizeOnDrop`](https://docs.rs/zeroize/latest/zeroize/trait.ZeroizeOnDrop.html).
  On drop, the heap bytes backing `api_key` and `secret_key` are
  overwritten with zeros before the allocator can reclaim them,
  closing the post-free memory-inspection window. Replaces the
  ad-hoc `impl Drop` on `BinanceClient` with the idiomatic
  derive. `#[zeroize(skip)]` is applied to non-credential fields
  (reqwest client, base URL, testnet flag, quote asset) — the
  reqwest `Client` has no `Zeroize` impl and holds no secrets.
  - New `broker/README.md` documents the three things
    zeroization does NOT protect against: runtime reads,
    intermediate buffers in crypto libraries, and the PyO3
    `&str → PyString` caveat. Recommends passing credentials
    via environment variables on the Rust side to avoid
    transiting a `PyString` at all.
  - `IbkrClient` is deliberately NOT `ZeroizeOnDrop` — TWS
    authenticates at the socket layer via `(host, port,
    client_id)` and no credentials live in process memory.

- **`nanobook-rebalancer` audit file (S7)**: The JSONL audit
  file is now created with mode `0o600` on Unix (owner
  read/write only), protecting the position / equity / order
  trail from leaks through shared filesystems, misconfigured
  backups, or lax umask defaults. The mode is applied at file
  creation; pre-existing audit files keep their current
  permissions. On Windows, permissions inherit from the parent
  directory and are not restricted — the audit-file doc-comment
  flags this.
- **`nanobook-broker` IBKR logs (S7)**: Equity, cash,
  buying-power, and position-count `info!` log lines are
  demoted to `debug!`. Financial PII no longer flows to
  aggregated info-level sinks by default; set `RUST_LOG=debug`
  locally when you need the visibility. Connection-status
  logs (`"Connecting to …"`, `"Connected (client_id=…)"`) stay
  at `info!` since they carry no PII.

- **`nanobook-broker` (S6)**: Consolidated the five inline
  "parse f64 string, warn on failure, fall back to 0" blocks
  introduced by S2 into a single `parse::parse_f64_or_warn`
  helper. All warn messages now share a uniform shape
  (`"{field}: failed to parse {raw:?} as f64 ({err}); using 0"`)
  making log-scraping and alert rules simpler. Field tags are
  module-qualified (`"binance balance.free"`,
  `"ibkr account.NetLiquidation"`) so a malformed integration
  partner is identifiable at a glance. No behavior change.

- **`nanobook::itch` (S3)**: NASDAQ ITCH 5.0 message parser no
  longer panics on malformed input. Every `try_into().unwrap()`
  slice read was replaced with a fallible helper
  (`read_u16_be`, `read_u32_be`, `read_u48_be`, `read_u64_be`)
  that returns `io::Error` of kind `InvalidData` on a short
  slice, carrying the field name. The existing `min_payload`
  fast-fail gate is preserved. A proptest covers 1000 randomized
  byte sequences and asserts the parser never panics — important
  because ITCH data comes from external transports where a
  panic is a DoS vector.

- **`nanobook-broker` (S2)**: Float-to-cents conversions at every
  IBKR and Binance boundary are now NaN/overflow-safe. The pattern
  `(value * 100.0) as i64` silently produced `0` on `NaN`,
  `i64::MAX` on `+Inf`, and `i64::MIN` on `-Inf`; garbage upstream
  fields became plausible-looking positions and balances
  downstream. A new `broker::types::f64_cents_checked` (and
  `f64_to_fixed_checked` for other scales like satoshis) rejects
  non-finite and out-of-range inputs as
  `BrokerError::NonFiniteValue` / `BrokerError::ValueOutOfRange`,
  each carrying the upstream field name. Rounding switches from
  truncation to `f64::round` for consistency with N6 and to avoid
  a systematic downward bias on positive money.

### Changed (Breaking, security)

- **`nanobook-broker` (S1)**: Default TLS backend is now `rustls`
  (pure Rust, no `openssl` transitive dependency). The `binance`
  feature no longer activates `native-tls-vendored` on its own.
  New mutually-independent feature flags `rustls` (default) and
  `native-tls` select the backend.
  - Migration:
    - Default users get rustls — no action needed.
    - Callers that relied on system OpenSSL (custom CA bundles via
      `OPENSSL_CONF`, enterprise roots managed through OpenSSL)
      should build with
      `--no-default-features --features "binance native-tls"`.
    - The removal of `native-tls-vendored` also drops the vendored
      OpenSSL source tree from every broker build, so openssl CVEs
      no longer require a broker rebuild-and-republish.

## [0.9.3] - 2026-04-21 - Honesty Release

### Fixed (Security)

- **IBKR market-order encoding** (`nanobook-broker` 0.4.0, Security-C1):
  Market orders previously encoded as a `$999,999.99` buy / `$0.01` sell
  aggressive limit. On halts, auction crosses, or dark-pool routes this
  could fill at the nominal limit. The IBKR adapter now uses true market
  orders when enabled and removes the sentinel-price shim. A quote-bounded
  encoder remains for explicit fallback behavior, returning
  `BrokerError::NoQuoteForMarketOrder` when no NBBO quote is available.
  Strict rejection mode is available via `--features strict-market-reject`.

### Changed (Breaking)

- **`nanobook-broker` 0.4.0 (Security-H4)**:
  `BrokerOrder` now carries an optional
  `client_order_id: Option<ClientOrderId>`. Deterministic client order IDs
  are derived from `(scope, symbol, side, qty)` and threaded into IBKR
  `orderRef` and Binance `newClientOrderId` for broker-side deduplication
  on retry.
- **`nanobook-rebalancer` 0.5.0**:
  Rebalancer target/config structs and risk config now use
  `#[serde(deny_unknown_fields)]`. Typos in config files, for example
  `max_leverage_pct` instead of `max_leverage`, now error at parse time
  instead of silently using the default. Audit your config files.

### Deprecated

- `nanobook::optimize::optimize_cvar` / `optimize_cdar` are renamed to
  `inverse_cvar_weights` / `inverse_cdar_weights`. The old names continue
  to work with a `DeprecationWarning` / `#[deprecated]` attribute and will
  be removed in 0.11. Migration:
  `sed -i 's/optimize_cvar/inverse_cvar_weights/g; s/optimize_cdar/inverse_cdar_weights/g' <your_files>`.
- `nanobook::garch::garch_forecast` is renamed to `garch_ewma_forecast`.
  The old name gave the false impression of an MLE-fitted model; the
  implementation uses fixed EWMA-style parameters.

### Added

- `nanobook-broker` 0.4.0: deterministic `ClientOrderId` tagged into IBKR
  `orderRef` and Binance `newClientOrderId`.
- `nanobook-broker` 0.4.0: `strict-market-reject` feature flag.
- Python 3.14-compatible PyO3 bindings and CI/wheel coverage.
- Tracked Rust and Python lockfiles for repeatable CI/test dependency
  resolution.

### Docs

- Added "What nanobook is NOT" block in README to clarify scope versus
  NautilusTrader, LEAN, Hummingbot, CCXT, vectorbt, and Riskfolio-Lib.
- Added `benches/README.md` documenting exact latency-measurement
  methodology and noting that README latency numbers are not end-to-end
  live-trading latencies.

## [0.9.2] - 2026-02-12

### Added

- **Risk engine hard caps** (`nanobook-risk` 0.4.0):
  - `max_order_value_cents` — per-order notional limit (single-order and batch checks)
  - `max_batch_value_cents` — aggregate batch notional limit
  - Config validation for both fields
  - Python bindings: `RiskEngine(max_order_value_cents=..., max_batch_value_cents=...)`
- **Rebalancer execution guardrail** (`nanobook-rebalancer` 0.4.0):
  - `enforce_max_orders_per_run()` — aborts rebalance when generated orders exceed `max_orders_per_run` config
  - Config validation: `max_orders_per_run` must be > 0

### Changed

- **Rebalancer risk centralization** (`nanobook-rebalancer` 0.4.0):
  - Replaced ~140 lines of hand-rolled risk checks with delegation to `nanobook-risk` crate
  - Re-exports `RiskReport`/`RiskCheck`/`RiskStatus` from shared risk crate
- **Broker abstraction** (`nanobook-rebalancer` 0.4.0):
  - New `BrokerGateway` trait decouples execution from IBKR internals
  - `connect_ibkr()` returns `Box<dyn BrokerGateway>` instead of concrete `IbkrClient`
  - `as_connection_error()` helper replaces repeated `.map_err(...)` chains

### Removed

- **CI: MIRI job** — removed from CI pipeline (stale nightly cache issues; core matching engine already well-tested via property tests and integration tests)

### Fixed

- **README**: documented that `max_drawdown_pct` is validated at construction but not yet enforced at execution time

## [0.9.1] - 2026-02-11

### Fixed

- **CI: Linux wheels** — switched `reqwest` to `native-tls-vendored` (statically linked OpenSSL); eliminates system OpenSSL dependency in manylinux containers and avoids `ring` aarch64 cross-compilation issues
- **CI: Windows wheels** — pinned Python to 3.13 and replaced `--find-interpreter` with explicit `--interpreter python3.13`; PyO3 0.24.x does not support Python 3.14
- **CI: crates.io publish** — made publish step idempotent (`|| true` per crate) so already-published versions don't fail the job
- **Clippy** — fixed `needless_range_loop` and `excessive_precision` warnings in `src/optimize.rs`

## [0.9.0] - 2026-02-10

### Added

- **GARCH(1,1) volatility forecasting** (`src/garch.rs`):
  - `garch_forecast()` — maximum-likelihood GARCH fit with multi-step ahead forecast
  - Python binding: `py_garch_forecast()`
- **Long-only portfolio optimizers** (`src/optimize.rs`):
  - `optimize_min_variance` — minimum-variance portfolio
  - `optimize_max_sharpe` — maximum Sharpe ratio portfolio
  - `optimize_risk_parity` — risk-parity (equal risk contribution) portfolio
  - `optimize_cvar` — CVaR (Conditional Value at Risk) minimization
  - `optimize_cdar` — CDaR (Conditional Drawdown at Risk) minimization
  - All exposed to Python via PyO3
- **Extended backtest bridge** for downstream Python integrations:
  - `py_capabilities()` — feature probing contract
  - Stop-aware `backtest_weights(..., stop_cfg=...)` with stop-loss/trailing support
  - Backtest payload extensions: `holdings`, `symbol_returns`, `stop_events`
- **Python v0.9 aliases** in `__init__.py` for clean import paths

### Fixed

- Mock broker order IDs now monotonically increase across calls

## [0.8.0] - 2026-02-09

### Added

- **Analytics module**: Technical indicators replacing ta-lib dependency
  - `rsi()` — Relative Strength Index (14-period default)
  - `macd()` — Moving Average Convergence Divergence with signal line
  - `bollinger_bands()` — Bollinger Bands (mean ± 2 std)
  - `atr()` — Average True Range for volatility measurement
- **Statistics module**: Statistical functions replacing scipy
  - `spearman()` — Spearman rank correlation with p-value (custom beta implementation)
  - `quintile_spread()` — Cross-sectional quintile spread for factor analysis
  - `rank_data()` — Fractional ranking with tie handling
- **Time-series cross-validation**: `time_series_split()` replacing sklearn
  - Expanding window splits with configurable train/test sizes
  - Python bindings for sklearn-compatible usage
- **Extended portfolio metrics**:
  - `cvar` — Conditional Value at Risk (parametric, 95% default)
  - `win_rate` — Percentage of positive returns
  - `profit_factor` — Ratio of gross profits to gross losses
  - `payoff_ratio` — Average win divided by average loss
  - `kelly_criterion` — Optimal Kelly fraction for position sizing
  - `rolling_sharpe()` — Rolling Sharpe ratio (252-day window default)
  - `rolling_volatility()` — Rolling annualized volatility
- **Python bindings**: All new functions exposed via PyO3 with NumPy integration
- **Property tests**: Hypothesis-based tests for indicators, stats, CV (44 new tests)
- **Reference tests**: Validation against ta-lib, scipy, sklearn

### Changed

- **Performance optimizations**:
  - Rolling metrics use O(N) running sums instead of O(N×K) window iteration
  - RSI/MACD eliminate 3 Vec allocations in hot paths
  - CVaR computes tail mean on iterator (no intermediate Vec)
- **Code quality**: Extracted helper functions to reduce duplication
  - Binance client: `check_response()`, `validate_query_params()`
  - Risk checks: `cmp_symbol()`, `ratio_or_inf()`, `exposure()`
  - Indicators: `rsi_from_avgs()` (de-duplicated seed + loop logic)
  - Metrics: `rolling_window()` shared by rolling Sharpe/volatility

### Fixed

- **Security (audit findings)**:
  - Validated Binance query params to prevent URL parameter injection
  - Safe `u64→i64` casts in risk checks with `try_from()` + `saturating_mul()`
  - Used `saturating_abs()` to fix negative price bypass and `i64::MIN` panic
  - Fail all risk checks when equity ≤ 0 (was silently passing, incorrect)
  - Guard `CostModel` `u128→i64` cast with `try_from()`
  - Zeroize Binance API keys on drop (prevents leak in debug/logs)
  - Redact order params from debug logs (prevent sensitive data leak)
- **Correctness**:
  - CV splits now match sklearn: `test_starts = range(n - k*test_size, n, test_size)`
  - MACD: align fast EMA start with slow EMA for correct initialization
  - CVaR: use parametric VaR (`norm.ppf`) matching quantstats convention
  - Spearman p-value: custom incomplete beta via Newton-Raphson `betacf` + symmetry
- **Overflow safety**: Portfolio `execute_fill()` uses `saturating_abs/mul/sub`
- **Clippy**: Fixed `iter_cloned_collect`, `needless_range_loop`, `excessive_precision`, `inconsistent_digit_grouping`

### Removed

- **ta-lib dependency**: All indicators reimplemented in pure Rust (breaking change if using C library directly)

## [0.7.0] - 2026-02-09

### Added

- **`nanobook-broker` crate**: Generic `Broker` trait with IBKR and Binance implementations
  - `MockBroker` with builder pattern, configurable fill modes, order recording
  - IBKR: TWS/Gateway blocking client, order execution with fill monitoring
  - Binance: REST spot client, HMAC-SHA256 auth, book ticker quotes
- **`nanobook-risk` crate**: Pre-trade risk engine
  - `RiskEngine::check_order()` — single-order position/leverage/short checks
  - `RiskEngine::check_batch()` — batch validation with aggregate limits
  - `RiskConfig::validate()` — fail-fast config validation at construction
- **Backtest bridge** (`backtest_weights`): Schedule-driven portfolio simulator
  with input validation (NaN/Inf, mismatched lengths, negative prices)
- **`Symbol::from_str_truncated()`**: Safe truncation with UTF-8 boundary handling
  for external input (broker feeds, ITCH data)
- **CI hardening**:
  - `cargo-deny` + `cargo-audit` security scanning with `deny.toml` policy
  - MIRI for undefined behavior detection (strict provenance, alignment checks)
  - `cargo-llvm-cov` code coverage → Codecov
- **446 tests** (was ~333, +34%):
  - Property tests: backtest bridge, portfolio overflow, risk engine
  - Edge cases: adversarial inputs for all public APIs
  - Risk engine `check_order` tests (was zero)
  - Broker parsing: Binance JSON round-trips, IBKR type tests
  - Rebalancer integration: execution helpers, constraint overrides, diff

### Changed

- `#[track_caller]` on `Symbol::new()` for better panic diagnostics
- Bare `unwrap()` → `expect("invariant: ...")` in matching engine and stop book
- Portfolio `unwrap()` sites → graceful `match` patterns
- Rebalancer execution helpers promoted to `pub` for testability
- `RiskConfig` gains `Default` impl (reuses serde defaults)

### Fixed

- Binance auth clock panic: `.expect()` → `.unwrap_or(Duration::ZERO)`
- Backtest bridge `.zip()` silently truncating mismatched schedule lengths

### Removed

- `examples/demo.rs` — 354-line educational walkthrough (superseded by `basic_usage.rs`)
- `SPECS.md` — outdated technical spec (superseded by `DOC.md`)

## [0.6.0] - 2026-02-06

### Added

- **O(1) order cancellation**: Tombstone-based cancellation in `Level` and `OrderBook`
  - ~350x speedup for deep level cancels (170 ns vs ~60 μs)
  - `Exchange::compact()` — manual compaction to reclaim tombstone memory
- **NASDAQ ITCH 5.0 parser** (feature: `itch`):
  - `ItchParser` — streaming binary parser for ITCH 5.0 protocol
  - Handles Add, Replace, Execute, Delete, Trade, and StockDirectory messages
  - `parse_itch()` exposed to Python
- **Expanded benchmarks**: Modify, event apply, multi-symbol throughput
  - Dedicated `stops.rs` benchmark for trigger cascades and trailing updates
  - CI regression detection against v0.5 baseline

### Changed

- `sweep_equal_weight` renamed to cleaner API name
- Python type stubs updated for new methods

## [0.5.0] - 2026-02-06

### Added

- **Complete Python bindings** (`pip install nanobook` via maturin):
  - `Order`, `Position`, `Event` classes
  - `Exchange`: `events()`, `replay()`, `full_book()`, stop order queries
  - `Portfolio`: position tracking, LOB rebalancing, snapshots
  - `MultiExchange`: method forwarding, `best_prices()`
  - `Strategy`: custom Python callback support in `run_backtest()`
- **Type stubs** (`nanobook.pyi`) for IDE support
- **Automated wheel builds** for Linux, macOS, Windows in CI
- 80 Python tests

### Changed

- Modernized to Rust 2024 edition (MSRV 1.85)
- Requires Python >= 3.11

## [0.4.0] - 2026-02-06

### Added

- **Trailing stops**: Multi-method trailing stop orders
  - `submit_trailing_stop_market()` — trailing stop with market trigger
  - `submit_trailing_stop_limit()` — trailing stop with limit trigger
  - `TrailMethod::Fixed(offset)` — fixed-offset trailing
  - `TrailMethod::Percentage(pct)` — percentage-based trailing
  - `TrailMethod::Atr { multiplier, period }` — ATR-based adaptive trailing
  - Watermark tracking: sell trailing tracks highs, buy trailing tracks lows
  - Stop price re-indexes automatically when watermark updates
  - Internal ATR computation from tick-level price changes
- **Strategy trait** (feature: `portfolio`):
  - `Strategy` trait — `compute_weights(bar_index, prices, portfolio) -> Vec<(Symbol, f64)>`
  - `run_backtest()` — orchestrates rebalance-record loop
  - `EqualWeight` — built-in equal-weight strategy implementation
  - `BacktestResult` — portfolio + optional metrics
  - `sweep_strategy()` — parallel parameter sweep over strategy instances
- **Portfolio persistence** (feature: `persistence`):
  - `Portfolio::save_json()` / `Portfolio::load_json()` — JSON serialization
  - `FxHashMap<Symbol, Position>` serde via ordered vec conversion
  - `Metrics` serde support
- **Python bindings** (`pip install nanobook` via maturin):
  - `nanobook.Exchange` — full exchange API with string-based enums
  - `nanobook.Portfolio` — portfolio management and rebalancing
  - `nanobook.CostModel` — transaction cost modeling
  - `nanobook.py_compute_metrics()` — financial metrics from return series
  - `nanobook.py_sweep_equal_weight()` — parallel sweep with GIL release
  - Stop orders, trailing stops, and all query methods
  - 39 Python tests covering exchange, portfolio, and sweep
- **Portfolio benchmarks**: Criterion benchmarks for backtest and sweep performance

### Changed

- `CostModel` now derives `Copy` (was `Clone` only)
- `Event` enum no longer derives `Eq` (only `PartialEq`) due to `f64` in `TrailMethod`
- Workspace layout: `python/` added as workspace member

## [0.3.0] - 2026-02-06

### Added

- **Symbol type**: Fixed-size `Symbol([u8; 8], u8)` — `Copy`, no heap allocation, max 8 ASCII bytes
  - `Symbol::new()`, `try_new()`, `Display`, `Debug`, `AsRef<str>`
  - Custom serde support (serializes as string)
- **MultiExchange**: Multi-symbol LOB — one `Exchange` per `Symbol`
  - `get_or_create(symbol)`, `get(symbol)`, `best_prices()`, `symbols()`
- **Portfolio engine** (feature: `portfolio`):
  - `Portfolio` — cash + positions + cost model + equity tracking
  - `Position` — per-symbol tracking with VWAP entry, realized/unrealized PnL
  - `CostModel` — commission + slippage in basis points, minimum fee
  - `rebalance_simple()` — instant execution for fast parameter sweeps
  - `rebalance_lob()` — route through real LOB matching engines
  - `record_return()`, `snapshot()`, `current_weights()`, `equity_curve()`
- **Financial metrics** (feature: `portfolio`):
  - `compute_metrics()` — Sharpe, Sortino, CAGR, max drawdown, Calmar, volatility
  - `Metrics` struct with `Display` for formatted output
- **Parallel sweep** (feature: `parallel`):
  - `sweep()` — rayon-based parallel parameter sweep over strategy configurations
- **Book analytics**:
  - `BookSnapshot::imbalance()` — order book imbalance ratio
  - `BookSnapshot::weighted_mid()` — volume-weighted midpoint price
  - `Trade::vwap()` — volume-weighted average price across trades
- **Examples**: `portfolio_backtest`, `multi_symbol_lob`
- **Tests**: `portfolio_invariants` integration test suite

### Changed

- `Symbol` added to core types (not feature-gated)
- `MultiExchange` added to public API (not feature-gated)

## [0.2.0] - 2026-02-05

### Added

- **Stop orders**: Stop-market and stop-limit orders with automatic triggering
  - `submit_stop_market()` — triggers market order on price threshold
  - `submit_stop_limit()` — triggers limit order on price threshold
  - Cascading triggers with depth limit (max 100 iterations)
  - `cancel()` works on both regular and stop orders
  - New types: `StopOrder`, `StopStatus`, `StopBook`, `StopSubmitResult`
- **Input validation**: `try_submit_limit()` and `try_submit_market()` with `ValidationError`
  - `ZeroQuantity` — quantity must be > 0
  - `ZeroPrice` — price must be > 0 for limit orders
- **Serde support**: Optional `serde` feature flag adds `Serialize`/`Deserialize` to all public types
- **Persistence**: Optional `persistence` feature for file-based event sourcing
  - `exchange.save(path)` / `Exchange::load(path)` — JSON Lines format
  - `save_events()` / `load_events()` — lower-level API
- **Examples**: `basic_usage`, `market_making`, `ioc_execution`
- **CLI commands**: `stop`, `stoplimit`, `save`, `load`

### Changed

- `cancel()` now checks stop book before regular order book
- `clear_order_history()` also clears triggered/cancelled stop orders
- Event enum extended with `SubmitStopMarket` and `SubmitStopLimit` variants

## [0.1.0] - 2026-02-05

Initial release of nanobook - a deterministic limit order book and matching engine.

### Added

- **Core types**: `Price`, `Quantity`, `Timestamp`, `OrderId`, `TradeId`, `Side`
- **Order management**: Limit orders, market orders, cancel, and modify operations
- **Time-in-force**: GTC (good-til-cancelled), IOC (immediate-or-cancel), FOK (fill-or-kill)
- **Matching engine**: Price-time priority with partial fills and price improvement
- **Event logging**: Optional replay capability via feature flag (`event-log`)
- **Snapshots**: L2 order book depth snapshots
- **CLI binary**: Interactive `lob` command for exploration
- **Examples**: `demo` (interactive) and `demo_quick` (non-interactive)
- **Benchmarks**: Criterion-based throughput and latency measurements
- **CI/CD**: GitHub Actions for testing (Ubuntu/macOS), linting, and releases
- **Multi-platform releases**: Linux (x86_64, aarch64), macOS (Intel, Silicon), Windows

### Performance

- 8.3M orders/sec submission throughput (no match)
- 5M orders/sec with matching
- Sub-microsecond latencies (120ns submit, 1ns BBO query)
- O(1) best bid/ask queries via caching
- FxHash for fast order lookups

### Technical

- Rust 2021 edition, MSRV 1.70 (upgraded to Rust 2024 / MSRV 1.85 in v0.5.0)
- Minimal dependencies: `thiserror`, `rustc-hash`
- Fixed-point price representation (avoids floating-point errors)
- Deterministic via monotonic timestamps (not system clock)

[Unreleased]: https://github.com/BoringQuantSystems/nanobook/compare/v0.17.2...HEAD
[0.17.2]: https://github.com/BoringQuantSystems/nanobook/compare/v0.17.1...v0.17.2
[0.17.1]: https://github.com/BoringQuantSystems/nanobook/compare/v0.17.0...v0.17.1
[0.17.0]: https://github.com/BoringQuantSystems/nanobook/compare/v0.16.2...v0.17.0
[0.16.2]: https://github.com/BoringQuantSystems/nanobook/compare/v0.16.1...v0.16.2
[0.16.1]: https://github.com/BoringQuantSystems/nanobook/compare/v0.16.0...v0.16.1
[0.16.0]: https://github.com/BoringQuantSystems/nanobook/compare/v0.15.2...v0.16.0
[0.15.2]: https://github.com/BoringQuantSystems/nanobook/compare/v0.15.1...v0.15.2
[0.15.1]: https://github.com/BoringQuantSystems/nanobook/compare/v0.15.0...v0.15.1
[0.15.0]: https://github.com/BoringQuantSystems/nanobook/compare/v0.14.0...v0.15.0
[0.14.0]: https://github.com/BoringQuantSystems/nanobook/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/BoringQuantSystems/nanobook/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/BoringQuantSystems/nanobook/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/BoringQuantSystems/nanobook/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/BoringQuantSystems/nanobook/compare/v0.9.3...v0.10.0
[0.9.3]: https://github.com/BoringQuantSystems/nanobook/compare/v0.9.2...v0.9.3
[0.9.2]: https://github.com/BoringQuantSystems/nanobook/compare/v0.9.1...v0.9.2
[0.9.1]: https://github.com/BoringQuantSystems/nanobook/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/BoringQuantSystems/nanobook/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/BoringQuantSystems/nanobook/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/BoringQuantSystems/nanobook/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/BoringQuantSystems/nanobook/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/BoringQuantSystems/nanobook/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/BoringQuantSystems/nanobook/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/BoringQuantSystems/nanobook/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/BoringQuantSystems/nanobook/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/BoringQuantSystems/nanobook/releases/tag/v0.1.0
