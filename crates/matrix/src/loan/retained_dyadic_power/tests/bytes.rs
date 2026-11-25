// SPDX-License-Identifier: AGPL-3.0-only
//! Independent aligned layouts and exact decimal inventories for the retained fixture.

use super::{TestResult, fixture};
use crate::loan::registry::{CleanupReceipt, Debit, Identity, Token};
use nrz_bindings::{
    Budget, DyadicPowerPreparation,
    readings::algebra::{Cas, enclosure::Interval},
};
use nrz_scalar::DyadicPowerProgram;
use std::rc::{Rc, Weak};

/// Reference counters and payload alignment form one owned allocation.
#[repr(C)]
struct Shared<T> {
    /// Strong reference count.
    strong: usize,
    /// Weak reference count.
    weak: usize,
    /// Inline owned payload.
    payload: T,
}
/// Original scope, account and owner identities.
#[repr(C)]
struct IdentityMirror {
    /// Heap scope.
    scope: u64,
    /// Original ledger.
    account: u64,
    /// Nonwrapping owner identity.
    owner: u64,
}
/// LC closure scope, account and join identities, independent of the production type.
#[repr(C)]
struct LcJoinIdentityMirror {
    /// Heap scope.
    scope: u64,
    /// Original ledger.
    account: u64,
    /// Shared closure and cleanup identity.
    join: u64,
}
/// LC root locator without borrowing the production key type.
#[expect(dead_code, reason = "the independent key is a layout witness")]
struct LcKeyMirror {
    /// Root slot.
    slot: usize,
    /// Root generation.
    generation: u64,
}
/// LC output format without borrowing the production format type.
#[expect(dead_code, reason = "both LC format variants are layout witnesses")]
enum LcFormatMirror {
    /// Binary32 output.
    Binary32,
    /// Binary64 output.
    Binary64,
}
/// Independent layout of the LC application debit identity.
#[repr(C)]
struct LcDebitIdentityMirror {
    /// Heap scope.
    scope: u64,
    /// Original account.
    account: u64,
    /// Root locator.
    root: LcKeyMirror,
    /// Request identity.
    request: u64,
    /// Nonce identity.
    nonce: u64,
    /// Checked receipt identity.
    receipt: u64,
    /// LC output format.
    format: LcFormatMirror,
}
/// Represented domain, exponent and scalar revisions.
#[repr(C)]
struct SealMirror {
    /// Exact endpoint bits.
    domain: [u64; 2],
    /// Canonical numerator and root degree.
    exponent: (u64, u32),
    /// Argument, arithmetic and separate-step revisions.
    revisions: [u32; 3],
}
/// Actual preparation and native program with two owned key buffers.
#[repr(C)]
struct PartsMirror {
    /// Checked result and bounded proof bytes.
    authority: DyadicPowerPreparation,
    /// Native scalar schedule.
    program: DyadicPowerProgram,
    /// Proof-sensitive authority frame.
    authority_key: Vec<u8>,
    /// Binary64 lowering frame.
    lowering_key: Vec<u8>,
}
/// The graph drops after dependent evidence and program.
#[repr(C)]
struct FrozenMirror {
    /// Returned result, program and keys.
    parts: PartsMirror,
    /// Immutable specialization.
    seal: SealMirror,
    /// Producing graph's separate logical inventory.
    graph_bytes: u64,
    /// Actual producing CAS.
    graph: Cas,
}
/// Complete unique owner payload excluding separately charged key backing.
#[repr(C)]
struct OwnerMirror {
    /// Filled frozen result after admission.
    payload: Option<FrozenMirror>,
    /// Original identity.
    identity: IdentityMirror,
    /// Logical checked enclosure allowance.
    evidence: usize,
    /// Separately retained key capacity.
    keys: usize,
    /// Owner debit locator.
    token: Token,
    /// Prepaid cleanup.
    cleanup: CleanupReceipt,
}
/// One separately funded use of the unique owner.
#[repr(C)]
struct UseMirror {
    /// One retaining edge.
    owner: Option<Rc<OwnerMirror>>,
    /// Exact owner/use pairing.
    binding: BindingMirror,
    /// Use debit locator.
    token: Token,
    /// Prepaid use cleanup.
    cleanup: CleanupReceipt,
}
/// Exact captured pairing behind each use.
#[repr(C)]
struct BindingMirror {
    /// Original owner scope.
    identity: IdentityMirror,
    /// Unique use identity.
    use_identity: u64,
    /// Native specialization.
    seal: SealMirror,
}
/// Owner authority classes use inline locators only.
#[expect(
    dead_code,
    reason = "independent layout mirrors enumerate variants without constructing payloads"
)]
enum OwnerKindMirror {
    /// Numerical occupations.
    Bundle(Identity),
    /// Checked LC application identity.
    LcApplication(LcDebitIdentityMirror),
    /// Existing retained expm1.
    Prepared(IdentityMirror),
    /// Retained power.
    Dyadic(IdentityMirror),
    /// Source-free constant scope.
    Constant(u64),
    /// Consuming LC closure identity.
    LcJoin(LcJoinIdentityMirror),
}
/// Mirrored allocation roles include expiration witnesses and at most one inline allowance, flag or receipt.
#[expect(
    dead_code,
    reason = "independent layout mirrors enumerate variants without constructing payloads"
)]
enum KindMirror {
    /// Numerical slab.
    Numerical,
    /// Initialization tracking.
    Tracking,
    /// Detached owner.
    Owner,
    /// Pinned view.
    Pin,
    /// Filled mask.
    Filled,
    /// Existing law owner and allowance.
    PreparedOwner {
        /// Existing law expiration witness.
        witness: Weak<()>,
        /// Existing law evidence allowance.
        evidence: usize,
    },
    /// Existing law use.
    PreparedUse(Weak<()>),
    /// Power owner and distinct allowances.
    DyadicOwner {
        /// New owner's expiration witness.
        witness: Weak<()>,
        /// New owner's evidence and key allowance.
        allowance: usize,
    },
    /// Power use.
    DyadicUse(Weak<()>),
    /// Source-free constant.
    Constant(Weak<dyn std::any::Any>),
    /// Pending LC allocation with shared liveness and header role.
    LcPending {
        /// Expiration witness.
        witness: Weak<()>,
        /// Whether this debit retains the common header.
        header: bool,
    },
    /// Shared LC closure header.
    LcClosure(Weak<()>),
    /// Exact LC row backing.
    LcRows(Weak<()>),
    /// Published LC bundle with its expiration witness.
    LcBundle(Weak<()>),
    /// Published LC use with its expiration witness and optional receipt.
    LcUse {
        /// Expiration witness for the independently funded use.
        witness: Weak<()>,
        /// Optional issued-use receipt in the inline carrier.
        receipt: Option<std::num::NonZeroU64>,
    },
}
/// Debit row contains no authority allocation.
#[repr(C)]
struct DebitMirror {
    /// Allocation classification.
    kind: KindMirror,
    /// Original owner identity.
    owner: OwnerKindMirror,
    /// Unique debit identity.
    key: u64,
    /// Addressable elements.
    elements: usize,
    /// Complete charged bytes.
    bytes: usize,
}

/// Independently align the two headers and debit row before using them as byte oracles.
pub(super) fn headers() -> [usize; 3] {
    assert_eq!(size_of::<KindMirror>(), 24);
    assert_eq!(align_of::<KindMirror>(), 8);
    assert_eq!(align_of::<Option<DebitMirror>>(), 8);
    assert_eq!(
        align_of::<Option<DebitMirror>>(),
        align_of::<Option<Debit>>()
    );
    assert_eq!(size_of::<LcKeyMirror>(), 16);
    assert_eq!(size_of::<LcFormatMirror>(), 1);
    assert_eq!(size_of::<LcDebitIdentityMirror>(), 64);
    assert_eq!(
        size_of::<LcDebitIdentityMirror>(),
        size_of::<crate::loan::registry::LcDebitIdentity>()
    );
    assert_eq!(
        align_of::<LcDebitIdentityMirror>(),
        align_of::<crate::loan::registry::LcDebitIdentity>()
    );
    let sizes = [
        size_of::<Shared<OwnerMirror>>(),
        size_of::<Shared<UseMirror>>(),
        size_of::<Option<DebitMirror>>(),
    ];
    assert_eq!(
        sizes,
        [
            crate::loan::rc_bytes::<super::super::PreparedDyadicPowerOwner>(),
            crate::loan::rc_bytes::<super::super::PreparedDyadicPowerUseRecord>(),
            size_of::<Option<Debit>>()
        ]
    );
    assert_eq!(sizes[2], 264);
    assert_eq!(
        size_of::<Option<super::super::super::retained_expm1::Attachment>>(),
        8
    );
    sizes
}

/// Long divide the exact decimal spelling to count magnitude bytes without `component_bits`.
fn decimal_bytes(text: &str) -> usize {
    let mut digits: Vec<_> = text
        .trim_start_matches('-')
        .bytes()
        .map(|digit| u16::from(digit - b'0'))
        .collect();
    let mut bytes = 0;
    while digits.iter().any(|digit| *digit != 0) {
        let mut carry = 0;
        for digit in &mut digits {
            let value = carry * 10 + *digit;
            *digit = value / 256;
            carry = value % 256;
        }
        bytes += 1;
    }
    bytes
}

/// Count both endpoint component magnitudes and one interval's fixed metadata.
fn interval_bytes(interval: &Interval, budget: &mut Budget) -> TestResult<usize> {
    let mut bytes = 128;
    for endpoint in [interval.lower(), interval.upper()] {
        let text = endpoint.exact_string(budget)?;
        let (numerator, denominator) = text.split_once('/').unwrap_or((&text, "1"));
        bytes += 4 * (decimal_bytes(numerator) + decimal_bytes(denominator));
    }
    Ok(bytes)
}

/// Enumerate the known fixture's original request and checked transcript without the CAS accessor.
pub(super) fn evidence(authority: &DyadicPowerPreparation) -> TestResult<usize> {
    let checked = authority.judgment();
    let proof = checked.certificate();
    let request = checked.request();
    let mut budget = Budget::new(fixture::limits())?;
    assert_eq!(request.coordinates.len(), 1);
    assert_eq!(proof.powers.len(), 1);
    assert_eq!(proof.powers[0].roots.len(), 1);
    assert!(
        proof.derivatives.is_empty()
            && proof.exponentials.is_empty()
            && proof.logarithms.is_empty()
    );
    let mut bytes = 512 + proof.scope.len() + 256 + 128;
    bytes += interval_bytes(&request.coordinates[0].interval, &mut budget)?;
    bytes += interval_bytes(checked.interval(), &mut budget)?;
    for record in &proof.records {
        bytes += interval_bytes(record, &mut budget)?;
    }
    bytes += 64;
    for root in &proof.powers[0].roots {
        bytes += interval_bytes(root, &mut budget)?;
    }
    Ok(bytes)
}
