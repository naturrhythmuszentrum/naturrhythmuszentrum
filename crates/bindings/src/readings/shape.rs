// SPDX-License-Identifier: AGPL-3.0-only
//! Checked ordered axes and total rectangular partitions retain actual CAS identities.

use super::{PhysicalReading, ReadingError, support};
use nrz_cas::{
    Cas, Handle,
    limits::Budget,
    readings::{EntrySelection, SelectionSpec},
    syntax::{AtomicSort, Operator, Sort},
};

/// An ordered structural index declaration, with explicit contiguous block boundaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadingAxis {
    /// Actual CAS structural index declaration.
    node: Handle,
    /// Declaration scope and admissible family.
    context: Handle,
    /// Exact coordinate permutation of the axis.
    order: Vec<u32>,
    /// Exclusive ends of the complete contiguous partition.
    boundaries: Vec<u32>,
}

impl ReadingAxis {
    /// Bind one declared integer index to an exact permutation and a complete partition.
    ///
    /// Boundaries are exclusive ends, strictly increasing and ending at the extent.
    /// Empty axes require empty order and boundaries; no physical count is invented.
    /// # Errors
    /// Refuses a foreign/noninteger index, repeated order, omitted entries or malformed blocks.
    pub fn new(
        cas: &Cas,
        context: &Handle,
        index: &Handle,
        order: Vec<u32>,
        boundaries: Vec<u32>,
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        let (sort, operator, _, header) = support::inspect(cas, index, budget)?;
        if sort != Sort::Value(AtomicSort::Integer)
            || !matches!(operator, Operator::Free(_))
            || header != cas.context_header(context)?
        {
            return Err(ReadingError::Definition);
        }
        let extent = u32::try_from(order.len()).map_err(|_| ReadingError::Shape)?;
        support::capacity::<u32>(order.len() + boundaries.len(), budget)?;
        for (position, entry) in order.iter().enumerate() {
            budget.work(
                u64::try_from(position + 1).map_err(|_| ReadingError::Shape)?,
                support::AT,
            )?;
            if *entry >= extent || order[..position].contains(entry) {
                return Err(ReadingError::Partition);
            }
        }
        let mut previous = 0;
        for boundary in &boundaries {
            if *boundary <= previous || *boundary > extent {
                return Err(ReadingError::Partition);
            }
            previous = *boundary;
        }
        if previous != extent {
            return Err(ReadingError::Partition);
        }
        Ok(Self {
            node: index.clone(),
            context: context.clone(),
            order,
            boundaries,
        })
    }
    /// Exact declaration scope and admissible family domain.
    #[must_use]
    pub const fn context(&self) -> &Handle {
        &self.context
    }
    /// Actual structural index symbol.
    #[must_use]
    pub const fn node(&self) -> &Handle {
        &self.node
    }
    /// Exact order; equal extents alone do not identify axes.
    #[must_use]
    pub fn order(&self) -> &[u32] {
        &self.order
    }
    /// Complete ordered block boundaries.
    #[must_use]
    pub fn boundaries(&self) -> &[u32] {
        &self.boundaries
    }
}

/// Shape metadata checked against one exact matrix reading, including a total block partition.
#[derive(Clone, Debug)]
pub struct ReadingShape {
    /// Matrix reading named by this shape record.
    reading: PhysicalReading,
    /// Ordered row and column axis identities.
    axes: [ReadingAxis; 2],
    /// Checked total partition of the actual reading entries.
    blocks: Vec<EntrySelection>,
}

impl ReadingShape {
    /// Check both ordered axes and every block before retaining shape evidence.
    /// # Errors
    /// Refuses wrong scopes/extents, nonrectangular blocks, overlap or incomplete coverage.
    pub fn new(
        reading: &PhysicalReading,
        axes: [ReadingAxis; 2],
        blocks: &[SelectionSpec],
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        let extent = support::shape(reading.sort())?;
        for (axis, count) in axes.iter().zip(extent) {
            let expected = usize::try_from(count).map_err(|_| ReadingError::Shape)?;
            if axis.context != *reading.family().context() || axis.order.len() != expected {
                return Err(ReadingError::Shape);
            }
        }
        let mut covered = support::reserve(reading.entries().len(), budget)?;
        let mut checked = support::reserve(blocks.len(), budget)?;
        for block in blocks {
            if !matches!(block, SelectionSpec::Block { .. }) {
                return Err(ReadingError::Partition);
            }
            let selection = EntrySelection::new(reading.sort(), block, budget)?;
            for index in selection.indices() {
                budget.work(
                    u64::try_from(covered.len() + 1).map_err(|_| ReadingError::Shape)?,
                    support::AT,
                )?;
                if covered.contains(index) {
                    return Err(ReadingError::Partition);
                }
                covered.push(*index);
            }
            checked.push(selection);
        }
        if covered.len() != reading.entries().len() {
            return Err(ReadingError::Partition);
        }
        Ok(Self {
            reading: reading.clone(),
            axes,
            blocks: checked,
        })
    }
    /// Exact matrix reading whose entries are partitioned.
    #[must_use]
    pub const fn reading(&self) -> &PhysicalReading {
        &self.reading
    }
    /// Ordered row and column axes, retaining their declaration identities.
    #[must_use]
    pub const fn axes(&self) -> &[ReadingAxis; 2] {
        &self.axes
    }
    /// Checked complete nonoverlapping blocks.
    #[must_use]
    pub fn blocks(&self) -> &[EntrySelection] {
        &self.blocks
    }
}
