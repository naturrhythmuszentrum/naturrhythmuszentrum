// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable native receipts share original meanings and measured capacity observations.

use super::{COUNT, PhasorError, PhasorSolve, Plan, RcLayout, Stage, State};
use nrz_bindings::{Budget, readings::algebra::limits::BudgetKind};
use nrz_scalar::{Precision, Scalar};
use std::rc::Rc;

/// Historical projection of one closed native allocation into its shared original plan.
#[derive(Clone, Debug)]
pub struct PhasorAllocation<P: Scalar> {
    /// Original exact definitions and immutable capacity observations.
    plan: Rc<Plan<P>>,
    /// Non-reusable schedule position.
    slot: usize,
}
impl<P: Scalar> PhasorAllocation<P> {
    /// Original equation, physical references, fixed inputs and every selected source version.
    #[must_use]
    pub fn plan(&self) -> &PhasorSolve<P> {
        &self.plan.solve
    }
    /// Exact numerical role within this original equation.
    /// # Errors
    /// Refuses a corrupt private schedule position.
    pub fn stage(&self) -> Result<Stage, PhasorError> {
        Stage::at(self.slot)
    }
    /// Opaque historical ticket; it cannot allocate or authorize a numerical read.
    #[must_use]
    pub fn ticket(&self) -> crate::PrivateAllocationTicket {
        crate::PrivateAllocationTicket::new(self.plan.execution, self.plan.execution.0, self.slot)
    }
    /// Actual fixed-owner size or observed vector capacity, excluding allocator overhead.
    #[must_use]
    pub fn owned_bytes(&self) -> usize {
        self.plan.bytes[self.slot].get()
    }
    /// Actual logical entries, excluding absent original fixed-input components.
    /// # Errors
    /// Refuses a corrupt private schedule position.
    pub fn entries(&self) -> Result<usize, PhasorError> {
        let stage = self.stage()?;
        Ok(if stage == Stage::FixedInputs {
            self.plan
                .solve
                .definition()
                .fixed_inputs()
                .iter()
                .flatten()
                .filter(|value| value.is_some())
                .count()
        } else {
            stage.entries()
        })
    }
    /// Primitive floating fields, including both endpoints of each present fixed component.
    /// # Errors
    /// Refuses a corrupt private schedule position.
    pub fn float_entries(&self) -> Result<usize, PhasorError> {
        let stage = self.stage()?;
        Ok(match stage {
            Stage::FixedInputs | Stage::References | Stage::Transfer => self.entries()? * 2,
            _ => self.entries()?,
        })
    }
    /// Same original allocating interpretation and stage, independent of equal values or addresses.
    #[must_use]
    pub fn same_binding(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.plan, &other.plan) && self.slot == other.slot
    }
}
/// Completed native solve inventory, optionally including its three publication copies.
///
/// It describes only the native solver's observed owners. Heap destination slabs and the
/// separate restoration product retain their own contracts; no backend allowance is used here.
#[derive(Clone, Debug)]
pub struct PhasorInventory<P: Scalar> {
    /// Original meanings and one capacity observation per allocation, never copied recipes.
    pub(super) plan: Rc<Plan<P>>,
    /// Peak captured at the completion boundary, unaffected by later clone/drop activity.
    pub(super) peak: usize,
    /// Whether the three publication-copy owners completed before this receipt was issued.
    pub(super) publication: bool,
}
impl<P: Scalar> PhasorInventory<P> {
    /// Allocations completed by this execution; absent companion channels create no buffers.
    pub fn allocations(&self) -> impl Iterator<Item = PhasorAllocation<P>> + '_ {
        let end = if self.publication { COUNT } else { 46 };
        (0..end)
            .filter(|&slot| self.plan.states[slot].get() != State::Absent)
            .map(|slot| PhasorAllocation {
                plan: Rc::clone(&self.plan),
                slot,
            })
    }
    /// Concrete scalar format of every owner, including enclosure endpoints.
    #[must_use]
    pub const fn precision(&self) -> Precision {
        P::PRECISION
    }
    /// Largest common live numerical payload, including fixed fields and actual vector capacities.
    #[must_use]
    pub const fn peak_payload_bytes(&self) -> usize {
        self.peak
    }
    /// Aligned plan allocation, with all exact declarations shared from their original owner.
    #[must_use]
    pub const fn planning_bytes(&self) -> usize {
        size_of::<RcLayout<Plan<P>>>()
    }
    /// New aligned plan, final owner metadata and inline historical receipt; excludes numerical fields.
    #[must_use]
    pub const fn metadata_bytes(&self) -> usize {
        metadata::<P>()
    }
    /// Three current publication copies are included only after all three owners complete.
    #[must_use]
    pub const fn includes_publication(&self) -> bool {
        self.publication
    }
}
/// Count aligned allocations, subtracting numerical fields already counted by their own permits.
pub(super) const fn metadata<P: Scalar>() -> usize {
    size_of::<RcLayout<Plan<P>>>()
        + size_of::<RcLayout<super::super::Contents<P>>>()
        + size_of::<PhasorInventory<P>>()
        - 31 * size_of::<P>()
}
/// Both metadata ceilings apply before the first covered numerical allocation.
pub(super) fn check_budget(bytes: usize, budget: &Budget) -> Result<(), PhasorError> {
    let bytes = u64::try_from(bytes).map_err(|_| crate::HeapError::Capacity)?;
    let at = super::super::super::AT;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, at)?;
    budget.check(BudgetKind::OutputBytes, bytes, at)?;
    Ok(())
}
