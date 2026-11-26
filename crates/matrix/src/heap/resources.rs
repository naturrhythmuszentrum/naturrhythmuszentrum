// SPDX-License-Identifier: AGPL-3.0-only
//! One admission inventory combines legacy storage, detached debits and native owners.

use crate::{
    Heap, HeapError,
    storage::{Format, PreparedStorage, Region},
};
use nrz_bindings::{Budget, Limits};
use nrz_scalar::Scalar;

/// A complete additional retained set and simultaneous private allocation peak.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ResourcePlan {
    /// Additional addressable destination elements by precision.
    pub(crate) elements: [usize; 2],
    /// Actually replaced legacy elements; their bytes remain in the staging peak.
    pub(crate) retired: [usize; 2],
    /// Newly retained owner capacity; replacement tables contribute their capacity increase.
    pub(crate) retained: usize,
    /// Private scratch and replacement buffers beyond retained additions.
    pub(crate) scratch: usize,
}
impl ResourcePlan {
    /// Compatibility routes admit bytes early and check destination elements at publication.
    pub(crate) const fn bytes_only(self) -> Self {
        Self {
            elements: [0; 2],
            retired: [0; 2],
            ..self
        }
    }
}
/// Effective original ceilings; compatibility calls preserve their default policy.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ResourcePolicy {
    /// Retained complete owner ceiling.
    pub(crate) output: u64,
    /// Simultaneous old-plus-new ceiling.
    pub(crate) staged: u64,
}
impl ResourcePolicy {
    /// Read effective scoped ceilings without modifying the original ledger.
    pub(crate) const fn from_budget(budget: &Budget) -> Self {
        Self {
            output: budget.limits().output_bytes,
            staged: budget.limits().staged_payload_bytes,
        }
    }
    /// Legacy no-account resource policy.
    pub(crate) fn compatibility() -> Self {
        let limits = Limits::default();
        Self {
            output: limits.output_bytes,
            staged: limits.staged_payload_bytes,
        }
    }
}
/// Occupation-qualified authorization for the complete prechecked allocation peak.
#[derive(Debug)]
pub(crate) struct ResourcePermit {
    /// Heap scope authenticated by the caller's exclusive borrow.
    pub(crate) scope: u64,
    /// Account-aware continuations retain the original ledger owner identity.
    pub(crate) account: Option<u64>,
    /// Storage epoch prevents use across relocation.
    pub(crate) epoch: u64,
    /// Complete admitted retained capacity.
    pub(crate) retained: usize,
    /// Complete simultaneous peak.
    pub(crate) peak: usize,
}
impl ResourcePermit {
    /// Isolated storage corruption fixtures have no managed owner or shared heap inventory.
    #[cfg(test)]
    pub(crate) const fn testing(epoch: u64) -> Self {
        Self {
            scope: 0,
            account: None,
            epoch,
            retained: 0,
            peak: 0,
        }
    }

    /// Every storage allocator consumes a preflight from this exact physical epoch.
    pub(crate) const fn storage(&self, epoch: u64) -> Result<(), HeapError> {
        if self.epoch == epoch && self.retained <= self.peak && !matches!(self.account, Some(0)) {
            Ok(())
        } else {
            Err(HeapError::StaleLayout)
        }
    }
}

impl Heap {
    /// Exact slot visits performed by the common element and byte inventory.
    pub(crate) fn resource_work(&self) -> Result<usize, HeapError> {
        add(
            add(self.storage.scan_work(), self.masks.layouts.scan_work())?,
            add(
                self.masks.masks.scan_work(),
                add(self.loans.debit_work(), self.executions.scan_work())?,
            )?,
        )
    }
    /// Admit a descriptor-only route before its one table changes.
    pub(crate) fn prepare_descriptors(
        &mut self,
        count: usize,
    ) -> Result<super::slots::SlotPlan, HeapError> {
        let plan = self.descriptor_plan(count, None, &[])?;
        let permit = self.preflight_resources(plan, ResourcePolicy::compatibility())?;
        self.change_descriptors(count, None, &[], &permit)
    }
    /// Descriptor growth and its private occupation keys, without changing either table.
    fn descriptor_plan(
        &self,
        count: usize,
        keep: Option<super::identity::Key>,
        retired: &[super::identity::Key],
    ) -> Result<ResourcePlan, HeapError> {
        let (retained, old) = self
            .descriptors
            .slots
            .growth_bytes_for(count, keep, |key| retired.contains(&key))?;
        let scratch = add(
            add(
                old.checked_mul(2).ok_or(HeapError::Capacity)?,
                count
                    .checked_mul(size_of::<super::records::Record>())
                    .ok_or(HeapError::Capacity)?,
            )?,
            add(count, retired.len())?
                .checked_mul(size_of::<super::identity::Key>())
                .ok_or(HeapError::Capacity)?,
        )?;
        Ok(ResourcePlan {
            retained,
            scratch,
            ..Default::default()
        })
    }
    /// One complete import or replacement plan precedes both descriptor and arena growth.
    pub(crate) fn legacy_plan<P: Scalar + 'static>(
        &self,
        lengths: impl ExactSizeIterator<Item = usize>,
        keep: Option<super::identity::Key>,
        retired_descriptors: &[super::identity::Key],
        retired_storage: &[Region],
    ) -> Result<ResourcePlan, HeapError> {
        let count = lengths.len();
        let descriptors = self.descriptor_plan(count, keep, retired_descriptors)?;
        let mut plan = self.storage_plan::<P>(lengths, retired_storage.iter().copied())?;
        plan.retained = add(plan.retained, descriptors.retained)?;
        plan.scratch = add(plan.scratch, descriptors.scratch)?;
        Ok(plan)
    }
    /// Both legacy allocation helpers consume the original complete per-call permit.
    pub(crate) fn change_descriptors(
        &mut self,
        count: usize,
        keep: Option<super::identity::Key>,
        retired: &[super::identity::Key],
        permit: &ResourcePermit,
    ) -> Result<super::slots::SlotPlan, HeapError> {
        permit.storage(self.storage.epoch())?;
        if permit.scope != self.scope {
            return Err(HeapError::ForeignHandle);
        }
        self.descriptors
            .change(count, keep, retired, self.limits.descriptors)
    }
    /// Inventory each owner at its unique registry entry, even while it is detached.
    /// The inline `Heap` is outside allocated payload ceilings; the native mask store keeps
    /// its existing inline-header allowance, while every allocated backing table is included.
    pub(crate) fn resource_bytes(&self) -> Result<usize, HeapError> {
        let masks = self
            .masks
            .inventory_bytes()
            .map_err(|_| HeapError::Capacity)?;
        add(
            add(
                self.storage.owned_bytes()?,
                self.descriptors.slots.owned_bytes()?,
            )?,
            add(
                add(masks, self.loans.owned_bytes()?)?,
                self.executions.owned_bytes()?,
            )?,
        )
    }
    /// One combined admission precedes growth: heap-wide elements and per-call owner bytes.
    /// The per-call byte scope preserves existing ceilings rather than imposing a new heap byte cap.
    /// `validate_with_resource_budget` and internal `validate_quantity` preserve metadata-only
    /// caller scopes and use default physical ceilings; `validate_with_account` supplies its own.
    pub(crate) fn preflight_resources(
        &self,
        plan: ResourcePlan,
        policy: ResourcePolicy,
    ) -> Result<ResourcePermit, HeapError> {
        self.preflight_resources_scoped(plan, policy, None)
    }
    /// Owning-layer overlap distinguishes already resident old buffers from new staging.
    pub(crate) fn preflight_resources_scoped(
        &self,
        plan: ResourcePlan,
        policy: ResourcePolicy,
        overlap: Option<usize>,
    ) -> Result<ResourcePermit, HeapError> {
        self.preflight_elements(plan.elements, plan.retired)?;
        // Heap limits govern aggregate destination elements; per-call byte ceilings govern
        // this call's new owners and its own replacement peak, not unrelated resident owners.
        let native = self.resource_bytes()?;
        // Retained growth counts the new capacity, while scratch includes both old portions
        // of a replacement: delta + old + old equals the simultaneous old and new buffers.
        let retained = plan.retained;
        let peak = add(retained, plan.scratch)?;
        if u64::try_from(retained).map_err(|_| HeapError::Capacity)? > policy.output {
            return Err(HeapError::ResourceBytes {
                staged: false,
                used: u64::try_from(retained).map_err(|_| HeapError::Capacity)?,
                limit: policy.output,
            });
        }
        if u64::try_from(peak).map_err(|_| HeapError::Capacity)? > policy.staged {
            return Err(HeapError::ResourceBytes {
                staged: true,
                used: u64::try_from(peak).map_err(|_| HeapError::Capacity)?,
                limit: policy.staged,
            });
        }
        if let Some(gate) = &self.graph_gate {
            gate.check(native, plan, overlap)?;
        }
        Ok(ResourcePermit {
            scope: self.scope,
            account: None,
            epoch: self.storage.epoch(),
            retained,
            peak,
        })
    }
    /// Destination occupations share one heap-wide precision ceiling at each allocation phase.
    pub(crate) fn preflight_elements(
        &self,
        elements: [usize; 2],
        retired: [usize; 2],
    ) -> Result<(), HeapError> {
        let managed = self.loans.elements()?;
        let legacy = self.storage.elements();
        for index in 0..2 {
            let retained = add(legacy[index], managed[index])?
                .checked_sub(retired[index])
                .ok_or(HeapError::Capacity)?;
            if add(retained, elements[index])? > self.limits.elements {
                if let Some(gate) = &self.graph_gate {
                    gate.element_failure(legacy, managed, retired, elements, self.limits.elements);
                }
                return Err(HeapError::Capacity);
            }
        }
        Ok(())
    }
    /// Complete legacy allocation plan, including backend rounding and table replacement.
    pub(crate) fn storage_plan<P: Scalar + 'static>(
        &self,
        lengths: impl Iterator<Item = usize>,
        retired: impl Iterator<Item = Region> + Clone,
    ) -> Result<ResourcePlan, HeapError> {
        let mut plan = ResourcePlan::default();
        let index = Format::of::<P>().index();
        let mut count = 0;
        for length in lengths {
            plan.elements[index] = add(plan.elements[index], length)?;
            plan.retained = add(plan.retained, slab_bytes::<P>(length)?)?;
            count = add(count, 1)?;
        }
        for region in retired.clone() {
            plan.scratch = add(
                plan.scratch,
                match region.format {
                    Format::Binary32 => slab_bytes::<f32>(region.length)?,
                    Format::Binary64 => slab_bytes::<f64>(region.length)?,
                },
            )?;
            plan.retired[region.format.index()] =
                add(plan.retired[region.format.index()], region.length)?;
        }
        let (tables, old_tables) = self.storage.growth_bytes::<P>(count, &retired)?;
        plan.retained = add(plan.retained, tables)?;
        plan.scratch = add(
            add(
                plan.scratch,
                old_tables.checked_mul(2).ok_or(HeapError::Capacity)?,
            )?,
            count
                .checked_mul(size_of::<faer::Mat<P>>())
                .ok_or(HeapError::Capacity)?,
        )?;
        // The arena plan retains its destination and retirement keys; preflight's separate
        // retirement-key vector stays live while change constructs the plan's copy.
        let keys = add(
            count,
            retired.count().checked_mul(2).ok_or(HeapError::Capacity)?,
        )?;
        plan.scratch = add(
            plan.scratch,
            keys.checked_mul(size_of::<super::identity::Key>())
                .ok_or(HeapError::Capacity)?,
        )?;
        Ok(plan)
    }
    /// Preserve the legacy element-refusal point while consuming the earlier complete byte permit.
    pub(crate) fn prepare_storage<P: Scalar + 'static>(
        &mut self,
        payloads: &[&[P]],
        retired: &[Region],
        permit: &ResourcePermit,
    ) -> Result<PreparedStorage, HeapError> {
        self.prepare_storage_iter(payloads.iter().copied(), retired, permit)
    }

    /// Preserve late element timing while borrowing payload slices without another owner.
    pub(crate) fn prepare_storage_iter<'a, P: Scalar + 'static>(
        &mut self,
        payloads: impl Iterator<Item = &'a [P]> + Clone,
        retired: &[Region],
        permit: &ResourcePermit,
    ) -> Result<PreparedStorage, HeapError> {
        let mut elements = [0; 2];
        elements[Format::of::<P>().index()] = payloads
            .clone()
            .try_fold(0_usize, |sum, values| add(sum, values.len()))?;
        let mut removed = [0; 2];
        for region in retired {
            removed[region.format.index()] = add(removed[region.format.index()], region.length)?;
        }
        self.preflight_elements(elements, removed)?;
        self.storage
            .prepare_iter(payloads, retired, self.limits, permit)
    }
}
/// Checked native addition used by every owner inventory.
pub(crate) fn add(left: usize, right: usize) -> Result<usize, HeapError> {
    left.checked_add(right).ok_or(HeapError::Capacity)
}
/// Pinned one-column backend rounding is admitted before the safe constructor.
pub(crate) fn slab_bytes<P: Scalar>(length: usize) -> Result<usize, HeapError> {
    let width = size_of::<P>();
    let alignment = 64 / width;
    let rows = (length / alignment)
        .checked_add(usize::from(!length.is_multiple_of(alignment)))
        .and_then(|groups| groups.checked_mul(alignment))
        .ok_or(HeapError::ExtentOverflow)?;
    let bytes = rows.checked_mul(width).ok_or(HeapError::ExtentOverflow)?;
    if bytes > isize::MAX.unsigned_abs() / 2 {
        return Err(HeapError::ExtentOverflow);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
