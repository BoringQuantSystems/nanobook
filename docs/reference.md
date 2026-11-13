# nanobook developer reference

Full API documentation for the nanobook workspace. The overview is in the [README](../README.md).

---

## Table of Contents

- [Quick Start](#quick-start)
- [Core Concepts](#core-concepts)
- [Exchange API](#exchange-api)
- [Types Reference](#types-reference)
- [Book Snapshots](#book-snapshots)
- [Stop Orders & Trailing Stops](#stop-orders--trailing-stops)
- [Event Replay](#event-replay)
- [Symbol & MultiExchange](#symbol--multiexchange)
- [Portfolio Engine](#portfolio-engine)
- [Strategy Trait](#strategy-trait)
- [Backtest Bridge](#backtest-bridge)
- [Broker Abstraction](#broker-abstraction)
- [Risk Engine](#risk-engine)
- [Rebalancer CLI](#rebalancer-cli)
- [Python Bindings](#python-bindings)
- [Book Analytics](#book-analytics)
- [Persistence & Serde](#persistence--serde)
- [CLI Reference](#cli-reference)
- [Performance](#performance)

---

## Quick Start

```toml
[dependencies]
nanobook = "0.19"
```

```rust
use nanobook::{Exchange, Side, Price, TimeInForce};

let mut exchange = Exchange::new();
exchange.submit_limit(Side::Sell, Price(50_00), 100, TimeInForce::GTC);
let result = exchange.submit_limit(Side::Buy, Price(50_00), 100, TimeInForce::GTC);

assert_eq!(result.trades.len(), 1);
assert_eq!(result.trades[0].price, Price(50_00));
```

---

## Core Concepts

### Prices

Prices are integers in the **smallest currency unit** (cents for USD), avoiding floating-point errors.

```rust
let price = Price(100_50);  // $100.50
assert!(Price(100_00) < Price(101_00));
```

Display formats as dollars: `Price(10050)` prints as `$100.50`.

Constants: `Price::ZERO`, `Price::MAX` (market buys), `Price::MIN` (market sells).

### Quantities and Timestamps

- `Quantity = u64` — shares or contracts, always positive.
- `Timestamp = u64` — monotonic nanosecond counter (not system clock), guaranteeing deterministic ordering.

### Determinism

No randomness anywhere. Same sequence of operations always produces identical trades. Event replay reconstructs exact state.

---

## Exchange API

`Exchange` is the main entry point, wrapping an `OrderBook` with order submission, cancellation, modification, and queries.

### Order Submission

```rust
// Limit order — matches against opposite side, remainder handled by TIF
let result = exchange.submit_limit(Side::Buy, Price(100_00), 100, TimeInForce::GTC);

// Market order — IOC semantics at Price::MAX (buy) or Price::MIN (sell)
let result = exchange.submit_market(Side::Buy, 500);
```

### Order Management

```rust
// Cancel — O(1) via tombstones
let result = exchange.cancel(order_id);  // CancelResult { success, cancelled_quantity, error }

// Modify — cancel + replace (loses time priority, gets new OrderId)
let result = exchange.modify(order_id, Price(101_00), 200);
```

### Queries

```rust
let (bid, ask) = exchange.best_bid_ask();  // L1 — O(1)
let spread = exchange.spread();             // Option<i64>
let snap = exchange.depth(10);              // L2 — top 10 levels
let full = exchange.full_book();            // L3 — everything
let order = exchange.get_order(OrderId(1)); // Option<&Order>
let trades = exchange.trades();             // &[Trade]
```

### Memory Management

```rust
exchange.clear_trades();           // Clear trade history
exchange.clear_order_history();    // Remove filled/cancelled orders
exchange.compact();                // Reclaim tombstone memory
```

### Input Validation

The `try_submit_*` methods validate inputs before processing:

```rust
let err = exchange.try_submit_limit(Side::Buy, Price(0), 100, TimeInForce::GTC);
assert_eq!(err.unwrap_err(), ValidationError::ZeroPrice);
```

### Time-in-Force Semantics

| TIF | Partial Fill? | Rests on Book? | No Liquidity |
|-----|:---:|:---:|---|
| **GTC** | Yes | Yes (remainder) | Rests entirely |
| **IOC** | Yes | No (remainder cancelled) | Cancelled |
| **FOK** | No | No (all-or-nothing) | Cancelled |

---

## Types Reference

| Type | Definition | Description | Display |
|------|-----------|-------------|---------|
| `Price` | `struct Price(pub i64)` | Price in smallest units (cents) | `$100.50` |
| `Quantity` | `type Quantity = u64` | Number of shares/contracts | — |
| `OrderId` | `struct OrderId(pub u64)` | Unique order identifier | `O42` |
| `TradeId` | `struct TradeId(pub u64)` | Unique trade identifier | `T7` |
| `Timestamp` | `type Timestamp = u64` | Nanosecond counter (not wall clock) | — |

### Result Types

```rust
pub struct SubmitResult {
    pub order_id: OrderId,
    pub status: OrderStatus,          // New | PartiallyFilled | Filled | Cancelled
    pub trades: Vec<Trade>,
    pub filled_quantity: Quantity,
    pub resting_quantity: Quantity,
    pub cancelled_quantity: Quantity,
}

pub struct Trade {
    pub id: TradeId,
    pub price: Price,                 // Resting order's price (aggressor gets price improvement)
    pub quantity: Quantity,
    pub aggressor_order_id: OrderId,
    pub passive_order_id: OrderId,
    pub aggressor_side: Side,
    pub timestamp: Timestamp,
}
```

### Enums

| Enum | Variants | Key Methods |
|------|----------|-------------|
| `Side` | `Buy`, `Sell` | `opposite()` |
| `TimeInForce` | `GTC`, `IOC`, `FOK` | `can_rest()`, `allows_partial()` |
| `OrderStatus` | `New`, `PartiallyFilled`, `Filled`, `Cancelled` | `is_active()`, `is_terminal()` |

---

## Book Snapshots

```rust
pub struct BookSnapshot {
    pub bids: Vec<LevelSnapshot>,  // Highest price first
    pub asks: Vec<LevelSnapshot>,  // Lowest price first
    pub timestamp: Timestamp,
}

pub struct LevelSnapshot {
    pub price: Price,
    pub quantity: Quantity,
    pub order_count: usize,
}
```

| Method | Returns |
|--------|---------|
| `snap.best_bid()` / `best_ask()` | `Option<Price>` |
| `snap.spread()` | `Option<i64>` |
| `snap.mid_price()` | `Option<f64>` |
| `snap.total_bid_quantity()` / `total_ask_quantity()` | `Quantity` |
| `snap.imbalance()` | `Option<f64>` — [-1.0, 1.0], positive = buy pressure |
| `snap.weighted_mid()` | `Option<f64>` — leans toward less liquid side |

---

## Stop Orders & Trailing Stops

### Stop Orders

```rust
// Stop-market: triggers market order when last trade price hits stop
exchange.submit_stop_market(Side::Sell, Price(95_00), 100);

// Stop-limit: triggers limit order at limit_price when stop hits
exchange.submit_stop_limit(Side::Sell, Price(95_00), Price(94_50), 100, TimeInForce::GTC);
```

| Side | Triggers When |
|------|---------------|
| Buy stop | `last_trade_price >= stop_price` |
| Sell stop | `last_trade_price <= stop_price` |

Key behaviors: a plain (non-trailing) stop triggers immediately if the last trade price is already past the stop, cascade up to 100 iterations, cancel via `exchange.cancel(stop_id)`. A trailing stop does not trigger on submission; it first needs price movement to set its watermark.

### Trailing Stops

Three trailing methods — stop price tracks the market and only moves in the favorable direction:

```rust
// Fixed: triggers if price drops $2.00 from peak
exchange.submit_trailing_stop_market(Side::Sell, Price(98_00), 100, TrailMethod::Fixed(200));

// Percentage: trail by 5% from peak
exchange.submit_trailing_stop_market(Side::Sell, Price(95_00), 100, TrailMethod::Percentage(0.05));

// Adaptive trail: 2x simple moving average of |Δ price| over the last
// 14 trades. NOT Wilder's ATR (no OHLC available at the stop module);
// for true Wilder ATR, pre-compute via `indicators::atr` on bar data
// and pass the result as `TrailMethod::Fixed(offset_cents)`.
exchange.submit_trailing_stop_market(Side::Sell, Price(95_00), 100,
    TrailMethod::SmaAbsChange { multiplier: 2.0, period: 14 });
```

Trailing stop-limit variant: `submit_trailing_stop_limit()` — same parameters plus `limit_price` and `TimeInForce`.

---

## Event Replay

**Feature flag:** `event-log` (enabled by default)

Every operation is recorded as an `Event`. Replaying events on a fresh exchange produces identical state.

```rust
// Save events
let events = exchange.events().to_vec();

// Reconstruct exact state
let replayed = Exchange::replay(&events);
assert_eq!(exchange.best_bid_ask(), replayed.best_bid_ask());
```

Event types: `SubmitLimit`, `SubmitMarket`, `Cancel`, `Modify`, `SubmitStopMarket`, `SubmitStopLimit`, `SubmitTrailingStopMarket`, `SubmitTrailingStopLimit`.

Disable for max performance:

```toml
nanobook = { version = "0.19", default-features = false }
```

---

## Symbol & MultiExchange

### Symbol

Fixed-size instrument identifier. `[u8; 8]` inline — `Copy`, no heap allocation, max 8 UTF-8 bytes (a multi-byte character counts as more than one byte).

```rust
let sym = Symbol::new("AAPL");                       // panics if longer than 8 bytes
assert!(Symbol::try_new("TOOLONGNAME").is_none());   // returns None instead
```

### MultiExchange

Independent per-symbol order books:

```rust
let mut multi = MultiExchange::new();
let aapl = Symbol::new("AAPL");

multi.get_or_create(&aapl).submit_limit(Side::Sell, Price(150_00), 100, TimeInForce::GTC);

for (sym, bid, ask) in multi.best_prices() {
    println!("{sym}: bid={bid:?} ask={ask:?}");
}
```

---

## Portfolio Engine

**Feature flag:** `portfolio`

Tracks cash, positions, costs, returns, and equity over time.

```rust
use nanobook::Symbol;
use nanobook::portfolio::{CostModel, Portfolio, compute_metrics};

// Commission and slippage in basis points (f64); minimum commission per fill in cents
let cost = CostModel { commission_bps: 5.0, slippage_bps: 3.0, min_commission: 1_00 };
let mut portfolio = Portfolio::new(1_000_000_00, cost);

// Rebalance to target weights
portfolio.rebalance_simple(&[(Symbol::new("AAPL"), 0.6)], &[(Symbol::new("AAPL"), 150_00)]);

// Record period return and compute metrics (None if no returns were recorded)
portfolio.record_return(&[(Symbol::new("AAPL"), 155_00)]);
let metrics = compute_metrics(portfolio.returns(), 252.0, 0.0);
```

`Portfolio::equity_curve()` starts with the initial cash and gains one entry per `record_return` call, so it is one longer than `returns()`.

### Execution Modes

- **SimpleFill** — instant at bar prices: `portfolio.rebalance_simple(targets, prices)`
- **LOBFill** — route through `Exchange` matching engines: `portfolio.rebalance_lob(targets, &mut exchanges)`, where `exchanges` is a `MultiExchange`

### Position

Per-symbol tracking with VWAP entry price and realized PnL:

```rust
use nanobook::Symbol;
use nanobook::portfolio::{Position, Shares};

let mut pos = Position::new(Symbol::new("AAPL"));
pos.apply_fill(Shares::from_whole(100), 150_00);   // buy 100 @ $150
pos.apply_fill(Shares::from_whole(-50), 160_00);   // sell 50 @ $160 → $500 realized PnL
assert_eq!(pos.realized_pnl, 500_00);              // cents
```

`Shares` is a signed fixed-point quantity in micro-shares (`Shares::SCALE` = 1_000_000 per share). Build one with `Shares::from_whole(n)` or `Shares::from_raw(micro_shares)`; a negative value is a sell.

### Financial Metrics

`compute_metrics(&returns, periods_per_year, risk_free)` returns `Option<Metrics>` (`None` for an empty return series or a non-positive or non-finite `periods_per_year`). `Metrics` has the fields `total_return`, `cagr`, `volatility`, `sharpe`, `sortino`, `max_drawdown`, `calmar`, `num_periods`, `winning_periods`, `losing_periods`, `cvar_95`, `win_rate`, `profit_factor`, `payoff_ratio`, `kelly`.

### Parallel Sweep

**Feature flag:** `parallel` (implies `portfolio`)

```rust
use nanobook::portfolio::sweep::sweep;

let params = vec![0.5_f64, 1.0, 2.0];
let results = sweep(&params, 12.0, 0.0, |&leverage| {
    vec![0.01 * leverage, -0.005 * leverage]
});   // Vec<Option<Metrics>>, one per parameter
```

---

## Strategy Trait

**Feature flag:** `portfolio`

Implement `compute_weights()` for batch-oriented backtesting:

```rust
impl Strategy for MomentumStrategy {
    fn compute_weights(
        &self,
        bar_index: usize,
        prices: &[(Symbol, i64)],
        _portfolio: &Portfolio,
    ) -> Vec<(Symbol, f64)> {
        if bar_index < self.lookback { return vec![]; }
        let w = 1.0 / prices.len() as f64;
        prices.iter().map(|(sym, _)| (*sym, w)).collect()
    }
}

// price_series: &[Vec<(Symbol, i64)>], one Vec of prices per bar
let result = run_backtest(&strategy, &price_series, 1_000_000_00, CostModel::zero(), 12.0, 0.0);
// result.portfolio: final Portfolio; result.metrics: Option<Metrics>
```

Built-in: `EqualWeight` strategy. Parallel variant: `sweep_strategy()`.

---

## Backtest Bridge

The bridge between Python strategy code and Rust execution. Python computes a weight schedule,
Rust simulates the portfolio at compiled speed.

### Rust API

**Feature flag:** `portfolio`

```rust
use nanobook::backtest_bridge::backtest_weights;
use nanobook::portfolio::CostModel;
use nanobook::{BarPrices, FillPolicy};

let cost_model = CostModel { commission_bps: 15.0, slippage_bps: 0.0, min_commission: 0 };
let result = backtest_weights(
    &weight_schedule,        // &[Vec<(Symbol, f64)>] — target weights per period
    &price_schedule,         // &[Vec<(Symbol, BarPrices)>] — OHLC bar per symbol per period
    1_000_000_00,            // initial cash in cents
    cost_model,              // CostModel
    FillPolicy::SignalBarClose,
    252.0,                   // periods per year
    0.0,                     // risk-free rate per period
);
```

`BarPrices` has the integer fields `open`, `high`, `low`, `close` (cents). `FillPolicy` selects the price each rebalance fills at:

| `FillPolicy` | Fill price |
|--------------|------------|
| `SignalBarClose` | Close of the same bar |
| `NextBarOpen` | Open of the next bar (or of `fill_schedule[i]`) |
| `NextBarTypical` | `(high + low + close) / 3` of the next bar (or of `fill_schedule[i]`) |

A period with no fill bar (for example the last period under a next-bar policy) does not rebalance and its index is listed in `skipped_rebalances`.

`backtest_weights_with_options` takes the same arguments plus a `BacktestBridgeOptions` value: `stop_cfg: Option<BacktestStopConfig>`, `record_executions`, `fill_schedule`, `quantity_step`, `min_order_value`, `max_order_value`, `no_trade_band_bps`, `max_trades_per_rebalance`, and day and month trade budgets. `backtest_weights` runs without stop simulation.

```rust
use nanobook::backtest_bridge::{BacktestBridgeOptions, BacktestStopConfig, backtest_weights_with_options};

let result = backtest_weights_with_options(
    &weight_schedule, &price_schedule, 1_000_000_00, cost_model,
    FillPolicy::SignalBarClose, 252.0, 0.0,
    BacktestBridgeOptions {
        stop_cfg: Some(BacktestStopConfig {
            trailing_stop_pct: Some(0.05),
            atr_period: 14,
            ..Default::default()
        }),
        ..Default::default()
    },
);
```

Returns `BacktestBridgeResult`:

| Field | Type | Description |
|-------|------|-------------|
| `initial_cash` | `i64` | Starting cash in cents (also set for empty or rejected runs) |
| `returns` | `Vec<f64>` | Per-period returns |
| `equity_curve` | `Vec<i64>` | Initial equity followed by the equity after each period (cents); length is the number of periods + 1 |
| `final_cash` | `i64` | Ending cash balance |
| `metrics` | `Option<Metrics>` | Sharpe, Sortino, max drawdown, etc. |
| `holdings` | `Vec<Vec<(Symbol, f64)>>` | Per-period holdings weights |
| `symbol_returns` | `Vec<Vec<(Symbol, f64)>>` | Per-period close-to-close symbol returns |
| `stop_events` | `Vec<BacktestStopEvent>` | Stop trigger metadata (`period_index`, `symbol`, `trigger_price`, `exit_price`, `reason`) |
| `skipped_rebalances` | `Vec<usize>` | Period indices with no fill bar |
| `executions` | `Option<Vec<ExecutionRecord>>` | Actual fills; `Some` only when `record_executions` is set |
| `cash_snapshots` | `Option<Vec<i64>>` | Cash at each period close; `Some` only when `record_executions` is set |
| `unpriced_holding` | `Option<UnpricedHolding>` | Set when a held symbol has no positive price; treat it as an error, not a finished run |

Invalid inputs (schedules of different length, non-positive cash, NaN or infinite weights, negative prices, a bar with `high` below `low`) return an empty result: no returns and an `equity_curve` of `[initial_cash]`.

### Python API

```python
def bar(open, high, low, close):
    return nanobook.BarPrices(open=open, high=high, low=low, close=close)

result = nanobook.backtest_weights(
    weight_schedule=[[("AAPL", 0.5), ("MSFT", 0.5)], ...],
    price_schedule=[[("AAPL", bar(185_00, 186_00, 184_00, 185_00)),
                     ("MSFT", bar(370_00, 372_00, 369_00, 371_00))], ...],
    initial_cash=1_000_000_00,
    cost_model=nanobook.CostModel(commission_bps=15.0),
    fill_policy=nanobook.FillPolicy.SignalBarClose,
    periods_per_year=252.0,
    risk_free=0.0,
    stop_cfg={"trailing_stop_pct": 0.05},
)
# result["returns"], result["equity_curve"], result["metrics"] (a Metrics object),
# result["holdings"], result["symbol_returns"], result["stop_events"],
# result["final_cash"], result["initial_cash"], result["skipped_rebalances"],
# result["executions"], result["cash_snapshots"]
```

`fill_policy` is `FillPolicy.SignalBarClose`, `FillPolicy.NextBarOpen` or `FillPolicy.NextBarTypical`. `stop_cfg` accepts the keys `fixed_stop_pct`, `trailing_stop_pct`, `atr_multiple` and `atr_period`. Optional keyword arguments: `fill_schedule`, `quantity_step`, `min_order_value`, `max_order_value`, `no_trade_band_bps`, `max_trades_per_rebalance`, `max_trades_per_day`, `max_trades_per_month`, `max_traded_value_per_month`, `period_day_ordinal`, `period_month_ordinal` and `record_executions` (the ledger keys `executions` and `cash_snapshots` are `None` unless it is `True`). A held symbol without a positive price raises `ValueError`.

`equity_curve` holds the initial equity plus one entry per period, as in the Rust result.

GIL is released during computation for maximum throughput.

Clean aliases (no `py_` prefix) are exported for new integrations:
`backtest_weights`, `capabilities`, `garch_ewma_forecast`,
`inverse_cvar_weights`, `inverse_cdar_weights`, and other optimizer helpers.
`py_backtest_weights` remains available with the same arguments.

### Capability Probing Pattern

Downstream Python applications can probe nanobook capabilities before choosing a Rust-backed fast path or a local fallback:

```python
import nanobook

def has_nanobook_feature(name: str) -> bool:
    caps = set(nanobook.py_capabilities()) if hasattr(nanobook, "py_capabilities") else set()
    if name in caps:
        return True

    symbol_map = {
        "backtest_stops": "py_backtest_weights",
        "garch_ewma_forecast": "py_garch_ewma_forecast",
        "optimize_min_variance": "py_optimize_min_variance",
        "optimize_max_sharpe": "py_optimize_max_sharpe",
        "optimize_risk_parity": "py_optimize_risk_parity",
        "inverse_cvar_weights": "py_inverse_cvar_weights",
        "inverse_cdar_weights": "py_inverse_cdar_weights",
        "backtest_holdings": "py_backtest_weights",
    }
    sym = symbol_map.get(name)
    return bool(sym and hasattr(nanobook, sym))
```

---

## Broker Abstraction

**Crate:** `nanobook-broker`

Generic trait over brokerages with concrete adapters for IBKR and Binance.

### Broker Trait

```rust
pub trait Broker {
    fn connect(&mut self) -> Result<(), BrokerError>;
    fn disconnect(&mut self) -> Result<(), BrokerError>;
    fn positions(&self) -> Result<Vec<Position>, BrokerError>;
    fn account(&self) -> Result<Account, BrokerError>;
    fn submit_order(&self, order: &BrokerOrder) -> Result<OrderId, BrokerError>;
    fn order_status(&self, id: OrderId) -> Result<BrokerOrderStatus, BrokerError>;
    fn open_orders(&self) -> Result<Vec<BrokerOrderStatus>, BrokerError>;
    fn cancel_order(&self, id: OrderId) -> Result<(), BrokerError>;
    fn quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError>;
}
```

`OrderId` here is `nanobook_broker::OrderId(pub u64)`, the broker-assigned id, not the matching engine's `nanobook::OrderId`.

### Key Types

```rust
pub struct Position {
    pub symbol: Symbol,
    pub quantity: i64,             // Positive = long, negative = short
    pub avg_cost_cents: i64,
    pub market_value_cents: i64,
    pub unrealized_pnl_cents: i64,
}

pub struct Account {
    pub equity_cents: i64,
    pub buying_power_cents: i64,
    pub cash_cents: i64,
    pub gross_position_value_cents: i64,
}

pub struct BrokerOrder {
    pub symbol: Symbol,
    pub side: BrokerSide,          // Buy or Sell
    pub quantity: u64,
    pub order_type: BrokerOrderType,  // Market | Limit(Price) | Stop(Price) | StopLimit { stop, limit }
    pub client_order_id: Option<ClientOrderId>,  // idempotency key, see below
}

pub struct Quote {
    pub symbol: Symbol,
    pub bid_cents: i64,
    pub ask_cents: i64,
    pub last_cents: i64,
    pub volume: u64,
    pub timestamp: SystemTime,     // when the quote was received from the broker
}

pub struct BrokerOrderStatus {
    pub id: OrderId,
    pub status: OrderState,        // Pending | Submitted | PartiallyFilled | Filled | Cancelled | Rejected
    pub filled_quantity: u64,
    pub remaining_quantity: u64,
    pub avg_fill_price_cents: i64,
}
```

`client_order_id` lets a retried submission be recognised as the same order. `ClientOrderId::derive(scope, symbol, side, qty)` builds a deterministic 32-character id from those inputs; `ClientOrderId::new(value)` accepts a caller-supplied id of 1 to 36 characters from `A-Z a-z 0-9 _ . -`. Set `client_order_id: None` to submit without one.

`Quote::is_stale(threshold_sec)` returns `true` when the quote is older than `threshold_sec` seconds, so a caller can refuse to trade on an outdated price.

### IBKR Adapter

**Feature:** `ibkr`

Connects to TWS/Gateway via the `ibapi` crate (blocking API).

```rust
let mut broker = IbkrBroker::new("127.0.0.1", 4002, 1);  // 4002 = paper, 4001 = live
broker.connect()?;
let positions = broker.positions()?;
let quote = broker.quote(&Symbol::new("AAPL"))?;
```

### Binance Adapter

**Feature:** `binance`

REST API via `reqwest::blocking`. Converts nanobook symbols (e.g., "BTC") to Binance pairs (e.g., "BTCUSDT").

```rust
let mut broker = BinanceBroker::new(api_key, secret_key, true);  // testnet
broker.connect()?;
```

### Python

```python
broker = nanobook.IbkrBroker("127.0.0.1", 4002, client_id=1)
broker.connect()
positions = broker.positions()   # List[Dict] with symbol, quantity, avg_cost_cents, ...
oid = broker.submit_order("AAPL", "buy", 100, order_type="limit", limit_price_cents=185_00,
                          client_order_id="rebalance-2026-02-08-AAPL")
quote = broker.quote("AAPL")     # Dict with symbol, bid_cents, ask_cents, last_cents, volume

broker = nanobook.BinanceBroker(api_key, secret_key, testnet=True, quote_asset="USDT")
```

`order_type` is `"market"`, `"limit"`, `"stop"` or `"stop_limit"` for `IbkrBroker` (stop orders take `stop_price_cents`). `BinanceBroker.submit_order` accepts only `"market"` and `"limit"` and has no `stop_price_cents`. The Python quote dict has no `timestamp`, and the Python brokers have no `open_orders` method; both exist only in the Rust API. `BinanceBroker` is present only in wheels built with `maturin develop --release --features binance`.

---

## Risk Engine

**Crate:** `nanobook-risk`

Pre-trade risk validation for single orders and rebalance batches.

### RiskConfig

```rust
pub struct RiskConfig {
    pub max_position_pct: f64,       // Max single position as fraction of equity (default 0.25)
    pub max_order_value_cents: i64,  // Max single order value
    pub max_batch_value_cents: i64,  // Max rebalance batch value
    pub max_leverage: f64,           // Max gross exposure / equity (default 1.5)
    pub max_drawdown_pct: f64,       // Circuit breaker threshold (default 0.20)
    pub allow_short: bool,           // Allow short positions (default true)
    pub max_short_pct: f64,          // Max short exposure fraction (default 0.30)
    pub min_trade_usd: f64,
    pub max_trade_usd: f64,          // Max single trade USD (default 100,000)
}
```

Notes:

- `max_drawdown_pct` is validated at engine construction and preserved in config,
  but not yet used in execution-time checks.

### Single Order Check

```rust
let engine = RiskEngine::new(RiskConfig::default()).expect("valid risk config");
let report = engine.check_order(
    &Symbol::new("AAPL"),
    BrokerSide::Buy,
    100,              // quantity
    185_00,           // price in cents
    &account,
    &current_positions,
);

if report.has_failures() {
    // Order violates risk limits — position concentration, short selling, etc.
}
```

### Batch Check

Validates a full rebalance against position limits, leverage, and short exposure:

```rust
let report = engine.check_batch(
    &orders,              // &[(Symbol, BrokerSide, u64, i64)]
    &account,
    &current_positions,
    &target_weights,      // &[(Symbol, f64)]
);
```

### RiskReport

```rust
pub struct RiskReport {
    pub checks: Vec<RiskCheck>,
}

pub struct RiskCheck {
    pub name: &'static str,
    pub status: RiskStatus,  // Pass | Warn | Fail
    pub detail: String,
}

impl RiskReport {
    pub fn has_failures(&self) -> bool;
    pub fn has_warnings(&self) -> bool;
}
```

### Python

```python
risk = nanobook.RiskEngine(max_position_pct=0.25, max_leverage=1.5)

# Single order
checks = risk.check_order("AAPL", "buy", 100, 185_00,
                          equity_cents=1_000_000_00,
                          positions=[("AAPL", 200)])

# Batch (full rebalance)
checks = risk.check_batch(
    orders=[("AAPL", "buy", 100, 185_00), ("MSFT", "sell", 50, 370_00)],
    equity_cents=1_000_000_00,
    positions=[("AAPL", 200), ("MSFT", 100)],
    target_weights=[("AAPL", 0.6), ("MSFT", 0.2)],
)
# Each check: {"name": "...", "status": "PASS|WARN|FAIL", "detail": "..."}
```

---

## Rebalancer CLI

**Crate:** `nanobook-rebalancer`

CLI tool that bridges target weights to IBKR execution with risk checks, rate limiting, and audit trail.

### Pipeline

1. Read target weights from `target.json` (output of your optimizer)
2. Connect to IBKR Gateway for live positions, prices, account data
3. Compute CURRENT → TARGET diff (share quantities, limit prices)
4. Run pre-trade risk checks (position limits, leverage, short exposure)
5. Show plan, confirm (or `--force` for automation)
6. Execute limit orders with rate limiting and timeout-based cancellation
7. Reconcile and log to JSONL audit trail

### Commands

```bash
rebalancer status                     # Check IBKR connection
rebalancer positions                  # Show current positions
rebalancer run target.json            # Plan → confirm → execute
rebalancer run target.json --dry-run  # Plan only
rebalancer run target.json --force    # Skip confirmation (cron/automation)
rebalancer run target.json --force --cron-mode  # Reject a second run for a window already complete
rebalancer reconcile target.json      # Compare actual vs target
rebalancer recover target.json        # Recover from crash using audit log
rebalancer recover target.json --dry-run  # Show the recovery plan only
rebalancer kill                       # SIGTERM the running rebalancer, check for dangling orders
```

All commands read `config.toml` from the current directory; `--config <path>` selects another file.

### Operations & Recovery

The rebalancer includes crash recovery via audit log reconstruction. After a process crash or TWS restart, use `rebalancer recover` to reconstruct state and determine the appropriate recovery action (Restart, Resume, ManualReview, or Rollback).

### target.json

```json
{
  "timestamp": "2026-02-08T15:30:00Z",
  "targets": [
    { "symbol": "AAPL", "weight": 0.40 },
    { "symbol": "MSFT", "weight": 0.30 },
    { "symbol": "SPY",  "weight": -0.10 },
    { "symbol": "QQQ",  "weight": 0.20 }
  ],
  "constraints": {
    "max_position_pct": 0.40,
    "max_leverage": 1.5
  }
}
```

Positive weights are long, negative are short. Symbols absent from the target but present in the account get closed. See `rebalancer/config.toml.example` for the full configuration reference.

---

## Python Bindings

Install: `pip install nanobook` or `cd python && maturin develop --release`

### Exchange

```python
ex = nanobook.Exchange()
result = ex.submit_limit("buy", 10050, 100, "gtc")
result = ex.submit_market("sell", 50)
ex.cancel(result.order_id)
bid, ask = ex.best_bid_ask()
snap = ex.depth(10)
```

### Stop Orders

```python
ex.submit_stop_market("sell", 9500, 100)
ex.submit_stop_limit("buy", 10500, 10600, 100, "gtc")
ex.submit_trailing_stop_market("sell", 9500, 100, "percentage", 0.05)
# trail_type: "fixed" (cents), "percentage" (fraction), or "sma_abs_change"
# (trail_value = multiplier, requires atr_period=N)
```

### Portfolio

```python
portfolio = nanobook.Portfolio(1_000_000_00, nanobook.CostModel(commission_bps=10))
portfolio.rebalance_simple([("AAPL", 0.6)], [("AAPL", 150_00)])
portfolio.record_return([("AAPL", 155_00)])
metrics = portfolio.compute_metrics(252.0, 0.0)
```

### Strategy Callback

```python
result = nanobook.run_backtest(
    strategy=lambda bar, prices, portfolio: [("AAPL", 0.5), ("GOOG", 0.5)],
    price_series=[{"AAPL": 150_00, "GOOG": 280_00}] * 252,
    initial_cash=1_000_000_00,
    cost_model=nanobook.CostModel.zero(),
)
```

### ITCH Parser

```python
events = nanobook.parse_itch("data/sample.itch")
```

---

## Book Analytics

### Imbalance

```rust
let snap = exchange.depth(10);
if let Some(imb) = snap.imbalance() {
    // [-1.0, 1.0]: positive = buy pressure
    println!("Imbalance: {imb:.4}");
}
```

### Weighted Midpoint

```rust
if let Some(wmid) = snap.weighted_mid() {
    println!("Weighted mid: {wmid:.2}");
}
```

### VWAP

```rust
if let Some(vwap) = Trade::vwap(exchange.trades()) {
    println!("VWAP: {vwap}");
}
```

---

## Persistence & Serde

### Persistence

**Feature flag:** `persistence` (includes `serde` and `event-log`); `Portfolio::save_json` and `load_json` also need `portfolio`

```rust
// Exchange — JSON Lines event sourcing
exchange.save(Path::new("orders.jsonl")).unwrap();
let loaded = Exchange::load(Path::new("orders.jsonl")).unwrap();

// Portfolio — JSON
portfolio.save_json(Path::new("portfolio.json")).unwrap();
let loaded = Portfolio::load_json(Path::new("portfolio.json")).unwrap();
```

### Serde

**Feature flag:** `serde`

All public types derive `Serialize`/`Deserialize`: `Price`, `OrderId`, `TradeId`, `Symbol`, `Side`, `TimeInForce`, `OrderStatus`, `Order`, `Trade`, `Event`, `SubmitResult`, `CancelResult`, `ModifyResult`, `BookSnapshot`, `LevelSnapshot`, `StopOrder`, `Position`, `CostModel`, and more.

---

## CLI Reference

Interactive REPL for the order book:

```bash
cargo run --bin lob
```

| Command | Example |
|---------|---------|
| `buy <price> <qty> [ioc\|fok]` | `buy 100.50 100` |
| `sell <price> <qty> [ioc\|fok]` | `sell 101.00 50 ioc` |
| `market <buy\|sell> <qty>` | `market buy 200` |
| `stop <buy\|sell> <price> <qty>` | `stop buy 105.00 100` |
| `stoplimit <buy\|sell> <stop> <limit> <qty> [ioc\|fok]` | `stoplimit sell 95.00 94.50 100` |
| `cancel <order_id>` | `cancel 3` |
| `status <order_id>` | Show order status |
| `book` / `trades` | Show book or trade history |
| `clear` | Reset the exchange |
| `save <path>` / `load <path>` | Persistence (build with `--features persistence`) |

---

## Performance

### Benchmarks

Single-threaded (AMD Ryzen / Intel Core):

| Operation | Latency | Throughput | Complexity |
|-----------|---------|------------|------------|
| Submit (no match) | **~155 ns** | ~6.4M ops/sec | O(log P) |
| Submit (with match) | ~197 ns | ~5M ops/sec | O(log P + M) |
| BBO query | **~1.1 ns** | ~900M ops/sec | O(1) |
| Cancel (tombstone, deep queue) | ~385 ns | ~2.6M ops/sec | **O(1)** |
| L2 snapshot (10 levels) | ~255 ns | ~4M ops/sec | O(D) |

Where P = price levels, M = orders matched, D = depth. Numbers are from
`benches/v0.10-comparison.md` on macOS arm64; expect ±10-20 % across
hardware and build flags.

### Time Breakdown (Submit, No Match)

```
submit_limit() ~155 ns:
├── FxHashMap insert     ~30 ns   order storage
├── BTreeMap insert      ~30 ns   price level (O(log P))
├── VecDeque push         ~5 ns   FIFO queue
├── Event recording      ~10 ns   (optional, for replay)
├── STP branch           ~20 ns   `owner` field + policy read (Off path)
└── Overhead             ~60 ns   struct creation, dispatch, etc.
```

### Optimizations

1. **O(1) cancel** — Tombstone-based, 350x faster than linear scan
2. **FxHash** — Non-cryptographic hash for OrderId lookups (+25% vs std HashMap)
3. **Cached BBO** — Best bid/ask cached for O(1) access
4. **Optional event logging** — disable `event-log` feature for max throughput

### Rust vs Numba

Single-threaded throughput is roughly equivalent (both compile to LLVM IR). Where Rust wins: zero cold-start (vs Numba's ~300 ms JIT), true parallelism via Rayon with no GIL contention, and deterministic memory without GC pauses.


