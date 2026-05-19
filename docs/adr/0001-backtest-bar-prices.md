# ADR-0001: backtest-bar-prices

- **Date:** 2026-05-19
- **Status:** Accepted

## Context

The current `price_schedule` argument to `backtest_weights` has type `Vec<Vec<(Symbol, i64)>>` — one scalar close price per (symbol, date) in integer cents (`nanobook/python/src/backtest_bridge.rs:51`). This shape cannot represent the open, high, or low of a bar. Every fill-realism upgrade is blocked on this:

- `NextBarOpen` fills require `open[t+1]`
- Intrabar stop simulation requires `high[t]` and `low[t]`
- VWAP fills require a VWAP price distinct from close

The shape also couples the input to a specific price type (`adj_close`) without naming it, making the semantics implicit and fragile. The simulation loop at `nanobook/src/backtest_bridge.rs:186-212` treats the scalar as an untyped price with no indication of whether it is open, close, or adjusted.

## Decision

Replace the scalar inner element with a struct carrying all four standard bar prices:

```rust
pub struct BarPrices {
    pub open: i64,
    pub high: i64,
    pub low: i64,
    pub close: i64,
}
```

The new `price_schedule` type is `Vec<Vec<(Symbol, BarPrices)>>`.

Rationale for design choices:

- Prices remain in integer cents (`i64`) to preserve the existing integer arithmetic invariant. No float conversion at the boundary.
- Volume is intentionally excluded. Cost models that need volume (e.g. VWAP-weighted impact) can receive it separately. Including volume here couples the price struct to execution concerns and increases the memory footprint of every bar.
- The struct is named `BarPrices`, not `OHLC`, to signal that this is the simulator's internal representation, not a general market-data type.

## Alternatives Considered

1. **Two parallel schedules** — `close_schedule` for signals, `open_schedule` for fills. Rejected: two arguments with a length invariant are easy to desync; the call site becomes brittle.
2. **Keep the scalar and shift in the caller** — nanotrade would pass `price_schedule[t+1]` for fills. Rejected: this corrupts the semantics of `price_schedule`; every downstream consumer (event log, attribution, future tooling) would need to know which bar offset the caller used.
3. **`Vec<f64>` of `[O, H, L, C]` by position** — rejected: loses type safety; wrong-order bugs are silent.
4. **Include `adj_close` as a fifth field** — rejected: adjustment is a data-preparation concern (nanolake's domain), not a simulator concern. The simulator receives already-adjusted prices.

## Consequences

- Breaking change to the Python binding (ADR-0004 governs migration).
- Data feeders (nanolake) must supply OHLC. nanotrade must pull adjusted OHLC from nanolake. This depends on nanolake implementing `adjust_ohlc()` (Phase 0.6 of `nanotrade/plan.md`).
- Enables Phase 1b implementation of `FillPolicy` (ADR-0002), future intrabar stop simulation, and VWAP fills.
- The `backtest_bridge` simulation loop must be updated to select the appropriate field (`open` vs `close`) based on `FillPolicy`.

## Evidence

- `nanotrade/docs/audit/2026-05-19-nanobook-fill-model.md` (Q1 — fill timing audit reveals the scalar price is implicitly `adj_close[t]`)
- `nanobook/src/backtest_bridge.rs:186-212` (simulation loop; prices are current-bar scalars with no OHLC distinction)
- `nanobook/python/src/backtest_bridge.rs:51` (`price_schedule: Vec<Vec<(String, i64)>>` — current scalar type visible at binding boundary)
