// SPDX-License-Identifier: AGPL-3.0-only
//! Foreign conditions are transported as exact guarded obligations, never borrowed.

use super::syntax::Correspondence;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::Cas;
use crate::proof_model::{
    ApplicationDraft, CheckedApplication, GuardedObligation, ScopedConditions, Statement,
};
use crate::solving::coefficients::storage::reserve;
use crate::solving::elementary::proof::{AT, invalid, premise};
use crate::solving::{RestrictionCondition, RestrictionConditionKind};

/// Reconstruct the full source obligation from an earlier checked application.
fn source(
    cas: &Cas,
    row: &RestrictionCondition,
    premises: &[CheckedApplication],
) -> Result<GuardedObligation, Fault> {
    let previous = premise(premises, row.premise)?;
    let index = usize::try_from(row.index).map_err(|_| invalid())?;
    match row.kind {
        RestrictionConditionKind::Requirement => previous
            .conditions
            .requirements
            .get(index)
            .cloned()
            .ok_or_else(invalid),
        RestrictionConditionKind::Residual => previous
            .conditions
            .residuals
            .get(index)
            .cloned()
            .ok_or_else(invalid),
        RestrictionConditionKind::Hypothesis => {
            let used = previous
                .conditions
                .hypotheses
                .get(index)
                .ok_or_else(invalid)?;
            let predicate = crate::context_check::predicate(cas, used)?;
            let (_, scope) =
                crate::domain_admission::predicate_context(&cas.state, predicate.logical_id())?;
            let frame = cas.state.context(previous.judgment.context.logical_id())?;
            let region = crate::context_check::domain(&previous.judgment.statement)
                .cloned()
                .unwrap_or_else(|| cas.handle(frame.admissible));
            Ok(GuardedObligation {
                home: used.context.clone(),
                scope: scope.clone(),
                layer: previous.judgment.layer,
                space: previous.judgment.space.clone(),
                region,
                predicate,
            })
        }
    }
}

/// A discharged image needs an exact target-context predicate theorem.
fn target(
    comparison: &mut Correspondence<'_>,
    draft: &ApplicationDraft,
    row: &RestrictionCondition,
    result: &mut ScopedConditions,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let original = source(comparison.cas, row, comparison.premises)?;
    if (
        &original.home,
        &original.region,
        &original.predicate,
        &row.contexts.real,
    ) != (
        &row.contexts.complex,
        &row.regions.complex,
        &row.predicates.complex,
        &draft.context,
    ) || original.space.is_some()
    {
        return Err(invalid());
    }
    comparison.check(&row.regions.real, &row.regions.complex, budget)?;
    comparison.check(&row.predicates.real, &row.predicates.complex, budget)?;
    let (_, scope) = crate::domain_admission::predicate_context(
        &comparison.cas.state,
        row.predicates.real.logical_id(),
    )?;
    if scope != &original.scope {
        return Err(invalid());
    }
    let obligation = GuardedObligation {
        home: draft.context.clone(),
        scope: scope.clone(),
        layer: original.layer,
        space: None,
        region: row.regions.real.clone(),
        predicate: row.predicates.real.clone(),
    };
    crate::statement::validate_guard(comparison.cas, &obligation, budget)?;
    reserve(&mut result.requirements, 1, budget)?;
    result.requirements.push(obligation.clone());
    if let Some(slot) = row.discharge {
        let discharged = premise(comparison.premises, slot)?;
        if (&discharged.judgment.context, &discharged.judgment.statement)
            != (
                &draft.context,
                &Statement::PredicateFact {
                    region: obligation.region,
                    predicate: obligation.predicate,
                },
            )
        {
            return Err(invalid());
        }
        crate::statement::merge(comparison.cas, result, &discharged.conditions, budget)?;
    } else if row.kind != RestrictionConditionKind::Requirement {
        reserve(&mut result.residuals, 1, budget)?;
        result.residuals.push(obligation);
    }
    Ok(())
}

/// Every used condition has exactly one row; extraneous and duplicate rows are refused.
fn rows(
    comparison: &mut Correspondence<'_>,
    draft: &ApplicationDraft,
    result: &mut ScopedConditions,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut count = 0_usize;
    let premises = comparison.premises;
    for (slot, previous) in premises.iter().enumerate() {
        if previous.judgment.context == draft.context {
            continue;
        }
        for (kind, length) in [
            (
                RestrictionConditionKind::Hypothesis,
                previous.conditions.hypotheses.len(),
            ),
            (
                RestrictionConditionKind::Requirement,
                previous.conditions.requirements.len(),
            ),
            (
                RestrictionConditionKind::Residual,
                previous.conditions.residuals.len(),
            ),
        ] {
            for index in 0..length {
                budget.work(
                    crate::graph::size(comparison.transport.conditions.len(), AT)?,
                    AT,
                )?;
                let mut matching = comparison.transport.conditions.iter().filter(|row| {
                    (
                        usize::try_from(row.premise).ok(),
                        row.kind,
                        usize::try_from(row.index).ok(),
                    ) == (Some(slot), kind, Some(index))
                });
                let row = matching.next().ok_or_else(invalid)?;
                if matching.next().is_some() {
                    return Err(invalid());
                }
                target(comparison, draft, row, result, budget)?;
                count = count.checked_add(1).ok_or_else(invalid)?;
            }
        }
    }
    if count != comparison.transport.conditions.len() {
        return Err(invalid());
    }
    Ok(())
}

/// Foreign context identity is checked before its conditions can support the result.
pub(crate) fn check(
    comparison: &mut Correspondence<'_>,
    draft: &ApplicationDraft,
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    if !draft.substitutions.is_empty() {
        return Err(invalid());
    }
    budget.solving(BudgetKind::SolveIdentities, 1, AT)?;
    let mut result = ScopedConditions::default();
    for previous in comparison.premises {
        budget.work(1, AT)?;
        if previous.judgment.context == draft.context {
            crate::statement::merge(comparison.cas, &mut result, &previous.conditions, budget)?;
        } else if comparison
            .cas
            .state
            .context(previous.judgment.context.logical_id())?
            .header_id
            != comparison.complex.header.logical_id()
        {
            return Err(invalid());
        }
    }
    rows(comparison, draft, &mut result, budget)?;
    result.requirements = crate::statement::union(comparison.cas, &result.requirements, budget)?;
    result.residuals = crate::statement::union(comparison.cas, &result.residuals, budget)?;
    Ok(result)
}
