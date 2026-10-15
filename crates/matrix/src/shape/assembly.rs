// SPDX-License-Identifier: AGPL-3.0-only
//! Complete block coverage is checked before a shape can escape.

use super::{
    Axis, CheckedShape, EntryContract, EntryDraft, ShapeError, ShapeLimits, ShapeRecipe,
    StructureScope,
};
use nrz_units::{BinaryOp, CheckedDeclarations, Role};
use std::{ops::Range, rc::Rc};

/// One rectangular block whose entries are explicitly column ordered.
#[derive(Clone, Debug)]
pub struct BlockDraft {
    /// Global logical row range.
    rows: Range<usize>,
    /// Global logical column range.
    columns: Range<usize>,
    /// Every original entry, including typed zeros.
    entries: Vec<EntryDraft>,
}

impl BlockDraft {
    /// Propose a block; complete range and payload checking occurs in `ShapeDraft::check`.
    #[must_use]
    pub const fn new(rows: Range<usize>, columns: Range<usize>, entries: Vec<EntryDraft>) -> Self {
        Self {
            rows,
            columns,
            entries,
        }
    }
}

/// Unchecked ordered axes and block recipes, without numerical storage.
#[derive(Clone, Debug)]
pub struct ShapeDraft {
    /// Proposed input coordinates.
    domain: Axis,
    /// Proposed output coordinates.
    codomain: Axis,
    /// Complete proposed block table.
    blocks: Vec<BlockDraft>,
    /// Scope of supplied structural zeros.
    scope: StructureScope,
}

impl ShapeDraft {
    /// Propose a linear operator with explicitly scoped structure.
    #[must_use]
    pub const fn new(
        domain: Axis,
        codomain: Axis,
        blocks: Vec<BlockDraft>,
        scope: StructureScope,
    ) -> Self {
        Self {
            domain,
            codomain,
            blocks,
            scope,
        }
    }

    /// Check scalar roles, original recipes, every quotient and complete block coverage.
    ///
    /// The frozen owner checked even unused declarations before this call. Named
    /// originals resolve only in that owner; scalar descriptors do not substitute
    /// for original-expression checks. Empty maps require an empty block table.
    ///
    /// # Errors
    /// Refuses malformed axes, overlaps, gaps, wrong entry meanings or work exhaustion.
    pub fn check(
        self,
        declarations: Rc<CheckedDeclarations>,
        limits: ShapeLimits,
    ) -> Result<CheckedShape, ShapeError> {
        let extent = limits.extent(self.codomain.entries.len(), self.domain.entries.len())?;
        limits.partitions(&self.domain, &self.codomain)?;
        if self.blocks.len() > limits.blocks {
            return Err(ShapeError::Limit);
        }
        let work = self.blocks.iter().try_fold(0_usize, |total, block| {
            total
                .checked_add(block.entries.len())
                .ok_or(ShapeError::ExtentOverflow)
        })?;
        limits.check_work(work)?;
        check_axes(&self.domain, &self.codomain, &declarations)?;
        let mut entries = vec![None; extent.entries()];
        for block in &self.blocks {
            self.check_block(block, &declarations, extent, &mut entries)?;
        }
        let entries = entries
            .into_iter()
            .map(|entry| entry.ok_or(ShapeError::Partition))
            .collect::<Result<_, _>>()?;
        Ok(CheckedShape {
            domain: self.domain,
            codomain: self.codomain,
            extent,
            entries,
            declarations,
            scope: self.scope,
            recipe: ShapeRecipe::Assembly,
            parents: Vec::new(),
            maps: Vec::new(),
            alignments: Vec::new(),
        })
    }

    /// Validate a rectangle against both partitions before visiting its entries.
    fn check_block(
        &self,
        block: &BlockDraft,
        owner: &CheckedDeclarations,
        extent: crate::Extent,
        entries: &mut [Option<EntryContract>],
    ) -> Result<(), ShapeError> {
        if !self.codomain.partition.contains_range(&block.rows)
            || !self.domain.partition.contains_range(&block.columns)
        {
            return Err(ShapeError::Partition);
        }
        let height = block
            .rows
            .end
            .checked_sub(block.rows.start)
            .ok_or(ShapeError::Partition)?;
        let width = block
            .columns
            .end
            .checked_sub(block.columns.start)
            .ok_or(ShapeError::Partition)?;
        if height.checked_mul(width) != Some(block.entries.len()) {
            return Err(ShapeError::LengthMismatch);
        }
        for (local, draft) in block.entries.iter().enumerate() {
            let row = block.rows.start + local % height;
            let column = block.columns.start + local / height;
            let index = extent.index(row, column)?;
            let entry = EntryContract::check(
                draft,
                self.codomain
                    .entries
                    .get(row)
                    .ok_or(ShapeError::EntryIndex)?,
                self.domain
                    .entries
                    .get(column)
                    .ok_or(ShapeError::EntryIndex)?,
                owner,
                index,
            )?;
            insert_entry(entries, index, entry)?;
        }
        Ok(())
    }
}

/// Axis admission checks even coordinates of an empty operator.
pub(crate) fn check_axes(
    domain: &Axis,
    codomain: &Axis,
    owner: &CheckedDeclarations,
) -> Result<(), ShapeError> {
    for entry in domain.entries.iter().chain(&codomain.entries) {
        let quantity = entry.scalar()?;
        if quantity.role() != Role::Difference {
            return Err(ShapeError::LinearAxis);
        }
        owner.registry().binary(BinaryOp::Add, quantity, quantity)?;
        nrz_units::PairingWitness::sensitivity(quantity, quantity)?;
    }
    Ok(())
}

/// An occupied coverage slot cannot be overwritten by a later block.
fn insert_entry(
    entries: &mut [Option<EntryContract>],
    index: usize,
    entry: EntryContract,
) -> Result<(), ShapeError> {
    let slot = entries.get_mut(index).ok_or(ShapeError::EntryIndex)?;
    if slot.is_some() {
        return Err(ShapeError::Partition);
    }
    *slot = Some(entry);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::insert_entry;
    use crate::shape::EntryContract;
    use crate::{AxisEntry, ShapeError};
    use nrz_units::Descriptor;

    /// A rejected overlapping write retains the first private coverage witness.
    /// ´test:unit:shape-coverage-slot-is-not-overwritten´
    #[test]
    fn shape_coverage_slot_is_not_overwritten() -> Result<(), ShapeError> {
        let coordinate = AxisEntry::new("x", Descriptor::ratio())?;
        let mut first = EntryContract::formal(&coordinate, &coordinate)?;
        first.original = Some("first".into());
        let mut entries = vec![Some(first)];
        let second = EntryContract::formal(&coordinate, &coordinate)?;
        assert!(matches!(
            insert_entry(&mut entries, 0, second),
            Err(ShapeError::Partition)
        ));
        assert_eq!(
            entries
                .first()
                .and_then(Option::as_ref)
                .and_then(EntryContract::original),
            Some("first")
        );
        Ok(())
    }
}
