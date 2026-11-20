# How nanobook's matching engine is verified

nanobook's order-matching core is checked in three independent ways:

1. **Proofs in Lean 4.** Eleven laws of the order book are proved about an executable
   Lean model of the engine.
2. **Rust against the Lean model.** On every push and pull request, the Rust engine and
   the Lean model replay the same event logs, and their trade logs must match byte for
   byte.
3. **Rust against an OCaml oracle.** A separate OCaml matching engine replays the
   golden corpus, and CI compares its trades with Rust.

This page explains what each layer covers, what it found, and what it does not cover.

## The model

`verification/lean/Nanobook/Spec.lean` is the matching engine written as plain Lean
functions. Levels are lists sorted best price first; each level is a price and a FIFO
list of order ids. It handles the operations the golden corpus uses:

- limit and market submits, with good-till-cancelled, immediate-or-cancel or
  fill-or-kill;
- cancels;
- an optional owner per order, with the four self-trade policies (`Off`,
  `CancelNewest`, `CancelOldest`, `DecrementAndCancel`).

`verification/lean/Nanobook/Queue.lean` models the queue the Rust engine uses for
speed: cancelled slots kept as tombstones, stored queue positions,
`popped_from_front`, and compaction. That is the layer where the worst bug lived (see
below).

The model is executable. `verification/lean/Main.lean` builds `nanobook-replay`, which
reads the same JSON Lines events as the Rust `corpus-replay` and writes trades in the
same format.

## The laws

The statements are in `verification/lean/Nanobook/Laws.lean`, with a plain-language
comment above each. Laws 1 to 9 hold for every book reachable from the empty book;
law 10 holds for every sequence of queue operations the engine can perform.

| Law | Statement in plain words | Proof file |
|-----|--------------------------|------------|
| 1 | Bids descend, asks ascend, no level is empty, each queued id is one active order at that price, and each active order is queued exactly once. | `BookShape.lean`, `WellFormed.lean` |
| 2 | The best bid is strictly below the best ask. | `Uncrossed.lean` |
| 3 | Trades of a submit sum to its filled quantity, which does not exceed the submit; each trade takes exactly its size off the resting order. | `Conservation.lean` |
| 4 | A submit trades resting orders in price-time order. An order skipped before the last trade was cancelled by self-trade prevention. | `Priority.lean` |
| 5 | Every trade prints at the resting level's price, at or inside the incoming limit. | `Price.lean` |
| 6 | The matching loop's step bound is always enough: more steps never change the result. | `Fuel.lean` |
| 7 | A fill-or-kill submit trades its whole quantity, or trades nothing and leaves every order and level unchanged. | `FillOrKill.lean` |
| 8 | Each self-trade policy does exactly what `src/matching.rs` documents. With any policy other than `Off`, no trade has the same owner on both sides. | `SelfTrade.lean` |
| 9 | A cancel changes only the cancelled order and drops only it from its level. Cancelling a missing or finished order changes nothing. | `CancelLocal.lean` |
| 10 | The tombstone queue always holds the same live orders, in the same order, as the plain lists. Every stored position points at its own order, and the totals and tombstone counts are exact. | `QueueRefines.lean` |
| 11 | A submitted order's stored record matches its status: filled means fully filled; resting means filled plus remaining equals the submit; cancelled means nothing remains; no record means a rejected fill-or-kill with no trades. | `StatusAccounting.lean` |

Two of the statements, quoted from `Laws.lean`:

```lean
/-- The best bid stays strictly below the best ask. -/
def law2 : Prop :=
  ∀ book, Reachable book →
    ∀ bid ask,
      bestPrice book.bids = some bid →
      bestPrice book.asks = some ask →
      bid < ask

/-- A fill-or-kill submit trades its whole quantity, or it trades nothing.
A rejected fill-or-kill leaves orders and levels unchanged.
Only the id counter and the timestamp counter advance. -/
def law7 : Prop :=
  ∀ book, Reachable book →
    ∀ side price quantity owner policy,
      let result := apply book (.submitLimit side price quantity .fok owner policy)
      tradeQtySum result.2 = quantity ∨
        (result.2 = [] ∧
          result.1.bids = book.bids ∧
          result.1.asks = book.asks ∧
          result.1.orders = book.orders ∧
          result.1.nextTradeId = book.nextTradeId ∧
          result.1.nextOrderId = book.nextOrderId + 1 ∧
          result.1.nextTimestamp = book.nextTimestamp + 1)
```

### What the proofs rest on

- Lean 4.35.0, core library only: no Mathlib, no other package.
- No `sorry`, `admit`, `axiom`, `partial`, `unsafe`, `implemented_by` or
  `native_decide`; `check.sh` rejects them.
- Every `lawN_holds` theorem depends only on `propext`, `Classical.choice` and
  `Quot.sound` (checked with `#print axioms`).
- `leanchecker --fresh` re-checks every declaration with an independent kernel
  replay.
- Size: about 12 000 lines (model 1 200, laws 300, proofs 10 700).

### Did the proofs catch broken models?

A proof that would also go through for a wrong model protects nothing. So, as each law
was proved, the model was deliberately broken in the way the law is meant to catch,
and the proof had to fail to build. Examples:

| Law | Deliberate break | Result |
|-----|------------------|--------|
| 6 | the step bound counts only the incoming remainder | proof fails |
| 7 | the old fill-or-kill check that counts the owner's own orders | proof fails |
| 10 | insert returns the raw slot index instead of the stored position | proof fails (`QueueRefines.lean:1255`) |
| 10 | compaction keeps stale positions | proof fails (`QueueRefines.lean:2216`) |
| 11 | a self-trade stop keeps the remainder | proof fails (`StatusAccounting.lean:10`) |
| 11 | an immediate-or-cancel cancel keeps the remainder | caught first by the law 1 proof |

These runs were done by hand when each law was proved and are recorded in the pull
request of each proof; CI does not repeat them.

## How the Rust code is tied to the model

The proofs cover the Lean model, not the Rust code. Two checks tie the two together:

- **Golden corpus.** 25 hand-checked cases in `oracle-ocaml/test/corpus/`, each with the
  trades Rust produced for it. The Lean replay must reproduce every one.
- **Random differential replay.** `verification/lean/differential.sh N` generates N
  random event logs of 64 events (`examples/corpus_gen.rs`), with random sides, prices,
  quantities, time-in-force, owners and self-trade policies, and runs both engines on
  each. The trade logs must be identical. CI runs 1000 seeds on every push; for
  0.20.0 we also ran 100 000.

Both checks compare trades only, not every order's state. A difference that never
changes a trade would not be seen; the Rust property and regression tests cover that
part.

## Mutation testing: does all of this catch real bugs in the Rust code?

[cargo-mutants](https://mutants.rs) plants one small change at a time in the code (a
`<` turned into `<=`, a `+` into `-`, a function body replaced by a constant) and runs
the test suite. If no test fails, the change "survives": the tests would not notice that
bug. For 0.20.0 we ran it on the risk, broker, portfolio and rebalancer code, with
every surviving change either killed by a new test or explained.

Result on the 0.20.0 code (only changes that compile are counted):

| Code | Changes | Caught by a failing test | Hang, stopped by the time limit | Cannot change any result | Text or unused code only | Out of reach of tests |
|---|---|---|---|---|---|---|
| `nanobook-risk` | 149 | 146 | 0 | 1 | 2 | 0 |
| `nanobook`: portfolio, metrics, backtest bridge | 1005 | 970 | 8 | 27 | 0 | 0 |
| `nanobook-broker` (IBKR and Binance) | 428 | 388 | 0 | 5 | 6 | 29 |
| `nanobook-rebalancer` (both builds) | 753 | 717 | 4 | 29 | 1 | 2 |

- "Cannot change any result": no reachable input gives a different answer, for
  example `>` against `>=` where the equal case is filtered out a few lines earlier.
  Each one is listed with its reason in the triage notes.
- "Out of reach of tests": 27 broker changes need a live Interactive Brokers
  connection (whole-function replacements on the client, its order cache and its
  callback de-duplication), one needs the Binance network, and one differs only when
  a 300 s age equals the limit to the nanosecond on a clock the test cannot set; in
  the rebalancer, two replace the interactive confirmation prompt.
- The matching engine itself (`book.rs`, `level.rs`, `matching.rs`,
  `exchange.rs`, `price_levels.rs`) had no surviving change in the full core run
  (5046 changes). A test added for 0.20.0 also pins the order record after a
  self-trade stop (`tests/stp_policy.rs`).

Sorting the surviving changes and writing the tests found twelve bugs, all fixed in
0.20.0 with a regression test: the ITCH Stock Directory symbol offset; the deflated
Sharpe ratio formula; IBKR connection-loss codes that never reached their check; IBKR
reconciliation that ignored the order's side; a Binance websocket that ignored
`testnet` and a reconnect loop that never gave up; and six in the rebalancer, among
them close orders priced on the wrong side of mid, an aborted run that exited with
code 0, and audit limits read as cents instead of dollars. The changelog lists each.

Not covered yet: the statistics, indicator, volatility, scenario and optimisation
modules (`stats.rs`, `indicators.rs`, `volatility.rs`, `scenarios.rs`,
`optimize.rs`, `garch.rs`, `cv.rs`) and the command-line tools. In the core run they
left 1153 surviving changes, most of them missing exact-value tests. They are the
next step after 0.20.0.

**How it was run.** cargo-mutants 27.1 on a separate Linux machine, each package on its
own (`--package`), the broker with `--features ibkr,binance`, the core with
`--all-features`, the rebalancer twice (default features, and
`--features write_ahead_logging,guaranteed_kill_switch`), with `--timeout 900`. The
time limit matters: the rebalancer's write-ahead tests take about 75 s, and with a
180 s limit 15 surviving changes were wrongly counted as hangs. A change counts as
caught when a test fails in at least one build that compiles it.

## What this work found

Four bugs in the Rust engine, none caught by the earlier tests, fuzzing or the 89.76 %
mutation baseline. All are fixed in 0.20.0, each with a regression test.

| Bug | Effect | Found by | Now covered by |
|-----|--------|----------|----------------|
| A cancel after a partial fill or a compaction used a stale queue index | the wrong order was cancelled; matching could stall | the verification work (model and property tests) | law 10, `tests/regression_cancel_targets_own_order.rs` |
| Level totals saturated at `u64::MAX` | orders above the limit were lost; debug builds panicked | the verification work (model and property tests) | `tests/regression_level_quantity_overflow.rs` |
| A finished immediate-or-cancel or self-trade-stopped order stayed active | it could be cancelled again; status was wrong | the verification work (model and property tests) | law 11 |
| Fill-or-kill counted the owner's own orders under self-trade prevention | the order could cancel resting orders, partly fill, then be rejected | the law 7 proof failed | law 7, `tests/regression_fok_self_trade.rs` |

One mistake in the specification was also found. Law 10 was first stated without
requiring the price levels to be sorted, and in that form it is false: a book with a
worse bid level ahead of a better one breaks it after an insert. The engine never
builds such a book. The proof attempt produced the counterexample, and the invariant
gained the sorting condition. A wrong law that happens to be provable would not show
up this way, so the statements are reviewed by people as well.

## What is not covered

- Modify orders and stop orders are not in the model.
- The model uses unbounded integers. Overflow is covered by Rust tests and checked
  arithmetic, not by the proofs.
- The portfolio, risk, broker and rebalancer layers are tested, not proved.
- The bridge between Rust and the model is testing (corpus and random replay), not
  proof.

## Check every claim yourself

The table of claims, commands and the outputs we got for 0.20.0 is in
[SECURITY.md](SECURITY.md#check-every-claim-yourself).

## Reproduce it

From the repository root:

```bash
verification/lean/check.sh 1000
```

It builds the model and proofs, rejects unchecked constructs, checks the axioms of all
eleven laws, runs `leanchecker --fresh`, replays the corpus in Lean, and compares Rust
and Lean on 1000 random logs. `check.sh 0` skips the Rust comparison for a machine
without Rust. It needs Lean 4.35.0 (`verification/lean/lean-toolchain`) and takes about
3 minutes in CI.

## Method and credit

The approach is an executable specification plus differential testing against the
production code, the method AWS uses for the Cedar authorization language: prove
properties about a model small enough to reason about, then check the real
implementation against the model on many inputs. The OCaml oracle was there first; the
Lean model adds proofs on top.
