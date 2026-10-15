// SPDX-License-Identifier: AGPL-3.0-only
//! Exact object access and scoped evidence for elementary solving.

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle};
use crate::proof_model::{ApplicationDraft, CheckedApplication, ScopedConditions};
use crate::solving::{CoordinateSpaceData, FamilyMapData, SolveDraft, SolveProblemData, TupleData};

/// Elementary replay uses one stable diagnostic location.
pub(crate) const AT: Location = Location::new("elementary solving");

/// Invalid evidence is never a candidate theorem.
pub(crate) const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Rule, AT)
}

/// Borrow an original problem, retaining its predicate and denominator vector.
pub(crate) fn problem<'a>(
    cas: &'a Cas,
    handle: &Handle,
) -> Result<&'a SolveProblemData<Handle>, Fault> {
    match crate::solving::graph::object(&cas.state, cas.checked(handle)?)? {
        SolveDraft::SolveProblem(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// Borrow the complete ordered coordinate product.
pub(crate) fn coordinates<'a>(
    cas: &'a Cas,
    handle: &Handle,
) -> Result<&'a CoordinateSpaceData<Handle>, Fault> {
    match crate::solving::graph::object(&cas.state, cas.checked(handle)?)? {
        SolveDraft::CoordinateSpace(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// Borrow a tuple without treating admission as membership.
pub(crate) fn tuple<'a>(cas: &'a Cas, handle: &Handle) -> Result<&'a TupleData<Handle>, Fault> {
    match crate::solving::graph::object(&cas.state, cas.checked(handle)?)? {
        SolveDraft::Tuple(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// Borrow a map without assuming any inverse or image theorem.
pub(crate) fn family<'a>(
    cas: &'a Cas,
    handle: &Handle,
) -> Result<&'a FamilyMapData<Handle>, Fault> {
    match crate::solving::graph::object(&cas.state, cas.checked(handle)?)? {
        SolveDraft::FamilyMap(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// A premise slot always indexes the application's actual ordered earlier premises.
pub(crate) fn premise(
    premises: &[CheckedApplication],
    slot: u32,
) -> Result<&CheckedApplication, Fault> {
    premises
        .get(usize::try_from(slot).map_err(|_| invalid())?)
        .ok_or_else(invalid)
}

/// Charge every identity and preserve all residuals and used hypotheses.
pub(crate) fn conditions(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    budget.solving(BudgetKind::SolveIdentities, 1, AT)?;
    crate::solving::coefficients::proof::conditions(cas, draft, premises, budget)
}

/// Require a coefficient region in the actual assertion context and original product.
pub(crate) fn region(
    cas: &Cas,
    context: &Handle,
    problem: &SolveProblemData<Handle>,
    region: &Handle,
) -> Result<(), Fault> {
    let frame = cas.state.context(cas.checked(context)?)?;
    let domain = cas.state.region(cas.checked(region)?)?;
    if (
        frame.header_id,
        frame.admissible,
        domain.header,
        &domain.scope,
        &domain.ambient,
    ) != (
        problem.header.logical_id(),
        region.logical_id(),
        problem.header.logical_id(),
        &problem.scope,
        &crate::domain_model::AmbientDomain::Product {
            coordinates: problem.coefficients.clone(),
        },
    ) {
        return Err(invalid());
    }
    Ok(())
}
