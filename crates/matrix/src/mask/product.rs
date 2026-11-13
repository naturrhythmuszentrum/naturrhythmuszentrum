// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded Cartesian cell associations retain their ordered factor identities.

use super::{
    CellAxis, CellLayoutVersion, MaskError, MaskVersion, charge, memory, storage::MaskRecord,
    word_count,
};
use crate::{Heap, HeapError, heap::identity};
use nrz_bindings::Budget;

/// One ordered Cartesian pair, composable into instance, sample and branch products.
///
/// This native map carries no physical independence or numerical broadcast authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellProduct {
    /// Fresh product identity and checked logical capacity.
    axis: CellAxis,
    /// Original outer and inner axes, in row-major order.
    factors: [CellAxis; 2],
}

/// The factor whose native coverage is expanded over a product.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CellFactor {
    /// Outer coordinate, repeated over the complete inner axis.
    Outer,
    /// Inner coordinate, repeated for each outer coordinate.
    Inner,
}
impl CellFactor {
    /// Ordered factor slot.
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Outer => 0,
            Self::Inner => 1,
        }
    }
}
impl CellProduct {
    /// Exact ordered product axis used by component layouts.
    #[must_use]
    pub const fn axis(self) -> CellAxis {
        self.axis
    }
    /// Original ordered factors, including their owner identities.
    #[must_use]
    pub const fn factors(self) -> [CellAxis; 2] {
        self.factors
    }
    /// Recover the original factor coordinates from one row-major product cell.
    /// # Errors
    /// Refuses every index outside the product, including any index of an empty product.
    pub const fn coordinates(self, cell: usize) -> Result<[usize; 2], MaskError> {
        if cell >= self.axis.capacity || self.factors[1].capacity == 0 {
            return Err(MaskError::CellOutOfBounds);
        }
        Ok([
            cell / self.factors[1].capacity,
            cell % self.factors[1].capacity,
        ])
    }
}
impl Heap {
    /// Declare a checked row-major product without allocating metadata per cell.
    /// # Errors
    /// Refuses foreign factors, capacity overflow, identity exhaustion and exhausted Work.
    pub fn create_cell_product(
        &self,
        outer: CellAxis,
        inner: CellAxis,
        budget: &mut Budget,
    ) -> Result<CellProduct, MaskError> {
        budget.with_work_phase(super::FAMILY, "cell product", |budget| {
            charge(budget, 3)?;
            if outer.scope != self.scope || inner.scope != self.scope {
                return Err(HeapError::ForeignHandle.into());
            }
            let capacity = outer
                .capacity
                .checked_mul(inner.capacity)
                .ok_or(MaskError::CountOverflow)?;
            Ok(CellProduct {
                axis: CellAxis {
                    scope: self.scope,
                    identity: identity::identity()?,
                    capacity,
                },
                factors: [outer, inner],
            })
        })
    }

    /// Pull one factor's coverage back onto a checked product component layout.
    ///
    /// Every target cell is selected exactly when its projected source cell is selected.
    /// Numerical broadcasts still require their own source and companion laws; this
    /// operation publishes only native coverage. Gathered layouts are not admitted.
    /// # Errors
    /// Refuses unrelated axes, stale versions, gathered layouts and exhausted resources.
    pub fn expand_product_mask(
        &mut self,
        product: CellProduct,
        factor: CellFactor,
        source: MaskVersion,
        target: CellLayoutVersion,
        budget: &mut Budget,
    ) -> Result<MaskVersion, MaskError> {
        budget.with_work_phase(super::FAMILY, "mask product expansion", |budget| {
            charge(budget, 3)?;
            let input = self.mask(source, source.layout())?;
            let destination = self.masks.layout(self.scope, target)?;
            if input.axis() != product.factors[factor.index()] || destination.axis != product.axis {
                return Err(MaskError::LayoutMismatch);
            }
            if input.gather().is_some() || destination.gather.is_some() {
                return Err(MaskError::NonIdentityCorrespondence);
            }
            let capacity = product.axis.capacity;
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
                for cell in 0..capacity {
                    charge(budget, 1)?;
                    let original = product.coordinates(cell)?[factor.index()];
                    if input.contains(original)? {
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
