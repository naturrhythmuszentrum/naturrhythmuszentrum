// SPDX-License-Identifier: AGPL-3.0-only
//! Historical receipts share original definitions without retaining covered scratch owners.

use super::{Channel, Role, plan::Plan};
use crate::{Dependency, ExecutionId, Extent, OperationError, PrivateAllocationTicket};
use nrz_scalar::Precision;
use std::rc::Rc;

/// One completed statistical request projected into its original shared finite plan.
#[derive(Clone, Debug)]
pub struct StatisticsAllocation {
    /// Original model, law, selections and observed capacities.
    pub(super) plan: Rc<Plan>,
    /// Exact request position; stage and shape are never copied into the receipt.
    pub(super) slot: usize,
}
impl StatisticsAllocation {
    /// Original authority issued before the allocating function was entered.
    #[must_use]
    pub fn ticket(&self) -> PrivateAllocationTicket {
        PrivateAllocationTicket::new(self.plan.trace.execution(), self.plan.token, self.slot)
    }
    /// Original mathematical matrix or publication role.
    #[must_use]
    pub fn role(&self) -> Role {
        self.plan.specifications[self.slot].role
    }
    /// Ordered input, output or cross-target side associated with this role.
    #[must_use]
    pub fn index(&self) -> usize {
        self.plan.specifications[self.slot].index
    }
    /// Candidate, enclosure, parameter error or opaque backend representation.
    #[must_use]
    pub fn channel(&self) -> Channel {
        self.plan.specifications[self.slot].channel
    }
    /// Complete logical rectangle fixed before allocation.
    #[must_use]
    pub fn extent(&self) -> Extent {
        self.plan.specifications[self.slot].extent
    }
    /// Actual requested scalar format; standalone operands can have different formats.
    #[must_use]
    pub fn precision(&self) -> Precision {
        self.plan.specifications[self.slot].precision
    }
    /// Logical primitive floating coordinates; this is not a backend capacity measurement.
    /// # Errors
    /// Refuses integer overflow instead of reporting a wrapped storage count.
    pub fn float_entries(&self) -> Result<usize, OperationError> {
        self.extent()
            .entries()
            .checked_mul(self.channel().components())
            .ok_or(OperationError::ResourceLimit)
    }
    /// Actual vector capacity in bytes; backend storage remains opaque.
    #[must_use]
    pub fn owned_bytes(&self) -> Option<usize> {
        self.plan.specifications[self.slot].bytes.get()
    }
}
/// Completed private statistical allocation history, independent of numerical resource lifetimes.
#[derive(Clone, Debug)]
pub struct StatisticsInventory {
    /// Shared original definitions and finite request table.
    pub(super) plan: Rc<Plan>,
    /// Completed request projections, excluding unexecuted conditional branches.
    pub(super) allocations: Vec<StatisticsAllocation>,
    /// Maximum simultaneous observed vector capacity in this scope.
    pub(super) peak: usize,
    /// Exact newly owned aligned metadata admitted before numerical allocation.
    pub(super) metadata: usize,
}
impl StatisticsInventory {
    /// Original numerical execution, distinct from any previous factor or source execution.
    #[must_use]
    pub fn execution(&self) -> ExecutionId {
        self.plan.trace.execution()
    }
    /// Completed requests retain their original model, shape and parameter role.
    #[must_use]
    pub fn allocations(&self) -> &[StatisticsAllocation] {
        &self.allocations
    }
    /// Original checked operation; absence identifies the closed ordered cross-covariance law.
    #[must_use]
    pub fn operation(&self) -> Option<&crate::CheckedOperation> {
        self.plan.operation.as_ref()
    }
    /// Exact selected input dependencies in original operand order.
    pub fn inputs(&self) -> impl ExactSizeIterator<Item = &Dependency> {
        self.plan.bindings.iter().map(|binding| &binding.dependency)
    }
    /// Exact common original source-column order used by the contractions.
    #[must_use]
    pub fn source_columns(&self) -> &[usize] {
        &self.plan.columns
    }
    /// Original input value and own-target companion symbols.
    /// # Errors
    /// Refuses an input outside the original finite operand inventory.
    pub fn input_symbols(
        &self,
        index: usize,
    ) -> Result<&nrz_bindings::BindingSymbols, OperationError> {
        self.plan
            .bindings
            .get(index)
            .map(|binding| &binding.symbols)
            .ok_or_else(|| crate::PrivateAllocationError::UnplannedMeaning.into())
    }
    /// Whether this operand reads previously published parameters instead of an original joint declaration.
    /// # Errors
    /// Refuses an input outside the original finite operand inventory.
    pub fn input_is_propagated(&self, index: usize) -> Result<bool, OperationError> {
        self.plan
            .bindings
            .get(index)
            .map(|input| {
                matches!(
                    input.origin,
                    crate::statistics::capture::Origin::Propagated(_)
                )
            })
            .ok_or_else(|| crate::PrivateAllocationError::UnplannedMeaning.into())
    }
    /// Original logical row selection, retaining repetitions and order before numerical capture.
    /// # Errors
    /// Refuses an input outside the original finite operand inventory.
    pub fn input_rows(&self, index: usize) -> Result<&[usize], OperationError> {
        self.plan
            .bindings
            .get(index)
            .map(|input| input.rows.as_slice())
            .ok_or_else(|| crate::PrivateAllocationError::UnplannedMeaning.into())
    }
    /// Exact original model identity shared by every covered operand.
    #[must_use]
    pub fn model(&self) -> crate::ModelId {
        self.plan.bindings[0].lineage.model.id
    }
    /// Original affine or explicitly linearized law retained at capture and classification.
    #[must_use]
    pub fn assurance(&self) -> crate::MomentAssurance {
        self.plan.assurance.get()
    }
    /// Common observed scope peak, never a sum of nested matrix peaks.
    #[must_use]
    pub const fn peak_payload_bytes(&self) -> usize {
        self.peak
    }
    /// Aligned original plan, copied selections and historical projections.
    #[must_use]
    pub const fn metadata_bytes(&self) -> usize {
        self.metadata
    }
    /// Named admitted backend storage and workspace allowance, not allocator measurement.
    #[must_use]
    pub fn backend_allowance_bytes(&self) -> usize {
        self.plan.backend.get()
    }
}
