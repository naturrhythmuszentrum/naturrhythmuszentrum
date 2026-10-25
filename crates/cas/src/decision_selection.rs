// SPDX-License-Identifier: AGPL-3.0-only
//! Certified selection cites the retained finite partition and checks the selected value locally.

use crate::budget::Budget;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, LogicalId, Vertex};
use crate::proof_model::{ApplicationDraft, Certificate, CheckedApplication, Statement};

/// Exact selection identity and retained proof accounting.
const AT: Location = Location::new("certified decision selection");

/// Changed functions, regions or membership evidence cannot reuse this partition.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, AT)
}

/// Read an authenticated introduction without reconstructing its already checked partition.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the selection producer and checker cite the same retained partition"
)]
pub(crate) fn retained_option(
    cas: &Cas,
    context: &Handle,
    budget: &mut Budget,
) -> Result<Option<(LogicalId, LogicalId)>, Fault> {
    let frame = cas.state.context(cas.checked(context)?)?;
    let Some(introduction) = frame.introduction else {
        return Ok(None);
    };
    budget.work(4, AT)?;
    let Vertex::Proof(application) = cas.state.node(introduction)? else {
        return Err(invalid());
    };
    let Vertex::ScopedApplication(proof) = cas.state.node(*application)? else {
        return Err(invalid());
    };
    let Certificate::Decision(evidence) = &proof.certificate else {
        return Err(invalid());
    };
    Ok(Some((
        evidence.partition.function.logical_id(),
        evidence.cut.association.logical_id(),
    )))
}

/// Retain the exact certified slot; ordinary selection keeps its existing path and costs.
#[expect(
    clippy::redundant_pub_crate,
    reason = "native selection consumes the retained introduction"
)]
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<Option<LogicalId>, Fault> {
    let Some((function, association)) = retained_option(cas, &draft.context, budget)? else {
        return Ok(None);
    };
    let frame = cas.state.context(cas.checked(&draft.context)?)?;
    let Statement::Equality {
        left,
        right,
        domain,
        ..
    } = &draft.conclusion
    else {
        return Err(invalid());
    };
    if left.logical_id() != function
        || domain.logical_id() != frame.admissible
        || cas.checked(right)? != cas.state.arm(association)?.value
    {
        return Err(invalid());
    }
    let [member] = premises else {
        return Err(invalid());
    };
    let expected = Statement::Subset {
        subset: cas.handle(frame.admissible),
        superset: cas.handle(frame.restriction.ok_or_else(invalid)?),
    };
    let used = crate::domain_model::HypothesisRef {
        context: draft.context.clone(),
        slot: 0,
    };
    budget.work(
        crate::graph::size(member.conditions.hypotheses.len(), AT)?.saturating_add(4),
        AT,
    )?;
    if member.judgment.context != draft.context
        || member.judgment.statement != expected
        || !member.conditions.residuals.is_empty()
        || !member.conditions.hypotheses.contains(&used)
    {
        return Err(invalid());
    }
    Ok(Some(association))
}

/// Uniform absence of the selected value remains a refutation under extra assumptions.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the request classifier distinguishes uniform absence from conditional countercells"
)]
pub(crate) fn undefined(
    cas: &mut Cas,
    request: &crate::request_model::Request,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let crate::task::Task::Select { region } = &request.task else {
        return Ok(false);
    };
    let Some((function, association)) = retained_option(cas, &request.context, budget)? else {
        return Ok(false);
    };
    let [input] = request.inputs.as_slice() else {
        return Err(invalid());
    };
    if region.logical_id()
        != cas
            .state
            .context(cas.checked(&request.context)?)?
            .admissible
        || cas.state.occurrence(cas.checked(&input.root)?)?.expression != function
    {
        return Ok(false);
    }
    let value = cas.state.arm(association)?.value;
    let order = crate::case_syntax::order(cas, &[value], budget)?;
    let domains = crate::case_syntax::domains(cas, &order, region.logical_id(), budget)?;
    match crate::case_obligations::empty(cas, *domains.get(&value).ok_or_else(invalid)?, budget) {
        Err(fault) if crate::case_obligations::unknown(&fault) => Ok(false),
        result => result,
    }
}
