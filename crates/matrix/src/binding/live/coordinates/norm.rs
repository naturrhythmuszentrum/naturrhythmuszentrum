// SPDX-License-Identifier: AGPL-3.0-only
//! Named mixed-coordinate radius norms retain their explicit positive per-entry metric.

use std::rc::Rc;

use super::{CoordinateContract, CoordinateError, CoordinateReading, lowering, numeric};
use crate::{Coverage, Heap, HeapError};
use nrz_accuracy::{BoundVectorRef, NormKind, NormSummary};
use nrz_bindings::{
    Budget,
    readings::{CoordinateDirection, CoordinateNormalization, algebra::Location},
};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Deterministic and numerical-only norm bounds in one explicitly retained mixed-coordinate metric.
#[derive(Clone, Debug)]
pub struct CoordinateNorm<P: Scalar> {
    /// The immutable restored snapshot whose component radii were summarized.
    source: CoordinateReading<P>,
    /// Actual exact metric and all enclosed divisors used by the calculation.
    metric: Rc<lowering::Lowered<P>>,
    /// Ordered positive physical reference definitions in the named metric.
    references: CoordinateNormalization,
    /// Total norm, already including the numerical component.
    total: NormSummary<P>,
    /// Numerical-only norm for the original nominal target.
    error: NormSummary<P>,
    /// Original represented-input or declared-family assurance scope.
    coverage: Coverage,
}

impl<P: Scalar> CoordinateNorm<P> {
    /// Complete original restored snapshot, including selected storage versions and both radius arrays.
    #[must_use]
    pub const fn source(&self) -> &CoordinateReading<P> {
        &self.source
    }
    /// Exact reference image whose scale enclosures supplied both outward divisions.
    #[must_use]
    pub fn image(&self) -> &nrz_bindings::readings::CoordinateImage {
        &self.metric.image
    }
    /// Scale and offset enclosures in metric entry order; only the positive scale divides radii.
    #[must_use]
    pub fn coefficients(&self) -> &[[crate::ConstantCoefficient<P>; 2]] {
        &self.metric.coefficients
    }

    /// Exact reference definitions in `N_(D,p)`, including order and physical kinds.
    #[must_use]
    pub const fn references(&self) -> &CoordinateNormalization {
        &self.references
    }
    /// Complete deterministic upper bound with its norm name and entry count.
    #[must_use]
    pub const fn total(&self) -> NormSummary<P> {
        self.total
    }
    /// Separate numerical-only upper bound, already covered by the total.
    #[must_use]
    pub const fn error(&self) -> NormSummary<P> {
        self.error
    }
    /// Assurance scope inherited from the selected input family.
    #[must_use]
    pub const fn coverage(&self) -> Coverage {
        self.coverage
    }
}

impl Heap {
    /// Bound restored component radii in `N_(D,p)` using outward division by declared references.
    ///
    /// `One`, `Euclidean` and `Infinity` name vector norms. Each positive difference reference
    /// belongs to the supplied metric; point origins never contribute to radius normalization.
    /// # Errors
    /// Refuses stale inputs, unrestored coordinates, incompatible kinds or arithmetic revisions,
    /// `Frobenius`, non-rational references, zero-containing divisors, overflow and finite limits.
    pub fn coordinate_norm<P: Scalar + 'static>(
        &self,
        input: &CoordinateReading<P>,
        metric: &CoordinateContract,
        kind: NormKind,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<CoordinateNorm<P>, CoordinateError> {
        self.validate_reading(&input.0.source)?;
        if input.0.direction != CoordinateDirection::Restore
            || input.0.revision != context.revision()
        {
            return Err(CoordinateError::Identity);
        }
        if kind == NormKind::Frobenius {
            return Err(CoordinateError::UnsupportedNorm);
        }
        check(input.0.contract.frame(), metric.frame())?;
        let lowered = lowering::prepare::<P>(
            &input.0.source,
            metric.restoration(),
            metric.0.literals,
            context,
            budget,
        )?;
        let lowered_bytes = lowered.bytes()?;
        let bytes = input
            .0
            .bytes
            .checked_add(metric.0.bytes)
            .and_then(|value| value.checked_add(lowered_bytes))
            .ok_or(HeapError::ExtentOverflow)?;
        let (total, error) =
            budget.with_retained_payload(bytes, Location::new("coordinate norm"), |budget| {
                evaluate(input, &lowered, kind, context, budget)
            })?;
        numeric::output::<P>(0, bytes, budget)?;
        Ok(CoordinateNorm {
            source: input.clone(),
            metric: Rc::new(lowered),
            references: metric.frame().clone(),
            total,
            error,
            coverage: input.0.coverage,
        })
    }
}

/// References may change deliberately; the ordered physical kinds may not.
fn check(
    input: &CoordinateNormalization,
    metric: &CoordinateNormalization,
) -> Result<(), CoordinateError> {
    if input.context() != metric.context() || input.entries().len() != metric.entries().len() {
        return Err(CoordinateError::Identity);
    }
    for (source, target) in input.entries().iter().zip(metric.entries()) {
        source
            .descriptor()
            .compatible_kind(target.descriptor())
            .map_err(nrz_bindings::readings::ReadingError::from)?;
    }
    Ok(())
}

/// Reserve both normalized radius arrays and a fixed schedule before the first numerical division.
fn evaluate<P: Scalar>(
    input: &CoordinateReading<P>,
    lowered: &lowering::Lowered<P>,
    kind: NormKind,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<(NormSummary<P>, NormSummary<P>), CoordinateError> {
    let count = input.centers().len();
    let length = u64::try_from(count).map_err(|_| HeapError::ExtentOverflow)?;
    budget.work(
        length
            .checked_mul(32)
            .and_then(|value| value.checked_add(16))
            .ok_or(HeapError::ExtentOverflow)?,
        Location::new("coordinate norm arithmetic"),
    )?;
    numeric::reserve::<P>(count, 5, budget)?;
    let mut totals = crate::binding::buffer(count)?;
    let mut errors = crate::binding::buffer(count)?;
    for (index, pair) in lowered.coefficients.iter().enumerate() {
        let divisor = pair[0].enclosure();
        totals.push(
            Enclosure::singleton(Finite::new(input.totals()[index])?)
                .div(divisor, context)?
                .upper()
                .get(),
        );
        errors.push(
            Enclosure::singleton(Finite::new(input.errors()[index])?)
                .div(divisor, context)?
                .upper()
                .get(),
        );
    }
    Ok((
        BoundVectorRef::new(&totals)?.norm(kind, context)?,
        BoundVectorRef::new(&errors)?.norm(kind, context)?,
    ))
}
