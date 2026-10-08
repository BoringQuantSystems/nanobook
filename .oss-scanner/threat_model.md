# Threat model: nanobook

## What this project does and where untrusted input enters
nanobook is a Rust execution engine for trading backtests and live rebalancing, with a
Python binding (`python/`, built with PyO3 and maturin). It holds a deterministic
limit-order book, portfolio accounting, pre-trade risk checks, and broker adapters for
Interactive Brokers (IBKR) and Binance. Some of it moves real money, so a wrong order
is as serious as a crash.

Untrusted input enters through:
- Market data files: the ITCH 5.0 binary parser (`src/itch.rs`, binaries
  `itch-slice` and `lob`). These files come from exchanges and third-party vendors.
- Event logs and replay corpora: `src/persistence.rs` (`load_events`, snapshot load),
  `src/bin/corpus-replay.rs` (JSONL).
- Network responses from brokers: Binance REST and WebSocket messages
  (`broker/src/binance/`), IBKR TWS API messages (`broker/src/ibkr/`). Treat the
  network as hostile (a compromised proxy, a malicious or buggy endpoint).
- The rebalancer (`rebalancer/`): target-weight JSON (`target.rs`) and its TOML config
  (`config.rs`). The target file usually comes from a separate research process, so
  treat its content as untrusted. The config file is trusted.
- Values passed from Python into the Rust binding (`python/src/`): prices, quantities,
  arrays, strategy callbacks.

Trusted: the operator's own config files, environment, and API credentials.

## Components that matter most / least
- Most: anything that can make the rebalancer or a broker adapter send an order the
  risk checks should have blocked (`risk/`, `rebalancer/src/validator.rs`, `risk.rs`,
  `execution.rs`, `kill.rs`, `reconcile.rs`); credential handling in
  `broker/src/binance/auth.rs` (signing, zeroize, logging of secrets); the ITCH parser.
- In scope: the matching engine (`src/book.rs`, `matching.rs`, `exchange.rs`,
  `stop.rs`), persistence, the Python binding.
- Less important: indicators, statistics, GARCH, optimizers, scenario generation.
  These are numerical tools; wrong numbers there are bugs, not usually security issues.
- Out of scope: `oracle-ocaml/` (reference implementation used for tests), `examples/`,
  `mockups/`, `benches/`, `scripts/`, `docs/`.

## How to exercise it
- Rust: `cargo test --workspace --features "$FEATURES"` (`$FEATURES` is set in the
  image). Broker tests use mocks (`broker/src/mock.rs`); no live account is needed.
- Python: `cd python && uv run --offline python -m pytest tests/ -q`.
- Fuzz (nightly, built in the image): `cd fuzz && cargo +nightly fuzz run fuzz_itch`,
  `fuzz_submit`, `fuzz_execute_fill`.
- Binaries in `target/debug/`: `lob`, `itch-slice`, `corpus-replay`, `rebalancer`.

## How we rate severity
- Critical: an order sent to a broker that the configured risk limits should have
  blocked (position, leverage, short, trade size, max orders per run), or an order with
  the wrong side, symbol or quantity; leak of API keys or secrets (to logs, audit files,
  error messages or the network); code execution from any untrusted input above.
- High: memory unsafety (there is one `unsafe` block, in `src/types.rs`); the kill
  switch or a reconcile step that fails open; a hostile broker or network response that
  makes the rebalancer act on a wrong position or fill; the engine silently producing
  wrong fills, cash or holdings from valid input.
- Medium: a panic or unbounded memory use from a malformed ITCH file, event log, replay
  file or broker message; Python input that crashes the interpreter.
- Low: wrong error messages, problems that need a hostile local config file, numerical
  inaccuracy in analytics that does not reach orders.

## Anything to leave alone
- Performance of valid backtests, and large memory use for large but valid ITCH files.
- The rebalancer's `--force` flag, which deliberately skips the trade-size limit.
- Paper-trading defaults in `config.toml.example`.
