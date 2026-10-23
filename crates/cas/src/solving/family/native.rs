// SPDX-License-Identifier: AGPL-3.0-only
//! Family cases reuse the existing domain, selection and congruence applications.

use super::{AT, body, invalid, local, region};
use crate::budget::Budget;
use crate::case_model::CaseCertificate;
use crate::error::Fault;
use crate::graph::{Cas, Draft, Handle, LogicalId};
use crate::proof_model::{
    ApplicationDraft, CheckedApplication, DomainContract, ScopedConditions, Statement,
};
use crate::rule::Primitive;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::elementary::{complex::Components, evaluate::Evaluation};
use crate::solving::{CoefficientDomainData, SolveDraft};

/// The old case checker never enters this path for an ordinary scalar case.
pub(crate) fn handles(cas: &Cas, statement: &Statement) -> Result<bool, Fault> {
    match statement {
        Statement::Equality { left, right, .. } => {
            Ok(local(cas, left.logical_id())? || local(cas, right.logical_id())?)
        }
        Statement::DefinedDomain { function, .. } => local(cas, function.logical_id()),
        Statement::PredicateFact {
            predicate: fact, ..
        } => {
            let (kind, arguments) = predicate(cas, fact.logical_id())?;
            if kind == &crate::model::PredicateKind::Defined
                && let [function] = arguments
            {
                local(cas, *function)
            } else {
                Ok(false)
            }
        }
        Statement::Compatibility { left, right, .. } => {
            Ok(
                body(cas, cas.state.arm(left.logical_id())?.value)?.is_some()
                    || body(cas, cas.state.arm(right.logical_id())?.value)?.is_some(),
            )
        }
        _ => Ok(false),
    }
}

/// Reconstruct an ordinary case certificate from original associations and exact projection premises.
pub(crate) fn reconstruct(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(CaseCertificate, ScopedConditions), Fault> {
    let (left, right, domain) = endpoints(cas, draft, budget)?;
    let within = cas.state.context(cas.checked(&draft.context)?)?.admissible;
    if !region::subset(cas, domain.logical_id(), within, budget)? {
        return Err(invalid());
    }
    let ground = crate::solving::radicals::construction::admit(
        cas,
        &Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Rational,
        )),
        budget,
    )?;
    let evaluation = Evaluation {
        exclusions: &[],
        cas,
        engine: Engine {
            state: &cas.state,
            domain: &ground,
        },
        region: &domain,
        premises,
        free: &[],
        bound: &[],
    };
    let mut components = Components::new(&evaluation, budget)?;
    let slots = (0..premises.len())
        .map(|index| u32::try_from(index).map_err(|_| invalid()))
        .collect::<Result<Vec<_>, _>>()?;
    components.premises(&slots, budget)?;
    components.equal(left, right, false, budget)?;
    let mut certificate = CaseCertificate::default();
    inventory(cas, left, &mut certificate, budget)?;
    inventory(cas, right, &mut certificate, budget)?;
    certificate.regions.push(domain);
    Ok((certificate, ScopedConditions::default()))
}

/// Selection identifies the actual local body; domain claims name the entire represented support.
fn endpoints(
    cas: &Cas,
    draft: &ApplicationDraft,
    budget: &mut Budget,
) -> Result<(LogicalId, LogicalId, Handle), Fault> {
    match (&draft.rule, &draft.conclusion) {
        (
            Primitive::CaseCongruence | Primitive::CaseSelect,
            Statement::Equality {
                left,
                right,
                domain,
                domain_contract,
            },
        ) => {
            if draft.rule == Primitive::CaseSelect {
                if *domain_contract != DomainContract::DefinedAndEqual {
                    return Err(invalid());
                }
                selected(
                    cas,
                    left.logical_id(),
                    right.logical_id(),
                    domain.logical_id(),
                    budget,
                )?;
            }
            Ok((left.logical_id(), right.logical_id(), domain.clone()))
        }
        (
            Primitive::CaseCongruence,
            Statement::Compatibility {
                left,
                right,
                region: domain,
            },
        ) => {
            let first = cas.state.arm(cas.checked(left)?)?;
            let second = cas.state.arm(cas.checked(right)?)?;
            if !region::subset(cas, domain.logical_id(), first.region, budget)?
                || !region::subset(cas, domain.logical_id(), second.region, budget)?
            {
                return Err(invalid());
            }
            Ok((first.value, second.value, domain.clone()))
        }
        (
            Primitive::CaseDomain,
            Statement::PredicateFact {
                region: domain,
                predicate: condition,
            },
        ) => {
            let (kind, arguments) = predicate(cas, condition.logical_id())?;
            let [function] = arguments else {
                return Err(invalid());
            };
            if kind != &crate::model::PredicateKind::Defined {
                return Err(invalid());
            }
            Ok((*function, *function, domain.clone()))
        }
        (Primitive::CaseDomain, Statement::DefinedDomain { function, domain }) => {
            let value = cas.state.expression(cas.checked(function)?)?;
            if value.arms.is_empty() {
                return Err(invalid());
            }
            for arm in &value.arms {
                budget.work(1, AT)?;
                if cas.state.arm(*arm)?.region != domain.logical_id() {
                    return Err(invalid());
                }
            }
            Ok((function.logical_id(), function.logical_id(), domain.clone()))
        }
        _ => Err(invalid()),
    }
}

/// A body is selectable only from its exact native association on a proved contained region.
fn selected(
    cas: &Cas,
    left: LogicalId,
    right: LogicalId,
    domain: LogicalId,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for id in &cas.state.expression(left)?.arms {
        budget.work(1, AT)?;
        let arm = cas.state.arm(*id)?;
        if arm.value == right && region::subset(cas, domain, arm.region, budget)? {
            return Ok(());
        }
    }
    Err(invalid())
}

/// The portable existing payload retains every placement instance and its incidence frame.
fn inventory(
    cas: &Cas,
    root: LogicalId,
    result: &mut CaseCertificate,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if let Some(value) = body(cas, root)? {
        result.regions.push(value.region.clone());
        return Ok(());
    }
    for id in &cas.state.expression(root)?.arms {
        budget.work(1, AT)?;
        let arm = cas.state.arm(*id)?;
        result.arms.push(cas.handle(*id));
        result.regions.push(cas.handle(arm.region));
    }
    Ok(())
}

/// Native family endpoints keep their independently admitted result sort and binder scope.
pub(crate) fn validate_endpoints(
    cas: &Cas,
    left: &Handle,
    right: &Handle,
    domain: &Handle,
    same_sort: bool,
) -> Result<(), Fault> {
    let (left_scope, left_sort) = endpoint_type(cas, left.logical_id())?;
    let (right_scope, right_sort) = endpoint_type(cas, right.logical_id())?;
    let region = cas.state.region(cas.checked(domain)?)?;
    let different_sort = same_sort && left_sort != right_sort;
    let different_region = !region.scope.is_empty() && &region.scope != left_scope;
    if left_scope != right_scope || different_sort || different_region {
        return Err(invalid());
    }
    Ok(())
}

/// Reading a local result never manufactures an ordinary scalar expression.
fn endpoint_type(
    cas: &Cas,
    id: LogicalId,
) -> Result<(&crate::model::Scope, &crate::model::Sort), Fault> {
    if let Some(value) = body(cas, id)? {
        return Ok((&value.scope, &value.result));
    }
    let value = cas.state.expression(id)?;
    Ok((&value.scope, &value.sort))
}

/// Definedness remains an ordinary predicate whose subject is a native family case.
pub(crate) fn predicate(
    cas: &Cas,
    id: LogicalId,
) -> Result<(&crate::model::PredicateKind, &[LogicalId]), Fault> {
    match cas.state.node(id)? {
        crate::graph::Vertex::Condition(value) => Ok((&value.kind, &value.arguments)),
        crate::graph::Vertex::Expression(value) => {
            let crate::model::Operator::Predicate(kind) = &value.operator else {
                return Err(invalid());
            };
            Ok((kind, &value.operands))
        }
        _ => Err(invalid()),
    }
}
