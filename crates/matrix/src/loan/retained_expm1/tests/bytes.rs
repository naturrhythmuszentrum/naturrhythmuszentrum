// SPDX-License-Identifier: AGPL-3.0-only
//! Independent aligned ABI fields and decimal spellings supply the byte oracles.

use super::{TestResult, fixture};
use crate::loan::registry::{CleanupReceipt, Debit, Identity, Token};
use nrz_bindings::readings::algebra::{Cas, enclosure::prepared_expm1::Prepared};
use nrz_scalar::Expm1Program;
use std::rc::{Rc, Weak};

/// Shared allocation ABI includes both reference counters and payload alignment.
#[repr(C)]
struct Shared<T> {
    strong: usize,
    weak: usize,
    payload: T,
}
/// Independent owner identity fields.
#[repr(C)]
struct IdentityMirror {
    scope: u64,
    account: u64,
    owner: u64,
}
/// Immutable native pairing fields.
#[repr(C)]
struct SealMirror {
    domain: [u64; 2],
    revisions: [u32; 4],
}
/// The original authority box pointer appears exactly once inside the owner.
#[repr(C)]
struct PartsMirror {
    authority: Box<Prepared>,
    program: Expm1Program,
    evidence: u64,
}
/// Evidence/program destruction precedes the actual producing graph.
#[repr(C)]
struct FrozenMirror {
    parts: PartsMirror,
    seal: SealMirror,
    graph_bytes: u64,
    graph: Cas,
}
/// Complete unique owner fields, independent of the resource planner.
#[repr(C)]
struct OwnerMirror {
    payload: Option<FrozenMirror>,
    identity: IdentityMirror,
    evidence: usize,
    token: Token,
    cleanup: CleanupReceipt,
}
/// Every individual use has its own header, owner pointer and prepaid release.
#[repr(C)]
struct UseMirror {
    owner: Option<Rc<()>>,
    binding: BindingMirror,
    token: Token,
    cleanup: CleanupReceipt,
}
/// The changed debit authority discriminant has closed numerical and law variants.
#[expect(
    dead_code,
    reason = "ABI mirror variants describe layout without constructing authority"
)]
enum DebitOwnerMirror {
    Bundle(Identity),
    Prepared(IdentityMirror),
}
/// Weak expiration witnesses are inline row fields, not additional allocations.
#[expect(
    dead_code,
    reason = "ABI mirror variants describe layout without constructing authority"
)]
enum KindMirror {
    Numerical,
    Tracking,
    Owner,
    Pin,
    Filled,
    PreparedOwner { witness: Weak<()>, evidence: usize },
    PreparedUse(Weak<()>),
}
/// Captured binding contains exact owner/use and represented-domain revision metadata.
#[repr(C)]
struct BindingMirror {
    identity: IdentityMirror,
    use_identity: u64,
    seal: SealMirror,
}
/// One pointer moves; its captured binding stays in the charged use allocation.
#[repr(C)]
pub(in crate::loan) struct AttachmentMirror {
    law: Rc<()>,
}
/// Complete changed debit row fields.
#[repr(C)]
struct DebitMirror {
    kind: KindMirror,
    owner: DebitOwnerMirror,
    key: u64,
    elements: usize,
    bytes: usize,
}

/// H, U and the changed table row are reconstructed without the production planner.
pub(super) fn headers() -> [usize; 3] {
    let values = [
        size_of::<Shared<OwnerMirror>>(),
        size_of::<Shared<UseMirror>>(),
        size_of::<Option<DebitMirror>>(),
    ];
    assert_eq!(
        values,
        [
            crate::loan::rc_bytes::<super::super::PreparedExpm1Owner>(),
            crate::loan::rc_bytes::<super::super::PreparedExpm1UseRecord>(),
            size_of::<Option<Debit>>()
        ]
    );
    assert_eq!(values, [2216, 144, 264]);
    assert_eq!(size_of::<DebitMirror>(), size_of::<Debit>());
    assert_eq!(
        size_of::<Option<AttachmentMirror>>(),
        size_of::<Option<super::super::Attachment>>()
    );
    assert_eq!(size_of::<Option<AttachmentMirror>>(), 8);
    values
}
/// Exact spelling long division, independent of native component-bit inventories.
fn decimal_bytes(text: &str) -> usize {
    let mut digits: Vec<_> = text
        .trim_start_matches('-')
        .bytes()
        .map(|c| u16::from(c - b'0'))
        .collect();
    let mut count = 0;
    while digits.iter().any(|digit| *digit != 0) {
        let mut remainder = 0;
        for digit in &mut digits {
            let value = remainder * 10 + *digit;
            *digit = value / 256;
            remainder = value % 256;
        }
        count += 1;
    }
    count
}
/// E is logical exact evidence, reconstructed from the complete fixture's exact spellings.
pub(super) fn evidence(authority: &Prepared) -> TestResult<usize> {
    let proof = authority.proof();
    let mut budget = nrz_bindings::Budget::new(fixture::limits())?;
    let mut bytes = proof.scope.len()
        + authority.statement_key().len()
        + 1024
        + 32 * proof.coefficients.len()
        + 256 * authority.request().original.coordinates.len();
    for number in [
        &proof.absolute_tail,
        &proof.relative_tail,
        &proof.import_ratio,
        &proof.central_ratio,
        &proof.endpoint_ratio,
        &proof.restoration_error,
    ]
    .into_iter()
    .chain(
        proof
            .stages
            .iter()
            .flat_map(|stage| [&stage.magnitude, &stage.error]),
    )
    .chain(
        authority
            .request()
            .original
            .coordinates
            .iter()
            .flat_map(|coordinate| [coordinate.interval.lower(), coordinate.interval.upper()]),
    ) {
        let text = number.exact_string(&mut budget)?;
        let (n, d) = text.split_once('/').unwrap_or((&text, "1"));
        bytes += 128 + 4 * (decimal_bytes(n) + decimal_bytes(d));
    }
    Ok(bytes)
}
/// Exact fresh growth and reuse count the same owner once and every use separately.
#[test]
fn retained_law_independent_byte_inventory() -> TestResult {
    let mut heap = crate::Heap::new(crate::HeapLimits::default())?;
    let mut account = fixture::account()?;
    let initial = heap.resource_bytes()?;
    let law = fixture::prepare(&mut heap, &mut account)?;
    let [h, u, row] = headers();
    let e = evidence(law.authority()?)?;
    assert_eq!(heap.resource_bytes()? - initial, h + e + u + 2 * row);
    let observed = heap.retained_expm1_inventory(&law, &mut account)?;
    assert_eq!(
        [
            observed.owner_bytes,
            observed.evidence_bytes,
            observed.use_bytes
        ],
        [h, e, u]
    );
    let second = heap.retain_retained_expm1(&law, &mut account)?;
    assert_eq!(heap.resource_bytes()? - initial, h + e + 2 * u + 3 * row);
    assert_ne!(
        heap.retained_expm1_inventory(&second, &mut account)?
            .use_identity,
        observed.use_identity
    );
    drop(second);
    assert!(matches!(
        heap.reconcile_detached(&mut account, 1000, None),
        crate::ScanOutcome::Complete
    ));
    let before = heap.resource_bytes()?;
    let second = heap.retain_retained_expm1(&law, &mut account)?;
    assert_eq!(heap.resource_bytes()? - before, u);
    drop((law, second));
    assert!(matches!(
        heap.residence(&mut account, 1000)?,
        crate::Residence::Empty { .. }
    ));
    assert_eq!(heap.resource_bytes()? - initial, 3 * row);
    Ok(())
}
