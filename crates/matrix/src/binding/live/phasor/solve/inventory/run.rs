// SPDX-License-Identifier: AGPL-3.0-only
//! A single finite plan precedes every native numerical owner and publication copy.

use super::owner::{Buffer, Regions};
use super::{COUNT, Owner, Permit, PhasorError, PhasorSolve, Plan, Stage, State};
use nrz_bindings::Budget;
use nrz_scalar::Scalar;
use std::{any::TypeId, cell::Cell, rc::Rc};

/// One native execution, including its optional atomic publication suffix.
#[derive(Debug)]
pub(crate) struct Run<P: Scalar>(pub(super) Rc<Plan<P>>);
impl<P: Scalar + 'static> Run<P> {
    /// Admit the full exact selection inventory before any covered numerical allocation.
    pub(crate) fn new(
        solve: &PhasorSolve<P>,
        publication: bool,
        budget: &Budget,
    ) -> Result<Self, PhasorError> {
        let metadata = super::record::metadata::<P>();
        let mut maximum = metadata;
        let mut states = [State::Planned; COUNT];
        for (slot, state) in states.iter_mut().enumerate() {
            let stage = Stage::at(slot)?;
            if !super::capture::present(solve, stage) || (slot >= 46 && !publication) {
                *state = State::Absent;
            } else {
                maximum = maximum
                    .checked_add(stage.representation::<P>().1)
                    .ok_or(crate::HeapError::Capacity)?;
            }
        }
        // The existing retained allowance covers these aligned owners and all scheduled payloads.
        if maximum > 4096 {
            return Err(crate::HeapError::Capacity.into());
        }
        super::record::check_budget(metadata, budget)?;
        Ok(Self(Rc::new(Plan {
            solve: solve.clone(),
            execution: crate::ExecutionId(crate::heap::identity::identity()?),
            states: states.map(Cell::new),
            bytes: std::array::from_fn(|_| Cell::new(0)),
            live: Cell::new(0),
            peak: Cell::new(0),
            publication,
        })))
    }
    /// The finite role and concrete type must match before a closure can allocate floats.
    fn grant<T: 'static>(&self, stage: Stage) -> Result<Permit<P>, PhasorError> {
        if TypeId::of::<T>() != stage.representation::<P>().0 {
            return Err(crate::PrivateAllocationError::Representation.into());
        }
        let slot = (0..COUNT)
            .find(|&slot| Stage::at(slot).is_ok_and(|value| value == stage))
            .ok_or(crate::PrivateAllocationError::UnplannedMeaning)?;
        if self.0.states[slot].get() != State::Planned {
            return Err(crate::PrivateAllocationError::DuplicateTicket.into());
        }
        self.0.states[slot].set(State::Issued);
        Ok(Permit {
            plan: Rc::clone(&self.0),
            slot,
        })
    }
    /// Inline arrays and proof fields keep the permit that preceded their construction.
    pub(crate) fn inline<T: 'static>(
        &self,
        stage: Stage,
        allocate: impl FnOnce() -> Result<T, PhasorError>,
    ) -> Result<Owner<P, T>, PhasorError> {
        if stage.is_vector() {
            return Err(crate::PrivateAllocationError::Representation.into());
        }
        let permit = self.grant::<T>(stage)?;
        let value = allocate()?;
        permit.allocated(size_of::<T>())?;
        Ok(Owner { value, permit })
    }
    /// The scalar vector cannot grow after its observed capacity has been bound to this permit.
    pub(crate) fn vector<T: 'static>(
        &self,
        stage: Stage,
        allocate: impl FnOnce() -> Result<Vec<T>, PhasorError>,
    ) -> Result<Buffer<P, T>, PhasorError> {
        if !stage.is_vector() {
            return Err(crate::PrivateAllocationError::Representation.into());
        }
        let permit = self.grant::<Vec<T>>(stage)?;
        let value = allocate()?;
        let entries = if stage == Stage::FixedInputs {
            self.0
                .solve
                .definition()
                .fixed_inputs()
                .iter()
                .flatten()
                .filter(|value| value.is_some())
                .count()
        } else {
            stage.entries()
        };
        if value.len() != entries {
            return Err(crate::PrivateAllocationError::IncompleteCoverage.into());
        }
        permit.allocated(
            value
                .capacity()
                .checked_mul(size_of::<T>())
                .ok_or(crate::HeapError::Capacity)?,
        )?;
        Ok(Buffer(Owner { value, permit }))
    }
    /// Each shared-verifier vector is completely initialized under its own original role.
    pub(crate) fn zeros(&self, stage: Stage) -> Result<Buffer<P>, PhasorError> {
        self.vector(stage, || {
            let mut values = crate::binding::buffer(stage.entries())?;
            values.resize(stage.entries(), P::ZERO);
            Ok(values)
        })
    }
    /// Prepare the three actual publication copies while retaining every pre-allocation permit.
    pub(crate) fn regions(
        &self,
        result: &super::super::Contents<P>,
    ) -> Result<Regions<P>, PhasorError> {
        self.complete(result)?;
        let center = self.copy(Stage::PublishedCenters, &result.payload.centers)?;
        let total = self.copy(Stage::PublishedTotals, &result.payload.totals)?;
        let error = self.copy(Stage::PublishedErrors, &result.payload.errors)?;
        Regions::new([center, total, error])
    }
    /// A fixed publication copy cannot request an unplanned capacity or change its source order.
    fn copy(&self, stage: Stage, values: &[P; 4]) -> Result<Buffer<P>, PhasorError> {
        self.vector(stage, || {
            let mut copy = crate::binding::buffer(4)?;
            copy.extend_from_slice(values);
            Ok(copy)
        })
    }
}
impl<P: Scalar> Run<P> {
    /// Exact ticket association is checked independently of lifecycle coverage.
    pub(super) fn check_permit(&self, permit: &Permit<P>, slot: usize) -> Result<(), PhasorError> {
        if !Rc::ptr_eq(&self.0, &permit.plan) || permit.slot != slot {
            return Err(crate::PrivateAllocationError::ForeignInventory.into());
        }
        if self.0.states[slot].get() != State::Live {
            return Err(crate::PrivateAllocationError::IncompleteCoverage.into());
        }
        Ok(())
    }
    /// Complete live final owners and a fully released transient prefix are mandatory.
    pub(crate) fn complete(
        &self,
        result: &super::super::Contents<P>,
    ) -> Result<super::PhasorInventory<P>, PhasorError> {
        self.finish(result, false)
    }
    /// Publication authenticates every actual numerical owner before the heap commit.
    pub(crate) fn published(
        &self,
        result: &super::super::Contents<P>,
        regions: &Regions<P>,
    ) -> Result<super::PhasorInventory<P>, PhasorError> {
        regions.values(self)?;
        self.finish(result, true)
    }
    /// Only complete transient coverage and the original live final owners may issue a receipt.
    fn finish(
        &self,
        result: &super::super::Contents<P>,
        publication: bool,
    ) -> Result<super::PhasorInventory<P>, PhasorError> {
        if publication && !self.0.publication {
            return Err(crate::PrivateAllocationError::UnplannedMeaning.into());
        }
        self.check_permit(&result.payload.permit, 40)?;
        self.check_permit(&result.certificate.permit, 41)?;
        self.check_permit(&result.transfer.permit, 42)?;
        self.check_permit(&result.residual.permit, 43)?;
        for (slot, state) in self.0.states.iter().enumerate() {
            let stage = Stage::at(slot)?;
            if slot >= 46 && !publication {
                continue;
            }
            let expected = if stage.retained() || slot >= 46 {
                State::Live
            } else {
                State::Released
            };
            if state.get() != State::Absent && state.get() != expected {
                return Err(crate::PrivateAllocationError::IncompleteCoverage.into());
            }
        }
        Ok(super::PhasorInventory {
            plan: Rc::clone(&self.0),
            peak: self.0.peak.get(),
            publication,
        })
    }
}
