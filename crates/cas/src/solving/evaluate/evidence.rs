// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit image definedness and earlier exact root selections precede specialization.

use super::{AT, invalid};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{ExpressionAttachment, Handle, LogicalId, Vertex, size};
use crate::model::PredicateKind;
use crate::proof_model::Statement;
use crate::solving::coefficients::storage::{reserve, vector};
use crate::solving::elementary::evaluate::Evaluation;
use crate::solving::elementary::proof::premise;
use crate::solving::{
    CertificateCoefficientEvaluation, CoefficientEvaluationData, EvaluationTarget, SolveDraft,
    SolveStatement,
};
use std::collections::BTreeSet;

/// Require an exact image-definedness premise for every ordered slot.
pub(super) fn check(
    evaluation: &Evaluation<'_>,
    mapping: &CoefficientEvaluationData<Handle>,
    certificate: &CertificateCoefficientEvaluation,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if mapping.bindings.len() != certificate.definedness.len() {
        return Err(invalid());
    }
    for (binding, slot) in mapping.bindings.iter().zip(&certificate.definedness) {
        budget.work(1, AT)?;
        let proof = premise(evaluation.premises, *slot)?;
        let Statement::PredicateFact { region, predicate } = &proof.judgment.statement else {
            return Err(invalid());
        };
        let (kind, arguments) = crate::solving::elementary::evidence::predicate(
            evaluation.cas,
            predicate.logical_id(),
        )?;
        let [argument] = arguments else {
            return Err(invalid());
        };
        if region != evaluation.region
            || *kind != PredicateKind::Defined
            || !defined_image(evaluation, *argument, binding.image.logical_id())?
        {
            return Err(invalid());
        }
    }
    target(evaluation, mapping, &certificate.right)?;
    selections(evaluation, mapping, certificate, budget)
}

/// The claimed result remains in the exact scalar target or local family.
fn target(
    evaluation: &Evaluation<'_>,
    mapping: &CoefficientEvaluationData<Handle>,
    right: &Handle,
) -> Result<(), Fault> {
    match (
        &mapping.target,
        evaluation.cas.state.node(right.logical_id())?,
    ) {
        (EvaluationTarget::Scalar { header, space }, Vertex::Expression(value)) => {
            let region = evaluation
                .cas
                .state
                .region(evaluation.region.logical_id())?;
            if value.header != header.logical_id()
                || value.ambient != Some(space.logical_id())
                || value.scope != region.scope
            {
                return Err(invalid());
            }
        }
        (EvaluationTarget::Family { family }, Vertex::Solve(value)) => {
            let SolveDraft::FamilyExpression(value) = value.as_ref() else {
                return Err(invalid());
            };
            let SolveDraft::SpaceFamily(shape) =
                crate::solving::graph::object(&evaluation.cas.state, family.logical_id())?
            else {
                return Err(invalid());
            };
            if value.family != *family
                || value.region != *evaluation.region
                || value.ambient != shape.fibre_space
            {
                return Err(invalid());
            }
        }
        _ => return Err(invalid()),
    }
    Ok(())
}

/// Every selected-root dependency has an explicitly indexed prior existence theorem.
fn selections(
    evaluation: &Evaluation<'_>,
    mapping: &CoefficientEvaluationData<Handle>,
    certificate: &CertificateCoefficientEvaluation,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut roots = vector(mapping.bindings.len().saturating_add(1), budget)?;
    roots.extend(
        mapping
            .bindings
            .iter()
            .map(|binding| binding.image.logical_id()),
    );
    roots.push(certificate.right.logical_id());
    let mut selections = selected(evaluation, roots, budget)?;
    if selections.len() != certificate.existence.len() {
        return Err(invalid());
    }
    for slot in &certificate.existence {
        budget.work(1, AT)?;
        let proof = premise(evaluation.premises, *slot)?;
        let Statement::Solve(statement) = &proof.judgment.statement else {
            return Err(invalid());
        };
        let SolveStatement::RootExistence(value) = statement.as_ref() else {
            return Err(invalid());
        };
        if !selections.remove(&value.relation.logical_id()) || value.region != *evaluation.region {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Walk complete attachments and family bodies without erasing a canceled dependency.
fn selected(
    evaluation: &Evaluation<'_>,
    roots: Vec<LogicalId>,
    budget: &mut Budget,
) -> Result<BTreeSet<LogicalId>, Fault> {
    let mut pending = vector(roots.len(), budget)?;
    pending.extend(roots.into_iter().map(|id| (id, 1_u64)));
    let mut visited = BTreeSet::new();
    let mut selections = BTreeSet::new();
    while let Some((id, depth)) = pending.pop() {
        budget.check(BudgetKind::Depth, depth, AT)?;
        let count = size(
            visited
                .len()
                .saturating_add(selections.len())
                .saturating_add(pending.len())
                .saturating_add(1),
            AT,
        )?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            count.saturating_mul(128),
            AT,
        )?;
        budget.work(size(visited.len().saturating_add(1), AT)?, AT)?;
        if !visited.insert(id) {
            continue;
        }
        if let Vertex::Expression(value) = evaluation.cas.state.node(id)?
            && let Some(ExpressionAttachment::SelectedRoot(selection)) = &value.attachment
        {
            selections.insert(*selection);
        }
        let children = crate::descriptor::children(&evaluation.cas.state, id, budget)?;
        reserve(&mut pending, children.len(), budget)?;
        pending.extend(
            children
                .into_iter()
                .map(|(_, id)| (id, depth.saturating_add(1))),
        );
    }
    Ok(selections)
}

/// A one-arm native case proves definedness of its exact local family body on that same region.
fn defined_image(
    evaluation: &Evaluation<'_>,
    actual: LogicalId,
    expected: LogicalId,
) -> Result<bool, Fault> {
    if actual == expected {
        return Ok(true);
    }
    let value = evaluation.cas.state.expression(actual)?;
    if value.domain != Some(evaluation.region.logical_id()) {
        return Ok(false);
    }
    let [arm] = value.arms.as_slice() else {
        return Ok(false);
    };
    let arm = evaluation.cas.state.arm(*arm)?;
    Ok(arm.region == evaluation.region.logical_id() && arm.value == expected)
}
