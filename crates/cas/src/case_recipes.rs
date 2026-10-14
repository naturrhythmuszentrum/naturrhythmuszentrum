// SPDX-License-Identifier: AGPL-3.0-only
//! Merge and ownership expansions contain only checked registered applications.

use crate::budget::Budget;
use crate::case_producer::{self, Plan};
use crate::case_syntax::{self, invalid};
use crate::domain_model::ContextDraft;
use crate::error::{Fault, FaultKind, InvalidCode, Location};
use crate::graph::{Cas, Draft, Handle, Reference};
use crate::proof_model::{Certificate, ConstructorLaw, DomainContract, Ownership, Statement};
use crate::rule::Primitive;

/// Merge by a complete cover and closed local equalities, retaining both exact domain theorems.
#[expect(
    clippy::too_many_arguments,
    reason = "both functions, explicit cover and independent ledgers are recipe inputs"
)]
pub(crate) fn merge(
    plan: &mut Plan<'_>,
    cas: &mut Cas,
    context: &Handle,
    function: &Handle,
    proposed: &Handle,
    regions: &[Handle],
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<Handle, Fault> {
    let (left, _) = case_producer::domain(plan, cas, context, function, generation, checker)?;
    let (right, _) = case_producer::domain(plan, cas, context, proposed, generation, checker)?;
    let domain = cas.handle(cas.state.context(cas.checked(context)?)?.admissible);
    let conclusion = Statement::Equality {
        left: function.clone(),
        right: proposed.clone(),
        domain: domain.clone(),
        domain_contract: DomainContract::SameDefinedDomain,
    };
    case_producer::family(regions.len(), generation)?;
    let mut premises = vec![cover(
        plan, cas, context, &domain, regions, generation, checker,
    )?];
    for region in regions {
        premises.push(local(
            plan,
            cas,
            context,
            region,
            &conclusion,
            generation,
            checker,
        )?);
    }
    let eliminated = plan.push(
        cas,
        context,
        Primitive::CaseEliminate,
        conclusion.clone(),
        &premises,
        Certificate::None,
        generation,
        checker,
    )?;
    plan.push(
        cas,
        context,
        Primitive::CaseCongruence,
        conclusion,
        &[left, right, eliminated],
        Certificate::None,
        generation,
        checker,
    )
}

/// An exact failed cover has a witnessed cell, including omitted rational singletons.
pub(crate) fn cover(
    plan: &mut Plan<'_>,
    cas: &mut Cas,
    context: &Handle,
    domain: &Handle,
    members: &[Handle],
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<Handle, Fault> {
    plan.push(
        cas,
        context,
        Primitive::RegionDecide,
        Statement::Cover {
            domain: domain.clone(),
            members: members.to_vec(),
        },
        &[],
        Certificate::None,
        generation,
        checker,
    )
    .map_err(|fault| match fault.kind {
        FaultKind::Invalid(InvalidCode::Rule) => {
            Fault::invalid(InvalidCode::CoverageHole, Location::new("case cover"))
        }
        _ => fault,
    })
}

/// Each member is proved in its actual local frame and closed with construction evidence.
fn local(
    plan: &mut Plan<'_>,
    cas: &mut Cas,
    context: &Handle,
    region: &Handle,
    conclusion: &Statement,
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<Handle, Fault> {
    let frame = case_syntax::admit(
        cas,
        Draft::Context(ContextDraft::Local {
            parent: Reference::Existing(context.clone()),
            region: Reference::Existing(region.clone()),
            hypotheses: Vec::new(),
        }),
        generation,
    )?;
    let local = cas.handle(cas.state.context(frame.logical_id())?.admissible);
    let conclusion = crate::context_check::restricted(conclusion, &local);
    let child = plan.push(
        cas,
        &frame,
        Primitive::CaseCongruence,
        conclusion.clone(),
        &[],
        Certificate::None,
        generation,
        checker,
    )?;
    let construction = plan.push(
        cas,
        context,
        Primitive::RegionConstruct,
        Statement::Constructor {
            region: local,
            law: ConstructorLaw::Membership,
        },
        &[],
        Certificate::None,
        generation,
        checker,
    )?;
    plan.push(
        cas,
        context,
        Primitive::ContextClose,
        conclusion,
        &[child, construction],
        Certificate::Close { frame },
        generation,
        checker,
    )
}

/// Ownership is function-indexed and requires recorded exact domain and singleton evidence.
pub(crate) fn ownership(
    plan: &mut Plan<'_>,
    cas: &mut Cas,
    context: &Handle,
    function: &Handle,
    boundary: &Handle,
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<Handle, Fault> {
    let (domain_proof, domain) =
        case_producer::domain(plan, cas, context, function, generation, checker)?;
    let context_id = cas.checked(context)?;
    let boundary_id = cas.checked(boundary)?;
    let singleton = crate::case_select::singleton(cas, context_id, boundary_id, generation)?;
    if crate::case_obligations::empty(cas, singleton, generation)? {
        return Err(invalid());
    }
    let analysis = crate::case_cells::analyze(
        cas,
        function.logical_id(),
        function.logical_id(),
        singleton,
        generation,
    )?;
    let value = analysis.point_value(cas, function.logical_id(), singleton, generation)?;
    let (ownership, membership) = match value {
        Some(value) => (
            Ownership::Present(cas.handle(value)),
            Statement::Subset {
                subset: cas.handle(singleton),
                superset: domain,
            },
        ),
        None => (
            Ownership::Absent,
            Statement::Disjoint {
                left: cas.handle(singleton),
                right: domain,
            },
        ),
    };
    let membership = plan.push(
        cas,
        context,
        Primitive::RegionDecide,
        membership,
        &[],
        Certificate::None,
        generation,
        checker,
    )?;
    plan.push(
        cas,
        context,
        Primitive::CaseSelect,
        Statement::Ownership {
            function: function.clone(),
            boundary: boundary.clone(),
            value: ownership,
        },
        &[domain_proof, membership],
        Certificate::None,
        generation,
        checker,
    )
}
