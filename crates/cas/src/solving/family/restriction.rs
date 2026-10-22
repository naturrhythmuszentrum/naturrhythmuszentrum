// SPDX-License-Identifier: AGPL-3.0-only
//! Context weakening along an explicit, checked base projection.

use super::{AT, invalid, region};
use crate::budget::Budget;
use crate::error::{Fault, InvalidCode};
use crate::graph::Cas;
use crate::proof_model::{ApplicationDraft, CheckedApplication, ScopedConditions, Statement};

/// Only a new family pullback shape can enter the projection branch of context weakening.
pub(crate) fn applies(cas: &Cas, premises: &[CheckedApplication]) -> Result<bool, Fault> {
    let [source, subset] = premises else {
        return Ok(false);
    };
    let Some(original) = crate::context_check::domain(&source.judgment.statement) else {
        return Ok(false);
    };
    let Statement::Subset { superset, .. } = &subset.judgment.statement else {
        return Ok(false);
    };
    let lifted = cas.state.region(superset.logical_id())?;
    Ok(lifted.lies_over.is_some()
        && lifted.ambient != cas.state.region(original.logical_id())?.ambient)
}

/// A closed base-local theorem is restricted through its exact predicate-for-predicate preimage.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let [source, subset] = premises else {
        return Err(invalid());
    };
    let original = crate::context_check::domain(&source.judgment.statement).ok_or_else(invalid)?;
    let target = cas.handle(cas.state.context(cas.checked(&draft.context)?)?.admissible);
    let Statement::Subset {
        subset: actual,
        superset,
    } = &subset.judgment.statement
    else {
        return Err(invalid());
    };
    let over = cas
        .state
        .region(superset.logical_id())?
        .lies_over
        .as_ref()
        .ok_or_else(invalid)?;
    if *actual != target
        || subset.judgment.context != draft.context
        || over.stratum != original.logical_id()
        || over.family != region::family(cas, target.logical_id(), budget)?
        || !region::lifted(cas, original.logical_id(), superset.logical_id(), budget)?
        || draft.conclusion != crate::context_check::restricted(&source.judgment.statement, &target)
    {
        return Err(invalid());
    }
    if !matches!(
        source.judgment.statement,
        Statement::Equality { .. } | Statement::PredicateFact { .. }
    ) {
        return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
    }
    if !source.conditions.requirements.is_empty()
        || !source.conditions.residuals.is_empty()
        || !source.conditions.hypotheses.is_empty()
    {
        return Err(Fault::invalid(InvalidCode::PendingPremise, AT));
    }
    base_only(cas, over.family, &source.judgment.statement, budget)?;
    Ok(subset.conditions.clone())
}

/// The complete source theorem retains only original base declarations, including hidden attachments.
fn base_only(
    cas: &Cas,
    family: crate::graph::LogicalId,
    statement: &Statement,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let crate::solving::SolveDraft::SpaceFamily(shape) =
        crate::solving::graph::object(&cas.state, family)?
    else {
        return Err(invalid());
    };
    let crate::solving::SolveDraft::ParameterBase(base) =
        crate::solving::graph::object(&cas.state, shape.base.logical_id())?
    else {
        return Err(invalid());
    };
    let frame = crate::solving::graph::coordinates(&cas.state, base.coordinates.logical_id())?;
    budget.work(
        crate::graph::size(frame.coordinates.len(), AT)?.saturating_mul(budget.limits().name_bytes),
        AT,
    )?;
    let keys = frame
        .coordinates
        .iter()
        .map(|coordinate| coordinate.key.as_str())
        .collect();
    for object in crate::statement::objects(statement) {
        crate::solving::graph::free_keys(&cas.state, object, &keys, true, budget)?;
    }
    Ok(())
}
