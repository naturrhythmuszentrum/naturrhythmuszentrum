// SPDX-License-Identifier: AGPL-3.0-only
//! Positive-base powers retain their selected route and every component premise.

mod check;
mod produce;
mod wire;

pub(super) use check::check;
pub(super) use produce::produce;
pub(super) use wire::{decode, encode};

use super::{AT, Interval};
use crate::budget::{Budget, BudgetKind};
use crate::{Fault, InvalidCode, Rational};

/// The closed route selected from a canonical reduced rational exponent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerRoute {
    /// Integral exponentiation by squaring, including checked exponent zero.
    Integer,
    /// Repeated square roots followed by integral exponentiation.
    Dyadic,
    /// Checked logarithm, rational scaling and exponential.
    LogExp,
}

/// Component evidence for one original positive-power node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PowerEvidence {
    /// Original postorder record index.
    pub record: u32,
    /// Proposed route, independently reconstructed from the original exponent.
    pub route: PowerRoute,
    /// Successive root bounds for the dyadic route, absent on other routes.
    pub roots: Vec<Interval>,
    /// Checked logarithm bound for the analytic route, absent on other routes.
    pub logarithm: Option<Interval>,
}

/// Bound and component instances produced without granting checked authority.
pub(super) struct Produced {
    /// Proposed result.
    pub bound: Interval,
    /// Route and intermediate endpoints.
    pub evidence: PowerEvidence,
    /// Logarithm premise for the analytic route.
    pub log: Option<super::LogEvidence>,
    /// Exponential premise for the analytic route.
    pub exp: Option<super::ExpEvidence>,
}

/// Reduced exponent components with bounded magnitude and root repetition.
pub(super) fn exponent(value: &Rational, budget: &mut Budget) -> Result<(i64, u64), Fault> {
    let (p, d) = value.small_ratio(budget)?.ok_or_else(invalid)?;
    budget.check(BudgetKind::IntegerExponent, p.unsigned_abs(), AT)?;
    if d.is_power_of_two() {
        budget.check(
            BudgetKind::IntegerExponent,
            u64::from(d.trailing_zeros()),
            AT,
        )?;
    }
    Ok((p, d))
}

/// Account for all route records before copying or retaining them.
pub(super) fn bytes(records: &[PowerEvidence]) -> u64 {
    records.iter().fold(0_u64, |total, record| {
        record.roots.iter().fold(
            total
                .saturating_add(64)
                .saturating_add(record.logarithm.as_ref().map_or(0, super::interval_bytes)),
            |size, value| size.saturating_add(super::interval_bytes(value)),
        )
    })
}

/// Unauthenticated route, bound or original-domain mismatch.
pub(super) const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, AT)
}
