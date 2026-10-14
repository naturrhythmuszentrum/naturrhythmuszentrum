// SPDX-License-Identifier: AGPL-3.0-only
//! Local association selection and function-indexed singleton ownership.

use crate::budget::Budget;
use crate::case_model::CaseCertificate;
use crate::case_syntax::{self, invalid};
use crate::domain_model::AmbientDomain;
use crate::error::Fault;
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, LogicalId, Reference};
use crate::model::{AtomicSort, Operator, PredicateKind, Sort};
use crate::proof_model::{
    ApplicationDraft, CheckedApplication, DomainContract, Ownership, Statement,
};

/// Reconstruct a local selection or the explicitly registered ownership mode.
pub(crate) fn reconstruct(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<CaseCertificate, Fault> {
    let context = cas.checked(&draft.context)?;
    let within = cas.state.context(context)?.admissible;
    match &draft.conclusion {
        Statement::Equality {
            left,
            right,
            domain,
            domain_contract: DomainContract::DefinedAndEqual,
        } => {
            let left = cas.checked(left)?;
            let right = cas.checked(right)?;
            let domain = cas.checked(domain)?;
            crate::case_check::subset(cas, domain, within, budget)?;
            let arm = selected_arm(cas, left, right, domain, budget)?;
            let analysis = crate::case_cells::analyze(cas, left, right, domain, budget)?;
            analysis.check(
                cas,
                left,
                right,
                domain,
                DomainContract::DefinedAndEqual,
                budget,
            )?;
            let mut result = crate::case_check::from_analysis(cas, &analysis);
            result.arms.push(cas.handle(arm));
            Ok(result)
        }
        Statement::Ownership {
            function,
            boundary,
            value,
        } => ownership(
            cas,
            &draft.context,
            function,
            boundary,
            value,
            premises,
            budget,
        ),
        _ => Err(invalid()),
    }
}

/// A selected expression must belong to an actual association covering the whole local region.
fn selected_arm(
    cas: &Cas,
    function: LogicalId,
    value: LogicalId,
    region: LogicalId,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let expression = cas.state.expression(function)?;
    if expression.domain.is_none() {
        return Err(invalid());
    }
    for id in case_syntax::canonical(cas, &expression.arms, budget)? {
        let arm = cas.state.arm(id)?;
        if arm.value == value {
            match crate::case_check::subset(cas, region, arm.region, budget) {
                Ok(()) => return Ok(id),
                Err(fault) if crate::case_obligations::unknown(&fault) => return Err(fault),
                Err(fault) if matches!(fault.kind, crate::error::FaultKind::Exhausted { .. }) => {
                    return Err(fault);
                }
                Err(_) => {}
            }
        }
    }
    Err(invalid())
}

/// Ownership requires an earlier domain theorem for this exact function and context.
fn ownership(
    cas: &mut Cas,
    context: &Handle,
    function: &Handle,
    boundary: &Handle,
    value: &Ownership,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<CaseCertificate, Fault> {
    let domain = premises
        .iter()
        .find_map(|premise| match &premise.judgment.statement {
            Statement::DefinedDomain {
                function: actual,
                domain,
            } if actual == function && premise.judgment.context == *context => Some(domain.clone()),
            _ => None,
        })
        .ok_or_else(invalid)?;
    let context_id = cas.checked(context)?;
    let boundary_id = cas.checked(boundary)?;
    let singleton = singleton(cas, context_id, boundary_id, budget)?;
    if crate::case_obligations::empty(cas, singleton, budget)? {
        return Err(invalid());
    }
    let function = cas.checked(function)?;
    let analysis = crate::case_cells::analyze(cas, function, function, singleton, budget)?;
    analysis.check(
        cas,
        function,
        function,
        singleton,
        DomainContract::SameDefinedDomain,
        budget,
    )?;
    let mut result = crate::case_check::from_analysis(cas, &analysis);
    match value {
        Ownership::Present(value) => {
            let value = cas.checked(value)?;
            let equality = crate::case_cells::analyze(cas, function, value, singleton, budget)?;
            equality.check(
                cas,
                function,
                value,
                singleton,
                DomainContract::DefinedAndEqual,
                budget,
            )?;
            result
                .regions
                .extend(equality.regions.iter().map(|id| cas.handle(*id)));
            result.cells.push(equality.cells);
            require_point_premise(context, singleton, &domain, true, cas, premises)?;
        }
        Ownership::Absent => {
            let cells =
                crate::region_cache::partition(cas, &[singleton, cas.checked(&domain)?], budget)?;
            let point = cells.membership.first().ok_or_else(invalid)?;
            let defined = cells.membership.get(1).ok_or_else(invalid)?;
            if point
                .iter()
                .zip(defined)
                .any(|(point, defined)| *point && *defined)
            {
                return Err(invalid());
            }
            result.cells.push(cells);
            require_point_premise(context, singleton, &domain, false, cas, premises)?;
        }
    }
    result.regions.extend([cas.handle(singleton), domain]);
    Ok(result)
}

/// The recipe must record the point membership or exclusion application explicitly.
fn require_point_premise(
    context: &Handle,
    point: LogicalId,
    domain: &Handle,
    present: bool,
    cas: &Cas,
    premises: &[CheckedApplication],
) -> Result<(), Fault> {
    let expected = if present {
        Statement::Subset {
            subset: cas.handle(point),
            superset: domain.clone(),
        }
    } else {
        Statement::Disjoint {
            left: cas.handle(point),
            right: domain.clone(),
        }
    };
    if premises.iter().any(|premise| {
        premise.judgment.context == *context && premise.judgment.statement == expected
    }) {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// Construct the singleton inside the requested context using the boundary's exact axis.
pub(crate) fn singleton(
    cas: &mut Cas,
    context: LogicalId,
    boundary: LogicalId,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let boundary = cas.state.boundary(boundary)?.clone();
    let within = cas.state.context(context)?.admissible;
    let region = cas.state.region(within)?;
    if region.ambient != boundary.ambient || region.scope != boundary.scope {
        return Err(invalid());
    }
    let (coordinate, sort) = match &boundary.ambient {
        AmbientDomain::Real(key) => (
            key.clone(),
            cas.state.expression(boundary.location)?.sort.clone(),
        ),
        AmbientDomain::Integer(key) => (key.clone(), Sort::Value(AtomicSort::Integer)),
        _ => return Err(invalid()),
    };
    let coordinate = Draft::Expression(ExpressionDraft {
        space: None,
        header: Reference::Existing(cas.handle(boundary.header)),
        scope: boundary.scope.clone(),
        sort,
        operator: Operator::Free(coordinate),
        binding: None,
        operands: Vec::new(),
    });
    let coordinate = case_syntax::admit(cas, coordinate, budget)?;
    let predicate = Draft::Condition {
        header: Reference::Existing(cas.handle(boundary.header)),
        scope: boundary.scope,
        kind: PredicateKind::Equal,
        arguments: vec![
            Reference::Existing(coordinate),
            Reference::Existing(cas.handle(boundary.location)),
        ],
    };
    let predicate = case_syntax::admit(cas, predicate, budget)?;
    case_syntax::guarded(cas, within, predicate.logical_id(), budget)
}
