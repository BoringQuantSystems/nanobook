//! Transaction cost modeling.

/// Models transaction costs for portfolio rebalancing.
///
/// Commissions are computed as a percentage of notional value (in basis points)
/// plus a minimum per-fill commission. Slippage is modeled as price impact by
/// portfolio execution, not by [`CostModel::compute_cost`].
///
/// ```ignore
/// use nanobook::portfolio::CostModel;
///
/// let model = CostModel { commission_bps: 10.0, slippage_bps: 5.0, min_commission: 1_00 };
/// // 10 bps commission on $10,000 notional = $10.00; slippage is price impact.
/// assert_eq!(model.compute_cost(1_000_000), 1000);
/// ```
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CostModel {
    /// Commission in basis points (1 bps = 0.01%)
    pub commission_bps: f64,
    /// Slippage estimate in basis points
    pub slippage_bps: f64,
    /// Minimum commission per fill (cents)
    pub min_commission: i64,
}

impl CostModel {
    /// A zero-cost model (no fees, no slippage).
    pub fn zero() -> Self {
        Self {
            commission_bps: 0.0,
            slippage_bps: 0.0,
            min_commission: 0,
        }
    }

    /// Compute commission for a trade with the given absolute notional value (cents).
    ///
    /// The notional should be `|quantity * price|`. Returns the cost in cents,
    /// which is always non-negative.
    pub fn compute_cost(&self, notional: i64) -> i64 {
        let notional = notional.unsigned_abs() as f64;
        let bps_cost = (notional * self.commission_bps / 10_000.0).round() as i64;
        bps_cost.max(self.min_commission)
    }
}

impl Default for CostModel {
    fn default() -> Self {
        Self::zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_cost() {
        let model = CostModel::zero();
        assert_eq!(model.compute_cost(1_000_000), 0);
    }

    #[test]
    fn bps_cost() {
        let model = CostModel {
            commission_bps: 10.0,
            slippage_bps: 5.0,
            min_commission: 0,
        };
        // ADR-0003: slippage_bps removed from compute_cost; now price impact in execute_fill.
        // Old formula: 1500 (15 bps), new: 1000 (commission-only 10 bps).
        assert_eq!(model.compute_cost(1_000_000), 1000);
    }

    #[test]
    fn min_fee_applied() {
        let model = CostModel {
            commission_bps: 1.0,
            slippage_bps: 0.0,
            min_commission: 1_00, // $1 minimum
        };
        // 1 bps on 10_000 cents ($100) = 1 cent, but min is $1.00
        assert_eq!(model.compute_cost(10_000), 1_00);
    }

    #[test]
    fn negative_notional_uses_abs() {
        let model = CostModel {
            commission_bps: 10.0,
            slippage_bps: 0.0,
            min_commission: 0,
        };
        assert_eq!(
            model.compute_cost(-1_000_000),
            model.compute_cost(1_000_000)
        );
    }

    #[test]
    fn cost_always_non_negative() {
        let model = CostModel::zero();
        assert!(model.compute_cost(0) >= 0);
        assert!(model.compute_cost(-100) >= 0);
    }

    #[test]
    fn parity_no_slippage_matches_old_formula() {
        // With slippage_bps = 0.0, the new compute_cost MUST equal the old
        // (commission_bps + 0) x notional / 1e4 floored by min_commission.
        let model = CostModel {
            commission_bps: 12.5,
            slippage_bps: 0.0,
            min_commission: 50,
        };
        assert_eq!(model.compute_cost(1_000_000), 1250); // 12.5 bps x $10,000 = $1.25 > $0.50 floor
        assert_eq!(model.compute_cost(10_000), 50); // 12.5 bps x $1.00 = $0.0125 < $0.50 floor
    }
}
