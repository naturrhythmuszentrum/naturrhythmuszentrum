// SPDX-License-Identifier: AGPL-3.0-only
//! Exact rational cells and scalar leaves used by untrusted boundary proposals.

use crate::budget::Budget;
use crate::case_syntax::{self, invalid};
use crate::domain_model::{BoundaryDraft, RegionExpr};
use crate::error::Fault;
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, Reference};
use crate::model::{AtomicSort, Field, Operator, PredicateKind, Sort};
use crate::number::Rational;

/// Construct a scalar under the region's exact header and scope.
pub(crate) fn term(
    cas: &mut Cas,
    domain: &Handle,
    operator: Operator,
    operands: &[Handle],
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let region = cas.state.region(cas.checked(domain)?)?;
    let draft = ExpressionDraft {
        space: None,
        header: Reference::Existing(cas.handle(region.header)),
        scope: region.scope.clone(),
        sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
        operator,
        binding: None,
        operands: operands.iter().cloned().map(Reference::Existing).collect(),
    };
    case_syntax::admit(cas, Draft::Expression(draft), budget)
}

/// Retain an exact rational point with no attached ownership.
pub(crate) fn boundary(
    cas: &mut Cas,
    domain: &Handle,
    value: &Rational,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let location = term(cas, domain, Operator::Rational(value.clone()), &[], budget)?;
    let region = cas.state.region(cas.checked(domain)?)?;
    let draft = BoundaryDraft {
        header: Reference::Existing(cas.handle(region.header)),
        scope: region.scope.clone(),
        ambient: region.ambient.clone(),
        location: Reference::Existing(location),
    };
    case_syntax::admit(cas, Draft::Boundary(draft), budget)
}

/// Build each exact open interval or singleton, retaining the requested-region intersection.
pub(crate) fn cell(
    cas: &mut Cas,
    domain: &Handle,
    key: &str,
    cuts: &[Rational],
    index: usize,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let coordinate = term(cas, domain, Operator::Free(key.into()), &[], budget)?;
    if index & 1 == 1 {
        let point = cuts.get(index / 2).ok_or_else(invalid)?;
        let point = term(cas, domain, Operator::Rational(point.clone()), &[], budget)?;
        return guard(
            cas,
            domain,
            PredicateKind::Equal,
            &[coordinate, point],
            budget,
        );
    }
    let position = index / 2;
    let mut members = vec![domain.logical_id()];
    if let Some(lower) = position.checked_sub(1).and_then(|slot| cuts.get(slot)) {
        let lower = term(cas, domain, Operator::Rational(lower.clone()), &[], budget)?;
        let difference = term(
            cas,
            domain,
            Operator::Sub,
            &[coordinate.clone(), lower],
            budget,
        )?;
        members
            .push(guard(cas, domain, PredicateKind::Positive, &[difference], budget)?.logical_id());
    }
    if let Some(upper) = cuts.get(position) {
        let upper = term(cas, domain, Operator::Rational(upper.clone()), &[], budget)?;
        let difference = term(cas, domain, Operator::Sub, &[upper, coordinate], budget)?;
        members
            .push(guard(cas, domain, PredicateKind::Positive, &[difference], budget)?.logical_id());
    }
    let result = case_syntax::meet(cas, domain.logical_id(), &members, budget)?;
    Ok(cas.handle(result))
}

/// Exact defined-and-true membership, with no implicit complement rule.
pub(crate) fn guard(
    cas: &mut Cas,
    domain: &Handle,
    kind: PredicateKind,
    arguments: &[Handle],
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let region = cas.state.region(cas.checked(domain)?)?;
    let draft = Draft::Condition {
        header: Reference::Existing(cas.handle(region.header)),
        scope: region.scope.clone(),
        kind,
        arguments: arguments.iter().cloned().map(Reference::Existing).collect(),
    };
    let predicate = case_syntax::admit(cas, draft, budget)?;
    let result = case_syntax::guarded(cas, domain.logical_id(), predicate.logical_id(), budget)?;
    Ok(cas.handle(result))
}

/// An exact empty region can represent the derivative-defined set of an empty map.
pub(crate) fn union(
    cas: &mut Cas,
    basis: &Handle,
    members: &[Handle],
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let expression = RegionExpr::Union(members.iter().cloned().map(Reference::Existing).collect());
    let id = case_syntax::region(cas, basis.logical_id(), expression, budget)?;
    Ok(cas.handle(id))
}
