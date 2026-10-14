// SPDX-License-Identifier: AGPL-3.0-only
//! Replay derivative-level roots once while retaining only covered, sealed subproofs.

use std::collections::BTreeSet;

use crate::boundary_producer::Runner;
use crate::budget::BudgetKind;
use crate::case_syntax::invalid;
use crate::error::{Fault, Location};
use crate::graph::{Handle, LogicalId, Vertex, size};
use crate::proof_model::VerifiedWitness;
use crate::task::{BoundaryFact, Task};

/// Replay-closure traversal and staging location.
const AT: Location = Location::new("derivative level replay");

impl Runner<'_> {
    /// Higher orders share large checked prefixes; first-order request accounting stays unchanged.
    pub(crate) fn higher_order(&self) -> bool {
        match &self.request.task {
            Task::Differentiate { order, .. } => *order > 1,
            Task::Boundary {
                requested_facts, ..
            } => requested_facts
                .iter()
                .any(|fact| matches!(fact, BoundaryFact::Derivative { order } if *order > 1)),
            _ => false,
        }
    }

    /// A later root covers its complete dependency DAG, but never an unrelated point obstruction.
    pub(crate) fn flush(&mut self) -> Result<(), Fault> {
        let Some(pending) = self.deferred.take() else {
            return Ok(());
        };
        let mut retained = BTreeSet::new();
        for root in pending.iter().rev() {
            if retained.contains(&root.logical_id()) {
                continue;
            }
            let closure = self.dependencies(root)?;
            let witness = self.cas.seal_scoped(root, self.checker)?;
            crate::case_producer::replay(self.cas, &witness, self.checker)?;
            self.keep_covered(&pending, &closure, &mut retained, witness)?;
        }
        Ok(())
    }

    /// Replayed applications certify every earlier application reached through premise edges.
    fn dependencies(&mut self, root: &Handle) -> Result<BTreeSet<LogicalId>, Fault> {
        let mut pending = vec![self.cas.checked(root)?];
        let mut reached = BTreeSet::new();
        while let Some(proof) = pending.pop() {
            self.checker.work(1, AT)?;
            self.checker.check(
                BudgetKind::ProofNodes,
                size(reached.len().saturating_add(1), AT)?,
                AT,
            )?;
            if !reached.insert(proof) {
                continue;
            }
            let Vertex::Proof(application) = self.cas.state.node(proof)? else {
                return Err(invalid());
            };
            let premises = match self.cas.state.node(*application)? {
                Vertex::ScopedApplication(value) => &value.premises,
                Vertex::Application(value) => &value.premises,
                _ => return Err(invalid()),
            };
            let count = pending
                .len()
                .checked_add(premises.len())
                .ok_or_else(invalid)?;
            self.checker
                .check(BudgetKind::ProofNodes, size(count, AT)?, AT)?;
            self.checker.check(
                BudgetKind::StagedPayloadBytes,
                size(count.saturating_add(reached.len()), AT)?.saturating_mul(128),
                AT,
            )?;
            pending
                .try_reserve(premises.len())
                .map_err(|_| Fault::allocation(AT))?;
            pending.extend(premises);
        }
        Ok(reached)
    }

    /// Preserve the pending fact order within each successfully replayed closure.
    fn keep_covered(
        &mut self,
        pending: &[Handle],
        closure: &BTreeSet<LogicalId>,
        retained: &mut BTreeSet<LogicalId>,
        witness: VerifiedWitness,
    ) -> Result<(), Fault> {
        let root = witness.proof().clone();
        let mut sealed = Some(witness);
        for proof in pending {
            let id = proof.logical_id();
            self.checker.work(1, AT)?;
            if retained.contains(&id) || !closure.contains(&id) {
                continue;
            }
            let value = if *proof == root {
                sealed.take().ok_or_else(invalid)?
            } else {
                self.cas.seal_scoped(proof, self.checker)?
            };
            self.keep(value)?;
            retained.insert(id);
        }
        Ok(())
    }
}
