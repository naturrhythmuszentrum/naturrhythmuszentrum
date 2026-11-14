// SPDX-License-Identifier: AGPL-3.0-only
//! Original coordinates are inspected before the normalized homogeneous descriptors are used.

use super::{ApplicationError, BinaryElementwise, BinaryRegion};
use crate::loan::{charge, registry::Bundle};
use crate::{LoanError, RegionContract};
use nrz_bindings::{BindingSymbols, Budget};

/// Three original witnesses share one frozen snapshot and literal rectangle.
pub(super) fn check(
    inputs: &[RegionContract; 2],
    output: &RegionContract,
    sources: [&Bundle; 2],
    destination: &BindingSymbols,
    geometry: super::Geometry,
    function: BinaryElementwise,
    budget: &mut Budget,
) -> Result<(), ApplicationError> {
    let extent = output.shape().extent();
    let axis = geometry.axis();
    if extent.rows() != axis.capacity() || extent.columns() == 0 {
        return Err(LoanError::LayoutMismatch.into());
    }
    if sources[0].identity == sources[1].identity
        || sources[0].declaration.symbols.value() == sources[1].declaration.symbols.value()
    {
        return Err(LoanError::TargetMismatch.into());
    }
    let extents = inputs.each_ref().map(|region| region.shape().extent());
    if sources
        .iter()
        .zip(extents)
        .any(|(source, extent)| source.declaration.extent != extent)
    {
        return Err(LoanError::LayoutMismatch.into());
    }
    geometry.extents(extents, extent)?;
    let projected = matches!(geometry, super::Geometry::Cartesian(..));
    let incomplete = sources
        .iter()
        .any(|source| source.channels.iter().any(Option::is_none));
    if projected && incomplete {
        return Err(LoanError::UnsupportedContract.into());
    }
    regions(inputs, output, budget)?;
    for (index, (source, region)) in sources.into_iter().zip(inputs).enumerate() {
        charge(budget, 4)?;
        let source_extent = region.shape().extent();
        let source_axis =
            geometry.source_axis(index, source.declaration.axis, source_extent, extent);
        super::validation::source(source, source_axis, source_extent)?;
        if source.declaration.symbols.value().descriptor() != region.descriptor() {
            return Err(LoanError::UnsupportedContract.into());
        }
        super::super::contract::declarations_with_shape(
            &source.declaration,
            destination,
            region.descriptor(),
            output.descriptor(),
            source_extent == extent,
        )?;
    }
    charge(budget, 1)?;
    let operation = match function {
        BinaryElementwise::Product => nrz_bindings::BinaryOp::Multiply,
        BinaryElementwise::Quotient => nrz_bindings::BinaryOp::Divide,
    };
    let derived = inputs[0]
        .shape()
        .declarations()
        .registry()
        .binary(operation, inputs[0].descriptor(), inputs[1].descriptor())
        .map_err(crate::ShapeError::from)
        .map_err(crate::OperationError::from)?;
    if output.descriptor() != &derived || destination.value().descriptor() != &derived {
        return Err(LoanError::UnsupportedContract.into());
    }
    Ok(())
}

/// Existing region/descriptor owners transfer unchanged; four new shared backings are admitted once.
pub(super) fn preflight(budget: &Budget) -> Result<(), LoanError> {
    use nrz_bindings::readings::algebra::limits::BudgetKind;
    let bytes = crate::loan::rc_bytes::<[RegionContract; 2]>()
        + crate::loan::rc_bytes::<[RegionContract; 1]>()
        + crate::loan::rc_bytes::<[crate::Extent; 2]>()
        + crate::loan::rc_bytes::<crate::kernels::KernelContract>();
    let bytes = u64::try_from(bytes).map_err(|_| LoanError::ByteOverflow)?;
    budget.check(BudgetKind::OutputBytes, bytes, crate::loan::AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, crate::loan::AT)?;
    Ok(())
}

/// Frozen original entries establish exclusion and coherent meanings for each distinct rectangle.
fn regions(
    inputs: &[RegionContract; 2],
    output: &RegionContract,
    budget: &mut Budget,
) -> Result<(), ApplicationError> {
    for (position, region) in [
        BinaryRegion::Left,
        BinaryRegion::Right,
        BinaryRegion::Output,
    ]
    .into_iter()
    .zip([&inputs[0], &inputs[1], output])
    {
        charge(budget, 1)?;
        if region.shape().declarations().id() != output.shape().declarations().id() {
            return Err(LoanError::LayoutMismatch.into());
        }
        for (entry, original) in region.shape().entries().iter().enumerate() {
            charge(budget, 1)?;
            if original.is_zero() {
                return Err(ApplicationError::UnsupportedBinaryStructure {
                    position,
                    entry,
                    scope: region.shape().structure_scope(),
                });
            }
            let descriptor = original
                .scalar_descriptor()
                .ok_or(LoanError::UnsupportedContract)?;
            // HomogeneousRecipe normalizes its result. Only the retained original proves coherence.
            crate::binding::schema::check_quantity(descriptor)
                .map_err(|_| LoanError::UnsupportedContract)?;
            if descriptor != region.descriptor() {
                return Err(LoanError::UnsupportedContract.into());
            }
        }
    }
    Ok(())
}
