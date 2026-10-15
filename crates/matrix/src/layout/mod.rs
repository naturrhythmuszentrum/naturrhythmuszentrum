// SPDX-License-Identifier: AGPL-3.0-only
//! Checked maps from logical column order into one owning region.

mod affine;
mod packing;

pub use packing::PackedRegion;

use crate::{Extent, HeapError};
use affine::Affine;

/// Immutable checked entry selection, relative to its owning region's first element.
///
/// Repeated addresses are permitted for reads. Construction does not authorize writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Layout {
    /// Logical output rectangle.
    extent: Extent,
    /// Exact length of the region into which this map was checked.
    region_entries: usize,
    /// Checked addressing rule; empty maps carry no address.
    map: Map,
}

/// Closed set of address maps, inaccessible to callers.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Map {
    /// No logical entry can be addressed.
    Empty,
    /// Signed two-axis affine addressing.
    Affine(Affine),
    /// Explicit offsets in logical column order.
    Gather(Vec<usize>),
}

impl Layout {
    /// Construct the full contiguous column-ordered map for an extent.
    ///
    /// # Errors
    /// Refuses extents or addresses that cannot be represented in signed arithmetic.
    pub fn contiguous(extent: Extent) -> Result<Self, HeapError> {
        let stride = isize::try_from(extent.rows()).map_err(|_| HeapError::ExtentOverflow)?;
        Self::strided(extent, extent.entries(), 0, 1, stride)
    }

    /// Check `offset + i*row_stride + j*column_stride` inside a region of the given length.
    ///
    /// Empty maps ignore the offset and strides and admit no addressable entry.
    ///
    /// # Errors
    /// Refuses signed conversion, product or sum overflow, or either extreme outside
    /// the owning region. Slab capacity never substitutes for the region length.
    pub fn strided(
        extent: Extent,
        region_entries: usize,
        offset: isize,
        row_stride: isize,
        column_stride: isize,
    ) -> Result<Self, HeapError> {
        let affine = Affine::check(extent, region_entries, offset, row_stride, column_stride)?;
        Ok(Self {
            extent,
            region_entries,
            map: affine.map_or(Map::Empty, Map::Affine),
        })
    }

    /// Validate and copy every explicit index, in logical column order.
    ///
    /// # Errors
    /// Refuses length mismatch, out-of-region indices, allocation failure or more
    /// than `work_limit` entries. Repeated indices are valid read selections.
    pub fn gather(
        extent: Extent,
        region_entries: usize,
        indices: &[usize],
        work_limit: usize,
    ) -> Result<Self, HeapError> {
        if indices.len() != extent.entries() {
            return Err(HeapError::LengthMismatch);
        }
        check_work(indices.len(), work_limit)?;
        let mut offsets = Vec::new();
        offsets
            .try_reserve_exact(indices.len())
            .map_err(|_| HeapError::Capacity)?;
        for &index in indices {
            if index >= region_entries {
                return Err(HeapError::InvalidLayout);
            }
            offsets.push(index);
        }
        Ok(Self {
            extent,
            region_entries,
            map: if offsets.is_empty() {
                Map::Empty
            } else {
                Map::Gather(offsets)
            },
        })
    }

    /// Logical extents of this selection.
    #[must_use]
    pub const fn extent(&self) -> Extent {
        self.extent
    }

    /// Exact owning-region length against which the map was checked.
    #[must_use]
    pub const fn region_entries(&self) -> usize {
        self.region_entries
    }

    /// Resolve one logical entry to its offset within the owning region.
    ///
    /// # Errors
    /// Refuses out-of-range logical indices or checked address arithmetic failure.
    pub fn index(&self, row: usize, column: usize) -> Result<usize, HeapError> {
        self.flat_index(self.extent.index(row, column)?)
    }

    /// Compose a selection of this view's logical entries, retaining base-region offsets.
    ///
    /// # Errors
    /// Refuses a mismatched intermediate extent, work exhaustion or allocation failure.
    /// General compositions enumerate at most `work_limit` output entries.
    pub fn compose(&self, selection: &Self, work_limit: usize) -> Result<Self, HeapError> {
        if selection.region_entries != self.extent.entries() {
            return Err(HeapError::InvalidLayout);
        }
        if self.is_identity() {
            return selection.copy_checked(self.region_entries, work_limit);
        }
        check_work(selection.extent.entries(), work_limit)?;
        let mut offsets = Vec::new();
        offsets
            .try_reserve_exact(selection.extent.entries())
            .map_err(|_| HeapError::Capacity)?;
        for logical in 0..selection.extent.entries() {
            offsets.push(self.flat_index(selection.flat_index(logical)?)?);
        }
        Ok(Self {
            extent: selection.extent,
            region_entries: self.region_entries,
            map: if offsets.is_empty() {
                Map::Empty
            } else {
                Map::Gather(offsets)
            },
        })
    }

    /// Revalidate the full map before resolving storage or preparing a relocation.
    pub(crate) fn validate(
        &self,
        region_entries: usize,
        work_limit: usize,
    ) -> Result<(), HeapError> {
        if self.region_entries != region_entries {
            return Err(HeapError::InvalidLayout);
        }
        match &self.map {
            Map::Empty if self.extent.entries() == 0 => Ok(()),
            Map::Empty => Err(HeapError::InvalidLayout),
            Map::Affine(affine) => affine.validate(self.extent, region_entries),
            Map::Gather(indices) => {
                check_work(indices.len(), work_limit)?;
                if indices.len() != self.extent.entries()
                    || indices.iter().any(|&index| index >= region_entries)
                {
                    return Err(HeapError::InvalidLayout);
                }
                Ok(())
            }
        }
    }

    /// Address one already column-vectorized logical entry, with checks retained.
    pub(crate) fn flat_index(&self, logical: usize) -> Result<usize, HeapError> {
        if logical >= self.extent.entries() {
            return Err(HeapError::InvalidLayout);
        }
        let index = match &self.map {
            Map::Empty => return Err(HeapError::InvalidLayout),
            Map::Affine(affine) => {
                affine.index(logical % self.extent.rows(), logical / self.extent.rows())?
            }
            Map::Gather(indices) => *indices.get(logical).ok_or(HeapError::InvalidLayout)?,
        };
        if index >= self.region_entries {
            return Err(HeapError::InvalidLayout);
        }
        Ok(index)
    }

    /// Copy a checked selection fallibly, bounding explicit-map allocation and scanning.
    fn copy_checked(&self, region_entries: usize, work_limit: usize) -> Result<Self, HeapError> {
        self.validate(region_entries, work_limit)?;
        match &self.map {
            Map::Gather(indices) => Self::gather(self.extent, region_entries, indices, work_limit),
            Map::Empty | Map::Affine(_) => Ok(self.clone()),
        }
    }

    /// Whether logical and owning-region offsets agree without any permutation.
    fn is_identity(&self) -> bool {
        self.region_entries == self.extent.entries()
            && match &self.map {
                Map::Empty => true,
                Map::Affine(affine) => affine.is_identity(self.extent),
                Map::Gather(_) => false,
            }
    }
}

/// Refuse enumeration before reserving its scratch storage.
pub(crate) const fn check_work(entries: usize, limit: usize) -> Result<(), HeapError> {
    if entries > limit {
        Err(HeapError::WorkLimit)
    } else {
        Ok(())
    }
}
