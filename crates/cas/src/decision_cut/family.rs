// SPDX-License-Identifier: AGPL-3.0-only
//! Checked unique floor projection over an explicitly declared Integer coordinate.

mod conjunction;
mod parameters;

use super::{evaluate, plan};
use crate::budget::Budget;
use crate::decision::{CaseCut, CoordinateInterval, IntegerFamily};
use crate::domain_model::AmbientDomain;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::number::Rational;
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, ScopedConditions, Statement,
};

/// The projected theorem is distinct from box inclusion and ordinary finite partitions.
const AT: Location = Location::new("integer floor family");

/// A changed family contract cannot borrow the floor theorem.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::DecisionIntegerRange, AT)
}

/// Validate exact typed roles even when the statement is only an explicit hypothesis.
pub(crate) fn roles(cas: &Cas, family: &IntegerFamily) -> Result<(), Fault> {
    let function = cas.state.expression(cas.checked(&family.function)?)?;
    let domain = cas.state.region(cas.checked(&family.domain)?)?;
    let integer = Sort::Value(AtomicSort::Integer);
    if function.sort != integer
        || !function.scope.is_empty()
        || domain.header != function.header
        || !domain.scope.is_empty()
        || domain.ambient != AmbientDomain::Assignments
    {
        return Err(invalid());
    }
    coordinate(
        cas,
        &family.coordinate,
        function.header,
        &Sort::Value(AtomicSort::Scalar(Field::Real)),
    )?;
    coordinate(cas, &family.index, function.header, &integer)?;
    for bound in [&family.lower, &family.upper].into_iter().flatten() {
        let value = cas.state.expression(cas.checked(bound)?)?;
        if value.header != function.header || value.sort != integer || !value.scope.is_empty() {
            return Err(invalid());
        }
    }
    if let Some(count) = &family.count
        && count.coordinate != family.coordinate
    {
        return Err(invalid());
    }
    Ok(())
}

/// Both binders refer to actual closed declarations, never an implicit index.
fn coordinate(cas: &Cas, handle: &Handle, header: LogicalId, sort: &Sort) -> Result<(), Fault> {
    let value = cas.state.expression(cas.checked(handle)?)?;
    if value.header != header
        || value.sort != *sort
        || !value.scope.is_empty()
        || value.binding.is_some()
        || value.space.is_some()
        || !matches!(value.operator, Operator::Free(_))
    {
        return Err(invalid());
    }
    Ok(())
}

/// Apply the closed floor lemma after reconstructing both complete region formulas.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let Statement::IntegerFamily(family) = &draft.conclusion else {
        return Err(invalid());
    };
    if draft.certificate != Certificate::None || !draft.substitutions.is_empty() {
        return Err(invalid());
    }
    roles(cas, family)?;
    let frame = cas.state.context(cas.checked(&draft.context)?)?;
    if frame.admissible != cas.checked(&family.domain)? {
        return Err(invalid());
    }
    if family.count.is_some() || !premises.is_empty() {
        return parameters::check(cas, draft, family, premises, budget);
    }
    budget.with_retained_payload(4_096, AT, |budget| reconstruct(cas, family, budget))?;
    Ok(ScopedConditions::default())
}

/// The exact native case has one association returning its own bound Integer coordinate.
fn native(cas: &Cas, family: &IntegerFamily, budget: &mut Budget) -> Result<CaseCut, Fault> {
    budget.work(8, AT)?;
    let function = cas.state.expression(cas.checked(&family.function)?)?;
    let [association] = function.arms.as_slice() else {
        return Err(invalid());
    };
    let arm = cas.state.arm(*association)?;
    if !arm.placements.is_empty()
        || !arm.scope.is_empty()
        || arm.value != cas.checked(&family.index)?
        || function.domain != Some(arm.region)
    {
        return Err(invalid());
    }
    let bounds = [&family.coordinate, &family.index]
        .into_iter()
        .map(|coordinate| CoordinateInterval {
            coordinate: coordinate.clone(),
            lower: Rational::zero(),
            upper: Rational::zero(),
        })
        .collect();
    Ok(CaseCut {
        function: family.function.clone(),
        association: cas.handle(*association),
        bounds,
        integer: None,
    })
}

/// Literal bounds prove nonemptiness without enumeration or numerical endpoint conversion.
fn endpoint<'a>(
    cas: &'a Cas,
    handle: Option<&Handle>,
    budget: &mut Budget,
) -> Result<Option<&'a Rational>, Fault> {
    let Some(handle) = handle else {
        return Ok(None);
    };
    budget.work(1, AT)?;
    let value = cas.state.expression(cas.checked(handle)?)?;
    let Operator::Rational(number) = &value.operator else {
        return Err(super::unsupported());
    };
    number.check(budget, AT)?;
    if !number.is_integer() {
        return Err(invalid());
    }
    Ok(Some(number))
}

/// Exact coefficient identities establish the universal relation rather than sample membership.
fn reconstruct(cas: &Cas, family: &IntegerFamily, budget: &mut Budget) -> Result<(), Fault> {
    let cut = native(cas, family, budget)?;
    let lower = endpoint(cas, family.lower.as_ref(), budget)?;
    let upper = endpoint(cas, family.upper.as_ref(), budget)?;
    if let (Some(lower), Some(upper)) = (lower, upper)
        && lower.compare(upper, budget)? != std::cmp::Ordering::Less
    {
        return Err(invalid());
    }
    let (header, relation) = super::identities(cas, &cut, budget)?;
    super::validate_box(cas, &cut, header, super::Coordinates::Mixed, budget)?;
    super::axes(
        cas,
        &cut,
        header,
        relation[0],
        super::Coordinates::Mixed,
        budget,
    )?;
    let domain = cas.checked(&family.domain)?;
    let roots = [relation[0], domain];
    let order = plan::order(cas, &roots, budget)?;
    let scratch = size(order.len(), AT)?.saturating_mul(256);
    budget.with_retained_payload(scratch, AT, |budget| {
        let values = evaluate::build(cas, &cut, header, domain, &order, budget)?;
        let endpoint_bytes = lower
            .into_iter()
            .chain(upper)
            .map(super::number_bytes)
            .fold(0_u64, u64::saturating_add);
        let bytes = evaluate::payload(&values, budget)?
            .saturating_add(endpoint_bytes.saturating_mul(16))
            .saturating_add(super::temporary(cut.bounds.len(), budget)?)
            .saturating_add(8_192);
        budget.with_retained_payload(bytes, AT, |budget| {
            let expected = conjunction::expected(lower, upper, true, budget)?;
            conjunction::check(cas, relation[0], &expected, &values, budget)?;
            drop(expected);
            let expected = conjunction::expected(lower, upper, false, budget)?;
            conjunction::check(cas, domain, &expected, &values, budget)
        })
    })
}

/// Selection checks its candidate against the same endpoint bindings used by the retained cover.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the selection consumer shares exact parameter-range checking"
)]
pub(crate) fn contains(
    cas: &Cas,
    draft: &ApplicationDraft,
    family: &IntegerFamily,
    cover: &CheckedApplication,
    value: &Rational,
    budget: &mut Budget,
) -> Result<(), Fault> {
    parameters::contains(cas, draft, family, cover, value, budget)
}
