// SPDX-License-Identifier: AGPL-3.0-only
//! Typed sensitivity buffers retain their original differential and output coordinate meanings.

use crate::solve::scratch::{Attempt, Run, Stage};
use crate::statistics::{Affine, Matrix};
use crate::{Extent, OperationError};
use nrz_scalar::{Enclosure, Finite, Scalar};

/// Allocate both parameter channels only after the complete equation schedule has been admitted.
pub(super) fn zeros<P: Scalar + 'static>(
    run: &Run,
    centers: Stage,
    truth: Stage,
    direction: Option<usize>,
    extent: Extent,
) -> Result<Matrix<P>, OperationError> {
    let mut values = run.buffer(centers, direction)?;
    let mut enclosures = run.buffer(truth, direction)?;
    for _ in 0..extent.entries() {
        values.push(P::ZERO)?;
        enclosures.push(Enclosure::singleton(Finite::<P>::ZERO))?;
    }
    Ok(Matrix {
        extent,
        centers: values,
        truth: enclosures,
    })
}
/// Nominal and differential RHS meanings are distinguished by their original direction index.
pub(super) fn rhs<P: Scalar + 'static>(
    attempt: Attempt<'_>,
    extent: Extent,
) -> Result<Matrix<P>, OperationError> {
    zeros(
        attempt.run,
        Stage::SensitivityRightHandSide,
        Stage::SensitivityRightHandSideTruth,
        Some(attempt.index),
        extent,
    )
}
/// Same-format copies preserve exact endpoints and keep Jacobian owners live through publication.
pub(super) fn transfer<P: Scalar + 'static, Q: Scalar + 'static>(
    map: Affine<P>,
    run: &Run,
) -> Result<Affine<Q>, OperationError> {
    Ok(Affine {
        jacobian: matrix(
            map.jacobian,
            run,
            Stage::OutputJacobianCenters,
            Stage::OutputJacobianTruth,
            true,
        )?,
        offsets: matrix(
            map.offsets,
            run,
            Stage::OutputOffsetCenters,
            Stage::OutputOffsetTruth,
            false,
        )?,
    })
}
/// The two endpoint components are transferred independently without a new numerical theorem.
fn matrix<P: Scalar + 'static, Q: Scalar + 'static>(
    input: Matrix<P>,
    run: &Run,
    center_stage: Stage,
    truth_stage: Stage,
    candidate: bool,
) -> Result<Matrix<Q>, OperationError> {
    let mut centers = run.buffer(center_stage, None)?;
    let mut truth = run.buffer(truth_stage, None)?;
    for &value in &input.centers {
        centers.push(crate::conversion::same_format(value)?)?;
    }
    for entry in &input.truth {
        truth.push(Enclosure::new(
            crate::conversion::same_format(entry.lower().get())?,
            crate::conversion::same_format(entry.upper().get())?,
        )?)?;
    }
    let extent = input.extent;
    drop(input);
    if candidate {
        centers = centers.into_candidate()?;
        truth = truth.into_candidate()?;
    }
    Ok(Matrix {
        extent,
        centers,
        truth,
    })
}
