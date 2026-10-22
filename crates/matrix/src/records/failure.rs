// SPDX-License-Identifier: AGPL-3.0-only
//! Refusals retain independently valid exact-copy evidence without success handles.
//!
//! # Retained partial evidence
//!
//! `Failure` carries a typed cause and an unpublished `FailureRecord`. A later candidate or provenance refusal preserves each independently validated earlier `CopyEvidence`, including its exact input coverage, without a success handle or an exposed candidate payload. `FailureRecord::evidence` rechecks the particular claim's actual source and companion dependencies; `historical_evidence` supplies only the recorded attempt facts. The copy recipe asserts represented bit identity and retention of declared assumptions, not a new floating certificate, statistical model, numerical budget success or source independence.

use nrz_scalar::Scalar;

use crate::{
    CheckedOperation, Coverage, ExecutionId, Heap, HeapError, InputSnapshot, OperationError,
};

/// Proof of an exact copy of one selected input and its complete supported companions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CopyEvidence {
    /// Input/output position in the closed native copy recipe.
    pub(crate) slot: usize,
    /// Logical center entries copied with identical represented bits.
    pub(crate) entries: usize,
    /// Input coverage retained without asserting new physical exactness.
    pub(crate) coverage: Coverage,
}

impl CopyEvidence {
    /// Operation-ordered input/output position covered by this claim.
    #[must_use]
    pub const fn slot(self) -> usize {
        self.slot
    }
    /// Number of bit-exact copied logical center entries.
    #[must_use]
    pub const fn entries(self) -> usize {
        self.entries
    }
    /// Preserved declared or represented-input scope.
    #[must_use]
    pub const fn coverage(self) -> Coverage {
        self.coverage
    }
}

/// Unpublished attempt facts; its partial evidence has no output handle or candidate payload.
#[derive(Debug)]
pub struct FailureRecord<P: Scalar> {
    /// Typed refusal, retained with the complete attempt facts.
    pub(crate) cause: OperationError,
    /// Binding identity of the refused attempt.
    pub(crate) execution: ExecutionId,
    /// Original recipe and exact normalization definitions.
    pub(crate) operation: CheckedOperation,
    /// Captured actual dependencies, preserved independently of the heap borrow.
    pub(crate) inputs: Vec<InputSnapshot<P>>,
    /// Individually valid claims established before the refusal.
    pub(crate) evidence: Vec<crate::RecipeEvidence>,
}

impl<P: Scalar + 'static> FailureRecord<P> {
    /// Execution of the refused attempt.
    #[must_use]
    pub const fn execution(&self) -> ExecutionId {
        self.execution
    }
    /// Checked recipe retained for failure inspection.
    #[must_use]
    pub const fn operation(&self) -> &CheckedOperation {
        &self.operation
    }
    /// Historical partial claims, without current-evidence authority.
    #[must_use]
    pub fn historical_evidence(&self) -> &[crate::RecipeEvidence] {
        &self.evidence
    }
    /// Inspect one partial claim after rechecking only its actual input dependencies.
    /// # Errors
    /// Refuses invalid claim positions, foreign owners and stale source evidence.
    pub fn evidence(&self, heap: &Heap, index: usize) -> Result<&crate::RecipeEvidence, HeapError> {
        let claim = self.evidence.get(index).ok_or(HeapError::InvalidLayout)?;
        for slot in self.operation.input_slots(claim.slot())? {
            let input = self.inputs.get(slot).ok_or(HeapError::StaleEvidence)?;
            super::Dependency::from_snapshot(input).validate(heap)?;
            heap.validate_provenance(input.handle())?;
        }
        Ok(claim)
    }
}

/// Explicit refusal with available partial evidence and no published result.
#[derive(Debug)]
pub struct Failure<P: Scalar> {
    /// Boxed attempt facts keep the public error cheap to move without caller-side boxing.
    pub(crate) record: Box<FailureRecord<P>>,
}

impl<P: Scalar> std::fmt::Display for Failure<P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.record.cause, formatter)
    }
}

impl<P: Scalar> std::error::Error for Failure<P> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.record.cause)
    }
}

impl<P: Scalar> Failure<P> {
    /// Typed refusal reason.
    #[must_use]
    pub const fn cause(&self) -> &OperationError {
        &self.record.cause
    }
    /// Unpublished attempt record; it cannot supply a value handle.
    #[must_use]
    pub const fn record(&self) -> &FailureRecord<P> {
        &self.record
    }
}
