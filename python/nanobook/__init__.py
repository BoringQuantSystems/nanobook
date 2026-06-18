"""nanobook Python package exports.

The Rust extension keeps legacy ``py_*`` names for compatibility.
These aliases provide clean v0.9 names for new callers.
"""

from .nanobook import *  # noqa: F401,F403

FillPolicy.SignalBarClose = FillPolicy.signal_bar_close()
FillPolicy.NextBarOpen = FillPolicy.next_bar_open()
FillPolicy.NextBarTypical = FillPolicy.next_bar_typical()


def capabilities():
    caps = list(py_capabilities())
    caps.extend(
        [
            "monte_carlo_stock_valuation",
            "scenario_terminal_paths",
            "scenario_calibrate",
        ]
    )
    return caps


def backtest_weights(
    weight_schedule,
    price_schedule,
    initial_cash,
    cost_model,
    fill_policy,
    periods_per_year=252.0,
    risk_free=0.0,
    stop_cfg=None,
):
    return py_backtest_weights(
        weight_schedule,
        price_schedule,
        initial_cash,
        cost_model,
        fill_policy,
        periods_per_year,
        risk_free,
        stop_cfg,
    )


def decompose_backtest(weight_schedule, return_schedule):
    return py_decompose_backtest(weight_schedule, return_schedule)


def tear_sheet(backtest_result, rolling_window=63, periods_per_year=252):
    return py_tear_sheet(backtest_result, rolling_window, periods_per_year)


def sma(close, period):
    return py_sma(close, period)


def ema(close, period):
    return py_ema(close, period)


def rsi(close, period=14):
    return py_rsi(close, period)


def macd(close, fast_period=12, slow_period=26, signal_period=9):
    return py_macd(close, fast_period, slow_period, signal_period)


def bollinger(close, period=20, num_std_up=2.0, num_std_dn=2.0):
    return py_bollinger(close, period, num_std_up, num_std_dn)


def wilder_atr(high, low, close, period=14):
    return py_wilder_atr(high, low, close, period)


def list_supported_indicators():
    return py_list_supported_indicators()


def realized_vol(open, high, low, close, method="close_to_close"):
    return py_realized_vol(open, high, low, close, method)


def drawdown_series(equity):
    return py_drawdown_series(equity)


def rolling_max_drawdown(equity, window):
    return py_rolling_max_drawdown(equity, window)


def walkforward(returns, params=None, n_windows=5, train_pct=0.7, periods_per_year=252.0, risk_free=0.0):
    return py_walkforward(returns, params, n_windows, train_pct, periods_per_year, risk_free)


def garch_ewma_forecast(returns, p=1, q=1, mean="zero"):
    return py_garch_ewma_forecast(returns, p, q, mean)


def optimize_min_variance(returns_matrix, symbols):
    return py_optimize_min_variance(returns_matrix, symbols)


def optimize_max_sharpe(returns_matrix, symbols, risk_free=0.0):
    return py_optimize_max_sharpe(returns_matrix, symbols, risk_free)


def optimize_risk_parity(returns_matrix, symbols):
    return py_optimize_risk_parity(returns_matrix, symbols)


def inverse_cvar_weights(returns_matrix, symbols, alpha=0.95):
    return py_inverse_cvar_weights(returns_matrix, symbols, alpha)


def inverse_cdar_weights(returns_matrix, symbols, alpha=0.95):
    return py_inverse_cdar_weights(returns_matrix, symbols, alpha)


def optimize_hrp(returns_matrix, symbols):
    return py_optimize_hrp(returns_matrix, symbols)


# Pure-Python research helpers (minimal deps, optional numpy accel)
try:
    from .scenarios import (
        MonteCarloResult,
        monte_carlo_stock_valuation,
        calibrate_from_fundamentals,
        compute_annualized_vol,
        terminal_prices_to_log_return_paths,
        summarize_distribution,
        ValuationParams,
    )
except Exception:  # pragma: no cover
    MonteCarloResult = None  # type: ignore
    monte_carlo_stock_valuation = None  # type: ignore
    calibrate_from_fundamentals = None  # type: ignore
    compute_annualized_vol = None  # type: ignore
    terminal_prices_to_log_return_paths = None  # type: ignore
    summarize_distribution = None  # type: ignore
    ValuationParams = None  # type: ignore


__all__ = [name for name in globals() if not name.startswith("_")]
