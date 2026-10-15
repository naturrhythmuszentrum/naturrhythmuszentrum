// SPDX-License-Identifier: AGPL-3.0-only
//! Exact scalar inverse identities justify nonzero pivots in the same interpretation.

use super::evaluate::Evaluation;
use super::proof::{AT, invalid};
use super::value::Value;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::Handle;
use crate::model::Operator;
use crate::proof_model::{Certificate, CheckedApplication, DomainContract, Statement};
use crate::solving::{
    CertificateCoefficientIdentity, MapTarget, SolveCertificate, SolveDraft, SolveStatement,
};

/// Only checked equality evidence with an explicit, exactly matching scalar map is eligible.
fn equality<'a>(
    evaluation: &Evaluation<'_>,
    context: &Handle,
    proof: &'a CheckedApplication,
    budget: &mut Budget,
) -> Result<Option<&'a CertificateCoefficientIdentity>, Fault> {
    budget.work(1, AT)?;
    let Certificate::Solve(certificate) = &proof.certificate else {
        return Ok(None);
    };
    let SolveCertificate::CoefficientIdentity(identity) = certificate.as_ref() else {
        return Ok(None);
    };
    if (&proof.judgment.context, &identity.region) != (context, evaluation.region) {
        return Ok(None);
    }
    let endpoints = match &proof.judgment.statement {
        Statement::Equality {
            left,
            right,
            domain,
            domain_contract: DomainContract::DefinedAndEqual,
        } if domain == evaluation.region => (left, right),
        Statement::Solve(statement) => match statement.as_ref() {
            SolveStatement::CoefficientEquality(value)
                if (&value.map, &value.region) == (&identity.map, evaluation.region) =>
            {
                (&value.left, &value.right)
            }
            _ => return Ok(None),
        },
        _ => return Ok(None),
    };
    if endpoints != (&identity.left, &identity.right) {
        return Err(invalid());
    }
    let SolveDraft::CoefficientMap(map) =
        crate::solving::graph::object(&evaluation.cas.state, identity.map.logical_id())?
    else {
        return Err(invalid());
    };
    let frame = evaluation.cas.state.context(context.logical_id())?;
    let MapTarget::ScalarAmbient { header, .. } = &map.target else {
        return Ok(None);
    };
    if (
        &map.source,
        &map.region,
        header.logical_id(),
        frame.admissible,
    ) != (
        evaluation.engine.domain,
        evaluation.region,
        frame.header_id,
        evaluation.region.logical_id(),
    ) {
        return Ok(None);
    }
    Ok(Some(identity))
}

/// Compare the exact map provenance separately from the normalized pivot value.
fn matches(
    evaluation: &Evaluation<'_>,
    pivot: &Value,
    candidate: &Value,
    map: &Handle,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    budget.work(
        crate::graph::size(
            pivot
                .maps
                .len()
                .checked_add(candidate.maps.len())
                .ok_or_else(invalid)?,
            AT,
        )?
        .max(1),
        AT,
    )?;
    if pivot
        .maps
        .iter()
        .chain(&candidate.maps)
        .any(|used| *used != map.logical_id())
        || pivot.maps.len() != candidate.maps.len()
    {
        return Ok(false);
    }
    super::product::decides(
        evaluation.zero(&pivot.sub(candidate, &evaluation.engine, budget)?, budget),
    )
}

/// Product syntax identifies the proposed inverse; both factors retain strict definedness.
fn product(
    evaluation: &Evaluation<'_>,
    pivot: &Value,
    identity: &CertificateCoefficientIdentity,
    left: &Handle,
    right: &Handle,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let expression = evaluation.cas.state.expression(left.logical_id())?;
    let (Operator::Mul, [first, second]) = (&expression.operator, expression.operands.as_slice())
    else {
        return Ok(false);
    };
    let unit = Value::integer(1, &evaluation.engine, budget)?;
    let other = evaluation.expression(right.logical_id(), true, budget)?;
    if !super::product::decides(
        evaluation.zero(&other.sub(&unit, &evaluation.engine, budget)?, budget),
    )? {
        return Ok(false);
    }
    if pivot
        .maps
        .iter()
        .all(|used| *used == identity.map.logical_id())
        && super::product::decides(
            evaluation.zero(&pivot.sub(&unit, &evaluation.engine, budget)?, budget),
        )?
    {
        return Ok(true);
    }
    let a = evaluation.expression(*first, true, budget)?;
    let b = evaluation.expression(*second, true, budget)?;
    if !matches(evaluation, pivot, &a, &identity.map, budget)?
        && !matches(evaluation, pivot, &b, &identity.map, budget)?
    {
        return Ok(false);
    }
    let multiplied = a.mul(&b, &evaluation.engine, budget)?;
    super::product::decides(
        evaluation.zero(&multiplied.sub(&unit, &evaluation.engine, budget)?, budget),
    )
}

/// A valid checked inverse identity is a field-scalar theorem, never an irreducibility assertion.
pub(crate) fn check(
    evaluation: &Evaluation<'_>,
    pivot: &Value,
    context: &Handle,
    proofs: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    if super::product::decides(evaluation.nonzero(pivot, budget))? {
        return Ok(());
    }
    for proof in proofs {
        let Some(identity) = equality(evaluation, context, proof, budget)? else {
            continue;
        };
        if product(
            evaluation,
            pivot,
            identity,
            &identity.left,
            &identity.right,
            budget,
        )? || product(
            evaluation,
            pivot,
            identity,
            &identity.right,
            &identity.left,
            budget,
        )? {
            return Ok(());
        }
    }
    Err(invalid())
}
