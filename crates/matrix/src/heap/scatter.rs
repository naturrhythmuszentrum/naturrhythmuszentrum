// SPDX-License-Identifier: AGPL-3.0-only
//! Selected writes are checked scatters into fresh whole-base scratch, then one publication.

use nrz_scalar::Scalar;

use super::publication::scan;
use super::{Handle, Heap, HeapError, PublishedBundle, Value, ValueVersion};
use crate::Layout;

impl Heap {
    /// Dispatch a base replacement or build complete base scratch from an injective alias.
    pub(crate) fn replace<P: Scalar + 'static>(
        &mut self,
        handle: Handle<Value, P>,
        expected: ValueVersion,
        centers: &[P],
        bounds: Option<&[P]>,
    ) -> Result<PublishedBundle<P>, HeapError> {
        let base = self.base_handle(handle)?;
        if base == handle {
            return self.replace_base(handle, expected, centers, bounds);
        }
        self.snapshot(base, Some(expected))?;
        let layout = &self.record(handle)?.layout;
        scan(layout.extent(), centers, bounds)?;
        self.validate_outputs(std::slice::from_ref(&handle))?;
        let mut whole_centers = self.value(base)?.region().export()?;
        scatter(layout, centers, &mut whole_centers)?;
        let whole_bounds = bounds
            .map(|entries| {
                let mut full = self.bounds_for_scatter(base, layout)?;
                scatter(layout, entries, &mut full)?;
                Ok::<_, HeapError>(full)
            })
            .transpose()?;
        let mut published =
            self.replace_base(base, expected, &whole_centers, whole_bounds.as_deref())?;
        published.value = handle;
        Ok(published)
    }

    /// Retain known bounds outside the selection; never invent missing physical coverage.
    fn bounds_for_scatter<P: Scalar + 'static>(
        &self,
        base: Handle<Value, P>,
        layout: &Layout,
    ) -> Result<Vec<P>, HeapError> {
        let value = self.value(base)?;
        if value.coverage() == super::Coverage::DeclaredBounds
            && let Some(bound) = value.bound()
        {
            return self.companion(bound)?.region().export();
        }
        // An injective map with this cardinality selects every base entry exactly once.
        if layout.extent().entries() != layout.region_entries() {
            return Err(HeapError::MissingCoverage);
        }
        let mut full = Vec::new();
        full.try_reserve_exact(layout.region_entries())
            .map_err(|_| HeapError::Capacity)?;
        full.resize(layout.region_entries(), P::ZERO);
        Ok(full)
    }
}

/// Copy through checked offsets only after output injectivity has been established.
fn scatter<P: Scalar>(layout: &Layout, selected: &[P], whole: &mut [P]) -> Result<(), HeapError> {
    if selected.len() != layout.extent().entries() || whole.len() != layout.region_entries() {
        return Err(HeapError::LengthMismatch);
    }
    for (logical, value) in selected.iter().enumerate() {
        let destination = whole
            .get_mut(layout.flat_index(logical)?)
            .ok_or(HeapError::InvalidLayout)?;
        *destination = *value;
    }
    Ok(())
}
