// SPDX-License-Identifier: AGPL-3.0-only
//! Ordered projections make partial overlaps distinct expected claims.

use super::{EntryMeaning, PhysicalReading, ReadingError, support};
use nrz_cas::{
    Cas, Handle,
    limits::Budget,
    readings::{EntryIndex, EntryOrder, SelectionSpec},
    syntax::{AtomicSort, Field, Operator, Sort},
};

/// A checked ordered component projection of one physical reading.
#[derive(Clone, Debug)]
pub struct ReadingProjection {
    /// Complete original reading behind the projection.
    pub(super) reading: PhysicalReading,
    /// Ordered result-coordinate indices, without duplicates.
    pub(super) coordinates: Vec<u32>,
    /// Actual CAS expression for this exact projection.
    pub(super) node: Handle,
    /// Physical meanings of the projected coordinates.
    pub(super) entries: Vec<EntryMeaning>,
}

impl PhysicalReading {
    /// Select exact result coordinates for an overlap claim.
    /// # Errors
    /// Refuses duplicate/out-of-range components and unsupported Complex projections.
    pub fn project(
        &self,
        cas: &mut Cas,
        coordinates: &[u32],
        budget: &mut Budget,
    ) -> Result<ReadingProjection, ReadingError> {
        let mut entries = support::reserve(coordinates.len(), budget)?;
        let mut ordered = support::reserve(coordinates.len(), budget)?;
        for coordinate in coordinates {
            budget.work(
                u64::try_from(ordered.len()).map_err(|_| ReadingError::Shape)?,
                support::AT,
            )?;
            if ordered.contains(coordinate) {
                return Err(ReadingError::Shape);
            }
            entries.push(
                self.entries
                    .get(usize::try_from(*coordinate).map_err(|_| ReadingError::Shape)?)
                    .ok_or(ReadingError::Shape)?
                    .clone(),
            );
            ordered.push(*coordinate);
        }
        let node = projection_node(self, cas, coordinates, budget)?;
        Ok(ReadingProjection {
            reading: self.clone(),
            coordinates: ordered,
            node,
            entries,
        })
    }
}

impl ReadingProjection {
    /// Exact original reading, including its source selection.
    #[must_use]
    pub const fn reading(&self) -> &PhysicalReading {
        &self.reading
    }
    /// Ordered result coordinates included in this claim.
    #[must_use]
    pub fn coordinates(&self) -> &[u32] {
        &self.coordinates
    }
    /// Actual CAS projection expression used by the equality checker.
    #[must_use]
    pub const fn node(&self) -> &Handle {
        &self.node
    }
    /// Exact physical meaning of every projected entry.
    #[must_use]
    pub fn entries(&self) -> &[EntryMeaning] {
        &self.entries
    }
}

/// Use native Real/Imag or finite selection syntax for the ordered physical projection.
fn projection_node(
    reading: &PhysicalReading,
    cas: &mut Cas,
    coordinates: &[u32],
    budget: &mut Budget,
) -> Result<Handle, ReadingError> {
    match (reading.sort(), coordinates) {
        (Sort::Value(AtomicSort::Scalar(Field::Real)), [0])
        | (Sort::Value(AtomicSort::Scalar(Field::Complex)), [0, 1]) => Ok(reading.node.clone()),
        (Sort::Value(AtomicSort::Scalar(Field::Complex)), [coordinate @ (0 | 1)]) => {
            let operator = if *coordinate == 0 {
                Operator::RealPart
            } else {
                Operator::ImagPart
            };
            support::expression(
                cas,
                reading.family.context(),
                operator,
                Sort::Value(AtomicSort::Scalar(Field::Real)),
                std::slice::from_ref(reading.node()),
                budget,
            )
        }
        (sort, _) => {
            let [rows, _] = support::shape(sort)?;
            let mut indices = support::reserve(coordinates.len(), budget)?;
            for coordinate in coordinates {
                if rows == 0 {
                    return Err(ReadingError::Shape);
                }
                indices.push(EntryIndex {
                    row: coordinate % rows,
                    column: coordinate / rows,
                });
            }
            let selection = if let [entry] = indices.as_slice() {
                SelectionSpec::Entry(*entry)
            } else {
                SelectionSpec::Entries {
                    entries: indices,
                    order: EntryOrder::Given,
                }
            };
            Ok(cas.select_entries(reading.node(), &selection, budget)?)
        }
    }
}
