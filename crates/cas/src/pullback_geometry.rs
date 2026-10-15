// SPDX-License-Identifier: AGPL-3.0-only
//! Exact interval maps and support containment, using the shared partition cache.

use crate::budget::Budget;
use crate::case_syntax::invalid;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle};
use crate::number::Rational;
use crate::pullback_model::PullbackValue;

/// No endpoint represents a signed infinity inside exact scalar syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Interval {
    /// Absent lower endpoint denotes negative infinity.
    pub(crate) lower: Option<Rational>,
    /// Absent upper endpoint denotes positive infinity.
    pub(crate) upper: Option<Rational>,
}

/// Exact map accounting location.
pub(crate) const AT: Location = Location::new("delta pullback map");

/// Reconstruct the connected open interval without trusting the space constructor alone.
pub(crate) fn interval(cas: &Cas, space: &Handle, budget: &mut Budget) -> Result<Interval, Fault> {
    let domain = cas.state.space(cas.checked(space)?)?.domain;
    let cells = crate::region_cache::partition(cas, &[domain], budget)?;
    crate::region::check_open(&cells)?;
    let row = cells.membership.first().ok_or_else(invalid)?;
    let first = row.iter().position(|value| *value).ok_or_else(invalid)? / 2;
    let last = row.iter().rposition(|value| *value).ok_or_else(invalid)? / 2;
    Ok(Interval {
        lower: first
            .checked_sub(1)
            .and_then(|index| cells.cuts.get(index))
            .cloned(),
        upper: cells.cuts.get(last).cloned(),
    })
}

/// Both map and inverse must preserve the entire declared open intervals and compact test supports.
pub(crate) fn affine(
    cas: &Cas,
    claim: &PullbackValue,
    coefficients: &[Rational],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let [offset, scale] = coefficients else {
        return Err(map_error());
    };
    if scale.is_zero() {
        return Err(map_error());
    }
    let source = interval(cas, &claim.source_space, budget)?;
    let target = interval(cas, &claim.target_space, budget)?;
    let (lower, upper) = if scale.is_negative() {
        (&target.upper, &target.lower)
    } else {
        (&target.lower, &target.upper)
    };
    let image = Interval {
        lower: mapped(lower.as_ref(), scale, offset, budget)?,
        upper: mapped(upper.as_ref(), scale, offset, budget)?,
    };
    if image != source {
        return Err(map_error());
    }
    let inverse = claim.inverse.as_ref().ok_or_else(map_error)?;
    let actual =
        crate::pullback_polynomial::coefficients(cas, inverse, &claim.source_space, budget)?;
    let expected = vec![
        offset.negated(budget)?.div(scale, budget)?,
        Rational::one().div(scale, budget)?,
    ];
    if actual != expected {
        return Err(map_error());
    }
    Ok(())
}

/// Map a finite endpoint, retaining the appropriate unbounded endpoint otherwise.
fn mapped(
    point: Option<&Rational>,
    scale: &Rational,
    offset: &Rational,
    budget: &mut Budget,
) -> Result<Option<Rational>, Fault> {
    point
        .map(|point| point.mul(scale, budget)?.add(offset, budget))
        .transpose()
}

/// Decide strict interior containment, including exclusion of finite interval endpoints.
pub(crate) fn contains(
    interval: &Interval,
    point: &Rational,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    if let Some(lower) = &interval.lower
        && !point.compare(lower, budget)?.is_gt()
    {
        return Ok(false);
    }
    if let Some(upper) = &interval.upper
        && !point.compare(upper, budget)?.is_lt()
    {
        return Ok(false);
    }
    Ok(true)
}

/// A failed coordinate or interval map is a refusal of this proposed pullback.
pub(crate) const fn map_error() -> Fault {
    Fault::invalid(InvalidCode::PullbackMapMismatch, AT)
}
