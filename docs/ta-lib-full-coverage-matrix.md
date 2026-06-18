# TA-Lib full coverage matrix

Total TA-Lib functions: **158**
Parity-implemented (registry): **25**

Status legend:
- **implemented** — Rust + golden parity in nanobook
- **deferred** — classified with explicit rationale (not a backlog gap)

| Group | Function | Status | Rationale |
|-------|----------|--------|-----------|
| Cycle Indicators | HT_DCPERIOD | deferred | Hilbert transform family — niche for equity daily bars |
| Cycle Indicators | HT_DCPHASE | deferred | Hilbert transform family — niche for equity daily bars |
| Cycle Indicators | HT_PHASOR | deferred | Hilbert transform family — niche for equity daily bars |
| Cycle Indicators | HT_SINE | deferred | Hilbert transform family — niche for equity daily bars |
| Cycle Indicators | HT_TRENDMODE | deferred | Hilbert transform family — niche for equity daily bars |
| Math Operators | ADD | deferred | element-wise math — not strategy indicators |
| Math Operators | DIV | deferred | element-wise math — not strategy indicators |
| Math Operators | MAX | deferred | element-wise math — not strategy indicators |
| Math Operators | MAXINDEX | deferred | element-wise math — not strategy indicators |
| Math Operators | MIN | deferred | element-wise math — not strategy indicators |
| Math Operators | MININDEX | deferred | element-wise math — not strategy indicators |
| Math Operators | MINMAX | deferred | element-wise math — not strategy indicators |
| Math Operators | MINMAXINDEX | deferred | element-wise math — not strategy indicators |
| Math Operators | MULT | deferred | element-wise math — not strategy indicators |
| Math Operators | SUB | deferred | element-wise math — not strategy indicators |
| Math Operators | SUM | deferred | element-wise math — not strategy indicators |
| Math Transform | ACOS | deferred | trigonometric transforms — not strategy indicators |
| Math Transform | ASIN | deferred | trigonometric transforms — not strategy indicators |
| Math Transform | ATAN | deferred | trigonometric transforms — not strategy indicators |
| Math Transform | CEIL | deferred | trigonometric transforms — not strategy indicators |
| Math Transform | COS | deferred | trigonometric transforms — not strategy indicators |
| Math Transform | COSH | deferred | trigonometric transforms — not strategy indicators |
| Math Transform | EXP | deferred | trigonometric transforms — not strategy indicators |
| Math Transform | FLOOR | deferred | trigonometric transforms — not strategy indicators |
| Math Transform | LN | deferred | trigonometric transforms — not strategy indicators |
| Math Transform | LOG10 | deferred | trigonometric transforms — not strategy indicators |
| Math Transform | SIN | deferred | trigonometric transforms — not strategy indicators |
| Math Transform | SINH | deferred | trigonometric transforms — not strategy indicators |
| Math Transform | SQRT | deferred | trigonometric transforms — not strategy indicators |
| Math Transform | TAN | deferred | trigonometric transforms — not strategy indicators |
| Math Transform | TANH | deferred | trigonometric transforms — not strategy indicators |
| Momentum Indicators | ADX | implemented | golden parity via indicator_registry.json |
| Momentum Indicators | ADXR | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Momentum Indicators | APO | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Momentum Indicators | AROON | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Momentum Indicators | AROONOSC | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Momentum Indicators | BOP | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Momentum Indicators | CCI | implemented | golden parity via indicator_registry.json |
| Momentum Indicators | CMO | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Momentum Indicators | DX | implemented | golden parity via indicator_registry.json |
| Momentum Indicators | MACD | implemented | golden parity via indicator_registry.json |
| Momentum Indicators | MACDEXT | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Momentum Indicators | MACDFIX | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Momentum Indicators | MFI | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Momentum Indicators | MINUS_DI | implemented | golden parity via indicator_registry.json |
| Momentum Indicators | MINUS_DM | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Momentum Indicators | MOM | implemented | golden parity via indicator_registry.json |
| Momentum Indicators | PLUS_DI | implemented | golden parity via indicator_registry.json |
| Momentum Indicators | PLUS_DM | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Momentum Indicators | PPO | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Momentum Indicators | ROC | implemented | golden parity via indicator_registry.json |
| Momentum Indicators | ROCP | implemented | golden parity via indicator_registry.json |
| Momentum Indicators | ROCR | implemented | golden parity via indicator_registry.json |
| Momentum Indicators | ROCR100 | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Momentum Indicators | RSI | implemented | golden parity via indicator_registry.json |
| Momentum Indicators | STOCH | implemented | golden parity via indicator_registry.json |
| Momentum Indicators | STOCHF | implemented | golden parity via indicator_registry.json |
| Momentum Indicators | STOCHRSI | implemented | golden parity via indicator_registry.json |
| Momentum Indicators | TRIX | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Momentum Indicators | ULTOSC | implemented | golden parity via indicator_registry.json |
| Momentum Indicators | WILLR | implemented | golden parity via indicator_registry.json |
| Overlap Studies | BBANDS | implemented | golden parity via indicator_registry.json |
| Overlap Studies | DEMA | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Overlap Studies | EMA | implemented | golden parity via indicator_registry.json |
| Overlap Studies | HT_TRENDLINE | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Overlap Studies | KAMA | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Overlap Studies | MA | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Overlap Studies | MAMA | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Overlap Studies | MAVP | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Overlap Studies | MIDPOINT | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Overlap Studies | MIDPRICE | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Overlap Studies | SAR | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Overlap Studies | SAREXT | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Overlap Studies | SMA | implemented | golden parity via indicator_registry.json |
| Overlap Studies | T3 | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Overlap Studies | TEMA | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Overlap Studies | TRIMA | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Overlap Studies | WMA | deferred | not in curated 25-35 high-signal set for Strategy Spec v2 |
| Pattern Recognition | CDL2CROWS | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDL3BLACKCROWS | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDL3INSIDE | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDL3LINESTRIKE | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDL3OUTSIDE | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDL3STARSINSOUTH | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDL3WHITESOLDIERS | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLABANDONEDBABY | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLADVANCEBLOCK | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLBELTHOLD | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLBREAKAWAY | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLCLOSINGMARUBOZU | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLCONCEALBABYSWALL | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLCOUNTERATTACK | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLDARKCLOUDCOVER | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLDOJI | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLDOJISTAR | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLDRAGONFLYDOJI | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLENGULFING | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLEVENINGDOJISTAR | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLEVENINGSTAR | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLGAPSIDESIDEWHITE | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLGRAVESTONEDOJI | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLHAMMER | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLHANGINGMAN | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLHARAMI | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLHARAMICROSS | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLHIGHWAVE | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLHIKKAKE | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLHIKKAKEMOD | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLHOMINGPIGEON | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLIDENTICAL3CROWS | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLINNECK | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLINVERTEDHAMMER | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLKICKING | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLKICKINGBYLENGTH | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLLADDERBOTTOM | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLLONGLEGGEDDOJI | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLLONGLINE | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLMARUBOZU | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLMATCHINGLOW | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLMATHOLD | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLMORNINGDOJISTAR | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLMORNINGSTAR | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLONNECK | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLPIERCING | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLRICKSHAWMAN | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLRISEFALL3METHODS | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLSEPARATINGLINES | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLSHOOTINGSTAR | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLSHORTLINE | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLSPINNINGTOP | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLSTALLEDPATTERN | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLSTICKSANDWICH | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLTAKURI | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLTASUKIGAP | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLTHRUSTING | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLTRISTAR | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLUNIQUE3RIVER | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLUPSIDEGAP2CROWS | deferred | candlestick patterns — out of scope for v2 price filters |
| Pattern Recognition | CDLXSIDEGAP3METHODS | deferred | candlestick patterns — out of scope for v2 price filters |
| Price Transform | AVGPRICE | deferred | AVGPRICE/MEDPRICE etc — low signal for manifests |
| Price Transform | MEDPRICE | deferred | AVGPRICE/MEDPRICE etc — low signal for manifests |
| Price Transform | TYPPRICE | deferred | AVGPRICE/MEDPRICE etc — low signal for manifests |
| Price Transform | WCLPRICE | deferred | AVGPRICE/MEDPRICE etc — low signal for manifests |
| Statistic Functions | BETA | deferred | stats primitives — covered elsewhere (nanobook stats) |
| Statistic Functions | CORREL | deferred | stats primitives — covered elsewhere (nanobook stats) |
| Statistic Functions | LINEARREG | deferred | stats primitives — covered elsewhere (nanobook stats) |
| Statistic Functions | LINEARREG_ANGLE | deferred | stats primitives — covered elsewhere (nanobook stats) |
| Statistic Functions | LINEARREG_INTERCEPT | deferred | stats primitives — covered elsewhere (nanobook stats) |
| Statistic Functions | LINEARREG_SLOPE | deferred | stats primitives — covered elsewhere (nanobook stats) |
| Statistic Functions | STDDEV | deferred | stats primitives — covered elsewhere (nanobook stats) |
| Statistic Functions | TSF | deferred | stats primitives — covered elsewhere (nanobook stats) |
| Statistic Functions | VAR | deferred | stats primitives — covered elsewhere (nanobook stats) |
| Volatility Indicators | ATR | implemented | golden parity via indicator_registry.json |
| Volatility Indicators | NATR | implemented | golden parity via indicator_registry.json |
| Volatility Indicators | TRANGE | implemented | golden parity via indicator_registry.json |
| Volume Indicators | AD | implemented | golden parity via indicator_registry.json |
| Volume Indicators | ADOSC | implemented | golden parity via indicator_registry.json |
| Volume Indicators | OBV | implemented | golden parity via indicator_registry.json |

## Summary

- Rows: 158 (100% of `talib.get_functions()`)
- Implemented: 25
- Deferred: 133

Regenerate: `uv run --with TA-Lib python tests/parity/generate_coverage_matrix.py`
