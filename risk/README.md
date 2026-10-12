# nanobook-risk

Pre-trade risk checks for nanobook. It checks a single order, or a full
target-weight rebalance, against configurable limits before anything is sent
to a broker.

## Checks

`RiskConfig` sets the limits:

| Field | Limit |
|-------|-------|
| `max_position_pct` | Largest single position, as a fraction of equity (default 0.25) |
| `max_order_value_cents` | Largest single order value |
| `max_batch_value_cents` | Largest total value of one rebalance batch |
| `max_leverage` | Largest gross exposure divided by equity (default 1.5) |
| `allow_short` | Whether short positions are allowed (default true) |
| `max_short_pct` | Largest total short exposure, as a fraction of equity (default 0.30) |
| `max_trade_usd` | Single trade value above which the check gives a warning, not a failure (default 100,000) |

`RiskEngine::new` validates the config once and returns an error for NaN or
out-of-range values. `max_drawdown_pct` and `min_trade_usd` are validated and
stored, but no check in this crate uses them.

## Example

```rust
use nanobook::Symbol;
use nanobook_broker::{Account, BrokerSide};
use nanobook_risk::{RiskConfig, RiskEngine};

let engine = RiskEngine::new(RiskConfig {
    max_position_pct: 0.25,
    max_order_value_cents: 50_000_00,
    allow_short: false,
    ..RiskConfig::default()
})
.unwrap();

let account = Account {
    equity_cents: 100_000_00,
    buying_power_cents: 100_000_00,
    cash_cents: 100_000_00,
    gross_position_value_cents: 0,
};

// Buy 100 shares at $150.00 with no current positions.
let report = engine.check_order(&Symbol::new("AAPL"), BrokerSide::Buy, 100, 150_00, &account, &[]);
assert!(!report.has_failures());
```

`check_batch` takes the orders of a full rebalance, the current positions and
the target weights, and checks position limits, leverage and short exposure
for the batch as a whole. Each check in the returned `RiskReport` has a name,
a status (`Pass`, `Warn` or `Fail`) and a detail message.

The same engine is available from Python as `nanobook.RiskEngine`. See the
[nanobook README](https://github.com/BoringQuantSystems/nanobook#risk-engine).

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
