// SPDX-License-Identifier: AGPL-3.0-only
//! Closed native recipes retain ordered input/output contracts and exact coordinate definitions.

use nrz_units::{Descriptor, Role};

use super::{ExecutionLimits, OperationError};
use crate::{Axis, CheckedShape, CoordinateMap, ReferenceScale, ShapeLimits};

/// Proposed closed native operation, with one fresh output per ordered input.
#[derive(Clone, Debug)]
pub struct OperationDraft {
    /// Original checked shapes; numerical structural obligations remain pending.
    pub(crate) inputs: Vec<CheckedShape>,
    /// Optional checked numerical maps; absence selects exact copy.
    pub(crate) conversions: Option<Vec<crate::conversion::ConversionContract>>,
}

impl OperationDraft {
    /// Propose bit-preserving copies with complete supported companion propagation.
    #[must_use]
    pub const fn copy(inputs: Vec<CheckedShape>) -> Self {
        Self {
            inputs,
            conversions: None,
        }
    }

    /// Check the closed copy recipe and prepare exact coherent unit-reference maps.
    ///
    /// No floating coefficients are evaluated. Each reference is exactly one coherent
    /// unit, making normalization and restoration exact copies of represented entries.
    /// # Errors
    /// Refuses empty recipes, nonhomogeneous or formal entry schemas, points and limits.
    pub fn check(self, limits: ExecutionLimits) -> Result<CheckedOperation, OperationError> {
        if let Some(conversions) = self.conversions {
            return crate::conversion::check_operations(conversions, limits);
        }
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
            inputs.push(RegionContract::check(shape)?);
        }
        Ok(CheckedOperation {
            outputs: inputs.clone().into(),
            inputs: inputs.into(),
            recipe: NativeRecipe::Copy,
            revision: 1,
        })
    }
}

/// A checked native recipe; it grants neither a heap borrow nor numerical evidence.
#[derive(Clone, Debug)]
pub struct CheckedOperation {
    /// Complete operand contracts, retained in execution order.
    pub(crate) inputs: std::rc::Rc<[RegionContract]>,
    /// Revision of the checked closed recipe.
    pub(crate) revision: u32,
    /// Output contracts checked independently of the source interpretation.
    pub(crate) outputs: std::rc::Rc<[RegionContract]>,
    /// Closed crate-defined numerical schedule.
    pub(crate) recipe: NativeRecipe,
}

impl CheckedOperation {
    /// Whether this sealed recipe propagates statistical coverage.
    ///
    /// Copy and deterministic conversions refuse statistical companions until a
    /// joint-model propagation recipe explicitly declares that capability.
    #[must_use]
    pub const fn carries_statistics(&self) -> bool {
        false
    }

    /// Ordered input contracts, independently checked against each bound value.
    #[must_use]
    pub fn inputs(&self) -> &[RegionContract] {
        self.inputs.as_ref()
    }

    /// Ordered fresh output contracts, including their destination interpretations.
    #[must_use]
    pub fn outputs(&self) -> &[RegionContract] {
        self.outputs.as_ref()
    }

    /// Revision of the sealed recipe and its verification rule.
    #[must_use]
    pub const fn revision(&self) -> u32 {
        self.revision
    }
}

/// One homogeneous region contract with retained tangent axes and actual coordinate descriptor.
#[derive(Clone, Debug)]
pub struct RegionContract {
    /// Original ordered shape and all frozen declaration evidence.
    pub(crate) shape: CheckedShape,
    /// Actual coherent, normalized or presentation interpretation required of the region.
    pub(crate) quantity: Descriptor,
    /// Exact coherent unit references for the input axis.
    domain: CoordinateMap,
    /// Exact coherent unit references for the output axis.
    codomain: CoordinateMap,
}

impl RegionContract {
    /// Admit only the concrete entry schemas implemented by homogeneous heap storage.
    pub(crate) fn check(shape: CheckedShape) -> Result<Self, OperationError> {
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

    /// Required actual homogeneous entry descriptor, including affine role and measure.
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

/// Closed supported recipe schedule; external code cannot install a numerical callback.
#[derive(Clone, Debug)]
pub(crate) enum NativeRecipe {
    /// Bit-preserving copy with independent equality verification.
    Copy,
    /// Checked exact affine descriptor maps, in output order.
    Affine(std::rc::Rc<[crate::conversion::ConversionContract]>),
}
