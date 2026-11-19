// SPDX-License-Identifier: AGPL-3.0-only
//! Complete native reservation and explicit allocation events precede every acquired prefix.
use super::super::{LcLiveness, LcOutcome};
use super::{Bundle, Storage, Use};
use crate::heap::{
    descriptors::DescriptorRecord,
    identity::Key,
    resources::{ResourcePermit, ResourcePlan, ResourcePolicy, add, slab_bytes},
};
use crate::loan::{
    charge,
    lifecycle::admission::growth,
    rc_bytes,
    registry::{CleanupReceipt, Debit},
    words,
};
use crate::mask::storage::{LayoutRecord, MaskRecord, Native};
use crate::{Heap, LcOmegaPrepared, LoanError};
use nrz_bindings::Budget;
use nrz_scalar::Scalar;

/// Fixed capacities and prechecked reservation identities, without any owned payload.
#[repr(C)]
pub(super) struct Plan {
    /// Descriptor keys selected during the planning walk.
    pub(super) descriptor_keys: Option<crate::heap::slots::FixedSlotPlan<3>>,
    /// Output-layout key selected during planning.
    pub(super) layout_keys: Option<crate::heap::slots::FixedSlotPlan<1>>,
    /// Completed-mask key selected during planning.
    pub(super) mask_keys: Option<crate::heap::slots::FixedSlotPlan<1>>,
    /// Complete original tuple capacity, independent of demand.
    pub(super) cells: usize,
    /// Packed initialization words per channel.
    pub(super) words: usize,
    /// Rounded bytes in one numerical backing.
    pub(super) slab: usize,
    /// Seventeen preselected new debit positions.
    pub(super) slots: [usize; 17],
    /// Distinct nonwrapping allocation identities.
    pub(super) keys: [u64; 17],
    /// Fresh producer request identity.
    pub(super) request: u64,
    /// Fresh native occupation nonce.
    pub(super) nonce: u64,
    /// Required retained group-table capacity.
    pub(super) groups: usize,
    /// Required retained debit-table capacity.
    pub(super) debits: usize,
    /// Prechecked admission revision.
    pub(super) revision: u64,
    /// Complete acquired-prefix destruction entitlement.
    pub(super) cleanup: CleanupReceipt,
    /// Authorization for the complete native allocation envelope.
    pub(super) permit: ResourcePermit,
}
impl Plan {
    /// Authenticate boundaries and prepay the complete new native owner before allocation.
    pub(super) fn new<P: Scalar + 'static, const N: usize>(
        prepared: &LcOmegaPrepared<'_, P, N>,
        heap: &Heap,
        account: u64,
        demand: crate::MaskVersion,
        budget: &mut Budget,
    ) -> Result<Self, LoanError> {
        super::check::account(prepared, account)?;
        charge(budget, 9)?;
        super::check::boundary(prepared, heap, demand)?;
        let cells = prepared
            .receipt
            .closure
            .rows
            .first()
            .ok_or(LoanError::UnsupportedContract)?
            .axes
            .combined
            .axis()
            .capacity();
        let word_count = words(cells);
        let slab = slab_bytes::<P>(cells)?;
        charge(
            budget,
            heap.descriptors.slots.scan_work()
                + heap.masks.layouts.scan_work()
                + heap.masks.masks.scan_work()
                + heap.loans.debits.len(),
        )?;
        let slots = heap.loans.vacant_inventory::<17>(17)?;
        charge(budget, 3)?; // Three table-capacity boundary checks.
        let descriptor_keys = heap
            .descriptors
            .slots
            .plan_fixed::<3>(heap.limits.descriptors)?;
        let layout_keys = heap
            .masks
            .layouts
            .plan_fixed::<1>(heap.limits.descriptors)?;
        let mask_keys = heap.masks.masks.plan_fixed::<1>(heap.limits.descriptors)?;
        charge(budget, 3)?; // Three selected descriptor keys determine group length.
        let groups = descriptor_keys
            .resulting_len()
            .max(heap.loans.groups.capacity());
        let debits = add(slots[16], 1)?.max(heap.loans.debits.capacity());
        let changes = [
            descriptor_keys.growth(&heap.descriptors.slots)?,
            layout_keys.growth(&heap.masks.layouts)?,
            mask_keys.growth(&heap.masks.masks)?,
            growth(&heap.loans.groups, groups)?,
            growth(&heap.loans.debits, debits)?,
        ];
        let replacements = changes.iter().filter(|(delta, _)| *delta != 0).count();
        charge(budget, heap.resource_work()?)?;
        let resource = resources::<P, N>(cells, slab, &changes)?;
        let mut permit = heap.preflight_resources(resource, ResourcePolicy::from_budget(budget))?;
        permit.account = Some(account);
        let revision = heap.loans.admission_revision()?;
        charge(budget, 20)?;
        let request = crate::heap::identity::identity()?;
        let nonce = crate::heap::identity::identity()?;
        let receipt = crate::heap::identity::identity()?;
        let mut keys = [0; 17];
        for key in &mut keys {
            *key = crate::heap::identity::identity()?;
        }
        let allocations = 6 + 11 * usize::from(cells != 0) + 7 + replacements;
        let cleanup = CleanupReceipt {
            identity: receipt,
            allocations,
            records: 23,
            edges: 25,
        };
        charge(budget, bulk::<N>(cells, word_count, replacements)?)?;
        Ok(Self {
            descriptor_keys: Some(descriptor_keys),
            layout_keys: Some(layout_keys),
            mask_keys: Some(mask_keys),
            cells,
            words: word_count,
            slab,
            slots,
            keys,
            request,
            nonce,
            groups,
            debits,
            revision,
            cleanup,
            permit,
        })
    }
}

/// Conservative preparation envelope counts the old facade until its final consuming move.
fn resources<P: Scalar + 'static, const N: usize>(
    cells: usize,
    slab: usize,
    changes: &[(usize, usize); 5],
) -> Result<ResourcePlan, LoanError> {
    let word_count = words(cells);
    let headers = size_of::<Storage>()
        + rc_bytes::<LcLiveness>()
        + rc_bytes::<LayoutRecord>()
        + rc_bytes::<MaskRecord>()
        + rc_bytes::<Bundle>()
        + rc_bytes::<Use>();
    let payload = add(
        add(
            slab.checked_mul(3).ok_or(LoanError::ByteOverflow)?,
            word_count.checked_mul(56).ok_or(LoanError::ByteOverflow)?,
        )?,
        cells
            .checked_mul(size_of::<LcOutcome>())
            .ok_or(LoanError::ByteOverflow)?,
    )?;
    let retained = changes
        .iter()
        .try_fold(add(headers, payload)?, |sum, (delta, _)| add(sum, *delta))?;
    let preparation = 17 * size_of::<Debit>()
        + 5 * size_of::<Key>()
        + 3 * size_of::<DescriptorRecord>()
        + size_of::<Native<LayoutRecord>>()
        + size_of::<Native<MaskRecord>>()
        + size_of::<super::admission::Transaction>()
        + size_of::<crate::loan::preparation::Preparation>()
        + size_of::<LcOmegaPrepared<'_, P, N>>();
    let scratch = changes.iter().try_fold(preparation, |sum, (_, old)| {
        add(sum, old.checked_mul(2).ok_or(crate::HeapError::Capacity)?)
    })?;

    let mut resource = ResourcePlan {
        retained,
        scratch,
        ..ResourcePlan::default()
    };
    resource.elements[crate::storage::Format::of::<P>().index()] =
        cells.checked_mul(3).ok_or(LoanError::CountOverflow)?;
    Ok(resource)
}

/// Enumerated variable initialization and fixed admission/cleanup event inventory.
fn bulk<const N: usize>(
    cells: usize,
    word_count: usize,
    replacements: usize,
) -> Result<usize, LoanError> {
    let fixed = 17 + 2 + 56 + 61 + 2 * replacements;
    let bulk = add(
        add(
            cells.checked_mul(4).ok_or(LoanError::CountOverflow)?,
            word_count.checked_mul(7).ok_or(LoanError::CountOverflow)?,
        )?,
        add(
            N.checked_mul(89).ok_or(LoanError::CountOverflow)?,
            fixed + 22 * usize::from(cells != 0),
        )?,
    )?;
    Ok(bulk)
}

/// Direct zero-capacity branch evidence where the existing homogeneous copy route cannot enter.
#[cfg(test)]
pub(super) fn zero_work() -> Result<(), LoanError> {
    let work = bulk::<5>(0, 0, 5)?;
    assert_eq!(work, 591);
    for short in [false, true] {
        let mut budget = Budget::new(nrz_bindings::Limits::default())?;
        let consume = budget.limits().work - (591 - u64::from(short));
        budget.work(consume, crate::loan::AT)?;
        let before = budget.usage().work;
        let result = charge(&mut budget, work);
        assert_eq!(result.is_err(), short);
        assert_eq!(budget.usage().work - before, if short { 0 } else { 591 });
    }
    Ok(())
}

/// Independent zero payload-byte branch, without claiming a reachable zero source transport.
#[cfg(test)]
pub(super) fn zero_bytes<P: Scalar + 'static>() -> Result<(), LoanError> {
    let changes = [
        (768, 7_680),
        (32, 512),
        (32, 352),
        (3_096, 30_960),
        (2_640, 22_176),
    ];
    let resource = resources::<P, 5>(0, 0, &changes)?;
    assert_eq!(resource.retained, 9_000);
    assert_eq!(resource.scratch, 131_192);
    assert_eq!(resource.elements, [0, 0]);
    Ok(())
}
