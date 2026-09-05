# nanobook Python bindings

Rust extension plus pure-Python research helpers. Build with `uv sync --group dev` from this directory.

## Monte Carlo scenarios (Rust-backed, parity-safe)

`nanobook.scenarios` provides terminal price distributions for stress testing and
forecasting. When the extension is built with the `scenarios` feature (default in
wheels), `int`/`None` seeds delegate to Rust: NumPy PCG64 draws feed Rust math so
results match the `nanotrade/calc` reference at tight tolerance. A pure-Python
fallback remains for `random.Random` seeds and environments without numpy.

```python
import nanobook

res = nanobook.monte_carlo_stock_valuation(
    "XYZ",
    74.0,
    version="advanced",
    n_paths=200,
    seed=42,
    gp_growth_mean=0.16,
    multiple_mean=22.0,
    macro_shock_mean=-0.03,
)
print(res)  # MonteCarloResult(..., median_price=86.36)
print(res.median_price, res.implied_median_annual_return)

paths = res.to_price_paths(4, method="linear")
# See examples/scenario_backtest.py for feeding paths into backtest_weights.
```

Regenerate frozen parity fixtures:

```bash
cd ../../nanotrade && uv run python ../nanobook/python/scripts/generate_scenarios_parity.py
```

Run scenario tests:

```bash
uv run pytest tests/test_scenarios*.py tests/property/test_prop_scenarios.py tests/reference/test_ref_scenarios.py -q
```