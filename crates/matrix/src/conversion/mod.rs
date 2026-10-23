// SPDX-License-Identifier: AGPL-3.0-only
//! Checked stored-coordinate conversions and their transaction contracts.
//!
//! # Checked maps, lowering and coverage
//!
//! `ConversionContract::new` checks an exact bindings `CheckedMap` against separate input and output region contracts and the same frozen declaration snapshot. Shapes describe ordered difference coordinates; point roles and their origins remain in the map and actual value descriptor. `OperationDraft::convert` forms a closed affine recipe with one fresh output per input, and uses the same `Binding` → `Bound` → private `Candidate` → consuming publication path as copy. No external numerical callback can become a recipe. `CheckedOperation::carries_statistics` reports the implemented statistical recipe; `ConversionMode::Statistical` requires a complete carried source model at binding.
//!
//! A measure conversion can produce an explicitly described presentation representation; this does not relax the coherent-coordinate import constructor. Bindings named-reference normalization maps produce dimensionless regions, and their inverses restore the retained physical reference and point origin. Missing origins and relative references retain typed bindings causes. `ConversionContract::from_normalization_schema` exports `NormalizationMaps` through bindings under an explicit exact-arithmetic limit. Scalar X/Y/W maps can enter the existing conversion transaction; formal operator, RHS and solution maps retain ordered quotient evidence and exact inverses without inventing scalar descriptors. Each shape reference or origin is exported once per bundle and reused across every map that names that definition; re-created twins remain distinct. Center and difference maps stay separate. The original schema retains RHS correction signs and positions. Formal quotient heap storage and execution of the multi-input operator-origin RHS correction belong to a later block kernel; exporting these maps does not execute those products. Bindings refusals retain their typed `MapError` causes.
//!
//! Descriptor constants are exported only through bindings and decoded only through scalar. A shared contract cache retains separate binary32 and binary64 lowerings and their arithmetic revision across clones and repeated executions. Each scale and shift retains its exact map identity, finite candidate, outward exact-definition enclosure and coefficient displacement. Heap coordinates never enter descriptor arithmetic. The affine kernel encloses the exact map at each represented center and computes its discrepancy from the floating candidate. It then forms the outward total `sup(abs(scale enclosure))*old_radius + new_error` through accuracy bound arithmetic. The numerical-only companion separately propagates the input's numerical error by the same absolute scale and adds the new map discrepancy. That prior error is already covered by the input total, so the total does not add it again. Point offsets affect centers and new evaluation error, never propagated radii.
//!
//! Every converted output reserves a fresh value, a complete total-bound region and a distinct evaluation-error region. `PublishedBundle::evaluation` and `ValueView::evaluation` identify the latter; it overlaps the total and must not be added a second time. Both companion regions target the same fresh value identity/version and follow selections, copying, staleness checks, retirement and compaction. `Coverage::RepresentedInputs` remains represented coverage even when a computed bound exists. Existing declared physical assumptions remain `DeclaredBounds`. No computational radius is relabeled as a measurement assumption or statistical uncertainty.
//!
//! `RecipeEvidence` distinguishes exact `CopyEvidence` from `ConversionEvidence`. The latter retains the actual lowered constants, format and arithmetic revision, while the operation's output dependencies retain the companion targets. Failure records keep independently completed recipe claims without publishing an output prefix. Copy publication still checks every represented bit; numerical publication checks its sealed recipe evidence, finite centers, nonnegative bounds, complete evaluation regions and reserved target/coverage relations. Binding counts all output regions in work and scratch limits before evaluation. The existing input and output precision parameters are distinct internally; this affine recipe requires them to agree.

/// A conversion refusal, retaining the numerical or exact descriptor cause.
#[derive(Clone, Debug, thiserror::Error)]
pub enum ConversionError {
    /// Exact descriptor map checking or bounded export failed.
    #[error("conversion map: {0}")]
    Map(#[from] nrz_bindings::MapError),
    /// An outward finite arithmetic result was unavailable.
    #[error("conversion arithmetic: {0}")]
    Arithmetic(#[from] nrz_scalar::ArithmeticFailure),
    /// The supplied or computed deterministic bound was invalid.
    #[error("conversion bound: {0}")]
    Accuracy(#[from] nrz_accuracy::AccuracyFailure),
    /// The operation's concrete input does not match the exact map source.
    #[error("conversion input or output contract does not match the checked map")]
    Contract,
    /// Coefficient lowering, region work or scratch exceeds explicit capacity.
    #[error("conversion resource limit exhausted")]
    ResourceLimit,
}

mod contract;
pub use contract::ConversionContract;
pub(crate) use contract::check_operations;

mod evidence;
pub use evidence::{ConstantEvidence, ConversionEvidence, PrecisionEvidence, RecipeEvidence};

mod evaluate;
pub(crate) use evaluate::evaluate;

/// Requested companion law, checked before any numerical operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversionMode {
    /// Preserve represented or declared deterministic coverage and numerical error.
    Deterministic,
    /// Require a complete carried joint source model and propagate every statistical parameter.
    Statistical,
}

/// Transfer an entry only when both sealed format types are identical.
pub(crate) fn same_format<P: nrz_scalar::Scalar + 'static, Q: nrz_scalar::Scalar + 'static>(
    value: P,
) -> Result<Q, crate::OperationError> {
    (&value as &dyn std::any::Any)
        .downcast_ref::<Q>()
        .copied()
        .ok_or_else(|| crate::HeapError::PrecisionMismatch.into())
}

#[cfg(test)]
mod tests {
    use crate::conversion_oracle::covers;
    use crate::conversion_testing::{TestResult, contract, execute, owner, shape};
    use crate::{Extent, Heap, HeapError, HeapLimits};
    use nrz_bindings::Descriptor;

    /// An evaluation companion cannot be substituted for another value's reverse target edge.
    /// ´test:unit:conversion-evaluation-target-corruption-refuses´
    #[test]
    fn conversion_evaluation_target_corruption_refuses() -> TestResult {
        let declarations = owner()?;
        let quantity = Descriptor::ratio();
        let input_shape = shape(&declarations, &quantity, 1)?;
        let map = declarations.conversion_map(&quantity, &quantity)?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let source = heap.import_represented(Extent::new(1, 1)?, &quantity, &[2.0_f64])?;
        let output = execute(
            &mut heap,
            contract(input_shape.clone(), input_shape, map)?,
            source.value(),
        )?;
        let error = output.evaluation().ok_or("evaluation")?;
        assert!(covers(
            heap.value(output.value())?.region().get(0, 0)?,
            heap.companion(error)?.region().get(0, 0)?,
            2,
            1
        )?);
        let record = heap.descriptors.get_mut(error.key)?;
        let crate::heap::records::Meaning::Bound { target, .. } = &mut record.meaning else {
            return Err("companion".into());
        };
        *target = source.value().key;
        assert!(matches!(
            heap.companion(error),
            Err(HeapError::StaleEvidence)
        ));
        assert!(matches!(
            heap.value(output.value()),
            Err(HeapError::StaleEvidence)
        ));
        Ok(())
    }
}

mod cache;

mod definitions;
mod schema;
pub use schema::{NormalizationMaps, NormalizedEntryMaps};
pub(crate) mod precision;
pub use precision::PrecisionContract;
