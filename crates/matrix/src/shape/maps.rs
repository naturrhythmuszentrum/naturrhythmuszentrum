// SPDX-License-Identifier: AGPL-3.0-only
//! Logical selections and explicit ordered axis alignment.

use super::{Axis, Partition, ShapeError, ShapeLimits};
use crate::Extent;
use nrz_units::Conversion;

/// A checked logical selection, independent of physical storage addresses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryMap {
    /// Original logical extent.
    source: Extent,
    /// Result logical extent.
    target: Extent,
    /// Source indices in target column order; repetitions retain identity.
    indices: Vec<usize>,
}

impl EntryMap {
    /// Check every selected entry; repeated inputs are permitted.
    ///
    /// # Errors
    /// Refuses a length mismatch, out-of-range index or work exhaustion.
    pub fn new(
        source: Extent,
        target: Extent,
        indices: Vec<usize>,
        limits: ShapeLimits,
    ) -> Result<Self, ShapeError> {
        limits.extent(source.rows(), source.columns())?;
        limits.extent(target.rows(), target.columns())?;
        if indices.len() != target.entries() {
            return Err(ShapeError::LengthMismatch);
        }
        if indices.iter().any(|index| *index >= source.entries()) {
            return Err(ShapeError::EntryIndex);
        }
        Ok(Self {
            source,
            target,
            indices,
        })
    }

    /// Select the complete source in column order, with explicitly changed extents.
    ///
    /// # Errors
    /// Refuses a changed entry count or exceeded limits.
    pub fn reshape(
        source: Extent,
        target: Extent,
        limits: ShapeLimits,
    ) -> Result<Self, ShapeError> {
        limits.extent(source.rows(), source.columns())?;
        if source.entries() != target.entries() {
            return Err(ShapeError::LengthMismatch);
        }
        Self::new(source, target, (0..source.entries()).collect(), limits)
    }

    /// Source extent in logical column order.
    #[must_use]
    pub const fn source(&self) -> Extent {
        self.source
    }

    /// Result extent in logical column order.
    #[must_use]
    pub const fn target(&self) -> Extent {
        self.target
    }

    /// Complete source-index selection.
    #[must_use]
    pub fn indices(&self) -> &[usize] {
        &self.indices
    }

    /// Compose a selection of this map's result back to the original logical entries.
    ///
    /// # Errors
    /// Refuses incompatible extents or exceeded limits.
    pub fn compose(&self, next: &Self, limits: ShapeLimits) -> Result<Self, ShapeError> {
        if self.target != next.source {
            return Err(ShapeError::LengthMismatch);
        }
        limits.check_work(next.indices.len())?;
        let indices = next
            .indices
            .iter()
            .map(|index| {
                self.indices
                    .get(*index)
                    .copied()
                    .ok_or(ShapeError::EntryIndex)
            })
            .collect::<Result<_, _>>()?;
        Self::new(self.source, next.target, indices, limits)
    }

    /// A transpose permutes full logical entries, regardless of the packed layout.
    pub(crate) fn transpose(source: Extent, limits: ShapeLimits) -> Result<Self, ShapeError> {
        let target = limits.extent(source.columns(), source.rows())?;
        let mut indices = Vec::with_capacity(target.entries());
        for column in 0..target.columns() {
            for row in 0..target.rows() {
                indices.push(source.index(column, row)?);
            }
        }
        Self::new(source, target, indices, limits)
    }
}

/// A bijection from target axis order to source order with exact conversion maps.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AxisAlignment {
    /// Axis whose order and measures the result adopts.
    target: Axis,
    /// Axis being explicitly aligned.
    source: Axis,
    /// Source entry for each target coordinate.
    indices: Vec<usize>,
    /// Source coordinate to target difference-coordinate conversions.
    conversions: Vec<Conversion>,
    /// Common refinement in target order, including permuted source segment changes.
    refinement: Partition,
}

impl AxisAlignment {
    /// Check a permutation by coordinate identities, kinds, roles and exact measure maps.
    ///
    /// # Errors
    /// Refuses repetitions, omissions, different identities or incompatible descriptors.
    pub fn check(
        target: &Axis,
        source: &Axis,
        indices: Vec<usize>,
        limits: ShapeLimits,
    ) -> Result<Self, ShapeError> {
        limits.check_work(indices.len())?;
        if target.entries.len() != source.entries.len() || indices.len() != target.entries.len() {
            return Err(ShapeError::AxisMismatch);
        }
        let mut seen = vec![false; indices.len()];
        let mut conversions = Vec::with_capacity(indices.len());
        for (entry, index) in target.entries.iter().zip(&indices) {
            let visited = seen.get_mut(*index).ok_or(ShapeError::AxisMismatch)?;
            if *visited {
                return Err(ShapeError::AxisMismatch);
            }
            *visited = true;
            conversions
                .push(entry.conversion_from(
                    source.entries.get(*index).ok_or(ShapeError::AxisMismatch)?,
                )?);
        }
        let refinement = aligned_partition(target, source, &indices, limits)?;
        Ok(Self {
            target: target.clone(),
            source: source.clone(),
            indices,
            conversions,
            refinement,
        })
    }

    /// Check explicit identity-order alignment; this still checks coordinate identities.
    ///
    /// # Errors
    /// Refuses incompatible axes or work exhaustion.
    pub fn identity(target: &Axis, source: &Axis, limits: ShapeLimits) -> Result<Self, ShapeError> {
        limits.check_work(target.entries.len())?;
        Self::check(target, source, (0..target.entries.len()).collect(), limits)
    }

    /// Source coordinate indices in target order.
    #[must_use]
    pub fn indices(&self) -> &[usize] {
        &self.indices
    }

    /// Explicit exact difference conversions; no numerical application is implied.
    #[must_use]
    pub fn conversions(&self) -> &[Conversion] {
        &self.conversions
    }

    /// Result partition in the target coordinate order.
    #[must_use]
    pub const fn refinement(&self) -> &Partition {
        &self.refinement
    }

    /// Prevent reuse of an alignment with different endpoint axes or partitions.
    pub(crate) fn validate(&self, target: &Axis, source: &Axis) -> Result<(), ShapeError> {
        if &self.target == target && &self.source == source {
            Ok(())
        } else {
            Err(ShapeError::AxisMismatch)
        }
    }
}

/// A permutation can split source segments; every change becomes a result boundary.
fn aligned_partition(
    target: &Axis,
    source: &Axis,
    indices: &[usize],
    limits: ShapeLimits,
) -> Result<Partition, ShapeError> {
    let mut boundaries = vec![0];
    for (position, adjacent) in indices.windows(2).enumerate() {
        let segment = |index: usize| {
            source
                .partition
                .boundaries()
                .partition_point(|edge| *edge <= index)
        };
        if segment(adjacent[0]) != segment(adjacent[1]) {
            boundaries.push(position + 1);
        }
    }
    if !indices.is_empty() {
        boundaries.push(indices.len());
    }
    let permuted = Partition::new(indices.len(), boundaries, limits)?;
    target.partition.refine(&permuted, limits)
}
