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
    let Some(original) = domain(&source.judgment.statement) else {
        return Ok(false);
    };
    let Statement::Subset { superset, .. } = &subset.judgment.statement else {
        return Ok(false);
    };
    let lifted = cas.state.region(superset.logical_id())?;
    let Some(over) = &lifted.lies_over else {
        return Ok(false);
    };
    let changed_frame = lifted.ambient != cas.state.region(original.logical_id())?.ambient;
    let exact_base = over.stratum == original.logical_id();
    Ok(changed_frame || exact_base)
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
    let original = domain(&source.judgment.statement).ok_or_else(invalid)?;
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
        || !statement_matches(
            cas,
            &source.judgment.statement,
            &draft.conclusion,
            &target,
            over.family,
            budget,
        )?
    {
        return Err(invalid());
    }
    let supported = matches!(
        source.judgment.statement,
        Statement::Equality { .. } | Statement::PredicateFact { .. } | Statement::Cover { .. }
    ) || matches!(&source.judgment.statement, Statement::Solve(value) if matches!(value.as_ref(), crate::solving::SolveStatement::RootExistence(_) | crate::solving::SolveStatement::RealSign(_)));
    if !supported {
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

/// Family restriction recognizes existing pointwise Real existence and sign statements.
pub(crate) fn domain(statement: &Statement) -> Option<&crate::graph::Handle> {
    if let Statement::Solve(value) = statement {
        return match value.as_ref() {
            crate::solving::SolveStatement::RootExistence(value) => Some(&value.region),
            crate::solving::SolveStatement::RealSign(value) => Some(&value.region),
            _ => None,
        };
    }
    crate::context_check::domain(statement)
}

/// Change only the evaluation region of an existing pointwise statement.
pub(crate) fn restricted(statement: &Statement, target: &crate::graph::Handle) -> Statement {
    let mut result = crate::context_check::restricted(statement, target);
    if let Statement::Solve(value) = &mut result {
        match value.as_mut() {
            crate::solving::SolveStatement::RootExistence(value) => value.region.clone_from(target),
            crate::solving::SolveStatement::RealSign(value) => value.region.clone_from(target),
            _ => {}
        }
    }
    result
}

/// A selected Real base relation keeps its exact source region under a checked pullback.
pub(crate) fn source(
    cas: &Cas,
    target: &crate::graph::Handle,
    original: &crate::graph::Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let family = region::family(cas, target.logical_id(), budget)?;
    for premise in premises {
        budget.work(1, AT)?;
        if let Statement::Subset { subset, superset } = &premise.judgment.statement
            && subset == target
            && cas
                .state
                .region(superset.logical_id())?
                .lies_over
                .as_ref()
                .is_some_and(|over| over.family == family && over.stratum == original.logical_id())
            && region::lifted(cas, original.logical_id(), superset.logical_id(), budget)?
        {
            return Ok(());
        }
    }
    Err(invalid())
}

/// Same-region selected relations retain their old checks; family pullbacks require a Real original.
pub(crate) fn relation_region(
    evaluation: &crate::solving::elementary::evaluate::Evaluation<'_>,
    relation: &crate::solving::RootRelationData<crate::graph::Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if relation.region == *evaluation.region {
        return Ok(());
    }
    if relation.ambient != crate::model::Field::Real {
        return Err(invalid());
    }
    source(
        evaluation.cas,
        evaluation.region,
        &relation.region,
        evaluation.premises,
        budget,
    )
}

/// Pulling back a cover preserves the complete original member inventory and predicate trees.
fn statement_matches(
    cas: &Cas,
    source: &Statement,
    conclusion: &Statement,
    target: &crate::graph::Handle,
    family: crate::graph::LogicalId,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    if let Statement::Cover { members, .. } = source {
        let Statement::Cover {
            domain,
            members: actual,
        } = conclusion
        else {
            return Ok(false);
        };
        if domain != target || members.len() != actual.len() {
            return Ok(false);
        }
        for (original, lifted) in members.iter().zip(actual) {
            let value = cas.state.region(lifted.logical_id())?;
            if value
                .lies_over
                .as_ref()
                .is_none_or(|over| over.family != family || over.stratum != original.logical_id())
                || value.ambient != cas.state.region(target.logical_id())?.ambient
                || !region::lifted(cas, original.logical_id(), lifted.logical_id(), budget)?
            {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    Ok(*conclusion == restricted(source, target))
}
