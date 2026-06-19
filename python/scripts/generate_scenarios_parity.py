#!/usr/bin/env python3
"""Regenerate tests/reference/scenarios_parity.json from nanotrade/calc reference.

Run from repo root:
  cd nanotrade && uv run python ../nanobook/python/scripts/generate_scenarios_parity.py
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "nanobook" / "python"))
sys.path.insert(0, str(ROOT / "nanotrade"))

import nanobook  # noqa: E402
from calc import scenarios as ref  # noqa: E402

CASES = [
    dict(
        name="xyz_advanced_42",
        ticker="XYZ",
        current_price=74.0,
        version="advanced",
        n_paths=200,
        seed=42,
        gp_growth_mean=0.16,
        multiple_mean=22.0,
        macro_shock_mean=-0.03,
    ),
    dict(
        name="simple_gbm_7",
        ticker="S",
        current_price=100.0,
        version="simple",
        n_paths=50,
        seed=7,
        expected_annual_return=0.18,
        annual_vol=0.38,
    ),
    dict(
        name="advanced_small_n",
        ticker="A",
        current_price=50.0,
        version="advanced",
        n_paths=5,
        seed=99,
    ),
]

OUT = Path(__file__).resolve().parents[1] / "tests" / "reference" / "scenarios_parity.json"


def main() -> int:
    out_cases = []
    for raw in CASES:
        name = raw.pop("name")
        params = dict(raw)
        ref_res = ref.monte_carlo_stock_valuation(**params)
        np_res = nanobook.monte_carlo_stock_valuation(**params)
        ref_sum = ref_res.summary.to_dicts()[0]
        sorted_paths = [round(float(x), 6) for x in sorted(ref_res.terminal_prices)]
        np_sorted = [round(float(x), 6) for x in sorted(np_res.terminal_prices)]
        if sorted_paths != np_sorted:
            raise SystemExit(f"path mismatch for {name}")
        if not math.isclose(np_res.median_price, float(ref_sum["median_price"]), rel_tol=1e-9, abs_tol=1e-6):
            raise SystemExit(f"median mismatch for {name}")
        out_cases.append(
            {
                "name": name,
                "params": params,
                "summary": {
                    "median_price": round(float(ref_sum["median_price"]), 2),
                    "mean_price": round(float(ref_sum["mean_price"]), 2),
                    "implied_median_annual_return": round(float(ref_sum["implied_median_annual_return"]), 4),
                    "p10": round(float(ref_sum["p10"]), 2),
                    "p90": round(float(ref_sum["p90"]), 2),
                },
                "terminal_prices_sorted": sorted_paths,
            }
        )
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps({"version": 1, "cases": out_cases}, indent=2) + "\n")
    print(f"wrote {OUT} ({len(out_cases)} cases)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())