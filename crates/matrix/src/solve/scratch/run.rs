// SPDX-License-Identifier: AGPL-3.0-only
//! Admit finite exact meanings and metadata before any covered solve array is allocated.

use super::plan::{RcLayout, Specification};
use super::{Backend, Permit, Plan, Stage, State};
use crate::{Bound, Dependency, OperationError, PrivateAllocationError as Error};
use nrz_bindings::{BindingSymbols, Budget};
use nrz_scalar::Scalar;
use std::{any::TypeId, cell::Cell, rc::Rc};

/// Shared schedule; its non-cloneable permits are retained by numerical owners.
#[derive(Debug)]
pub(crate) struct Run(pub(super) Rc<Plan>);
impl Run {
    /// The checked original bindings and finite policy determine the full schedule.
    pub(crate) fn new<P: Scalar + 'static, Q: Scalar + 'static>(
        bound: &Bound<'_, P, Q>,
        token: u64,
        budget: &Budget,
    ) -> Result<Self, OperationError> {
        let crate::binding::NativeRecipe::Solve(contract) = &bound.operation.recipe else {
            return Err(Error::UnplannedMeaning.into());
        };
        if TypeId::of::<P>() != TypeId::of::<Q>() {
            return Err(Error::Representation.into());
        }
        let sensitivity = bound.statistics.derivative.is_some();
        let count = super::schedule::count(contract)?
            .checked_add(super::sensitivity::count(contract, sensitivity)?)
            .and_then(|n| n.checked_add(super::statistics::count(&bound.statistics)))
            .ok_or(OperationError::ResourceLimit)?;
        let maps = bound.dependencies.iter().try_fold(0_usize, |sum, input| {
            sum.checked_add(input.layout().owned_bytes())
                .ok_or(OperationError::ResourceLimit)
        })?;
        let predicted = metadata(count, bound.dependencies.len(), maps)?
            .checked_add(super::statistics::Binding::predicted(&bound.statistics)?)
            .ok_or(OperationError::ResourceLimit)?;
        check_budget(predicted, budget)?;
        let plan = Self::construct(bound, token, count)?;
        check_budget(plan.metadata, budget)?;
        Ok(Self(Rc::new(plan)))
    }
    /// Allocate admitted metadata only; every numerical allocator remains behind a later permit.
    fn construct<P: Scalar + 'static, Q: Scalar + 'static>(
        bound: &Bound<'_, P, Q>,
        token: u64,
        count: usize,
    ) -> Result<Plan, OperationError> {
        let crate::binding::NativeRecipe::Solve(contract) = &bound.operation.recipe else {
            return Err(Error::UnplannedMeaning.into());
        };
        let sensitivity = bound.statistics.derivative.is_some();
        let mut specifications = crate::binding::buffer(count)?;
        super::schedule::specifications(contract, &mut specifications)?;
        super::sensitivity::specifications(contract, sensitivity, &mut specifications)?;
        super::statistics::specifications(contract, &bound.statistics, &mut specifications)?;
        let statistics = super::statistics::Binding::capture(&bound.statistics)?;
        let mut sources = crate::binding::buffer(bound.dependencies.len())?;
        for input in &bound.dependencies {
            sources.push((Dependency::from_snapshot(input), input.symbols.clone()));
        }
        let owned_maps = sources.iter().try_fold(0_usize, |sum, (dependency, _)| {
            sum.checked_add(dependency.layout().owned_bytes())
                .ok_or(OperationError::ResourceLimit)
        })?;
        let bytes = metadata(specifications.capacity(), sources.capacity(), owned_maps)?
            .checked_add(statistics.as_ref().map_or(Ok(0), |v| v.owned_bytes())?)
            .ok_or(OperationError::ResourceLimit)?;
        contract
            .work
            .checked_add(bound.statistics.work)
            .and_then(|work| work.checked_mul(size_of::<P>()))
            .ok_or(OperationError::ResourceLimit)?;
        Ok(Plan {
            execution: bound.execution,
            token,
            contract: Rc::clone(contract),
            derivative: bound.statistics.derivative.clone(),
            statistics,
            enclosure: TypeId::of::<nrz_scalar::Enclosure<P>>(),
            sources,
            output: bound
                .reservations
                .symbols
                .first()
                .ok_or(Error::IncompleteCoverage)?
                .clone(),
            specifications,
            kind: TypeId::of::<P>(),
            precision: P::PRECISION,
            width: size_of::<P>(),
            peak: Cell::new(0),
            completed: Cell::new(0),
            metadata: bytes,
        })
    }
    /// Match the exact finite role before drawing any numerical authority.
    pub(super) fn grant(
        &self,
        stage: Stage,
        iteration: Option<usize>,
    ) -> Result<Permit, OperationError> {
        let slot = self
            .0
            .specifications
            .iter()
            .position(|entry| entry.stage == stage && entry.iteration == iteration)
            .ok_or(Error::UnplannedMeaning)?;
        let entry = &self.0.specifications[slot];
        if entry.state.get() != State::Planned {
            return Err(Error::DuplicateTicket.into());
        }
        entry.state.set(State::Issued);
        Ok(Permit {
            plan: Rc::clone(&self.0),
            slot,
        })
    }
    /// Concrete scalar type is checked before the fallible owned-vector allocation.
    pub(crate) fn buffer<P: 'static>(
        &self,
        stage: Stage,
        iteration: Option<usize>,
    ) -> Result<crate::private::Buffer<P>, OperationError> {
        let kind = if stage.components() == 2 {
            self.0.enclosure
        } else {
            self.0.kind
        };
        if stage.is_backend() || TypeId::of::<P>() != kind {
            return Err(Error::Representation.into());
        }
        crate::private::Buffer::allocate_solve(self.grant(stage, iteration)?)
    }
    /// Construct fully initialized caller-owned verifier storage under its exact planned meaning.
    pub(crate) fn zeros<P: Scalar + 'static>(
        &self,
        stage: Stage,
        iteration: usize,
    ) -> Result<crate::private::Buffer<P>, OperationError> {
        let mut values = self.buffer(stage, Some(iteration))?;
        let count = self
            .0
            .specifications
            .iter()
            .find(|s| s.stage == stage && s.iteration == Some(iteration))
            .ok_or(Error::UnplannedMeaning)?
            .extent
            .entries();
        for _ in 0..count {
            values.push(P::ZERO)?;
        }
        Ok(values)
    }
    /// Backend scalar and object representation are fixed before its allocator callback can run.
    pub(crate) fn backend<T: 'static>(
        &self,
        stage: Stage,
        iteration: usize,
        allocate: impl FnOnce() -> Result<T, OperationError>,
    ) -> Result<Backend<T>, OperationError> {
        let kind = if self.0.precision == nrz_scalar::Precision::F32 {
            TypeId::of::<faer::Mat<f32>>()
        } else {
            TypeId::of::<faer::Mat<f64>>()
        };
        if !stage.is_backend() || TypeId::of::<T>() != kind {
            return Err(Error::Representation.into());
        }
        let permit = self.grant(stage, Some(iteration))?;
        let value = allocate()?;
        super::backend::validate(&value, self.0.specifications[permit.slot].extent)?;
        permit.plan.specifications[permit.slot]
            .state
            .set(State::Live);
        Ok(Backend { value, permit })
    }
    /// Certificates form a contiguous prefix; skipping or replaying a certificate refuses.
    pub(crate) fn certified(&self, iteration: usize) -> Result<(), OperationError> {
        if iteration != self.0.completed.get() {
            return Err(Error::IncompleteCoverage.into());
        }
        self.0.completed.set(
            iteration
                .checked_add(1)
                .ok_or(OperationError::ResourceLimit)?,
        );
        Ok(())
    }
}

/// Both byte policies govern the complete new metadata before entering any numerical allocator.
fn check_budget(bytes: usize, budget: &Budget) -> Result<(), OperationError> {
    use nrz_bindings::readings::algebra::{Location, limits::BudgetKind};
    let bytes = u64::try_from(bytes).map_err(|_| OperationError::ResourceLimit)?;
    let at = Location::new("general solve allocation inventory");
    budget.check(BudgetKind::StagedPayloadBytes, bytes, at)?;
    budget.check(BudgetKind::OutputBytes, bytes, at)?;
    Ok(())
}
/// Charge aligned plan, source selection copies and the maximum historical receipt exactly once.
fn metadata(count: usize, sources: usize, maps: usize) -> Result<usize, OperationError> {
    let fixed = size_of::<RcLayout>() + size_of::<super::SolveInventory>() + 2 * size_of::<usize>();
    count
        .checked_mul(size_of::<Specification>() + size_of::<super::SolveAllocation>())
        .and_then(|bytes| {
            sources
                .checked_mul(size_of::<(Dependency, BindingSymbols)>())?
                .checked_add(bytes)
        })
        .and_then(|bytes| bytes.checked_add(fixed)?.checked_add(maps))
        .ok_or(OperationError::ResourceLimit)
}

/// A borrowed finite attempt cannot change the immutable schedule or duplicate a permit.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Attempt<'a> {
    /// Original finite allocation schedule borrowed for this attempt.
    pub(crate) run: &'a Run,
    /// Initial or correction position in the independently certified prefix.
    pub(crate) index: usize,
}
impl Attempt<'_> {
    /// Draw the exact stage for this certificate attempt.
    pub(crate) fn buffer<P: 'static>(
        self,
        stage: Stage,
    ) -> Result<crate::private::Buffer<P>, OperationError> {
        self.run.buffer(stage, Some(self.index))
    }
    /// Initialize verifier storage while preserving its permit.
    pub(crate) fn zeros<P: Scalar + 'static>(
        self,
        stage: Stage,
    ) -> Result<crate::private::Buffer<P>, OperationError> {
        self.run.zeros(stage, self.index)
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
