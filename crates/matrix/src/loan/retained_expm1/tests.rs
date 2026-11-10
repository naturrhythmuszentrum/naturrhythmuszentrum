// SPDX-License-Identifier: AGPL-3.0-only
//! Independent ownership, byte, phase and numerical controls for retained law authority.

mod attachments;
pub(in crate::loan) mod bytes;
mod constructor_work;
mod fixture;
pub(in crate::loan) mod memos;
mod public_boundary;
mod refusal;
mod resources;
mod unwind;
mod work;
use super::*;
use crate::{Heap, HeapLimits, Residence, ScanOutcome};
use fixture::account;
use std::cell::Cell;

/// Test failures preserve the original exact and managed refusal detail.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
thread_local! {
    /// This counter runs after the actual graph/evidence/program fields are destroyed.
    static DESTROYED: Cell<usize> = const { Cell::new(0) };
    /// Exact detached witness whose last-law destruction order is observed.
    static WATCH_WITNESS: Cell<Option<(u64, usize)>> = const { Cell::new(None) };
    /// Actual post-payload destruction count at that witness's destructor.
    static AT_WITNESS: Cell<Option<usize>> = const { Cell::new(None) };
}
/// Last-field destruction sentinel adds no allocation or production ownership edge.
#[derive(Debug)]
pub(super) struct Destroyed;
impl Drop for Destroyed {
    fn drop(&mut self) {
        DESTROYED.set(DESTROYED.get() + 1);
    }
}
/// Inject only this thread's prepared-owner identity acquisitions.
pub(super) fn before_identity() -> Result<(), LoanError> {
    unwind::before_identity()
}
/// Observe actual graph/evidence destruction before the detached witness expires.
pub(in crate::loan) fn witness_expiring(receipt: u64) {
    if let Some((expected, before)) = WATCH_WITNESS.get()
        && expected == receipt
    {
        AT_WITNESS.set(Some(DESTROYED.get() - before));
        WATCH_WITNESS.set(None);
    }
}
/// Observe actual completed graph/evidence/program destruction in private application fixtures.
pub(in crate::loan) fn destroyed() -> usize {
    DESTROYED.get()
}
/// Share exact fixtures with private application retention controls.
pub(in crate::loan) fn prepare(
    heap: &mut Heap,
    account: &mut crate::ExecutionAccount,
) -> TestResult<RetainedExpm1> {
    fixture::prepare(heap, account)
}
/// Original free-parameter request for checked application correspondence controls.
pub(in crate::loan) fn request() -> TestResult<(
    nrz_bindings::readings::algebra::Cas,
    nrz_bindings::readings::algebra::enclosure::prepared_expm1::Request,
)> {
    fixture::request()
}
/// Keep immutable original limits equal across the ownership fixture.
pub(in crate::loan) fn limits() -> nrz_bindings::Limits {
    fixture::limits()
}
/// Move a funded use through the same private attachment seam.
pub(in crate::loan) fn attach(
    law: RetainedExpm1,
    heap: &Heap,
    account: &mut crate::ExecutionAccount,
) -> TestResult<Attachment> {
    let binding = Binding::of(&law)?;
    Attachment::prepare(law, binding, heap, &mut account.ledger).map_err(|(error, _)| error.into())
}
/// Observe the exact use identity and borrowed scalar route without retaining another edge.
pub(in crate::loan) fn check_attachment(attachment: &Attachment, identity: u64) -> TestResult {
    attachment.validate()?;
    assert_eq!(attachment.law.record.binding.use_identity, identity);
    attachment
        .law
        .evaluate(fixture::point(-0.5), &ArithmeticContext::new()?)?;
    Ok(())
}
/// Private environment substitution supplies no new owner or numerical program.
pub(in crate::loan) fn substituted_environment(mut binding: Binding) -> Binding {
    binding.seal.revisions[3] = 2;
    binding
}
/// Independent uses survive heap destruction and release the exact authority once.
#[test]
fn retained_law_last_use_and_heap_lifetime() -> TestResult {
    let before = DESTROYED.get();
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = account()?;
    let law = prepare(&mut heap, &mut account)?;
    let other = heap.retain_retained_expm1(&law, &mut account)?;
    assert!(std::ptr::eq(law.authority()?, other.authority()?));
    let inventory = heap.retained_expm1_inventory(&law, &mut account)?;
    assert_eq!(inventory.uses, 2);
    drop(law);
    assert_eq!(DESTROYED.get(), before);
    assert!(matches!(
        heap.residence(&mut account, 1000)?,
        Residence::Pending {
            elements: Some([0, 0]),
            ..
        }
    ));
    drop(heap);
    let context = ArithmeticContext::new()?;
    assert_eq!(
        other
            .evaluate(fixture::point(0.0), &context)?
            .reading()
            .center
            .to_bits(),
        0
    );
    assert_eq!(DESTROYED.get(), before);
    drop(other);
    assert_eq!(DESTROYED.get(), before + 1);
    Ok(())
}
/// Owner and use debits remain conservative until separate charged visits remove them.
#[test]
fn retained_law_interrupted_reconciliation() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = account()?;
    let law = prepare(&mut heap, &mut account)?;
    let before = heap.resource_bytes()?;
    drop(law);
    assert_eq!(heap.resource_bytes()?, before);
    assert!(matches!(
        heap.residence(&mut account, 0)?,
        Residence::Pending { .. }
    ));
    let ScanOutcome::Pending(cursor) = heap.reconcile_detached(&mut account, 1, None) else {
        return Err("owner visit must leave use visit pending".into());
    };
    assert!(heap.resource_bytes()? < before);
    assert!(matches!(
        heap.reconcile_detached(&mut account, 1, Some(cursor)),
        ScanOutcome::Complete
    ));
    assert!(matches!(
        heap.residence(&mut account, 1000)?,
        Residence::Empty { .. }
    ));
    let vacant = heap.resource_bytes()?;
    assert!(matches!(
        heap.reconcile_detached(&mut account, 1000, None),
        ScanOutcome::Complete
    ));
    assert_eq!(heap.resource_bytes()?, vacant);
    Ok(())
}
