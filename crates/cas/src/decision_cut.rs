// SPDX-License-Identifier: AGPL-3.0-only
//! Pure rational cut checking over the actual native region syntax.

mod affine;
mod evaluate;
pub(crate) mod family;
pub(crate) mod partition;
pub(crate) mod plan;

use std::cmp::Ordering;

use crate::budget::{Budget, BudgetKind};
use crate::decision::{CaseCut, CoordinateInterval, IntegerCellRange};
use crate::domain_model::AmbientDomain;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, LogicalId, size};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::number::Rational;

/// Stable diagnostic location for exact inclusion checks.
const AT: Location = Location::new("exact case box cut");

/// Finite partition grids remain Real; direct cuts also admit exact Integer axes.
#[derive(Clone, Copy)]
enum Coordinates {
    /// A continuous Real cell decomposition.
    Real,
    /// A closed box with integral endpoints on every Integer coordinate.
    Mixed,
}

/// No unsupported grammar acquires authority from its name or representation.
const fn unsupported() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedCertificate, AT)
}

/// A malformed box is distinct from an undecidable cut.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::DecisionBox, AT)
}

/// Reconstruct both required region inclusions without creating a fact or consulting a cache.
pub(crate) fn check(cas: &Cas, cut: &CaseCut, budget: &mut Budget) -> Result<(), Fault> {
    let (header, roots) = identities(cas, cut, budget)?;
    let bytes = input_payload(cut, budget)?;
    budget.with_retained_payload(bytes, AT, |budget| {
        validate_box(cas, cut, header, Coordinates::Mixed, budget)?;
        axes(cas, cut, header, roots[0], Coordinates::Mixed, budget)?;
        let order = plan::order(cas, &roots, budget)?;
        let bytes = size(order.len(), AT)?
            .saturating_mul(256)
            .saturating_add(128);
        budget.with_retained_payload(bytes, AT, |budget| {
            evaluate::check(cas, cut, header, &roots, &order, budget)?;
            if let Some(range) = &cut.integer {
                budget.with_retained_payload(temporary(0, budget)?, AT, |budget| {
                    integer(cas, cut, range, budget)
                })?;
            }
            Ok(())
        })
    })
}

/// The option is an actual association, never a caller-supplied replacement region.
fn identities(
    cas: &Cas,
    cut: &CaseCut,
    budget: &mut Budget,
) -> Result<(LogicalId, [LogicalId; 2]), Fault> {
    let function = cas.state.expression(cas.checked(&cut.function)?)?;
    let association = cas.checked(&cut.association)?;
    budget.work(size(function.arms.len(), AT)?.saturating_add(1), AT)?;
    if !function.arms.contains(&association) {
        return Err(Fault::invalid(InvalidCode::DecisionOptionMismatch, AT));
    }
    let arm = cas.state.arm(association)?;
    if !arm.placements.is_empty() {
        return Err(unsupported());
    }
    let domain = function.domain.ok_or_else(unsupported)?;
    if !function.scope.is_empty() || !arm.scope.is_empty() || function.header != arm.header {
        return Err(Fault::invalid(InvalidCode::Scope, AT));
    }
    let ambient = &cas.state.region(domain)?.ambient;
    if !matches!(ambient, AmbientDomain::Real(_) | AmbientDomain::Assignments) {
        return Err(unsupported());
    }
    Ok((function.header, [domain, arm.region]))
}

/// Reserve the retained input and endpoint-comparison copies before any rational replay.
fn input_payload(cut: &CaseCut, budget: &mut Budget) -> Result<u64, Fault> {
    if cut.bounds.is_empty() {
        return Err(invalid());
    }
    budget.check(BudgetKind::ConditionAtoms, size(cut.bounds.len(), AT)?, AT)?;
    let mut bytes = 128_u64;
    for interval in &cut.bounds {
        budget.work(1, AT)?;
        bytes = bytes
            .saturating_add(64)
            .saturating_add(number_bytes(&interval.lower))
            .saturating_add(number_bytes(&interval.upper));
    }
    if let Some(range) = &cut.integer {
        for bound in [range.lower.as_ref(), range.upper.as_ref()]
            .into_iter()
            .flatten()
        {
            budget.work(1, AT)?;
            bytes = bytes.saturating_add(number_bytes(bound));
        }
    }
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    Ok(bytes)
}

/// Check exact source identities and endpoint order inside the prepaid input reservation.
fn validate_box(
    cas: &Cas,
    cut: &CaseCut,
    header: LogicalId,
    coordinates: Coordinates,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for (index, interval) in cut.bounds.iter().enumerate() {
        budget.work(size(index, AT)?.saturating_add(1), AT)?;
        let coordinate = cas.checked(&interval.coordinate)?;
        let value = cas.state.expression(coordinate)?;
        let real = value.sort == Sort::Value(AtomicSort::Scalar(Field::Real));
        let integer = matches!(coordinates, Coordinates::Mixed)
            && value.sort == Sort::Value(AtomicSort::Integer);
        let closed = value.scope.is_empty() && value.binding.is_none() && value.space.is_none();
        if value.header != header
            || !(real || integer)
            || !closed
            || !matches!(value.operator, Operator::Free(_))
        {
            return Err(Fault::invalid(InvalidCode::Scope, AT));
        }
        if integer {
            integer_endpoints(interval, budget)?;
        }
        let duplicate = cut.bounds[..index]
            .iter()
            .any(|other| other.coordinate == interval.coordinate);
        let reversed = interval.lower.compare(&interval.upper, budget)? == Ordering::Greater;
        if duplicate || reversed {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Integral ordered endpoints ensure that every admitted Integer box is nonempty.
fn integer_endpoints(interval: &CoordinateInterval, budget: &mut Budget) -> Result<(), Fault> {
    for endpoint in [&interval.lower, &interval.upper] {
        budget.work(1, AT)?;
        endpoint.check(budget, AT)?;
        if !endpoint.is_integer() {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Account for bigint allocation overhead without formatting or copying a rational.
fn number_bytes(value: &Rational) -> u64 {
    let (numerator, denominator) = value.component_bits();
    numerator
        .div_ceil(8)
        .saturating_add(denominator.div_ceil(8))
        .saturating_mul(4)
        .saturating_add(128)
}

/// Prepay one new dense vector, its map slot and eight simultaneous scalar temporaries.
fn temporary(dimensions: usize, budget: &Budget) -> Result<u64, Fault> {
    let component = budget
        .limits()
        .component_bits
        .div_ceil(8)
        .saturating_mul(8)
        .saturating_add(128);
    let bytes = size(dimensions, AT)?
        .saturating_add(9)
        .saturating_mul(component)
        .saturating_add(1_024);
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    Ok(bytes)
}

/// Integer-option data remains explicit and exact, including absent range endpoints.
fn integer(
    cas: &Cas,
    cut: &CaseCut,
    range: &IntegerCellRange,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let arm = cas.state.arm(cas.checked(&cut.association)?)?;
    let value = cas.state.expression(arm.value)?;
    let Operator::Rational(integer) = &value.operator else {
        return Err(unsupported());
    };
    if value.sort != Sort::Value(AtomicSort::Integer) || !integer.is_integer() {
        return Err(Fault::invalid(InvalidCode::DecisionIntegerRange, AT));
    }
    check_range(integer, range, budget)?;
    let interval = cut
        .bounds
        .iter()
        .find(|value| value.coordinate == range.coordinate)
        .ok_or_else(invalid)?;
    let end = integer.add(&Rational::one(), budget)?;
    let contains_lower = integer.compare(&interval.lower, budget)? != Ordering::Greater;
    let contains_upper = interval.upper.compare(&end, budget)? == Ordering::Less;
    if contains_lower && contains_upper {
        Ok(())
    } else {
        Err(Fault::invalid(
            InvalidCode::DecisionBoundaryIntersection,
            AT,
        ))
    }
}

/// Check integral endpoints and exact membership of the candidate option index.
fn check_range(
    integer: &Rational,
    range: &IntegerCellRange,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for bound in [range.lower.as_ref(), range.upper.as_ref()]
        .into_iter()
        .flatten()
    {
        bound.check(budget, AT)?;
        if !bound.is_integer() {
            return Err(Fault::invalid(InvalidCode::DecisionIntegerRange, AT));
        }
    }
    if let Some(lower) = &range.lower
        && integer.compare(lower, budget)? == Ordering::Less
    {
        return Err(Fault::invalid(InvalidCode::DecisionIntegerRange, AT));
    }
    if let Some(upper) = &range.upper
        && integer.compare(upper, budget)? != Ordering::Less
    {
        return Err(Fault::invalid(InvalidCode::DecisionIntegerRange, AT));
    }
    Ok(())
}

/// Exact closed-box extrema, with sign-directed endpoint choice for every affine coefficient.
fn bounds(
    coefficients: &[Rational],
    intervals: &[CoordinateInterval],
    budget: &mut Budget,
) -> Result<(Rational, Rational), Fault> {
    let first = coefficients.first().ok_or_else(unsupported)?;
    let mut lower = first.add(&Rational::zero(), budget)?;
    let mut upper = first.add(&Rational::zero(), budget)?;
    for (coefficient, interval) in coefficients.iter().skip(1).zip(intervals) {
        budget.work(1, AT)?;
        if coefficient.is_zero() {
            continue;
        }
        let (least, greatest) = if coefficient.is_negative() {
            (&interval.upper, &interval.lower)
        } else {
            (&interval.lower, &interval.upper)
        };
        lower = lower.add(&coefficient.mul(least, budget)?, budget)?;
        upper = upper.add(&coefficient.mul(greatest, budget)?, budget)?;
    }
    Ok((lower, upper))
}

/// Supplied axes exhaust the admitted numeric declarations; another axis is not a substitute.
fn axes(
    cas: &Cas,
    cut: &CaseCut,
    header: LogicalId,
    domain: LogicalId,
    coordinates: Coordinates,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let region = cas.state.region(domain)?;
    match &region.ambient {
        AmbientDomain::Real(key) => {
            if cut.bounds.len() != 1 || !contains_axis(cas, cut, key, budget)? {
                return Err(invalid());
            }
        }
        AmbientDomain::Assignments => {
            let declarations = &cas.state.header(header)?.environment.declarations;
            for declaration in declarations {
                budget.work(1, AT)?;
                let real = declaration.sort == Sort::Value(AtomicSort::Scalar(Field::Real));
                let integer = matches!(coordinates, Coordinates::Mixed)
                    && declaration.sort == Sort::Value(AtomicSort::Integer);
                if (real || integer) && !contains_axis(cas, cut, &declaration.key, budget)? {
                    return Err(invalid());
                }
            }
        }
        _ => return Err(unsupported()),
    }
    Ok(())
}

/// Charge complete declaration-name comparisons when matching the box's coordinate inventory.
fn contains_axis(cas: &Cas, cut: &CaseCut, key: &str, budget: &mut Budget) -> Result<bool, Fault> {
    for interval in &cut.bounds {
        budget.work(size(key.len(), AT)?.saturating_add(1), AT)?;
        let expression = cas.state.expression(cas.checked(&interval.coordinate)?)?;
        if matches!(&expression.operator, Operator::Free(name) if name == key) {
            return Ok(true);
        }
    }
    Ok(false)
}
