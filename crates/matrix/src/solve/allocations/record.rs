// SPDX-License-Identifier: AGPL-3.0-only
//! Logical backend requests remain distinct from observable owned vector capacities.

use super::run::Plan;
use crate::{FactorDependency, PrivateAllocationTicket, RegionContract};
use nrz_bindings::BindingSymbols;
use std::rc::Rc;

/// Closed factor-construction schedule in the original normalized operator coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FactorAllocationStage {
    /// Copy the selected original operator into backend storage.
    PackedOperator,
    /// Opaque partial-pivoted LU storage and its internal workspace allowance.
    Factors,
    /// Source-free identity on the original equation axis for the inverse witness.
    Identity,
    /// Backend solution of the identity columns; this is only an inverse candidate.
    InverseResult,
    /// Matrix-owned scalar copy of that inverse candidate.
    InverseCandidate,
    /// Componentwise absolute upper bounds of the independently enclosed I minus Z A defect.
    InverseDefects,
}
impl FactorAllocationStage {
    /// Complete closed factor schedule, before the first operator packing allocation.
    pub const ALL: [Self; 6] = [
        Self::PackedOperator,
        Self::Factors,
        Self::Identity,
        Self::InverseResult,
        Self::InverseCandidate,
        Self::InverseDefects,
    ];
    /// Whether this request is owned by the opaque backend rather than a matrix vector.
    #[must_use]
    pub const fn is_backend(self) -> bool {
        matches!(
            self,
            Self::PackedOperator | Self::Factors | Self::Identity | Self::InverseResult
        )
    }
}

/// Historical factor-stage ticket with its original exact allocating interpretation.
#[derive(Clone, Debug)]
pub struct FactorAllocation {
    /// Shared original operator symbols, selection and checked coordinate contract.
    pub(super) plan: Rc<Plan>,
    /// Closed stage identifying the quantity computed at each ordered coordinate.
    pub(super) stage: FactorAllocationStage,
    /// Opaque authority drawn before the corresponding allocation call.
    pub(super) ticket: PrivateAllocationTicket,
    /// Actual owned vector capacity, absent for backend-owned storage with hidden padding.
    pub(super) bytes: Option<usize>,
}
impl FactorAllocation {
    /// Closed mathematical role, including candidate versus independently checked defect.
    #[must_use]
    pub const fn stage(&self) -> FactorAllocationStage {
        self.stage
    }
    /// Original source value and companion declarations; no independent replacement graph is built.
    #[must_use]
    pub fn source(&self) -> &BindingSymbols {
        &self.plan.source
    }
    /// Exact selected operator/version and normalized algorithm interpretation.
    #[must_use]
    pub fn dependency(&self) -> &FactorDependency {
        &self.plan.dependency
    }
    /// Original checked operator contract and its ordered equation/unknown axes.
    #[must_use]
    pub fn operator(&self) -> &RegionContract {
        &self.plan.inputs[0]
    }
    /// Column-ordered coordinate of one logical request entry; padding is not an entry.
    #[must_use]
    pub fn coordinate(&self, index: usize) -> Option<[usize; 2]> {
        if index >= self.entries() {
            return None;
        }
        let order = self.plan.order;
        Some([index % order, index / order])
    }
    /// Complete logical square requested for this stage.
    #[must_use]
    pub fn entries(&self) -> usize {
        self.plan.entries
    }
    /// Exact logical scalar bytes, excluding hidden backend capacity and padding.
    #[must_use]
    pub fn logical_bytes(&self) -> usize {
        self.plan.logical
    }
    /// Actual owned vector bytes; backend allocation capacity is deliberately not fabricated.
    #[must_use]
    pub const fn owned_bytes(&self) -> Option<usize> {
        self.bytes
    }
    /// Historical allocation authority, never permission to allocate or read a value.
    #[must_use]
    pub const fn ticket(&self) -> PrivateAllocationTicket {
        self.ticket
    }
    /// Identity of the original allocating plan and stage, independent of equal-shaped declarations.
    #[must_use]
    pub fn same_binding(&self, other: &Self) -> bool {
        self.stage == other.stage && Rc::ptr_eq(&self.plan, &other.plan)
    }
}

/// Completed factor-construction inventory; it does not inventory a later RHS solve.
#[derive(Clone, Debug)]
pub struct FactorInventory {
    /// All closed stages, validated before the factor becomes externally available.
    pub(super) allocations: [FactorAllocation; 6],
    /// Named reservation for opaque backend heap storage and workspaces, not measured capacity.
    pub(super) backend: usize,
    /// Inline backend factor object bytes, excluding its opaque heap allocations.
    pub(super) inline: usize,
    /// Newly owned plan/receipt metadata including copied selection storage.
    pub(super) metadata: usize,
}
impl FactorInventory {
    /// Named opaque backend owner; matrix padding and factor workspaces are included.
    pub const BACKEND_NAME: &str = "faer storage and workspaces";

    /// Every factor-construction allocation request in its planned order.
    #[must_use]
    pub const fn allocations(&self) -> &[FactorAllocation; 6] {
        &self.allocations
    }
    /// Bounded faer storage/workspace reservation derived from admitted work and scalar width.
    ///
    /// This is an execution allowance, not a measurement of faer's private allocator capacity.
    #[must_use]
    pub const fn backend_allowance_bytes(&self) -> usize {
        self.backend
    }
    /// Observable inline factor object size; heap storage remains in the named allowance.
    #[must_use]
    pub const fn inline_factor_bytes(&self) -> usize {
        self.inline
    }
    /// Inline historical receipt bytes; these are not a separately allocated backend matrix.
    #[must_use]
    pub const fn receipt_bytes(&self) -> usize {
        size_of::<Self>()
    }
    /// Aligned shared plan allocation and its independently copied selection storage.
    #[must_use]
    pub const fn planning_bytes(&self) -> usize {
        self.metadata - size_of::<Self>()
    }
    /// Newly owned planning and historical receipt metadata, excluding shared exact declarations.
    #[must_use]
    pub const fn metadata_bytes(&self) -> usize {
        self.metadata
    }
}
