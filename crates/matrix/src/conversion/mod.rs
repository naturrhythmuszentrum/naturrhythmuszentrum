// SPDX-License-Identifier: AGPL-3.0-only
//! Checked stored-coordinate conversions and their transaction contracts.

/// A conversion refusal, retaining the numerical or exact descriptor cause.
#[derive(Clone, Debug, thiserror::Error)]
pub enum ConversionError {
    /// Exact descriptor map checking or bounded export failed.
    #[error("conversion map: {0}")]
    Map(#[from] nrz_units::MapError),
    /// An outward finite arithmetic result was unavailable.
    #[error("conversion arithmetic: {0}")]
    Arithmetic(#[from] nrz_scalar::ArithmeticFailure),
    /// The supplied or computed deterministic bound was invalid.
    #[error("conversion bound: {0}")]
    Accuracy(#[from] nrz_accuracy::AccuracyFailure),
    /// Statistical conversion requires the joint model propagation recipe.
    #[error("statistical conversion is unavailable until joint model propagation is connected")]
    StatisticalUnavailable,
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
    /// Joint statistical conversion, explicitly unavailable until its propagation recipe lands.
    Statistical,
}

impl ConversionMode {
    /// Refuse an unsupported companion law without approximating or discarding it.
    /// # Errors
    /// Statistical requests return `StatisticalUnavailable`.
    pub const fn validate(self) -> Result<(), ConversionError> {
        match self {
            Self::Deterministic => Ok(()),
            Self::Statistical => Err(ConversionError::StatisticalUnavailable),
        }
    }
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
    use nrz_units::Descriptor;

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
