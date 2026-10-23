// SPDX-License-Identifier: AGPL-3.0-only
//! Complete native value/radius replacements retain target maps until one atomic commit.

mod plan;
mod storage;

use super::{LiveBindingError, ReadingBinding};
use crate::{Heap, HeapError};
use nrz_bindings::{Budget, readings::algebra::Location};
use nrz_scalar::Scalar;

/// One accounting location for the complete replacement schedule.
const AT: Location = Location::new("bounded reading replacement");

impl Heap {
    /// Atomically replace selected native centers, total bounds and numerical errors.
    ///
    /// The arrays follow the reading's selected storage order. Unselected entries
    /// retain all three channels. Supplied radii are declarations for the new version;
    /// they do not inherit the previous computation's certificate. Every affected
    /// native target and companion advances together, including for identical centers.
    /// Old reading bindings and dependent results become unavailable or stale.
    /// # Errors
    /// Refuses stale readings, nonnative companions, expanded families, incomplete
    /// bundles, negative/nonfinite radii, errors exceeding totals, product contributions,
    /// exhausted counters and resource failures without publishing any change.
    pub fn replace_reading_bundle<P: Scalar + 'static>(
        &mut self,
        binding: &ReadingBinding<P>,
        centers: &[P],
        totals: &[P],
        errors: &[P],
        budget: &mut Budget,
    ) -> Result<(), LiveBindingError> {
        self.validate_reading(binding)?;
        let channels = [centers, totals, errors];
        plan::check(binding, channels, budget)?;
        let schedule = plan::prepare(self, binding, budget)?;
        let retained = storage::reservation::<P>(self, &schedule, budget)?;
        budget.with_retained_payload(retained, AT, |budget| {
            let payloads = storage::payloads(self, binding, &schedule, channels)?;
            let writes = schedule
                .iter()
                .map(|item| (item.keys[0], item.indices.clone()))
                .collect::<Vec<_>>();
            let mut invalidated = super::invalidation::prepare(self, &writes, budget)?;
            storage::invalidate_native(&mut invalidated, &schedule, budget)?;
            let prepared = storage::prepare(self, &schedule, &payloads)?;
            storage::publish(self, prepared);
            super::invalidation::publish(invalidated);
            Ok(())
        })
    }
}

/// Checked scalar counts never silently wrap into a smaller allowance.
fn count(value: usize) -> Result<u64, LiveBindingError> {
    u64::try_from(value).map_err(|_| HeapError::ExtentOverflow.into())
}
