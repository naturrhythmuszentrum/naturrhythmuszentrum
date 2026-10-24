// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed reciprocal declarations share one materialization under their complete live key.

mod error;
mod key;
mod value;
pub use error::InvariantError;
pub use value::{InvariantUse, InvariantValue};

use super::{Binding, NativeRecipe, OperationError, ValueBinding};
use crate::{Dependency, Heap, HeapError, InputSnapshot, RecordId};
use nrz_bindings::{
    Budget,
    readings::algebra::{Location, limits::BudgetKind},
};
use nrz_scalar::{ArithmeticContext, Precision, Scalar};
use std::{any::Any, rc::Rc};

/// One checked lookup entry and one source-key comparison are charged as discrete operations.
const AT: Location = Location::new("invariant reciprocal materialization");

/// A bounded cache of fixed declaration versions, shared by later grid consumers.
///
/// The heap owns the published value regions. The cache retains their exact operation records
/// and rejects stale materializations; it never treats a changed source as an unchanged constant.
#[derive(Debug)]
pub struct InvariantCache {
    /// Numerical owner scope is part of every complete key.
    scope: u64,
    /// Explicit bound on distinct retained keys.
    maximum: usize,
    /// One original materialization per complete key, across both supported precisions.
    entries: Vec<Entry>,
}

/// Type-erased payloads can only be recovered after exact precision and key checks.
struct Entry {
    /// Original successful operation contains every source/companion version and exact definition.
    record: RecordId,
    /// Sealed storage and arithmetic format.
    precision: Precision,
    /// Admitted scalar theorem revision, independent of physical declaration identity.
    revision: u32,
    /// Shared original materialization; neither hits nor new consumers clone float arrays.
    payload: Rc<dyn Any>,
}

impl std::fmt::Debug for Entry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Entry")
            .field("record", &self.record)
            .field("precision", &self.precision)
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}

impl InvariantCache {
    /// Fix the owning heap and maximum number of distinct materialization keys.
    #[must_use]
    pub const fn new(heap: &Heap, maximum: usize) -> Self {
        Self {
            scope: heap.scope,
            maximum,
            entries: Vec::new(),
        }
    }
    /// Number of original materializations retained, including any subsequently stale key.
    #[must_use]
    pub const fn materializations(&self) -> usize {
        self.entries.len()
    }

    /// Materialize a checked reciprocal coefficient recipe once for its fixed declaration versions.
    ///
    /// Inputs are exact symbol-selected declarations, independent of later grid/RHS inputs.
    /// The key retains the original map and guards, frozen shape declarations, precision,
    /// arithmetic/import policy and all source/companion versions. Every hit revalidates them.
    /// # Errors
    /// Refuses other recipes, foreign owners, missing symbol selections, stale sources/outputs,
    /// nonpositive full input boxes, changed arithmetic assumptions and resource exhaustion.
    pub fn materialize<P: Scalar + 'static>(
        &mut self,
        heap: &mut Heap,
        request: Binding<P>,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<InvariantUse<P>, InvariantError<P>> {
        let start = budget.usage().work;
        let sources = self.capture(heap, &request, context, budget)?;
        if let Some(value) = self.find(heap, &request, &sources, context, budget)? {
            return Ok(value::usage(
                value,
                true,
                start,
                budget,
                key::work(&request.operation)?,
            )?);
        }
        let bytes = self.reserve::<P>(&request, budget)?;
        let work = key::work(&request.operation)?;
        budget.work(work, AT)?;
        let definition = request
            .operation
            .reciprocal_map()
            .ok_or(HeapError::MissingBinding)?
            .clone();
        let bound = request.validate(heap)?;
        let binding = bound
            .result_bindings()
            .first()
            .ok_or(HeapError::MissingBinding)?
            .clone();
        let bundle = *bound
            .reservations
            .bundles
            .first()
            .ok_or(HeapError::MissingBinding)?;
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let outcome = bound.publish(candidate)?;
        let value = Rc::new(value::Materialization {
            binding,
            bytes,
            bundle,
            definition,
            record: outcome.record(),
            precision: P::PRECISION,
            revision: context.revision(),
        });
        self.entries.push(Entry {
            record: outcome.record(),
            precision: P::PRECISION,
            revision: context.revision(),
            payload: value.clone(),
        });
        Ok(value::usage(
            InvariantValue(value),
            false,
            start,
            budget,
            0,
        )?)
    }

    /// Validate every selected source before attempting a cache lookup, including unused operands.
    fn capture<P: Scalar + 'static>(
        &self,
        heap: &Heap,
        request: &Binding<P>,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<Vec<Dependency>, OperationError> {
        if self.scope != heap.scope {
            return Err(HeapError::ForeignHandle.into());
        }
        key::work(&request.operation)?;
        let admitted = ArithmeticContext::new().map_err(crate::KernelError::from)?;
        if context.revision() != admitted.revision() {
            return Err(crate::KernelError::Contract.into());
        }
        let selected = request
            .symbol_inputs
            .as_ref()
            .ok_or(HeapError::MissingBinding)?;
        if selected.len() != request.inputs.len() {
            return Err(OperationError::InputCount);
        }
        let bytes = selected
            .len()
            .checked_mul(
                std::mem::size_of::<Dependency>() + std::mem::size_of::<InputSnapshot<P>>(),
            )
            .ok_or(OperationError::ResourceLimit)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            u64::try_from(bytes).map_err(|_| OperationError::ResourceLimit)?,
            AT,
        )?;
        let mut sources = super::buffer(selected.len())?;
        for binding in selected {
            budget.work(1, AT)?;
            heap.value_by_symbol(binding.symbol(), binding)?;
            let input = InputSnapshot::capture(heap, binding.handle())?;
            sources.push(Dependency::from_snapshot(&input));
        }
        Ok(sources)
    }

    /// Compare immutable keys first; only the matching entry can grant a live numerical value.
    fn find<P: Scalar + 'static>(
        &self,
        heap: &Heap,
        request: &Binding<P>,
        sources: &[Dependency],
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<Option<InvariantValue<P>>, OperationError> {
        for entry in &self.entries {
            budget.work(1, AT)?;
            if entry.precision != P::PRECISION || entry.revision != context.revision() {
                continue;
            }
            let record = heap.historical_record(entry.record)?;
            if !key::same_operation(record.operation(), &request.operation, budget)? {
                continue;
            }
            budget.work(
                u64::try_from(sources.len()).map_err(|_| OperationError::ResourceLimit)?,
                AT,
            )?;
            if sources.len() != record.inputs().len()
                || !sources
                    .iter()
                    .zip(record.inputs())
                    .all(|(left, right)| left.same_source(right))
            {
                continue;
            }
            let value = Rc::clone(&entry.payload)
                .downcast::<value::Materialization<P>>()
                .map_err(|_| HeapError::PrecisionMismatch)?;
            let value = InvariantValue(value);
            value.binding(heap, context)?;
            return Ok(Some(value));
        }
        Ok(None)
    }

    /// Reserve the cache slot and new logical output payload before any reciprocal allocation.
    fn reserve<P: Scalar>(
        &mut self,
        request: &Binding<P>,
        budget: &Budget,
    ) -> Result<usize, OperationError> {
        if self.entries.len() >= self.maximum {
            return Err(HeapError::Capacity.into());
        }
        let output = request
            .operation
            .outputs()
            .first()
            .ok_or(OperationError::InputCount)?;
        let entries = output.shape().extent().entries();
        let bytes = entries
            .checked_mul(3 * std::mem::size_of::<P>())
            .and_then(|count| {
                count.checked_add(
                    std::mem::size_of::<value::Materialization<P>>()
                        + std::mem::size_of::<Entry>()
                        + super::lifecycle::bounded_binding_payload_bytes()
                        + 2 * std::mem::size_of::<usize>(),
                )
            })
            .ok_or(OperationError::ResourceLimit)?;
        budget.check(
            BudgetKind::OutputBytes,
            u64::try_from(bytes).map_err(|_| OperationError::ResourceLimit)?,
            AT,
        )?;
        self.entries
            .try_reserve_exact(1)
            .map_err(|_| HeapError::Capacity)?;
        Ok(bytes)
    }
}
