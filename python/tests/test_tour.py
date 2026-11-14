"""Run examples/tour.py as a newcomer would and check its key printed numbers."""

import re
import subprocess
import sys
from pathlib import Path

import pytest

PYTHON_DIR = Path(__file__).resolve().parents[1]
TOUR = PYTHON_DIR / "examples" / "tour.py"
VERIFICATION = PYTHON_DIR.parent / "VERIFICATION.md"


@pytest.fixture(scope="module")
def output():
    proc = subprocess.run(
        [sys.executable, str(TOUR)], capture_output=True, text=True, timeout=60, check=True
    )
    return proc.stdout


def money(text):
    return float(text.replace("$", "").replace(",", ""))


def scene(output, n):
    parts = re.split(r"^=== Scene (\d): .* ===$", output, flags=re.M)
    return dict(zip(parts[1::2], parts[2::2]))[str(n)]


def test_five_scenes_and_runtime(output):
    assert re.findall(r"^=== Scene (\d):", output, flags=re.M) == ["1", "2", "3", "4", "5"]
    seconds = float(re.search(r"tour finished in ([\d.]+) s", output).group(1))
    assert seconds < 10.0


def test_scene1_backtest_numbers(output):
    s = scene(output, 1)
    net, gross = re.search(r"with costs:(.*?)zero cost:(.*)", s, flags=re.S).groups()
    net_eq = money(re.search(r"final equity\s+(\$[\d,.]+)", net).group(1))
    gross_eq = money(re.search(r"final equity\s+(\$[\d,.]+)", gross).group(1))
    assert net_eq == pytest.approx(1_127_584.21, rel=1e-4)
    assert gross_eq == pytest.approx(1_131_767.05, rel=1e-4)
    assert gross_eq > net_eq  # costs reduce final equity
    assert float(re.search(r"Sharpe\s+([\d.-]+)", net).group(1)) == pytest.approx(1.22, abs=0.02)
    assert float(re.search(r"CAGR\s+([\d.-]+) %", net).group(1)) == pytest.approx(12.76, abs=0.1)
    assert float(re.search(r"max drawdown\s+([\d.]+) %", net).group(1)) == pytest.approx(9.88, abs=0.1)
    assert int(re.search(r"executions\s+(\d+)", net).group(1)) == 31
    assert float(re.search(r"backtest time\s+([\d.]+) ms", net).group(1)) > 0.0
    cost = money(re.search(r"took (\$[\d,.]+) of final equity", s).group(1))
    assert cost == pytest.approx(gross_eq - net_eq, abs=0.02)


def test_scene2_order_book(output):
    s = scene(output, 2)
    assert "trade 50 @ $101.00 (aggressor: buy)" in s
    assert "trade 30 @ $102.00 (aggressor: buy)" in s
    assert "result Cancelled, filled 0, trades 0" in s
    assert "book unchanged: True; stored record: None" in s
    before = re.search(r"before: (.*)", s).group(1)
    after = re.search(r"after:  (.*)", s).group(1)
    assert before == after == "best bid $99.00 / best ask $102.00"
    assert "result PartiallyFilled: filled 20, cancelled 80" in s
    assert "status 'cancelled', filled 20, remaining 0" in s


def test_scene3_risk_checks(output):
    s = scene(output, 3)
    batch_a, rest = s.split("batch B:")
    batch_b, order_c = rest.split("order C:")
    assert "FAIL" not in batch_a
    assert re.search(r"FAIL\s+Leverage\s+1\.85x > 1\.50x limit", batch_b)
    assert re.search(r"PASS\s+Short exposure", batch_b)
    assert re.search(r"FAIL\s+Short selling\s+short selling not allowed", order_c)


def test_scene4_first_rebalance_orders(output):
    s = scene(output, 4)
    first = s.split("period 21")[0]
    lines = re.findall(r"^\s+(buy|sell)\s+([\d.]+) (\w+) @ (\$[\d,.]+)\s+notional (\$[\d,.]+)", first, flags=re.M)
    assert [l[2] for l in lines] == ["AAA", "BBB", "CCC"]
    assert all(l[0] == "buy" for l in lines)  # starting from cash
    total = sum(money(l[4]) for l in lines)
    assert total == pytest.approx(1_000_000.0, rel=0.01)  # three roughly equal thirds of the cash
    assert "period 21" in s


def test_scene5_laws_match_verification_md(output):
    s = scene(output, 5)
    assert len(re.findall(r"^\s+law\s+\d+:", s, flags=re.M)) == 11
    assert "verification/lean/check.sh 0" in s
    assert "mutation score: see VERIFICATION.md" in s
    if not VERIFICATION.exists():
        pytest.skip("VERIFICATION.md not next to python/ (installed layout)")
    table = re.findall(r"^\| (\d+) \| (.*?) \| `", VERIFICATION.read_text(), flags=re.M)
    assert len(table) == 11
    flat = " ".join(s.split())
    for _, statement in table:
        assert " ".join(statement.split()) in flat
