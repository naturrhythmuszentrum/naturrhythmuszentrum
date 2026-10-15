// SPDX-License-Identifier: AGPL-3.0-only
//! Candidate validation finishes before any descriptor, source edge or record is published.

use super::{Candidate, Payload};
use crate::binding::buffer;
use crate::heap::records::Meaning;
use crate::provenance::PreparedGraph;
use crate::{
    Bound, CopyEvidence, Coverage, Dependency, Failure, HeapError, OperationError, OperationRecord,
    Outcome, OutputRecord, ProvenanceId, PublishedBundle, RecordId, SchemaId,
};
use nrz_scalar::Scalar;

impl<P: Scalar + 'static> Bound<'_, P> {
    /// Consume this execution and atomically publish its complete checked output bundle.
    ///
    /// Every payload, target, source edge, record, limit and allocation is prepared before
    /// the no-fail commit. A refusal leaves all existing values and versions unchanged.
    /// # Errors
    /// Refuses foreign/replayed candidates, incomplete companions, changed contracts,
    /// nonfinite or incorrect copies, stale dependencies and provenance capacity/work.
    pub fn publish(
        mut self,
        candidate: Candidate<P>,
    ) -> Result<Outcome<Vec<PublishedBundle<P>>>, Failure<P>> {
        let mut evidence = Vec::new();
        let prepared = self.prepare_publication(&candidate, &mut evidence);
        drop(candidate);
        match prepared {
            Ok(graph) => Ok(self.commit(graph)),
            Err(cause) => Err(self.failure(cause, evidence)),
        }
    }

    /// Complete all checks and fallible allocations under the uninterrupted exclusive borrow.
    fn prepare_publication(
        &mut self,
        candidate: &Candidate<P>,
        evidence: &mut Vec<CopyEvidence>,
    ) -> Result<PreparedGraph, OperationError> {
        if candidate.execution != self.execution || self.issued.get() != Some(candidate.token) {
            return Err(OperationError::ForeignCandidate);
        }
        if candidate.revision != self.operation.revision()
            || candidate.outputs.len() != self.dependencies.len()
        {
            return Err(OperationError::CandidateContract);
        }
        self.reservations.validate(self.heap)?;
        evidence
            .try_reserve_exact(candidate.outputs.len())
            .map_err(|_| HeapError::Capacity)?;
        for (slot, payload) in candidate.outputs.iter().enumerate() {
            self.verify_copy(slot, payload)?;
            let output = self
                .reservations
                .outputs
                .get(slot)
                .ok_or(OperationError::CandidateContract)?;
            evidence.push(CopyEvidence {
                slot,
                entries: payload.extent.entries(),
                coverage: output.coverage(),
            });
        }
        let record = self.prepare_record(candidate.token, evidence)?;
        super::fill::fill(&mut self.reservations.storage, &candidate.outputs)?;
        self.heap.provenance.prepare(record)
    }

    /// Independently recheck bit identity and all carried bounds against the frozen inputs.
    fn verify_copy(&self, slot: usize, payload: &Payload<P>) -> Result<(), OperationError> {
        let reservation = self
            .reservations
            .outputs
            .get(slot)
            .ok_or(OperationError::CandidateContract)?;
        if payload.target != reservation.stamp()
            || payload.extent != reservation.extent()
            || payload.bounds.is_some() != (reservation.coverage() == Coverage::DeclaredBounds)
        {
            return Err(OperationError::CandidateContract);
        }
        crate::heap::publication::scan(
            payload.extent,
            &payload.centers,
            payload.bounds.as_deref(),
        )?;
        let input = self
            .dependencies
            .get(slot)
            .ok_or(OperationError::CandidateContract)?;
        Dependency::from_snapshot(input).validate(self.heap)?;
        let source = self.heap.value(input.handle())?;
        let bound = self.heap.selected_bound(input.handle())?;
        for (index, center) in payload.centers.iter().enumerate() {
            let row = index % payload.extent.rows();
            let column = index / payload.extent.rows();
            if !center.same_bits(source.region().get(row, column)?) {
                return Err(OperationError::CandidateContract);
            }
            if let (Some(values), Some(original)) = (&payload.bounds, &bound) {
                let radius = values.get(index).ok_or(OperationError::CandidateContract)?;
                if !radius.same_bits(original.region().get(row, column)?) {
                    return Err(OperationError::CandidateContract);
                }
            }
        }
        Ok(())
    }

    /// Build immutable source/target and operation/evidence relations before graph insertion.
    fn prepare_record(
        &self,
        token: u64,
        evidence: &[CopyEvidence],
    ) -> Result<OperationRecord, OperationError> {
        let mut inputs = buffer(self.dependencies.len())?;
        for input in &self.dependencies {
            inputs.push(Dependency::from_snapshot(input));
        }
        let outputs = self.output_records()?;
        let mut claims = buffer(evidence.len())?;
        claims.extend_from_slice(evidence);
        Ok(OperationRecord {
            id: RecordId {
                scope: self.heap.scope,
                index: 0,
            },
            execution: self.execution,
            token,
            operation: self.operation.clone(),
            inputs,
            outputs,
            evidence: claims,
            precision: P::PRECISION,
        })
    }

    /// Validate both directions of each reserved companion relation in the same bundle.
    fn output_records(&self) -> Result<Vec<OutputRecord>, OperationError> {
        let mut outputs = buffer(self.reservations.outputs.len())?;
        let mut position = 0_usize;
        for (bundle, reservation) in self
            .reservations
            .bundles
            .iter()
            .zip(&self.reservations.outputs)
        {
            let record = self
                .reservations
                .records
                .values
                .get(position)
                .ok_or(OperationError::CandidateContract)?;
            if !matches!(record.meaning, Meaning::Value { stamp, bound }
                if stamp == reservation.stamp() && bound == bundle.bound().map(|handle| handle.key))
                || record.schema != reservation.schema().index
            {
                return Err(OperationError::CandidateContract);
            }
            position = position.checked_add(1).ok_or(HeapError::Capacity)?;
            let bound_schema = if bundle.bound().is_some() {
                let companion = self
                    .reservations
                    .records
                    .values
                    .get(position)
                    .ok_or(OperationError::CandidateContract)?;
                if !matches!(companion.meaning, Meaning::Bound { target, stamp }
                    if target == bundle.value().key && stamp == reservation.stamp())
                {
                    return Err(OperationError::CandidateContract);
                }
                position = position.checked_add(1).ok_or(HeapError::Capacity)?;
                Some(SchemaId {
                    scope: self.heap.scope,
                    index: companion.schema,
                })
            } else {
                None
            };
            outputs.push(OutputRecord {
                dependency: Dependency::reserved(*bundle, *reservation, bound_schema)?,
                provenance: ProvenanceId {
                    scope: self.heap.scope,
                    index: 0,
                },
            });
        }
        if position != self.reservations.records.values.len() {
            return Err(OperationError::CandidateContract);
        }
        Ok(outputs)
    }

    /// Link only already prepared objects; no fallible allocation, callback or kernel remains.
    fn commit(self, graph: PreparedGraph) -> Outcome<Vec<PublishedBundle<P>>> {
        let record = graph.record_id();
        self.heap.storage.publish(self.reservations.storage);
        self.heap.schemas.extend(self.reservations.schemas);
        self.heap.descriptors.publish(self.reservations.records);
        self.heap.provenance.publish(graph);
        Outcome {
            output: self.reservations.bundles,
            record,
        }
    }
}
