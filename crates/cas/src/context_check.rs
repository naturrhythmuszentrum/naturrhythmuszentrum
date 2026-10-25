// SPDX-License-Identifier: AGPL-3.0-only
//! Qualified hypotheses and checked restriction, embedding and local-frame closure.

use std::collections::BTreeSet;

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::{HypothesisRef, RegionDraft, RegionExpr};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Draft, Handle, LogicalId, Reference, size};
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, GuardedObligation, ScopedConditions,
    Statement,
};

/// Scoped hypothesis theorem location.
const AT: Location = Location::new("context theorem");

/// A malformed scope transport never becomes an unknown condition.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Context, AT)
}

/// Check exact ancestry with a bounded parent walk.
pub(crate) fn ancestor(
    cas: &Cas,
    introducing: LogicalId,
    context: LogicalId,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let mut current = Some(context);
    while let Some(id) = current {
        budget.condition(1, AT)?;
        let frame = cas.state.context(id)?;
        budget.check(BudgetKind::HypothesisDepth, frame.depth, AT)?;
        if id == introducing {
            return Ok(true);
        }
        current = frame.parent;
    }
    Ok(false)
}

/// Resolve a qualified predicate slot; membership slot zero has a separate typed law.
pub(crate) fn predicate(cas: &Cas, hypothesis: &HypothesisRef) -> Result<Handle, Fault> {
    let context = cas.state.context(cas.checked(&hypothesis.context)?)?;
    let slot = usize::try_from(hypothesis.slot).map_err(|_| invalid())?;
    let frame = if context.parent.is_some() {
        &context.frame
    } else {
        &context.assumptions
    };
    let slot = if context.parent.is_some() {
        slot.checked_sub(1).ok_or_else(invalid)?
    } else {
        slot
    };
    Ok(cas.handle(frame.get(slot).ok_or_else(invalid)?.1))
}

/// Reconstruct exactly the predicate introduced by the requested ancestor slot.
pub(crate) fn hypothesis(
    cas: &Cas,
    draft: &ApplicationDraft,
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let Certificate::Hypothesis(reference) = &draft.certificate else {
        return Err(invalid());
    };
    if !draft.premises.is_empty() || !draft.substitutions.is_empty() {
        return Err(invalid());
    }
    let context = cas.checked(&draft.context)?;
    let introducing = cas.checked(&reference.context)?;
    if !ancestor(cas, introducing, context, budget)? {
        return Err(invalid());
    }
    let frame = cas.state.context(introducing)?;
    if frame.parent.is_some() && reference.slot == 0 {
        let Statement::Subset { subset, superset } = &draft.conclusion else {
            return Err(invalid());
        };
        if cas.checked(subset)? != cas.state.context(context)?.admissible
            || Some(cas.checked(superset)?) != frame.restriction
        {
            return Err(invalid());
        }
    } else {
        let Statement::PredicateFact {
            region,
            predicate: expected,
        } = &draft.conclusion
        else {
            return Err(invalid());
        };
        if cas.checked(region)? != cas.state.context(context)?.admissible
            || *expected != predicate(cas, reference)?
        {
            return Err(invalid());
        }
    }
    let mut conditions = ScopedConditions {
        hypotheses: vec![reference.clone()],
        ..ScopedConditions::default()
    };
    if reference.slot == 0
        && let Some(introduction) = frame.introduction
    {
        let proof = crate::proof_view::application(cas, introduction, budget)?;
        crate::statement::merge(cas, &mut conditions, &proof.conditions, budget)?;
    }
    Ok(conditions)
}

/// The explicitly stated pointwise domain, if the proposition has one.
pub(crate) const fn domain(statement: &Statement) -> Option<&Handle> {
    match statement {
        Statement::CaseDerivative(value) => Some(&value.domain),
        Statement::Equality { domain, .. }
        | Statement::DerivativeOn { domain, .. }
        | Statement::Jacobian { domain, .. }
        | Statement::SquareFree { domain, .. }
        | Statement::DefinedDomain { domain, .. }
        | Statement::Elaboration { domain, .. }
        | Statement::Cover { domain, .. } => Some(domain),
        Statement::PredicateFact { region, .. } | Statement::Compatibility { region, .. } => {
            Some(region)
        }
        _ => None,
    }
}

/// Mechanically restrict the domain without changing endpoints, coordinate or contract.
pub(crate) fn restricted(statement: &Statement, region: &Handle) -> Statement {
    let mut result = statement.clone();
    match &mut result {
        Statement::CaseDerivative(value) => value.domain.clone_from(region),
        Statement::Equality { domain, .. }
        | Statement::DerivativeOn { domain, .. }
        | Statement::Jacobian { domain, .. }
        | Statement::SquareFree { domain, .. }
        | Statement::DefinedDomain { domain, .. }
        | Statement::Elaboration { domain, .. }
        | Statement::Cover { domain, .. }
        | Statement::PredicateFact { region: domain, .. }
        | Statement::Compatibility { region: domain, .. } => domain.clone_from(region),
        _ => {}
    }
    result
}

/// Construct an exact meet for a transported guarded predicate, without logical simplification.
fn intersection(
    cas: &mut Cas,
    left: &Handle,
    right: &Handle,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    if left == right {
        return Ok(left.clone());
    }
    let region = cas.state.region(cas.checked(left)?)?.clone();
    let handles = cas.admit(
        &[Draft::Region(RegionDraft {
            header: Reference::Existing(cas.handle(region.header)),
            scope: region.scope,
            ambient: region.ambient,
            expression: RegionExpr::Meet(vec![
                Reference::Existing(left.clone()),
                Reference::Existing(right.clone()),
            ]),
        })],
        budget,
    )?;
    handles.first().cloned().ok_or_else(invalid)
}

/// Preserve every used assumption when transporting to an independent extended root.
fn embed_hypotheses(
    cas: &Cas,
    source: &CheckedApplication,
    target: &Handle,
    budget: &mut Budget,
) -> Result<Vec<HypothesisRef>, Fault> {
    let target_id = cas.checked(target)?;
    let source_id = cas.checked(&source.judgment.context)?;
    if ancestor(cas, source_id, target_id, budget)? {
        return Ok(source.conditions.hypotheses.clone());
    }
    let original = cas.state.context(source_id)?;
    let destination = cas.state.context(target_id)?;
    if original.parent.is_some()
        || destination.parent.is_some()
        || !crate::header::embeds(
            &cas.state,
            original.header_id,
            destination.header_id,
            budget,
        )?
    {
        return Err(invalid());
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(source.conditions.hypotheses.len())
        .map_err(|_| Fault::allocation(AT))?;
    for used in &source.conditions.hypotheses {
        if used.context != source.judgment.context {
            return Err(invalid());
        }
        let index = usize::try_from(used.slot).map_err(|_| invalid())?;
        let assumption = original.assumptions.get(index).ok_or_else(invalid)?;
        let slot = destination
            .assumptions
            .iter()
            .position(|item| item == assumption)
            .ok_or_else(invalid)?;
        result.push(HypothesisRef {
            context: target.clone(),
            slot: u32::try_from(slot).map_err(|_| invalid())?,
        });
    }
    Ok(result)
}

/// Restrict a parent theorem only with explicit checked subset evidence.
pub(crate) fn weaken(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    if draft.certificate != Certificate::None
        || !draft.substitutions.is_empty()
        || premises.len() != 2
    {
        return Err(invalid());
    }
    if crate::solving::family::restriction::applies(cas, premises)? {
        return crate::solving::family::restriction::check(cas, draft, premises, budget);
    }
    let source = premises.first().ok_or_else(invalid)?;
    let subset = premises.get(1).ok_or_else(invalid)?;
    if !matches!(
        &source.judgment.statement,
        Statement::Equality { .. }
            | Statement::CaseDerivative(_)
            | Statement::DerivativeOn { .. }
            | Statement::Jacobian { .. }
            | Statement::SquareFree { .. }
            | Statement::PredicateFact { .. }
            | Statement::Compatibility { .. }
    ) {
        return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
    }
    let target = cas.handle(cas.state.context(cas.checked(&draft.context)?)?.admissible);
    let original = match domain(&source.judgment.statement) {
        Some(value) => value.clone(),
        None => cas.handle(
            cas.state
                .context(cas.checked(&source.judgment.context)?)?
                .admissible,
        ),
    };
    let expected = Statement::Subset {
        subset: target.clone(),
        superset: original,
    };
    if subset.judgment.statement != expected
        || (subset.judgment.context != draft.context
            && subset.judgment.context != source.judgment.context)
        || draft.conclusion != restricted(&source.judgment.statement, &target)
    {
        return Err(invalid());
    }
    let hypotheses = embed_hypotheses(cas, source, &draft.context, budget)?;
    let mut result = source.conditions.clone();
    result.hypotheses = hypotheses;
    for values in [&mut result.requirements, &mut result.residuals] {
        for guarded in values.iter_mut() {
            guarded.home.clone_from(&draft.context);
            guarded.region = intersection(cas, &guarded.region, &target, budget)?;
        }
        *values = crate::statement::union(cas, values, budget)?;
    }
    crate::statement::merge(cas, &mut result, &subset.conditions, budget)?;
    Ok(result)
}

/// Close the exact local frame while keeping its mathematical region unchanged.
pub(crate) fn close(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let Certificate::Close { frame } = &draft.certificate else {
        return Err(invalid());
    };
    if !draft.substitutions.is_empty() {
        return Err(invalid());
    }
    if crate::solving::coefficients::formal_close::boundary(cas, draft, premises)? {
        return crate::solving::coefficients::formal_close::close(cas, draft, premises, budget);
    }
    let child = premises.first().ok_or_else(invalid)?;
    let construction = premises.get(1).ok_or_else(invalid)?;
    let frame_id = cas.checked(frame)?;
    let local = cas.state.context(frame_id)?;
    let parent = local.parent.ok_or_else(invalid)?;
    if cas.checked(&draft.context)? != parent
        || child.judgment.context != *frame
        || child.judgment.statement != draft.conclusion
        || construction.judgment.context != draft.context
        || construction.judgment.statement
            != (Statement::Constructor {
                region: cas.handle(local.admissible),
                law: crate::proof_model::ConstructorLaw::Membership,
            })
    {
        return Err(invalid());
    }
    let mut result = child.conditions.clone();
    result.hypotheses.clear();
    let mut used = BTreeSet::new();
    for reference in &child.conditions.hypotheses {
        if reference.context != *frame {
            if !ancestor(cas, cas.checked(&reference.context)?, parent, budget)? {
                return Err(invalid());
            }
            result.hypotheses.push(reference.clone());
            continue;
        }
        if reference.slot == 0 {
            continue;
        }
        let predicate = predicate(cas, reference)?;
        let region = cas.handle(local.admissible);
        let fact = Statement::PredicateFact {
            region: region.clone(),
            predicate: predicate.clone(),
        };
        if let Some((position, proof)) = premises.iter().enumerate().skip(2).find(|(_, proof)| {
            proof.judgment.context == draft.context && proof.judgment.statement == fact
        }) {
            used.insert(position);
            crate::statement::merge(cas, &mut result, &proof.conditions, budget)?;
        } else {
            let (_, scope) =
                crate::domain_admission::predicate_context(&cas.state, cas.checked(&predicate)?)?;
            let required = GuardedObligation {
                home: draft.context.clone(),
                scope: scope.clone(),
                layer: child.judgment.layer,
                space: child.judgment.space.clone(),
                region,
                predicate,
            };
            result.requirements.push(required.clone());
            result.residuals.push(required);
        }
    }
    if used.len() != premises.len().saturating_sub(2) {
        return Err(invalid());
    }
    for values in [&mut result.requirements, &mut result.residuals] {
        for required in values.iter_mut() {
            if required.home == *frame {
                required.home.clone_from(&draft.context);
            }
        }
        *values = crate::statement::union(cas, values, budget)?;
    }
    crate::statement::merge(cas, &mut result, &construction.conditions, budget)?;
    crate::statement::canonical_hypotheses(cas, &mut result.hypotheses, budget)?;
    budget.condition(size(result.hypotheses.len(), AT)?, AT)?;
    Ok(result)
}
