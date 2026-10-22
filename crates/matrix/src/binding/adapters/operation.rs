// SPDX-License-Identifier: AGPL-3.0-only
//! Kernel input and planned result records match closed exact operation identities.

use super::{EntryTicket, ReadingAdapter, ReadingAdapterError, capacity, entry};
use nrz_bindings::{
    Budget,
    readings::{AffineOperation, OperatorApplication},
};

/// Read-only affine kernel obligation, with exact represented operand and result identities.
#[derive(Clone, Debug)]
pub struct AffineAdapter<T> {
    /// Checked complete-family affine operation equation.
    operation: AffineOperation,
    /// Exact input tickets and representation witness.
    input: ReadingAdapter<T>,
    /// Exact planned output tickets and representation witness.
    output: ReadingAdapter<T>,
}

impl<T> AffineAdapter<T> {
    /// Match the exact original operand and result before a numerical owner prepares execution.
    /// # Errors
    /// Refuses another physical reading even if its shape, names or rounded values agree.
    pub fn new(
        operation: AffineOperation,
        input: ReadingAdapter<T>,
        output: ReadingAdapter<T>,
        budget: &mut Budget,
    ) -> Result<Self, ReadingAdapterError> {
        capacity::<Self>(1, budget)?;
        super::reading_work(input.reading(), budget)?;
        super::reading_work(operation.input(), budget)?;
        super::reading_work(output.reading(), budget)?;
        super::reading_work(operation.output(), budget)?;
        if operation.input() != input.reading() || operation.output() != output.reading() {
            return Err(ReadingAdapterError::Target);
        }
        Ok(Self {
            operation,
            input,
            output,
        })
    }
    /// Exact mathematical law and its full-family result equation.
    #[must_use]
    pub const fn operation(&self) -> &AffineOperation {
        &self.operation
    }
    /// Operand tickets whose live versions and companions the numerical owner must resolve.
    #[must_use]
    pub const fn input(&self) -> &ReadingAdapter<T> {
        &self.input
    }
    /// Planned result tickets, which remain unavailable until separate atomic publication.
    #[must_use]
    pub const fn output(&self) -> &ReadingAdapter<T> {
        &self.output
    }
}

/// Read-only ordered product obligation with formal coefficients and exact result definition.
#[derive(Clone, Debug)]
pub struct OperatorAdapter<T> {
    /// Native ordered matrix product with physical contraction and result axes.
    operation: OperatorApplication,
    /// Every coefficient entry, including typed structural zeros.
    coefficients: Vec<EntryTicket<T>>,
    /// Exact input reading and its representation.
    input: ReadingAdapter<T>,
    /// Complete planned result positions in the operation's logical column order.
    result: Vec<EntryTicket<T>>,
}

impl<T: Eq> OperatorAdapter<T> {
    /// Match coefficient, input and result inventories to the exact ordered product.
    /// # Errors
    /// Refuses substituted inputs, missing entries or duplicate storage positions.
    pub fn new(
        operation: OperatorApplication,
        coefficients: Vec<EntryTicket<T>>,
        input: ReadingAdapter<T>,
        result: Vec<EntryTicket<T>>,
        budget: &mut Budget,
    ) -> Result<Self, ReadingAdapterError> {
        capacity::<Self>(1, budget)?;
        super::reading_work(input.reading(), budget)?;
        super::reading_work(operation.input().reading(), budget)?;
        if operation.input().reading() != input.reading() {
            return Err(ReadingAdapterError::Target);
        }
        entry::check(&coefficients, operation.operator().entries().len(), budget)?;
        entry::check(&result, operation.entries().len(), budget)?;
        Ok(Self {
            operation,
            coefficients,
            input,
            result,
        })
    }
}

impl<T> OperatorAdapter<T> {
    /// Exact native result definition, axes, per-entry meanings and original operand order.
    #[must_use]
    pub const fn operation(&self) -> &OperatorApplication {
        &self.operation
    }
    /// Exact coefficient positions, whose formal units come from the declared operator.
    #[must_use]
    pub fn coefficients(&self) -> &[EntryTicket<T>] {
        &self.coefficients
    }
    /// Exact represented input and every selected original copy.
    #[must_use]
    pub const fn input(&self) -> &ReadingAdapter<T> {
        &self.input
    }
    /// Planned numerical result coordinates; these tickets grant no live read or write authority.
    #[must_use]
    pub fn result(&self) -> &[EntryTicket<T>] {
        &self.result
    }
}
