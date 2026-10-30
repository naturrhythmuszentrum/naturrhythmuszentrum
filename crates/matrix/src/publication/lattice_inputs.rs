// SPDX-License-Identifier: AGPL-3.0-only
//! Historical lattice input capture shares the scalar native inventory and lease checker.

use super::decision_inputs;
use crate::{DecisionInputSnapshot, DecisionSnapshotError, Heap, ValueBinding};
use nrz_bindings::{Budget, LatticeInputs, readings::algebra::limits::BudgetUsage};
use nrz_scalar::Scalar;

/// Complete original lattice inputs captured together under one immutable heap borrow.
///
/// The association retains axis order, physical maps, basis, offsets and convention.
/// Operands follow its deduplicated original-source order, which is distinct from
/// target-coordinate order. Historical centers and radii grant no CAS enclosure or
/// selected-tuple authority; reuse requires `Heap::check_lattice_inputs`.
#[derive(Debug)]
pub struct FrozenLatticeInputs<P: Scalar> {
    /// Original numerical owner, including for an empty input inventory.
    scope: u64,
    /// Complete immutable physical question and original value/companion symbols.
    association: LatticeInputs,
    /// Native original-source versions, with total and included numerical radii.
    operands: Vec<DecisionInputSnapshot<P>>,
    /// Newly retained metadata, excluding shared immutable declaration owners.
    bytes: u64,
    /// Caller usage before and after the complete capture.
    usage: [BudgetUsage; 2],
}

impl<P: Scalar> FrozenLatticeInputs<P> {
    /// Retained physical question and complete ordered original-source association.
    #[must_use]
    pub const fn association(&self) -> &LatticeInputs {
        &self.association
    }

    /// Historical original-source coordinates; these are not a target-vector certificate.
    #[must_use]
    pub fn operands(&self) -> &[DecisionInputSnapshot<P>] {
        &self.operands
    }

    /// Owned native metadata allowance, excluding the shared original physical question.
    #[must_use]
    pub const fn retained_payload_bytes(&self) -> u64 {
        self.bytes
    }

    /// Cumulative caller ledger snapshots; failed attempts never restore spent Work.
    #[must_use]
    pub const fn usage(&self) -> [BudgetUsage; 2] {
        self.usage
    }
}

impl Heap {
    /// Capture every designated native scalar and both companions for a lattice question.
    ///
    /// Bindings authenticates the complete original symbol order before any capture.
    /// Shared source occurrences retain one dependency, while equal independent sources
    /// remain distinct. No selection, enclosure certificate or heap result is published.
    /// # Errors
    /// Refuses missing, reordered, foreign or stale bindings, unsupported selections,
    /// incomplete companions and exhausted cumulative resources atomically.
    pub fn freeze_lattice_inputs<P: Scalar + 'static>(
        &self,
        association: &LatticeInputs,
        inputs: &[ValueBinding<P>],
        budget: &mut Budget,
    ) -> Result<FrozenLatticeInputs<P>, DecisionSnapshotError> {
        let before = budget.usage();
        let bytes = decision_inputs::snapshot_bytes::<P, FrozenLatticeInputs<P>>(inputs.len())?;
        let operands = decision_inputs::capture(self, inputs, bytes, budget, |symbols, budget| {
            association.authenticate(association.case(), symbols, budget)
        })?;
        Ok(FrozenLatticeInputs {
            scope: self.scope,
            association: association.clone(),
            operands,
            bytes,
            usage: [before, budget.usage()],
        })
    }

    /// Authenticate every captured dependency against the current original numerical owner.
    /// # Errors
    /// Refuses foreign heaps, changed values or companions, alias writes and resources.
    /// Even an unchanged center or an equal future tuple cannot authenticate old versions.
    pub fn check_lattice_inputs<P: Scalar + 'static>(
        &self,
        frozen: &FrozenLatticeInputs<P>,
        budget: &mut Budget,
    ) -> Result<(), DecisionSnapshotError> {
        decision_inputs::check(self, frozen.scope, &frozen.operands, budget)
    }
}
