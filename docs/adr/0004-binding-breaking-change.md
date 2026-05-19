# ADR-0004: binding-breaking-change

- **Date:** 2026-05-19
- **Status:** Accepted (confirmed by Ricardo 2026-05-19)

## Context

ADRs 0001, 0002, and 0003 collectively require changes to the signature of the `backtest_weights` PyFunction (`nanobook/python/src/backtest_bridge.rs:45-77`):

- ADR-0001: `price_schedule` changes from `Vec<Vec<(String, i64)>>` to `Vec<Vec<(String, BarPrices)>>`
- ADR-0002: a new `fill_policy: FillPolicy` argument is required
- ADR-0003: `cost_bps: u32` is replaced by `cost_model: CostModel` (`commission_bps: f64`, `slippage_bps: f64`, `min_commission: i64` — the last renamed from `min_trade_fee`); `slippage_bps` semantics change from cash charge to price impact

These changes are mutually breaking. The question is whether to introduce them in-place or maintain a deprecation shim.

nanobook is pre-1.0 (version `0.15.1`, `nanobook/python/pyproject.toml`). Its only Python caller is nanotrade (`nanotrade/pyproject.toml`: `nanobook>=0.9`).

## Decision

**In-place replacement.** The new Python binding signature is:

```python
backtest_weights(
    weight_schedule,         # list[list[tuple[str, float]]]   — unchanged
    price_schedule,          # list[list[tuple[str, BarPrices]]] — changed (ADR-0001)
    initial_cash,            # int (cents)                      — unchanged
    cost_model,              # CostModel                        — replaces cost_bps (ADR-0003)
    fill_policy,             # FillPolicy                       — new (ADR-0002)
    periods_per_year=252.0,  # float                            — unchanged
    risk_free=0.0,           # float                            — unchanged
    stop_cfg=None,           # dict | None                      — unchanged
)
```

No legacy entry point (`backtest_weights_v2`, `backtest_weights_compat`, etc.) is preserved.

**Justification:**

1. nanobook is pre-1.0. Pre-1.0 breaking changes are expected and do not require a deprecation window.
2. The only Python caller is nanotrade. There is no external user base to protect.
3. A shim that maps the old `cost_bps: u32` to a new `CostModel` cannot map to `FillPolicy` or `BarPrices` without fabricating values. Such a shim would be silent and misleading rather than helpful.
4. With a single internal caller, a coordinated bump (nanobook lands, nanotrade pin bumps, nanotrade rewrite) is cheaper than maintaining two API surfaces indefinitely.

## Alternatives Considered

1. **Side-by-side entry point `backtest_weights_v2` with a deprecation warning on the old one.** Rejected: the single-caller rationale makes this pure maintenance debt. The old signature cannot fabricate `BarPrices` or `FillPolicy` without lying to the simulator.
2. **Python-only wrapper in nanotrade that maps old kwargs to the new API.** Rejected: `cost_bps` has no honest mapping to the new `CostModel` + `FillPolicy` + `BarPrices` triple. Any mapping would require choosing values (which `FillPolicy`? what `BarPrices` shape?) that the old caller cannot supply.
3. **Bump to 1.0 and maintain the old API as deprecated until 2.0.** Rejected: the library is not ready for a 1.0 stability commitment, and the added bureaucracy serves no one when the only caller is nanotrade.

## Consequences

- **Coordinated release sequence:** nanobook implements Phase 1b → nanobook tagged with a new version → nanotrade bumps the pin in `pyproject.toml` → nanotrade `calc/nb_backtest.py` is rewritten to use the new API → both repos merge in coordination.
- `calc/nb_backtest.py` rewrite is mandatory, not optional. The function `run_backtest_nb` must be updated to: (a) build `BarPrices` from a DataFrame with `open, high, low, close` columns; (b) accept `cost_model: nanobook.CostModel | None`; (c) accept `fill_policy: nanobook.FillPolicy = nanobook.FillPolicy.NextBarOpen`.
- The coordinated release dance should be documented in `nanobook/RELEASING.md` once Phase 1b lands. This is a follow-up action, out of scope for this ADR.
- nanotrade must refuse to run backtests with a close-only price DataFrame after this change. It must raise a clear error rather than fabricating OHLC from close.
- The nanotrade pin `nanobook>=0.9` must be tightened to the new tagged version (e.g. `nanobook>=0.16.0`) to prevent accidental use of the old API.

## Evidence

- `nanotrade/docs/audit/2026-05-19-nanobook-fill-model.md` (Q4 — `CostModel` end-to-end; binding analysis)
- `nanobook/python/src/backtest_bridge.rs:45-77` (current `PyFunction` signature)
- `nanobook/python/pyproject.toml` — `version = "0.15.1"` (confirms pre-1.0 status)
- `nanotrade/pyproject.toml` — `nanobook>=0.9` (confirms single-caller scope)
