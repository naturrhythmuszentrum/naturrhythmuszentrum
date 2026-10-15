// SPDX-License-Identifier: AGPL-3.0-only
//! Version comparison and reverse-edge staleness occur on every public evidence access.

use nrz_scalar::Scalar;

use super::{Provenance, ProvenanceId, ProvenanceLimits, ProvenanceUsage};
use crate::heap::records::Meaning;
use crate::{
    Dependency, Handle, Heap, HeapError, OperationError, OperationRecord, RecordId, Value,
    ValueStamp,
};

/// Borrowed current graph edge; its dependencies were rechecked for this access.
#[derive(Debug)]
pub struct ProvenanceView<'h> {
    /// Current evidence node identity.
    identity: ProvenanceId,
    /// Earlier source node, absent for a direct import dependency.
    parent: Option<ProvenanceId>,
    /// Checked longest lineage depth.
    depth: usize,
    /// Operation containing this individual claim.
    record: RecordId,
    /// Concrete input dependency for this copy output.
    source: &'h Dependency,
    /// Concrete output dependency, including retargeted companions.
    target: &'h Dependency,
}

impl ProvenanceView<'_> {
    /// Current node identity.
    #[must_use]
    pub const fn identity(&self) -> ProvenanceId {
        self.identity
    }
    /// Ordered parent relation; the copy recipe has at most one prior operation parent.
    #[must_use]
    pub const fn parent(&self) -> Option<ProvenanceId> {
        self.parent
    }
    /// Longest checked operation lineage.
    #[must_use]
    pub const fn depth(&self) -> usize {
        self.depth
    }
    /// Originating immutable operation record.
    #[must_use]
    pub const fn record(&self) -> RecordId {
        self.record
    }
    /// Exact source version, selection and companion target.
    #[must_use]
    pub const fn source(&self) -> &Dependency {
        self.source
    }
    /// Exact output version and companion target.
    #[must_use]
    pub const fn target(&self) -> &Dependency {
        self.target
    }
}

impl Heap {
    /// Set provenance policy after checking all already retained metadata against it.
    /// # Errors
    /// Refuses a limit smaller than current graph usage; existing policy stays unchanged.
    pub fn set_provenance_limits(
        &mut self,
        limits: ProvenanceLimits,
    ) -> Result<(), OperationError> {
        limits.check(self.provenance.usage)?;
        self.provenance.limits = limits;
        Ok(())
    }

    /// Retained graph accounting, independent of whether any source has since changed.
    #[must_use]
    pub const fn provenance_usage(&self) -> ProvenanceUsage {
        self.provenance.usage
    }

    /// Inspect historical facts without granting current evidence or output handles.
    /// # Errors
    /// Refuses a foreign owner or unknown record identity.
    pub fn historical_record(&self, id: RecordId) -> Result<&OperationRecord, HeapError> {
        if id.scope != self.scope {
            return Err(HeapError::ForeignHandle);
        }
        self.provenance
            .records
            .get(id.index)
            .filter(|record| record.id == id)
            .ok_or(HeapError::StaleEvidence)
    }

    /// Inspect a complete operation record after checking every output's actual dependencies.
    /// # Errors
    /// Refuses foreign records, stale inputs/companions/outputs or exhausted traversal work.
    pub fn operation_record(&self, id: RecordId) -> Result<&OperationRecord, HeapError> {
        let record = self.historical_record(id)?;
        self.provenance.refresh(self)?;
        for output in &record.outputs {
            self.provenance.check_current(output.provenance)?;
        }
        Ok(record)
    }

    /// Inspect one current output's provenance independently of its siblings.
    /// # Errors
    /// Refuses foreign identities, stale lineage or exhausted traversal work.
    pub fn provenance(&self, id: ProvenanceId) -> Result<ProvenanceView<'_>, HeapError> {
        if id.scope != self.scope {
            return Err(HeapError::ForeignHandle);
        }
        self.provenance.refresh(self)?;
        self.provenance.check_current(id)?;
        let node = self
            .provenance
            .nodes
            .get(id.index)
            .ok_or(HeapError::StaleEvidence)?;
        let record = self.historical_record(node.record)?;
        Ok(ProvenanceView {
            identity: id,
            parent: node.parent,
            depth: node.depth,
            record: node.record,
            source: record
                .inputs
                .get(node.slot)
                .ok_or(HeapError::StaleEvidence)?,
            target: &record
                .outputs
                .get(node.slot)
                .ok_or(HeapError::StaleEvidence)?
                .dependency,
        })
    }

    /// Existing heap accessors enter here; raw descriptors below never recursively call them.
    pub(crate) fn validate_provenance<P: Scalar + 'static>(
        &self,
        handle: Handle<Value, P>,
    ) -> Result<(), HeapError> {
        let base = self.base_handle(handle)?;
        let Meaning::Value { stamp, .. } = self.record(base)?.meaning else {
            return Err(HeapError::CategoryMismatch);
        };
        self.provenance.refresh(self)?;
        if let Some(id) = self.provenance.find(stamp) {
            self.provenance.check_current(id)?;
        }
        Ok(())
    }
}

impl Provenance {
    /// Locate exact source versions, never just equal schemas or unchanged addresses.
    pub(super) fn find(&self, stamp: ValueStamp) -> Option<ProvenanceId> {
        self.nodes.iter().enumerate().find_map(|(index, node)| {
            let record = self.records.get(node.record.index)?;
            let output = record.outputs.get(node.slot)?;
            (output.dependency.stamp() == stamp).then_some(ProvenanceId {
                scope: node.record.scope,
                index,
            })
        })
    }

    /// Traverse explicit reverse edges in topological order and mark dependents monotonically.
    pub(super) fn refresh(&self, heap: &Heap) -> Result<(), HeapError> {
        if self.usage.work > self.limits.work {
            return Err(HeapError::WorkLimit);
        }
        for edge in &self.reverse {
            let node = self
                .nodes
                .get(edge.dependent.index)
                .ok_or(HeapError::StaleEvidence)?;
            let record = self
                .records
                .get(node.record.index)
                .ok_or(HeapError::StaleEvidence)?;
            let source = record
                .inputs
                .get(node.slot)
                .ok_or(HeapError::StaleEvidence)?;
            let target = &record
                .outputs
                .get(node.slot)
                .ok_or(HeapError::StaleEvidence)?
                .dependency;
            let parent_stale = node
                .parent
                .is_some_and(|parent| self.check_current(parent).is_err());
            if edge.source != source.stamp()
                || source.validate(heap).is_err()
                || target.validate(heap).is_err()
                || parent_stale
            {
                self.stale
                    .get(edge.dependent.index)
                    .ok_or(HeapError::StaleEvidence)?
                    .set(true);
            }
        }
        Ok(())
    }

    /// A stale output cannot regain authority when bytes happen to become equal again.
    pub(super) fn check_current(&self, id: ProvenanceId) -> Result<(), HeapError> {
        let node = self.nodes.get(id.index).ok_or(HeapError::StaleEvidence)?;
        if node.record.scope != id.scope {
            return Err(HeapError::ForeignHandle);
        }
        if self.stale.get(id.index).is_none_or(std::cell::Cell::get) {
            return Err(HeapError::StaleEvidence);
        }
        Ok(())
    }
}

impl Heap {
    /// A published shape cannot be replaced by freshly named equal-dimensional axes.
    pub(crate) fn validate_bound_shape<P: Scalar + 'static>(
        &self,
        handle: Handle<Value, P>,
        contract: &crate::RegionContract,
        slot: usize,
    ) -> Result<(), OperationError> {
        let base = self.base_handle(handle)?;
        let Meaning::Value { stamp, .. } = self.record(base)?.meaning else {
            return Err(HeapError::CategoryMismatch.into());
        };
        if let Some(id) = self.provenance.find(stamp) {
            let node = self
                .provenance
                .nodes
                .get(id.index)
                .ok_or(HeapError::StaleEvidence)?;
            let record = self.historical_record(node.record)?;
            let original = record
                .operation
                .outputs
                .get(node.slot)
                .ok_or(HeapError::StaleEvidence)?
                .shape();
            if original.domain() != contract.shape().domain()
                || original.codomain() != contract.shape().codomain()
            {
                return Err(OperationError::InputMismatch { slot });
            }
        }
        Ok(())
    }
}
