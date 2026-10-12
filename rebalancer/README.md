# nanobook-rebalancer

Command-line rebalancer that moves an Interactive Brokers (IBKR) account to a
set of target portfolio weights. It reads the targets from a JSON file, gets
live positions and prices from IB Gateway or TWS, computes the orders, runs the
[nanobook-risk](https://crates.io/crates/nanobook-risk) pre-trade checks, and
sends limit orders. Each run is recorded in a JSONL audit log.

## Install

```bash
cargo install nanobook-rebalancer
```

This installs the `rebalancer` command.

## Steps of a run

1. Read the target weights from `target.json`.
2. Connect to IB Gateway or TWS for positions, prices and account data.
3. Compute the share quantities and limit prices that move the account to the targets.
4. Run the pre-trade risk checks: position limits, leverage and short exposure.
5. Show the plan and ask for confirmation (`--force` skips the prompt).
6. Send limit orders with a minimum interval between them, and cancel orders that
   do not fill before the timeout.
7. Compare the result with the targets and write it to the audit log.

## Commands

```bash
rebalancer status                       # Check the IBKR connection
rebalancer positions                    # Show current positions
rebalancer run target.json --dry-run    # Show the plan only
rebalancer run target.json              # Plan, confirm, execute
rebalancer run target.json --force      # No confirmation prompt (automation)
rebalancer reconcile target.json        # Compare actual and target positions
rebalancer recover target.json          # Recover after a crash from the audit log
rebalancer kill                         # Stop a running rebalancer and check for open orders
```

All commands read `config.toml` from the current directory; `--config <path>`
selects another file. `config.toml.example` in this crate lists every setting:
connection, account, order pacing, risk limits, costs and logging. Port 4002 is
the IB Gateway paper-trading account.

## target.json

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

A positive weight is a long position and a negative weight is a short position.
The rebalancer closes positions in the account whose symbol is not in the
targets.

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
