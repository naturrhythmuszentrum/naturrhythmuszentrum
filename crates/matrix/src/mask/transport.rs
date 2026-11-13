// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit cell-identity transport separates component addresses from mask positions.

use super::{
    CellLayoutVersion, MaskError, MaskVersion, charge, memory, storage::MaskRecord, word_count,
};
use crate::Heap;
use nrz_bindings::Budget;

impl Heap {
    /// Copy coverage to a distinct component layout of the same original ordered cells.
    ///
    /// The shared axis proves the identity cell map, independently of component counts
    /// and addresses. Gathered layouts require their original-index map and are refused.
    /// This publishes only native coverage, not initialized numerical values or companion
    /// authority. Consumers must still authenticate the target numerical publication.
    /// # Errors
    /// Refuses unrelated axes, gathered layouts, stale versions and exhausted resources.
    pub fn transport_mask(
        &mut self,
        source: MaskVersion,
        target: CellLayoutVersion,
        budget: &mut Budget,
    ) -> Result<MaskVersion, MaskError> {
        budget.with_work_phase(super::FAMILY, "mask cell transport", |budget| {
            charge(budget, 2)?;
            let input = self.mask(source, source.layout())?;
            let destination = self.masks.layout(self.scope, target)?;
            if input.axis() != destination.axis {
                return Err(MaskError::LayoutMismatch);
            }
            if input.gather().is_some() || destination.gather.is_some() {
                return Err(MaskError::NonIdentityCorrespondence);
            }
            let words = word_count(input.axis().capacity());
            let reservation =
                memory::Reservation::new(self, None, Some(words), 0, [&[], &[]], budget)?;
            reservation.run(budget, |budget| {
                let mut words = memory::zeroed(words, budget)?;
                let input = self.mask(source, source.layout())?;
                for (output, input) in words.iter_mut().zip(input.words()) {
                    charge(budget, 1)?;
                    *output = *input;
                }
                self.masks.publish_mask(
                    self.scope,
                    MaskRecord {
                        layout: target,
                        words,
                    },
                    self.limits.descriptors,
                    budget,
                )
            })
        })
    }
}
