# ADR-0002: fill-policy

- **Date:** 2026-05-19
- **Status:** Accepted

## Context

The Phase 0 audit (Q1) found that fills in `backtest_weights` execute at the signal-bar price — the same `adj_close` used to compute weights (`nanobook/src/backtest_bridge.rs:212`). There is no bar shift and no lookahead guard. This is the structural lookahead that produces unrealistically smooth equity curves: the strategy observes a price, immediately fills at that same price, and records a return to the next bar.

In a real EOD strategy, orders are submitted after the close and fill at the next day's open. Filling at signal-bar close means the strategy implicitly knows the execution price at the moment it decides to trade, which is impossible in live trading.

This is the mechanism behind the Pine Script backtesting warning cited in `nanotrade/plan.md`: "realistic fills, not next-bar close defaults — that's where every 100% win rate screenshot comes from."

## Decision

Add a `FillPolicy` enum threaded through the simulator call:

```rust
pub enum FillPolicy {
    SignalBarClose,  // fills at close[t] — legacy behavior, true MoC semantics
    NextBarOpen,     // fills at open[t+1] — default
    NextBarTypical,     // fills at (high[t+1] + low[t+1] + close[t+1]) / 3 — typical price (HLC/3)
}
```

The default is `NextBarOpen`. Rationale: for EOD signal-generation strategies that submit orders after the signal-bar close, the next-bar open is the earliest honest fill price. It avoids structural lookahead while remaining realistically achievable as a market-on-open order.

**Last-bar edge case:** When `FillPolicy` is `NextBarOpen` or `NextBarTypical`, the final rebalance in the schedule requires a `t+1` bar that does not exist. The chosen behavior: the final rebalance is **skipped**, and a diagnostic entry is added to the result indicating how many shares could not be filled. The alternative — fall back to signal-bar close on the final bar — is rejected because it silently mixes two policies in a single run. The returned diagnostic makes the skip explicit and auditable.

## Alternatives Considered

1. **Per-symbol `FillPolicy`** — each symbol can have a different policy. Rejected: no realistic use case; complicates the interface and the simulation loop without benefit.
2. **Make `SignalBarClose` the default for backwards compatibility.** Rejected: backwards compatibility would defeat the purpose of the change. Existing tests that need the old behavior should pass `FillPolicy::SignalBarClose` explicitly. Parity tests are required in Phase 1b.
3. **`bool fill_next_bar: bool`** — a simple flag. Rejected: does not extend cleanly to `NextBarTypical` or future policies (e.g. TWAP, true VWAP over N bars with volume).
4. **Name the variant `NextBarVwap`.** Rejected: the formula `(H + L + C) / 3` is the "typical price" of standard technical analysis literature, not a volume-weighted average. Calling it VWAP would mislead callers who read it as `Σ(price × volume) / Σ(volume)`. A real `NextBarVwap` variant can be added later when `BarPrices` carries volume.
4. **Fall back to signal-bar close on the last bar when `NextBarOpen` has no `t+1`.** Rejected: silently mixes policies; the equity curve for the last period would use a different fill convention than every other period, which is invisible to callers.

## Consequences

- Existing test fixtures are pinned to current results (`SignalBarClose` mode). Phase 1b MUST add explicit regression tests showing bit-for-bit parity in `SignalBarClose` mode before any tests are updated for `NextBarOpen`.
- The simulation loop must index prices as `price_schedule[t+1].open` for `NextBarOpen` fills, requiring `BarPrices` (ADR-0001).
- nanolake price feeds must include OHLC (depends on ADR-0001 and Phase 0.6 of `nanotrade/plan.md`).
- The returned `BacktestBridgeResult` should carry a `skipped_final_rebalance` flag or diagnostic count.

## Evidence

- `nanotrade/docs/audit/2026-05-19-nanobook-fill-model.md` (Q1 — fill timing audit; simulation loop evidence)
- `nanobook/src/backtest_bridge.rs:212` (`portfolio.rebalance_simple(weights, prices)` fills at current-bar prices)
- `nanotrade/plan.md` (Phase 0 table row B; "latent lookahead" designation)
