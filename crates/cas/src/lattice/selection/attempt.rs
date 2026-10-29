// SPDX-License-Identifier: AGPL-3.0-only
//! Sealed valid evidence survives an inconclusive cut or exhausted decision ledger.

use super::{AT, LatticeEnclosure, LatticeSelection, invalid};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, FaultKind, InvalidCode};
use crate::graph::{Cas, Handle, Vertex};
use crate::number::Rational;
use crate::proof_model::VerifiedWitness;

/// The complete fixed decision policy; there is no hidden refinement or fallback candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LatticeAttemptPolicy {
    /// Check one named tuple with one supplied original-target enclosure, then publish atomically.
    NamedTupleSingleEnclosure,
}

/// No cut proof or portable selected theorem is retained by a prepared or undecided attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LatticePendingObligation {
    /// Establish all included lower and strictly excluded upper floor cuts together.
    WholeBoxCuts,
    /// Publish the complete independently replayable witness with its original dependencies.
    PortableWitness,
}

/// One attempted coordinate cut, with no selected-coordinate authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LatticeFloorCut<'a> {
    /// Exact proposed Integer k; the required comparisons are k <= lower and upper < k + 1.
    pub candidate: &'a Handle,
    /// Certified transformed lower endpoint.
    pub lower: &'a Rational,
    /// Certified transformed upper endpoint.
    pub upper: &'a Rational,
}

/// A complete validated question and enclosure, without a selected tuple or published theorem.
///
/// The fixed policy checks one named tuple against one supplied enclosure. There is no implicit
/// refinement or fallback index; a stronger certificate requires a separately prepared attempt.
#[derive(Clone, Debug)]
pub struct CheckedLatticeAttempt {
    /// Authenticated immutable cover root in the preparing arena.
    cover: Handle,
    /// Original question and attempted Integer tuple, never yet a selected descriptor.
    candidate: LatticeSelection,
    /// Both the original-target identity and every enclosure scalar have passed replay.
    enclosure: LatticeEnclosure,
    /// Cumulative Work at successful preparation, including the caller's prior Work.
    spent_work: u64,
}

impl CheckedLatticeAttempt {
    /// Explicit policy shared by preparation, successful selection and undecided handoff.
    #[must_use]
    pub const fn policy(&self) -> LatticeAttemptPolicy {
        LatticeAttemptPolicy::NamedTupleSingleEnclosure
    }

    /// Complete obligations still required to obtain a selected theorem from this receipt.
    #[must_use]
    pub const fn pending_obligations(&self) -> [LatticePendingObligation; 2] {
        [
            LatticePendingObligation::WholeBoxCuts,
            LatticePendingObligation::PortableWitness,
        ]
    }

    /// Borrow one proposed cut without allocating or claiming that any coordinate was selected.
    #[must_use]
    pub fn cut(&self, coordinate: usize) -> Option<LatticeFloorCut<'_>> {
        let candidate = self.candidate.values.get(coordinate)?;
        let (lower, upper) = super::bounds(&self.enclosure, coordinate).ok()?;
        Some(LatticeFloorCut {
            candidate,
            lower,
            upper,
        })
    }

    /// Complete original question and attempted tuple; no cut authority is implied.
    #[must_use]
    pub const fn candidate(&self) -> &LatticeSelection {
        &self.candidate
    }

    /// Last valid original and transformed boxes, or the independently replayed direct H.
    #[must_use]
    pub const fn enclosure(&self) -> &LatticeEnclosure {
        &self.enclosure
    }

    /// Retained cover proof preserving every qualified parameter assumption.
    #[must_use]
    pub const fn cover_proof(&self) -> &Handle {
        &self.cover
    }

    /// Monotone caller Work recorded after complete evidence validation.
    #[must_use]
    pub const fn spent_work(&self) -> u64 {
        self.spent_work
    }
}

/// Distinct reasons for a valid prepared attempt to lack a complete selected theorem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LatticeUndecidedReason {
    /// The certified outer box is not wholly inside the named half-open cell.
    /// This alone does not assert that the exact target occupies multiple cells.
    BoundaryIntersection,
    /// The decision or its complete atomic publication exhausted a declared resource.
    DecisionBudgetExhausted,
}

/// A complete valid evidence record borrowed without allocating after exhaustion.
///
/// Pending obligations are the whole-box cuts and complete portable witness publication.
/// The attempted tuple gains no selection authority, complement membership or default value.
#[derive(Debug)]
pub struct LatticeUndecided<'a> {
    /// Named inconclusive condition, distinct from malformed evidence.
    pub reason: LatticeUndecidedReason,
    /// Complete sealed evidence and fixed one-enclosure policy.
    pub evidence: &'a CheckedLatticeAttempt,
    /// Exact rejecting owner or resource ceiling, retaining its needed and allowed amounts.
    pub fault: Fault,
    /// Cumulative decision-ledger Work at refusal, including the caller's prior Work.
    pub spent_work: u64,
}

/// Only a complete portable witness confers selected tuple authority.
#[derive(Debug)]
pub enum LatticeSelectionOutcome<'a> {
    /// Original-target theorem retaining all cover and enclosure dependencies.
    Decided(Box<VerifiedWitness>),
    /// Valid evidence remains available without any partially published tuple.
    Undecided(LatticeUndecided<'a>),
}

impl Cas {
    /// Validate complete enclosure and candidate range evidence before attempting any cell cut.
    ///
    /// This read-only operation creates local evidence only. Unsupported or malformed origins
    /// refuse here; a decision can later retain this receipt even if its own ledger is exhausted.
    /// # Errors
    /// Refuses a foreign or mismatched cover, altered original target, invalid tuple or enclosure,
    /// excluded range entry and exhausted preparation storage or Work. No graph record is added.
    pub fn prepare_lattice_selection(
        &self,
        cover: &VerifiedWitness,
        expected: &LatticeSelection,
        enclosure: &LatticeEnclosure,
        budget: &mut Budget,
    ) -> Result<CheckedLatticeAttempt, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = self.lattice_selection_attempt(cover, expected, enclosure, budget);
        budget.restore_limits(previous);
        result
    }

    /// Validate and prepay local evidence under both the caller's and arena's unchanged ceilings.
    fn lattice_selection_attempt(
        &self,
        cover: &VerifiedWitness,
        expected: &LatticeSelection,
        enclosure: &LatticeEnclosure,
        budget: &mut Budget,
    ) -> Result<CheckedLatticeAttempt, Fault> {
        budget.work(4, AT)?;
        let Vertex::Proof(application) = self.state.node(self.checked(cover.proof())?)? else {
            return Err(invalid());
        };
        let Vertex::ScopedApplication(premise) = self.state.node(*application)? else {
            return Err(invalid());
        };
        let bytes = super::wire::payload(self, expected, enclosure, budget)?.saturating_add(128);
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        let scratch = budget
            .limits()
            .component_bits
            .div_ceil(8)
            .saturating_mul(32)
            .saturating_add(4_096);
        budget.with_retained_payload(bytes.saturating_add(scratch), AT, |budget| {
            super::cover_matches(expected, premise)?;
            super::roles(self, expected, budget)?;
            super::validate(self, expected, enclosure, budget)?;
            super::ranges(self, expected, premise, budget)?;
            Ok(CheckedLatticeAttempt {
                cover: cover.proof().clone(),
                candidate: expected.clone(),
                enclosure: enclosure.clone(),
                spent_work: budget.usage().work,
            })
        })
    }

    /// Attempt the strict cuts and atomic portable publication under the prepared fixed policy.
    ///
    /// Both undecided cases borrow the sealed valid evidence, requiring no allocation after a
    /// resource refusal. Reusing this local receipt in another arena is an identity error.
    /// # Errors
    /// Refuses foreign evidence and any fault other than inconclusive cuts or resource exhaustion.
    pub fn select_lattice_attempt<'a>(
        &mut self,
        attempt: &'a CheckedLatticeAttempt,
        budget: &mut Budget,
    ) -> Result<LatticeSelectionOutcome<'a>, Fault> {
        self.checked(&attempt.cover)?;
        let result = self.prove_lattice_selection_root(
            &attempt.cover,
            &attempt.candidate,
            &attempt.enclosure,
            budget,
        );
        match result {
            Ok(proof) => Ok(LatticeSelectionOutcome::Decided(Box::new(proof))),
            Err(fault) => {
                let reason = match fault.kind {
                    FaultKind::Invalid(InvalidCode::DecisionBoundaryIntersection)
                        if fault.location == AT =>
                    {
                        LatticeUndecidedReason::BoundaryIntersection
                    }
                    FaultKind::Exhausted { .. } => LatticeUndecidedReason::DecisionBudgetExhausted,
                    FaultKind::Invalid(_) => return Err(fault),
                };
                Ok(LatticeSelectionOutcome::Undecided(LatticeUndecided {
                    reason,
                    evidence: attempt,
                    fault,
                    spent_work: budget.usage().work,
                }))
            }
        }
    }
}
