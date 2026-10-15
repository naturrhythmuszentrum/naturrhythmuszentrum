// SPDX-License-Identifier: AGPL-3.0-only
//! Finite closed covers and exact integer ranges reconstructed from case geometry.

use crate::budget::{Budget, BudgetKind};
use crate::case_syntax::{self, invalid};
use crate::domain_model::{AmbientDomain, RegionExpr};
use crate::error::{Fault, Location};
use crate::graph::{Cas, Handle, size};
use crate::model::{AtomicSort, Field, Operator, PredicateKind, Sort};
use crate::number::Rational;
use crate::split_model::{
    SplitCertificate, SplitKind, SplitPiece, SplitPoint, SplitRange, SplitValue,
};
use std::cmp::Ordering;

/// Shared finite-cover accounting location.
pub(crate) const AT: Location = Location::new("finite split calculus");

/// Validate the named real coordinate, exact finite bounds and scalar function.
pub(crate) fn validate(cas: &Cas, value: &SplitValue, budget: &mut Budget) -> Result<(), Fault> {
    let function = cas.state.expression(cas.checked(&value.function)?)?;
    let domain = cas.state.region(cas.checked(&value.domain)?)?;
    if function.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
        || !function.scope.is_empty()
        || function.space.is_some()
        || !domain.scope.is_empty()
        || domain.ambient != AmbientDomain::Real(value.range.coordinate.clone())
    {
        return Err(invalid());
    }
    for number in [&value.range.lower, &value.range.upper, &value.value] {
        number.check(budget, AT)?;
    }
    if value.kind == SplitKind::Sum
        && (!value.range.lower.is_integer()
            || !value.range.upper.is_integer()
            || value.range.lower.compare(&value.range.upper, budget)? == Ordering::Greater)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Build the full closed interval from the ambient axis, never from a restricted context.
pub(crate) fn domain(
    cas: &mut Cas,
    basis: &Handle,
    range: &SplitRange,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let ambient = case_syntax::region(cas, cas.checked(basis)?, RegionExpr::Ambient, budget)?;
    let ambient = cas.handle(ambient);
    let coordinate = crate::boundary_regions::term(
        cas,
        &ambient,
        Operator::Free(range.coordinate.clone()),
        &[],
        budget,
    )?;
    let (lower, upper) = ordered(range, budget)?;
    let low = crate::boundary_regions::term(cas, &ambient, Operator::Rational(lower), &[], budget)?;
    let high =
        crate::boundary_regions::term(cas, &ambient, Operator::Rational(upper), &[], budget)?;
    let start = crate::boundary_regions::term(
        cas,
        &ambient,
        Operator::Sub,
        &[coordinate.clone(), low],
        budget,
    )?;
    let end =
        crate::boundary_regions::term(cas, &ambient, Operator::Sub, &[high, coordinate], budget)?;
    let first = crate::boundary_regions::guard(
        cas,
        &ambient,
        PredicateKind::NonNegative,
        &[start],
        budget,
    )?;
    crate::boundary_regions::guard(cas, &first, PredicateKind::NonNegative, &[end], budget)
}

/// Sort bounds for geometry while the statement retains their original orientation.
pub(crate) fn ordered(
    range: &SplitRange,
    budget: &mut Budget,
) -> Result<(Rational, Rational), Fault> {
    if range.lower.compare(&range.upper, budget)? == Ordering::Greater {
        Ok((range.upper.clone(), range.lower.clone()))
    } else {
        Ok((range.lower.clone(), range.upper.clone()))
    }
}

/// Reconstruct every occupied cell and finite-point value before calculating any endpoint.
pub(crate) fn reconstruct(
    cas: &mut Cas,
    value: &SplitValue,
    budget: &mut Budget,
) -> Result<SplitCertificate, Fault> {
    validate(cas, value, budget)?;
    let expected = domain(cas, &value.domain, &value.range, budget)?;
    if expected != value.domain {
        return Err(invalid());
    }
    crate::boundary_partition::inventory(cas, &value.function, &value.domain, budget)?;
    let root = cas.checked(&value.function)?;
    let within = cas.checked(&value.domain)?;
    let analysis = crate::case_cells::analyze(cas, root, root, within, budget)?;
    crate::region_check::measure(&analysis.cells, budget)?;
    let row = analysis
        .regions
        .iter()
        .position(|id| *id == within)
        .ok_or_else(invalid)?;
    let membership = analysis.cells.membership.get(row).ok_or_else(invalid)?;
    let mut result = SplitCertificate {
        partition: analysis.cells.clone(),
        pieces: Vec::new(),
        points: Vec::new(),
    };
    for (cell, active) in membership.iter().enumerate() {
        if !active {
            continue;
        }
        budget.work(1, AT)?;
        let polynomial =
            analysis.calculus_cell(cas, root, within, cell, &value.range.coordinate, budget)?;
        append(&mut result, value.kind, cell, polynomial, budget)?;
    }
    Ok(result)
}

/// Finite excluded points are explicit; an undefined open cell is an integrability blocker.
fn append(
    result: &mut SplitCertificate,
    kind: SplitKind,
    cell: usize,
    polynomial: Option<Vec<Rational>>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if kind == SplitKind::Integral && cell & 1 == 1 {
        let point = result
            .partition
            .cuts
            .get(cell / 2)
            .ok_or_else(invalid)?
            .clone();
        let value = polynomial.map(|values| values.first().cloned().unwrap_or_else(Rational::zero));
        budget.specialized(
            BudgetKind::BoundaryNodes,
            size(result.points.len().saturating_add(1), AT)?,
            false,
            AT,
        )?;
        result
            .points
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        result.points.push(SplitPoint { point, value });
        return Ok(());
    }
    let Some((lower, upper)) = bounds(&result.partition.cuts, cell, kind, budget)? else {
        return Ok(());
    };
    let polynomial = polynomial.ok_or_else(crate::boundary_producer::pending)?;
    budget.specialized(
        BudgetKind::CaseArms,
        size(result.pieces.len().saturating_add(1), AT)?,
        false,
        AT,
    )?;
    result
        .pieces
        .try_reserve(1)
        .map_err(|_| Fault::allocation(AT))?;
    result.pieces.push(SplitPiece {
        cell: u32::try_from(cell).map_err(|_| invalid())?,
        lower,
        upper,
        polynomial,
        primitive: Vec::new(),
        start: Rational::zero(),
        end: Rational::zero(),
    });
    Ok(())
}

/// Open real cells induce disjoint inclusive integer ranges, including noninteger cuts.
fn bounds(
    cuts: &[Rational],
    cell: usize,
    kind: SplitKind,
    budget: &mut Budget,
) -> Result<Option<(Rational, Rational)>, Fault> {
    if cell & 1 == 1 {
        let point = cuts.get(cell / 2).ok_or_else(invalid)?;
        return Ok(point.is_integer().then(|| (point.clone(), point.clone())));
    }
    let index = cell / 2;
    let mut lower = cuts
        .get(index.checked_sub(1).ok_or_else(invalid)?)
        .ok_or_else(invalid)?
        .clone();
    let mut upper = cuts.get(index).ok_or_else(invalid)?.clone();
    if kind == SplitKind::Sum {
        lower = lower.floor(budget)?.add(&Rational::one(), budget)?;
        upper = upper
            .negated(budget)?
            .floor(budget)?
            .negated(budget)?
            .sub(&Rational::one(), budget)?;
    }
    Ok((lower.compare(&upper, budget)? != Ordering::Greater).then_some((lower, upper)))
}
