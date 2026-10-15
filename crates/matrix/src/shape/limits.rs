// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit descriptor work bounds, independent of numerical storage limits.

use super::ShapeError;

/// Per-check capacities for axes, logical entries, partitions and scalar work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShapeLimits {
    /// Largest admitted axis length.
    pub(crate) axis: usize,
    /// Largest logical entry count.
    pub(crate) entries: usize,
    /// Largest block or refinement count.
    pub(crate) blocks: usize,
    /// Maximum scalar checks or map entries in one operation.
    pub(crate) work: usize,
}

impl ShapeLimits {
    /// Set all capacities; zero is a meaningful refusal policy.
    #[must_use]
    pub const fn new(axis: usize, entries: usize, blocks: usize, work: usize) -> Self {
        Self {
            axis,
            entries,
            blocks,
            work,
        }
    }

    /// Check both axes and their product before allocating entry metadata.
    pub(crate) fn extent(self, rows: usize, columns: usize) -> Result<crate::Extent, ShapeError> {
        let extent = crate::Extent::new(rows, columns)?;
        if rows > self.axis || columns > self.axis || extent.entries() > self.entries {
            return Err(ShapeError::Limit);
        }
        self.check_work(extent.entries())?;
        Ok(extent)
    }

    /// Count checked scalar work before entering a loop.
    pub(crate) const fn check_work(self, count: usize) -> Result<(), ShapeError> {
        if count > self.work {
            Err(ShapeError::Limit)
        } else {
            Ok(())
        }
    }

    /// Bound the complete Cartesian refinement, not just each separate axis partition.
    pub(crate) fn partitions(
        self,
        domain: &super::Axis,
        codomain: &super::Axis,
    ) -> Result<(), ShapeError> {
        let columns = domain.partition.boundaries().len().saturating_sub(1);
        let rows = codomain.partition.boundaries().len().saturating_sub(1);
        let blocks = rows
            .checked_mul(columns)
            .ok_or(ShapeError::ExtentOverflow)?;
        if rows > self.blocks || columns > self.blocks || blocks > self.blocks {
            Err(ShapeError::Limit)
        } else {
            Ok(())
        }
    }
}

impl Default for ShapeLimits {
    fn default() -> Self {
        Self::new(4096, 1_048_576, 4096, 1_048_576)
    }
}
