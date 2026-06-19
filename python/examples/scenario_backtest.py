"""Example: Monte Carlo scenarios → price paths → deterministic backtest.

Uses the Rust-backed scenarios engine when the extension is built with the
``scenarios`` feature (default); falls back to pure-Python otherwise.

Run with: python examples/scenario_backtest.py
"""

import nanobook
from nanobook.scenarios import _HAS_RUST_SCENARIOS

def main():
    # 1. Run scenario forecast (Rust when available, else pure-Python)
    res = nanobook.monte_carlo_stock_valuation(
        "XYZ",
        current_price=74.0,
        version="advanced",
        n_paths=3,  # tiny for demo
        seed=42,
        gp_growth_mean=0.16,
        multiple_mean=22.0,
        macro_shock_mean=-0.03,
    )
    print("MC result:", res.method, "median_price=", res.median_price)

    # 2. Turn one terminal into a 4-period price schedule (cents)
    paths = res.to_price_paths(4, method="linear")
    one_path = paths[0]
    price_schedule = []
    for p in one_path:
        cents = int(round(p * 100))
        price_schedule.append([("XYZ", nanobook.BarPrices(cents, cents, cents, cents))])

    # 3. Simple weight schedule (hold 100%)
    weight_schedule = [[("XYZ", 1.0)]] * len(price_schedule)

    # 4. Run through nanobook deterministic execution
    result = nanobook.backtest_weights(
        weight_schedule=weight_schedule,
        price_schedule=price_schedule,
        initial_cash=1_000_000_00,
        cost_model=nanobook.CostModel.zero(),
        fill_policy=nanobook.FillPolicy.NextBarOpen,
    )
    print("nanobook backtest on scenario path final equity cents:", result["equity_curve"][-1])
    backend = "Rust scenarios" if _HAS_RUST_SCENARIOS else "pure-Python scenarios"
    print(f"Example complete. {backend} + Rust backtest execution.")


if __name__ == "__main__":
    main()
