// SPDX-License-Identifier: AGPL-3.0-only
//! Descriptor admission prepares a numerical schedule after checking original meanings.

use super::{KernelDraft, regions};
use crate::kernels::contract::{Recipe, work};
use crate::{
    CheckedShape, Elementwise, KernelError, OperationDraft, OperationError, RegionContract,
};
use nrz_bindings::{Conversion, Descriptor, Measure, Role, Scale};

impl OperationDraft {
    /// Propose a native elementwise operation with a separately checked output shape.
    ///
    /// Bindings checks the corresponding scalar power. Numerical domain checks are deferred
    /// until the entire actual deterministic input box is available under the binding.
    /// # Errors
    /// Refuses unlike snapshots/extents, wrong output kinds, points or unsupported powers.
    pub fn map_elementwise(
        input: CheckedShape,
        output: CheckedShape,
        function: Elementwise,
    ) -> Result<Self, OperationError> {
        input.same_snapshot(&output)?;
        if input.extent() != output.extent() {
            return Err(KernelError::Contract.into());
        }
        let expected = descriptor(&input, function)?;
        let output = RegionContract::check(output)?;
        if output.descriptor() != &expected {
            return Err(KernelError::Contract.into());
        }
        Ok(super::draft::kernel(KernelDraft {
            work: work(output.shape().extent().entries(), 12)?,
            inputs: regions(vec![input])?,
            output,
            recipe: Recipe::Elementwise(function),
        }))
    }
}

/// Derive only descriptor constants through bindings; no stored coordinate enters exact algebra.
fn descriptor(input: &CheckedShape, function: Elementwise) -> Result<Descriptor, OperationError> {
    let quantity = RegionContract::check(input.clone())?;
    let unit = Measure::coherent();
    let twice = Conversion {
        scale: unit.scale().clone(),
        shift: unit.scale().clone(),
    }
    .apply(unit.rational_scale().map_err(crate::ShapeError::from)?)
    .map_err(crate::ShapeError::from)?;
    let double = Measure::new(
        "native-exponent",
        Scale::Exact(twice),
        unit.offset().clone(),
    )
    .map_err(crate::ShapeError::from)?;
    let exponent = match function {
        Elementwise::Square => {
            double
                .conversion_to(&unit, Role::Difference)
                .map_err(crate::ShapeError::from)?
                .scale
        }
        Elementwise::SquareRoot => {
            unit.conversion_to(&double, Role::Difference)
                .map_err(crate::ShapeError::from)?
                .scale
        }
        Elementwise::Reciprocal => {
            let shifted = Measure::new(
                "native-negative-exponent",
                Scale::Exact(
                    unit.rational_scale()
                        .map_err(crate::ShapeError::from)?
                        .clone(),
                ),
                unit.rational_scale()
                    .map_err(crate::ShapeError::from)?
                    .clone(),
            )
            .map_err(crate::ShapeError::from)?;
            unit.conversion_to(&shifted, Role::Point)
                .map_err(crate::ShapeError::from)?
                .shift
        }
    };
    Ok(input
        .declarations()
        .registry()
        .power(
            quantity.descriptor(),
            exponent
                .as_rational()
                .ok_or(nrz_bindings::UnitError::UnsupportedExactConstantExport)
                .map_err(crate::ShapeError::from)?,
        )
        .map_err(crate::ShapeError::from)?)
}
