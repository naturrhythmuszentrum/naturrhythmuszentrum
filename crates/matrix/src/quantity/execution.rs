// SPDX-License-Identifier: AGPL-3.0-only
//! Complete output declarations consume the caller ledger before ordinary atomic publication.

use super::{Quantity, QuantityContext, QuantityError, allocation, charge};
use crate::{Binding, CheckedOperation, Heap, ValueBinding};
use nrz_bindings::{BindingSymbols, Budget};
use nrz_scalar::Scalar;
use std::{marker::PhantomData, rc::Rc};

impl QuantityContext {
    /// Use symbol-selected operands and the existing same-format transaction.
    pub(super) fn execute<P: Scalar + 'static>(
        &self,
        heap: &mut Heap,
        operation: CheckedOperation,
        inputs: Vec<ValueBinding<P>>,
        budget: &mut Budget,
    ) -> Result<Quantity<P>, QuantityError<P>> {
        let symbols = Self::outputs(&operation, budget)?;
        let binding = Binding::from_symbols(operation, inputs, self.execution);
        Self::publish(heap, binding, symbols, budget)
    }

    /// Declare all three destination channels on the original caller's monotone ledger.
    pub(super) fn outputs(
        operation: &CheckedOperation,
        budget: &mut Budget,
    ) -> Result<Vec<BindingSymbols>, crate::OperationError> {
        let mut symbols = Vec::new();
        symbols
            .try_reserve_exact(operation.outputs().len())
            .map_err(|_| crate::HeapError::Capacity)?;
        for output in operation.outputs() {
            let extent = output.shape().extent();
            let shape = [
                u32::try_from(extent.rows()).map_err(|_| crate::HeapError::ExtentOverflow)?,
                u32::try_from(extent.columns()).map_err(|_| crate::HeapError::ExtentOverflow)?,
            ];
            symbols.push(
                BindingSymbols::declare(output.descriptor(), shape, [true, true], budget)
                    .map_err(crate::HeapError::from)?,
            );
        }
        Ok(symbols)
    }

    /// Prepare the returned owner while outputs are private, then publish with no fallible tail.
    pub(super) fn publish<P: Scalar + 'static, Q: Scalar + 'static>(
        heap: &mut Heap,
        binding: Binding<P, Q>,
        symbols: Vec<BindingSymbols>,
        budget: &mut Budget,
    ) -> Result<Quantity<Q>, QuantityError<P>> {
        charge(budget, 1)?;
        allocation::<Q>(budget)?;
        let bound = binding.validate_quantity(heap, budget, symbols)?;
        let result = bound
            .result_bindings()
            .first()
            .ok_or(crate::OperationError::CandidateContract)?
            .clone();
        let result = Quantity {
            binding: Rc::new(result),
            counts: None,
            role: PhantomData,
        };
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        bound.publish(candidate)?;
        Ok(result)
    }
}
