// SPDX-License-Identifier: AGPL-3.0-only
//! Parameter bindings discharge range nonemptiness before universal floor reconstruction.

mod bindings;
mod formula;

use super::{AT, conjunction, invalid};
use crate::budget::{Budget, BudgetKind};
use crate::decision::{CaseCut, CoordinateInterval, IntegerFamily};
use crate::decision_cut::{evaluate, plan};
use crate::error::Fault;
use crate::graph::{Cas, size};
use crate::model::Operator;
use crate::number::Rational;
use crate::proof_model::{ApplicationDraft, CheckedApplication, ScopedConditions};

/// Exact binding premises retain their full source conditions; count bounds are never inferred.
pub(super) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    family: &IntegerFamily,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    budget.with_retained_payload(4_096, AT, |budget| {
        let conditions = bindings::check(cas, draft, family, premises, budget)?;
        let mut cut = super::native(cas, family, budget)?;
        parameters(cas, family, &mut cut, budget)?;
        let axes = size(cut.bounds.len().saturating_sub(4), AT)?.saturating_mul(256);
        budget.with_retained_payload(axes, AT, |budget| reconstruct(cas, family, &cut, budget))?;
        Ok(conditions)
    })
}

/// Traverse original bound operands before adding their explicit Integer parameter coordinates.
fn parameters(
    cas: &Cas,
    family: &IntegerFamily,
    cut: &mut CaseCut,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut roots = Vec::new();
    roots
        .try_reserve_exact(2)
        .map_err(|_| Fault::allocation(AT))?;
    for endpoint in [&family.lower, &family.upper].into_iter().flatten() {
        roots.push(cas.checked(endpoint)?);
    }
    let order = plan::order(cas, &roots, budget)?;
    let bytes = size(order.len(), AT)?.saturating_mul(256);
    budget.with_retained_payload(bytes, AT, |budget| {
        for id in &order {
            let value = cas.state.expression(*id)?;
            if matches!(value.operator, Operator::Free(_)) {
                let parameter = cas.handle(*id);
                if parameter == family.index || parameter == family.coordinate {
                    return Err(invalid());
                }
                budget.work(size(cut.bounds.len(), AT)?.saturating_add(1), AT)?;
                if !cut.bounds.iter().any(|axis| axis.coordinate == parameter) {
                    budget.check(
                        BudgetKind::ConditionAtoms,
                        size(cut.bounds.len(), AT)?.saturating_add(1),
                        AT,
                    )?;
                    cut.bounds
                        .try_reserve(1)
                        .map_err(|_| Fault::allocation(AT))?;
                    cut.bounds.push(CoordinateInterval {
                        coordinate: parameter,
                        lower: Rational::zero(),
                        upper: Rational::zero(),
                    });
                }
            }
        }
        Ok(())
    })
}

/// All referenced coordinates are reconstructed; unrelated declarations add no implicit axis.
fn reconstruct(
    cas: &Cas,
    family: &IntegerFamily,
    cut: &CaseCut,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let (header, relation) = crate::decision_cut::identities(cas, cut, budget)?;
    crate::decision_cut::validate_box(
        cas,
        cut,
        header,
        crate::decision_cut::Coordinates::Mixed,
        budget,
    )?;
    let domain = cas.checked(&family.domain)?;
    let mut roots = vec![relation[0], domain];
    roots
        .try_reserve_exact(2)
        .map_err(|_| Fault::allocation(AT))?;
    for endpoint in [&family.lower, &family.upper].into_iter().flatten() {
        roots.push(cas.checked(endpoint)?);
    }
    let order = plan::order(cas, &roots, budget)?;
    let scratch = size(order.len(), AT)?.saturating_mul(256);
    budget.with_retained_payload(scratch, AT, |budget| {
        let values = evaluate::build(cas, cut, header, domain, &order, budget)?;
        let bytes = evaluate::payload(&values, budget)?.saturating_add(formula::reservation(
            cas,
            family,
            cut.bounds.len(),
            &values,
            budget,
        )?);
        budget.with_retained_payload(bytes, AT, |budget| {
            let mut expected =
                formula::expected(cas, family, cut.bounds.len(), &values, true, budget)?;
            conjunction::check(cas, relation[0], &expected, &values, budget)?;
            formula::project(&mut expected, budget)?;
            conjunction::check(cas, domain, &expected, &values, budget)
        })
    })
}

/// Retain exact checked endpoint premises while deciding one selected index's declared range.
pub(super) fn contains(
    cas: &Cas,
    draft: &ApplicationDraft,
    family: &IntegerFamily,
    cover: &CheckedApplication,
    value: &Rational,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut premises = Vec::new();
    premises
        .try_reserve_exact(cover.premises.len())
        .map_err(|_| Fault::allocation(AT))?;
    for id in &cover.premises {
        premises.push(crate::proof_view::application(cas, *id, budget)?);
    }
    bindings::contains(cas, draft, family, &premises, value, budget)
}
