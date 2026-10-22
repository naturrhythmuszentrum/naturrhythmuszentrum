// SPDX-License-Identifier: AGPL-3.0-only
//! A finite ordered derivation; no supplied sign vector is trusted.

use super::{AT, invalid};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::{AtomicSort, Field, Sort};
use crate::proof_model::{ApplicationDraft, CheckedApplication, Statement};
use crate::solving::{CertificateRealSign, RealSignConclusion, RealSignStep, SolveStatement};

/// A sign set uses negative, zero and positive bits, respectively.
pub(super) const fn mask(value: RealSignConclusion) -> u8 {
    match value {
        RealSignConclusion::Negative => 1,
        RealSignConclusion::Zero => 2,
        RealSignConclusion::Positive => 4,
        RealSignConclusion::NonNegative => 6,
        RealSignConclusion::NonPositive => 3,
        RealSignConclusion::NonZero => 5,
    }
}

/// Negation reverses strict order and fixes zero.
pub(super) const fn negate(value: u8) -> u8 {
    ((value & 1) << 2) | (value & 2) | ((value & 4) >> 2)
}

/// Weakening may add possibilities; it may never discard an unexcluded sign.
pub(super) const fn entails(actual: u8, claim: RealSignConclusion) -> Result<(), Fault> {
    if actual == 0 || actual & !mask(claim) != 0 {
        return Err(invalid());
    }
    Ok(())
}

/// Every operand has exactly the original Real frame and no distribution space.
pub(super) fn scalar(cas: &Cas, subject: &Handle, region: &Handle) -> Result<(), Fault> {
    let term = cas.state.expression(cas.checked(subject)?)?;
    let region = cas.state.region(cas.checked(region)?)?;
    if term.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
        || term.header != region.header
        || term.scope != region.scope
        || term.space.is_some()
    {
        return Err(invalid());
    }
    Ok(())
}

/// All indices point backward, so evaluating the supplied order is a bounded DAG walk.
fn arguments<'a>(
    step: &RealSignStep,
    earlier: &'a [RealSignStep],
    budget: &mut Budget,
) -> Result<Vec<&'a RealSignStep>, Fault> {
    let mut result = crate::solving::coefficients::storage::vector(step.arguments.len(), budget)?;
    for index in &step.arguments {
        budget.work(1, AT)?;
        result.push(
            earlier
                .get(usize::try_from(*index).map_err(|_| invalid())?)
                .ok_or_else(invalid)?,
        );
    }
    Ok(result)
}

/// Replay every node, including unused nodes, before comparing the exact requested conclusion.
pub(super) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateRealSign,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    super::region(cas, &draft.context, &certificate.region)?;
    super::closure::check(
        cas,
        certificate.steps.iter().map(|step| &step.subject),
        None,
        budget,
    )?;
    budget.check(
        BudgetKind::ConditionSteps,
        crate::graph::size(certificate.steps.len(), AT)?,
        AT,
    )?;
    for (index, step) in certificate.steps.iter().enumerate() {
        budget.solving(BudgetKind::IsolationSteps, 1, AT)?;
        let earlier = certificate.steps.get(..index).ok_or_else(invalid)?;
        let arguments = arguments(step, earlier, budget)?;
        super::sign_rules::check(cas, &certificate.region, step, &arguments, premises, budget)?;
    }
    let root = certificate
        .steps
        .get(usize::try_from(certificate.root).map_err(|_| invalid())?)
        .ok_or_else(invalid)?;
    if root.subject != certificate.subject {
        return Err(invalid());
    }
    match &draft.conclusion {
        Statement::Solve(statement) => {
            let SolveStatement::RealSign(claim) = statement.as_ref() else {
                return Err(invalid());
            };
            if claim.subject != certificate.subject || claim.region != certificate.region {
                return Err(invalid());
            }
            entails(mask(root.conclusion), claim.conclusion)
        }
        Statement::Equality {
            left,
            right,
            domain,
            domain_contract: crate::proof_model::DomainContract::DefinedAndEqual,
        } if *domain == certificate.region && root.conclusion == RealSignConclusion::Zero => {
            let difference = cas.state.expression(root.subject.logical_id())?;
            if difference.operator != crate::model::Operator::Sub
                || difference.operands != [left.logical_id(), right.logical_id()]
            {
                return Err(invalid());
            }
            Ok(())
        }
        Statement::PredicateFact { predicate, region } if *region == certificate.region => {
            super::sign_evidence::predicate(
                cas,
                predicate,
                &root.subject,
                mask(root.conclusion),
                budget,
            )
        }
        _ => Err(invalid()),
    }
}
