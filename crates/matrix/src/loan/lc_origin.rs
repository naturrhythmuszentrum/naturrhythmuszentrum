// SPDX-License-Identifier: AGPL-3.0-only
//! Closed LC owners retain the original law, captured sources and sealed numerical outcomes.
//!
//! C is the future checked correspondence owner, not an erased receipt or a stand-in
//! proof. Only its sized Rc edge is native storage; its backend inventory is separate.
#![cfg_attr(
    test,
    expect(
        dead_code,
        reason = "layout inventories include unconstructed alternatives"
    )
)]

use super::{
    lc_shape::LcShapeAxes,
    payload::Numerical,
    registry::{CleanupReceipt, Identity, PinRecord, QualifiedMaskVersion, Token},
};
use crate::mask::storage::{LayoutRecord, MaskRecord};
use crate::{CellLayoutVersion, CellProduct, Coverage, MaskVersion, NumericalErrorScope};
use nrz_bindings::BindingSymbols;
use std::rc::Rc;

/// No implicit padding: status, target and explicit closed reason code.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LcOutcome {
    /// Explicit version-one outcome status byte.
    status: u8,
    /// Original operand or kernel target code.
    target: u8,
    /// Lossless closed cause code; never a Rust enum discriminant.
    reason: u16,
}

/// Six symbol handles and one mapped-source pin retain each original position.
#[repr(C)]
pub(in crate::loan) struct LcSourceRow {
    /// Original symbol ownership; the complete companions remain private.
    pub(in crate::loan) symbols: [BindingSymbols; 2],
    /// One mapped-source pin retains its entire immutable incoming closure.
    pub(in crate::loan) pin: Rc<PinRecord>,
    /// Original supplied-constraint position, including repeats.
    pub(in crate::loan) position: usize,
    /// Nonwrapping original input schema identity.
    pub(in crate::loan) schema: u64,
    /// Captured original host occupation.
    pub(in crate::loan) source: Identity,
    /// Captured mapped-copy occupation and receipt identity.
    pub(in crate::loan) mapped: Identity,
    /// Original ordered instance, sample and branch roles.
    pub(in crate::loan) axes: LcShapeAxes,
    /// Exact tuple-to-instance affine product map.
    pub(in crate::loan) projection: CellProduct,
    /// Captured source layout version.
    pub(in crate::loan) layout: CellLayoutVersion,
    /// Three complete qualified mapped companion locators.
    pub(in crate::loan) companions: [QualifiedMaskVersion; 3],
}

/// Exact-capacity rows have their own allocation and debit.
#[repr(C)]
pub(in crate::loan) struct LcClosure {
    /// Exact immutable owner or occupation identity.
    pub(in crate::loan) identity: u64,
    /// Exact-capacity original positional row backing.
    pub(in crate::loan) rows: Vec<LcSourceRow>,
}

/// Closed schedule metadata contains no C-sized intermediate storage.
#[repr(C)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::loan) struct LcSchedule {
    /// Original ordered inductance and capacitance positions.
    pub(in crate::loan) positions: [usize; 2],
    /// Immutable requested-quantity specialization identity.
    pub(in crate::loan) specialization: u64,
    /// Correspondence, recipe, lowering and arithmetic revisions.
    pub(in crate::loan) revisions: [u32; 4],
    /// Closed native scalar precision.
    pub(in crate::loan) format: crate::storage::Format,
}

/// The only certainty claim permitted by this layout slice.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LcCertainty {
    /// Neither feasibility nor determinacy is asserted.
    Undecided,
}

/// Inline locators bind the immutable storage; they do not replace checked evidence.
#[repr(C)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct LcSeal {
    /// Exact immutable owner or occupation identity.
    identity: Identity,
    /// Checked matrix-to-correspondence join identity.
    join: u64,
    /// Sealed checked-correspondence allocation identity.
    correspondence: u64,
    /// Sealed captured-closure allocation identity.
    closure: u64,
    /// Fixed original operand and revision schedule.
    schedule: LcSchedule,
    /// Original immutable demand version.
    demand: MaskVersion,
    /// Sealed outcome-backing allocation identity.
    outcomes: u64,
    /// Sealed completed-mask version.
    completed: MaskVersion,
    /// Represented-input outer-cover interpretation.
    coverage: Coverage,
    /// Final reciprocal `LocalOperation` error interpretation.
    error_scope: NumericalErrorScope,
    /// Explicit undecided certainty, independent of numerical completion.
    certainty: LcCertainty,
}

/// The numerical owner stays closed over binary32 and binary64, with both tracking maps.
#[repr(C)]
struct LcChannel {
    /// Closed native numerical storage.
    numerical: Numerical,
    /// Per-component initialization word backing.
    components: Vec<u64>,
    /// Per-cell initialization word backing, retained after publication.
    complete: Vec<u64>,
}

/// Expiration witness is the final retaining field of storage.
#[repr(C)]
#[derive(Debug)]
pub(in crate::loan) struct LcLiveness {
    /// Exact immutable owner or occupation identity.
    identity: Identity,
    /// Original prepaid cleanup receipt identity.
    receipt: u64,
}

/// Nineteen named slots include zero-length backings; no slot is an allocation itself.
#[repr(C)]
struct LcDebits {
    /// Three numerical-storage allocation debit tokens.
    numerical: [Option<Token>; 3],
    /// Six independent component/cell word allocation debits.
    tracking: [Option<Token>; 6],
    /// Boxed-storage allocation debit token.
    storage: Option<Token>,
    /// Liveness shared-header allocation debit token.
    liveness: Option<Token>,
    /// Output-layout allocation debit token.
    layout: Option<Token>,
    /// Outcome-backing allocation debit token.
    outcomes: Option<Token>,
    /// Completed-mask word backing debit.
    completed_words: Option<Token>,
    /// Completed-mask shared header debit.
    completed_header: Option<Token>,
    /// Captured-closure shared-header allocation debit token.
    closure: Option<Token>,
    /// Exact-capacity positional-row backing allocation debit token.
    rows: Option<Token>,
    /// Bundle shared-shell allocation debit token.
    bundle: Option<Token>,
    /// Initial specialized use shell debit, transferred on publication.
    initial_use: Option<Token>,
}

/// Boxed payload survives publication without releasing its initialization backings.
#[repr(C)]
struct LcStorage<C> {
    /// Exact immutable owner or occupation identity.
    identity: Identity,
    /// Fixed original operand and revision schedule.
    schedule: LcSchedule,
    /// Original symbol ownership; the complete companions remain private.
    symbols: BindingSymbols,
    /// Original immutable demand version.
    demand: MaskVersion,
    /// Existing demand-mask edge, moved on cancellation.
    demand_mask: Rc<MaskRecord>,
    /// Existing demand-layout edge, moved on cancellation.
    demand_layout: Rc<LayoutRecord>,
    /// Retained output-layout edge.
    layout: Rc<LayoutRecord>,
    /// Preallocated completed-mask edge.
    completed: Rc<MaskRecord>,
    /// Complete captured-closure edge.
    closure: Rc<LcClosure>,
    /// Checked-correspondence edge.
    correspondence: Rc<C>,
    /// Center, total radius and local error, retaining all six tracking maps.
    channels: [LcChannel; 3],
    /// Exact outcome backing.
    outcomes: Vec<LcOutcome>,
    /// Nineteen movable tokens; a transferred token leaves an empty slot.
    debits: LcDebits,
    /// Prepaid allocation, record and retaining-edge destruction.
    cleanup: CleanupReceipt,
    /// Final expiration edge.
    liveness: Rc<LcLiveness>,
}

/// Private shell is completed before any owning edge escapes; seal is never boxed.
#[repr(C)]
pub(in crate::loan) struct LcBundleOwner<C> {
    /// Boxed storage ownership, absent only while the private shell is empty.
    storage: Option<Box<LcStorage<C>>>,
    /// Inline complete seal, absent only while the private shell is empty.
    seal: Option<LcSeal>,
}

impl<C> LcBundleOwner<C> {
    /// Borrow the law retained by this actual published bundle, without another owner edge.
    pub(in crate::loan) fn retained_correspondence(&self) -> Option<&Rc<C>> {
        self.storage.as_ref().map(|storage| &storage.correspondence)
    }
}

/// Outside uses have their own debit, without an ordinary pin-index entry.
#[repr(C)]
struct LcUsePayload<C> {
    /// Exact immutable owner or occupation identity.
    identity: u64,
    /// Original continuation account identity.
    account: u64,
    /// Immutable bundle edge.
    bundle: Rc<LcBundleOwner<C>>,
    /// Unique specialized-use allocation debit.
    token: Token,
}

/// Empty initial shell is reserved beside storage, never inside its own bundle.
#[repr(C)]
pub(in crate::loan) struct LcUse<C> {
    /// Complete use payload, absent only in the private initial shell.
    payload: Option<LcUsePayload<C>>,
}

#[cfg(test)]
mod tests;

/// Closed native join owner, distinct from any numerical bundle occupation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::loan) struct LcJoinIdentity {
    /// Original heap scope.
    pub(in crate::loan) scope: u64,
    /// Original execution account.
    pub(in crate::loan) account: u64,
    /// Nonwrapping identity shared by the closure and cleanup receipts.
    pub(in crate::loan) join: u64,
}

pub(in crate::loan) mod pending;

#[cfg(test)]
impl Drop for LcLiveness {
    fn drop(&mut self) {
        crate::loan::fault::witness_expiring(self.receipt);
    }
}

impl<C> std::fmt::Debug for LcBundleOwner<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LcBundleOwner").finish_non_exhaustive()
    }
}
impl<C> std::fmt::Debug for LcUse<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LcUse").finish_non_exhaustive()
    }
}
