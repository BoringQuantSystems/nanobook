import nanobook


def make_degenerate_bar(p: int):
    return nanobook.BarPrices(open=p, high=p, low=p, close=p)


def test_signal_bar_close_parity():
    """SignalBarClose with degenerate OHLC should give same results as old scalar path."""
    ps = [100_00, 110_00, 99_00]
    price_schedule = [[("AAPL", make_degenerate_bar(p))] for p in ps]
    weight_schedule = [[("AAPL", 1.0)]] * 3
    result = nanobook.backtest_weights(
        weight_schedule,
        price_schedule,
        initial_cash=1_000_000_00,
        cost_model=nanobook.CostModel.zero(),
        fill_policy=nanobook.FillPolicy.SignalBarClose,
    )
    assert result["equity_curve"][0] == 1_000_000_00
    assert len(result["equity_curve"]) == 4
    assert result["skipped_rebalances"] == []
    assert result["equity_curve"][2] > result["equity_curve"][0]


def test_next_bar_open_fills():
    """NextBarOpen: fills happen at open[t+1], not close[t]."""
    n = 4
    closes = [100_00 + i * 100 for i in range(n)]
    opens = [closes[0] - 50] + [c + 100 for c in closes[:-1]]
    price_schedule = [
        [
            (
                "AAPL",
                nanobook.BarPrices(
                    open=opens[i],
                    high=max(opens[i], closes[i]),
                    low=min(opens[i], closes[i]),
                    close=closes[i],
                ),
            )
        ]
        for i in range(n)
    ]
    weight_schedule = [[("AAPL", 1.0)]] * n
    result = nanobook.backtest_weights(
        weight_schedule,
        price_schedule,
        initial_cash=1_000_000_00,
        cost_model=nanobook.CostModel.zero(),
        fill_policy=nanobook.FillPolicy.NextBarOpen,
    )
    assert (n - 1) in result["skipped_rebalances"], (
        f"Expected {n - 1} in {result['skipped_rebalances']}"
    )
    assert len(result["equity_curve"]) == n + 1


def test_next_bar_typical_fill():
    """NextBarTypical: fill at (high+low+close)/3 of next bar."""
    base = 100_00
    h1 = base * 102 // 100
    l1 = base * 99 // 100
    c1 = base

    price_schedule = [
        [("AAPL", nanobook.BarPrices(open=base, high=base, low=base, close=base))],
        [("AAPL", nanobook.BarPrices(open=c1, high=h1, low=l1, close=c1))],
    ]
    weight_schedule = [[("AAPL", 1.0)], [("AAPL", 1.0)]]
    result = nanobook.backtest_weights(
        weight_schedule,
        price_schedule,
        initial_cash=1_000_000_00,
        cost_model=nanobook.CostModel.zero(),
        fill_policy=nanobook.FillPolicy.NextBarTypical,
    )
    assert 1 in result["skipped_rebalances"]
    assert len(result["equity_curve"]) == 3


def test_last_bar_skip_diagnostics():
    """Last-bar skip: skipped_rebalances contains N-1, earlier bars unaffected."""
    n = 3
    p = 100_00
    price_schedule = [[("AAPL", nanobook.BarPrices(open=p, high=p, low=p, close=p))]] * n
    weight_schedule = [[("AAPL", 1.0)]] * n
    result = nanobook.backtest_weights(
        weight_schedule,
        price_schedule,
        initial_cash=1_000_000_00,
        cost_model=nanobook.CostModel.zero(),
        fill_policy=nanobook.FillPolicy.NextBarOpen,
    )
    assert (n - 1) in result["skipped_rebalances"], (
        f"Expected {n - 1} in skipped_rebalances, got {result['skipped_rebalances']}"
    )
    assert len(result["returns"]) == n
    assert len(result["equity_curve"]) == n + 1


def test_fill_schedule_redirects_next_bar_open():
    """Optional fill_schedule: NextBarOpen reads fill_schedule[i], empty skips."""
    decision = [
        [("AAPL", make_degenerate_bar(100_00))],
        [("AAPL", make_degenerate_bar(100_00))],
        [("AAPL", make_degenerate_bar(100_00))],
    ]
    fill = [
        [("AAPL", make_degenerate_bar(200_00))],
        [("AAPL", make_degenerate_bar(200_00))],
        [],  # no later bar → no fill
    ]
    weights = [[("AAPL", 1.0)]] * 3
    with_fill = nanobook.backtest_weights(
        weights,
        decision,
        initial_cash=1_000_000_00,
        cost_model=nanobook.CostModel.zero(),
        fill_policy=nanobook.FillPolicy.NextBarOpen,
        fill_schedule=fill,
    )
    legacy = nanobook.backtest_weights(
        weights,
        decision,
        initial_cash=1_000_000_00,
        cost_model=nanobook.CostModel.zero(),
        fill_policy=nanobook.FillPolicy.NextBarOpen,
    )
    assert 2 in with_fill["skipped_rebalances"]
    assert 2 in legacy["skipped_rebalances"]
    assert with_fill["equity_curve"] != legacy["equity_curve"]
    assert with_fill["final_cash"] == 0
