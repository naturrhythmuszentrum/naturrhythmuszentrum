// SPDX-License-Identifier: AGPL-3.0-only
//! Finite membership and distinctness turn an exact whole-line count into ordinary coverage.

use super::{AT, invalid};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{ExpressionAttachment, Handle};
use crate::model::{Field, Operator};
use crate::proof_model::{DomainContract, Statement};
use crate::solving::elementary::{evaluate::Evaluation, product, proof, selected, transport};
use crate::solving::{CertificateRealRootCount, StatementCoverage};

/// A prior defined equality identifies the original equation up to one independently nonzero scalar.
fn source(
    evaluation: &Evaluation<'_>,
    certificate: &CertificateRealRootCount,
    coverage: &StatementCoverage,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let [slot] = certificate.identities.as_slice() else {
        return Err(invalid());
    };
    let Statement::Equality {
        left,
        right,
        domain,
        domain_contract: DomainContract::DefinedAndEqual,
    } = &proof::premise(evaluation.premises, *slot)?
        .judgment
        .statement
    else {
        return Err(invalid());
    };
    if domain != evaluation.region {
        return Err(invalid());
    }
    let original = proof::problem(evaluation.cas, &coverage.problem)?;
    if original.ambient != Field::Real {
        return Err(invalid());
    }
    let actual = product::equation(evaluation, original, budget)?;
    let claimed = evaluation.expression(left.logical_id(), true, budget)?;
    evaluation.zero(&actual.sub(&claimed, &evaluation.engine, budget)?, budget)?;
    let scaled = evaluation.cas.state.expression(right.logical_id())?;
    let [scalar, polynomial] = scaled.operands.as_slice() else {
        return Err(invalid());
    };
    if scaled.operator != Operator::Mul {
        return Err(invalid());
    }
    evaluation.nonzero(&evaluation.expression(*scalar, true, budget)?, budget)?;
    let unknowns = transport::unknowns(evaluation, original, budget)?;
    let [unknown] = unknowns.as_slice() else {
        return Err(invalid());
    };
    let relation = selected::relation(evaluation.cas, &certificate.relation)?;
    let expected = evaluation.root_equation(unknown, relation, budget)?;
    let supplied = evaluation.expression(*polynomial, true, budget)?;
    evaluation.zero(
        &expected.sub(&supplied, &evaluation.engine, budget)?,
        budget,
    )
}

/// Distinct count cells separate their matching isolated selections without a numerical root comparison.
fn cell(
    evaluation: &Evaluation<'_>,
    entry: &Handle,
    certificate: &CertificateRealRootCount,
) -> Result<Option<Handle>, Fault> {
    let value = evaluation.cas.state.expression(entry.logical_id())?;
    let (Operator::RootValue { slot: 0 }, Some(ExpressionAttachment::SelectedRoot(handle))) =
        (&value.operator, &value.attachment)
    else {
        return Ok(None);
    };
    let selection = selected::selection(evaluation.cas, &evaluation.cas.handle(*handle))?;
    if selection.relations != std::slice::from_ref(&certificate.relation) {
        return Ok(None);
    }
    Ok(selection
        .cell
        .as_ref()
        .filter(|cell| certificate.cells.contains(cell))
        .cloned())
}

/// Checked pairwise differences or disjoint checked cells prevent duplicate tuples from inflating the count.
fn distinct(
    evaluation: &Evaluation<'_>,
    entries: &[Handle],
    certificate: &CertificateRealRootCount,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for (index, first) in entries.iter().enumerate() {
        for second in entries.iter().skip(index + 1) {
            budget.work(1, AT)?;
            if let (Some(left), Some(right)) = (
                cell(evaluation, first, certificate)?,
                cell(evaluation, second, certificate)?,
            ) && left != right
            {
                continue;
            }
            let left = evaluation.expression(first.logical_id(), true, budget)?;
            let right = evaluation.expression(second.logical_id(), true, budget)?;
            evaluation.nonzero(&left.sub(&right, &evaluation.engine, budget)?, budget)?;
        }
    }
    Ok(())
}

/// A finite subset of the original roots with exactly their checked cardinality exhausts them.
pub(super) fn check(
    evaluation: &Evaluation<'_>,
    certificate: &CertificateRealRootCount,
    coverage: &StatementCoverage,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if coverage.region != certificate.region {
        return Err(invalid());
    }
    let original = proof::problem(evaluation.cas, &coverage.problem)?;
    crate::solving::elementary::member::contained(
        evaluation.cas,
        &coverage.region,
        &original.region,
        budget,
    )?;
    source(evaluation, certificate, coverage, budget)?;
    let entries = product::entries(evaluation.cas, coverage, budget)?;
    product::membership(evaluation, original, &entries, budget)?;
    distinct(evaluation, &entries, certificate, budget)?;
    let mut count = certificate
        .exterior_left
        .checked_add(certificate.exterior_right)
        .ok_or_else(invalid)?;
    for number in &certificate.counts {
        count = count.checked_add(*number).ok_or_else(invalid)?;
    }
    if usize::try_from(count).map_err(|_| invalid())? != entries.len() {
        return Err(invalid());
    }
    Ok(())
}
