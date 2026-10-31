// SPDX-License-Identifier: AGPL-3.0-only
//! Complete output declarations consume the caller ledger before ordinary atomic publication.

use super::{Quantity, QuantityContext, QuantityError, allocation, charge};
use crate::{Binding, CheckedOperation, Heap, ValueBinding};
use nrz_bindings::{AdmissionError, BindingSymbols, Budget, Descriptor, SymbolSchema};
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
        let binding = Binding::from_symbols(operation, inputs, self.execution);
        self.finish(heap, binding, budget)
    }

    /// Keep a newly frozen schema private until the complete numerical transaction succeeds.
    pub(super) fn finish<P: Scalar + 'static, Q: Scalar + 'static>(
        &self,
        heap: &mut Heap,
        binding: Binding<P, Q>,
        budget: &mut Budget,
    ) -> Result<Quantity<Q>, QuantityError<P>> {
        let mut retained = self
            .schema
            .try_lock()
            .map_err(|_| crate::HeapError::Capacity)?;
        let (symbols, prepared) = Self::outputs(&binding.operation, retained.as_ref(), budget)?;
        let result = Self::publish(heap, binding, symbols, budget)?;
        *retained = prepared;
        drop(retained);
        Ok(result)
    }

    /// Declare all destination channels while retaining only immutable environment authority.
    fn outputs(
        operation: &CheckedOperation,
        existing: Option<&SymbolSchema>,
        budget: &mut Budget,
    ) -> Result<(Vec<BindingSymbols>, Option<SymbolSchema>), crate::OperationError> {
        let mut symbols = Vec::new();
        symbols
            .try_reserve_exact(operation.outputs().len())
            .map_err(|_| crate::HeapError::Capacity)?;
        let mut prepared = None;
        for output in operation.outputs() {
            let extent = output.shape().extent();
            let shape = [
                u32::try_from(extent.rows()).map_err(|_| crate::HeapError::ExtentOverflow)?,
                u32::try_from(extent.columns()).map_err(|_| crate::HeapError::ExtentOverflow)?,
            ];
            let schema = Self::output_schema(
                prepared.as_ref().or(existing),
                output.descriptor(),
                shape,
                budget,
            )?;
            symbols.push(schema.declare(budget).map_err(crate::HeapError::from)?);
            prepared = Some(schema);
        }
        Ok((symbols, prepared))
    }

    /// A bounded single-slot lookup compares the complete exact descriptor, never its spelling.
    fn output_schema(
        existing: Option<&SymbolSchema>,
        descriptor: &Descriptor,
        shape: [u32; 2],
        budget: &mut Budget,
    ) -> Result<SymbolSchema, crate::HeapError> {
        charge(budget, 1).map_err(AdmissionError::from)?;
        if let Some(existing) = existing
            && existing.matches(descriptor, shape, [true, true], budget)?
        {
            return Ok(existing.clone());
        }
        Ok(SymbolSchema::freeze(
            descriptor,
            shape,
            [true, true],
            budget,
        )?)
    }

    /// An independent import has the same schema publication boundary as an evaluated result.
    pub(super) fn with_output_symbols<P: Scalar, T>(
        &self,
        descriptor: &Descriptor,
        shape: [u32; 2],
        budget: &mut Budget,
        consume: impl FnOnce(&BindingSymbols, &mut Budget) -> Result<T, QuantityError<P>>,
    ) -> Result<T, QuantityError<P>> {
        let mut retained = self
            .schema
            .try_lock()
            .map_err(|_| crate::HeapError::Capacity)?;
        let schema = Self::output_schema(retained.as_ref(), descriptor, shape, budget)?;
        let symbols = schema.declare(budget).map_err(crate::HeapError::from)?;
        let result = consume(&symbols, budget)?;
        *retained = Some(schema);
        drop(retained);
        Ok(result)
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
