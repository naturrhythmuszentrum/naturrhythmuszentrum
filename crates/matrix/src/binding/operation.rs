// SPDX-License-Identifier: AGPL-3.0-only
//! Closed native recipes retain ordered input/output contracts and exact coordinate definitions.
//!
//! # Closed copy admission
//!
//! `OperationDraft::copy` checks a native recipe with one fresh output per input. Each `RegionContract` retains its checked ordered shape, frozen declarations and concrete homogeneous difference descriptor. It explicitly defines one coherent unit reference for every domain and codomain coordinate through `CoordinateMap`; normalization and restoration therefore copy the represented entries exactly. This admits physical homogeneous differences as well as ratios. Points, heterogeneous entry schemas and formal entries without a concrete scalar descriptor refuse. The copy recipe continues to refuse general scales and affine origins; the separate `ConversionContract` recipe admits checked bindings maps. Empty rectangles use ratio schemas and have no addressable entries.
//!
//! The homogeneous copy admission comes from bindings `HomogeneousRecipe` and a retained `CopyWitness` over actual value, deterministic-bound, numerical-error and planned result symbols. Matrix resolves each numerical region's unique opaque owner and checks live extents, selected layouts, finite payloads, versions and companion targets. Missing or double owners and mismatched companion/result symbols refuse. `RegionContract::check` delegates its physical homogeneity decision to bindings; the duplicate matrix descriptor comparison is replaced by the declared symbol check.

use nrz_bindings::{Descriptor, HomogeneousRecipe};

use super::{ExecutionLimits, OperationError};
use crate::{Axis, CheckedShape, CoordinateMap};

/// Proposed closed native operation, with one fresh output per ordered input.
#[derive(Clone, Debug)]
pub struct OperationDraft {
    /// Optional square solve with an independent represented-center certificate.
    pub(crate) solve: Option<Box<crate::solve::SolveContract>>,
    /// Optional deterministic kernel, checked independently of representation conversions.
    pub(crate) kernel: Option<crate::kernels::KernelContract>,
    /// Original checked shapes; numerical structural obligations remain pending.
    pub(crate) inputs: Vec<CheckedShape>,
    /// Optional checked numerical maps; absence selects exact copy.
    pub(crate) conversions: Option<Vec<crate::conversion::ConversionContract>>,
    /// Explicit distinct source and target formats with preserved region contracts.
    pub(crate) precision: Option<(
        Vec<crate::conversion::PrecisionContract>,
        nrz_scalar::Precision,
        nrz_scalar::Precision,
    )>,
}

impl OperationDraft {
    /// Propose bit-preserving copies with complete supported companion propagation.
    #[must_use]
    pub const fn copy(inputs: Vec<CheckedShape>) -> Self {
        Self {
            inputs,
            conversions: None,
            precision: None,
            kernel: None,
            solve: None,
        }
    }

    /// Check the closed copy recipe and prepare exact coherent unit-reference maps.
    ///
    /// No floating coefficients are evaluated. Each reference is exactly one coherent
    /// unit, making normalization and restoration exact copies of represented entries.
    /// # Errors
    /// Refuses empty recipes, nonhomogeneous or formal entry schemas, points and limits.
    pub fn check(self, limits: ExecutionLimits) -> Result<CheckedOperation, OperationError> {
        if let Some(solve) = self.solve {
            return solve.check(limits);
        }
        if let Some(kernel) = self.kernel {
            return kernel.check(limits);
        }
        if let Some((contracts, source, target)) = self.precision {
            return crate::conversion::precision::check(contracts, source, target, limits);
        }
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
    /// Only recipes with complete source-preserving propagation report this capability.
    #[must_use]
    pub const fn carries_statistics(&self) -> bool {
        self.statistical_recipe()
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
    /// Bindings-owned homogeneous admission, retained beside numerical execution.
    pub(crate) admission: HomogeneousRecipe,
    /// Exact coherent unit references for the input axis.
    domain: CoordinateMap,
    /// Exact coherent unit references for the output axis.
    codomain: CoordinateMap,
}

impl RegionContract {
    /// Consume the bindings predicate for this closed homogeneous recipe.
    pub(crate) fn check(shape: CheckedShape) -> Result<Self, OperationError> {
        let admission = HomogeneousRecipe::check(
            shape
                .entries()
                .iter()
                .map(crate::EntryContract::scalar_descriptor),
            shape.extent().entries(),
        )
        .map_err(|_| OperationError::UnsupportedShape)?;
        let quantity = admission.descriptor().clone();
        let domain = unit_coordinates(shape.domain())?;
        let codomain = unit_coordinates(shape.codomain())?;
        Ok(Self {
            shape,
            quantity,
            admission,
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
    Ok(CoordinateMap::coherent_units(axis)?)
}

/// Closed supported recipe schedule; external code cannot install a numerical callback.
#[derive(Clone, Debug)]
pub(crate) enum NativeRecipe {
    /// Square solve under an independent inverse-defect and residual theorem.
    Solve(Box<crate::solve::SolveContract>),
    /// Checked deterministic multi-input region operation.
    Kernel(Box<crate::kernels::KernelContract>),
    /// Bit-preserving copy with independent equality verification.
    Copy,
    /// Checked exact affine descriptor maps, in output order.
    Affine(std::rc::Rc<[crate::conversion::ConversionContract]>),
    /// A new arena representation with a separately checked displacement region.
    Precision {
        /// Source format required by every typed input slot.
        source: nrz_scalar::Precision,
        /// Fresh output format, never a reinterpretation of an old arena.
        target: nrz_scalar::Precision,
    },
}
