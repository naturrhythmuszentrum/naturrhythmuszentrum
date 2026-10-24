// SPDX-License-Identifier: AGPL-3.0-only
//! A reused coefficient retains the original publication, source guards and live symbol binding.

use super::{Budget, OperationError, ValueBinding};
use crate::{Dependency, Heap, HeapError, PublishedBundle, RecordId};
use nrz_bindings::readings::PositiveReciprocalMap;
use nrz_scalar::{ArithmeticContext, Precision, Scalar};
use std::rc::Rc;

/// One complete original coefficient allocation, shared without copying its value or companions.
#[derive(Debug)]
pub(super) struct Materialization<P: Scalar> {
    /// Newly retained cache metadata and logical value/companion payload bytes.
    pub(super) bytes: usize,
    /// Exact published output and all numerical tickets.
    pub(super) binding: ValueBinding<P>,
    /// Original value, deterministic total and numerical-error handles.
    pub(super) bundle: PublishedBundle<P>,
    /// Complete original map, canonical key and retained guards.
    pub(super) definition: PositiveReciprocalMap,
    /// Original operation and every captured source/companion version.
    pub(super) record: RecordId,
    /// Sealed arena format retained in the complete numerical key.
    pub(super) precision: Precision,
    /// Scalar enclosure theorem used for this materialization.
    pub(super) revision: u32,
}

/// An invariant coefficient remains readable only under its original complete live key.
#[derive(Clone, Debug)]
pub struct InvariantValue<P: Scalar>(pub(super) Rc<Materialization<P>>);

impl<P: Scalar + 'static> InvariantValue<P> {
    /// Resolve the original symbol binding for a later consumer without another materialization.
    /// # Errors
    /// Refuses changed input, companion, result or arithmetic versions and foreign owners.
    pub fn binding<'a>(
        &'a self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<&'a ValueBinding<P>, HeapError> {
        if context.revision() != self.0.revision {
            return Err(HeapError::StaleEvidence);
        }
        heap.operation_record(self.0.record)?;
        heap.value_by_symbol(self.0.binding.symbol(), &self.0.binding)?;
        Ok(&self.0.binding)
    }
    /// Original materialized value and radius regions, after validating their complete closure.
    /// # Errors
    /// Refuses any stale source, companion, result or arithmetic version.
    pub fn bundle(
        &self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<PublishedBundle<P>, HeapError> {
        self.binding(heap, context)?;
        Ok(self.0.bundle)
    }
    /// Original complete dependency keys, including selected parameter versions and source lineage.
    /// # Errors
    /// Refuses foreign or stale operation evidence.
    pub fn sources<'a>(&self, heap: &'a Heap) -> Result<&'a [Dependency], HeapError> {
        Ok(heap.operation_record(self.0.record)?.inputs())
    }
    /// Retained cache entry, reference-counted value, tickets and logical floating payload bytes.
    ///
    /// Shared exact declarations, existing heap records, arena padding and allocator overhead
    /// remain under their owners' accounting rather than being charged again here.
    #[must_use]
    pub fn owned_bytes(&self) -> usize {
        self.0.bytes
    }
    /// Original exact map and guards remain inspectable as historical definitions.
    #[must_use]
    pub fn definition(&self) -> &PositiveReciprocalMap {
        &self.0.definition
    }
    /// Original publication identity, shared by every successful cache hit.
    #[must_use]
    pub fn record(&self) -> RecordId {
        self.0.record
    }
    /// Sealed numerical format in this key.
    #[must_use]
    pub fn precision(&self) -> Precision {
        self.0.precision
    }
    /// Retained scalar theorem revision in this key.
    #[must_use]
    pub fn arithmetic_revision(&self) -> u32 {
        self.0.revision
    }
}

/// One lookup's resource evidence distinguishes reuse from the original numerical evaluation.
#[derive(Clone, Debug)]
pub struct InvariantUse<P: Scalar> {
    /// Original shared materialization and its complete live dependency closure.
    value: InvariantValue<P>,
    /// True only when this call read an already published matching entry.
    reused: bool,
    /// Actual lookup and admitted numerical Work added to the caller ledger.
    work: u64,
    /// Numerical phase allowance avoided; lookup Work is reported separately above.
    avoided: u64,
}

impl<P: Scalar> InvariantUse<P> {
    /// Original coefficient, reusable by every later grid consumer with the same fixed declarations.
    #[must_use]
    pub const fn value(&self) -> &InvariantValue<P> {
        &self.value
    }
    /// Whether this request read an earlier materialization.
    #[must_use]
    pub const fn reused(&self) -> bool {
        self.reused
    }
    /// Work charged for this complete lookup or fresh materialization.
    #[must_use]
    pub const fn work(&self) -> u64 {
        self.work
    }
    /// Numerical evaluation allowance avoided, excluding this call's lookup overhead.
    #[must_use]
    pub const fn avoided_evaluation_work(&self) -> u64 {
        self.avoided
    }
}

/// The cumulative ledger is never reset when a cache hit avoids numerical execution.
pub(super) fn usage<P: Scalar>(
    value: InvariantValue<P>,
    reused: bool,
    start: u64,
    budget: &Budget,
    avoided: u64,
) -> Result<InvariantUse<P>, OperationError> {
    let work = budget
        .usage()
        .work
        .checked_sub(start)
        .ok_or(OperationError::ResourceLimit)?;
    Ok(InvariantUse {
        value,
        reused,
        work,
        avoided,
    })
}
