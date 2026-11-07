"""Actual fills and cash reconciliation through the compiled binding."""

import struct

import nanobook
import pytest


SCALE = 1_000_000
METRICS = (
    "total_return", "cagr", "volatility", "sharpe", "sortino", "max_drawdown",
    "calmar", "num_periods", "winning_periods", "losing_periods", "cvar_95",
    "win_rate", "profit_factor", "payoff_ratio", "kelly",
)


def bar(price):
    return nanobook.BarPrices(open=price, high=price, low=price, close=price)


def bits(value):
    if isinstance(value, float):
        return struct.pack("!d", value)
    if isinstance(value, dict):
        return {key: bits(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [bits(item) for item in value]
    return value


def assert_legacy_equal(off, on):
    assert off.keys() == on.keys()
    for key in off.keys() - {"executions", "cash_snapshots", "metrics"}:
        assert bits(off[key]) == bits(on[key]), key
    if off["metrics"] is None:
        assert on["metrics"] is None
    else:
        for name in METRICS:
            assert bits(getattr(off["metrics"], name)) == bits(getattr(on["metrics"], name))
    assert off["executions"] is None
    assert off["cash_snapshots"] is None


def trunc_div(value, denominator):
    return (1 if value >= 0 else -1) * (abs(value) // denominator)


def assert_reconciles(result, prices, cost):
    records = result["executions"]
    cash = result["initial_cash"]
    assert len(result["cash_snapshots"]) == len(result["returns"]) == len(result["holdings"])
    for sequence, record in enumerate(records):
        assert record.keys() == {
            "sequence", "period_index", "kind", "symbol", "quantity", "side",
            "price", "notional", "cost", "cash_delta", "cash_after",
        }
        assert record["sequence"] == sequence
        assert 0 <= record["period_index"] < len(prices)
        assert isinstance(record["quantity"], int) and record["quantity"] != 0
        assert record["side"] == ("buy" if record["quantity"] > 0 else "sell")
        signed = trunc_div(record["quantity"] * record["price"], SCALE)
        assert record["notional"] == abs(signed)
        assert record["cost"] == cost.compute_cost(record["notional"])
        assert record["cash_delta"] == -signed - record["cost"]
        cash += record["cash_delta"]
        assert record["cash_after"] == cash
    assert cash == result["final_cash"]
    quantities = {}
    cursor = 0
    cash = result["initial_cash"]
    for period, snapshot in enumerate(result["cash_snapshots"]):
        while cursor < len(records) and records[cursor]["period_index"] <= period:
            record = records[cursor]
            cash = record["cash_after"]
            symbol = record["symbol"]
            quantities[symbol] = quantities.get(symbol, 0) + record["quantity"]
            cursor += 1
        assert snapshot == cash
        values = {
            symbol: trunc_div(quantities.get(symbol, 0) * price.close, SCALE)
            for symbol, price in prices[period]
        }
        equity = cash + sum(values.values())
        assert result["equity_curve"][period + 1] == equity
        for symbol, weight in result["holdings"][period]:
            assert weight == pytest.approx(values[symbol] / equity, rel=1e-12, abs=1e-15)
    assert cursor == len(records)


@pytest.mark.parametrize("step", [SCALE, 1_000])
@pytest.mark.parametrize("policy_name", ["SignalBarClose", "NextBarOpen", "NextBarTypical"])
@pytest.mark.parametrize("with_cost", [False, True])
def test_parity_and_reconciliation(step, policy_name, with_cost):
    weights = [[("AAA", 1.0)], [("AAA", 0.4), ("BBB", 0.5)], [],
               [("BBB", 1.0)], [("AAA", 0.6), ("BBB", 0.3)]]
    prices = [[("AAA", bar(p)), ("BBB", bar(2 * p))] for p in [333, 350, 310, 320, 340]]
    cost = nanobook.CostModel(15.0, 50.0, 75) if with_cost else nanobook.CostModel.zero()
    args = (weights, prices, 10_000, cost, getattr(nanobook.FillPolicy, policy_name))
    off = nanobook.backtest_weights(*args, quantity_step=step)
    on = nanobook.backtest_weights(*args, quantity_step=step, record_executions=True)
    assert_legacy_equal(off, on)
    assert_reconciles(on, prices, cost)
    assert {r["side"] for r in on["executions"]} == {"buy", "sell"}
    assert all(r["quantity"] % step == 0 for r in on["executions"])
    assert all(r["kind"] == "Rebalance" for r in on["executions"])
    assert on["cash_snapshots"][2] == on["equity_curve"][3]
    if policy_name != "SignalBarClose":
        assert on["skipped_rebalances"] == [4]
        assert all(r["period_index"] < 4 for r in on["executions"])


def test_trimmed_buy_records_filled_quantity_and_legacy_positional_calls():
    cost = nanobook.CostModel(10.0, 100.0, 75)
    prices = [[("AAA", bar(100))]]
    args = ([[("AAA", 1.0)]], prices, 1_000, cost,
            nanobook.FillPolicy.SignalBarClose, 252.0, 0.0, None)
    off = nanobook.backtest_weights(*args, None)
    legacy = nanobook.py_backtest_weights(*args, *([None] * 11))
    on = nanobook.backtest_weights(*args, record_executions=True)
    native_on = nanobook.py_backtest_weights(*args, record_executions=True)
    assert_legacy_equal(off, on)
    assert_legacy_equal(legacy, native_on)
    assert on["executions"] == native_on["executions"]
    assert_reconciles(on, prices, cost)
    assert on["initial_cash"] == 1_000
    assert len(on["executions"]) == 1
    record = on["executions"][0]
    assert record["quantity"] == 9 * SCALE
    assert record["price"] == 101
    assert record["notional"] == 909
    assert record["cost"] == 75
    assert record["cash_delta"] == -984
    assert record["cash_after"] == 16


@pytest.mark.parametrize("stop_key", ["fixed_stop_pct", "trailing_stop_pct"])
def test_stop_exit_and_reentry(stop_key):
    prices = [[("AAA", bar(p))] for p in [100, 120, 80, 100]]
    args = ([[("AAA", 1.0)]] * 4, prices, 10_000,
            nanobook.CostModel.zero(), nanobook.FillPolicy.SignalBarClose)
    off = nanobook.backtest_weights(*args, stop_cfg={stop_key: 0.1})
    on = nanobook.backtest_weights(*args, stop_cfg={stop_key: 0.1}, record_executions=True)
    assert_legacy_equal(off, on)
    assert_reconciles(on, prices, args[3])
    stops = [r for r in on["executions"] if r["kind"] == "StopExit"]
    assert len(stops) == len(on["stop_events"]) == 1
    assert stops[0]["period_index"] == on["stop_events"][0]["period_index"] == 2
    assert stops[0]["symbol"] == on["stop_events"][0]["symbol"]
    assert stops[0]["price"] == on["stop_events"][0]["exit_price"]
    assert any(r["period_index"] == 3 and r["kind"] == "Rebalance" for r in on["executions"])


@pytest.mark.parametrize("weights", [[], [[("AAA", 0.0)]], [[("AAA", 1.0)]], [[("AAA", -0.5)]]])
def test_empty_and_rejected_fills(weights):
    prices = [[("AAA", bar(10))]] * len(weights)
    cost = nanobook.CostModel(min_commission=200)
    args = (weights, prices, 100, cost, nanobook.FillPolicy.SignalBarClose)
    off = nanobook.backtest_weights(*args)
    on = nanobook.backtest_weights(*args, record_executions=True)
    assert_legacy_equal(off, on)
    assert on["initial_cash"] == 100
    assert on["executions"] == []
    assert on["cash_snapshots"] == [100] * len(weights)
    assert_reconciles(on, prices, cost)
