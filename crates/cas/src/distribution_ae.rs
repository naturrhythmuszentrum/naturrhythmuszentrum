// SPDX-License-Identifier: AGPL-3.0-only
//! Finite-exception transport of regular distributions, without pointwise equality.

use crate::budget::Budget;
use crate::distribution_model::{DistributionCertificate, DistributionStatement};
use crate::distribution_syntax::invalid;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::Operator;
use crate::proof_model::{ApplicationDraft, Certificate, CheckedApplication, Statement};

/// Both local-integrability premises and the exact complement cover are mandatory.
pub(crate) fn check(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::Distribution(value) = &draft.conclusion else {
        return Err(invalid());
    };
    let DistributionStatement::Equality { left, right, space } = value.as_ref() else {
        return Err(invalid());
    };
    let first = representative(cas, left)?;
    let second = representative(cas, right)?;
    crate::distribution_regular::evidence(&first, space, premises)?;
    crate::distribution_regular::evidence(&second, space, premises)?;
    let expected = certificate(cas, &first, &second, space, budget)?;
    if draft.certificate != Certificate::Distribution(Box::new(expected)) {
        return Err(invalid());
    }
    Ok(())
}

/// Reconstruct an exact finite exceptional set and compare every open complement cell.
pub(crate) fn certificate(
    cas: &mut Cas,
    left: &Handle,
    right: &Handle,
    space: &Handle,
    budget: &mut Budget,
) -> Result<DistributionCertificate, Fault> {
    let definition = cas.state.space(space.logical_id())?.clone();
    let analysis = crate::case_cells::analyze(
        cas,
        left.logical_id(),
        right.logical_id(),
        definition.domain,
        budget,
    )?;
    let first = analysis.polynomial_cells(
        cas,
        left.logical_id(),
        definition.domain,
        &definition.coordinate,
        budget,
    )?;
    let second = analysis.polynomial_cells(
        cas,
        right.logical_id(),
        definition.domain,
        &definition.coordinate,
        budget,
    )?;
    if first != second {
        return Err(invalid());
    }
    let domain_row = analysis
        .regions
        .iter()
        .position(|id| *id == definition.domain)
        .ok_or_else(invalid)?;
    let row = analysis
        .cells
        .membership
        .get(domain_row)
        .ok_or_else(invalid)?;
    let mut exceptions = Vec::new();
    for (index, point) in analysis.cells.cuts.iter().enumerate() {
        if row.get(
            index
                .checked_mul(2)
                .and_then(|n| n.checked_add(1))
                .ok_or_else(invalid)?,
        ) == Some(&true)
        {
            exceptions
                .try_reserve(1)
                .map_err(|_| Fault::allocation(crate::distribution_syntax::AT))?;
            exceptions.push(point.clone());
        }
    }
    Ok(DistributionCertificate::AlmostEverywhere {
        exceptions,
        partition: analysis.cells,
    })
}

/// Only regular embeddings can lose finite point information by this rule.
pub(crate) fn representative(cas: &Cas, value: &Handle) -> Result<Handle, Fault> {
    let expression = cas.state.expression(value.logical_id())?;
    if expression.operator != Operator::RegularEmbed {
        return Err(invalid());
    }
    Ok(cas.handle(*expression.operands.first().ok_or_else(invalid)?))
}
