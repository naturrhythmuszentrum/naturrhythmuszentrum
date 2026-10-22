// SPDX-License-Identifier: AGPL-3.0-only
//! Indexed grid selections preserve one exact family and its source lineage.

use super::{PhysicalReading, ReadingAxis, ReadingError, ReadingProjection, support};
use nrz_cas::{Cas, limits::Budget};

/// One indexed physical column with an exact structural axis and fixed entry meaning.
#[derive(Clone, Debug)]
pub struct IndexedReading {
    /// Complete grid reading and shared source coordinate family.
    reading: PhysicalReading,
    /// Exact structural index, order and partition.
    axis: ReadingAxis,
}

impl IndexedReading {
    /// Check the declared extent, component order and common physical coordinate meaning.
    /// # Errors
    /// Refuses a noncolumn grid, wrong axis scope/extent or inconsistent per-entry meaning.
    pub fn new(
        reading: PhysicalReading,
        axis: ReadingAxis,
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        support::capacity::<Self>(1, budget)?;
        let [rows, columns] = support::shape(reading.sort())?;
        if columns != 1
            || usize::try_from(rows).map_err(|_| ReadingError::Shape)? != axis.order().len()
        {
            return Err(ReadingError::Shape);
        }
        if let Some(first) = reading.entries().first() {
            for entry in reading.entries() {
                budget.work(1, support::AT)?;
                first.check(entry)?;
            }
        }
        if axis.context() != reading.family().context() {
            return Err(ReadingError::Definition);
        }
        Ok(Self { reading, axis })
    }
    /// Project one declared grid position without creating a new source declaration.
    /// # Errors
    /// Refuses out-of-range indices and exhausted exact selection resources.
    pub fn project(
        &self,
        cas: &mut Cas,
        position: usize,
        budget: &mut Budget,
    ) -> Result<ReadingProjection, ReadingError> {
        let coordinate = self.axis.order().get(position).ok_or(ReadingError::Shape)?;
        self.reading.project(cas, &[*coordinate], budget)
    }
    /// Match the exact axis rather than accepting equal extents or display names.
    /// # Errors
    /// Refuses changed declaration, order, extent or block partition.
    pub fn check_axis(&self, actual: &ReadingAxis) -> Result<(), ReadingError> {
        if &self.axis == actual {
            Ok(())
        } else {
            Err(ReadingError::Partition)
        }
    }
    /// Complete indexed symbol reading.
    #[must_use]
    pub const fn reading(&self) -> &PhysicalReading {
        &self.reading
    }
    /// Exact index identity and point order.
    #[must_use]
    pub const fn axis(&self) -> &ReadingAxis {
        &self.axis
    }
}
