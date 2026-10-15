// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable owned graph evidence and descriptor-annotated logical entry schedules.

use nrz_cas::{Handle, proof::VerifiedWitness};

use super::{SensitivityKernel, SensitivityObligation, SensitivityRecipe};
use crate::{CheckedDeclarations, CheckedExpression, Descriptor, PairingWitness, SnapshotId};

/// Bounded work observed while producing a checked sensitivity's graph evidence.
///
/// Construction and physical checks have separate bounded ledgers. The request maxima
/// include both derivative and coefficient-equality requests, never a numerical evaluation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SensitivityWork {
    /// Work consumed by the shared derivative-phase graph construction ledger.
    pub construction: u64,
    /// Largest generation work counter among the completed symbolic requests.
    pub generation: u64,
    /// Largest independent replay work counter among those requests.
    pub replay: u64,
    /// Number of completed requests, each bounded by its explicit limits.
    pub requests: u64,
}

/// One replayed scalar partial in logical column order.
#[derive(Debug)]
pub struct JacobianEntry {
    /// The CAS derivative expression, never an original physical expression.
    pub(crate) expression: Handle,
    /// Index of the replayed derivative or assembly in `JacobianEvidence::witnesses`.
    pub(crate) witness_index: usize,
    /// Expected quotient supplied by the derivative relationship, including for zero.
    pub(crate) pairing: PairingWitness,
    /// Replayed equality to the closed recipe's expected coefficient.
    pub(crate) identity: VerifiedWitness,
}

impl JacobianEntry {
    /// Graph-owned scalar partial.
    #[must_use]
    pub const fn expression(&self) -> &Handle {
        &self.expression
    }

    /// Exact replayed proof backing this partial in `JacobianEvidence::witnesses`.
    #[must_use]
    pub const fn witness_index(&self) -> usize {
        self.witness_index
    }

    /// Formal output-over-input descriptor; zero has this same typed relationship.
    #[must_use]
    pub const fn pairing(&self) -> &PairingWitness {
        &self.pairing
    }

    /// Equality connecting the CAS partial to the permitted coefficient recipe.
    #[must_use]
    pub const fn identity(&self) -> &VerifiedWitness {
        &self.identity
    }
}

/// A complete ordered Jacobian of checked original scalar relations.
#[derive(Debug)]
pub struct JacobianEvidence {
    /// Original relations checked before asking for any derivatives.
    pub(crate) originals: Vec<CheckedExpression>,
    /// Independent scalar coordinates in exact request order.
    pub(crate) coordinates: Vec<String>,
    /// Replayed native assembly or individually replayed implicit scalar partials.
    pub(crate) witnesses: Vec<VerifiedWitness>,
    /// Observed bounded request work, including coefficient equalities.
    pub(crate) work: SensitivityWork,
    /// Explicitly permuted cells at `row + rows*column`.
    pub(crate) entries: Vec<JacobianEntry>,
}

impl JacobianEvidence {
    /// Checked original relations in output order.
    #[must_use]
    pub fn originals(&self) -> &[CheckedExpression] {
        &self.originals
    }

    /// Exact independent-coordinate declaration keys.
    #[must_use]
    pub fn coordinates(&self) -> &[String] {
        &self.coordinates
    }

    /// CAS derivative proofs with their domains and qualified assumptions intact.
    ///
    /// Native recipes retain one row-ordered assembly; implicit recipes retain one
    /// scalar derivative per cell. Each entry identifies its proof by index.
    #[must_use]
    pub fn witnesses(&self) -> &[VerifiedWitness] {
        &self.witnesses
    }

    /// Per-request work peaks, useful for selecting explicit finite service limits.
    #[must_use]
    pub const fn work(&self) -> SensitivityWork {
        self.work
    }

    /// Scalar partials in logical column order, including formal typed zeros.
    #[must_use]
    pub fn entries(&self) -> &[JacobianEntry] {
        &self.entries
    }
}

/// One output/input pairing and the only permitted numerical evaluation for it.
#[derive(Debug)]
pub struct SensitivityEntry {
    /// Formal final sensitivity meaning.
    pub(crate) pairing: PairingWitness,
    /// Closed schedule instruction referencing the retained coordinate inventory.
    pub(crate) kernel: SensitivityKernel,
}

impl SensitivityEntry {
    /// Descriptor of this derivative entry.
    #[must_use]
    pub const fn pairing(&self) -> &PairingWitness {
        &self.pairing
    }

    /// Fixed numerical instruction; descriptor validity alone cannot create one.
    #[must_use]
    pub const fn kernel(&self) -> SensitivityKernel {
        self.kernel
    }
}

/// A closed native recipe retaining its frozen CAS owner and complete symbolic proof.
///
/// This authorizes only the stated numerical schedule after its domain obligations
/// are established. It proves no floating result, uncertainty model or enclosure.
#[doc = include_str!("../../tests/conformance/sensitivity.inc")]
#[derive(Debug)]
pub struct CheckedSensitivity {
    /// Actual immutable owner of every original and witness handle.
    pub(crate) declarations: CheckedDeclarations,
    /// Native operation family.
    pub(crate) recipe: SensitivityRecipe,
    /// Independent input keys in the final derivative's column order.
    pub(crate) coordinates: Vec<String>,
    /// Checked value recipes, or entrywise defining relations for an implicit solve.
    pub(crate) originals: Vec<CheckedExpression>,
    /// Output placeholder keys, excluded from the independent input inventory.
    pub(crate) auxiliary: Vec<String>,
    /// Input descriptors in their declared coordinates.
    pub(crate) inputs: Vec<Descriptor>,
    /// Output descriptors in coherent coordinates.
    pub(crate) outputs: Vec<Descriptor>,
    /// Inspected and replayed scalar relation derivatives.
    pub(crate) jacobian: JacobianEvidence,
    /// Final sensitivity cells in logical column order.
    pub(crate) entries: Vec<SensitivityEntry>,
    /// Numerical premises that a symbolic theorem cannot establish.
    pub(crate) obligations: Vec<SensitivityObligation>,
}

impl CheckedSensitivity {
    /// Frozen declaration, rule and graph owner; no mutable CAS access escapes.
    #[must_use]
    pub const fn declarations(&self) -> &CheckedDeclarations {
        &self.declarations
    }

    /// Semantic identity required when binding this schedule.
    #[must_use]
    pub const fn snapshot_id(&self) -> &SnapshotId {
        self.declarations.id()
    }

    /// Admitted native operation family.
    #[must_use]
    pub const fn recipe(&self) -> SensitivityRecipe {
        self.recipe
    }

    /// Input declaration keys in exact sensitivity column order.
    #[must_use]
    pub fn coordinates(&self) -> &[String] {
        &self.coordinates
    }

    /// Original value recipes, or the entrywise defining equations of a solve.
    #[must_use]
    pub fn originals(&self) -> &[CheckedExpression] {
        &self.originals
    }

    /// Root/solution placeholders in output order; these are not independent inputs.
    #[must_use]
    pub fn auxiliary_symbols(&self) -> &[String] {
        &self.auxiliary
    }

    /// Input descriptors, whose scales participate in the fixed schedule.
    #[must_use]
    pub fn inputs(&self) -> &[Descriptor] {
        &self.inputs
    }

    /// Output descriptors in exact sensitivity row order.
    #[must_use]
    pub fn outputs(&self) -> &[Descriptor] {
        &self.outputs
    }

    /// Symbolic relation derivatives underlying this recipe.
    #[must_use]
    pub const fn jacobian(&self) -> &JacobianEvidence {
        &self.jacobian
    }

    /// Final entries at `row + outputs().len()*column`.
    #[must_use]
    pub fn entries(&self) -> &[SensitivityEntry] {
        &self.entries
    }

    /// Unresolved numerical premises, never certified by symbolic assumptions.
    #[must_use]
    pub fn obligations(&self) -> &[SensitivityObligation] {
        &self.obligations
    }
}
