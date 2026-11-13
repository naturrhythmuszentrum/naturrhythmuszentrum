// SPDX-License-Identifier: AGPL-3.0-only
//! Literal owner inventory and cleanup prepayment precede all admission allocation.

use super::{
    BundleOwner, Debit, DescriptorRecord, HostCore, HostTileDeclaration, LayoutRecord, Liveness,
    LoanError, MaskRecord, Native, PinRecord,
};
use crate::{
    Heap,
    heap::{
        identity::Key,
        resources::{ResourcePlan, ResourcePolicy, add, slab_bytes},
    },
    loan::{charge, rc_bytes, registry::CleanupReceipt, words},
    storage::Format,
};
use nrz_scalar::Scalar;

/// Fixed admission geometry and exact index-growth capacities.
struct Geometry {
    /// Present native channels in declaration order.
    present: [bool; 3],
    /// Present channel count.
    channels: usize,
    /// Addressable scalar length per channel.
    length: usize,
    /// Component flag words per channel.
    component_words: usize,
    /// Complete-cell words per channel.
    cell_words: usize,
    /// Checked backend payload capacity per channel.
    rounded: usize,
}
/// Prechecked finite preparation inventory; all counts are fixed before construction.
pub(super) struct Plan {
    /// Present native channels.
    pub(super) present: [bool; 3],
    /// Present channel count.
    pub(super) channels: usize,
    /// Scalar entries per channel.
    pub(super) length: usize,
    /// Component flag words per channel.
    pub(super) component_words: usize,
    /// Complete-cell flag words per channel.
    pub(super) cell_words: usize,
    /// Backend rounded bytes per channel.
    pub(super) rounded: usize,
    /// Full detached core and private shared-owner headers.
    pub(super) headers: [usize; 5],
    /// Number of individual allocation debit records.
    pub(super) debit_count: usize,
    /// Fixed indexed debit destinations.
    pub(super) debit_slots: [usize; 15],
    /// Parallel group capacity required by descriptor occupations.
    pub(super) group_capacity: usize,
    /// Debit index capacity after vacancy reuse.
    pub(super) debit_capacity: usize,
    /// Exact weak-pin row reserved for the initial pin.
    pub(super) pin_slot: usize,
    /// Weak-pin index capacity reserved for the initial pin.
    pub(super) pin_capacity: usize,
    /// Nonwrapping mutation revision.
    pub(super) revision: u64,
    /// Prepaid full-prefix cleanup inventory.
    pub(super) cleanup: CleanupReceipt,
}
impl Plan {
    /// Authenticate inventory and prepay every acquisition/failure prefix on the original ledger.
    pub(super) fn new<P: Scalar + 'static>(
        heap: &Heap,
        declaration: &HostTileDeclaration<P>,
        layout: &crate::Layout,
        budget: &mut nrz_bindings::Budget,
    ) -> Result<Self, LoanError> {
        charge(budget, 1)?;
        if layout.extent() != declaration.extent || layout.dense_range()?.is_none() {
            return Err(LoanError::LayoutMismatch);
        }
        // Planning inspects descriptor vacancies three times and native vacancies twice;
        // the final occupation scans are prepaid with preparation below.
        charge(
            budget,
            heap.descriptors
                .slots
                .scan_work()
                .checked_mul(3)
                .and_then(|work| {
                    work.checked_add(
                        (heap.masks.layouts.scan_work() + heap.masks.masks.scan_work()) * 2,
                    )
                })
                .ok_or(LoanError::CountOverflow)?,
        )?;
        let present = declaration.present();
        let channels = present.into_iter().filter(|present| *present).count();
        let length = layout.region_entries();
        let geometry = Geometry {
            present,
            channels,
            length,
            component_words: words(declaration.extent.entries()),
            cell_words: words(declaration.capacity()),
            rounded: slab_bytes::<P>(length)?,
        };
        let debit_count = add(channels.checked_mul(3).ok_or(LoanError::CountOverflow)?, 5)?;
        charge(budget, heap.loans.debits.len())?;
        let debit_slots = heap.loans.vacant_debits(debit_count)?;
        let group_capacity = heap
            .descriptors
            .slots
            .planned_len(channels)?
            .max(heap.loans.groups.capacity());
        let debit_capacity =
            add(debit_slots[debit_count - 1], 1)?.max(heap.loans.debits.capacity());
        charge(budget, heap.loans.pins.len())?;
        let pin_slot = heap.loans.vacant_pin();
        let pin_capacity = add(pin_slot, 1)?.max(heap.loans.pins.capacity());
        let headers = [
            size_of::<HostCore>(),
            rc_bytes::<LayoutRecord>(),
            rc_bytes::<Liveness>(),
            rc_bytes::<BundleOwner>(),
            rc_bytes::<PinRecord>(),
        ];
        let cleanup = CleanupReceipt {
            identity: crate::heap::identity::identity()?,
            allocations: channels
                * (usize::from(length != 0)
                    + usize::from(geometry.component_words != 0)
                    + usize::from(geometry.cell_words != 0))
                + 12
                + usize::from(heap.descriptors.slots.growth_bytes_reserved(channels)?.0 != 0)
                + usize::from(heap.masks.layouts.growth_bytes_reserved(1)?.0 != 0)
                + usize::from(heap.masks.masks.growth_bytes_reserved(1)?.0 != 0)
                + usize::from(group_capacity > heap.loans.groups.capacity())
                + usize::from(debit_capacity > heap.loans.debits.capacity())
                + usize::from(pin_capacity > heap.loans.pins.capacity()),
            records: channels + debit_count + 4,
            edges: channels + 3,
        };
        let plan = Self {
            present: geometry.present,
            channels: geometry.channels,
            length: geometry.length,
            component_words: geometry.component_words,
            cell_words: geometry.cell_words,
            rounded: geometry.rounded,
            headers,
            debit_count,
            debit_slots,
            group_capacity,
            debit_capacity,
            pin_slot,
            pin_capacity,
            revision: heap.loans.admission_revision()?,
            cleanup,
        };
        charge(budget, heap.resource_work()?)?;
        plan.preflight::<P>(heap, budget)?;
        let cleanup_work = add(
            add(plan.cleanup.allocations, plan.cleanup.records)?,
            add(plan.cleanup.edges, 11 + 4 * channels)?,
        )?;
        let initialization = channels
            .checked_mul(add(length, add(plan.component_words, plan.cell_words)?)?)
            .ok_or(LoanError::CountOverflow)?;
        let slots = add(
            heap.descriptors.slots.scan_work(),
            add(heap.masks.layouts.scan_work(), heap.masks.masks.scan_work())?,
        )?;
        let preparation = add(plan.cleanup.records, add(plan.cleanup.edges + 1, slots)?)?;
        charge(
            budget,
            add(cleanup_work, add(initialization, preparation)?)?,
        )?;
        Ok(plan)
    }
    /// Retained deltas and replacement buffers are disjoint entries in the common plan.
    fn preflight<P: Scalar + 'static>(
        &self,
        heap: &Heap,
        budget: &nrz_bindings::Budget,
    ) -> Result<(), LoanError> {
        let channel_bytes = add(
            self.rounded,
            add(self.component_words, self.cell_words)?
                .checked_mul(8)
                .ok_or(LoanError::ByteOverflow)?,
        )?;
        let mut retained = add(
            self.channels
                .checked_mul(channel_bytes)
                .ok_or(LoanError::ByteOverflow)?,
            self.headers.into_iter().try_fold(0_usize, add)?,
        )?;
        let mut scratch = 0;
        for (growth, old) in [
            heap.descriptors
                .slots
                .growth_bytes_reserved(self.channels)?,
            heap.masks.layouts.growth_bytes_reserved(1)?,
            heap.masks.masks.growth_bytes_reserved(1)?,
            super::growth(&heap.loans.groups, self.group_capacity)?,
            super::growth(&heap.loans.debits, self.debit_capacity)?,
            super::growth(&heap.loans.pins, self.pin_capacity)?,
        ] {
            retained = add(retained, growth)?;
            scratch = add(scratch, old.checked_mul(2).ok_or(LoanError::ByteOverflow)?)?;
        }
        // Prepared keys and values coexist until their no-fail installation consumes the vectors.
        scratch = add(
            scratch,
            self.debit_count
                .checked_mul(size_of::<Debit>())
                .ok_or(LoanError::ByteOverflow)?,
        )?;
        scratch = add(
            scratch,
            (self.channels + 2)
                .checked_mul(size_of::<Key>())
                .ok_or(LoanError::ByteOverflow)?,
        )?;
        scratch = add(
            scratch,
            self.channels
                .checked_mul(size_of::<DescriptorRecord>())
                .ok_or(LoanError::ByteOverflow)?,
        )?;
        scratch = add(
            scratch,
            add(
                size_of::<Native<LayoutRecord>>(),
                size_of::<Native<MaskRecord>>(),
            )?,
        )?;
        let mut resources = ResourcePlan {
            retained,
            scratch,
            ..ResourcePlan::default()
        };
        resources.elements[Format::of::<P>().index()] = self
            .length
            .checked_mul(self.channels)
            .ok_or(LoanError::CountOverflow)?;
        heap.preflight_resources(resources, ResourcePolicy::from_budget(budget))?;
        Ok(())
    }
}
