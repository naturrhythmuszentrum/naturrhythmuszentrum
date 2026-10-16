// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed source-free matrix definitions and independently typed affine offsets.

use super::KernelError;
use super::contract::work;
use crate::{CheckedShape, OperationError, RegionContract, ShapeLimits};
use nrz_scalar::{Finite, LiteralLimits};
use nrz_units::{Descriptor, DescriptorConstant};

/// An exact fixed coefficient definition, never a declaration of uncertain heap data.
#[derive(Clone, Debug)]
pub struct FixedCoefficient {
    /// Coherent scalar meaning of the coefficient or offset.
    pub(crate) quantity: Descriptor,
    /// Exact represented constant or units-owned descriptor definition.
    pub(crate) value: CoefficientValue,
}

/// Definitions cross the descriptor boundary before any numerical operand is read.
#[derive(Clone, Debug)]
pub(crate) enum CoefficientValue {
    /// This finite binary64 value defines the exact mathematical coefficient.
    Represented(f64),
    /// Units-owned exact descriptor metadata, lowered under explicit limits.
    Descriptor(Box<DescriptorConstant>),
}

impl FixedCoefficient {
    /// Define a fixed coefficient by an exact finite binary64 value in coherent coordinates.
    ///
    /// An uncertain coefficient must be a value operand of a product instead.
    /// # Errors
    /// Refuses nonfinite values, points, counts and noncoherent descriptors.
    pub fn represented(quantity: Descriptor, value: f64) -> Result<Self, OperationError> {
        check_quantity(&quantity)?;
        Ok(Self {
            quantity,
            value: CoefficientValue::Represented(
                Finite::new(value).map_err(KernelError::from)?.get(),
            ),
        })
    }

    /// Define a coefficient from an exact units-owned constant and its physical meaning.
    /// # Errors
    /// Refuses points, counts and noncoherent descriptors; map construction checks ownership.
    pub fn descriptor(
        quantity: Descriptor,
        value: DescriptorConstant,
    ) -> Result<Self, OperationError> {
        check_quantity(&quantity)?;
        Ok(Self {
            quantity,
            value: CoefficientValue::Descriptor(Box::new(value)),
        })
    }
}

/// A fixed `L x + d` contract with checked matrix composition and ordered output offsets.
#[derive(Clone, Debug)]
pub struct LinearMap {
    /// Operand region, independent of the fixed operator definition.
    pub(crate) input: RegionContract,
    /// Derived output with coherent unit references.
    pub(crate) output: RegionContract,
    /// Exact original operator shape and its structure obligations.
    pub(crate) operator: CheckedShape,
    /// Fixed coefficients in logical column order.
    pub(crate) coefficients: Vec<FixedCoefficient>,
    /// One fixed offset for every output coordinate.
    pub(crate) offsets: Vec<FixedCoefficient>,
    /// Explicit per-constant decoding limits.
    pub(crate) literals: LiteralLimits,
    /// Complete numerical phase admission count.
    pub(crate) work: usize,
}

impl LinearMap {
    /// Check matrix composition, every coefficient, and every affine offset.
    ///
    /// Omit offsets with an empty vector to define a purely linear map. Coefficients
    /// are fixed mathematical definitions; this constructor accepts no uncertain handle.
    /// # Errors
    /// Refuses incompatible axes, coefficient kinds, snapshots, lengths and limits.
    pub fn new(
        operator: CheckedShape,
        input: CheckedShape,
        coefficients: Vec<FixedCoefficient>,
        mut offsets: Vec<FixedCoefficient>,
        literals: LiteralLimits,
        limits: ShapeLimits,
    ) -> Result<Self, OperationError> {
        let operator_region = RegionContract::check(operator.clone())?;
        let input = RegionContract::check(input)?;
        let mut shape = product_shape(&operator, input.shape(), limits)?;
        let output = RegionContract::check(shape.clone())?;
        check_coefficients(&coefficients, operator_region.descriptor(), &operator)?;
        if offsets.is_empty() {
            offsets = vec![
                FixedCoefficient::represented(output.descriptor().clone(), 0.0)?;
                shape.extent().entries()
            ];
        }
        check_coefficients(&offsets, output.descriptor(), &shape)?;
        for entry in &mut shape.entries {
            entry.zero = false;
        }
        let count = work(
            shape.extent().entries(),
            operator
                .extent()
                .columns()
                .checked_mul(8)
                .and_then(|n| n.checked_add(12))
                .ok_or(OperationError::ResourceLimit)?,
        )?;
        let count = count
            .checked_add(work(coefficients.len(), 4)?)
            .and_then(|n| n.checked_add(input.shape().extent().entries()))
            .ok_or(OperationError::ResourceLimit)?;
        Ok(Self {
            input,
            output: RegionContract::check(shape)?,
            operator,
            coefficients,
            offsets,
            literals,
            work: count,
        })
    }

    /// Derived result shape for later checked operations.
    #[must_use]
    pub const fn output(&self) -> &CheckedShape {
        self.output.shape()
    }
}

/// Finish empty ratio contractions with a checked typed zero, never an untyped shortcut.
pub(crate) fn product_shape(
    left: &CheckedShape,
    right: &CheckedShape,
    limits: ShapeLimits,
) -> Result<CheckedShape, OperationError> {
    let mut shape = left.product(right, limits)?;
    if left.extent().columns() == 0 {
        for entry in &mut shape.entries {
            let quantity = Descriptor::ratio();
            entry.conversion = Some(
                entry
                    .pairing
                    .check_entry(shape.declarations.registry(), &quantity)
                    .map_err(crate::ShapeError::from)?,
            );
            entry.scalar = Some(quantity);
        }
    }
    Ok(shape)
}

/// A fixed definition must match each original coefficient schema before zero elimination.
fn check_coefficients(
    values: &[FixedCoefficient],
    quantity: &Descriptor,
    shape: &CheckedShape,
) -> Result<(), OperationError> {
    if values.len() != shape.extent().entries() {
        return Err(KernelError::Contract.into());
    }
    for coefficient in values {
        if &coefficient.quantity != quantity {
            return Err(KernelError::Contract.into());
        }
        if let CoefficientValue::Descriptor(value) = &coefficient.value
            && value.map_id().snapshot_id() != shape.declarations().id()
        {
            return Err(KernelError::Contract.into());
        }
    }
    Ok(())
}

/// Fixed numerical definitions have the same continuous coherent difference restriction as operands.
fn check_quantity(quantity: &Descriptor) -> Result<(), OperationError> {
    if quantity.role() != nrz_units::Role::Difference
        || quantity.kind().count_domain().is_some()
        || quantity != &quantity.coherent()
    {
        return Err(KernelError::Contract.into());
    }
    Ok(())
}
