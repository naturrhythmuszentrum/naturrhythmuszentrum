// SPDX-License-Identifier: AGPL-3.0-only
//! Physical and normalized borrows are distinct and stamped by one execution identity.

use nrz_scalar::Scalar;

use super::{Bound, ExecutionId, OperationError, RegionContract, buffer};
use crate::{CompanionView, Extent, HeapError, ValueStamp, ValueView};

/// Immutable physical coordinates, with their checked original shape.
#[derive(Debug)]
pub struct PhysicalView<'b, P: Scalar> {
    /// Current region and complete supported companion relation.
    value: ValueView<'b, P>,
    /// Operation-owned physical shape and exact unit-reference definitions.
    contract: &'b RegionContract,
}

impl<P: Scalar + 'static> PhysicalView<'_, P> {
    /// Read a physical entry without granting mutation or normalized kernel authority.
    /// # Errors
    /// Refuses an out-of-range entry or invalid storage.
    pub fn get(&self, row: usize, column: usize) -> Result<P, HeapError> {
        self.value.region().get(row, column)
    }
    /// Exact physical interpretation and ordered shape.
    #[must_use]
    pub const fn contract(&self) -> &RegionContract {
        self.contract
    }
    /// Captured current value identity and version.
    #[must_use]
    pub const fn stamp(&self) -> ValueStamp {
        self.value.stamp()
    }
}

/// Read-only dimensionless coordinates under explicit coherent unit references.
#[derive(Debug)]
pub struct NormalizedView<'b, P: Scalar> {
    /// Identity normalization borrows the immutable physical payload without arithmetic.
    value: ValueView<'b, P>,
    /// Selected companion, using exactly the same logical map as the value.
    bound: Option<CompanionView<'b, P>>,
    /// Separately selected numerical error; it overlaps the total bound.
    evaluation: Option<CompanionView<'b, P>>,
    /// Selected overlapping product derivation regions.
    pub(crate) contributions: Option<[CompanionView<'b, P>; 3]>,
    /// Original shape and retained exact reference identities.
    contract: &'b RegionContract,
}

impl<P: Scalar + 'static> NormalizedView<'_, P> {
    /// Read the separately retained numerical evaluation radius.
    /// # Errors
    /// Refuses an invalid selected coordinate or stale companion.
    pub fn evaluation(&self, row: usize, column: usize) -> Result<Option<P>, HeapError> {
        self.evaluation
            .as_ref()
            .map(|view| view.region().get(row, column))
            .transpose()
    }

    /// Read one normalized coordinate; coherent unit references make this bit-exact.
    /// # Errors
    /// Refuses an out-of-range entry or invalid storage.
    pub fn get(&self, row: usize, column: usize) -> Result<P, HeapError> {
        self.value.region().get(row, column)
    }
    /// Read the complete selected declared bound, when present, in the same references.
    /// # Errors
    /// Refuses out-of-range entries or invalid companion storage.
    pub fn bound(&self, row: usize, column: usize) -> Result<Option<P>, HeapError> {
        self.bound
            .as_ref()
            .map(|bound| bound.region().get(row, column))
            .transpose()
    }
    /// Logical input rectangle.
    #[must_use]
    pub fn extent(&self) -> Extent {
        self.value.region().extent()
    }
    /// Retained physical contract and exact unit-reference normalization.
    #[must_use]
    pub const fn contract(&self) -> &RegionContract {
        self.contract
    }
}

/// Operation-ordered normalized inputs, privately stamped by their owning binding.
#[derive(Debug)]
pub struct NormalizedInputs<'b, P: Scalar> {
    /// Binding identity, independent of recipe or schema equality.
    pub(crate) execution: ExecutionId,
    /// Immutable normalized operands in recipe order.
    pub(crate) views: Vec<NormalizedView<'b, P>>,
}

impl<P: Scalar> NormalizedInputs<'_, P> {
    /// The execution that granted this bundle's normalized authority.
    #[must_use]
    pub const fn execution(&self) -> ExecutionId {
        self.execution
    }
    /// Complete immutable operation-ordered numerical views.
    #[must_use]
    pub fn views(&self) -> &[NormalizedView<'_, P>] {
        &self.views
    }
}

impl<P: Scalar + 'static, Q: Scalar + 'static> Bound<'_, P, Q> {
    /// Borrow one physical operand for inspection, without normalized kernel authority.
    /// # Errors
    /// Refuses an invalid slot or any no-longer-resolvable internal dependency.
    pub fn physical_input(&self, slot: usize) -> Result<PhysicalView<'_, P>, OperationError> {
        let input = self
            .dependencies
            .get(slot)
            .ok_or(OperationError::InputCount)?;
        let contract = self
            .operation
            .inputs
            .get(slot)
            .ok_or(OperationError::InputCount)?;
        Ok(PhysicalView {
            value: self.heap.value(input.handle())?,
            contract,
        })
    }

    /// Lend the entire normalized operand bundle for this execution only.
    ///
    /// The native copy recipe's explicit coherent unit references require no arithmetic.
    /// General normalization and affine origins require later numerical conversion kernels.
    /// # Errors
    /// Refuses allocation failure or any invalid concrete value/companion dependency.
    pub fn normalized_inputs(&self) -> Result<NormalizedInputs<'_, P>, OperationError> {
        let mut views = buffer(self.dependencies.len())?;
        for (input, contract) in self.dependencies.iter().zip(self.operation.inputs.iter()) {
            views.push(NormalizedView {
                value: self.heap.value(input.handle())?,
                bound: self.heap.selected_bound(input.handle())?,
                evaluation: self.heap.selected_evaluation(input.handle())?,
                contributions: crate::kernels::contribution_views(self.heap, input.handle())?,
                contract,
            });
        }
        Ok(NormalizedInputs {
            execution: self.execution,
            views,
        })
    }
}
