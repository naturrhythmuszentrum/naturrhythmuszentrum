// SPDX-License-Identifier: AGPL-3.0-only
//! Literal field inventories are independent of production byte-accounting helpers.

use super::*;
use crate::loan::registry;
use std::mem::{align_of, offset_of, size_of};
use std::rc::Weak;

/// No checked correspondence can be manufactured by these layout tests.
#[expect(
    clippy::empty_enums,
    reason = "layout tests must not manufacture checked correspondence"
)]
enum Unavailable {}

/// Two native counters precede a shared payload under the accounting ABI.
#[repr(C)]
struct Shared<T> {
    /// Strong reference counter.
    strong: usize,
    /// Weak reference counter.
    weak: usize,
    /// Aligned shared payload.
    payload: T,
}

/// Complete proposed alternatives, without installing any executable dispatch.
enum CandidateState {
    /// Private prospective occupation.
    Reserved,
    /// Existing detached host expiration witness.
    Out(Weak<registry::Liveness>),
    /// Existing immutable ordinary bundle root.
    Published(Rc<registry::BundleOwner>),
    /// Payload absent while a terminal receipt remains.
    Retiring,
    /// Proposed LC detached expiration witness.
    LcOut(Weak<LcLiveness>),
    /// Proposed LC immutable bundle root.
    LcPublished(Rc<LcBundleOwner<Unavailable>>),
}

/// The existing largest inventory remains the prepared application's 23 slots.
enum CandidateInventory {
    /// Existing host debit inventory.
    Host([Option<(usize, u64)>; 15]),
    /// Existing local application debit inventory.
    Application([Option<(usize, u64)>; 17]),
    /// Existing largest prepared application inventory.
    PreparedApplication([Option<(usize, u64)>; 23]),
    /// Existing two-input application inventory.
    BinaryApplication([Option<(usize, u64)>; 18]),
    /// Proposed nineteen-slot LC inventory.
    LcApplication([Option<(usize, u64)>; 19]),
}

/// All existing allocation-kind alternatives plus the three LC expiration witnesses.
enum CandidateAllocationKind {
    /// Native scalar slab.
    Numerical,
    /// Native initialization words.
    Tracking,
    /// Existing owner header.
    Owner,
    /// Existing ordinary pin header.
    Pin,
    /// Existing filled words and shell.
    Filled,
    /// Existing proof owner witness and evidence allowance.
    PreparedOwner {
        /// Expiration witness only.
        witness: Weak<crate::loan::retained_expm1::PreparedExpm1Owner>,
        /// Separately reported evidence allowance.
        evidence: usize,
    },
    /// Existing proof-use witness.
    PreparedUse(Weak<crate::loan::retained_expm1::PreparedExpm1UseRecord>),
    /// Existing type-erased constant witness.
    Constant(Weak<dyn std::any::Any>),
    /// Proposed LC detached owner witness.
    LcPending(Weak<LcLiveness>),
    /// Proposed LC bundle witness.
    LcBundle(Weak<LcBundleOwner<Unavailable>>),
    /// Proposed LC outside-use witness.
    LcUse(Weak<LcUse<Unavailable>>),
}

/// Whole candidate group catches optional-row niche changes as well as enum width.
struct CandidateGroup {
    /// Original occupation.
    identity: Identity,
    /// Proposed closed state alternatives.
    state: CandidateState,
    /// Existing terminal delivery record.
    outcome: registry::ProducerOutcome,
    /// Existing delivery flag.
    delivered: bool,
    /// Inapplicable to LC but physically unchanged for ordinary groups.
    pin: usize,
    /// Proposed closed inventory alternatives.
    debits: CandidateInventory,
}

/// Whole candidate debit includes its optional-table representation.
struct CandidateDebit {
    /// Proposed closed expiration witnesses.
    kind: CandidateAllocationKind,
    /// Existing occupation authority.
    owner: registry::DebitOwner,
    /// Allocation identity.
    key: u64,
    /// Logical scalar capacity.
    elements: usize,
    /// Retained allocation bytes.
    bytes: usize,
}

/// Pinned dyn-stack 0.13.2 layout requirement: two words, no extra option tag.
#[repr(C)]
struct LayoutRequirement {
    /// Zero is the absent alignment niche.
    align: Option<std::num::NonZeroUsize>,
    /// Exact allocation byte requirement.
    size: usize,
}

/// Independent field inventory of faer 0.24.4 `RawMat` and `Own`, not its size helper.
#[repr(C)]
struct MatrixOwner<P> {
    /// `RawMat`'s scalar allocation pointer.
    pointer: std::ptr::NonNull<P>,
    /// `RawMat`'s admitted row capacity.
    row_capacity: usize,
    /// `RawMat`'s admitted column capacity.
    col_capacity: usize,
    /// `RawMat`'s retained `StackReq` is the previously omitted two words.
    layout: LayoutRequirement,
    /// `Own`'s logical row count.
    rows: usize,
    /// `Own`'s logical column count.
    cols: usize,
    /// `RawMat`'s ownership marker adds no bytes.
    marker: std::marker::PhantomData<P>,
}

/// Closed native format enum over the independently inventoried owner payloads.
enum NumericalInventory {
    /// Binary32 owner.
    Single(MatrixOwner<f32>),
    /// Binary64 owner.
    Double(MatrixOwner<f64>),
}

/// Pinned faer 0.24.4 and dyn-stack 0.13.2 supply seven native words per `Mat`.
fn native_matrix_inventory<P: nrz_scalar::Scalar>() {
    // RawMat: pointer + two capacities + StackReq(align,size) = five words.
    // Own adds nrows and ncols: seven words = 56 bytes, for either scalar format.
    assert_eq!(size_of::<LayoutRequirement>(), 16);
    assert_eq!(align_of::<LayoutRequirement>(), 8);
    assert_eq!(size_of::<MatrixOwner<P>>(), 56);
    assert_eq!(align_of::<MatrixOwner<P>>(), 8);
    assert_eq!(size_of::<faer::Mat<P>>(), 56);
    assert_eq!(align_of::<faer::Mat<P>>(), 8);
    // Two inhabited variants need an eight-byte tag/alignment area: 56 + 8 = 64.
    assert_eq!(size_of::<NumericalInventory>(), 64);
    assert_eq!(align_of::<NumericalInventory>(), 8);
    assert_eq!(size_of::<Numerical>(), 64);
    assert_eq!(align_of::<Numerical>(), 8);
}

/// The two-byte reason follows two bytes of discriminants without a padding hole.
/// ´test:unit:lc-origin-outcome-layout´
#[test]
fn lc_origin_outcome_layout() {
    assert_eq!((size_of::<LcOutcome>(), align_of::<LcOutcome>()), (4, 2));
    assert_eq!(
        [
            offset_of!(LcOutcome, status),
            offset_of!(LcOutcome, target),
            offset_of!(LcOutcome, reason)
        ],
        [0, 1, 2]
    );
    assert_eq!(size_of::<[LcOutcome; 3]>(), 12);
}

/// Format-erased fixed records retain the same literal inventory for both scalar backings.
/// ´test:unit:lc-origin-native-layout-inventory´
#[test]
fn lc_origin_native_layout_inventory() {
    inventory::<f32>();
    inventory::<f64>();
}

/// Both scalar choices must share the same format-erased owner inventory.
fn inventory<P: nrz_scalar::Scalar>() {
    // C is deliberately uninhabited. Neither format can create a fake checked owner.
    assert_eq!(size_of::<Rc<Unavailable>>(), 8);
    native_matrix_inventory::<P>();
    // BundlePin owns the same 8-byte Rc<PinRecord> edge as the row, plus a zero-size marker.
    assert_eq!(size_of::<crate::BundlePin<P>>(), 8);
    // Source row: 48 symbols + 8 pin + 16 position/schema + 432 occupations
    // + 288 axes + 72 projection + 24 layout + 672 qualified companions.
    assert_eq!(size_of::<LcSourceRow>(), 1560);
    assert_eq!(align_of::<LcSourceRow>(), 8);
    // The debit fields are three numerical, six tracking, and ten named owners/backings.
    assert_eq!(size_of::<Option<Token>>(), 32);
    assert_eq!(size_of::<LcDebits>(), 608);
    assert_eq!(align_of::<LcDebits>(), 8);
    // Use payload: 8 identity + 8 account + 8 bundle Rc + 24 token = 48.
    assert_eq!(size_of::<LcUsePayload<Unavailable>>(), 48);
    assert_eq!(align_of::<LcUsePayload<Unavailable>>(), 8);
    assert_eq!(size_of::<Option<LcSeal>>(), 400);
    assert_eq!(offset_of!(LcBundleOwner<Unavailable>, seal), 8);
    assert_eq!(
        [
            offset_of!(LcDebits, numerical),
            offset_of!(LcDebits, tracking),
            offset_of!(LcDebits, storage),
            offset_of!(LcDebits, liveness),
            offset_of!(LcDebits, layout),
            offset_of!(LcDebits, outcomes),
            offset_of!(LcDebits, completed_words),
            offset_of!(LcDebits, completed_header),
            offset_of!(LcDebits, closure),
            offset_of!(LcDebits, rows),
            offset_of!(LcDebits, bundle),
            offset_of!(LcDebits, initial_use),
        ],
        [0, 96, 288, 320, 352, 384, 416, 448, 480, 512, 544, 576]
    );
    assert_eq!(
        [
            size_of::<Numerical>(),
            // Each channel adds two 24-byte Vec headers to Numerical: 64 + 48 = 112.
            size_of::<LcChannel>(),
            // Schedule: 16 positions + 8 specialization + 16 revisions + 1 format = 41, padded to 48.
            size_of::<LcSchedule>(),
            // Seal: 216 + 3*8 + 48 + 48 + 8 + 48 + 3 = 395, padded to 400.
            size_of::<LcSeal>(),
            // Storage: 216 identity + 48 schedule + 24 symbols + 48 demand + 48 shared edges
            // + 24 outcome Vec + 608 tokens + 32 cleanup + 8 liveness = 1056;
            // three 112-byte channels bring the total to 1392.
            size_of::<LcStorage<Unavailable>>(),
            // Liveness: 216 identity + 8 receipt = 224.
            size_of::<LcLiveness>(),
            size_of::<LcClosure>(),
            size_of::<LcBundleOwner<Unavailable>>(),
            size_of::<LcUse<Unavailable>>(),
            size_of::<LayoutRecord>(),
            size_of::<MaskRecord>(),
        ],
        [64, 112, 48, 400, 1392, 224, 32, 408, 48, 232, 48]
    );
    assert_eq!(
        [
            align_of::<LcChannel>(),
            align_of::<LcSchedule>(),
            align_of::<LcSeal>(),
            align_of::<LcStorage<Unavailable>>(),
            align_of::<LcLiveness>(),
            align_of::<LcClosure>(),
            align_of::<LcBundleOwner<Unavailable>>(),
            align_of::<LcUse<Unavailable>>(),
            align_of::<LayoutRecord>(),
            align_of::<MaskRecord>(),
        ],
        [8; 10]
    );
    assert_eq!(
        [
            size_of::<Shared<LcLiveness>>(),
            size_of::<Shared<LcClosure>>(),
            size_of::<Shared<LcBundleOwner<Unavailable>>>(),
            size_of::<Shared<LcUse<Unavailable>>>(),
            size_of::<Shared<LayoutRecord>>(),
            size_of::<Shared<MaskRecord>>(),
        ],
        [240, 48, 424, 64, 248, 64]
    );
}

/// Candidate alternatives must fit the existing closed records, not widen byte pins.
/// ´test:unit:lc-origin-common-layout-preservation´
#[test]
fn lc_origin_common_layout_preservation() {
    // The largest payload is 23 * 24 = 552. An inner Option discriminant niche
    // carries the outer inventory tag; there is no additional eight-byte tag.
    assert_eq!(size_of::<Option<(usize, u64)>>(), 24);
    assert_eq!(size_of::<[Option<(usize, u64)>; 23]>(), 552);
    assert_eq!(size_of::<Option<CandidateGroup>>(), 1032);
    assert_eq!(align_of::<Option<CandidateGroup>>(), 8);
    assert_eq!(size_of::<Option<CandidateDebit>>(), 264);
    assert_eq!(align_of::<Option<CandidateDebit>>(), 8);
    assert_eq!(
        [
            size_of::<registry::State>(),
            size_of::<CandidateState>(),
            size_of::<registry::AllocationKind>(),
            size_of::<CandidateAllocationKind>(),
            size_of::<registry::DebitInventory>(),
            size_of::<CandidateInventory>()
        ],
        [16, 16, 24, 24, 552, 552]
    );
    assert_eq!(
        [
            align_of::<registry::State>(),
            align_of::<CandidateState>(),
            align_of::<registry::AllocationKind>(),
            align_of::<CandidateAllocationKind>(),
            align_of::<registry::DebitInventory>(),
            align_of::<CandidateInventory>()
        ],
        [8; 6]
    );
    assert_eq!(
        [
            size_of::<registry::Bundle>(),
            size_of::<registry::BundleOwner>(),
            size_of::<registry::PinRecord>(),
            size_of::<registry::PinPayload>(),
            size_of::<Option<registry::Group>>(),
            size_of::<Option<registry::Debit>>()
        ],
        [3200, 3200, 56, 56, 1032, 264]
    );
    assert_eq!(
        [
            align_of::<registry::Bundle>(),
            align_of::<registry::BundleOwner>(),
            align_of::<registry::PinRecord>(),
            align_of::<registry::PinPayload>(),
            align_of::<Option<registry::Group>>(),
            align_of::<Option<registry::Debit>>()
        ],
        [8; 6]
    );
}
