// SPDX-License-Identifier: AGPL-3.0-only
//! Named root-existence theorems with independently checked exact hypotheses.

use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::Field;
use crate::proof_model::{ApplicationDraft, CheckedApplication, Statement};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::proof::{defined, fault};
use crate::solving::coefficients::storage::{AT, invalid, polynomial};
use crate::solving::{
    AdjunctionMode, CertificateRootAdjunction, RootRelationData, RootSelectionData, SelectionMode,
    SolveDraft, SolveStatement,
};

/// The leading unit must name exactly the specialized polynomial's retained leading coefficient.
fn leading(
    cas: &Cas,
    relation: &RootRelationData<Handle>,
    slot: u32,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let stored = polynomial(&cas.state, &relation.polynomial)?;
    let first = stored.terms.first().ok_or_else(invalid)?;
    if stored.variables.len() != 1 || first.exponents.first().is_none_or(|degree| *degree == 0) {
        return Err(invalid());
    }
    let premise = premises
        .get(usize::try_from(slot).map_err(|_| invalid())?)
        .ok_or_else(invalid)?;
    let Statement::Solve(statement) = &premise.judgment.statement else {
        return Err(invalid());
    };
    let SolveStatement::Unit(unit) = statement.as_ref() else {
        return Err(invalid());
    };
    if (&unit.value, &unit.domain, &unit.region)
        != (&first.coefficient, &stored.domain, &relation.region)
    {
        return Err(invalid());
    }
    defined(
        cas,
        &relation.polynomial,
        &relation.region,
        premises,
        budget,
    )
}

/// Optional selection syntax must be exactly the single relation whose existence is being proved.
fn selection<'a>(
    cas: &'a Cas,
    certificate: &CertificateRootAdjunction,
    budget: &mut Budget,
) -> Result<Option<&'a RootSelectionData<Handle>>, Fault> {
    let Some(handle) = &certificate.selection else {
        return Ok(None);
    };
    budget.solving(BudgetKind::SelectionSlots, 1, AT)?;
    let SolveDraft::RootSelection(value) =
        crate::solving::graph::object(&cas.state, cas.checked(handle)?)?
    else {
        return Err(invalid());
    };
    if value.relations.as_slice() != std::slice::from_ref(&certificate.relation) {
        return Err(invalid());
    }
    Ok(Some(value))
}

/// Ordinary FTA and primitive-unity existence do not discharge a bare constraint or principal convention.
fn unrestricted(
    cas: &Cas,
    selected: Option<&RootSelectionData<Handle>>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if let Some(value) = selected {
        if value.mode != SelectionMode::Any {
            return Err(invalid());
        }
        super::selection::unconstrained(cas, value, budget)?;
    }
    Ok(())
}

/// Primitive unity uses the exact divisor-product definition of `Φ_n` and the named primitive-root theorem.
fn unity(
    cas: &Cas,
    relation: &RootRelationData<Handle>,
    order: u32,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if relation.ambient != Field::Complex {
        return Err(invalid());
    }
    let stored = polynomial(&cas.state, &relation.polynomial)?;
    let engine = Engine {
        state: &cas.state,
        domain: &stored.domain,
    };
    let expected = super::cyclotomic::polynomial(&engine, order, budget)?;
    let supplied = super::dense::Dense::read(&cas.state, &relation.polynomial, budget)
        .map_err(|error| fault(&error))?;
    if !supplied
        .equals(&expected, &cas.state, budget)
        .map_err(|error| fault(&error))?
    {
        return Err(invalid());
    }
    Ok(())
}

/// Each theorem subcase checks exactly its own ambient, order, sign and selection hypotheses.
fn theorem(
    cas: &Cas,
    certificate: &CertificateRootAdjunction,
    relation: &RootRelationData<Handle>,
    selected: Option<&RootSelectionData<Handle>>,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    match certificate.mode {
        AdjunctionMode::ComplexFTA | AdjunctionMode::AlgebraicClosure => {
            if relation.ambient != Field::Complex || certificate.order.is_some() {
                return Err(invalid());
            }
            unrestricted(cas, selected, budget)?;
            super::splitting::check(
                cas,
                &relation.polynomial,
                &certificate.splitting_chain,
                budget,
            )
        }
        AdjunctionMode::PrimitiveUnity => {
            if !certificate.splitting_chain.is_empty() {
                return Err(invalid());
            }
            unrestricted(cas, selected, budget)?;
            unity(
                cas,
                relation,
                certificate.order.ok_or_else(invalid)?,
                budget,
            )
        }
        AdjunctionMode::RealOddRoot => {
            let order = certificate.order.ok_or_else(invalid)?;
            if relation.ambient != Field::Real
                || order.is_multiple_of(2)
                || !certificate.splitting_chain.is_empty()
            {
                return Err(invalid());
            }
            unrestricted(cas, selected, budget)?;
            super::real_roots::binomial(cas, relation, order, budget)?;
            Ok(())
        }
        AdjunctionMode::PositiveRealSquareRoot => {
            if relation.ambient != Field::Real
                || certificate.order != Some(2)
                || !certificate.splitting_chain.is_empty()
            {
                return Err(invalid());
            }
            let strict = selected.map_or(Ok(false), |value| {
                super::real_roots::positive_selection(cas, value, budget)
            })?;
            let constant = super::real_roots::binomial(cas, relation, 2, budget)?;
            super::real_roots::sign(cas, relation, constant.as_ref(), strict, premises, budget)
        }
    }
}

/// Root existence is the only conclusion: no theorem here implies uniqueness or a principal branch.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateRootAdjunction,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(1, AT)?;
    let SolveDraft::RootRelation(relation) =
        crate::solving::graph::object(&cas.state, cas.checked(&certificate.relation)?)?
    else {
        return Err(invalid());
    };
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let SolveStatement::RootExistence(existence) = statement.as_ref() else {
        return Err(invalid());
    };
    let target = certificate
        .selection
        .as_ref()
        .unwrap_or(&certificate.relation);
    if existence.relation != *target
        || existence.region != relation.region
        || cas.state.context(cas.checked(&draft.context)?)?.admissible
            != relation.region.logical_id()
    {
        return Err(invalid());
    }
    leading(cas, relation, certificate.leading_unit, premises, budget)?;
    let map = crate::solving::coefficients::proof::mapping(
        cas,
        &relation.map,
        &draft.context,
        &relation.region,
        budget,
    )?;
    crate::solving::coefficients::proof::identity_map(cas, map, premises, budget)?;
    let selected = selection(cas, certificate, budget)?;
    theorem(cas, certificate, relation, selected, premises, budget)
}
