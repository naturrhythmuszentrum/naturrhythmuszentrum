// SPDX-License-Identifier: AGPL-3.0-only
//! Signed affine arithmetic and its two extreme-address proof.

use crate::{Extent, HeapError};

/// A checked nonempty affine map, with no unchecked constructor outside this module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Affine {
    /// Relative address at logical row zero and column zero.
    offset: isize,
    /// Signed element step between rows.
    row_stride: isize,
    /// Signed element step between columns.
    column_stride: isize,
}

impl Affine {
    /// Check all signed conversions, both extreme products and both extreme sums.
    pub(crate) fn check(
        extent: Extent,
        region_entries: usize,
        offset: isize,
        row_stride: isize,
        column_stride: isize,
    ) -> Result<Option<Self>, HeapError> {
        let rows = isize::try_from(extent.rows()).map_err(|_| HeapError::ExtentOverflow)?;
        let columns = isize::try_from(extent.columns()).map_err(|_| HeapError::ExtentOverflow)?;
        isize::try_from(extent.entries()).map_err(|_| HeapError::ExtentOverflow)?;
        let length = isize::try_from(region_entries).map_err(|_| HeapError::ExtentOverflow)?;
        if extent.entries() == 0 {
            return Ok(None);
        }
        let row_end = (rows - 1)
            .checked_mul(row_stride)
            .ok_or(HeapError::ExtentOverflow)?;
        let column_end = (columns - 1)
            .checked_mul(column_stride)
            .ok_or(HeapError::ExtentOverflow)?;
        let minimum = offset
            .checked_add(row_end.min(0))
            .and_then(|value| value.checked_add(column_end.min(0)))
            .ok_or(HeapError::ExtentOverflow)?;
        let maximum = offset
            .checked_add(row_end.max(0))
            .and_then(|value| value.checked_add(column_end.max(0)))
            .ok_or(HeapError::ExtentOverflow)?;
        if minimum < 0 || maximum >= length {
            return Err(HeapError::InvalidLayout);
        }
        Ok(Some(Self {
            offset,
            row_stride,
            column_stride,
        }))
    }

    /// Repeat the proof when checking an affected physical indirection.
    pub(crate) fn validate(self, extent: Extent, region_entries: usize) -> Result<(), HeapError> {
        Self::check(
            extent,
            region_entries,
            self.offset,
            self.row_stride,
            self.column_stride,
        )?
        .ok_or(HeapError::InvalidLayout)?;
        Ok(())
    }

    /// Compute a checked address after the caller has checked logical bounds.
    pub(crate) fn index(self, row: usize, column: usize) -> Result<usize, HeapError> {
        let row = isize::try_from(row).map_err(|_| HeapError::ExtentOverflow)?;
        let column = isize::try_from(column).map_err(|_| HeapError::ExtentOverflow)?;
        let across = row
            .checked_mul(self.row_stride)
            .ok_or(HeapError::ExtentOverflow)?;
        let down = column
            .checked_mul(self.column_stride)
            .ok_or(HeapError::ExtentOverflow)?;
        let address = self
            .offset
            .checked_add(across)
            .and_then(|start| start.checked_add(down))
            .ok_or(HeapError::ExtentOverflow)?;
        usize::try_from(address).map_err(|_| HeapError::InvalidLayout)
    }

    /// Constant-time proof that vectorization itself is the map.
    pub(crate) fn is_identity(self, extent: Extent) -> bool {
        self.offset == 0
            && (extent.rows() <= 1 || self.row_stride == 1)
            && (extent.columns() <= 1 || usize::try_from(self.column_stride) == Ok(extent.rows()))
    }
}
