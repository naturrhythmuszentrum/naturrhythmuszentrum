// SPDX-License-Identifier: AGPL-3.0-only
//! Prepare every graph record, edge, counter and allocation before the visible commit.

use super::limits::add;
use super::{Node, Provenance, ProvenanceId, ProvenanceUsage, ReverseEdge};
use crate::binding::buffer;
use crate::{
    CopyEvidence, Dependency, HeapError, OperationError, OperationRecord, OutputRecord, RecordId,
};
use std::cell::Cell;

/// Fully allocated immutable graph append, coupled to one atomic descriptor publication.
#[derive(Debug)]
pub(crate) struct PreparedGraph {
    /// The complete operation facts.
    record: OperationRecord,
    /// Topologically ordered new output nodes.
    nodes: Vec<Node>,
    /// Source-to-dependent edges for those nodes.
    reverse: Vec<ReverseEdge>,
    /// Separate initially current flags.
    stale: Vec<Cell<bool>>,
    /// Complete prospective lifetime accounting.
    usage: ProvenanceUsage,
}

impl PreparedGraph {
    /// Scoped record returned only after the same commit publishes all descriptors.
    pub(crate) const fn record_id(&self) -> RecordId {
        self.record.id
    }
}

impl Provenance {
    /// Assign future graph identities and reserve all append capacities before publication.
    pub(crate) fn prepare(
        &mut self,
        mut record: OperationRecord,
    ) -> Result<PreparedGraph, OperationError> {
        let count = record.outputs.len();
        if count != record.inputs.len() || count != record.evidence.len() {
            return Err(OperationError::CandidateContract);
        }
        self.preflight(&record)?;
        record.id.index = self.records.len();
        let mut nodes = buffer(count)?;
        let mut reverse = buffer(count)?;
        let mut stale = buffer(count)?;
        for (slot, (input, output)) in record.inputs.iter().zip(&mut record.outputs).enumerate() {
            let parent = self.find(input.stamp());
            let depth = parent.map_or(Ok(1), |id| {
                self.nodes
                    .get(id.index)
                    .ok_or(OperationError::CandidateContract)
                    .and_then(|node| add(node.depth, 1))
            })?;
            let identity = ProvenanceId {
                scope: record.id.scope,
                index: add(self.nodes.len(), slot)?,
            };
            output.provenance = identity;
            nodes.push(Node {
                record: record.id,
                slot,
                parent,
                depth,
            });
            reverse.push(ReverseEdge {
                source: input.stamp(),
                dependent: identity,
            });
            stale.push(Cell::new(false));
        }
        let usage = self.account(&record, &nodes)?;
        self.limits.check(usage)?;
        self.reserve(count)?;
        Ok(PreparedGraph {
            record,
            nodes,
            reverse,
            stale,
            usage,
        })
    }

    /// Bound parent searches and account metadata before constructing any pending nodes.
    fn preflight(&self, record: &OperationRecord) -> Result<(), OperationError> {
        let count = record.outputs.len();
        let searches = self
            .nodes
            .len()
            .checked_mul(count)
            .ok_or(OperationError::ProvenanceLimit)?;
        let required = add(self.usage.work, add(searches, count)?)?;
        if required > self.limits.work {
            return Err(OperationError::ProvenanceLimit);
        }
        let future_records = add(self.usage.records, add(count, 1)?)?;
        if future_records > self.limits.records {
            return Err(OperationError::ProvenanceLimit);
        }
        if self.records.iter().any(|old| old.token == record.token) {
            return Err(OperationError::ForeignCandidate);
        }
        Ok(())
    }

    /// Charge fixed graph objects and logical-map payloads; schema owners remain shared evidence.
    fn account(
        &self,
        record: &OperationRecord,
        nodes: &[Node],
    ) -> Result<ProvenanceUsage, OperationError> {
        let mut usage = self.usage;
        usage.records = add(usage.records, add(nodes.len(), 1)?)?;
        usage.bytes = add(usage.bytes, size_of::<OperationRecord>())?;
        for ((input, output), node) in record.inputs.iter().zip(&record.outputs).zip(nodes) {
            usage.depth = usage.depth.max(node.depth);
            usage.edges = add(usage.edges, 3 + usize::from(node.parent.is_some()))?;
            let fixed = size_of::<Node>()
                + size_of::<ReverseEdge>()
                + size_of::<Cell<bool>>()
                + size_of::<Dependency>()
                + size_of::<OutputRecord>()
                + size_of::<CopyEvidence>();
            let entries = add(
                input.layout().extent().entries(),
                output.dependency.layout().extent().entries(),
            )?;
            let maps = entries
                .checked_mul(size_of::<usize>())
                .ok_or(OperationError::ProvenanceLimit)?;
            usage.bytes = add(usage.bytes, add(fixed, maps)?)?;
            let comparisons = input
                .layout()
                .validation_work()
                .checked_mul(2)
                .ok_or(OperationError::ProvenanceLimit)?;
            usage.work = add(usage.work, add(3, comparisons)?)?;
        }
        Ok(usage)
    }

    /// Reserve all graph vectors; capacity growth alone is invisible and may be discarded.
    fn reserve(&mut self, count: usize) -> Result<(), HeapError> {
        self.records
            .try_reserve(1)
            .map_err(|_| HeapError::Capacity)?;
        self.nodes
            .try_reserve(count)
            .map_err(|_| HeapError::Capacity)?;
        self.reverse
            .try_reserve(count)
            .map_err(|_| HeapError::Capacity)?;
        self.stale
            .try_reserve(count)
            .map_err(|_| HeapError::Capacity)?;
        Ok(())
    }

    /// No fallible work, numerical operation or callback remains in this append.
    pub(crate) fn publish(&mut self, prepared: PreparedGraph) {
        self.records.push(prepared.record);
        self.nodes.extend(prepared.nodes);
        self.reverse.extend(prepared.reverse);
        self.stale.extend(prepared.stale);
        self.usage = prepared.usage;
    }
}

/// Counter exhaustion is inaccessible through bounded public constructors.
#[cfg(test)]
mod tests {
    use crate::binding_testing::{TestResult, operation};
    use crate::{Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationError};
    use nrz_units::Descriptor;

    /// Lifetime record accounting never wraps into an apparently affordable append.
    /// ´test:unit:provenance-counter-overflow-refuses-atomically´
    #[test]
    fn provenance_counter_overflow_refuses_atomically() -> TestResult {
        let mut heap = Heap::new(HeapLimits::default())?;
        let source =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0_f64])?;
        heap.provenance.usage.records = usize::MAX;
        heap.provenance.limits.records = usize::MAX;
        let bound = Binding::new(
            operation(1, 1)?,
            vec![source.value()],
            ExecutionLimits::default(),
        )
        .validate(&mut heap)?;
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let refusal = bound.publish(candidate).err().ok_or("overflow admitted")?;
        assert!(matches!(refusal.cause(), OperationError::ProvenanceLimit));
        assert_eq!(heap.provenance.records.len(), 0);
        assert_eq!(heap.provenance.nodes.len(), 0);
        assert_eq!(heap.descriptors.iter().count(), 1);
        Ok(())
    }
}
