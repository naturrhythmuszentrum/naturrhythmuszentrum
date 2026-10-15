// SPDX-License-Identifier: AGPL-3.0-only
//! Exact affine region contracts, checked before any numerical value is borrowed.

use super::ConversionError;
use crate::binding::{NativeRecipe, buffer};
use crate::{
    CheckedOperation, CheckedShape, ExecutionLimits, OperationDraft, OperationError, RegionContract,
};
use nrz_scalar::LiteralLimits;
use nrz_units::{CheckedMap, Descriptor};
use std::rc::Rc;

/// A source and destination rectangle with an exact units-owned affine map.
///
/// Shapes describe the ordered difference coordinates. For point regions the
/// map additionally retains the affine role and its declared origin.
#[derive(Clone, Debug)]
pub struct ConversionContract {
    /// Actual source interpretation and ordered tangent-coordinate shape.
    pub(crate) input: RegionContract,
    /// Fresh destination interpretation and ordered tangent-coordinate shape.
    pub(crate) output: RegionContract,
    /// Exact map identity retained through lowering and publication.
    pub(crate) map: CheckedMap,
    /// Explicit bound on each coefficient's numerical decoding.
    pub(crate) literals: LiteralLimits,
    /// Shared immutable-definition lowering, never containing a stored coordinate.
    pub(crate) cache: Rc<std::cell::RefCell<super::cache::ConstantCache>>,
}

impl ConversionContract {
    /// Check source and output shapes against the exact map's descriptors.
    /// # Errors
    /// Refuses different extents, incompatible original schemas, unsupported
    /// homogeneous storage and maps belonging to another declaration snapshot.
    pub fn new(
        input: CheckedShape,
        output: CheckedShape,
        map: CheckedMap,
        literals: LiteralLimits,
        mode: super::ConversionMode,
    ) -> Result<Self, OperationError> {
        mode.validate()?;
        if input.extent() != output.extent() {
            return Err(ConversionError::Contract.into());
        }
        map.validate_snapshot(&input.declarations)?;
        map.validate_snapshot(&output.declarations)?;
        let input = region(
            input,
            map.id()
                .source()
                .descriptor()
                .ok_or(OperationError::UnsupportedShape)?,
        )?;
        let output = region(
            output,
            map.id()
                .target()
                .descriptor()
                .ok_or(OperationError::UnsupportedShape)?,
        )?;
        Ok(Self {
            input,
            output,
            map,
            literals,
            cache: Rc::default(),
        })
    }

    /// Exact checked map, with source, destination, reference and origin identity.
    #[must_use]
    pub const fn map(&self) -> &CheckedMap {
        &self.map
    }
}

impl OperationDraft {
    /// Propose an atomic bundle of checked affine conversions.
    #[must_use]
    pub const fn convert(conversions: Vec<ConversionContract>) -> Self {
        Self {
            inputs: Vec::new(),
            conversions: Some(conversions),
        }
    }
}

/// Validate the homogeneous tangent shape while preserving the actual point or measure schema.
fn region(shape: CheckedShape, descriptor: &Descriptor) -> Result<RegionContract, OperationError> {
    if descriptor.role() == nrz_units::Role::Point
        && shape.entries().iter().any(crate::EntryContract::is_zero)
    {
        return Err(ConversionError::Contract.into());
    }
    let mut contract = RegionContract::check(shape)?;
    if contract.descriptor() != &descriptor.difference().coherent() {
        return Err(ConversionError::Contract.into());
    }
    contract.quantity = descriptor.clone();
    Ok(contract)
}

/// Assemble the closed schedule and independent output contracts under explicit limits.
pub(crate) fn check_operations(
    conversions: Vec<ConversionContract>,
    limits: ExecutionLimits,
) -> Result<CheckedOperation, OperationError> {
    if conversions.is_empty() {
        return Err(OperationError::InputCount);
    }
    let entries = conversions.iter().try_fold(0_usize, |total, conversion| {
        total
            .checked_add(conversion.input.shape().extent().entries())
            .ok_or(OperationError::ResourceLimit)
    })?;
    limits.check(conversions.len(), entries)?;
    let mut inputs = buffer(conversions.len())?;
    let mut outputs = buffer(conversions.len())?;
    for conversion in &conversions {
        inputs.push(conversion.input.clone());
        outputs.push(conversion.output.clone());
    }
    Ok(CheckedOperation {
        inputs: inputs.into(),
        outputs: outputs.into(),
        recipe: NativeRecipe::Affine(Rc::from(conversions)),
        revision: 1,
    })
}
