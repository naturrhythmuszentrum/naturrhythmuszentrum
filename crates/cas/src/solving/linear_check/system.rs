// SPDX-License-Identifier: AGPL-3.0-only
//! Original affine equations and independently reconstructed rank metadata.

use super::matrix::Matrix;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Handle, LogicalId};
use crate::model::PredicateKind;
use crate::solving::coefficients::storage::{reserve, vector};
use crate::solving::elementary::evaluate::Evaluation;
use crate::solving::elementary::proof::{AT, coordinates, invalid, problem};
use crate::solving::elementary::transport::unknowns;
use crate::solving::{CertificateExactLinearEquivalence, StatementLinearSystem};

/// Flatten the complete conjunction without silently dropping inactive or repeated equations.
fn equations(
    evaluation: &Evaluation<'_>,
    root: LogicalId,
    budget: &mut Budget,
) -> Result<Vec<(LogicalId, LogicalId)>, Fault> {
    let mut pending = vector(1, budget)?;
    pending.push(root);
    let mut result = vector(0, budget)?;
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        match crate::solving::elementary::evidence::predicate(evaluation.cas, id)? {
            (PredicateKind::And, arguments) => {
                reserve(&mut pending, arguments.len(), budget)?;
                pending.extend(arguments.iter().rev());
            }
            (PredicateKind::Equal, [left, right]) => {
                reserve(&mut result, 1, budget)?;
                result.push((*left, *right));
            }
            _ => return Err(invalid()),
        }
    }
    Ok(result)
}

/// Rebuild A x - b and compare each row with its original equation.
pub(crate) fn reconstruct(
    evaluation: &Evaluation<'_>,
    handle: &Handle,
    matrix: &Matrix,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let original = problem(evaluation.cas, handle)?;
    let values = unknowns(evaluation, original, budget)?;
    if values.len().checked_add(1) != Some(matrix.columns) {
        return Err(invalid());
    }
    let equations = equations(evaluation, original.predicate.logical_id(), budget)?;
    if equations.len() != matrix.rows {
        return Err(invalid());
    }
    for (row, (left, right)) in equations.iter().enumerate() {
        let mut reconstructed = matrix
            .at(row, matrix.columns.checked_sub(1).ok_or_else(invalid)?)?
            .neg(&evaluation.engine, budget)?;
        for (column, value) in values.iter().enumerate() {
            reconstructed = reconstructed.add(
                &matrix
                    .at(row, column)?
                    .mul(value, &evaluation.engine, budget)?,
                &evaluation.engine,
                budget,
            )?;
        }
        let expected = evaluation.expression(*left, true, budget)?.sub(
            &evaluation.expression(*right, true, budget)?,
            &evaluation.engine,
            budget,
        )?;
        evaluation.zero(
            &reconstructed.sub(&expected, &evaluation.engine, budget)?,
            budget,
        )?;
    }
    Ok(())
}

/// The last augmented pivot is a contradiction; free columns are exactly the missing coefficient pivots.
pub(crate) fn check(
    evaluation: &Evaluation<'_>,
    claim: &StatementLinearSystem,
    certificate: &CertificateExactLinearEquivalence,
    original_matrix: &Matrix,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if (&claim.reduced, &claim.region, certificate.augmented_columns)
        != (&certificate.reduced, &certificate.region, 1)
    {
        return Err(invalid());
    }
    let original = problem(evaluation.cas, &claim.problem)?;
    let count = u32::try_from(
        coordinates(evaluation.cas, &original.unknowns)?
            .coordinates
            .len(),
    )
    .map_err(|_| invalid())?;
    let rank = certificate
        .pivots
        .iter()
        .filter(|column| **column < count)
        .count();
    if (claim.rank, claim.augmented_rank)
        != (
            u32::try_from(rank).map_err(|_| invalid())?,
            u32::try_from(certificate.pivots.len()).map_err(|_| invalid())?,
        )
    {
        return Err(invalid());
    }
    let mut free = vector(usize::try_from(count).map_err(|_| invalid())?, budget)?;
    for column in 0..count {
        budget.work(1, AT)?;
        if !certificate.pivots.contains(&column) {
            free.push(column);
        }
    }
    if free != claim.free_columns {
        return Err(invalid());
    }
    reconstruct(evaluation, &claim.problem, original_matrix, budget)
}
