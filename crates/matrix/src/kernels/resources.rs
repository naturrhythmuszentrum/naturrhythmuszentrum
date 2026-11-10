// SPDX-License-Identifier: AGPL-3.0-only
//! Every closed kernel inventories its simultaneous numerical owners before allocation.

use super::{KernelContract, arithmetic::Entry, contract::Recipe, product::ProductEntry};
use crate::{OperationError, heap::resources::slab_bytes};
use nrz_scalar::Scalar;

impl KernelContract {
    /// Candidate channels are already counted by `Reservations`; add only the excess peak.
    pub(crate) fn physical_scratch<P: Scalar, Q: Scalar>(
        &self,
        scalar_scratch: usize,
    ) -> Result<usize, OperationError> {
        match &self.recipe {
            Recipe::Product => self.product_scratch::<P, Q>(),
            Recipe::PreparedExpm1 => Err(super::KernelError::Contract.into()),
            Recipe::Linear(map) => {
                let coefficients = bytes(
                    map.coefficients.len(),
                    size_of::<super::lowering::Coefficient<P>>(),
                )?;
                let entries = bytes(self.output.entries(), size_of::<Entry<P>>())?;
                let candidate = bytes(self.output.entries(), 3 * size_of::<Q>())?;
                // Lowered coefficients coexist with Entry results, then drop before splitting.
                excess(
                    coefficients.checked_add(entries),
                    entries,
                    candidate,
                    scalar_scratch,
                )
            }
            Recipe::PositiveReciprocal(schedule) => {
                let literals = schedule
                    .references
                    .len()
                    .checked_add(schedule.coefficients.len())
                    .and_then(|count| count.checked_add(schedule.offsets.len()))
                    .ok_or(OperationError::ResourceLimit)?;
                let decoded = bytes(literals, size_of::<nrz_scalar::DecodedLiteral<P>>())?;
                let images = bytes(
                    schedule.references.len(),
                    size_of::<super::reciprocal::Image<P>>(),
                )?;
                let entries = bytes(self.output.entries(), size_of::<Entry<P>>())?;
                let candidate = bytes(self.output.entries(), 3 * size_of::<Q>())?;
                // All decoded definitions and reciprocal images survive the contracted entries.
                // They drop before those entries coexist with the three candidate channels.
                let contraction = decoded
                    .checked_add(images)
                    .and_then(|sum| sum.checked_add(entries));
                excess(contraction, entries, candidate, scalar_scratch)
            }
            Recipe::Sum { .. } | Recipe::Transform(_) | Recipe::Elementwise(_) => {
                // Entry results coexist with candidate channels; transform's optional three
                // contribution channels are also already counted by Reservations.
                Ok(scalar_scratch.max(bytes(self.output.entries(), size_of::<Entry<P>>())?))
            }
            Recipe::Quantity(_) => {
                // Product, quotient, integer power and square root use stack enclosures and
                // only the three admitted candidate vectors. Integer-only certificate records
                // have their separate quantity evidence admission before this common plan.
                Ok(scalar_scratch)
            }
        }
    }

    /// Product packing rounds each column's row capacity before multiplying by columns.
    fn product_scratch<P: Scalar, Q: Scalar>(&self) -> Result<usize, OperationError> {
        let mut packing = 0_usize;
        for extent in self.inputs.iter().chain(std::iter::once(&self.output)) {
            let bytes = slab_bytes::<P>(extent.rows())?
                .checked_mul(extent.columns())
                .ok_or(OperationError::ResourceLimit)?;
            packing = packing
                .checked_add(bytes)
                .ok_or(OperationError::ResourceLimit)?;
        }
        let entries = self.output.entries();
        let width = size_of::<P>();
        let candidate_width = 6 * size_of::<Q>();
        // Three backend matrices coexist with their copied centers. Verification then owns
        // centers and ProductEntry values; splitting owns ProductEntry, Entry and three terms;
        // final channel construction owns Entry and all six candidate channels.
        let mut peak = packing
            .checked_add(
                entries
                    .checked_mul(width)
                    .ok_or(OperationError::ResourceLimit)?,
            )
            .ok_or(OperationError::ResourceLimit)?;
        for width in [
            width + size_of::<ProductEntry<P>>(),
            size_of::<ProductEntry<P>>() + size_of::<Entry<P>>() + 3 * size_of::<Q>(),
            size_of::<Entry<P>>() + candidate_width,
        ] {
            peak = peak.max(
                entries
                    .checked_mul(width)
                    .ok_or(OperationError::ResourceLimit)?,
            );
        }
        // Reservations already include the six candidate vectors. Add only the excess peak
        // under ``rule:nrz:matrix-loans-legacy-preflight`` and rounded ``rule:nrz:matrix-loans-slab-capacity``.
        let candidate = entries
            .checked_mul(candidate_width)
            .ok_or(OperationError::ResourceLimit)?;
        peak.checked_sub(candidate)
            .ok_or(OperationError::ResourceLimit)
    }
}

/// Checked array capacity in the concrete numerical representation.
fn bytes(count: usize, width: usize) -> Result<usize, OperationError> {
    count
        .checked_mul(width)
        .ok_or(OperationError::ResourceLimit)
}

/// Compare contraction and channel construction without duplicating admitted candidate owners.
fn excess(
    contraction: Option<usize>,
    entries: usize,
    candidate: usize,
    scalar_scratch: usize,
) -> Result<usize, OperationError> {
    let channels = entries
        .checked_add(candidate)
        .ok_or(OperationError::ResourceLimit)?;
    let peak = contraction
        .ok_or(OperationError::ResourceLimit)?
        .max(channels);
    Ok(scalar_scratch.max(
        peak.checked_sub(candidate)
            .ok_or(OperationError::ResourceLimit)?,
    ))
}
