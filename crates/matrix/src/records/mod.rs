// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable dependency-specific operation records and scoped publication identities.

mod dependency;
mod failure;

pub use dependency::Dependency;
pub use failure::{CopyEvidence, Failure, FailureRecord};

use crate::{CheckedOperation, ExecutionId, ProvenanceId};
use nrz_scalar::Precision;

/// A heap-scoped immutable operation record identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordId {
    /// Numerical owner of the record.
    pub(crate) scope: u64,
    /// Append-only record position; never reused.
    pub(crate) index: usize,
}

/// One output's exact version and dependency-specific provenance node.
#[derive(Debug)]
pub struct OutputRecord {
    /// Exact published version, schema, selection and companion targets.
    pub(crate) dependency: Dependency,
    /// Node linking only the inputs relevant to this copy output.
    pub(crate) provenance: ProvenanceId,
}

impl OutputRecord {
    /// Published version and concrete interpretation, without a success handle.
    #[must_use]
    pub const fn dependency(&self) -> &Dependency {
        &self.dependency
    }
    /// Scoped evidence node for this individual output.
    #[must_use]
    pub const fn provenance(&self) -> ProvenanceId {
        self.provenance
    }
}

/// Historical operation facts; current access additionally validates every dependency.
#[derive(Debug)]
pub struct OperationRecord {
    /// Allocating owner and immutable record position.
    pub(crate) id: RecordId,
    /// Unique binding execution.
    pub(crate) execution: ExecutionId,
    /// Consumed candidate token, allocated independently of the execution identity.
    pub(crate) token: u64,
    /// Original checked recipe, ordered shapes and exact normalization definitions.
    pub(crate) operation: CheckedOperation,
    /// Actual complete input dependency snapshots, in operation order.
    pub(crate) inputs: Vec<Dependency>,
    /// Output-specific facts and graph edges, in matching order.
    pub(crate) outputs: Vec<OutputRecord>,
    /// Independently established bit-copy claims, never floating arithmetic certificates.
    pub(crate) evidence: Vec<CopyEvidence>,
    /// Representation of the copied coordinates and companions.
    pub(crate) precision: Precision,
}

impl OperationRecord {
    /// Scoped immutable record identity.
    #[must_use]
    pub const fn id(&self) -> RecordId {
        self.id
    }
    /// Execution that checked the actual dependencies and published the bundle.
    #[must_use]
    pub const fn execution(&self) -> ExecutionId {
        self.execution
    }
    /// Complete checked recipe and retained original shape evidence.
    #[must_use]
    pub const fn operation(&self) -> &CheckedOperation {
        &self.operation
    }
    /// Frozen operation-ordered input dependencies.
    #[must_use]
    pub fn inputs(&self) -> &[Dependency] {
        &self.inputs
    }
    /// Published version facts; these are not handles authorizing current access.
    #[must_use]
    pub fn outputs(&self) -> &[OutputRecord] {
        &self.outputs
    }
    /// Established exact-copy claims, with explicitly preserved input coverage.
    #[must_use]
    pub fn evidence(&self) -> &[CopyEvidence] {
        &self.evidence
    }
    /// Sealed input/output precision, unchanged by this recipe.
    #[must_use]
    pub const fn precision(&self) -> Precision {
        self.precision
    }
    /// Environment interpretation: exact finite bit copies require no rounding-mode premise.
    #[must_use]
    pub const fn environment(&self) -> &'static str {
        match self.precision {
            Precision::F32 => "finite binary32 bit copy; no floating arithmetic",
            Precision::F64 => "finite binary64 bit copy; no floating arithmetic",
        }
    }
}

/// A successful publication result and its independently resolvable operation record.
#[derive(Debug)]
pub struct Outcome<T> {
    /// Complete published output bundle, available only after the no-fail commit.
    pub(crate) output: T,
    /// Scoped historical record with current-dependency access through the heap.
    pub(crate) record: RecordId,
}

impl<T> Outcome<T> {
    /// Complete published result.
    #[must_use]
    pub const fn output(&self) -> &T {
        &self.output
    }
    /// Consume the outcome and take its complete published result.
    #[must_use]
    pub fn into_output(self) -> T {
        self.output
    }
    /// Operation record; resolving current evidence rechecks its captured dependencies.
    #[must_use]
    pub const fn record(&self) -> RecordId {
        self.record
    }
}
