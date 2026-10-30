// SPDX-License-Identifier: AGPL-3.0-only
//! Prepared mathematical evidence acquires no live authority from an inconclusive cut.

use crate::{FrozenLatticeEnclosure, Heap, LiveDecisionError};
use nrz_bindings::{
    Budget,
    readings::algebra::{
        Cas, Location,
        lattice::{CheckedLatticeAttempt, LatticeEnclosure, LatticeSelectionOutcome},
    },
};
use nrz_scalar::Scalar;

/// Only the live attempt pays for matching its independently checked original box.
const AT: Location = Location::new("matrix live lattice attempt");

impl Heap {
    /// Return the CAS's complete selection or typed undecided under a freshly checked lease.
    ///
    /// The caller retains both the native enclosure and sealed mathematical attempt. Lend
    /// an archived selected owner's held graph guard through this call without reacquiring
    /// that lock. Undecided borrows all valid evidence and publishes no selected prefix.
    /// A decided mathematical witness still needs physical admission, exact box matching
    /// and final native lease validation through the ordinary live publication route.
    /// # Errors
    /// Refuses changed questions or original boxes, unsupported enclosure forms, stale or
    /// foreign leases and exhausted receiver resources before attempting the CAS cuts.
    pub fn select_lattice_attempt<'a, P: Scalar + 'static>(
        &self,
        enclosure: &'a FrozenLatticeEnclosure<P>,
        graph: &mut Cas,
        attempt: &'a CheckedLatticeAttempt,
        budget: &mut Budget,
    ) -> Result<LatticeSelectionOutcome<'a>, LiveDecisionError> {
        enclosure
            .inputs()
            .association()
            .case()
            .validate_attempt(attempt, budget)
            .map_err(|cause| LiveDecisionError::Admission(Box::new(cause)))?;
        let LatticeEnclosure::Original(original) = attempt.enclosure() else {
            return Err(LiveDecisionError::Recipe);
        };
        budget.work(enclosure.original.retained_bytes(), AT)?;
        if &original.original != enclosure.original.contract() {
            return Err(LiveDecisionError::Enclosure);
        }
        self.check_lattice_inputs(enclosure.inputs(), budget)?;
        Ok(graph.select_lattice_attempt(attempt, budget)?)
    }
}
