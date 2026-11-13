<p align="center">
  <img src="assets/nanobook-banner.png" alt="nanobook banner" width="900">
</p>

[![CI](https://github.com/BoringQuantSystems/nanobook/actions/workflows/ci.yml/badge.svg)](https://github.com/BoringQuantSystems/nanobook/actions/workflows/ci.yml)
[![Lean proofs](https://github.com/BoringQuantSystems/nanobook/actions/workflows/lean.yml/badge.svg)](https://github.com/BoringQuantSystems/nanobook/actions/workflows/lean.yml)
[![OCaml oracle](https://github.com/BoringQuantSystems/nanobook/actions/workflows/oracle.yml/badge.svg)](https://github.com/BoringQuantSystems/nanobook/actions/workflows/oracle.yml)
[![crates.io](https://img.shields.io/crates/v/nanobook.svg)](https://crates.io/crates/nanobook)
[![docs.rs](https://docs.rs/nanobook/badge.svg)](https://docs.rs/nanobook)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![cargo-deny](https://img.shields.io/badge/cargo--deny-audited-brightgreen)](https://github.com/BoringQuantSystems/nanobook/actions/workflows/ci.yml)

`nanobook` is a small execution kernel for the part of a trading system that is
easy to underestimate: state. Your Python code can keep doing research,
signals, sizing, and scheduling. nanobook handles the execution mechanics around
that strategy: portfolio accounting, transaction costs, stops, deterministic
limit-order-book simulation, pre-trade risk checks, and optional IBKR
rebalancing.

Eleven laws of its order-book matching are proved in Lean, on a model of the
engine that CI tests against the Rust code on every change.

Use it when you want to ask: "if my strategy emits these target weights, what
exactly happens to cash, holdings, risk checks, order-book events, and audit
logs?"

## Matching rules proved in Lean

The matching engine at the heart of nanobook comes with machine-checked proofs.
Eleven laws of the order book are stated and proved in [Lean 4](https://lean-lang.org),
a proof assistant whose kernel checks every step. The proofs live in
[`verification/lean/`](verification/lean/README.md): about 12 000 lines of Lean,
core library only, with no `sorry` and no unchecked shortcuts.

| Law | What is proved |
|-----|----------------|
| 1 | Every reachable book stays well formed: bids descend, asks ascend, no empty level, and each active order is queued exactly once at its own price. |
| 2 | The book is never crossed: the best bid stays strictly below the best ask. |
| 3 | A submit conserves quantity: trades sum to the filled quantity, and each trade takes exactly its size off the resting order. |
| 4 | Trades follow price-time priority. A resting order is skipped only when self-trade prevention cancels it. |
| 5 | Every trade prints at the resting price, at or inside the incoming limit. |
| 6 | Matching always stops: the step bound is enough, and extra steps change nothing. |
| 7 | Fill-or-kill is all or nothing: it fills completely, or it is rejected and the book is untouched. |
| 8 | Each of the four self-trade policies does exactly what its documentation says. With prevention on, no trade has the same owner on both sides. |
| 9 | A cancel changes only the cancelled order. Every other order keeps its record and its place in the queue. |
| 10 | The fast queue (tombstones, stored positions, compaction) always holds the same orders, in the same order, as a plain FIFO list. |
| 11 | A submitted order's stored record matches its status: filled, resting, cancelled, or rejected. |

**How the proofs reach the Rust code.** The laws are proved about a Lean model of
the engine. On every pull request, CI rebuilds every proof, checks that each law
rests only on Lean's three standard axioms, rechecks the whole development with an
independent kernel checker (`leanchecker --fresh`), replays the 25-case golden
corpus in Lean against the trades the Rust engine recorded for it, and runs both
engines on 1000 random event logs: their trade logs must match byte for byte.

When each law was proved, it was also run against a deliberately broken copy of the
model, to show the proof fails when the behaviour it protects is wrong. The pull
request of each proof records those runs; CI does not repeat them.

**Bugs the verification work found** (all fixed, each with a regression test):

- a price level's total quantity could overflow and strand orders;
- a cancel after a partial fill or a compaction could remove the wrong order, and
  matching could then loop;
- a finished immediate-or-cancel order, or one stopped by self-trade prevention,
  stayed active;
- a fill-or-kill order with self-trade prevention could cancel resting orders and
  then be rejected. It is now rejected with no effect.

**What is not covered.** Modify orders and stop orders are not in the model yet.
The model uses unbounded integers, so overflow is covered by Rust tests and checked
arithmetic, not by the proofs. The portfolio, risk, broker and rebalancer layers are
tested, not proved.

Check it yourself:

```bash
verification/lean/check.sh 1000   # proofs, axioms, kernel recheck, corpus, 1000 random logs
```

A second, independent cross-check runs on every PR too: a small OCaml matching
engine in `oracle-ocaml/` replays the golden corpus, and CI (`.github/workflows/oracle.yml`)
compares its trades with Rust byte for byte. To run it locally, build `corpus-replay`
(`cargo build --release --features serde --bin corpus-replay`), run the loop in
`oracle-ocaml/test/corpus/README.md`, and fix any divergence before merging.

## Architecture

```
┌─────────────────────────────────────────────────┐
│        Your Python Strategy  (private)          │
│   Factors · Signals · Sizing · Scheduling       │
├─────────────────────────────────────────────────┤
│            nanobook (Rust, open-source)         │
│  ┌──────────┬──────────┬──────────┬──────────┐  │
│  │  Broker  │   Risk   │Portfolio │   LOB    │  │
│  │   IBKR   │  Engine  │Simulator │  Engine  │  │
│  │  Binance │ PreTrade │ Backtest │ 6M ops/s │  │
│  └──────────┴──────────┴──────────┴──────────┘  │
│   Rebalancer CLI: weights → diff → execute      │
│   LOB matching: 11 laws proved in Lean 4        │
└─────────────────────────────────────────────────┘
```

## What You Can Do

- Turn a Python target-weight schedule into holdings, returns, equity curve,
  stop events, and portfolio metrics.
- Simulate transaction costs, fixed/trailing stops, and deterministic
  limit-order-book execution.
- Test order-book behavior with replayable events instead of ad hoc mocks, on a
  matching engine whose rules are proved in Lean.
- Run pre-trade risk checks for concentration, leverage, and short exposure.
- Rebalance an IBKR account from a target-weight file with dry-run, confirmation,
  reconciliation, and audit logs.

## Why It Is Worth a Look

- The boundary is sharp: Python decides **what** to trade; Rust accounts for
  **what happened**.
- The core is deterministic: same inputs, same order matching, same portfolio
  path, same replay.
- The Python package is a native PyO3 extension, so heavy loops run outside the
  GIL instead of turning into another slow Python backtester.
- The test suite is aimed at the failure modes trading code usually hides:
  edge cases, property tests, reference-parity fixtures, fuzz harnesses, and
  mutation-testing notes, on top of the Lean proofs of the matching engine.
- The scope is intentionally narrow: execution mechanics, not a UI, not a
  connector zoo, not a full research stack.

## Hardening

- Checked arithmetic for trade notional and VWAP; NaN/overflow-safe broker conversions.
- Fallible risk-engine construction instead of config-time panics.
- `rustls` default TLS backend, zeroize-on-drop for Binance credentials, and scrubbed broker logs.
- Audit logs constrained to the working directory, with `0o600` permissions on Unix.
- cargo-fuzz harnesses for matching and ITCH, plus an 89.76 % mutation-testing
  baseline for the matcher.

## Competitive Positioning

| Project | Scope | Execution Model | Python API | Rust Core | Auditability | Niche |
|---------|-------|-----------------|------------|-----------|--------------|-------|
| **nanobook** | Execution kernel | Deterministic LOB (6M ops/s) | PyO3 native extension | ✓ Full (LOB, portfolio, risk) | Very high (11 matching laws proved in Lean, event sourcing, property tests) | Research → production bridge |
| vectorbt | Research framework | Vectorized backtesting | ✓ Pure Python | ✗ | Medium (open source) | Factor research, fast backtests |
| NautilusTrader | Full platform | Event-driven, multi-venue | ✓ Python bindings | ✓ Partial | High (audit trails) | Institutional quant trading |
| Hummingbot | Bot platform | Market making, arbitrage | ✓ Pure Python | ✗ | Medium (logging) | Crypto market making |
| Freqtrade | Bot platform | Technical analysis, backtesting | ✓ Pure Python | ✗ | Low | Crypto technical trading |
| LEAN | Full platform | Event-driven, multi-asset | ✓ (C# core, Python API) | ✗ (C#) | High (institutional) | Multi-asset backtesting & live |

**Strengths:**
- Deterministic execution: same inputs → same outputs (event replay)
- Performance: LOB matching at 6M ops/sec, outside Python GIL
- Proved matching: 11 order-book laws proved in Lean 4 and checked against the Rust engine on every PR
- Auditability: event sourcing, property tests, mutation testing
- Sharp boundary: Python for strategy, Rust for execution

**Weaknesses:**
- No GUI: CLI-only, no web dashboard
- Small community: single maintainer, no plugin ecosystem
- Limited venue coverage: IBKR + Binance only (extensible via broker trait)
- No FIX protocol: FIX 4.4/5.0 not planned
- Not an OMS yet: event-sourced OMS on v1.0 roadmap

## Workspace

| Crate | Description |
|-------|-------------|
| `nanobook` | Core Rust crate: deterministic LOB, portfolio simulator, metrics, backtest bridge, GARCH, optimizers |
| `nanobook-broker` | Broker trait with IBKR and Binance adapters |
| `nanobook-risk` | Pre-trade risk engine (position limits, leverage, short exposure) |
| `nanobook-python` | PyO3 bindings for all layers |
| `nanobook-rebalancer` | CLI: target weights → IBKR execution with audit trail |

## Install

**Python:**

```bash
pip install nanobook
```

**Rust:**

```toml
[dependencies]
nanobook = "0.19"
```

**From source:**

```bash
git clone https://github.com/BoringQuantSystems/nanobook
cd nanobook
cargo build --release
cargo test

# Python bindings
cd python && maturin develop --release

# Binance adapter (feature-gated, not in PyPI wheels)
cd python && maturin develop --release --features binance
```

## The Bridge: Python Strategy → Rust Execution

The canonical integration pattern — Python computes a weight schedule,
Rust simulates the portfolio and returns metrics:

```python
import nanobook

result = nanobook.backtest_weights(
    weight_schedule=[
        [("AAPL", 0.5), ("MSFT", 0.5)],
        [("AAPL", 0.3), ("NVDA", 0.7)],
    ],
    price_schedule=[
        [("AAPL", 185_00), ("MSFT", 370_00)],
        [("AAPL", 190_00), ("MSFT", 380_00), ("NVDA", 600_00)],
    ],
    initial_cash=1_000_000_00,  # $1M in cents
    cost_bps=15,                # 15 bps round-trip
    stop_cfg={"trailing_stop_pct": 0.05},
)

print(f"Sharpe: {result['metrics'].sharpe:.2f}")
print(f"Max DD: {result['metrics'].max_drawdown:.1%}")
print(result["holdings"][-1])    # per-period symbol weights
print(result["stop_events"])     # stop trigger metadata
```

Your optimizer produces weights. `backtest_weights()` handles rebalancing,
cost modeling, position tracking, and return computation at compiled speed
with the GIL released.

Portfolio tools include fixed-parameter EWMA-style GARCH forecasting,
optimizers (min-variance, max-Sharpe, risk-parity, inverse CVaR, inverse
CDaR), and trailing/fixed stop-loss simulation — all accessible from Python.

To inspect actual backtest fills, pass `record_executions=True` to
`backtest_weights` (or `py_backtest_weights`). The result always includes
`initial_cash` in cents, `executions`, and `cash_snapshots`. The two ledgers
are `None` by default; enabled empty or rejected runs have empty ledgers.
Cash snapshots are period-close balances aligned with `returns` and `holdings`.

Each execution contains `sequence` (zero-based engine order), `period_index`
(index into your schedule, including with next-bar policies), `kind`
(`Rebalance` or `StopExit`), `symbol`, `quantity`, `side`, `price`,
`reference_price`, `notional`, `reference_notional`, `cost`, `cash_delta`, and
`cash_after`. Quantity is signed raw micro-shares
(1 share = 1,000,000; positive buys, negative sells), and `side` is `buy` or
`sell`. Price is effective cents per share after slippage; `reference_price`
is the unimpacted input price in cents per share (the current bar close for
stop exits, including on hold days without a rebalance fill bar). All other
money fields are cents. Notional truncates `abs(quantity * price) / 1_000_000`
toward zero and clamps to i64, as in the engine. Cash fields use actual
balances, including saturation at integer limits. Records contain the
quantity actually filled after affordability trimming; skipped fills produce
no record. Reference notional uses the same truncation and clamp with
`reference_price` and the final filled quantity. Side-aware slippage cost is
`notional - reference_notional` for buys and `reference_notional - notional`
for sells, in cents, up to truncation; `cost` is commission only.
Unpriced holdings still raise an error in Python.

### Optimizer Example

```python
import nanobook
import numpy as np

# Daily returns matrix (T × N)
returns = np.random.randn(252, 5) * 0.01

weights = nanobook.optimize_max_sharpe(returns, risk_free_rate=0.0)
print(dict(zip(["A","B","C","D","E"], weights)))
```

## Layer Examples

### LOB Engine (Rust)

```rust
use nanobook::{Exchange, Side, Price, TimeInForce};

let mut exchange = Exchange::new();
exchange.submit_limit(Side::Sell, Price(50_00), 100, TimeInForce::GTC);
let result = exchange.submit_limit(Side::Buy, Price(50_00), 100, TimeInForce::GTC);

assert_eq!(result.trades.len(), 1);
assert_eq!(result.trades[0].price, Price(50_00));
```

### Portfolio + Metrics (Python)

```python
portfolio = nanobook.Portfolio(1_000_000_00, nanobook.CostModel(commission_bps=5))
portfolio.rebalance_simple([("AAPL", 0.6)], [("AAPL", 150_00)])
portfolio.record_return([("AAPL", 155_00)])
metrics = portfolio.compute_metrics(252.0, 0.0)
print(f"Sharpe: {metrics.sharpe:.2f}")
```

### Broker + Risk (Python)

```python
# Pre-trade risk check
risk = nanobook.RiskEngine(max_position_pct=0.25, max_leverage=1.5)
checks = risk.check_order("AAPL", "buy", 100, 185_00,
                          equity_cents=1_000_000_00,
                          positions=[("AAPL", 200)])

# Execute through IBKR
broker = nanobook.IbkrBroker("127.0.0.1", 4002, client_id=1)
broker.connect()
oid = broker.submit_order("AAPL", "buy", 100, order_type="limit",
                          limit_price_cents=185_00)
```

### Rebalancer CLI

```bash
# Build
cargo build -p nanobook-rebalancer --release

# Dry run — show plan without executing
rebalancer run target.json --dry-run

# Execute with confirmation prompt
rebalancer run target.json

# Compare actual vs target positions
rebalancer reconcile target.json
```

## Performance

**End-to-end ITCH replay performance** (measured on NASDAQ TotalView-ITCH 2019-07-30, Apple M1 Pro, 16 GB RAM, N=973,285 measured events, warmup excluded)¹:

| Stage | p50 latency | p95 latency | p99 latency |
|-------|-------------|-------------|-------------|
| ITCH parse | 83 ns | 166 ns | 417 ns |
| LOB book-update | 250 ns | 1,042 ns | 3,541 ns |

¹ Reproducible by following `examples/itch-replay/README.md` and `REPRODUCIBILITY.md`.
Numbers exclude warmup events and reflect single-threaded replay of a 1-minute
NASDAQ trading window (09:30-09:31 ET). See `examples/itch-replay/data/report-v2/report.html`
for full latency distribution histograms.

### Kernel microbenchmarks

Single-threaded synthetic microbenchmarks (macOS arm64, v0.10.0, stock clocks)²:

| Operation | Latency | Throughput |
|-----------|---------|------------|
| Submit (no match) | ~155 ns | ~6.4M ops/sec |
| Submit (with match) | ~197 ns | ~5M ops/sec |
| BBO query | ~1.1 ns | ~900M ops/sec |
| Cancel (tombstone, deep queue) | ~385 ns | ~2.6M ops/sec |
| L2 snapshot (10 levels) | ~255 ns | ~4M ops/sec |

² Numbers are criterion medians measured with `cargo bench --bench throughput`.
Hardware, build flags, and background load all move these ±10-20 %. See
`benches/README.md` for the methodology and `benches/v0.10-comparison.md`
for the v0.9.3 → v0.10.0 delta (the v0.10.0 submit path pays a ~20 ns cost
for the new self-trade-prevention field even with STP off — a future
revision will lift the off-path check out of the matching loop).

Note: End-to-end ITCH replay numbers above reflect real-world parsing and book-update
performance on market data, while kernel microbenchmarks measure isolated operations on synthetic inputs.

Single-threaded throughput is roughly equivalent to Numba (both compile to
LLVM IR). Where Rust wins: zero cold-start, true parallelism via Rayon with
no GIL contention, and deterministic memory without GC pauses.

```bash
cargo bench
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `event-log` | Yes | Event recording for deterministic replay |
| `serde` | No | Serialize/deserialize all public types |
| `persistence` | No | File-based event sourcing (JSON Lines) |
| `portfolio` | No | Portfolio engine, position tracking, metrics, strategy trait |
| `parallel` | No | Rayon-based parallel parameter sweeps |
| `itch` | No | NASDAQ ITCH 5.0 binary protocol parser |

## Design Constraints

Engineering decisions that keep the system simple and fast:

| Constraint | Rationale |
|------------|-----------|
| **Single-threaded** | Deterministic by design — same inputs always produce same outputs |
| **In-process** | No networking overhead; wrap externally if needed |
| **Execution scope, not compliance** | STP policies are supported; regulatory workflows and circuit breakers are out of scope |
| **No complex orders** | No iceberg or pegged orders |
| **Integer prices** | Fixed-point arithmetic avoids floating-point rounding |
| **Statistics in Python** | Spearman/IC/t-stat belong in scipy/Polars — proven, mature |

## Where nanobook Fits

nanobook is not trying to replace a full trading platform. It is the execution
kernel you can put under a Python strategy when you want deterministic accounting,
matching, risk checks, and a cautious path to broker execution.

| If you need... | Use... |
|----------------|--------|
| Python signal research with vectorized factor tooling | pandas, Polars, scipy, vectorbt, Riskfolio-Lib |
| A broad venue/connector layer | CCXT, Hummingbot, or a dedicated broker stack |
| A full platform with calendars, operator workflows, and many venues | NautilusTrader or LEAN |
| A compact Rust execution layer around target weights | **nanobook** |
| A standalone order-book crate only | A narrower LOB library may be enough |

nanobook's measured LOB hot path is still fast (~155 ns submit/no-match on the
v0.10 benchmark run), but the reason to use the project is the combination: LOB,
portfolio accounting, metrics, risk, Python bindings, broker adapters, and a
rebalancer in one small workspace. Benchmark on your own hardware before making
latency-sensitive decisions.

## Documentation

- **[Documentation index](docs/README.md)** — versioning, domain language, operations, audit, and learning notes.
- **[docs.rs](https://docs.rs/nanobook)** — Rust API docs.
- **[Versioning policy](SEMVER.md)** — how nanobook handles SemVer while it remains pre-1.0.
- **[Ubiquitous language](UBIQUITOUS_LANGUAGE.md)** — glossary for order, portfolio, broker, risk, and rebalancer terms.
- **[Developer reference](docs/reference.md)** — full API reference for every layer.
- **[Lean verification](verification/lean/README.md)** — the model, the laws, and how to recheck them.

## License

MIT

## Disclaimer

This software is provided "as is", without warranty of any kind. To the extent
permitted by law, the authors and contributors are not liable for any damage, loss
or claim arising from its use or misuse. You are responsible for how you use it and
for following the laws and rules that apply to you. The full terms are in
[LICENSE](LICENSE).

nanobook is not financial advice and is not a broker. Trading can lose money,
including more than you put in, and backtests do not predict future results. The
broker adapters and the rebalancer send real orders: test them with a paper account
first and use them at your own risk. nanobook is not affiliated with or endorsed by
Interactive Brokers or Binance; their names are trademarks of their owners.
