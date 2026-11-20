"""A five-scene tour of nanobook: what it does, and why its matching rules can be trusted.

Offline, no API keys, plain printed output, under 10 s.

Run from the python/ directory:

    uv run python examples/tour.py            # print the five scenes
    uv run --with matplotlib python examples/tour.py --figure
                                              # also save assets/tour-equity.png

Scenes:
  1. Target weights to a backtest, with and without trading costs.
  2. The order book: crossing orders, a rejected fill-or-kill, an IOC that reads Cancelled.
  3. Pre-trade risk checks on an order batch and on a short sale.
  4. The orders a rebalance makes, read from the backtest's execution records.
  5. The eleven matching laws proved in Lean, and the command that rechecks them.
"""

import time

_T0 = time.perf_counter()  # measured from before the nanobook import

import math
import random
import sys
import textwrap
from pathlib import Path

import nanobook as nb

SYMBOLS = ("AAA", "BBB", "CCC")
START_PRICES_USD = (100.0, 50.0, 200.0)
N_DAYS = 252
REBALANCE_EVERY = 21  # trading days, about one month
INITIAL_CASH_CENTS = 1_000_000_00
SEED = 20261114
COSTS = dict(commission_bps=10.0, slippage_bps=5.0)

# Plain-language statements, copied from VERIFICATION.md (python/tests/test_tour.py checks they still match).
LAWS = [
    "Bids descend, asks ascend, no level is empty, each queued id is one active order at that price, and each active order is queued exactly once.",
    "The best bid is strictly below the best ask.",
    "Trades of a submit sum to its filled quantity, which does not exceed the submit; each trade takes exactly its size off the resting order.",
    "A submit trades resting orders in price-time order. An order skipped before the last trade was cancelled by self-trade prevention.",
    "Every trade prints at the resting level's price, at or inside the incoming limit.",
    "The matching loop's step bound is always enough: more steps never change the result.",
    "A fill-or-kill submit trades its whole quantity, or trades nothing and leaves every order and level unchanged.",
    "Each self-trade policy does exactly what `src/matching.rs` documents. With any policy other than `Off`, no trade has the same owner on both sides.",
    "A cancel changes only the cancelled order and drops only it from its level. Cancelling a missing or finished order changes nothing.",
    "The tombstone queue always holds the same live orders, in the same order, as the plain lists. Every stored position points at its own order, and the totals and tombstone counts are exact.",
    "A submitted order's stored record matches its status: filled means fully filled; resting means filled plus remaining equals the submit; cancelled means nothing remains; no record means a rejected fill-or-kill with no trades.",
]


def usd(cents):
    return f"${cents / 100:,.2f}"


def heading(n, title, what):
    print()
    print(f"=== Scene {n}: {title} ===")
    print(what)
    print()


# --------------------------------------------------------------------------- scene 1


def make_bars(seed=SEED):
    """Daily bars (cents) for three symbols: geometric random walks from a fixed seed."""
    rng = random.Random(seed)

    def normal():  # Box-Muller from random(), whose stream is fixed across Python versions
        return math.sqrt(-2.0 * math.log(1.0 - rng.random())) * math.cos(2.0 * math.pi * rng.random())

    closes = [[p * 100.0] for p in START_PRICES_USD]
    bars = []  # bars[t][i] = BarPrices
    for _ in range(N_DAYS):
        day = []
        for i in range(len(SYMBOLS)):
            prev = closes[i][-1]
            op = prev * (1.0 + 0.002 * normal())
            cl = op * math.exp(0.0004 + 0.012 * normal())
            hi = max(op, cl) * (1.0 + 0.003 * abs(normal()))
            lo = min(op, cl) * (1.0 - 0.003 * abs(normal()))
            closes[i].append(cl)
            day.append(nb.BarPrices(round(op), round(hi), round(lo), round(cl)))
        bars.append(day)
    return bars


def inverse_vol_weights(bars, t):
    """Weights proportional to 1 / volatility of the last 20 daily closes, summing to 1."""
    inv = []
    for i in range(len(SYMBOLS)):
        c = [bars[k][i].close for k in range(t - 20, t + 1)]
        r = [c[k + 1] / c[k] - 1.0 for k in range(20)]
        m = sum(r) / len(r)
        sd = math.sqrt(sum((x - m) ** 2 for x in r) / (len(r) - 1))
        inv.append(1.0 / sd)
    s = sum(inv)
    return [w / s for w in inv]


def build_schedules(bars):
    """Return (weights, prices, fills) per period. Rebalance every 21 days, filled at the next bar's open.

    On days without a rebalance the fill entry is empty, which the engine reads as "no fill".
    """
    weights, prices, fills = [], [], []
    current = [1.0 / len(SYMBOLS)] * len(SYMBOLS)
    for t in range(N_DAYS):
        rebalance = t % REBALANCE_EVERY == 0
        if rebalance and t > 0:
            current = inverse_vol_weights(bars, t)
        weights.append(list(zip(SYMBOLS, current)))
        prices.append(list(zip(SYMBOLS, bars[t])))
        fills.append(list(zip(SYMBOLS, bars[t + 1])) if rebalance and t + 1 < N_DAYS else [])
    return weights, prices, fills


def run_backtest(schedules, cost_model):
    weights, prices, fills = schedules
    t0 = time.perf_counter()
    result = nb.backtest_weights(
        weights,
        prices,
        INITIAL_CASH_CENTS,
        cost_model,
        nb.FillPolicy.NextBarOpen,
        252.0,
        0.0,
        fill_schedule=fills,
        record_executions=True,
    )
    ms = (time.perf_counter() - t0) * 1e3
    return result, ms


def scene1():
    heading(
        1,
        "From weights to a backtest",
        f"{len(SYMBOLS)} synthetic stocks, {N_DAYS} daily bars (seed {SEED}), inverse-volatility weights,\n"
        f"rebalanced every {REBALANCE_EVERY} days at the next bar's open, "
        f"{COSTS['commission_bps']:.0f} bps commission + {COSTS['slippage_bps']:.0f} bps slippage.",
    )
    schedules = build_schedules(make_bars())
    net, net_ms = run_backtest(schedules, nb.CostModel(**COSTS))
    gross, gross_ms = run_backtest(schedules, nb.CostModel.zero())

    out = {"net": net, "gross": gross, "net_ms": net_ms, "gross_ms": gross_ms}
    for label, res, ms in (("with costs", net, net_ms), ("zero cost", gross, gross_ms)):
        m = res["metrics"]
        print(f"{label}:")
        print(f"  final equity     {usd(res['equity_curve'][-1])}  (start {usd(INITIAL_CASH_CENTS)})")
        print(f"  CAGR             {m.cagr * 100:.2f} %")
        print(f"  Sharpe           {m.sharpe:.2f}")
        print(f"  max drawdown     {m.max_drawdown * 100:.2f} %")
        print(f"  executions       {len(res['executions'])}")
        print(f"  backtest time    {ms:.2f} ms")
    cost = gross["equity_curve"][-1] - net["equity_curve"][-1]
    print(f"cost effect: trading costs took {usd(cost)} of final equity.")
    out["cost_cents"] = cost
    return out


# --------------------------------------------------------------------------- scene 2


def book_state(ex):
    depth = ex.depth(10)
    return (
        ex.best_bid_ask(),
        [(l.price, l.quantity) for l in depth.bids],
        [(l.price, l.quantity) for l in depth.asks],
    )


def fmt_bba(bba):
    bid, ask = bba
    f = lambda p: "none" if p is None else usd(p)
    return f"best bid {f(bid)} / best ask {f(ask)}"


def scene2():
    heading(
        2,
        "The order book does the right thing",
        "Rest orders, cross them, then two cases the Lean proofs cover (laws 7 and 11).",
    )
    ex = nb.Exchange()
    ex.submit_limit("sell", 10100, 50)
    ex.submit_limit("sell", 10200, 50)
    ex.submit_limit("buy", 9900, 100)
    print("resting: sell 50 @ $101.00, sell 50 @ $102.00, buy 100 @ $99.00")
    print(f"  {fmt_bba(ex.best_bid_ask())}")

    cross = ex.submit_limit("buy", 10200, 80)
    print("buy 80 @ $102.00 crosses the asks:")
    for tr in cross.trades:
        print(f"  trade {tr.quantity} @ {usd(tr.price)} (aggressor: {tr.aggressor_side})")
    print(f"  order status {cross.status}; {fmt_bba(ex.best_bid_ask())}; ask left at $102.00: "
          f"{ex.depth(1).asks[0].quantity}")

    before = book_state(ex)
    fok = ex.submit_limit("buy", 10200, 100, "fok")
    after = book_state(ex)
    print("fill-or-kill buy 100 @ $102.00, only 20 are available:")
    print(f"  result {fok.status}, filled {fok.filled_quantity}, trades {len(fok.trades)}")
    print(f"  before: {fmt_bba(before[0])}")
    print(f"  after:  {fmt_bba(after[0])}")
    print(f"  book unchanged: {before == after}; stored record: {ex.get_order(fok.order_id)}")

    ioc = ex.submit_limit("buy", 10200, 100, "ioc")
    order = ex.get_order(ioc.order_id)
    print("immediate-or-cancel buy 100 @ $102.00, 20 fill and the rest cannot rest:")
    print(f"  result {ioc.status}: filled {ioc.filled_quantity}, cancelled {ioc.cancelled_quantity}")
    print(f"  get_order -> status {order.status!r}, filled {order.filled_quantity}, remaining {order.remaining_quantity}")
    return {
        "cross_trades": [(tr.price, tr.quantity) for tr in cross.trades],
        "fok_status": fok.status,
        "fok_filled": fok.filled_quantity,
        "book_unchanged": before == after,
        "fok_record": ex.get_order(fok.order_id),
        "ioc_order_status": order.status,
        "ioc_filled": order.filled_quantity,
        "ioc_remaining": order.remaining_quantity,
    }


# --------------------------------------------------------------------------- scene 3


def print_report(title, report):
    print(title)
    for c in report:
        print(f"  {c['status']:<4}  {c['name']:<18} {c['detail']}")


def scene3():
    heading(
        3,
        "Risk checks stop bad orders",
        "RiskEngine(allow_short=False, max_leverage=1.5, max_position_pct=1.0), equity $100,000.00.",
    )
    equity = 10_000_000
    risk = nb.RiskEngine(max_leverage=1.5, max_position_pct=1.0, allow_short=False)

    ok = risk.check_batch(
        [("AAA", "buy", 300, 10_000), ("BBB", "buy", 400, 10_000)],
        equity, [], [("AAA", 0.30), ("BBB", 0.40)],
    )
    print_report("batch A: buy $30,000 AAA + $40,000 BBB (0.70x):", ok)

    bad = risk.check_batch(
        [("AAA", "buy", 900, 10_000), ("BBB", "buy", 950, 10_000)],
        equity, [], [("AAA", 0.90), ("BBB", 0.95)],
    )
    print_report("batch B: buy $90,000 AAA + $95,000 BBB (1.85x):", bad)

    short = risk.check_order("CCC", "sell", 10, 5000, equity, [])
    print_report("order C: sell 10 CCC @ $50.00 while holding none (a short):", short)

    return {"ok": ok, "bad": bad, "short": short}


# --------------------------------------------------------------------------- scene 4


def scene4(net):
    heading(
        4,
        "What a rebalance does, without a broker",
        "The orders the portfolio rebalance placed, read from the backtest's execution records\n"
        "(record_executions=True). No network, no broker object.",
    )
    ex = net["executions"]
    periods = sorted({e["period_index"] for e in ex})[:2]
    for p in periods:
        label = "first rebalance" if p == periods[0] else "second rebalance (the change from drifted holdings)"
        print(f"period {p}, {label}:")
        for e in ex:
            if e["period_index"] == p:
                print(f"  {e['side']:<4} {abs(e['quantity']) / 1e6:>9.4f} {e['symbol']} @ {usd(e['price'])}"
                      f"  notional {usd(e['notional'])}  cost {usd(e['cost'])}")
    return {"periods": periods, "first": [e for e in ex if e["period_index"] == periods[0]]}


# --------------------------------------------------------------------------- scene 5


def scene5():
    heading(
        5,
        "Proof, not promises",
        "Eleven laws of the matching engine are proved in Lean 4 on an executable model;\n"
        "CI replays the same event logs through the Rust engine and the model and compares trades.",
    )
    for n, text in enumerate(LAWS, 1):
        print(textwrap.fill(text, width=100, initial_indent=f"  law {n:>2}: ", subsequent_indent=" " * 11))
    print()
    print("recheck the proofs yourself, from the repository root:")
    print("  verification/lean/check.sh 0")
    print("mutation-testing results, crate by crate: VERIFICATION.md")


# --------------------------------------------------------------------------- figure


def save_figure(s1):
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    import matplotlib.ticker

    path = Path(__file__).resolve().parents[2] / "assets" / "tour-equity.png"
    gross = [c / 100 for c in s1["gross"]["equity_curve"]]
    net = [c / 100 for c in s1["net"]["equity_curve"]]
    days = list(range(len(net)))
    fig, ax = plt.subplots(figsize=(7, 4.2))
    ax.plot(days, gross, marker="o", markersize=4, markevery=12, linewidth=4.0, color="tab:blue",
            label=f"gross of costs ({s1['gross_ms']:.2f} ms)")
    ax.plot(days, net, marker="s", markersize=4, markevery=(6, 12), linewidth=1.6, color="tab:orange",
            label=f"net of costs ({s1['net_ms']:.2f} ms)")
    ax.set_xlabel("trading day")
    ax.yaxis.set_major_formatter(matplotlib.ticker.FuncFormatter(lambda v, _: f"{v:,.0f}"))
    ax.set_ylabel("equity (USD)")
    ax.set_title("(a) equity, net vs gross of costs (SYNTHETIC prices, seeded)", loc="left", fontsize=10)
    ax.legend(frameon=True)
    ax.grid(alpha=0.3)
    fig.tight_layout()
    path.parent.mkdir(exist_ok=True)
    fig.savefig(path, dpi=400)
    plt.close(fig)
    print(f"\nfigure saved: {path}")


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    s1 = scene1()
    scene2()
    scene3()
    scene4(s1["net"])
    scene5()
    if "--figure" in argv:
        save_figure(s1)
    print(f"\ntour finished in {time.perf_counter() - _T0:.2f} s (includes import)")


if __name__ == "__main__":
    main()
