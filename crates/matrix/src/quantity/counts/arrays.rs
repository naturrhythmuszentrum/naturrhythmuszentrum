// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable shaped counts share exact element nodes outside the floating arenas.

use super::{CountError, CountQuantity, charge};
use crate::Extent;
use nrz_bindings::readings::algebra::{Location, limits::BudgetKind};
use nrz_bindings::{Budget, CountDescriptor, Descriptor};
use std::rc::Rc;

/// A homogeneous rectangular array of exact counts in logical column order.
#[derive(Clone, Debug)]
pub struct CountArray(Rc<Data>);

/// One complete array owner with shared exact elements and explicit empty-array meaning.
#[derive(Debug)]
struct Data {
    /// Original checked counted-kind authority, also retained when there are no elements.
    source: CountDescriptor,
    /// Structural rows and columns, never a physical count.
    extent: Extent,
    /// Ordered immutable counts retain their own original evidence and operation histories.
    elements: Vec<CountQuantity>,
    /// Conservative logical envelope, including all element lineages and source authority.
    bytes: u64,
}

/// A closed operation inventory delegates actual arithmetic to the existing scalar count owner.
#[derive(Clone, Copy)]
enum Operation<'a> {
    /// Same-shape exact addition.
    Add(&'a CountArray),
    /// Same-shape exact subtraction.
    Subtract(&'a CountArray),
    /// Explicit signed integer multiplication.
    Scale(i128),
    /// Exact integer partition with an independently checked nonzero divisor.
    Quotient(i128),
}

impl CountArray {
    /// Capture exact elements with an explicit checked source defining the homogeneous kind.
    ///
    /// The source supplies counted meaning; its scalar value is not an array element.
    /// # Errors
    /// Refuses length/extent disagreement, incompatible element meanings and caller resources.
    pub fn capture(
        source: CountDescriptor,
        extent: Extent,
        elements: &[CountQuantity],
        budget: &mut Budget,
    ) -> Result<Self, CountError> {
        charge(budget)?;
        if extent.entries() != elements.len() {
            return Err(CountError::Shape);
        }
        let mut bytes = base(&source, extent, budget)?;
        let mut prepared = storage(extent)?;
        for element in elements {
            charge(budget)?;
            source.descriptor().compatible_kind(element.descriptor())?;
            bytes = retain(bytes, element, budget)?;
            prepared.push(element.clone());
        }
        Ok(Self(Rc::new(Data {
            source,
            extent,
            elements: prepared,
            bytes,
        })))
    }

    /// The checked counted kind shared by every element, including an empty array's kind.
    #[must_use]
    pub fn descriptor(&self) -> &Descriptor {
        self.0.source.descriptor()
    }

    /// Original checked counted-kind authority without copying its declaration or proof.
    #[must_use]
    pub fn source(&self) -> &CountDescriptor {
        &self.0.source
    }

    /// Checked shape metadata in logical column order.
    #[must_use]
    pub fn extent(&self) -> Extent {
        self.0.extent
    }

    /// Exact immutable elements, with each original source and operation history retained.
    #[must_use]
    pub fn elements(&self) -> &[CountQuantity] {
        &self.0.elements
    }

    /// Access one exact element by row and column without numerical materialization.
    /// # Errors
    /// Refuses coordinates outside the checked extent.
    pub fn get(&self, row: usize, column: usize) -> Result<&CountQuantity, CountError> {
        let index = self
            .extent()
            .index(row, column)
            .map_err(|_| CountError::Shape)?;
        self.0.elements.get(index).ok_or(CountError::Shape)
    }

    /// Conservative logical storage for the owner, handle vector and complete exact lineages.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }

    /// Add compatible count arrays elementwise and publish only the complete result.
    /// # Errors
    /// Refuses shape/kind mismatch or any element arithmetic, domain, capacity or resource failure.
    pub fn add(
        &self,
        right: &Self,
        capacity: u128,
        budget: &mut Budget,
    ) -> Result<Self, CountError> {
        self.apply(Operation::Add(right), capacity, budget)
    }

    /// Subtract compatible count arrays within their actual declared result domains.
    /// # Errors
    /// Refuses shape/kind mismatch or any element arithmetic, domain, capacity or resource failure.
    pub fn subtract(
        &self,
        right: &Self,
        capacity: u128,
        budget: &mut Budget,
    ) -> Result<Self, CountError> {
        self.apply(Operation::Subtract(right), capacity, budget)
    }

    /// Scale every exact element by the explicit signed integer factor.
    /// # Errors
    /// Refuses any element overflow, domain, capacity or resource failure atomically.
    pub fn scale(
        &self,
        factor: i128,
        capacity: u128,
        budget: &mut Budget,
    ) -> Result<Self, CountError> {
        self.apply(Operation::Scale(factor), capacity, budget)
    }

    /// Partition every element exactly; even an empty array refuses a zero divisor.
    /// # Errors
    /// Refuses zero division, fractional results or any element domain, capacity or resource failure.
    pub fn quotient(
        &self,
        divisor: i128,
        capacity: u128,
        budget: &mut Budget,
    ) -> Result<Self, CountError> {
        self.apply(Operation::Quotient(divisor), capacity, budget)
    }

    /// Retain each prepared prefix against the caller's existing staged storage policy.
    fn apply(
        &self,
        operation: Operation<'_>,
        capacity: u128,
        budget: &mut Budget,
    ) -> Result<Self, CountError> {
        charge(budget)?;
        match operation {
            Operation::Add(right) | Operation::Subtract(right) => {
                if self.extent() != right.extent() {
                    return Err(CountError::Shape);
                }
                self.descriptor().compatible_kind(right.descriptor())?;
            }
            Operation::Quotient(0) => return Err(CountError::ZeroDivisor),
            Operation::Scale(_) | Operation::Quotient(_) => {}
        }
        let mut bytes = base(self.source(), self.extent(), budget)?;
        let mut elements = storage(self.extent())?;
        for (index, element) in self.elements().iter().enumerate() {
            charge(budget)?;
            let result = budget.with_retained_payload(
                bytes,
                Location::new("exact count array"),
                |budget| operation.element(index, element, capacity, budget),
            )?;
            bytes = retain(bytes, &result, budget)?;
            elements.push(result);
        }
        Ok(Self(Rc::new(Data {
            source: self.0.source.clone(),
            extent: self.extent(),
            elements,
            bytes,
        })))
    }
}

impl Operation<'_> {
    /// The existing scalar operation checks the actual result domain before capacity.
    fn element(
        self,
        index: usize,
        left: &CountQuantity,
        capacity: u128,
        budget: &mut Budget,
    ) -> Result<CountQuantity, CountError> {
        match self {
            Self::Add(right) => left.add(&right.elements()[index], capacity, budget),
            Self::Subtract(right) => left.subtract(&right.elements()[index], capacity, budget),
            Self::Scale(factor) => left.scale(factor, capacity, budget),
            Self::Quotient(divisor) => left.quotient(divisor, capacity, budget),
        }
    }
}

/// Bound complete fixed metadata and exact-element handles before allocating their vector.
fn base(source: &CountDescriptor, extent: Extent, budget: &Budget) -> Result<u64, CountError> {
    let handles = extent
        .entries()
        .checked_mul(size_of::<CountQuantity>())
        .ok_or(CountError::Capacity)?;
    let own = (size_of::<Data>() + 2 * size_of::<usize>())
        .checked_add(handles)
        .ok_or(CountError::Capacity)?;
    let bytes = source
        .retained_payload_bytes()
        .checked_add(u64::try_from(own).map_err(|_| CountError::Capacity)?)
        .ok_or(CountError::Capacity)?;
    check(bytes, budget)?;
    Ok(bytes)
}

/// Allocate only a private exact-element handle vector under the checked base envelope.
fn storage(extent: Extent) -> Result<Vec<CountQuantity>, CountError> {
    let mut elements = Vec::new();
    elements
        .try_reserve_exact(extent.entries())
        .map_err(|_| CountError::Capacity)?;
    Ok(elements)
}

/// Include every shared parent in full before extending a private result prefix.
fn retain(bytes: u64, element: &CountQuantity, budget: &Budget) -> Result<u64, CountError> {
    let bytes = bytes
        .checked_add(element.retained_payload_bytes())
        .ok_or(CountError::Capacity)?;
    check(bytes, budget)?;
    Ok(bytes)
}

/// Existing policies bound both complete publication and temporary retained ownership.
fn check(bytes: u64, budget: &Budget) -> Result<(), CountError> {
    for kind in [BudgetKind::StagedPayloadBytes, BudgetKind::OutputBytes] {
        budget.check(kind, bytes, Location::new("exact count array"))?;
    }
    Ok(())
}
