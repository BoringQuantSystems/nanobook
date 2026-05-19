# ADR-0003: cost-model-semantics

- **Date:** 2026-05-19
- **Status:** Accepted

## Context

`CostModel` is defined at `nanobook/src/portfolio/cost_model.rs:17-24`:

```rust
pub struct CostModel {
    pub commission_bps: u32,
    pub slippage_bps: u32,
    pub min_trade_fee: i64,
}
```

Two defects found in the Phase 0 audit:

**Defect 1 (Q4):** The Python binding `backtest_weights` hardwires `slippage_bps = 0` and `min_trade_fee = 0`, exposing only `cost_bps: u32`. Half-implemented feature shipped as if complete (`nanobook/src/backtest_bridge.rs:169-173`).

**Defect 2 (Q3):** The current `compute_cost` implementation collapses `commission_bps` and `slippage_bps` into a single undifferentiated cash charge (`nanobook/src/portfolio/cost_model.rs:40-47`: `total_bps = commission_bps + slippage_bps`). Slippage is not modeled as a price impact on the fill price — the position is booked at the raw price and the combined cost is deducted from cash separately. This means per-position P&L attribution does not reflect microstructure cost.

The existing `min_trade_fee` field semantics (`max(bps_cost, min_trade_fee)` floor — `nanobook/src/portfolio/cost_model.rs:40-47`) are retained. This pattern matches the only retail US brokerage structures actually wired into nanotrade (`exec/ibkr.py` IBKR Tiered: `$0.0035/share, min $0.35/order`; `exec/alpaca.py`: zero commission). Additive flat-fee structures (UK routes, futures per-contract + exchange fees) are not in scope; if a future broker integration needs them, a follow-up ADR introduces an additional field rather than overloading this one.

## Decision

Restructure `CostModel` with three fields — same arity as today, but with corrected types and semantics:

```rust
pub struct CostModel {
    pub commission_bps: f64,
    pub slippage_bps: f64,
    pub min_commission: i64,  // floor on commission cost (cents); 0 = none
}
```

Cost computation per fill (per side):

```
commission_cost = max(|notional| * commission_bps / 10_000, min_commission)
```

The fill price is separately adjusted by `slippage_bps` *before* `execute_fill` records the position; slippage does not appear in `compute_cost`. See field table below.

| Field | Type | Mechanism | Rationale |
|-------|------|-----------|-----------|
| `commission_bps` | `f64` | Cash charge, per-side, on `\|notional\|`. Subject to the `min_commission` floor. | Matches real broker billing. Type widened from `u32` to allow fractional bps (e.g. 0.5 bps for IBKR US equities). |
| `slippage_bps` | `f64` | **Price impact**: fill price is adjusted before `execute_fill` records it. Buy: `effective_price = price + price * slippage_bps / 10_000`. Sell: `effective_price = price - price * slippage_bps / 10_000`. The adjusted price is used for both the position cost basis and the cash delta. | Matches TradingView `strategy(slippage=N)` semantics. Per-position P&L attribution correctly reflects microstructure cost. |
| `min_commission` | `i64` (cents) | Floor on commission: `commission = max(bps_cost, min_commission)`. Renamed from `min_trade_fee` to make the semantics unambiguous. | Models IBKR Tiered/Fixed US (`$0.0035/share, min $0.35/order`). Zero disables the floor (Alpaca's $0 commission). |

The rename `min_trade_fee` → `min_commission` is bundled into the breaking change governed by ADR-0004; in a single-caller pre-1.0 setting the rename cost is trivial relative to the clarity gained.

**Type recommendation for `commission_bps` and `slippage_bps`:**

Both fields are currently `u32` (integer basis points). nanotrade Phase 1c requires `commission_bps = 0.5` bps for US equities, which cannot be represented as a `u32` integer.

Recommended change: widen both fields to `f64` (fractional basis points). Rationale:
- `0.5` bps is a real, commonly cited US-equity commission rate (IBKR, Tradier) and must be representable.
- The alternative (`u32` in units of 0.01 bps, where 50 means 0.5 bps) is non-obvious and error-prone at call sites.
- `f64` has sufficient precision for basis-point arithmetic on notional values in the range of typical equity fills.
- The integer arithmetic invariant (prices in cents) is preserved; only the bps multiplier moves to float. The final cost computation remains in integer cents via rounding.

[Inference] The choice of `f64` over a dedicated fixed-point type is a pragmatic tradeoff. A `FixedPoint<2>` type would be more rigorous but adds implementation burden with no current benefit given the scope of this change.

## Alternatives Considered

**For slippage mechanism:**

1. **Cash-charge model (identical to commission, as currently implemented):** rejected because it does not reflect in per-position cost basis. Per-position P&L shows the raw fill price; total equity is reduced but individual position attribution is misleading.
2. **Keep commission and slippage identical in mechanism (both cash charges):** rejected because it collapses two distinct concepts and loses the ability to independently A/B test execution quality vs market-impact assumptions.
3. **Apply slippage as a separate post-fill cash deduction (not price adjustment):** rejected for the same reason as (1) — position attribution is wrong.

**For `min_trade_fee` semantics:**

1. **Keep current `max` floor behavior, retaining the original field name:** rejected only on naming grounds. The `max` semantics are correct for every broker currently wired into nanotrade.
2. **Switch the single field to additive semantics (`commission + flat_fee`):** rejected. Cannot represent IBKR US's `min $0.35/order` floor, which is the structure of nanotrade's default broker (`exec/ibkr.py`).
3. **Two fields — `min_commission` (floor) + `flat_fee_per_fill` (additive):** rejected as speculative. No broker currently wired into nanotrade needs the additive component. When a future broker (futures routing, non-US fixed routes) demands it, a follow-up ADR adds the field with concrete evidence.
4. **Add a `max_commission` cap field (for IBKR Tiered's 1%-of-notional ceiling):** rejected as speculative. Phase 1c defaults do not exercise it; if/when needed, a follow-up ADR adds it.
5. **Keep `max` semantics but rename the field to `min_commission`:** **selected.** The rename is bundled into the breaking change governed by ADR-0004 at zero marginal cost and removes the ambiguity in `min_trade_fee` (was the floor on the bps cost? on the total bill? on commission specifically?).

**For `commission_bps` type:**

1. **`u32` in units of 0.01 bps (50 = 0.5 bps):** rejected because the scaling factor is non-obvious and error-prone at every call site.
2. **Dedicated `BasisPoints` newtype wrapping `f64`:** reasonable, deferred. The value proposition increases when more code references this type; at current scope, `f64` suffices.

## Consequences

- `execute_fill` must compute `effective_price` (slippage-adjusted) before calling `pos.apply_fill(qty, effective_price)` and before computing the cash delta.
- `compute_cost` is restructured: `commission_bps` (with `min_commission` floor) flows through it as the cash charge; `slippage_bps` moves into the fill-price path and is no longer part of `compute_cost`.
- Phase 1b must add tests for each field independently: slippage (price impact verified via position cost basis), commission_bps (cash charge), min_commission (floor active when bps cost is below floor).
- nanotrade Phase 1c sets non-zero defaults appropriate to the brokers actually wired in `exec/`:
  - US equities (IBKR Tiered): `commission_bps=0.5, slippage_bps=2.0, min_commission=35` ($0.35 minimum per order).
  - Crypto: `commission_bps=10.0, slippage_bps=5.0, min_commission=0` (no floor; matches typical exchange fee structures).
- The struct retains three fields. Type changes (`commission_bps`/`slippage_bps`: `u32` → `f64`) and the field rename (`min_trade_fee` → `min_commission`) are breaking changes captured under ADR-0004.

## Evidence

- `nanobook/src/portfolio/cost_model.rs:17-24` (`CostModel` struct definition; field types confirmed as `u32` for bps fields, `i64` for `min_trade_fee`)
- `nanobook/src/portfolio/cost_model.rs:40-47` (`compute_cost`; current `max(min_fee)` behavior and collapsed bps documented)
- `nanobook/src/backtest_bridge.rs:169-173` (hardwired `slippage_bps=0`, `min_trade_fee=0`)
- `nanobook/python/src/backtest_bridge.rs:45-46` (`cost_bps: u32` — only field exposed to Python)
- `nanotrade/docs/audit/2026-05-19-nanobook-fill-model.md` (Q3 — price impact vs cash charge; Q4 — `CostModel` end-to-end)
