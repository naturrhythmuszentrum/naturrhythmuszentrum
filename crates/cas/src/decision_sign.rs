// SPDX-License-Identifier: AGPL-3.0-only
//! A retained strict-sign membership proves the identical coordinate nonzero.

use crate::budget::Budget;
use crate::domain_model::RegionExpr;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, LogicalId, Vertex};
use crate::model::{Operator, PredicateKind};
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, ScopedConditions, Statement,
};

/// Certified strict-sign consequence and discharge accounting.
const AT: Location = Location::new("certified membership nonzero");

/// A changed scope, predicate or original guard cannot be discharged.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, AT)
}

/// Interpret only strict positive or negative guards of the identical free coordinate.
fn strict(
    cas: &Cas,
    region: LogicalId,
    subject: LogicalId,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(3, AT)?;
    let RegionExpr::GuardTrue { predicate, .. } = &cas.state.region(region)?.expression else {
        return Err(invalid());
    };
    let Vertex::Condition(predicate) = cas.state.node(*predicate)? else {
        return Err(invalid());
    };
    let [argument] = predicate.arguments.as_slice() else {
        return Err(invalid());
    };
    if predicate.kind != PredicateKind::Positive {
        return Err(invalid());
    }
    let expression = cas.state.expression(subject)?;
    if !matches!(expression.operator, Operator::Free(_)) {
        return Err(invalid());
    }
    if *argument == subject {
        return Ok(());
    }
    let negative = cas.state.expression(*argument)?;
    if negative.operator == Operator::Neg && negative.operands == [subject] {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// Consume the exact certified slot and preserve its original source assumptions.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the native proof dispatcher calls this checker"
)]
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let Certificate::DecisionNonZero(reference) = &draft.certificate else {
        return Err(invalid());
    };
    let [membership] = premises else {
        return Err(invalid());
    };
    budget.work(
        crate::graph::size(membership.conditions.hypotheses.len(), AT)?.saturating_add(6),
        AT,
    )?;
    let context = cas.checked(&draft.context)?;
    let frame = cas.state.context(cas.checked(&reference.context)?)?;
    if reference.slot != 0
        || frame.introduction.is_none()
        || !draft.substitutions.is_empty()
        || !crate::context_check::ancestor(cas, cas.checked(&reference.context)?, context, budget)?
    {
        return Err(invalid());
    }
    let region = cas.handle(cas.state.context(context)?.admissible);
    let restriction = frame.restriction.ok_or_else(invalid)?;
    let expected = Statement::Subset {
        subset: region.clone(),
        superset: cas.handle(restriction),
    };
    if membership.judgment.context != draft.context
        || membership.judgment.statement != expected
        || !membership.conditions.residuals.is_empty()
        || !membership.conditions.hypotheses.contains(reference)
    {
        return Err(invalid());
    }
    let Statement::PredicateFact {
        region: domain,
        predicate,
    } = &draft.conclusion
    else {
        return Err(invalid());
    };
    let Vertex::Condition(predicate) = cas.state.node(cas.checked(predicate)?)? else {
        return Err(invalid());
    };
    let [subject] = predicate.arguments.as_slice() else {
        return Err(invalid());
    };
    if *domain != region || predicate.kind != PredicateKind::NonZero {
        return Err(invalid());
    }
    strict(cas, restriction, *subject, budget)?;
    let mut conditions = ScopedConditions::default();
    crate::statement::merge(cas, &mut conditions, &membership.conditions, budget)?;
    Ok(conditions)
}

/// Discharge exactly one original nonzero guard with the checked certified sign consequence.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the native context dispatcher calls this checker"
)]
pub(crate) fn discharge(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let [source, fact] = premises else {
        return Err(invalid());
    };
    let [guard] = source.conditions.residuals.as_slice() else {
        return Err(invalid());
    };
    if draft.certificate != Certificate::DecisionDischarge
        || !draft.substitutions.is_empty()
        || source.judgment.context != draft.context
        || fact.judgment.context != draft.context
        || source.judgment.statement != draft.conclusion
        || !fact.conditions.residuals.is_empty()
        || !matches!(&fact.certificate, Certificate::DecisionNonZero(_))
    {
        return Err(invalid());
    }
    let region = cas.handle(cas.state.context(cas.checked(&draft.context)?)?.admissible);
    if guard.home != draft.context
        || guard.region != region
        || !guard.scope.is_empty()
        || guard.space.is_some()
        || guard.layer != crate::domain_model::Layer::Pointwise
        || !matches!(
            &draft.conclusion,
            Statement::Equality {
                domain_contract: crate::proof_model::DomainContract::DefinedAndEqual,
                ..
            }
        )
        || crate::context_check::domain(&source.judgment.statement) != Some(&region)
        || fact.judgment.statement
            != (Statement::PredicateFact {
                region,
                predicate: guard.predicate.clone(),
            })
    {
        return Err(invalid());
    }
    let Vertex::Condition(predicate) = cas.state.node(cas.checked(&guard.predicate)?)? else {
        return Err(invalid());
    };
    if predicate.kind != PredicateKind::NonZero || predicate.arguments.len() != 1 {
        return Err(invalid());
    }
    budget.work(4, AT)?;
    let mut conditions = source.conditions.clone();
    conditions.residuals.clear();
    crate::statement::merge(cas, &mut conditions, &fact.conditions, budget)?;
    Ok(conditions)
}
