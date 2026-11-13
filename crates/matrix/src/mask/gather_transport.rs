// SPDX-License-Identifier: AGPL-3.0-only
//! Native gather transport joins checked original cell identities, never buffer positions.

use super::{
    CellAxis, CellGather, CellLayoutVersion, MaskError, MaskVersion, charge, memory,
    storage::MaskRecord, word_count,
};
use crate::Heap;
use nrz_bindings::Budget;

impl Heap {
    /// Transport native coverage through retained original-index maps of compacted layouts.
    ///
    /// Dense layouts use their own ordered axis; compacted layouts use their retained
    /// original axis and injective original-index map. Missing source associations stay
    /// clear, as does unused target capacity. This moves no numerical values and grants
    /// no numerical lifetime or companion authority.
    /// # Errors
    /// Refuses unrelated original axes, stale occupations and exhausted resources.
    pub fn transport_gather_mask(
        &mut self,
        source: MaskVersion,
        target: CellLayoutVersion,
        budget: &mut Budget,
    ) -> Result<MaskVersion, MaskError> {
        budget.with_work_phase(super::FAMILY, "mask gather transport", |budget| {
            charge(budget, 2)?;
            let input = self.mask(source, source.layout())?;
            let destination = self.masks.layout(self.scope, target)?;
            if original_axis(input.axis(), input.gather())
                != original_axis(destination.axis, destination.gather.as_ref())
            {
                return Err(MaskError::LayoutMismatch);
            }
            let capacity = destination.axis.capacity();
            let reservation = memory::Reservation::new(
                self,
                None,
                Some(word_count(capacity)),
                0,
                [&[], &[]],
                budget,
            )?;
            reservation.run(budget, |budget| {
                let mut words = memory::zeroed(word_count(capacity), budget)?;
                let input = self.mask(source, source.layout())?;
                let destination = self.masks.layout(self.scope, target)?;
                let associated = destination
                    .gather
                    .as_ref()
                    .map_or(capacity, |map| map.indices.len());
                for cell in 0..associated {
                    charge(budget, 1)?;
                    let original = match &destination.gather {
                        Some(map) => *map.indices.get(cell).ok_or(MaskError::UnmappedCell)?,
                        None => cell,
                    };
                    if let Some(source_cell) = source_cell(original, input.gather(), budget)?
                        && input.contains(source_cell)?
                    {
                        let word = words.get_mut(cell / 64).ok_or(MaskError::CellOutOfBounds)?;
                        *word |= 1_u64 << (cell % 64);
                    }
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

/// A compacted layout retains its root association independently of its numerical parents.
const fn original_axis(axis: CellAxis, gather: Option<&CellGather>) -> CellAxis {
    match gather {
        Some(map) => map.axis,
        None => axis,
    }
}

/// Every comparison in the bounded search of an ordered injective map consumes Work.
fn source_cell(
    original: usize,
    gather: Option<&CellGather>,
    budget: &mut Budget,
) -> Result<Option<usize>, MaskError> {
    let Some(map) = gather else {
        return Ok(Some(original));
    };
    let mut low = 0;
    let mut high = map.indices.len();
    while low < high {
        charge(budget, 1)?;
        let middle = low + (high - low) / 2;
        let value = *map.indices.get(middle).ok_or(MaskError::UnmappedCell)?;
        match value.cmp(&original) {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle,
            std::cmp::Ordering::Equal => return Ok(Some(middle)),
        }
    }
    Ok(None)
}
