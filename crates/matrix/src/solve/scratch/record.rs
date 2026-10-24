// SPDX-License-Identifier: AGPL-3.0-only
//! Historical deterministic solve requests distinguish opaque storage from observable arrays.

use crate::{Dependency, Extent, PrivateAllocationTicket, RegionContract};
use nrz_bindings::BindingSymbols;
use nrz_scalar::Precision;
use std::rc::Rc;

/// Closed roles over the original checked operator, RHS and restored result coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolveAllocationStage {
    /// Binary64 residual evaluation selects these finite correction RHS centers in candidate format.
    CorrectionRightHandSide,
    /// Backend copy of physical RHS, typed identity, or the current correction RHS.
    PackedRightHandSide,
    /// Opaque backend solution candidate, with no numerical theorem inferred from it.
    BackendResult,
    /// Matrix-owned copy of every backend solution coordinate.
    Centers,
    /// Finite sum of the previous iterate and the backend correction.
    UpdatedCenters,
    /// Independent copy retained for a complete certificate while recovery keeps the iterate.
    RetainedCenters,
    /// Absolute upper endpoints of the original represented residual B minus A X.
    Residuals,
    /// Selected total radius for each original operator coordinate.
    OperatorRadii,
    /// Selected total radius for each physical RHS coordinate, or exact identity zeros.
    RightHandSideRadii,
    /// Candidate centers after the checked affine restoration.
    RestoredCenters,
    /// Complete total radius in final output coordinates.
    Totals,
    /// Overlapping numerical radius in final output coordinates.
    Errors,
    /// New affine restoration discrepancy, counted once in both enclosing channels.
    RestorationErrors,
    /// Same-format copy of final centers, owned by the unpublished candidate.
    OutputCenters,
    /// Same-format copy of final total radii, owned by the unpublished candidate.
    OutputTotals,
    /// Same-format copy of final numerical radii, owned by the unpublished candidate.
    OutputErrors,
}
impl SolveAllocationStage {
    /// Opaque backend requests expose logical rectangles, never fabricated allocator capacities.
    #[must_use]
    pub const fn is_backend(self) -> bool {
        matches!(self, Self::PackedRightHandSide | Self::BackendResult)
    }
}
/// One completed request from the original finite schedule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolveAllocation {
    /// Closed allocating role in the original checked coordinates.
    pub(super) stage: SolveAllocationStage,
    /// Initial/correction attempt or final transfer.
    pub(super) iteration: Option<usize>,
    /// Complete logical request shape.
    pub(super) extent: Extent,
    /// Historical authority drawn before allocation.
    pub(super) ticket: PrivateAllocationTicket,
    /// Requested logical scalar bytes without backend padding.
    pub(super) logical: usize,
    /// Observed vector capacity, or absence for an opaque backend owner.
    pub(super) bytes: Option<usize>,
}
impl SolveAllocation {
    /// Mathematical role of each coordinate, interpreted through the inventory's original bindings.
    #[must_use]
    pub const fn stage(&self) -> SolveAllocationStage {
        self.stage
    }
    /// Certificate attempt, with zero denoting the initial solve and absence denoting final transfer.
    #[must_use]
    pub const fn iteration(&self) -> Option<usize> {
        self.iteration
    }
    /// Complete logical request rectangle in column order.
    #[must_use]
    pub const fn extent(&self) -> Extent {
        self.extent
    }
    /// Ordered logical coordinate for one requested entry; hidden backend padding is never an entry.
    #[must_use]
    pub const fn coordinate(&self, index: usize) -> Option<[usize; 2]> {
        if index >= self.extent.entries() {
            None
        } else {
            Some([index % self.extent.rows(), index / self.extent.rows()])
        }
    }
    /// Number of logical coordinates requested; each coordinate has one scalar component.
    #[must_use]
    pub const fn entries(&self) -> usize {
        self.extent.entries()
    }
    /// Primitive floats requested, excluding hidden backend padding and workspace.
    #[must_use]
    pub const fn float_entries(&self) -> usize {
        self.entries()
    }
    /// Exact logical scalar bytes requested, independent of actual backend allocation.
    #[must_use]
    pub const fn logical_bytes(&self) -> usize {
        self.logical
    }
    /// Actual vector capacity times scalar width; opaque backend requests return absence.
    #[must_use]
    pub const fn owned_bytes(&self) -> Option<usize> {
        self.bytes
    }
    /// Historical allocation authority, never permission to allocate or read a live region.
    #[must_use]
    pub const fn ticket(&self) -> PrivateAllocationTicket {
        self.ticket
    }
}
/// Complete deterministic RHS, certificate, restoration and recovery array inventory.
///
/// Factor construction, inline scalar proof fields, statistics and reserved destination slabs
/// have separate owners. This scoped receipt does not claim a complete execution footprint.
#[derive(Clone)]
pub struct SolveInventory {
    /// Shared exact definitions, source versions and finite schedule.
    pub(super) plan: Rc<super::Plan>,
    /// Every executed request in planned order.
    pub(super) allocations: Vec<SolveAllocation>,
    /// Existing one-output candidate container allocation, separate from new inventory metadata.
    pub(super) candidate: usize,
    /// Largest simultaneous observable vector capacity.
    pub(super) peak: usize,
    /// Observed vector capacity, or absence for an opaque backend owner.
    pub(super) bytes: usize,
}
impl PartialEq for SolveInventory {
    fn eq(&self, other: &Self) -> bool {
        self.plan.execution == other.plan.execution
            && self.plan.token == other.plan.token
            && self.allocations == other.allocations
            && self.peak == other.peak
            && self.bytes == other.bytes
            && self.candidate == other.candidate
    }
}
impl SolveInventory {
    /// Every executed request; a permitted recovery suffix that was never needed is absent.
    #[must_use]
    pub fn allocations(&self) -> &[SolveAllocation] {
        &self.allocations
    }
    /// Original selected input versions and their immutable source/companion symbols.
    #[must_use]
    pub fn sources(&self) -> &[(Dependency, BindingSymbols)] {
        &self.plan.sources
    }
    /// Original checked operator and physical RHS coordinate contracts.
    #[must_use]
    pub fn inputs(&self) -> &[RegionContract] {
        &self.plan.contract.inputs
    }
    /// Original output declaration prepared before numerical execution.
    #[must_use]
    pub fn output(&self) -> &BindingSymbols {
        &self.plan.output
    }
    /// Original checked normalized solution coordinates before any affine restoration.
    #[must_use]
    pub fn solution(&self) -> &RegionContract {
        &self.plan.contract.solution
    }
    /// Exact affine restoration, whose scale and shift distinguish the final output meanings.
    #[must_use]
    pub fn restoration(&self) -> Option<&crate::ConversionContract> {
        self.plan.contract.restoration.as_ref()
    }
    /// A typed identity has no independent physical RHS symbol or source.
    #[must_use]
    pub fn is_inverse(&self) -> bool {
        self.plan.contract.identity.is_some()
    }
    /// Number of independently certified initial/correction attempts in the completed prefix.
    #[must_use]
    pub fn completed_attempts(&self) -> usize {
        self.plan.completed.get()
    }
    /// Candidate scalar representation shared by all requests.
    #[must_use]
    pub fn precision(&self) -> Precision {
        self.plan.precision
    }
    /// Maximum simultaneously owned observable vector capacity within this schedule.
    #[must_use]
    pub const fn peak_payload_bytes(&self) -> usize {
        self.peak
    }
    /// Shared plan, owned schedule/source metadata and this completed receipt allocation.
    #[must_use]
    pub const fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    /// Named backend reservation shared with factor construction, never added to it as measured memory.
    #[must_use]
    pub fn backend_allowance_bytes(&self) -> usize {
        self.plan.contract.work * self.plan.width
    }
    /// Aligned shared plan and its actual finite schedule allocation.
    #[must_use]
    pub fn planning_bytes(&self) -> usize {
        size_of::<super::plan::RcLayout>()
            + self.plan.specifications.capacity() * size_of::<super::plan::Specification>()
    }
    /// Newly copied selected input snapshots and map storage; original exact declarations are shared.
    #[must_use]
    pub fn source_bytes(&self) -> usize {
        self.plan.sources.capacity() * size_of::<(Dependency, BindingSymbols)>()
            + self
                .plan
                .sources
                .iter()
                .map(|(source, _)| source.layout().owned_bytes())
                .sum::<usize>()
    }
    /// Aligned shared historical receipt and its actual request-vector capacity.
    #[must_use]
    pub const fn receipt_bytes(&self) -> usize {
        size_of::<Self>()
            + 2 * size_of::<usize>()
            + self.allocations.capacity() * size_of::<SolveAllocation>()
    }
    /// Existing candidate-container allocation, excluded from newly owned inventory metadata.
    #[must_use]
    pub const fn candidate_metadata_bytes(&self) -> usize {
        self.candidate
    }
    /// Opaque owner covering backend storage, padding and workspaces.
    pub const BACKEND_NAME: &str = super::super::FactorInventory::BACKEND_NAME;
}

impl std::fmt::Debug for SolveInventory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SolveInventory")
            .field("allocations", &self.allocations)
            .field("precision", &self.precision())
            .field("peak", &self.peak)
            .field("metadata", &self.bytes)
            .field("candidate", &self.candidate)
            .finish_non_exhaustive()
    }
}
