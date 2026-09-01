"""Coverage gap tests for nanobook/__init__.py and scenarios.py.

These tests target missed lines from edge cases, error paths, optional
dependencies, and API entry points not exercised by the existing suite.
"""

from __future__ import annotations

import math
import random

import pytest

import nanobook
from nanobook.scenarios import (
    MonteCarloResult,
    ValuationParams,
    _get_rng,
    _make_pure_rng,
    _normal,
    _normal_box_muller,
    _pure_mean,
    _pure_percentile,
    _pure_prob_above,
    _pure_python_mc,
    _validate_mc_inputs,
    advanced_multi_driver_terminal,
    compute_annualized_vol,
    simple_gbm_terminal,
    summarize_distribution,
    terminal_prices_to_log_return_paths,
)

np = pytest.importorskip("numpy")


class TestInitWrappers:
    """Test the public wrapper functions in nanobook/__init__.py.

    These are simple delegators to Rust py_* functions, tested here
    to ensure the wrapper forwarding works end-to-end.
    """

    def test_sma_wrapper(self):
        """Test simple moving average wrapper."""
        closes = [100.0, 101.0, 102.0, 103.0, 104.0]
        result = nanobook.sma(closes, 3)
        assert result is not None
        assert len(result) > 0

    def test_ema_wrapper(self):
        """Test exponential moving average wrapper."""
        closes = [100.0, 101.0, 102.0, 103.0, 104.0]
        result = nanobook.ema(closes, 3)
        assert result is not None
        assert len(result) > 0

    def test_rsi_wrapper(self):
        """Test RSI wrapper."""
        closes = [100.0, 101.0, 102.0, 101.0, 103.0, 104.0, 103.0, 105.0]
        result = nanobook.rsi(closes, 14)
        assert result is not None

    def test_macd_wrapper(self):
        """Test MACD wrapper."""
        closes = list(range(100, 130))
        result = nanobook.macd(closes, fast_period=12, slow_period=26, signal_period=9)
        assert result is not None

    def test_bollinger_wrapper(self):
        """Test Bollinger Bands wrapper."""
        closes = list(range(100, 120))
        result = nanobook.bollinger(closes, period=20, num_std_up=2.0, num_std_dn=2.0)
        assert result is not None

    def test_wilder_atr_wrapper(self):
        """Test Wilder's ATR wrapper."""
        highs = list(range(110, 130))
        lows = list(range(90, 110))
        closes = list(range(100, 120))
        result = nanobook.wilder_atr(highs, lows, closes, period=14)
        assert result is not None
        assert len(result) > 0

    def test_realized_vol_wrapper(self):
        """Test realized volatility wrapper."""
        opens = [100.0] * 20
        highs = [105.0] * 20
        lows = [95.0] * 20
        closes = [100.0, 101.0, 102.0, 101.0, 103.0] * 4
        result = nanobook.realized_vol(opens, highs, lows, closes, method="close_to_close")
        assert isinstance(result, float)
        assert result >= 0.0

    def test_drawdown_series_wrapper(self):
        """Test drawdown series wrapper."""
        equity = [1000.0, 1100.0, 1050.0, 1200.0, 1150.0]
        result = nanobook.drawdown_series(equity)
        assert result is not None
        assert len(result) > 0

    def test_rolling_max_drawdown_wrapper(self):
        """Test rolling max drawdown wrapper."""
        equity = [1000.0, 1100.0, 1050.0, 1200.0, 1150.0, 1300.0]
        result = nanobook.rolling_max_drawdown(equity, window=3)
        assert result is not None
        assert len(result) > 0

    def test_list_supported_indicators_wrapper(self):
        """Test list supported indicators wrapper."""
        result = nanobook.list_supported_indicators()
        assert isinstance(result, list)
        assert len(result) > 0

    def test_optimize_hrp_wrapper(self):
        """Test HRP optimization wrapper."""
        returns_matrix = [
            [0.01, 0.02, -0.01],
            [0.02, 0.01, 0.00],
            [-0.01, 0.00, 0.02],
            [0.00, 0.02, 0.01],
            [0.01, -0.01, 0.02],
        ]
        symbols = ["A", "B", "C"]
        result = nanobook.optimize_hrp(returns_matrix, symbols)
        assert isinstance(result, dict)
        assert set(result.keys()) == set(symbols)
        assert math.isclose(sum(result.values()), 1.0, abs_tol=1e-6)


class TestMonteCarloResultProperties:
    """Test MonteCarloResult property accessors (mean_price, quantile, etc.)."""

    def test_mean_price_property(self):
        """Test mean_price property access."""
        prices = [100.0, 110.0, 120.0, 130.0]
        result = MonteCarloResult(
            ticker="TEST",
            method="Simple GBM",
            horizon_years=1.0,
            current_price=100.0,
            terminal_prices=prices,
            summary={"mean_price": 115.0, "median_price": 115.0},
        )
        assert result.mean_price == 115.0
        assert isinstance(result.mean_price, float)

    def test_median_price_property(self):
        """Test median_price property access."""
        prices = [100.0, 110.0, 120.0, 130.0]
        result = MonteCarloResult(
            ticker="TEST",
            method="Simple GBM",
            horizon_years=1.0,
            current_price=100.0,
            terminal_prices=prices,
            summary={"median_price": 115.0},
        )
        assert result.median_price == 115.0

    def test_prob_above_empty_prices(self):
        """Test prob_above with empty terminal_prices."""
        result = MonteCarloResult(
            ticker="TEST",
            method="Simple GBM",
            horizon_years=1.0,
            current_price=100.0,
            terminal_prices=[],
            summary={},
        )
        assert result.prob_above(110.0) == 0.0

    def test_prob_above_with_prices(self):
        """Test prob_above computation with prices."""
        prices = [100.0, 105.0, 110.0, 115.0, 120.0]
        result = MonteCarloResult(
            ticker="TEST",
            method="Simple GBM",
            horizon_years=1.0,
            current_price=100.0,
            terminal_prices=prices,
            summary={},
        )
        # 2 out of 5 are strictly > 110 (115, 120)
        assert result.prob_above(110.0) == pytest.approx(0.4, abs=1e-6)
        # 1 out of 5 is > 115
        assert result.prob_above(115.0) == pytest.approx(0.2, abs=1e-6)

    def test_quantile_empty_prices(self):
        """Test quantile with empty terminal_prices."""
        result = MonteCarloResult(
            ticker="TEST",
            method="Simple GBM",
            horizon_years=1.0,
            current_price=100.0,
            terminal_prices=[],
            summary={},
        )
        assert result.quantile(0.5) == 0.0

    def test_quantile_with_prices(self):
        """Test quantile computation."""
        prices = [100.0, 110.0, 120.0, 130.0, 140.0]
        result = MonteCarloResult(
            ticker="TEST",
            method="Simple GBM",
            horizon_years=1.0,
            current_price=100.0,
            terminal_prices=prices,
            summary={},
        )
        assert result.quantile(0.0) == 100.0  # min
        assert result.quantile(0.5) == pytest.approx(120.0, abs=0.1)  # median
        assert result.quantile(1.0) == 140.0  # max

    def test_as_log_returns_invalid_price(self):
        """Test as_log_returns with invalid current_price <= 0."""
        prices = [100.0, 110.0, 120.0]
        result = MonteCarloResult(
            ticker="TEST",
            method="Simple GBM",
            horizon_years=1.0,
            current_price=0.0,
            terminal_prices=prices,
            summary={},
        )
        log_returns = result.as_log_returns()
        assert all(lr == 0.0 for lr in log_returns)

    def test_as_log_returns_valid(self):
        """Test as_log_returns with valid current_price."""
        prices = [100.0, 110.0, 120.0]
        result = MonteCarloResult(
            ticker="TEST",
            method="Simple GBM",
            horizon_years=1.0,
            current_price=100.0,
            terminal_prices=prices,
            summary={},
        )
        log_returns = result.as_log_returns()
        assert len(log_returns) == 3
        assert log_returns[0] == pytest.approx(0.0, abs=1e-10)
        assert log_returns[1] == pytest.approx(math.log(110.0 / 100.0), abs=1e-10)
        assert log_returns[2] == pytest.approx(math.log(120.0 / 100.0), abs=1e-10)

    def test_to_price_paths_invalid_n_periods(self):
        """Test to_price_paths with invalid n_periods."""
        prices = [100.0, 110.0]
        result = MonteCarloResult(
            ticker="TEST",
            method="Simple GBM",
            horizon_years=1.0,
            current_price=100.0,
            terminal_prices=prices,
            summary={},
        )
        with pytest.raises(ValueError, match="n_periods must be >= 1"):
            result.to_price_paths(0)

    def test_to_price_paths_terminal_only(self):
        """Test to_price_paths with terminal_only method."""
        prices = [100.0, 110.0, 120.0]
        result = MonteCarloResult(
            ticker="TEST",
            method="Simple GBM",
            horizon_years=1.0,
            current_price=100.0,
            terminal_prices=prices,
            summary={},
        )
        paths = result.to_price_paths(5, method="terminal_only")
        assert len(paths) == 3
        for i, path in enumerate(paths):
            assert len(path) == 5
            # First 4 should be current_price, last should be terminal
            assert all(p == 100.0 for p in path[:4])
            assert path[-1] == prices[i]

    def test_to_price_paths_invalid_price_terminal_only(self):
        """Test to_price_paths with invalid prices in terminal_only mode."""
        prices = [100.0, 0.0, 120.0]
        result = MonteCarloResult(
            ticker="TEST",
            method="Simple GBM",
            horizon_years=1.0,
            current_price=100.0,
            terminal_prices=prices,
            summary={},
        )
        paths = result.to_price_paths(5, method="terminal_only")
        assert len(paths) == 3
        assert paths[1][-1] == 0.0  # terminal price preserved

    def test_to_price_paths_linear_invalid_prices(self):
        """Test to_price_paths linear interpolation with invalid prices."""
        prices = [100.0, 0.0, 120.0]
        result = MonteCarloResult(
            ticker="TEST",
            method="Simple GBM",
            horizon_years=1.0,
            current_price=100.0,
            terminal_prices=prices,
            summary={},
        )
        paths = result.to_price_paths(5, method="linear")
        assert len(paths) == 3
        # Path with invalid terminal price should be flat at current_price
        assert all(p == 100.0 for p in paths[1])

    def test_to_price_paths_linear_valid(self):
        """Test to_price_paths linear interpolation."""
        prices = [100.0, 130.0]
        result = MonteCarloResult(
            ticker="TEST",
            method="Simple GBM",
            horizon_years=1.0,
            current_price=100.0,
            terminal_prices=prices,
            summary={},
        )
        paths = result.to_price_paths(3, method="linear")
        assert len(paths) == 2
        # First path: 100 stays as 100 (log-linear of 100->100)
        # Second path: 100 -> ?interpolated -> 130 (log-linear)
        path = paths[1]
        assert len(path) == 3
        assert path[0] == pytest.approx(100.0, abs=1e-4)
        assert path[-1] == pytest.approx(130.0, abs=1e-4)


class TestValidation:
    """Test validation functions and error paths."""

    def test_validate_mc_inputs_invalid_price_negative(self):
        """Test validation with negative price."""
        with pytest.raises(ValueError, match="current_price must be positive"):
            _validate_mc_inputs(-10.0, 100, 1.0, 0.2)

    def test_validate_mc_inputs_invalid_price_zero(self):
        """Test validation with zero price."""
        with pytest.raises(ValueError, match="current_price must be positive"):
            _validate_mc_inputs(0.0, 100, 1.0, 0.2)

    def test_validate_mc_inputs_invalid_price_inf(self):
        """Test validation with infinite price."""
        with pytest.raises(ValueError, match="current_price must be positive"):
            _validate_mc_inputs(float("inf"), 100, 1.0, 0.2)

    def test_validate_mc_inputs_invalid_horizon_negative(self):
        """Test validation with negative horizon."""
        with pytest.raises(ValueError, match="horizon must be positive"):
            _validate_mc_inputs(100.0, 100, -1.0, 0.2)

    def test_validate_mc_inputs_invalid_horizon_zero(self):
        """Test validation with zero horizon."""
        with pytest.raises(ValueError, match="horizon must be positive"):
            _validate_mc_inputs(100.0, 100, 0.0, 0.2)

    def test_validate_mc_inputs_invalid_horizon_inf(self):
        """Test validation with infinite horizon."""
        with pytest.raises(ValueError, match="horizon must be positive"):
            _validate_mc_inputs(100.0, 100, float("inf"), 0.2)

    def test_validate_mc_inputs_invalid_vol_negative(self):
        """Test validation with negative vol."""
        with pytest.raises(ValueError, match="annual_vol must be non-negative"):
            _validate_mc_inputs(100.0, 100, 1.0, -0.1)

    def test_validate_mc_inputs_invalid_vol_inf(self):
        """Test validation with infinite vol."""
        with pytest.raises(ValueError, match="annual_vol must be non-negative"):
            _validate_mc_inputs(100.0, 100, 1.0, float("inf"))

    def test_validate_mc_inputs_valid(self):
        """Valid inputs return None, and each boundary is one step away.

        The four rejections prove the validator is still live on this same
        call, so acceptance means something. Without them the test would also
        pass if the function body were deleted.
        """
        assert _validate_mc_inputs(100.0, 1000, 1.0, 0.2) is None

        # n_paths = 0 is allowed (the guard is < 0), and vol = 0 is allowed
        # (the guard is < 0), so both edges of the accepted range stay in.
        assert _validate_mc_inputs(100.0, 0, 1.0, 0.0) is None

        with pytest.raises(ValueError, match="current_price must be positive"):
            _validate_mc_inputs(0.0, 1000, 1.0, 0.2)
        with pytest.raises(ValueError, match="n_paths must be >= 0"):
            _validate_mc_inputs(100.0, -1, 1.0, 0.2)
        with pytest.raises(ValueError, match="horizon must be positive"):
            _validate_mc_inputs(100.0, 1000, 0.0, 0.2)
        with pytest.raises(ValueError, match="annual_vol must be non-negative"):
            _validate_mc_inputs(100.0, 1000, 1.0, -0.1)


class TestRngAndNormal:
    """Test RNG creation and normal distribution drawing."""

    def test_make_pure_rng_with_seed(self):
        """Test _make_pure_rng with integer seed."""
        rng = _make_pure_rng(42)
        assert isinstance(rng, random.Random)
        val1 = rng.random()
        
        rng2 = _make_pure_rng(42)
        val2 = rng2.random()
        assert val1 == val2

    def test_make_pure_rng_with_random_instance(self):
        """Test _make_pure_rng with Random instance."""
        provided = random.Random(99)
        rng = _make_pure_rng(provided)
        assert rng is provided

    def test_make_pure_rng_with_none(self):
        """Test _make_pure_rng with None seed."""
        rng = _make_pure_rng(None)
        assert isinstance(rng, random.Random)

    def test_get_rng_with_numpy_generator(self):
        """Test _get_rng with numpy Generator."""
        gen = np.random.default_rng(42)
        result = _get_rng(gen)
        assert result is gen

    def test_get_rng_with_int_seed(self):
        """Test _get_rng with int seed uses numpy if available."""
        result = _get_rng(42)
        # When numpy available, should return np.random.Generator
        if np is not None:
            assert hasattr(result, "normal")

    def test_get_rng_with_random_instance(self):
        """Test _get_rng with random.Random instance."""
        rng = random.Random(42)
        result = _get_rng(rng)
        # With numpy available, _get_rng may convert to numpy generator
        # but the behavior is to accept random.Random too
        assert result is not None

    def test_normal_with_zero_sigma(self):
        """Test _normal with zero sigma returns mu."""
        rng = random.Random(42)
        result = _normal(rng, 100.0, 0.0)
        assert result == 100.0

    def test_normal_with_random_rng(self):
        """Test _normal with random.Random."""
        rng = random.Random(42)
        result = _normal(rng, 100.0, 1.0)
        assert isinstance(result, float)
        assert 95.0 < result < 105.0  # Rough sanity check

    def test_normal_box_muller_with_zero_sigma(self):
        """Test _normal_box_muller with zero sigma returns mu."""
        rng = random.Random(42)
        result = _normal_box_muller(rng, 100.0, 0.0)
        assert result == 100.0

    def test_normal_box_muller_draws(self):
        """Test _normal_box_muller draws approximately normal."""
        rng = random.Random(42)
        samples = [_normal_box_muller(rng, 100.0, 10.0) for _ in range(1000)]
        mean = sum(samples) / len(samples)
        # Mean should be roughly 100
        assert 98.0 < mean < 102.0


class TestPureStatisticsFunctions:
    """Test pure-Python statistics utilities."""

    def test_pure_mean_empty(self):
        """Test _pure_mean with empty list."""
        assert _pure_mean([]) == 0.0

    def test_pure_mean_values(self):
        """Test _pure_mean computation."""
        assert _pure_mean([1.0, 2.0, 3.0, 4.0, 5.0]) == pytest.approx(3.0)

    def test_pure_percentile_boundaries(self):
        """Test _pure_percentile at boundaries."""
        values = [1.0, 2.0, 3.0, 4.0, 5.0]
        assert _pure_percentile(values, 0.0) == 1.0
        assert _pure_percentile(values, 1.0) == 5.0

    def test_pure_percentile_mid(self):
        """Test _pure_percentile at middle."""
        values = [1.0, 2.0, 3.0, 4.0, 5.0]
        result = _pure_percentile(values, 0.5)
        assert result == pytest.approx(3.0, abs=0.1)

    def test_pure_prob_above_empty(self):
        """Test _pure_prob_above with empty list."""
        assert _pure_prob_above([], 100.0) == 0.0

    def test_pure_prob_above_values(self):
        """Test _pure_prob_above computation."""
        values = [100.0, 110.0, 120.0, 130.0, 140.0]
        result = _pure_prob_above(values, 115.0)
        # Only 120, 130, 140 are > 115
        assert result == pytest.approx(0.6, abs=1e-6)


class TestSimpleGBM:
    """Test simple GBM path generation."""

    def test_simple_gbm_terminal_deterministic(self):
        """Test simple_gbm_terminal with fixed seed."""
        rng = random.Random(42)
        prices1 = simple_gbm_terminal(
            current_price=100.0,
            n_paths=100,
            horizon=1.0,
            expected_annual_return=0.10,
            annual_vol=0.20,
            rng=rng,
        )
        assert len(prices1) == 100
        assert all(p > 0 for p in prices1)
        
        # Same seed should give same results
        rng2 = random.Random(42)
        prices2 = simple_gbm_terminal(
            current_price=100.0,
            n_paths=100,
            horizon=1.0,
            expected_annual_return=0.10,
            annual_vol=0.20,
            rng=rng2,
        )
        assert prices1 == prices2

    def test_simple_gbm_terminal_invalid_price(self):
        """Test simple_gbm_terminal with invalid price."""
        rng = random.Random(42)
        with pytest.raises(ValueError):
            simple_gbm_terminal(
                current_price=-100.0,
                n_paths=10,
                horizon=1.0,
                expected_annual_return=0.10,
                annual_vol=0.20,
                rng=rng,
            )


class TestAdvancedMultiDriver:
    """Test advanced multi-driver model."""

    def test_advanced_multi_driver_deterministic(self):
        """Test advanced_multi_driver_terminal with fixed seed."""
        rng = random.Random(42)
        params = ValuationParams()
        prices1 = advanced_multi_driver_terminal(
            current_price=100.0,
            n_paths=100,
            horizon=1.0,
            params=params,
            rng=rng,
        )
        assert len(prices1) == 100
        assert all(p > 0 for p in prices1)
        
        # Same seed should give same results
        rng2 = random.Random(42)
        prices2 = advanced_multi_driver_terminal(
            current_price=100.0,
            n_paths=100,
            horizon=1.0,
            params=params,
            rng=rng2,
        )
        assert prices1 == prices2

    def test_advanced_multi_driver_invalid_price(self):
        """Test advanced_multi_driver_terminal with invalid price."""
        rng = random.Random(42)
        params = ValuationParams()
        with pytest.raises(ValueError):
            advanced_multi_driver_terminal(
                current_price=-100.0,
                n_paths=10,
                horizon=1.0,
                params=params,
                rng=rng,
            )

    def test_advanced_multi_driver_invalid_horizon(self):
        """Test advanced_multi_driver_terminal with invalid horizon."""
        rng = random.Random(42)
        params = ValuationParams()
        with pytest.raises(ValueError):
            advanced_multi_driver_terminal(
                current_price=100.0,
                n_paths=10,
                horizon=0.0,
                params=params,
                rng=rng,
            )


class TestComputeAnnualizedVol:
    """Test compute_annualized_vol with various inputs."""

    def test_compute_annualized_vol_small_sample(self):
        """Test compute_annualized_vol with < 2 samples returns default."""
        result = compute_annualized_vol([0.01])
        assert result == pytest.approx(0.30)

    def test_compute_annualized_vol_valid_list(self):
        """Test compute_annualized_vol with valid list."""
        returns = [0.01, 0.02, -0.01, 0.015, -0.005]
        result = compute_annualized_vol(returns)
        assert result > 0.0
        assert result < 1.0

    def test_compute_annualized_vol_with_nans(self):
        """Test compute_annualized_vol filters NaNs."""
        returns = [0.01, float("nan"), 0.02, -0.01, float("nan")]
        result = compute_annualized_vol(returns)
        assert result > 0.0
        assert math.isfinite(result)

    def test_compute_annualized_vol_numpy_array(self):
        """Test compute_annualized_vol with numpy array."""
        returns = np.array([0.01, 0.02, -0.01, 0.015, -0.005])
        result = compute_annualized_vol(returns)
        assert result > 0.0
        assert math.isfinite(result)


class TestTerminalPricesPaths:
    """Test terminal_prices_to_log_return_paths function."""

    def test_terminal_prices_to_paths_invalid_periods(self):
        """Test with invalid n_periods."""
        with pytest.raises(ValueError, match="n_periods must be >= 1"):
            terminal_prices_to_log_return_paths([100.0, 110.0], 100.0, 0)

    def test_terminal_prices_to_paths_terminal_only(self):
        """Test terminal_only method."""
        terminal_prices = [100.0, 110.0, 120.0]
        paths = terminal_prices_to_log_return_paths(
            terminal_prices,
            current_price=100.0,
            n_periods=5,
            method="terminal_only",
        )
        assert len(paths) == 3
        for i, path in enumerate(paths):
            assert len(path) == 5
            assert all(p == 100.0 for p in path[:4])
            assert path[-1] == terminal_prices[i]

    def test_terminal_prices_to_paths_linear(self):
        """Test linear interpolation."""
        terminal_prices = [100.0, 130.0]
        paths = terminal_prices_to_log_return_paths(
            terminal_prices,
            current_price=100.0,
            n_periods=3,
            method="linear",
        )
        assert len(paths) == 2
        # First terminal = current, so flat path
        path1 = paths[0]
        assert len(path1) == 3
        assert all(p == pytest.approx(100.0, abs=1e-4) for p in path1)
        
        # Second terminal = 130, so interpolated path
        path2 = paths[1]
        assert len(path2) == 3
        assert path2[0] == pytest.approx(100.0, abs=1e-4)
        assert path2[-1] == pytest.approx(130.0, abs=1e-4)

    def test_terminal_prices_to_paths_invalid_terminal(self):
        """Test with invalid terminal price (zero or negative)."""
        terminal_prices = [100.0, 0.0, 120.0]
        paths = terminal_prices_to_log_return_paths(
            terminal_prices,
            current_price=100.0,
            n_periods=3,
            method="linear",
        )
        assert len(paths) == 3
        # Path with zero terminal should be flat at current_price
        assert all(p == 100.0 for p in paths[1])

    def test_terminal_prices_to_paths_zero_current_price(self):
        """Test with zero current_price."""
        terminal_prices = [100.0, 110.0]
        paths = terminal_prices_to_log_return_paths(
            terminal_prices,
            current_price=0.0,
            n_periods=3,
            method="linear",
        )
        assert len(paths) == 2
        # Both paths should be flat at 0
        assert all(p == 0.0 for p in paths[0])


class TestSummarizeDistribution:
    """Test summarize_distribution helper."""

    def test_summarize_distribution_empty(self):
        """Test with empty prices."""
        result = summarize_distribution([], 100.0)
        assert result["median"] == 0.0
        assert result["mean"] == 0.0
        assert result["p05"] == 0.0
        assert result["p95"] == 0.0
        assert result["prob_above_current"] == 0.0

    def test_summarize_distribution_values(self):
        """Test with actual prices."""
        prices = [100.0, 110.0, 120.0, 130.0, 140.0]
        result = summarize_distribution(prices, current_price=100.0)
        assert result["median"] > 0.0
        assert result["mean"] > 0.0
        assert result["p05"] < result["median"]
        assert result["median"] < result["p95"]
        assert result["prob_above_current"] > 0.0
        assert result["prob_above_10pct"] >= 0.0


class TestPureMonteCarloPath:
    """Test _pure_python_mc with simple model using random.Random."""

    def test_pure_python_mc_simple_with_random_rng(self):
        """Test _pure_python_mc simple version with pure random.Random."""
        # This exercises line 470: the simple_gbm_terminal path in _pure_python_mc
        result = _pure_python_mc(
            ticker="TEST",
            current_price=100.0,
            version="simple",
            n_paths=100,
            horizon=1.0,
            seed=random.Random(42),
            expected_annual_return=0.10,
            annual_vol=0.20,
            gp_growth_mean=0.16,
            gp_growth_sd=0.06,
            margin_boost_mean=0.02,
            margin_boost_sd=0.03,
            multiple_mean=22.0,
            multiple_sd=3.5,
            macro_shock_mean=-0.03,
            macro_shock_sd=0.11,
            bear_skew_factor=0.04,
            hurdle_rate=0.08,
            bull_price=None,
            bear_price=None,
        )
        assert isinstance(result, MonteCarloResult)
        assert len(result.terminal_prices) == 100
        assert result.method == "Simple GBM"
        assert all(p > 0 for p in result.terminal_prices)
