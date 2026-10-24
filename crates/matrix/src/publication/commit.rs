// SPDX-License-Identifier: AGPL-3.0-only
//! Candidate validation finishes before any descriptor, source edge or record is published.

use super::{Candidate, Payload};
use crate::binding::buffer;
use crate::heap::records::Meaning;
use crate::provenance::PreparedGraph;
use crate::{
    Bound, Dependency, Failure, HeapError, OperationError, OperationRecord, Outcome, OutputRecord,
    ProvenanceId, PublishedBundle, RecordId, SchemaId,
};
use nrz_scalar::Scalar;

impl<P: Scalar + 'static, Q: Scalar + 'static> Bound<'_, P, Q> {
    /// Consume this execution and atomically publish its complete checked output bundle.
    ///
    /// Every payload, target, source edge, record, limit and allocation is prepared before
    /// the no-fail commit. A refusal leaves all existing values and versions unchanged.
    /// # Errors
    /// Refuses foreign/replayed candidates, incomplete companions, changed contracts,
    /// nonfinite or incorrect copies, stale dependencies and provenance capacity/work.
    pub fn publish(
        mut self,
        candidate: Candidate<Q>,
    ) -> Result<Outcome<Vec<PublishedBundle<Q>>>, Failure<P>> {
        let mut evidence = Vec::new();
        let prepared = self.prepare_publication(&candidate, &mut evidence);
        drop(candidate);
        match prepared {
            Ok((graph, moments)) => Ok(self.commit(graph, moments)),
            Err(cause) => Err(self.failure(cause, evidence)),
        }
    }

    /// Complete all checks and fallible allocations under the uninterrupted exclusive borrow.
    fn prepare_publication(
        &mut self,
        candidate: &Candidate<Q>,
        evidence: &mut Vec<crate::RecipeEvidence>,
    ) -> Result<(PreparedGraph, crate::statistics::PreparedMoments), OperationError> {
        if candidate.execution != self.execution || self.issued.get() != Some(candidate.token) {
            return Err(OperationError::ForeignCandidate);
        }
        if candidate.revision != self.operation.revision()
            || candidate.outputs.len() != self.reservations.outputs.len()
        {
            return Err(OperationError::CandidateContract);
        }
        for input in &self.dependencies {
            Dependency::from_snapshot(input).validate(self.heap)?;
            self.heap.validate_provenance(input.handle())?;
        }
        self.reservations.validate(self.heap)?;
        evidence
            .try_reserve_exact(candidate.outputs.len())
            .map_err(|_| HeapError::Capacity)?;
        for (slot, payload) in candidate.outputs.iter().enumerate() {
            self.verify_payload(slot, payload)?;
            self.verify_recipe(slot, payload, candidate.evidence.get(slot))?;
            let output = self
                .reservations
                .outputs
                .get(slot)
                .ok_or(OperationError::CandidateContract)?;
            let claim = candidate
                .evidence
                .get(slot)
                .ok_or(OperationError::CandidateContract)?;
            if claim.slot() != slot
                || claim.entries() != payload.extent.entries()
                || claim.coverage() != output.coverage()
            {
                return Err(OperationError::CandidateContract);
            }
            evidence.push(claim.clone());
        }
        let private = crate::private::Run::publication(self, candidate)?;
        let solve = crate::solve::scratch::Run::publication(self, candidate)?;
        if let Some(crate::RecipeEvidence::Solve(claim)) = evidence.first_mut() {
            claim.inventory = solve;
        }
        let record = self.prepare_record(candidate.token, evidence, private)?;
        super::fill::fill(&mut self.reservations.storage, &candidate.outputs)?;
        let moments = crate::statistics::prepare(self, candidate, &record.outputs)?;
        crate::binding::parameters::validate(
            &self.reservations.covers,
            &self.reservations.records,
        )?;
        Ok((self.heap.provenance.prepare(record)?, moments))
    }

    /// Match the private proof to the exact closed operation before committing any output.
    fn verify_recipe(
        &self,
        slot: usize,
        payload: &Payload<Q>,
        evidence: Option<&crate::RecipeEvidence>,
    ) -> Result<(), OperationError> {
        match &self.operation.recipe {
            crate::binding::NativeRecipe::Solve(contract) => contract.verify::<Q>(evidence)?,
            crate::binding::NativeRecipe::Kernel(contract)
            | crate::binding::NativeRecipe::Reciprocal {
                kernel: contract, ..
            } => {
                let Some(crate::RecipeEvidence::Kernel(claim)) = evidence else {
                    return Err(OperationError::CandidateContract);
                };
                if claim.kind() != contract.kind()
                    || claim.precision() != Q::PRECISION
                    || claim.arithmetic_revision() != 1
                    || claim.work() != contract.work
                {
                    return Err(OperationError::CandidateContract);
                }
            }
            crate::binding::NativeRecipe::Copy => {
                if !matches!(evidence, Some(crate::RecipeEvidence::Copy(_))) {
                    return Err(OperationError::CandidateContract);
                }
                self.verify_copy(slot, payload)?;
            }
            crate::binding::NativeRecipe::Affine(contracts) => {
                let contract = contracts
                    .get(slot)
                    .ok_or(OperationError::CandidateContract)?;
                let Some(crate::RecipeEvidence::Conversion(claim)) = evidence else {
                    return Err(OperationError::CandidateContract);
                };
                if claim.map() != contract.map() || claim.precision() != Q::PRECISION {
                    return Err(OperationError::CandidateContract);
                }
            }
            crate::binding::NativeRecipe::Precision { source, target } => {
                let Some(crate::RecipeEvidence::Precision(claim)) = evidence else {
                    return Err(OperationError::CandidateContract);
                };
                if claim.source_precision() != *source || claim.target_precision() != *target {
                    return Err(OperationError::CandidateContract);
                }
            }
        }
        Ok(())
    }

    /// Check reserved targets, finite coordinates and every required companion region.
    fn verify_payload(&self, slot: usize, payload: &Payload<Q>) -> Result<(), OperationError> {
        let input = self
            .dependencies
            .get(slot)
            .ok_or(OperationError::CandidateContract)?;
        Dependency::from_snapshot(input).validate(self.heap)?;
        let reservation = self
            .reservations
            .outputs
            .get(slot)
            .ok_or(OperationError::CandidateContract)?;
        if payload.target != reservation.stamp()
            || payload.extent != reservation.extent()
            || payload.bounds.is_some() != reservation.bounded
            || payload.evaluation.is_some() != reservation.evaluation
            || payload.contributions.is_some() != reservation.contributions
        {
            return Err(OperationError::CandidateContract);
        }
        crate::heap::publication::scan(
            payload.extent,
            &payload.centers,
            payload.bounds.as_deref(),
        )?;
        if let Some(evaluation) = &payload.evaluation {
            crate::heap::publication::scan(payload.extent, &payload.centers, Some(evaluation))?;
            let total = payload
                .bounds
                .as_ref()
                .ok_or(OperationError::CandidateContract)?;
            if evaluation
                .iter()
                .zip(total)
                .any(|(error, radius)| error > radius)
            {
                return Err(OperationError::CandidateContract);
            }
        }
        if let Some(parts) = &payload.contributions {
            let total = payload
                .bounds
                .as_ref()
                .ok_or(OperationError::CandidateContract)?;
            for part in parts {
                crate::heap::publication::scan(payload.extent, &payload.centers, Some(part))?;
                if part.iter().zip(total).any(|(term, radius)| term > radius) {
                    return Err(OperationError::CandidateContract);
                }
            }
        }
        self.verify_output_structure(slot, payload)
    }

    /// Every declared output structural zero is checked before its schema can escape.
    fn verify_output_structure(
        &self,
        slot: usize,
        payload: &Payload<Q>,
    ) -> Result<(), OperationError> {
        let contract = self
            .operation
            .outputs
            .get(slot)
            .ok_or(OperationError::CandidateContract)?;
        for (entry, meaning) in contract.shape().entries().iter().enumerate() {
            if meaning.is_zero() {
                let center = payload
                    .centers
                    .get(entry)
                    .ok_or(OperationError::CandidateContract)?;
                let bound = payload.bounds.as_ref().and_then(|values| values.get(entry));
                if *center != Q::ZERO
                    || (contract.shape().structure_scope() == crate::StructureScope::WholeFamily
                        && bound.is_some_and(|radius| *radius != Q::ZERO))
                {
                    return Err(OperationError::StructureMismatch { slot, entry });
                }
            }
        }
        Ok(())
    }

    /// Exact-copy publication still requires identity of every coordinate and companion.
    fn verify_copy(&self, slot: usize, payload: &Payload<Q>) -> Result<(), OperationError> {
        let input = self
            .dependencies
            .get(slot)
            .ok_or(OperationError::CandidateContract)?;
        Dependency::from_snapshot(input).validate(self.heap)?;
        let source = self.heap.value(input.handle())?;
        let bound = self.heap.selected_bound(input.handle())?;
        let evaluation = self.heap.selected_evaluation(input.handle())?;
        let contributions = crate::kernels::contribution_views(self.heap, input.handle())?;
        for (index, center) in payload.centers.iter().enumerate() {
            let row = index % payload.extent.rows();
            let column = index / payload.extent.rows();
            if !center
                .to_f64()
                .same_bits(source.region().get(row, column)?.to_f64())
            {
                return Err(OperationError::CandidateContract);
            }
            if let (Some(parts), Some(originals)) = (&payload.contributions, &contributions) {
                for (part, original) in parts.iter().zip(originals) {
                    let radius = part.get(index).ok_or(OperationError::CandidateContract)?;
                    if !radius
                        .to_f64()
                        .same_bits(original.region().get(row, column)?.to_f64())
                    {
                        return Err(OperationError::CandidateContract);
                    }
                }
            }
            if let (Some(values), Some(original)) = (&payload.evaluation, &evaluation)
                && !values
                    .get(index)
                    .ok_or(OperationError::CandidateContract)?
                    .to_f64()
                    .same_bits(original.region().get(row, column)?.to_f64())
            {
                return Err(OperationError::CandidateContract);
            }
            if let (Some(values), Some(original)) = (&payload.bounds, &bound) {
                let radius = values.get(index).ok_or(OperationError::CandidateContract)?;
                if !radius
                    .to_f64()
                    .same_bits(original.region().get(row, column)?.to_f64())
                {
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
        evidence: &[crate::RecipeEvidence],
        private: Option<Box<crate::ReciprocalInventory>>,
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
            precision: Q::PRECISION,
            private,
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
            if !matches!(record.meaning, Meaning::Value { stamp, bound, evaluation, contributions, coverage }
                if stamp == reservation.stamp() && bound == bundle.bound().map(|handle| handle.key)
                    && evaluation == bundle.evaluation().map(|handle| handle.key)
                    && contributions == bundle.contributions.map(|handles| handles.map(|handle| handle.key)) && coverage == reservation.coverage())
                || record.schema != reservation.schema().index
            {
                return Err(OperationError::CandidateContract);
            }
            position = position.checked_add(1).ok_or(HeapError::Capacity)?;
            let mut bound_schema = None;
            for handle in bundle
                .bound()
                .into_iter()
                .chain(bundle.evaluation())
                .chain(bundle.contributions.into_iter().flatten())
            {
                let schema = self.reserved_companion(position, *bundle, *reservation, handle)?;
                if bound_schema.is_some_and(|current| current != schema) {
                    return Err(OperationError::CandidateContract);
                }
                bound_schema = Some(schema);
                position = position.checked_add(1).ok_or(HeapError::Capacity)?;
            }
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

    /// Validate one private companion's exact target, occupation and immutable schema.
    fn reserved_companion(
        &self,
        position: usize,
        bundle: PublishedBundle<Q>,
        reservation: crate::OutputReservation,
        handle: crate::Handle<crate::Companion, Q>,
    ) -> Result<SchemaId, OperationError> {
        let companion = self
            .reservations
            .records
            .values
            .get(position)
            .ok_or(OperationError::CandidateContract)?;
        if !matches!(companion.meaning, Meaning::Bound { target, stamp }
            if target == bundle.value().key && stamp == reservation.stamp())
            || self.reservations.records.keys().get(position) != Some(&handle.key)
        {
            return Err(OperationError::CandidateContract);
        }
        Ok(SchemaId {
            scope: self.heap.scope,
            index: companion.schema,
        })
    }

    /// Link only already prepared objects; no fallible allocation, callback or kernel remains.
    fn commit(
        self,
        graph: PreparedGraph,
        moments: crate::statistics::PreparedMoments,
    ) -> Outcome<Vec<PublishedBundle<Q>>> {
        let record = graph.record_id();
        self.heap.storage.publish(self.reservations.storage);
        crate::binding::schema::publish(self.heap, self.reservations.schemas);
        self.heap.descriptors.publish(self.reservations.records);
        self.heap.symbols.publish(self.reservations.covers);
        self.heap.provenance.publish(graph);
        self.heap.statistics.records.extend(moments.records);
        self.reservations.planned.publish();
        Outcome {
            output: self.reservations.bundles,
            record,
        }
    }
}
