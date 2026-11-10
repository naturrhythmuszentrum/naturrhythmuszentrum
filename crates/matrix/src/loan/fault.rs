// SPDX-License-Identifier: AGPL-3.0-only
//! Thread-local allocation refusals exercise every acquired preparation prefix in parallel tests.

use super::LoanError;
use std::cell::Cell;
thread_local! {
    /// Diagnostic unwind at a refused allocation, confined to this test thread.
    static UNWIND: Cell<bool> = const { Cell::new(false) };
    /// Actual filled-membership visits during publication, independent of charged Work.
    static PUBLICATION_CELLS: Cell<usize> = const { Cell::new(0) };
    /// A test watches one exact application's source edge at final witness destruction.
    static WITNESS_SOURCE: std::cell::RefCell<Option<(u64, std::rc::Weak<super::registry::PinRecord>)>> = const { std::cell::RefCell::new(None) };
    /// Observed source strong count at that precise destruction boundary.
    static WITNESS_SOURCE_COUNT: Cell<Option<usize>> = const { Cell::new(None) };
    /// Scalar helper entries observed at the shared dense/selected seam.
    static SCALARS: Cell<usize> = const { Cell::new(0) };
    /// Only the current diagnostic test's allocation schedule is affected.
    static REMAINING: Cell<Option<usize>> = const { Cell::new(None) };
    /// Number of acquisition boundaries reached by this diagnostic attempt.
    static ATTEMPTS: Cell<usize> = const { Cell::new(0) };
    /// Acquired, retained and destroyed prefix observed when the private guard closes.
    static PREFIX: Cell<[usize; 3]> = const { Cell::new([0; 3]) };
    /// Native scalar loads observed independently of stored initialization flags.
    static READS: Cell<usize> = const { Cell::new(0) };
    /// Actual ordinary product backend entries, independent of the resource estimator.
    static PRODUCT_BACKENDS: Cell<usize> = const { Cell::new(0) };
    /// Actual raw export allocation entries, independent of resource planning.
    static RAW_COPIES: Cell<usize> = const { Cell::new(0) };
    /// Ordinary alias-retirement key allocation sites entered in this thread.
    static RETIREMENT_KEYS: Cell<usize> = const { Cell::new(0) };
    /// Actual reading-write physical backing count and allocated bytes.
    static WRITE_BUFFERS: Cell<[usize; 2]> = const { Cell::new([0; 2]) };
    /// Actual statistical numerical owner-header backing count and bytes.
    static STATISTICAL_HEADERS: Cell<[usize; 2]> = const { Cell::new([0; 2]) };
    /// Actual ordinary `Record` backing count and reserved entry capacity.
    static RECORD_BACKINGS: Cell<[usize; 2]> = const { Cell::new([0; 2]) };
    /// Actual lowered-coefficient and decoded-reciprocal allocation entries.
    static KERNEL_ALLOCATIONS: Cell<[usize; 2]> = const { Cell::new([0; 2]) };
}
/// Scoped fault policy is removed even when a diagnostic unwinds.
pub(crate) struct Injection;
impl Injection {
    /// Refuse the allocation after this many successful acquisition boundaries.
    pub(crate) fn after(allocations: usize) -> Self {
        REMAINING.set(Some(allocations));
        ATTEMPTS.set(0);
        PRODUCT_BACKENDS.set(0);
        KERNEL_ALLOCATIONS.set([0; 2]);
        RAW_COPIES.set(0);
        RETIREMENT_KEYS.set(0);
        RECORD_BACKINGS.set([0; 2]);
        WRITE_BUFFERS.set([0; 2]);
        STATISTICAL_HEADERS.set([0; 2]);
        PREFIX.set([0; 3]);
        Self
    }
    /// Unwind at an acquisition boundary to exercise lexical prefix destruction.
    pub(crate) fn unwinding_after(allocations: usize) -> Self {
        let injection = Self::after(allocations);
        UNWIND.set(true);
        injection
    }
    /// Last preparation's precise acquired-prefix cleanup, independent of allocator attempts.
    pub(super) fn prefix() -> [usize; 3] {
        PREFIX.get()
    }
    /// Actual product backend calls in this thread's diagnostic attempt.
    pub(crate) fn product_backends() -> usize {
        PRODUCT_BACKENDS.get()
    }
    /// Independent observations at the linear and reciprocal numerical allocation sites.
    pub(crate) fn kernel_allocations() -> [usize; 2] {
        KERNEL_ALLOCATIONS.get()
    }
    /// Raw exports entered in this thread's diagnostic attempt.
    pub(crate) fn raw_copies() -> usize {
        RAW_COPIES.get()
    }
    /// Ordinary retirement snapshot vectors, independent of the failure injector.
    pub(crate) fn retirement_keys() -> usize {
        RETIREMENT_KEYS.get()
    }
    /// Ordinary physical record buffers observed at their allocating helper.
    pub(crate) fn record_backings() -> [usize; 2] {
        RECORD_BACKINGS.get()
    }
    /// Physical replacement and location buffers observed independently of planning.
    pub(crate) fn write_buffers() -> [usize; 2] {
        WRITE_BUFFERS.get()
    }
    /// Numerical header capacities observed at their ordinary allocation helper.
    pub(crate) fn statistical_headers() -> [usize; 2] {
        STATISTICAL_HEADERS.get()
    }
    /// Independently observed acquisition boundaries, including the refused one.
    pub(crate) fn attempts() -> usize {
        ATTEMPTS.get()
    }
}
impl Drop for Injection {
    fn drop(&mut self) {
        REMAINING.set(None);
        UNWIND.set(false);
    }
}
/// Call immediately before an actual allocation, after cleanup prepayment.
pub(crate) fn before() -> Result<(), LoanError> {
    let Some(remaining) = REMAINING.get() else {
        return Ok(());
    };
    ATTEMPTS.set(ATTEMPTS.get() + 1);
    if remaining == 0 {
        assert!(!UNWIND.get(), "injected allocation unwind");
        return Err(LoanError::AllocationRefused);
    }
    REMAINING.set(Some(remaining - 1));
    Ok(())
}

/// Record fixed cleanup observations without installing an observer on numerical payloads.
pub(super) fn closed_prefix(acquired: usize, retained: usize, destroyed: usize) {
    PREFIX.set([acquired, retained, destroyed]);
}

/// Instrument the actual numerical accessor, rather than inferring hole safety from zero fill.
pub(super) fn loaded() {
    READS.set(READS.get() + 1);
}
/// Reset the independent selected-load observer for one diagnostic phase.
pub(super) fn reset_reads() {
    READS.set(0);
}
/// Actual accessor loads in this thread's current diagnostic phase.
pub(super) fn reads() -> usize {
    READS.get()
}

/// Observe the allocating backend route independently of the common physical plan.
pub(crate) fn entered_product_backend() {
    PRODUCT_BACKENDS.set(PRODUCT_BACKENDS.get() + 1);
}

/// Observe actual coefficient owners without coupling the counter to the resource estimator.
pub(crate) fn entered_kernel_allocation(reciprocal: bool) {
    let mut counts = KERNEL_ALLOCATIONS.get();
    counts[usize::from(reciprocal)] += 1;
    KERNEL_ALLOCATIONS.set(counts);
}

/// Observe raw-copy allocation at the exporter, independently of per-call admission.
pub(crate) fn entered_raw_copy() {
    RAW_COPIES.set(RAW_COPIES.get() + 1);
}

/// Observe the actual ordinary retirement-key reservation after complete byte admission.
pub(crate) fn entered_retirement_keys() {
    RETIREMENT_KEYS.set(RETIREMENT_KEYS.get() + 1);
}

/// Observe the actual backing capacity, independently of the admission arithmetic.
pub(crate) fn entered_record_backing(capacity: usize) {
    let [allocations, entries] = RECORD_BACKINGS.get();
    RECORD_BACKINGS.set([allocations + usize::from(capacity != 0), entries + capacity]);
}

/// Observe ordinary reading-write buffer capacities at their actual reservation sites.
pub(crate) fn entered_write_buffer(bytes: usize) {
    let [allocations, total] = WRITE_BUFFERS.get();
    WRITE_BUFFERS.set([allocations + usize::from(bytes != 0), total + bytes]);
}

/// Observe the four physical statistical header buffers after each actual reservation.
pub(crate) fn entered_statistical_headers(bytes: usize) {
    let [allocations, total] = STATISTICAL_HEADERS.get();
    STATISTICAL_HEADERS.set([allocations + usize::from(bytes != 0), total + bytes]);
}

/// Count every scalar helper invocation independently of the traversal plan.
pub(crate) fn entered_scalar() {
    SCALARS.set(SCALARS.get() + 1);
}
/// Reset independent scalar invocation observations.
pub(super) fn reset_scalars() {
    SCALARS.set(0);
}
/// Actual scalar helper invocations.
pub(super) fn scalars() -> usize {
    SCALARS.get()
}

/// Watch the exact receipt's input edge without retaining numerical ownership.
pub(super) fn watch_source_release(
    receipt: u64,
    source: std::rc::Weak<super::registry::PinRecord>,
) {
    WITNESS_SOURCE.with_borrow_mut(|watch| *watch = Some((receipt, source)));
    WITNESS_SOURCE_COUNT.set(None);
}
/// Observe input release before the final liveness witness expires.
pub(super) fn witness_expiring(receipt: u64) {
    super::retained_expm1::tests::witness_expiring(receipt);
    WITNESS_SOURCE.with_borrow_mut(|watch| {
        if let Some((expected, source)) = watch
            && *expected == receipt
        {
            WITNESS_SOURCE_COUNT.set(Some(source.strong_count()));
            *watch = None;
        }
    });
}
/// Last exact source-edge count captured at witness destruction.
pub(super) fn source_at_witness() -> Option<usize> {
    WITNESS_SOURCE_COUNT.get()
}

/// Count the actual shared publication membership operation.
pub(super) fn visited_publication_cell() {
    PUBLICATION_CELLS.set(PUBLICATION_CELLS.get() + 1);
}
/// Reset actual publication membership observations for one attempt.
pub(super) fn reset_publication_cells() {
    PUBLICATION_CELLS.set(0);
}
/// Actual membership visits in this thread's publication attempt.
pub(super) fn publication_cells() -> usize {
    PUBLICATION_CELLS.get()
}
