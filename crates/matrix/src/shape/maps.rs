// SPDX-License-Identifier: AGPL-3.0-only
//! Logical selections and explicit ordered axis alignment.

use super::{Axis, Partition, ShapeError, ShapeLimits};
use crate::Extent;
use nrz_bindings::Conversion;

/// A checked logical selection, independent of physical storage addresses.
#[derive(Clone, Debug)]
pub struct EntryMap {
    /// Original logical extent.
    source: Extent,
    /// Result logical extent.
    target: Extent,
    /// Source indices in target column order; repetitions retain identity.
    indices: Vec<usize>,
    /// Native repeated-cell projection needs no entry-sized descriptor.
    product: Option<(crate::CellProduct, crate::CellFactor)>,
    /// Shared native selection metadata keeps its original occupation protected.
    selection: Option<std::rc::Rc<crate::mask::storage::LayoutRecord>>,
}

impl PartialEq for EntryMap {
    fn eq(&self, other: &Self) -> bool {
        let selection = match (&self.selection, &other.selection) {
            (Some(left), Some(right)) => std::rc::Rc::ptr_eq(left, right),
            (None, None) => true,
            _ => false,
        };
        self.source == other.source
            && self.target == other.target
            && self.indices == other.indices
            && self.product == other.product
            && selection
    }
}
impl Eq for EntryMap {}

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
            product: None,
            selection: None,
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

    /// Construct a component-preserving native product projection.
    /// # Errors
    /// Refuses an invalid extent or exceeded shape limits.
    pub fn product_cells(
        product: crate::CellProduct,
        factor: crate::CellFactor,
        components: usize,
        limits: ShapeLimits,
    ) -> Result<Self, ShapeError> {
        let source = limits.extent(product.factors()[factor.index()].capacity(), components)?;
        let target = limits.extent(product.axis().capacity(), components)?;
        Ok(Self {
            source,
            target,
            indices: Vec::new(),
            selection: None,
            product: Some((product, factor)),
        })
    }

    /// Native product association, when the descriptor preserves complete cells.
    pub(crate) const fn product(&self) -> Option<(crate::CellProduct, crate::CellFactor)> {
        self.product
    }

    /// Source index at one checked target entry without enumerating preceding entries.
    #[must_use]
    pub fn source_index(&self, entry: usize) -> Option<usize> {
        if entry >= self.target.entries() {
            return None;
        }
        if self.product.is_none() && self.selection.is_none() {
            return self.indices.get(entry).copied();
        }
        let original = self.source_cell(entry % self.target.rows())?;
        self.source.index(original, entry / self.target.rows()).ok()
    }

    /// Native cell projection composes original-index gathering before product coordinates.
    pub(crate) fn source_cell(&self, cell: usize) -> Option<usize> {
        if cell >= self.target.rows() {
            return None;
        }
        let cell = self
            .selection
            .as_ref()
            .and_then(|layout| layout.gather.as_ref())
            .map_or(Some(cell), |gather| gather.indices().get(cell).copied())?;
        match self.product {
            Some((product, factor)) => Some(product.coordinates(cell).ok()?[factor.index()]),
            None => Some(cell),
        }
    }

    /// Borrow the shared native selection for constant-size authentication.
    pub(crate) fn selection(&self) -> Option<&crate::mask::storage::LayoutRecord> {
        self.selection.as_deref()
    }

    /// Retain already checked native metadata without copying its original-index inventory.
    pub(crate) fn selected_cells(
        source: Extent,
        selection: std::rc::Rc<crate::mask::storage::LayoutRecord>,
        product: Option<(crate::CellProduct, crate::CellFactor)>,
    ) -> Result<Self, ShapeError> {
        let target = Extent::new(selection.axis.capacity(), source.columns())?;
        let original = selection
            .gather
            .as_ref()
            .map_or(selection.axis, crate::CellGather::original_axis);
        let source_rows = product.map_or_else(
            || original.capacity(),
            |(product, factor)| product.factors()[factor.index()].capacity(),
        );
        let empty_source_padding = source.rows() == 0 && target.entries() != 0;
        if empty_source_padding
            || source.rows() != source_rows
            || selection.components.extent().columns() != source.columns()
            || product.is_some_and(|(product, _)| product.axis() != original)
        {
            return Err(ShapeError::LengthMismatch);
        }
        Ok(Self {
            source,
            target,
            indices: Vec::new(),
            product,
            selection: Some(selection),
        })
    }

    /// Unmapped native padding requires a mask-aware consumer rather than dense evaluation.
    pub(crate) fn has_padding(&self) -> bool {
        self.selection
            .as_ref()
            .and_then(|layout| layout.gather.as_ref())
            .is_some_and(|gather| gather.indices().len() != self.target.rows())
    }

    /// Enumerate source indices without allocating a descriptor for native products.
    /// # Errors
    /// Refuses native padding; each yielded position retains any source-index refusal.
    pub fn indices(
        &self,
    ) -> Result<impl Iterator<Item = Result<usize, ShapeError>> + '_, ShapeError> {
        if self.has_padding() {
            return Err(ShapeError::UnmappedEntries);
        }
        Ok((0..self.target.entries())
            .map(|entry| self.source_index(entry).ok_or(ShapeError::EntryIndex)))
    }

    /// Compose a selection of this map's result back to the original logical entries.
    ///
    /// # Errors
    /// Refuses native padding, incompatible extents or exceeded limits.
    pub fn compose(&self, next: &Self, limits: ShapeLimits) -> Result<Self, ShapeError> {
        if self.has_padding() || next.has_padding() {
            return Err(ShapeError::UnmappedEntries);
        }
        if self.target != next.source {
            return Err(ShapeError::LengthMismatch);
        }
        limits.check_work(next.target.entries())?;
        let indices = next
            .indices()?
            .map(|index| self.source_index(index?).ok_or(ShapeError::EntryIndex))
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
