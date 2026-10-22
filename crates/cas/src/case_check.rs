// SPDX-License-Identifier: AGPL-3.0-only
//! Independent reconstruction of the five native case theorem schemas.

use crate::budget::Budget;
use crate::case_model::CaseCertificate;
use crate::case_syntax::{self, invalid};
use crate::error::Fault;
use crate::graph::{Cas, Handle, LogicalId};
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, DomainContract, ScopedConditions, Statement,
};
use crate::rule::Primitive;

/// Reconstruct every claimed role and condition before accepting untrusted certificate data.
pub(crate) fn check(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let Certificate::Cases(actual) = &draft.certificate else {
        return Err(invalid());
    };
    if !draft.substitutions.is_empty() {
        return Err(invalid());
    }
    let (expected, mut conditions) = reconstruct(cas, draft, premises, budget)?;
    if **actual != expected {
        return Err(invalid());
    }
    for premise in premises {
        if premise.judgment.context != draft.context {
            return Err(invalid());
        }
        crate::statement::merge(cas, &mut conditions, &premise.conditions, budget)?;
    }
    Ok(conditions)
}

/// Construct schema data from syntax and already checked premises, never from an acceptance flag.
pub(crate) fn reconstruct(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(CaseCertificate, ScopedConditions), Fault> {
    if premises
        .iter()
        .any(|premise| premise.judgment.context != draft.context)
    {
        return Err(invalid());
    }
    if crate::solving::family::native::handles(cas, &draft.conclusion)? {
        return crate::solving::family::native::reconstruct(cas, draft, premises, budget);
    }
    let context = cas.checked(&draft.context)?;
    let within = cas.state.context(context)?.admissible;
    let certificate = match draft.rule {
        Primitive::CaseOrdered => {
            let Statement::Elaboration {
                source,
                result,
                domain,
            } = &draft.conclusion
            else {
                return Err(invalid());
            };
            let source = cas.checked(source)?;
            if cas.state.source_form(source)?.domain != cas.checked(domain)? {
                return Err(invalid());
            }
            subset(cas, cas.checked(domain)?, within, budget)?;
            let built = crate::case_ordered::construct(cas, context, source, premises, budget)?;
            if built.result != *result {
                return Err(invalid());
            }
            return Ok((built.certificate, built.conditions));
        }
        Primitive::CaseDomain => domain(cas, &draft.conclusion, within, budget)?,
        Primitive::CaseCongruence => agreement(cas, &draft.conclusion, within, premises, budget)?,
        Primitive::CaseSelect => crate::case_select::reconstruct(cas, draft, premises, budget)?,
        Primitive::CaseEliminate => {
            crate::case_eliminate::reconstruct(cas, draft, premises, budget)?
        }
        _ => return Err(invalid()),
    };
    Ok((certificate, ScopedConditions::default()))
}

/// Certify both the actual domain and compatibility of all associations reached there.
fn domain(
    cas: &mut Cas,
    statement: &Statement,
    within: LogicalId,
    budget: &mut Budget,
) -> Result<CaseCertificate, Fault> {
    if let Statement::PredicateFact { region, predicate } = statement {
        let crate::graph::Vertex::Condition(predicate) = cas.state.node(cas.checked(predicate)?)?
        else {
            return Err(invalid());
        };
        if predicate.kind != crate::model::PredicateKind::Defined || predicate.arguments.len() != 1
        {
            return Err(invalid());
        }
        let function = *predicate.arguments.first().ok_or_else(invalid)?;
        let region = cas.checked(region)?;
        subset(cas, region, within, budget)?;
        let analysis = crate::case_cells::analyze(cas, function, function, region, budget)?;
        analysis.check(
            cas,
            function,
            function,
            region,
            DomainContract::DefinedAndEqual,
            budget,
        )?;
        return Ok(from_analysis(cas, &analysis));
    }
    let Statement::DefinedDomain { function, domain } = statement else {
        return Err(invalid());
    };
    let function = cas.checked(function)?;
    let claimed = cas.checked(domain)?;
    let analysis = crate::case_cells::analyze(cas, function, function, within, budget)?;
    analysis.check(
        cas,
        function,
        function,
        within,
        DomainContract::SameDefinedDomain,
        budget,
    )?;
    let actual = analysis.domain(function)?;
    let comparison = crate::region_cache::partition(cas, &[actual, claimed], budget)?;
    if comparison.membership.first() != comparison.membership.get(1) {
        return Err(invalid());
    }
    let mut certificate = from_analysis(cas, &analysis);
    certificate
        .regions
        .extend([cas.handle(actual), cas.handle(claimed)]);
    certificate.cells.push(comparison);
    Ok(certificate)
}

/// Local agreement keeps endpoint identities and both definedness directions explicit.
fn agreement(
    cas: &mut Cas,
    statement: &Statement,
    within: LogicalId,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<CaseCertificate, Fault> {
    let (left, right, region, contract) = match statement {
        Statement::Equality {
            left,
            right,
            domain,
            domain_contract,
        } => (
            cas.checked(left)?,
            cas.checked(right)?,
            cas.checked(domain)?,
            *domain_contract,
        ),
        Statement::Compatibility {
            left,
            right,
            region,
        } => {
            let first = cas.state.arm(cas.checked(left)?)?.clone();
            let second = cas.state.arm(cas.checked(right)?)?.clone();
            let region = cas.checked(region)?;
            subset(cas, region, first.region, budget)?;
            subset(cas, region, second.region, budget)?;
            (
                first.value,
                second.value,
                region,
                DomainContract::SameDefinedDomain,
            )
        }
        _ => return Err(invalid()),
    };
    subset(cas, region, within, budget)?;
    refinement_cover(cas, left, right, premises, budget)?;
    let analysis = crate::case_cells::analyze(cas, left, right, region, budget)?;
    analysis.check(cas, left, right, region, contract, budget)?;
    let mut certificate = from_analysis(cas, &analysis);
    for endpoint in [left, right] {
        let arms = case_syntax::canonical(cas, &cas.state.expression(endpoint)?.arms, budget)?;
        certificate
            .arms
            .extend(arms.into_iter().map(|id| cas.handle(id)));
    }
    Ok(certificate)
}

/// A newly refined binary result carries the explicit complete cover used by its recipe.
fn refinement_cover(
    cas: &Cas,
    left: LogicalId,
    right: LogicalId,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let before = cas.state.expression(left)?;
    let after = cas.state.expression(right)?;
    let Some(domain) = after.domain else {
        return Ok(());
    };
    if before.domain.is_some()
        || before.operands.is_empty()
        || matches!(before.operator, crate::model::Operator::NamedCase(_))
    {
        return Ok(());
    }
    let mut native = false;
    for id in case_syntax::order(cas, &before.operands, budget)? {
        native |= cas.state.expression(id)?.domain.is_some();
    }
    if !native {
        return Ok(());
    }
    let members = after
        .arms
        .iter()
        .map(|id| cas.state.arm(*id).map(|arm| arm.region))
        .collect::<Result<Vec<_>, _>>()?;
    let members = case_syntax::canonical(cas, &members, budget)?;
    for premise in premises {
        if let Statement::Cover {
            domain: actual,
            members: actual_members,
        } = &premise.judgment.statement
        {
            let ids: Vec<_> = actual_members.iter().map(Handle::logical_id).collect();
            if actual.logical_id() == domain
                && case_syntax::canonical(cas, &ids, budget)? == members
            {
                return Ok(());
            }
        }
    }
    Err(invalid())
}

/// Retain every exact partition row in its independently reconstructed structural order.
pub(crate) fn from_analysis(cas: &Cas, analysis: &crate::case_cells::Analysis) -> CaseCertificate {
    CaseCertificate {
        regions: analysis.regions.iter().map(|id| cas.handle(*id)).collect(),
        cells: vec![analysis.cells.clone()],
        ..CaseCertificate::default()
    }
}

/// Check subset without silently treating an unknown region as empty.
pub(crate) fn subset(
    cas: &Cas,
    subset: LogicalId,
    superset: LogicalId,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if subset == superset {
        return Ok(());
    }
    let cells = crate::region_cache::partition(cas, &[subset, superset], budget)?;
    let left = cells.membership.first().ok_or_else(invalid)?;
    let right = cells.membership.get(1).ok_or_else(invalid)?;
    if left.iter().zip(right).all(|(left, right)| !left || *right) {
        Ok(())
    } else {
        Err(invalid())
    }
}
