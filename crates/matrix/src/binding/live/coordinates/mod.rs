// SPDX-License-Identifier: AGPL-3.0-only
//! Read-only mixed coordinate execution retains exact maps, selected copies and all deterministic errors.

mod input;
mod lowering;
mod norm;
mod numeric;

pub use norm::CoordinateNorm;

use super::{LiveBindingError, ReadingBinding};
use crate::{Coverage, Heap, HeapError};
use nrz_bindings::{
    Budget,
    readings::{
        CoordinateDirection, CoordinateImage, CoordinateNormalization, ReadingError, algebra::Cas,
    },
};
use nrz_scalar::{ArithmeticContext, LiteralLimits, Scalar};
use std::rc::Rc;

/// A refused exact or numerical coordinate evaluation; no partial result becomes available.
#[derive(Clone, Debug, thiserror::Error)]
pub enum CoordinateError {
    /// A selected value or companion is missing, stale or outside its checked inventory.
    #[error("coordinate input: {0}")]
    Live(#[from] LiveBindingError),
    /// The numerical heap refused access or allocation.
    #[error("coordinate storage: {0}")]
    Heap(#[from] HeapError),
    /// Typed references, exact correspondence or rational coefficient export refused.
    #[error("coordinate definition: {0}")]
    Reading(#[from] ReadingError),
    /// The cumulative exact and numerical lowering allowance was exhausted.
    #[error("coordinate resources: {0}")]
    Exact(#[from] nrz_bindings::readings::algebra::Fault),
    /// Outward arithmetic or finite coefficient decoding refused.
    #[error("coordinate arithmetic: {0}")]
    Arithmetic(#[from] nrz_scalar::ArithmeticFailure),
    /// A supplied nonnegative radius or outward bound operation refused.
    #[error("coordinate radius: {0}")]
    Bound(#[from] nrz_accuracy::AccuracyFailure),
    /// This evaluation requires an identity stored-coordinate reading and coherent physical input.
    #[error("unsupported coordinate reading map")]
    UnsupportedReading,
    /// Statistical or nonidentity companion maps require a separately checked execution recipe.
    #[error("unsupported coordinate companion map")]
    UnsupportedCompanion,
    /// The recorded direction, exact references or arithmetic revision differs.
    #[error("coordinate execution identity mismatch")]
    Identity,
    /// This mixed vector metric supports only one, Euclidean and infinity norms.
    #[error("unsupported mixed coordinate norm")]
    UnsupportedNorm,
}

/// Exact mixed normalization and restoration prepared before numerical input access.
#[derive(Clone, Debug)]
pub struct CoordinateContract(Rc<Contract>);

/// Shared exact maps are retained once across all read-only evaluations and restorations.
#[derive(Debug)]
struct Contract {
    /// Original per-entry reference and origin identities with both inverse certificates.
    frame: CoordinateNormalization,
    /// Independently checked normalization image.
    normalize: CoordinateImage,
    /// Independently checked coherent restoration image.
    restore: CoordinateImage,
    /// Explicit coefficient import policy.
    literals: LiteralLimits,
    /// Complete retained exact declaration bound for nested numerical operations.
    bytes: u64,
}

impl CoordinateContract {
    /// Prepare both exact images under one caller ledger before borrowing any float.
    /// # Errors
    /// Refuses foreign exact authority, unavailable correspondence or cumulative exact limits.
    pub fn new(
        cas: &mut Cas,
        frame: &CoordinateNormalization,
        literals: LiteralLimits,
        budget: &mut Budget,
    ) -> Result<Self, CoordinateError> {
        let normalize = frame.image(cas, CoordinateDirection::Normalize, budget)?;
        let bytes = normalize.retained_payload_bytes();
        let restore = budget.with_retained_payload(
            bytes,
            nrz_bindings::readings::algebra::Location::new("coordinate contract"),
            |budget| frame.image(cas, CoordinateDirection::Restore, budget),
        )?;
        let bytes = bytes
            .checked_add(restore.retained_payload_bytes())
            .ok_or(HeapError::ExtentOverflow)?;
        budget.check(
            nrz_bindings::readings::algebra::limits::BudgetKind::OutputBytes,
            bytes,
            nrz_bindings::readings::algebra::Location::new("coordinate contract"),
        )?;
        Ok(Self(Rc::new(Contract {
            frame: frame.clone(),
            normalize,
            restore,
            literals,
            bytes,
        })))
    }
    /// Every original physical coordinate, positive reference and optional point origin.
    #[must_use]
    pub fn frame(&self) -> &CoordinateNormalization {
        &self.0.frame
    }
    /// Exact normalization image and complete original correspondence key.
    #[must_use]
    pub fn normalization(&self) -> &CoordinateImage {
        &self.0.normalize
    }
    /// Exact restoration image; point offsets never enter radius propagation.
    #[must_use]
    pub fn restoration(&self) -> &CoordinateImage {
        &self.0.restore
    }
}

/// One complete private result of a live coordinate reading, with all numerical coverage retained.
#[derive(Clone, Debug)]
pub struct CoordinateReading<P: Scalar>(Rc<Snapshot<P>>);

/// Shared immutable arrays and exact evidence preserve one evaluated snapshot across derived summaries.
#[derive(Debug)]
struct Snapshot<P: Scalar> {
    /// Previous immutable numerical image, retained by restoration without copying its arrays.
    previous: Option<CoordinateReading<P>>,
    /// Conservative complete payload of this result and its retained numerical parent.
    bytes: u64,
    /// Selected original storage copies and their complete dependency inventory.
    source: ReadingBinding<P>,
    /// Original typed normalization, retained unchanged through restoration.
    contract: CoordinateContract,
    /// Complete center, total and numerical-only arrays in the declared entry order.
    payload: numeric::Entries<P>,
    /// Exact image and enclosed coefficients actually executed.
    lowered: Rc<lowering::Lowered<P>>,
    /// Distinguish dimensionless normalized coordinates from restored coherent coordinates.
    direction: CoordinateDirection,
    /// Retained represented-input or declared deterministic family coverage.
    coverage: Coverage,
    /// Scalar enclosure theorem revision used by this result.
    revision: u32,
}

impl<P: Scalar> CoordinateReading<P> {
    /// Previous normalized snapshot, with the actual coefficient enclosures and numerical arrays.
    #[must_use]
    pub fn previous(&self) -> Option<&Self> {
        self.0.previous.as_ref()
    }

    /// Ordered finite center coordinates; interpretation follows `Self::direction` and `Self::contract`.
    #[must_use]
    pub fn centers(&self) -> &[P] {
        &self.0.payload.centers
    }
    /// Complete deterministic bounds, already covering the separate numerical error.
    #[must_use]
    pub fn totals(&self) -> &[P] {
        &self.0.payload.totals
    }
    /// Numerical-only radii relative to the original nominal input target.
    #[must_use]
    pub fn errors(&self) -> &[P] {
        &self.0.payload.errors
    }
    /// Actual ordered stored copies; duplicate storage never implies a fresh physical source.
    #[must_use]
    pub fn source(&self) -> &ReadingBinding<P> {
        &self.0.source
    }
    /// Full physical meanings and original exact maps.
    #[must_use]
    pub fn contract(&self) -> &CoordinateContract {
        &self.0.contract
    }
    /// Normalized or restored coordinate interpretation of all three arrays.
    #[must_use]
    pub fn direction(&self) -> CoordinateDirection {
        self.0.direction
    }
    /// Original deterministic assurance scope, independent of computed rounding error.
    #[must_use]
    pub fn coverage(&self) -> Coverage {
        self.0.coverage
    }
    /// Exact per-entry image that covers all lowered scales, offsets and result entries.
    #[must_use]
    pub fn image(&self) -> &CoordinateImage {
        &self.0.lowered.image
    }
    /// Coefficient enclosures in scale/offset order for each logical coordinate.
    #[must_use]
    pub fn coefficients(&self) -> &[[nrz_scalar::DecodedLiteral<P>; 2]] {
        &self.0.lowered.coefficients
    }
}

impl Heap {
    /// Execute an identity stored-coordinate mixed reading in its declared positive references.
    ///
    /// This read-only result retains its selected storage versions and exact per-entry coverage.
    /// Coefficients and all three output arrays are prepared privately before a result is returned.
    /// # Errors
    /// Refuses stale inputs, changed references, noncoherent input measures, nonidentity readings,
    /// unsupported statistical companions, non-rational export, overflow and finite resources.
    pub fn normalize_reading<P: Scalar + 'static>(
        &self,
        binding: &ReadingBinding<P>,
        contract: &CoordinateContract,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<CoordinateReading<P>, CoordinateError> {
        self.validate_reading(binding)?;
        input::check(binding, contract, budget)?;
        let lowered = lowering::prepare::<P>(
            binding,
            &contract.0.normalize,
            contract.0.literals,
            context,
            budget,
        )?;
        let bytes = contract
            .0
            .bytes
            .checked_add(lowered.bytes)
            .ok_or(HeapError::ExtentOverflow)?;
        let (payload, coverage) = budget.with_retained_payload(
            bytes,
            nrz_bindings::readings::algebra::Location::new("coordinate execution"),
            |budget| {
                let (input, coverage) = input::capture(self, binding, budget)?;
                Ok::<_, CoordinateError>((
                    numeric::evaluate(&input, &lowered.coefficients, context, budget)?,
                    coverage,
                ))
            },
        )?;
        let bytes = numeric::output::<P>(payload.centers.len(), bytes, budget)?;
        Ok(CoordinateReading(Rc::new(Snapshot {
            previous: None,
            bytes,
            source: binding.clone(),
            contract: contract.clone(),
            payload,
            lowered: Rc::new(lowered),
            direction: CoordinateDirection::Normalize,
            coverage,
            revision: context.revision(),
        })))
    }

    /// Restore every center, deterministic total and prior numerical error to coherent physical units.
    /// # Errors
    /// Refuses stale selected copies, repeated restoration, changed arithmetic revision,
    /// unavailable rational coefficients, overflow and finite resource limits atomically.
    pub fn restore_reading<P: Scalar + 'static>(
        &self,
        input: &CoordinateReading<P>,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<CoordinateReading<P>, CoordinateError> {
        self.validate_reading(&input.0.source)?;
        if input.0.direction != CoordinateDirection::Normalize
            || input.0.revision != context.revision()
        {
            return Err(CoordinateError::Identity);
        }
        let lowered = lowering::prepare::<P>(
            &input.0.source,
            &input.0.contract.0.restore,
            input.0.contract.0.literals,
            context,
            budget,
        )?;
        let bytes = input
            .0
            .bytes
            .checked_add(lowered.bytes)
            .ok_or(HeapError::ExtentOverflow)?;
        let payload = budget.with_retained_payload(
            bytes,
            nrz_bindings::readings::algebra::Location::new("coordinate restoration"),
            |budget| numeric::evaluate(&input.0.payload, &lowered.coefficients, context, budget),
        )?;
        let bytes = numeric::output::<P>(payload.centers.len(), bytes, budget)?;
        Ok(CoordinateReading(Rc::new(Snapshot {
            previous: Some(input.clone()),
            bytes,
            source: input.0.source.clone(),
            contract: input.0.contract.clone(),
            payload,
            lowered: Rc::new(lowered),
            direction: CoordinateDirection::Restore,
            coverage: input.0.coverage,
            revision: context.revision(),
        })))
    }
}

#[cfg(test)]
mod tests;
