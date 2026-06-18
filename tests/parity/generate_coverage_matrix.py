"""Generate TA-Lib full classification coverage matrix.

Maps every talib.get_functions() entry to implementation status using
indicator_registry.json (parity-implemented) and heuristics for deferrals.

Usage:
    uv run --with TA-Lib python tests/parity/generate_coverage_matrix.py
"""

from __future__ import annotations

import json
from pathlib import Path

import talib

PARITY_DIR = Path(__file__).parent
REGISTRY_PATH = PARITY_DIR / "indicator_registry.json"
OUT_PATH = PARITY_DIR.parent.parent / "docs" / "ta-lib-full-coverage-matrix.md"

IMPLEMENTED_FUNCS: set[str] = set()
GROUP_DEFER = {
    "Pattern Recognition": ("deferred", "candlestick patterns — out of scope for v2 price filters"),
    "Math Operators": ("deferred", "element-wise math — not strategy indicators"),
    "Math Transform": ("deferred", "trigonometric transforms — not strategy indicators"),
    "Statistic Functions": ("deferred", "stats primitives — covered elsewhere (nanobook stats)"),
    "Price Transform": ("deferred", "AVGPRICE/MEDPRICE etc — low signal for manifests"),
    "Cycle Indicators": ("deferred", "Hilbert transform family — niche for equity daily bars"),
}
DEFAULT_DEFER = ("deferred", "not in curated 25-35 high-signal set for Strategy Spec v2")


def load_implemented() -> set[str]:
    data = json.loads(REGISTRY_PATH.read_text())
    return {entry["talib_func"].upper() for entry in data["indicators"]}


def status_for(func: str, group: str) -> tuple[str, str]:
    if func in IMPLEMENTED_FUNCS:
        return "implemented", "golden parity via indicator_registry.json"
    if group in GROUP_DEFER:
        return GROUP_DEFER[group]
    return DEFAULT_DEFER


def main() -> None:
    global IMPLEMENTED_FUNCS
    IMPLEMENTED_FUNCS = load_implemented()
    groups = talib.get_function_groups()
    all_funcs = sorted(talib.get_functions())
    total = len(all_funcs)

    lines = [
        "# TA-Lib full coverage matrix",
        "",
        f"Total TA-Lib functions: **{total}**",
        f"Parity-implemented (registry): **{len(IMPLEMENTED_FUNCS)}**",
        "",
        "Status legend:",
        "- **implemented** — Rust + golden parity in nanobook",
        "- **deferred** — classified with explicit rationale (not a backlog gap)",
        "",
        "| Group | Function | Status | Rationale |",
        "|-------|----------|--------|-----------|",
    ]

    counts = {"implemented": 0, "deferred": 0}
    for group, funcs in sorted(groups.items()):
        for func in sorted(funcs):
            status, rationale = status_for(func, group)
            counts[status] = counts.get(status, 0) + 1
            lines.append(f"| {group} | {func} | {status} | {rationale} |")

    lines.extend(
        [
            "",
            "## Summary",
            "",
            f"- Rows: {total} (100% of `talib.get_functions()`)",
            f"- Implemented: {counts.get('implemented', 0)}",
            f"- Deferred: {counts.get('deferred', 0)}",
            "",
            "Regenerate: `uv run --with TA-Lib python tests/parity/generate_coverage_matrix.py`",
        ]
    )

    OUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUT_PATH.write_text("\n".join(lines) + "\n")
    print(f"Wrote {OUT_PATH} ({total} rows, {counts.get('implemented', 0)} implemented)")


if __name__ == "__main__":
    main()