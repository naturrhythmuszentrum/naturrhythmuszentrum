// SPDX-License-Identifier: AGPL-3.0-only
//! Typed requests precede every observed statistical vector and opaque contraction rectangle.

use super::{
    Channel, Role,
    plan::{Permit, Plan, State},
};
use crate::{OperationError, PrivateAllocationError as Error, statistics::capture::InputBinding};
use nrz_scalar::{Enclosure, Finite, Scalar};
use std::{any::TypeId, cell::Cell, rc::Rc};

/// Shared immutable original definitions; permits remain non-cloneable numerical owners.
#[derive(Clone, Debug)]
pub(crate) struct Run(pub(super) Rc<Plan>);
impl Run {
    /// Complete original selections and the finite closed law precede the first float allocation.
    pub(crate) fn new<Q: Scalar + 'static>(
        bindings: Vec<InputBinding>,
        operation: Option<&crate::CheckedOperation>,
        trace: &crate::execution::Trace,
        budget: &nrz_bindings::Budget,
    ) -> Result<Self, OperationError> {
        let original = bindings.first().ok_or(Error::IncompleteCoverage)?;
        let mut columns = Vec::new();
        for input in &bindings {
            if input.lineage.model.id != original.lineage.model.id {
                return Err(crate::StatisticsError::MissingJointModel.into());
            }
            columns.extend_from_slice(&input.lineage.columns);
        }
        columns.sort_unstable();
        columns.dedup();
        let count = super::schedule::count(&bindings, operation)?;
        let mut specifications = crate::binding::buffer(count)?;
        super::schedule::capture(&bindings, operation, &mut specifications)?;
        if let Some(operation) = operation {
            super::schedule::evaluate(
                &bindings,
                operation,
                columns.len(),
                Q::PRECISION,
                &mut specifications,
            )?;
        } else {
            super::schedule::cross(&bindings, columns.len(), Q::PRECISION, &mut specifications)?;
        }
        if specifications.len() != count {
            return Err(Error::IncompleteCoverage.into());
        }
        let mut plan = Plan {
            bindings,
            operation: operation.cloned(),
            columns,
            specifications,
            trace: trace.clone(),
            token: crate::heap::identity::identity()?,
            candidate: Cell::new(None),
            live: Cell::new(0),
            peak: Cell::new(0),
            metadata: 0,
            backend: Cell::new(0),
            assurance: Cell::new(crate::MomentAssurance::AffineMoments),
        };
        plan.metadata = super::metadata::bytes(&plan)?;
        super::metadata::check(plan.metadata, budget)?;
        Ok(Self(Rc::new(plan)))
    }
    /// Original immutable selected operand; no numerical payload supplies its model or law.
    pub(crate) fn binding(&self, index: usize) -> Result<&InputBinding, OperationError> {
        self.0
            .bindings
            .get(index)
            .ok_or_else(|| Error::UnplannedMeaning.into())
    }
    /// Borrow the complete original common source order.
    pub(crate) fn columns(&self) -> &[usize] {
        &self.0.columns
    }
    /// A role can be absent only when the original finite plan excludes it.
    pub(crate) fn contains(&self, role: Role, index: usize) -> bool {
        self.0
            .specifications
            .iter()
            .any(|entry| entry.role == role && entry.index == index)
    }
    /// Select the exact logical rectangle before entering any allocator.
    pub(crate) fn extent(
        &self,
        role: Role,
        index: usize,
        channel: Channel,
    ) -> Result<crate::Extent, OperationError> {
        Ok(self.0.specifications[self.slot(role, index, channel)?].extent)
    }
    /// A finite role identifies one request regardless of numerical addresses or equal dimensions.
    pub(super) fn slot(
        &self,
        role: Role,
        index: usize,
        channel: Channel,
    ) -> Result<usize, OperationError> {
        self.0
            .specifications
            .iter()
            .position(|entry| {
                entry.role == role && entry.index == index && entry.channel == channel
            })
            .ok_or_else(|| Error::UnplannedMeaning.into())
    }
    /// A request is consumed once before allocation; it cannot be reissued after owner destruction.
    pub(super) fn grant(&self, slot: usize) -> Result<Permit, OperationError> {
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
    /// Check the scalar or enclosure representation before allocating the planned vector.
    pub(crate) fn buffer<T: 'static>(
        &self,
        role: Role,
        index: usize,
        channel: Channel,
    ) -> Result<crate::private::Buffer<T>, OperationError> {
        let slot = self.slot(role, index, channel)?;
        let entry = &self.0.specifications[slot];
        if channel.backend() || TypeId::of::<T>() != representation(entry.precision, channel) {
            return Err(Error::Representation.into());
        }
        crate::private::Buffer::allocate_statistics(self.grant(slot)?)
    }
    /// Both matrix channels draw independent typed permits against the same exact logical role.
    pub(crate) fn matrix<P: Scalar + 'static>(
        &self,
        role: Role,
        index: usize,
    ) -> Result<crate::statistics::Matrix<P>, OperationError> {
        let extent = self.extent(role, index, Channel::Centers)?;
        let mut centers = self.buffer(role, index, Channel::Centers)?;
        let mut truth = self.buffer(role, index, Channel::Truth)?;
        for _ in 0..extent.entries() {
            centers.push(P::ZERO)?;
            truth.push(Enclosure::singleton(Finite::<P>::ZERO))?;
        }
        Ok(crate::statistics::Matrix {
            extent,
            centers,
            truth,
        })
    }
    /// Match a caller's independently checked rectangle before drawing either numerical permit.
    pub(crate) fn matrix_at<P: Scalar + 'static>(
        &self,
        role: Role,
        index: usize,
        extent: crate::Extent,
    ) -> Result<crate::statistics::Matrix<P>, OperationError> {
        if self.extent(role, index, Channel::Centers)? != extent {
            return Err(Error::ForeignInventory.into());
        }
        self.matrix(role, index)
    }
    /// Borrow an exact role without drawing or duplicating an allocation permit.
    pub(crate) const fn request(&self, role: Role, index: usize) -> Request<'_> {
        Request {
            run: self,
            role,
            index,
        }
    }
    /// Associate this successful evaluation with the exact transaction candidate token.
    pub(crate) fn associate(&self, token: u64) -> Result<(), OperationError> {
        if self.0.candidate.get().is_some() {
            return Err(Error::DuplicateTicket.into());
        }
        self.0.candidate.set(Some(token));
        Ok(())
    }
    /// Retain the actual law established by the original statistical classification checks.
    pub(crate) fn law(&self, assurance: crate::MomentAssurance) {
        self.0.assurance.set(assurance);
    }
    /// The unchanged admitted work defines a named reservation, never backend allocator measurements.
    pub(crate) fn backend_allowance(
        &self,
        work: usize,
        width: usize,
    ) -> Result<(), OperationError> {
        self.0.backend.set(
            work.checked_mul(width)
                .ok_or(OperationError::ResourceLimit)?,
        );
        Ok(())
    }
}
/// Closed primitive types distinguish candidate scalars from two-endpoint enclosures.
fn representation(precision: nrz_scalar::Precision, channel: Channel) -> TypeId {
    match (precision, channel == Channel::Truth) {
        (nrz_scalar::Precision::F32, false) => TypeId::of::<f32>(),
        (nrz_scalar::Precision::F32, true) => TypeId::of::<Enclosure<f32>>(),
        (nrz_scalar::Precision::F64, false) => TypeId::of::<f64>(),
        (nrz_scalar::Precision::F64, true) => TypeId::of::<Enclosure<f64>>(),
    }
}

/// Borrowed original matrix role; each channel still requires its own non-cloneable permit.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Request<'a> {
    /// Original finite schedule.
    pub(crate) run: &'a Run,
    /// Exact mathematical matrix role.
    pub(crate) role: Role,
    /// Original operand or output position.
    pub(crate) index: usize,
}
impl Request<'_> {
    /// Allocate one scalar or enclosure channel under its original declared role.
    pub(crate) fn buffer<T: 'static>(
        self,
        channel: Channel,
    ) -> Result<crate::private::Buffer<T>, OperationError> {
        self.run.buffer(self.role, self.index, channel)
    }
    /// Check the original backend rectangle before invoking its numerical allocator.
    pub(crate) fn backend<T: 'static>(
        self,
        channel: Channel,
        allocate: impl FnOnce() -> Result<T, OperationError>,
    ) -> Result<super::backend::Backend<T>, OperationError> {
        self.run.backend(self.role, self.index, channel, allocate)
    }
    /// The exact logical rectangle is available before any numerical allocation.
    pub(crate) fn extent(self, channel: Channel) -> Result<crate::Extent, OperationError> {
        self.run.extent(self.role, self.index, channel)
    }
}
