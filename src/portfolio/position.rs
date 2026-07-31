//! Position tracking for a single symbol.

use crate::types::Symbol;

/// A share count in fixed-point micro-shares: `1` unit = `0.000001` share.
///
/// A newtype rather than `type Shares = i64` deliberately: a bare alias would let
/// every existing `i64`-based call site keep compiling while silently meaning a
/// quantity 1,000,000x smaller. The newtype forces each call site to be revisited.
///
/// [`Portfolio`](super::Portfolio)'s `quantity_step` (also in these units) controls
/// the granularity actually used when sizing orders:
///
/// | `quantity_step` | Meaning | Use |
/// |---|---|---|
/// | `1_000_000` | whole shares | default — bit-identical to pre-fractional behaviour |
/// | `1_000` | 0.001 share | Alpaca's fractional minimum |
/// | `100` | 0.0001 share | IBKR's fractional minimum |
/// | `1` | 0.000001 share | effectively continuous |
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Shares(i64);

impl Shares {
    /// Micro-shares per whole share.
    pub const SCALE: i64 = 1_000_000;

    /// A flat (zero) quantity.
    pub const ZERO: Shares = Shares(0);

    /// Build from a whole-share count (e.g. `Shares::from_whole(100)` = 100 shares).
    pub fn from_whole(n: i64) -> Self {
        Shares(n * Self::SCALE)
    }

    /// Build from a raw micro-share count.
    pub fn from_raw(raw: i64) -> Self {
        Shares(raw)
    }

    /// The raw micro-share count (signed: positive = long, negative = short).
    pub fn raw(self) -> i64 {
        self.0
    }

    /// The quantity as a fractional share count.
    pub fn to_f64(self) -> f64 {
        self.0 as f64 / Self::SCALE as f64
    }

    /// Truncate to a whole-share count (drops any fractional remainder).
    pub fn whole(self) -> i64 {
        self.0 / Self::SCALE
    }

    pub fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub fn is_positive(self) -> bool {
        self.0 > 0
    }

    pub fn is_negative(self) -> bool {
        self.0 < 0
    }

    pub fn abs(self) -> Self {
        Shares(self.0.saturating_abs())
    }

    /// Absolute value as an unsigned raw micro-share count.
    pub fn unsigned_abs(self) -> u64 {
        self.0.unsigned_abs()
    }
}

impl std::ops::Add for Shares {
    type Output = Shares;
    fn add(self, rhs: Self) -> Self::Output {
        Shares(self.0.saturating_add(rhs.0))
    }
}

impl std::ops::Sub for Shares {
    type Output = Shares;
    fn sub(self, rhs: Self) -> Self::Output {
        Shares(self.0.saturating_sub(rhs.0))
    }
}

impl std::ops::Neg for Shares {
    type Output = Shares;
    fn neg(self) -> Self::Output {
        Shares(self.0.saturating_neg())
    }
}

impl std::ops::AddAssign for Shares {
    fn add_assign(&mut self, rhs: Self) {
        self.0 = self.0.saturating_add(rhs.0);
    }
}

/// A position in a single instrument.
///
/// Tracks quantity (positive = long, negative = short), average entry price,
/// and realized PnL. All monetary values are in the smallest currency unit (cents).
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Position {
    /// Symbol this position is for
    pub symbol: Symbol,
    /// Net quantity: positive = long, negative = short, zero = flat
    pub quantity: Shares,
    /// Volume-weighted average entry price (cents)
    pub avg_entry_price: i64,
    /// Cumulative realized PnL (cents)
    pub realized_pnl: i64,
    /// Cumulative cost of entry (quantity * avg_entry_price), used for VWAP tracking
    pub total_cost: i64,
}

/// `shares_raw * price_cents`, normalized back from micro-shares to whole-share
/// notional (cents), via an `i128` intermediate so a large position can't wrap
/// an `i64` before the `/ SCALE` narrows it back down. Saturates to `i64` range.
#[inline]
fn notional_cents(shares_raw: i64, price_cents: i64) -> i64 {
    let product = (shares_raw as i128) * (price_cents as i128) / (Shares::SCALE as i128);
    product.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

impl Position {
    /// Create a new flat position for the given symbol.
    pub fn new(symbol: Symbol) -> Self {
        Self {
            symbol,
            quantity: Shares::ZERO,
            avg_entry_price: 0,
            realized_pnl: 0,
            total_cost: 0,
        }
    }

    /// Apply a fill to this position.
    ///
    /// `qty` is signed micro-shares: positive = buy, negative = sell.
    /// `price` is in cents (matches `Price.0`).
    ///
    /// If the fill increases the position (same direction), the average entry
    /// price is updated via VWAP. If it reduces or flips the position,
    /// realized PnL is recorded for the closed portion.
    pub fn apply_fill(&mut self, qty: Shares, price: i64) {
        if qty.is_zero() {
            return;
        }

        let same_direction = (self.quantity.raw() >= 0 && qty.is_positive())
            || (self.quantity.raw() <= 0 && qty.is_negative());

        if self.quantity.is_zero() {
            // Opening a new position
            self.quantity = qty;
            self.avg_entry_price = price;
            self.total_cost = notional_cents(qty.raw(), price);
        } else if same_direction {
            // Adding to position — update VWAP
            self.total_cost = self
                .total_cost
                .saturating_add(notional_cents(qty.raw(), price));
            self.quantity += qty;
            self.avg_entry_price = avg_price(self.total_cost, self.quantity);
        } else {
            // Reducing or flipping
            let close_qty = qty.abs().min(self.quantity.abs());
            let pnl_per_unit = if self.quantity.is_positive() {
                price - self.avg_entry_price // long: sell higher = profit
            } else {
                self.avg_entry_price - price // short: buy lower = profit
            };
            self.realized_pnl = self
                .realized_pnl
                .saturating_add(notional_cents(close_qty.raw(), pnl_per_unit));

            let net = self.quantity + qty;
            if net.is_zero() {
                // Fully closed
                self.quantity = Shares::ZERO;
                self.avg_entry_price = 0;
                self.total_cost = 0;
            } else if net.is_positive() == self.quantity.is_positive() {
                // Partially closed, same side — subtract closed portion's cost
                // to preserve any fractional remainder in total_cost
                self.total_cost = self
                    .total_cost
                    .saturating_sub(notional_cents(close_qty.raw(), self.avg_entry_price));
                self.quantity = net;
                self.avg_entry_price = avg_price(self.total_cost, self.quantity);
            } else {
                // Flipped sides
                self.quantity = net;
                self.avg_entry_price = price;
                self.total_cost = notional_cents(net.raw(), price);
            }
        }
    }

    /// Current market value at the given price (cents).
    #[inline]
    pub fn market_value(&self, price: i64) -> i64 {
        notional_cents(self.quantity.raw(), price)
    }

    /// Unrealized PnL at the given market price (cents).
    #[inline]
    pub fn unrealized_pnl(&self, price: i64) -> i64 {
        if self.quantity.is_zero() {
            return 0;
        }
        notional_cents(
            self.quantity.raw(),
            price.saturating_sub(self.avg_entry_price),
        )
    }

    /// Returns true if the position is flat (zero quantity).
    #[inline]
    pub fn is_flat(&self) -> bool {
        self.quantity.is_zero()
    }
}

/// Recover average entry price (cents/share) from total cost basis (cents) and
/// quantity (micro-shares), via an `i128` intermediate.
#[inline]
fn avg_price(total_cost_cents: i64, quantity: Shares) -> i64 {
    if quantity.is_zero() {
        return 0;
    }
    let product = (total_cost_cents as i128) * (Shares::SCALE as i128) / (quantity.raw() as i128);
    product.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym() -> Symbol {
        Symbol::new("AAPL")
    }

    fn w(n: i64) -> Shares {
        Shares::from_whole(n)
    }

    #[test]
    fn new_position_is_flat() {
        let pos = Position::new(sym());
        assert!(pos.is_flat());
        assert_eq!(pos.quantity, Shares::ZERO);
        assert_eq!(pos.realized_pnl, 0);
        assert_eq!(pos.unrealized_pnl(100_00), 0);
    }

    #[test]
    fn open_long() {
        let mut pos = Position::new(sym());
        pos.apply_fill(w(100), 50_00);
        assert_eq!(pos.quantity, w(100));
        assert_eq!(pos.avg_entry_price, 50_00);
        assert_eq!(pos.market_value(55_00), 100 * 55_00);
        assert_eq!(pos.unrealized_pnl(55_00), 100 * 5_00);
    }

    #[test]
    fn add_to_long_vwap() {
        let mut pos = Position::new(sym());
        pos.apply_fill(w(100), 50_00); // buy 100 @ $50
        pos.apply_fill(w(100), 60_00); // buy 100 @ $60
        assert_eq!(pos.quantity, w(200));
        assert_eq!(pos.avg_entry_price, 55_00); // VWAP
    }

    #[test]
    fn close_long_with_profit() {
        let mut pos = Position::new(sym());
        pos.apply_fill(w(100), 50_00); // buy 100 @ $50
        pos.apply_fill(w(-100), 60_00); // sell 100 @ $60
        assert!(pos.is_flat());
        assert_eq!(pos.realized_pnl, 100 * 10_00); // $10 * 100 shares
    }

    #[test]
    fn close_long_with_loss() {
        let mut pos = Position::new(sym());
        pos.apply_fill(w(100), 50_00); // buy 100 @ $50
        pos.apply_fill(w(-100), 45_00); // sell 100 @ $45
        assert!(pos.is_flat());
        assert_eq!(pos.realized_pnl, -100 * 5_00); // -$5 * 100 shares
    }

    #[test]
    fn partial_close() {
        let mut pos = Position::new(sym());
        pos.apply_fill(w(100), 50_00);
        pos.apply_fill(w(-50), 60_00); // close half
        assert_eq!(pos.quantity, w(50));
        assert_eq!(pos.avg_entry_price, 50_00); // unchanged
        assert_eq!(pos.realized_pnl, 50 * 10_00);
    }

    #[test]
    fn flip_long_to_short() {
        let mut pos = Position::new(sym());
        pos.apply_fill(w(100), 50_00); // long 100 @ $50
        pos.apply_fill(w(-150), 60_00); // sell 150 — close 100, open short 50
        assert_eq!(pos.quantity, w(-50));
        assert_eq!(pos.avg_entry_price, 60_00);
        assert_eq!(pos.realized_pnl, 100 * 10_00); // profit on closed long
    }

    #[test]
    fn short_position() {
        let mut pos = Position::new(sym());
        pos.apply_fill(w(-100), 50_00); // short 100 @ $50
        assert_eq!(pos.quantity, w(-100));
        assert_eq!(pos.unrealized_pnl(45_00), 100 * 5_00); // profit when price drops
        assert_eq!(pos.unrealized_pnl(55_00), -100 * 5_00); // loss when price rises
    }

    #[test]
    fn close_short_with_profit() {
        let mut pos = Position::new(sym());
        pos.apply_fill(w(-100), 50_00); // short @ $50
        pos.apply_fill(w(100), 40_00); // cover @ $40
        assert!(pos.is_flat());
        assert_eq!(pos.realized_pnl, 100 * 10_00); // $10 * 100
    }

    #[test]
    fn zero_fill_is_noop() {
        let mut pos = Position::new(sym());
        pos.apply_fill(w(100), 50_00);
        pos.apply_fill(Shares::ZERO, 60_00);
        assert_eq!(pos.quantity, w(100));
        assert_eq!(pos.avg_entry_price, 50_00);
    }

    #[test]
    fn fractional_quantity_round_trips() {
        // 0.5 share = 500_000 micro-shares.
        let half = Shares::from_raw(500_000);
        assert_eq!(half.to_f64(), 0.5);
        assert_eq!(half.whole(), 0);
        assert!(!half.is_zero());
    }

    #[test]
    fn market_value_overflow_guard() {
        // 1,000,000 shares at $10,000 (1_000_000_00 cents): the raw micro-share
        // count is 1_000_000 * Shares::SCALE = 1e12. Multiplying that directly
        // by price (1e8) before normalizing by SCALE gives ~1e20, well past
        // i64::MAX (~9.2e18) — the naive computation this guards against.
        let quantity = w(1_000_000);
        let price = 1_000_000_00i64;
        let naive_product = quantity.raw() as i128 * price as i128;
        assert!(
            naive_product > i64::MAX as i128,
            "test setup doesn't actually stress the overflow path"
        );

        let mut pos = Position::new(sym());
        pos.apply_fill(quantity, price);

        // The correctly normalized market value (cents) is quantity_shares * price.
        let expected: i128 = 1_000_000i128 * price as i128;
        assert_eq!(pos.market_value(price) as i128, expected);
    }
}
