// SPDX-License-Identifier: AGPL-3.0-only
//! Literal owner inventory and cleanup prepayment precede all admission allocation.

use super::{
    BundleOwner, Debit, DescriptorRecord, LayoutRecord, Liveness, LoanError, MaskRecord, Native,
    PinRecord,
};
use crate::{
    Heap,
    heap::{
        identity::Key,
        resources::{ResourcePermit, ResourcePlan, ResourcePolicy, add, slab_bytes},
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
    /// Receipt, request, nonce and every selected debit identity reserved before allocation.
    pub(super) identities: [u64; 26],
    /// Complete admitted scope, account and storage epoch.
    pub(super) permit: ResourcePermit,
    /// Extra filled-word, outcome and per-cell-extent allocations.
    pub(super) extras: [usize; 3],
    /// Additional boxed right correspondence, absent for every unary member.
    pub(super) side_bytes: usize,
    /// Present native channels.
    pub(super) present: [bool; 3],
    /// Present channel count.
    pub(super) channels: usize,
    /// Complete storage-role count, including result-specific endpoints.
    pub(super) roles: usize,
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
    pub(super) debit_slots: [usize; 23],
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
        request: &super::super::prepared::Request<'_, P>,
        budget: &mut nrz_bindings::Budget,
    ) -> Result<Self, LoanError> {
        let declaration = &request.destination;
        charge(budget, 1)?;
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
        let length = declaration.extent.entries();
        let geometry = Geometry {
            present,
            channels,
            length,
            component_words: words(length),
            cell_words: words(declaration.capacity()),
            rounded: slab_bytes::<P>(length)?,
        };
        let member = request.member()?;
        let debit_count = member.debits();
        charge(budget, heap.loans.debits.len())?;
        let debit_slots = heap.loans.vacant_inventory::<23>(debit_count)?;
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
        let headers = header_bytes::<P>();
        let identities = identities(debit_count, member.is_binary())?;
        let input_channels = request.input_channels()?;
        let cleanup = CleanupReceipt {
            identity: identities[0],
            allocations: (channels + 2 * usize::from(member.is_prepared()))
                * (usize::from(length != 0)
                    + usize::from(geometry.component_words != 0)
                    + usize::from(geometry.cell_words != 0))
                + 13
                + usize::from(member.is_binary())
                + usize::from(geometry.cell_words != 0)
                + usize::from(declaration.capacity() != 0)
                + geometry.replacements(heap, group_capacity, debit_capacity, pin_capacity)?,
            records: channels + debit_count + 6,
            // Destination symbol handles and common owners; input declaration handles,
            // three operation handles, three exclusion witnesses and the one-cell extent.
            // The transferred numerical pin's drop was already prepaid by its creator.
            edges: channels
                + 3
                + input_channels
                + 7
                + 7 * usize::from(member.is_prepared())
                + (input_channels + 6) * usize::from(member.is_binary()),
        };
        let mut extras = geometry.extra_bytes(declaration.capacity())?;
        if member.is_binary() {
            extras[2] = rc_bytes::<[crate::Extent; 2]>();
        }
        let mut plan = Self {
            identities,
            permit: ResourcePermit {
                scope: heap.scope,
                account: Some(declaration.account),
                epoch: heap.storage.epoch(),
                retained: 0,
                peak: 0,
            },
            extras,
            side_bytes: if member.is_binary() {
                size_of::<super::super::binary::Side>()
            } else {
                0
            },
            present: geometry.present,
            channels: geometry.channels,
            roles: member.roles(),
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
        plan.prepay::<P>(heap, request, budget)?;
        Ok(plan)
    }
    /// Prepay all explicit loops and acquired-prefix destruction after complete common admission.
    fn prepay<P: Scalar + 'static>(
        &mut self,
        heap: &Heap,
        request: &super::super::prepared::Request<'_, P>,
        budget: &mut nrz_bindings::Budget,
    ) -> Result<(), LoanError> {
        charge(budget, heap.resource_work()?)?;
        self.permit = self.preflight::<P>(heap, request.member()?.is_prepared(), budget)?;
        self.permit.account = Some(request.destination.account);
        let cleanup_work = add(
            add(self.cleanup.allocations, self.cleanup.records)?,
            add(
                self.cleanup.edges,
                15 + 4 * self.channels + usize::from(self.side_bytes != 0),
            )?,
        )?;
        let initialization = self
            .roles
            .checked_mul(add(
                self.length,
                add(self.component_words, self.cell_words)?,
            )?)
            .ok_or(LoanError::CountOverflow)?;
        let input_channels = request.input_channels()?;
        let mask_work = if request.member()?.is_transform() {
            super::super::movement::availability_work(
                request.destination.capacity(),
                self.cell_words,
            )?
        } else {
            (input_channels + 4)
                .checked_mul(self.cell_words)
                .ok_or(LoanError::CountOverflow)?
        };
        let initialization = add(
            initialization,
            add(mask_work, request.destination.capacity())?,
        )?;
        let slots = add(
            heap.descriptors.slots.scan_work(),
            add(heap.masks.layouts.scan_work(), heap.masks.masks.scan_work())?,
        )?;
        let preparation = add(self.cleanup.records, add(self.cleanup.edges + 1, slots)?)?;
        charge(
            budget,
            add(cleanup_work, add(initialization, preparation)?)?,
        )?;
        Ok(())
    }
    /// Retained deltas and replacement buffers are disjoint entries in the common plan.
    fn preflight<P: Scalar + 'static>(
        &self,
        heap: &Heap,
        prepared: bool,
        budget: &nrz_bindings::Budget,
    ) -> Result<ResourcePermit, LoanError> {
        let channel_bytes = add(
            self.rounded,
            add(self.component_words, self.cell_words)?
                .checked_mul(8)
                .ok_or(LoanError::ByteOverflow)?,
        )?;
        let mut retained = add(
            self.roles
                .checked_mul(channel_bytes)
                .ok_or(LoanError::ByteOverflow)?,
            self.headers.into_iter().try_fold(0_usize, add)?,
        )?;
        retained = add(retained, self.extras.into_iter().try_fold(0_usize, add)?)?;
        retained = add(retained, self.side_bytes)?;
        let mut scratch = 0;
        for (growth, old) in [
            heap.descriptors
                .slots
                .growth_bytes_reserved_limit(self.channels, heap.limits.descriptors)?,
            heap.masks
                .layouts
                .growth_bytes_reserved_limit(1, heap.limits.descriptors)?,
            heap.masks
                .masks
                .growth_bytes_reserved_limit(1, heap.limits.descriptors)?,
            crate::loan::lifecycle::admission::growth(&heap.loans.groups, self.group_capacity)?,
            crate::loan::lifecycle::admission::growth(&heap.loans.debits, self.debit_capacity)?,
            crate::loan::lifecycle::admission::growth(&heap.loans.pins, self.pin_capacity)?,
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
            .checked_mul(self.roles)
            .ok_or(LoanError::CountOverflow)?;
        if prepared {
            budget.check(
                nrz_bindings::readings::algebra::limits::BudgetKind::StagedPayloadBytes,
                u64::try_from(add(retained, scratch)?).map_err(|_| LoanError::ByteOverflow)?,
                crate::loan::AT,
            )?;
        }
        Ok(heap.preflight_resources(resources, ResourcePolicy::from_budget(budget))?)
    }
}

/// Reserve only the selected closed member's contiguous identity block.
fn identities(debits: usize, binary: bool) -> Result<[u64; 26], LoanError> {
    if debits == 23 {
        return Ok(crate::heap::identity::identity_block::<26>()?);
    }
    if binary {
        let source = crate::heap::identity::identity_block::<21>()?;
        Ok(std::array::from_fn(|index| {
            source.get(index).copied().unwrap_or(0)
        }))
    } else {
        let source = crate::heap::identity::identity_block::<20>()?;
        Ok(std::array::from_fn(|index| {
            source.get(index).copied().unwrap_or(0)
        }))
    }
}

/// Core and shared shells are counted through their containing inline headers exactly once.
const fn header_bytes<P: Scalar>() -> [usize; 5] {
    [
        size_of::<super::super::ApplicationCore<P>>(),
        rc_bytes::<LayoutRecord>(),
        rc_bytes::<Liveness>(),
        rc_bytes::<BundleOwner>(),
        rc_bytes::<PinRecord>(),
    ]
}
impl Geometry {
    /// Capacity refusal uses the same already prepaid vacancy scans as replacement planning.
    fn replacements(
        &self,
        heap: &Heap,
        groups: usize,
        debits: usize,
        pins: usize,
    ) -> Result<usize, LoanError> {
        let replacements = [
            heap.descriptors
                .slots
                .growth_bytes_reserved_limit(self.channels, heap.limits.descriptors)?,
            heap.masks
                .layouts
                .growth_bytes_reserved_limit(1, heap.limits.descriptors)?,
            heap.masks
                .masks
                .growth_bytes_reserved_limit(1, heap.limits.descriptors)?,
        ]
        .into_iter()
        .filter(|(growth, _)| *growth != 0)
        .count();
        Ok(replacements
            + usize::from(groups > heap.loans.groups.capacity())
            + usize::from(debits > heap.loans.debits.capacity())
            + usize::from(pins > heap.loans.pins.capacity()))
    }
    /// Three disjoint extra backings have no hidden per-cell allocations.
    fn extra_bytes(&self, cells: usize) -> Result<[usize; 3], LoanError> {
        Ok([
            self.cell_words
                .checked_mul(8)
                .ok_or(LoanError::ByteOverflow)?,
            cells
                .checked_mul(size_of::<super::super::CellOutcome>())
                .ok_or(LoanError::ByteOverflow)?,
            rc_bytes::<[crate::Extent; 1]>(),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::Geometry;

    /// Isolated arithmetic overflow controls refuse before entering any numerical constructor.
    /// ´test:unit:pointwise-count-byte-and-signed-address-overflow´
    #[test]
    fn pointwise_count_byte_and_signed_address_overflow() {
        let injection = crate::loan::fault::Injection::after(0);
        let geometry = Geometry {
            present: [true; 3],
            channels: 3,
            length: 0,
            component_words: 0,
            cell_words: usize::MAX / 8 + 1,
            rounded: 0,
        };
        assert!(matches!(
            geometry.extra_bytes(0),
            Err(crate::LoanError::ByteOverflow)
        ));
        let geometry = Geometry {
            cell_words: 0,
            ..geometry
        };
        assert!(matches!(
            geometry.extra_bytes(usize::MAX),
            Err(crate::LoanError::ByteOverflow)
        ));
        assert!(crate::Extent::new(usize::MAX, 2).is_err());
        assert!(crate::heap::resources::slab_bytes::<f32>(usize::MAX / 4).is_err());
        assert!(crate::heap::resources::slab_bytes::<f64>(usize::MAX / 8).is_err());
        assert!(crate::heap::resources::slab_bytes::<f64>((isize::MAX as usize / 8) + 1).is_err());
        assert!(crate::kernels::contract::work(usize::MAX / 12 + 1, 12).is_err());
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        drop(injection);
    }
}
