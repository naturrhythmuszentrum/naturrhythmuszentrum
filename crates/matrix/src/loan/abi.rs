// SPDX-License-Identifier: AGPL-3.0-only
//! Independent field inventories for every record widened by prepared application storage.

use super::registry::{
    CleanupReceipt, Declaration, Identity, Liveness, PinRecord, ProducerOutcome,
    QualifiedMaskVersion, State, Token,
};
use crate::mask::storage::{LayoutRecord, MaskRecord};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Shared allocation ABI includes both counters before the payload.
#[repr(C)]
struct Shared<T> {
    /// Strong reference count.
    strong: usize,
    /// Weak reference count.
    weak: usize,
    /// Aligned payload.
    value: T,
}
/// Exact runtime law identity is independent of symbolic equality.
#[repr(C)]
struct LawIdentity {
    /// Heap identity.
    scope: u64,
    /// Original account.
    account: u64,
    /// Fresh owner identity.
    owner: u64,
}
/// Domain and schedule revisions are retained in the captured use binding.
#[repr(C)]
struct Seal {
    /// Exact binary64 endpoint encodings.
    domain: [u64; 2],
    /// Law, schedule, lowering and arithmetic revisions.
    revisions: [u32; 4],
}
/// Operation-to-use pairing never retains a second numerical owner.
#[repr(C)]
struct Binding {
    /// Original law scope and account.
    identity: LawIdentity,
    /// Exact funded use.
    use_identity: u64,
    /// Immutable domain and revisions.
    seal: Seal,
}
/// Fixed original correspondence fields contain only symbolic witnesses and binding metadata.
#[repr(C)]
struct Operation {
    /// Checked one-input/output operation and zero native allowance.
    checked: crate::CheckedOperation,
    /// Original host value.
    source: nrz_bindings::BindingSymbols,
    /// Independently declared result.
    destination: nrz_bindings::BindingSymbols,
    /// Original cell association.
    axis: crate::CellAxis,
    /// Exact law/use/domain pairing.
    binding: Binding,
    /// Admitted target.
    bits: u32,
}
/// The optional attachment remains one already funded pointer.
#[repr(C)]
struct Attachment {
    /// Shared use record.
    law: Rc<()>,
}
/// Every immutable outcome has an inline, lossless first refusal.
#[expect(
    dead_code,
    reason = "the mirror enumerates ABI alternatives without constructing outcomes"
)]
enum Outcome {
    NotRequested,
    MissingInput,
    Pending,
    Completed,
    PreparedRefused {
        component: usize,
        reason: nrz_scalar::Expm1Failure,
    },
    CellRefused {
        component: usize,
        reason: crate::KernelError,
    },
}
/// Completion metadata contains no new numerical or exact-evidence backing.
#[repr(C)]
struct Evidence {
    /// Closed prepared alternative.
    prepared: Option<Operation>,
    /// Fixed Lower/Upper allocation token identities.
    endpoints: Option<[(usize, u64, u64); 2]>,
    /// Exact immutable completed owner; a non-owning identity adds no allocation or edge.
    completed_mask: Option<std::ptr::NonNull<MaskRecord>>,
    /// Original shape witnesses.
    operation: crate::CheckedOperation,
    /// Excluded input shapes.
    inputs: Rc<[crate::RegionContract]>,
    /// Excluded output shapes.
    outputs: Rc<[crate::RegionContract]>,
    /// Original numerical contract.
    law: Rc<crate::kernels::KernelContract>,
    /// Historical source declaration.
    source: Declaration,
    /// Historical source occupation.
    source_identity: Identity,
    /// Original filled attachments.
    attachments: [Option<QualifiedMaskVersion>; 3],
    /// Original demand.
    demand: crate::MaskVersion,
    /// Result occupation.
    destination: Identity,
    /// Arithmetic completion revision.
    revision: u32,
    /// The one-byte scope shares the existing revision alignment slot.
    error_scope: super::NumericalErrorScope,
    /// Separately allocated fixed-size outcomes.
    outcomes: Vec<Outcome>,
    /// Outcome allocation token.
    token: Token,
}
/// An immutable numerical owner is its existing native matrix plus exact payload token.
#[repr(C)]
struct PublishedChannel {
    /// Native slab metadata, with no per-endpoint shared header.
    matrix: super::payload::Numerical,
    /// Exact retained allocation.
    token: Token,
}
/// Result root and pins all share this inline numerical inventory.
#[repr(C)]
struct Bundle {
    /// One funded law edge.
    law: Option<Attachment>,
    /// Host absence or application completion evidence.
    origin: Option<Evidence>,
    /// Result symbolic declaration.
    declaration: Declaration,
    /// Exact result occupation.
    identity: Identity,
    /// Inline Lower/Upper pair.
    endpoints: Option<[PublishedChannel; 2]>,
    /// Center/total/error roles.
    channels: [Option<PublishedChannel>; 3],
    /// Shared identity layout.
    layout: Rc<LayoutRecord>,
    /// Shared completed mask.
    mask: Rc<MaskRecord>,
    /// Qualified host-channel attachments.
    qualified: [Option<QualifiedMaskVersion>; 3],
    /// Prepaid destruction.
    cleanup: CleanupReceipt,
    /// Layout, bundle and mask tokens.
    retained: [Token; 3],
}
/// The existing immutable bundle shell has no allocation beside its optional payload.
#[repr(C)]
struct BundleOwner {
    /// Absent only during private preparation.
    payload: Option<Bundle>,
}
/// Existing host fields are counted once inside the enclosing application core.
#[repr(C)]
struct Host {
    /// Funded law edge; host requests leave it absent.
    law: Option<Attachment>,
    /// Managed identity.
    identity: Identity,
    /// Immutable declaration.
    declaration: Declaration,
    /// Existing separately owned numerical and tracking channels.
    channels: [Option<super::payload::LoanChannel>; 3],
    /// Shared layout.
    layout: Rc<LayoutRecord>,
    /// Private result shell.
    bundle: Rc<BundleOwner>,
    /// Private initial pin shell.
    pin: Rc<PinRecord>,
    /// Fixed header allocation identities.
    headers: [Token; 5],
    /// Destruction entitlements.
    cleanup: CleanupReceipt,
    /// Final liveness witness.
    witness: Rc<Liveness>,
}
/// One-cell invocation seal owns extents but no full numerical recipe.
#[repr(C)]
struct CellContract {
    inputs: Rc<[crate::Extent]>,
    output: crate::Extent,
    kind: crate::KernelKind,
    work: usize,
}
/// No additional box is introduced for either endpoints or the original correspondence.
#[repr(C)]
struct Core<P: Scalar> {
    /// Two complete endpoint storage roles, inline.
    endpoints: Option<[super::payload::LoanChannel; 2]>,
    /// Transferred source pin.
    input: Option<crate::BundlePin<P>>,
    /// One-cell extent owner and closed recipe.
    contract: CellContract,
    /// Per-cell extent debit.
    extent_token: Token,
    /// Prospective completed words.
    filled: Vec<u64>,
    /// Completed word debit.
    filled_token: Token,
    /// Complete operation/outcome seal.
    evidence: Evidence,
    /// Original selected population.
    population: usize,
    /// Detached phase.
    candidate: bool,
    /// Common owners; witness expires last.
    host: Host,
}
/// All optional group rows reserve the largest closed cancellation inventory, even when vacant.
#[expect(
    dead_code,
    reason = "the mirror declares all inventory layouts without producing authority"
)]
enum Inventory {
    Host([Option<(usize, u64)>; 15]),
    Application([Option<(usize, u64)>; 17]),
    Prepared([Option<(usize, u64)>; 23]),
}
/// Every request family shares this widened optional group-table row.
#[repr(C)]
struct Group {
    /// Exact occupation.
    identity: Identity,
    /// Runtime ownership phase.
    state: State,
    /// Fixed terminal outcome.
    outcome: ProducerOutcome,
    /// Delivery status.
    delivered: bool,
    /// Initial pin index.
    pin: usize,
    /// Complete bounded cancellation inventory.
    inventory: Inventory,
}
/// New outcome width is obtained from an independently declared enum.
pub(super) const fn outcome_width() -> usize {
    size_of::<Outcome>()
}
/// Every occupied or vacant group row uses the largest inventory alternative.
pub(super) const fn group_row() -> usize {
    size_of::<Option<Group>>()
}
/// Changed immutable bundle header, including both endpoint records and original correspondence.
pub(super) const fn bundle_header() -> usize {
    size_of::<Shared<BundleOwner>>()
}
/// Original host core changes only through shared pointees, not its own pointer-sized fields.
pub(super) const fn host_header() -> usize {
    size_of::<Host>()
}
/// Complete application header inventory includes the one-element extent allocation once.
pub(super) const fn application_headers<P: Scalar>() -> usize {
    size_of::<Core<P>>()
        + size_of::<Shared<LayoutRecord>>()
        + size_of::<Shared<Liveness>>()
        + bundle_header()
        + size_of::<Shared<PinRecord>>()
        + size_of::<Shared<[crate::Extent; 1]>>()
}
/// Mirrors are compared to actual records only after supplying the independent expected inventory.
#[test]
fn prepared_application_independent_abi_layouts() {
    assert_eq!(
        size_of::<Operation>(),
        size_of::<crate::PreparedExpm1Operation>()
    );
    assert_eq!(
        size_of::<Evidence>(),
        size_of::<super::application::Evidence>()
    );
    assert_eq!(
        size_of::<PublishedChannel>(),
        size_of::<super::payload::PublishedChannel>()
    );
    assert_eq!(size_of::<Bundle>(), size_of::<super::registry::Bundle>());
    assert_eq!(
        size_of::<Core<f64>>(),
        size_of::<super::application::ApplicationCore<f64>>()
    );
    assert_eq!(host_header(), size_of::<super::lifecycle::HostCore>());
    assert_eq!(outcome_width(), size_of::<crate::CellOutcome>());
    assert_eq!(group_row(), size_of::<Option<super::registry::Group>>());
    // The local seal has 16-byte inputs, a 24-byte extent, a two-byte kind,
    // six alignment bytes and eight Work bytes: 56 instead of the former 128.
    assert_eq!(size_of::<CellContract>(), 56);
    assert_eq!(
        [
            size_of::<Operation>(),
            size_of::<Evidence>(),
            size_of::<Bundle>(),
            size_of::<Core<f64>>(),
            outcome_width(),
            group_row(),
            bundle_header(),
            application_headers::<f64>()
        ],
        [208, 1664, 3200, 3224, 32, 1032, 3216, 7040]
    );
}
