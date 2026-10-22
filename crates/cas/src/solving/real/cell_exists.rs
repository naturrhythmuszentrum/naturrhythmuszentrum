// SPDX-License-Identifier: AGPL-3.0-only
//! Count-one existence and uniqueness for the exact isolated Real selection.

use super::{AT, invalid};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::{AtomicSort, Field, Operator, PredicateKind, Sort};
use crate::proof_model::{ApplicationDraft, CheckedApplication, Statement};
use crate::solving::elementary::{evidence, proof::premise, selected};
use crate::solving::{CertificateRealCellExists, RealEndpoint, SelectionMode, SolveStatement};

/// The cell itself is the isolating constraint; no additional unproved predicate is admitted.
fn predicate(cas: &Cas, certificate: &CertificateRealCellExists) -> Result<(), Fault> {
    let relation = selected::relation(cas, &certificate.relation)?;
    if relation.ambient != Field::Real || relation.region != certificate.region {
        return Err(invalid());
    }
    let (PredicateKind::Equal, [left, right]) =
        evidence::predicate(cas, certificate.predicate.logical_id())?
    else {
        return Err(invalid());
    };
    if left != right {
        return Err(invalid());
    }
    let expression = cas.state.expression(*left)?;
    let outer = expression.scope.get(..relation.scope.len());
    let binder = expression.scope.last();
    if expression.operator != (Operator::Bound { depth: 0, slot: 0 })
        || expression.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
        || expression.header != relation.header.logical_id()
        || outer != Some(relation.scope.as_slice())
        || expression.scope.len() != relation.scope.len().saturating_add(1)
        || binder.map(Vec::as_slice) != Some([AtomicSort::Scalar(Field::Real)].as_slice())
    {
        return Err(invalid());
    }
    Ok(())
}

/// The earlier theorem must count precisely this relation and cell once on the same stratum.
fn count(
    certificate: &CertificateRealCellExists,
    premises: &[CheckedApplication],
) -> Result<(), Fault> {
    let Statement::Solve(statement) = &premise(premises, certificate.count)?.judgment.statement
    else {
        return Err(invalid());
    };
    let SolveStatement::RealRootCount(count) = statement.as_ref() else {
        return Err(invalid());
    };
    if count.relation != certificate.relation || count.region != certificate.region {
        return Err(invalid());
    }
    let mut found = false;
    for (cell, number) in count.cells.iter().zip(&count.counts) {
        if cell == &certificate.cell {
            if found || *number != 1 {
                return Err(invalid());
            }
            found = true;
        }
    }
    if !found {
        return Err(invalid());
    }
    Ok(())
}

/// Prior count evidence includes all endpoint definedness, order and root-status checks.
fn endpoints(
    cas: &Cas,
    certificate: &CertificateRealCellExists,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let cell = super::count::cell(cas, &certificate.cell)?;
    if cell.region != certificate.region {
        return Err(invalid());
    }
    let mut finite = 0;
    for endpoint in [&cell.left, &cell.right] {
        super::count::independent(
            cas,
            endpoint,
            &certificate.relation,
            &certificate.region,
            budget,
        )?;
        if matches!(endpoint, RealEndpoint::RealExpression(_)) {
            finite += 1;
        }
    }
    if certificate.definedness.len() != finite
        || certificate
            .definedness
            .iter()
            .any(|slot| *slot != certificate.count)
        || certificate.order != [certificate.count]
    {
        return Err(invalid());
    }
    Ok(())
}

/// Ordinary existence and uniqueness are consequences of the same exact isolated-cell theorem.
fn conclusion(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateRealCellExists,
) -> Result<(), Fault> {
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let selection = match statement.as_ref() {
        SolveStatement::RealCellExists(claim) => {
            if claim.relation != certificate.relation
                || claim.cell != certificate.cell
                || claim.predicate != certificate.predicate
                || claim.region != certificate.region
            {
                return Err(invalid());
            }
            return Ok(());
        }
        SolveStatement::RootExistence(claim) if claim.region == certificate.region => {
            &claim.relation
        }
        SolveStatement::SelectionUnique(claim) if claim.region == certificate.region => {
            &claim.selection
        }
        _ => return Err(invalid()),
    };
    selection_matches(cas, selection, certificate)
}

/// A differently constrained selection cannot borrow this cell's inhabitedness.
fn selection_matches(
    cas: &Cas,
    handle: &Handle,
    certificate: &CertificateRealCellExists,
) -> Result<(), Fault> {
    let selection = selected::selection(cas, handle)?;
    if selection.mode != SelectionMode::Isolated
        || selection.cell.as_ref() != Some(&certificate.cell)
        || selection.relations != std::slice::from_ref(&certificate.relation)
        || selection.predicate != certificate.predicate
        || selection.interval.is_some()
        || selection.convention.is_some()
    {
        return Err(invalid());
    }
    Ok(())
}

/// Count-one means a unique root; every structural and scoped premise is replayed first.
pub(super) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateRealCellExists,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(1, AT)?;
    super::region(cas, &draft.context, &certificate.region)?;
    count(certificate, premises)?;
    predicate(cas, certificate)?;
    endpoints(cas, certificate, budget)?;
    conclusion(cas, draft, certificate)
}
