// SPDX-License-Identifier: AGPL-3.0-only
//! Historical solve requests distinguish opaque storage from observable arrays.

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
    /// Represented nominal RHS or a checked differential RHS center.
    SensitivityRightHandSide,
    /// Exact nominal or differential RHS parameter enclosure, with two endpoints per entry.
    SensitivityRightHandSideTruth,
    /// Backend copy of the nominal or differential RHS.
    SensitivityPackedRightHandSide,
    /// Opaque nominal or differential backend solution candidate.
    SensitivityBackendResult,
    /// Matrix-owned nominal or differential solution centers.
    SensitivityCenters,
    /// Independent absolute residual endpoints including RHS parameter error.
    SensitivityResiduals,
    /// Independently certified exact nominal or differential solution enclosure.
    SensitivityTruth,
    /// Local Jacobian centers in concatenated original operator/RHS coordinate order.
    JacobianCenters,
    /// Local Jacobian parameter enclosures in the same original coordinate order.
    JacobianTruth,
    /// Affine offset centers, including an optional exact restoration origin.
    OffsetCenters,
    /// Affine offset parameter enclosures after optional restoration.
    OffsetTruth,
    /// Same-format local Jacobian centers retained by the statistical candidate.
    OutputJacobianCenters,
    /// Same-format local Jacobian enclosures retained by the statistical candidate.
    OutputJacobianTruth,
    /// Same-format offsets consumed by statistical mean propagation.
    OutputOffsetCenters,
    /// Same-format offset enclosures consumed by statistical mean propagation.
    OutputOffsetTruth,
    /// Original captured input sensitivities remapped into the common source order.
    StatisticalInputs,
    /// Independent parameter enclosures for the stacked input sensitivities.
    StatisticalInputsTruth,
    /// The original model covariance restricted to the exact common source order.
    StatisticalModel,
    /// Independent source covariance parameter enclosures.
    StatisticalModelTruth,
    /// Transposed composed source sensitivity used by the final covariance contraction.
    StatisticalTranspose,
    /// Transposed source sensitivity parameter enclosures.
    StatisticalTransposeTruth,
    /// Explicit input expectations in original concatenated input order.
    StatisticalMeans,
    /// Independent explicit input expectation enclosures.
    StatisticalMeansTruth,
    /// First backend operand of a predeclared statistical contraction.
    ContractionLeft,
    /// Second backend operand of a predeclared statistical contraction.
    ContractionRight,
    /// Opaque backend product of the two predeclared statistical operands.
    ContractionResult,
    /// Matrix-owned statistical product candidate, without a redundant zero-center allocation.
    ContractionCenters,
    /// Independently accumulated statistical product parameter enclosures.
    ContractionTruth,
    /// Final parameter-center copy for publication; the index selects its exact parameter role.
    ParameterCenters,
    /// Final numerical parameter-error copy for publication, targeting that same parameter.
    ParameterErrors,
}
impl SolveAllocationStage {
    /// Opaque backend requests expose logical rectangles, never fabricated allocator capacities.
    #[must_use]
    pub const fn is_backend(self) -> bool {
        matches!(
            self,
            Self::PackedRightHandSide
                | Self::BackendResult
                | Self::SensitivityPackedRightHandSide
                | Self::SensitivityBackendResult
                | Self::ContractionLeft
                | Self::ContractionRight
                | Self::ContractionResult
        )
    }
}
/// One completed request projects its immutable meaning and observed capacity from the original plan.
#[derive(Clone)]
pub struct SolveAllocation {
    /// Original shared definition and allocation observation; no duplicate schedule is retained.
    pub(super) plan: Rc<super::Plan>,
    /// Exact finite stage consumed by the original non-cloneable owner.
    pub(super) slot: usize,
}
impl SolveAllocation {
    /// Mathematical role of each coordinate, interpreted through the inventory's original bindings.
    #[must_use]
    pub fn stage(&self) -> SolveAllocationStage {
        self.specification().stage
    }
    /// Certificate attempt, with zero denoting the initial solve and absence for other phases.
    #[must_use]
    pub fn iteration(&self) -> Option<usize> {
        let stage = self.stage();
        if stage.is_sensitivity() || stage.is_statistical() {
            None
        } else {
            self.specification().iteration
        }
    }
    /// Nominal solve is zero; later values are one plus the original differentiated input coordinate.
    #[must_use]
    pub fn sensitivity_solve(&self) -> Option<usize> {
        if self.stage().is_sensitivity() {
            self.specification().iteration
        } else {
            None
        }
    }
    /// Statistical contraction: zero composes sources, one multiplies their covariance, two completes covariance, three propagates means.
    #[must_use]
    pub fn contraction(&self) -> Option<usize> {
        if self.stage().is_contraction() {
            self.specification().iteration
        } else {
            None
        }
    }
    /// Published parameter: zero covariance, one source sensitivity, two local Jacobian, three means.
    #[must_use]
    pub fn parameter(&self) -> Option<usize> {
        if matches!(
            self.stage(),
            SolveAllocationStage::ParameterCenters | SolveAllocationStage::ParameterErrors
        ) {
            self.specification().iteration
        } else {
            None
        }
    }
    /// Complete logical request rectangle in column order.
    #[must_use]
    pub fn extent(&self) -> Extent {
        self.specification().extent
    }
    /// Ordered logical coordinate for one requested entry; hidden backend padding is never an entry.
    #[must_use]
    pub fn coordinate(&self, index: usize) -> Option<[usize; 2]> {
        if index >= self.entries() {
            None
        } else {
            Some([index % self.extent().rows(), index / self.extent().rows()])
        }
    }
    /// Number of logical coordinates requested, independently of enclosure endpoint count.
    #[must_use]
    pub fn entries(&self) -> usize {
        self.extent().entries()
    }
    /// Primitive floats requested, excluding hidden backend padding and workspace.
    #[must_use]
    pub fn float_entries(&self) -> usize {
        self.entries() * self.stage().components()
    }
    /// Exact logical scalar bytes, checked before a completed projection is exposed.
    #[must_use]
    pub fn logical_bytes(&self) -> usize {
        self.float_entries() * self.plan.width
    }
    /// Actual vector capacity times scalar width; opaque backend requests return absence.
    #[must_use]
    pub fn owned_bytes(&self) -> Option<usize> {
        self.specification().bytes.get()
    }
    /// Historical allocation authority, never permission to allocate or read a live region.
    #[must_use]
    pub fn ticket(&self) -> PrivateAllocationTicket {
        PrivateAllocationTicket::new(self.plan.trace.execution(), self.plan.token, self.slot)
    }
    /// Publication creates projections only for valid slots whose allocation observation is final.
    fn specification(&self) -> &super::plan::Specification {
        &self.plan.specifications[self.slot]
    }
}
impl PartialEq for SolveAllocation {
    fn eq(&self, other: &Self) -> bool {
        self.ticket() == other.ticket()
    }
}
impl Eq for SolveAllocation {}
impl std::fmt::Debug for SolveAllocation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SolveAllocation")
            .field("stage", &self.stage())
            .field("index", &self.specification().iteration)
            .field("extent", &self.extent())
            .field("ticket", &self.ticket())
            .field("owned_bytes", &self.owned_bytes())
            .finish()
    }
}
/// Scoped general solve, recovery, sensitivity and moment array inventory.
///
/// Factor construction, inline scalar proof fields, captured input parameters and destination slabs
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
        self.plan.trace.execution() == other.plan.trace.execution()
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
    /// Original model identity for statistical contractions, absent for deterministic-only solves.
    #[must_use]
    pub fn model(&self) -> Option<crate::ModelId> {
        self.plan.statistics.as_ref().map(|v| v.id)
    }
    /// Original selected source-column order used by every statistical contraction.
    #[must_use]
    pub fn statistical_sources(&self) -> &[usize] {
        self.plan
            .statistics
            .as_ref()
            .map_or(&[], |v| v.columns.as_slice())
    }
    /// Original bindings-owned nominal/differential equation, when statistics are present.
    #[must_use]
    pub fn derivative(&self) -> Option<&nrz_bindings::CheckedSensitivity> {
        self.plan.derivative.as_deref()
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
    /// Named backend reservation: the factor/solve allowance plus existing statistical work admission, never measured memory.
    #[must_use]
    pub fn backend_allowance_bytes(&self) -> usize {
        (self.plan.contract.work + self.plan.statistics.as_ref().map_or(0, |v| v.work))
            * self.plan.width
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
            + self.plan.statistics.as_ref().map_or(0, |v| {
                size_of_val(v.as_ref())
                    + (v.columns.capacity() + v.roots.capacity()) * size_of::<usize>()
            })
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
