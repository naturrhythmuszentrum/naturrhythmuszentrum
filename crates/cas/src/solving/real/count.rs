// SPDX-License-Identifier: AGPL-3.0-only
//! Sturm variation differences, disjoint cells and complete exterior accounting.

use super::{count_signs::Signs, invalid};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::Field;
use crate::proof_model::{ApplicationDraft, CheckedApplication, Statement};
use crate::solving::coefficients::{arithmetic::Engine, proof, storage};
use crate::solving::elementary::{evaluate::Evaluation, selected};
use crate::solving::{
    CertificateRealRootCount, MapTarget, RealCellData, RealEndpoint, SolveDraft, SolveStatement,
};

/// Borrow a structural cell; admission supplies neither endpoint order nor existence.
pub(super) fn cell<'a>(cas: &'a Cas, handle: &Handle) -> Result<&'a RealCellData<Handle>, Fault> {
    match crate::solving::graph::object(&cas.state, cas.checked(handle)?)? {
        SolveDraft::RealCell(cell) => Ok(cell),
        _ => Err(invalid()),
    }
}

/// Endpoints must precede the selected root mathematically, even if an earlier selection has the same relation.
pub(super) fn independent(
    cas: &Cas,
    endpoint: &RealEndpoint<Handle>,
    relation: &Handle,
    region: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let RealEndpoint::RealExpression(handle) = endpoint else {
        return Ok(());
    };
    super::sign::scalar(cas, handle, region)?;
    super::closure::check(cas, std::iter::once(handle), Some(relation), budget)
}

/// Exact finite subject matching precedes all arithmetic and theorem use.
fn subject(draft: &ApplicationDraft, certificate: &CertificateRealRootCount) -> Result<(), Fault> {
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let SolveStatement::RealRootCount(claim) = statement.as_ref() else {
        return Err(invalid());
    };
    if claim.relation != certificate.relation
        || claim.region != certificate.region
        || claim.cells != certificate.cells
        || claim.counts != certificate.counts
        || claim.exterior_left != certificate.exterior_left
        || claim.exterior_right != certificate.exterior_right
    {
        return Err(invalid());
    }
    Ok(())
}

/// Real field, same-sort map, exact square-free/gcd identities and sign evidence are mandatory.
pub(super) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateRealRootCount,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    subject(draft, certificate)?;
    super::region(cas, &draft.context, &certificate.region)?;
    super::closure::check(cas, std::iter::once(&certificate.relation), None, budget)?;
    let relation = selected::relation(cas, &certificate.relation)?;
    if relation.ambient != Field::Real || relation.region != certificate.region {
        return Err(invalid());
    }
    let map = proof::mapping(
        cas,
        &relation.map,
        &draft.context,
        &certificate.region,
        budget,
    )?;
    if !matches!(&map.target, MapTarget::ScalarAmbient { field: Field::Real, header }
        if *header == relation.header)
    {
        return Err(invalid());
    }
    proof::identity_map(cas, map, premises, budget)?;
    let _chain = super::sturm::check(cas, &relation.polynomial, certificate, premises, budget)?;
    let stored = storage::polynomial(&cas.state, &relation.polynomial)?;
    let evaluation = Evaluation {
        cas,
        engine: Engine {
            state: &cas.state,
            domain: &stored.domain,
        },
        region: &certificate.region,
        premises,
        free: &[],
        bound: &[],
    };
    let mut signs = Signs {
        evaluation,
        map,
        slots: &certificate.signs,
        cursor: 0,
    };
    for scale in &certificate.rescalings {
        let value = signs.evaluation.mapped(scale, map, budget)?;
        if signs.next(&value, budget)? != 4 {
            return Err(invalid());
        }
    }
    let below = signs.infinity(&certificate.chain, true, budget)?;
    let above = signs.infinity(&certificate.chain, false, budget)?;
    inventory(&mut signs, certificate, (below, above), budget)?;
    if signs.cursor != signs.slots.len() {
        return Err(invalid());
    }
    Ok(())
}

/// Cell counts plus two exterior counts must exhaust the whole-line Sturm count.
fn inventory(
    signs: &mut Signs<'_>,
    certificate: &CertificateRealRootCount,
    infinities: (u32, u32),
    budget: &mut Budget,
) -> Result<(), Fault> {
    if certificate.cells.is_empty() || certificate.cells.len() != certificate.counts.len() {
        return Err(invalid());
    }
    let mut previous: Option<&RealCellData<Handle>> = None;
    let mut total = certificate
        .exterior_left
        .checked_add(certificate.exterior_right)
        .ok_or_else(invalid)?;
    for (index, (handle, claimed)) in certificate
        .cells
        .iter()
        .zip(&certificate.counts)
        .enumerate()
    {
        let cell = cell(signs.evaluation.cas, handle)?;
        if cell.region != certificate.region {
            return Err(invalid());
        }
        independent(
            signs.evaluation.cas,
            &cell.left,
            &certificate.relation,
            &certificate.region,
            budget,
        )?;
        independent(
            signs.evaluation.cas,
            &cell.right,
            &certificate.relation,
            &certificate.region,
            budget,
        )?;
        if let Some(previous) = previous {
            disjoint(signs, previous, cell, budget)?;
        }
        signs.ordered(&cell.left, &cell.right, budget)?;
        let left = signs.endpoint(&certificate.chain, &cell.left, infinities, budget)?;
        let right = signs.endpoint(&certificate.chain, &cell.right, infinities, budget)?;
        if count(cell, left, right)? != *claimed {
            return Err(invalid());
        }
        if index == 0 {
            let variation = if cell.left_open { left.1 } else { left.0 };
            if infinities.0.checked_sub(variation) != Some(certificate.exterior_left) {
                return Err(invalid());
            }
        }
        if index + 1 == certificate.cells.len() {
            let variation = if cell.right_open { right.0 } else { right.1 };
            if variation.checked_sub(infinities.1) != Some(certificate.exterior_right) {
                return Err(invalid());
            }
        }
        total = total.checked_add(*claimed).ok_or_else(invalid)?;
        previous = Some(cell);
    }
    if infinities.0.checked_sub(infinities.1) != Some(total) {
        return Err(invalid());
    }
    Ok(())
}

/// Empty intervals, duplicate singletons and overlapping closed endpoint ownership are refused.
fn disjoint(
    signs: &mut Signs<'_>,
    previous: &RealCellData<Handle>,
    next: &RealCellData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if matches!(previous.right, RealEndpoint::PositiveInfinity)
        || matches!(next.left, RealEndpoint::NegativeInfinity)
    {
        return Err(invalid());
    }
    if previous.right == next.left && !previous.right_open && !next.left_open {
        return Err(invalid());
    }
    signs.ordered(&previous.right, &next.left, budget)
}

/// One-sided counts exclude roots at open ends; closed singletons contribute exactly their root status.
fn count(
    cell: &RealCellData<Handle>,
    left: (u32, u32, bool),
    right: (u32, u32, bool),
) -> Result<u32, Fault> {
    if cell.left == cell.right {
        if cell.left_open || cell.right_open || left != right {
            return Err(invalid());
        }
        return Ok(u32::from(left.2));
    }
    left.1
        .checked_sub(right.0)
        .and_then(|value| value.checked_add(u32::from(!cell.left_open && left.2)))
        .and_then(|value| value.checked_add(u32::from(!cell.right_open && right.2)))
        .ok_or_else(invalid)
}
