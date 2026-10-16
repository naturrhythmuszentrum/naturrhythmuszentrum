// SPDX-License-Identifier: AGPL-3.0-only
//! Restore a norm ball entrywise and retain the new numerical error exactly once.

use super::{SolveContract, SolveError};
use crate::binding::buffer;
use crate::coordinate_numeric::LoweredMap;
use crate::publication::Payload;
use crate::{ConversionEvidence, OperationError, OutputReservation};
use nrz_accuracy::Nonnegative;
use nrz_scalar::{ArithmeticContext, Scalar};

/// Complete output coordinates and independently interpreted norm summaries.
#[derive(Debug)]
pub(crate) struct Restored<P: Scalar> {
    /// Finite solution coordinates in the final output interpretation.
    centers: Vec<P>,
    /// Complete conservative entrywise total bounds.
    totals: Vec<P>,
    /// Overlapping represented-center numerical bounds.
    evaluation: Vec<P>,
    /// Joint Frobenius total bound, before any entrywise expansion loses tightness.
    pub(crate) total_norm: P,
    /// Joint Frobenius numerical-only bound in the same output coordinates.
    pub(crate) numerical_norm: P,
    /// Exact restoration map and lowered coefficients, if arithmetic restoration occurred.
    pub(crate) evidence: Option<ConversionEvidence>,
    /// Admitted scalar rounding revision.
    pub(crate) revision: u32,
}

/// Identity unit-reference restoration is exact; other maps use checked coefficient enclosures.
pub(crate) fn restore<P: Scalar + 'static>(
    contract: &SolveContract,
    centers: Vec<P>,
    total: P,
    numerical: P,
    reservation: OutputReservation,
    context: &ArithmeticContext,
) -> Result<Restored<P>, OperationError> {
    if centers.len() != reservation.extent().entries() {
        return Err(SolveError::Contract.into());
    }
    if let Some(conversion) = &contract.restoration {
        let map = LoweredMap::new(&conversion.map, conversion.literals, context)?;
        return mapped(centers, total, numerical, reservation, &map, context);
    }
    let mut totals = buffer(centers.len())?;
    let mut evaluation = buffer(centers.len())?;
    totals.resize(centers.len(), total);
    evaluation.resize(centers.len(), numerical);
    Ok(Restored {
        centers,
        totals,
        evaluation,
        total_norm: total,
        numerical_norm: numerical,
        evidence: None,
        revision: context.revision(),
    })
}

/// Uniform homogeneous affine restoration preserves the joint norm via its scale plus new error.
fn mapped<P: Scalar>(
    centers: Vec<P>,
    total: P,
    numerical: P,
    reservation: OutputReservation,
    map: &LoweredMap<P>,
    context: &ArithmeticContext,
) -> Result<Restored<P>, OperationError> {
    let scale =
        Nonnegative::new(map.scale.enclosure().abs().upper().get()).map_err(SolveError::from)?;
    let carried = Nonnegative::new(numerical)
        .map_err(SolveError::from)?
        .scale(scale, context)
        .map_err(SolveError::from)?;
    let mut result = Restored {
        centers: buffer(centers.len())?,
        totals: buffer(centers.len())?,
        evaluation: buffer(centers.len())?,
        total_norm: total,
        numerical_norm: numerical,
        evidence: Some(ConversionEvidence::new(
            0,
            centers.len(),
            reservation.coverage(),
            map,
        )),
        revision: context.revision(),
    };
    let mut errors = buffer(centers.len())?;
    for center in centers {
        let converted = map.evaluate(center, total, context)?;
        let error = Nonnegative::new(converted.evaluation).map_err(SolveError::from)?;
        result.centers.push(converted.center);
        result.totals.push(converted.total);
        result
            .evaluation
            .push(carried.add(error, context).map_err(SolveError::from)?.get());
        errors.push(converted.evaluation);
    }
    let new_error = Nonnegative::new(
        nrz_scalar::norm_upper(&errors, context)
            .map_err(SolveError::from)?
            .get(),
    )
    .map_err(SolveError::from)?;
    result.total_norm = Nonnegative::new(total)
        .map_err(SolveError::from)?
        .scale(scale, context)
        .map_err(SolveError::from)?
        .add(new_error, context)
        .map_err(SolveError::from)?
        .get();
    result.numerical_norm = carried
        .add(new_error, context)
        .map_err(SolveError::from)?
        .get();
    Ok(result)
}

impl<P: Scalar + 'static> Restored<P> {
    /// Transfer sealed same-format coordinates into the ordinary private transaction payload.
    pub(crate) fn payload<Q: Scalar + 'static>(
        self,
        reservation: OutputReservation,
    ) -> Result<Payload<Q>, OperationError> {
        Ok(Payload {
            extent: reservation.extent(),
            target: reservation.stamp(),
            centers: transfer(self.centers)?,
            bounds: Some(transfer(self.totals)?),
            evaluation: Some(transfer(self.evaluation)?),
            contributions: None,
        })
    }
}

/// Same-format transfer never silently narrows a certified upper endpoint.
fn transfer<P: Scalar + 'static, Q: Scalar + 'static>(
    values: Vec<P>,
) -> Result<Vec<Q>, OperationError> {
    let mut result = buffer(values.len())?;
    for value in values {
        result.push(crate::conversion::same_format(value)?);
    }
    Ok(result)
}
