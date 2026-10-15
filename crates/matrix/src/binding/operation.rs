// SPDX-License-Identifier: AGPL-3.0-only
//! Closed copy recipes retain ordered physical contracts and exact unit references.

use nrz_units::{Descriptor, Role};

use super::{ExecutionLimits, OperationError};
use crate::{Axis, CheckedShape, CoordinateMap, ReferenceScale, ShapeLimits};

/// Proposed native copy operation, with one fresh output per ordered input.
#[derive(Clone, Debug)]
pub struct OperationDraft {
    /// Original checked shapes; numerical structural obligations remain pending.
    inputs: Vec<CheckedShape>,
}

impl OperationDraft {
    /// Propose bit-preserving copies with complete supported companion propagation.
    #[must_use]
    pub const fn copy(inputs: Vec<CheckedShape>) -> Self {
        Self { inputs }
    }

    /// Check the closed copy recipe and prepare exact coherent unit-reference maps.
    ///
    /// No floating coefficients are evaluated. Each reference is exactly one coherent
    /// unit, making normalization and restoration exact copies of represented entries.
    /// # Errors
    /// Refuses empty recipes, nonhomogeneous or formal entry schemas, points and limits.
    pub fn check(self, limits: ExecutionLimits) -> Result<CheckedOperation, OperationError> {
        if self.inputs.is_empty() {
            return Err(OperationError::InputCount);
        }
        let work = self.inputs.iter().try_fold(0_usize, |total, shape| {
            total
                .checked_add(shape.extent().entries())
                .ok_or(OperationError::ResourceLimit)
        })?;
        limits.check(self.inputs.len(), work)?;
        let mut inputs = super::buffer(self.inputs.len())?;
        for shape in self.inputs {
            inputs.push(CopyContract::check(shape)?);
        }
        Ok(CheckedOperation {
            inputs: inputs.into(),
            revision: 1,
        })
    }
}

/// A checked native recipe; it grants neither a heap borrow nor numerical evidence.
#[derive(Clone, Debug)]
pub struct CheckedOperation {
    /// Complete operand contracts, retained in execution order.
    pub(crate) inputs: std::rc::Rc<[CopyContract]>,
    /// Revision of the checked closed recipe.
    revision: u32,
}

impl CheckedOperation {
    /// Ordered copy contracts; the corresponding output has exactly the same shape.
    #[must_use]
    pub fn inputs(&self) -> &[CopyContract] {
        self.inputs.as_ref()
    }

    /// Revision of the bit-preserving copy recipe and its identity normalization.
    #[must_use]
    pub const fn revision(&self) -> u32 {
        self.revision
    }
}

/// One homogeneous copy contract with retained physical axes and exact references.
#[derive(Clone, Debug)]
pub struct CopyContract {
    /// Original ordered shape and all frozen declaration evidence.
    shape: CheckedShape,
    /// Coherent physical interpretation required of the heap region.
    quantity: Descriptor,
    /// Exact coherent unit references for the input axis.
    domain: CoordinateMap,
    /// Exact coherent unit references for the output axis.
    codomain: CoordinateMap,
}

impl CopyContract {
    /// Admit only the concrete entry schemas implemented by homogeneous heap storage.
    fn check(shape: CheckedShape) -> Result<Self, OperationError> {
        let quantity = shape.entries().first().map_or_else(
            || Ok(Descriptor::ratio()),
            |entry| {
                entry
                    .scalar_descriptor()
                    .map(Descriptor::coherent)
                    .ok_or(OperationError::UnsupportedShape)
            },
        )?;
        if quantity.role() != Role::Difference
            || shape.entries().iter().any(|entry| {
                entry
                    .scalar_descriptor()
                    .is_none_or(|value| value.coherent() != quantity)
            })
        {
            return Err(OperationError::UnsupportedShape);
        }
        let domain = unit_coordinates(shape.domain())?;
        let codomain = unit_coordinates(shape.codomain())?;
        Ok(Self {
            shape,
            quantity,
            domain,
            codomain,
        })
    }

    /// The physical shape, including structure scope and original declarations.
    #[must_use]
    pub const fn shape(&self) -> &CheckedShape {
        &self.shape
    }

    /// Required coherent homogeneous entry descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        &self.quantity
    }

    /// Source-free coherent unit references on the domain axis.
    #[must_use]
    pub const fn domain_coordinates(&self) -> &CoordinateMap {
        &self.domain
    }

    /// Source-free coherent unit references on the codomain axis.
    #[must_use]
    pub const fn codomain_coordinates(&self) -> &CoordinateMap {
        &self.codomain
    }
}

/// Explicitly define positive coherent references; no scale is read from heap data.
fn unit_coordinates(axis: &Axis) -> Result<CoordinateMap, OperationError> {
    let mut references = super::buffer(axis.entries().len())?;
    let mut origins = super::buffer(axis.entries().len())?;
    for entry in axis.entries() {
        let quantity = entry.quantity().ok_or(OperationError::UnsupportedShape)?;
        references.push(ReferenceScale::new(entry.name(), &quantity.coherent())?);
        origins.push(None);
    }
    Ok(CoordinateMap::check(
        axis,
        references,
        origins,
        ShapeLimits::default(),
    )?)
}
