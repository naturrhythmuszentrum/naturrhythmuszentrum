// SPDX-License-Identifier: AGPL-3.0-only
//! Checked physical matrix contracts, independent of heap layout and numerical proof.

mod assembly;
mod axis;
mod entry;
mod error;
mod expansion;
mod factor;
mod limits;
mod maps;
mod operations;
mod partition;

use crate::Extent;
use nrz_bindings::CheckedDeclarations;
use std::rc::Rc;

pub use assembly::{BlockDraft, ShapeDraft};
pub use axis::{Axis, AxisEntry};
pub use entry::{EntryContract, EntryDraft};
pub use error::ShapeError;
pub use expansion::{
    ExactCoefficient, ExpansionEntry, ExpansionMetric, ExpansionNorm, ExpansionRow, ExpansionTerm,
    StructuredExpansion,
};
pub use factor::{CholeskyRootContract, QrShapeContract};
pub use limits::ShapeLimits;
pub use maps::{AxisAlignment, EntryMap};
pub use partition::Partition;

/// Which values a structural assertion constrains; it never asserts numerical evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructureScope {
    /// Only the stored center; companions retain full logical coordinates.
    CenterOnly,
    /// Every admitted perturbation obeys the declared structural relations.
    WholeFamily,
}

/// Closed descriptor-only recipe that produced a checked shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShapeRecipe {
    /// Original typed block assembly.
    Assembly,
    /// Typed identity with formal quotient zeros and ratio diagonal entries.
    Identity,
    /// Aligned sum with common partitions.
    Sum,
    /// Ordered matrix composition.
    Product,
    /// Logical transpose with reciprocal dual axes.
    DualTranspose,
    /// Square solve axis contract, without invertibility evidence.
    Solve,
    /// Square inverse axis contract, without invertibility evidence.
    Inverse,
    /// Explicit logical selection or reshape, checked against new declared axes.
    Selection,
}

/// Immutable ordered physical shape; construction checks every supplied entry.
///
/// This is descriptor evidence, never a heap value or a numerical certificate.
#[derive(Clone, Debug)]
pub struct CheckedShape {
    /// Declared input order and coordinates.
    pub(crate) domain: Axis,
    /// Declared output order and coordinates.
    pub(crate) codomain: Axis,
    /// Checked rectangular size.
    pub(crate) extent: Extent,
    /// Every logical coefficient in column order, including zeros.
    pub(crate) entries: Vec<EntryContract>,
    /// Owner of every original and semantic rule used by this shape.
    pub(crate) declarations: Rc<CheckedDeclarations>,
    /// Scope of exact structure such as typed zeros.
    pub(crate) scope: StructureScope,
    /// Operation identity, without numerical execution claims.
    pub(crate) recipe: ShapeRecipe,
    /// Operand contracts retain original evidence and operation relationships.
    pub(crate) parents: Vec<Rc<Self>>,
    /// Logical transformations applied to operand entries.
    pub(crate) maps: Vec<EntryMap>,
    /// Explicit axis conversions retained by addition.
    pub(crate) alignments: Vec<AxisAlignment>,
}

impl CheckedShape {
    /// Ordered domain axis.
    #[must_use]
    pub const fn domain(&self) -> &Axis {
        &self.domain
    }
    /// Ordered codomain axis.
    #[must_use]
    pub const fn codomain(&self) -> &Axis {
        &self.codomain
    }
    /// Logical rectangular extent, independent of numerical layout.
    #[must_use]
    pub const fn extent(&self) -> Extent {
        self.extent
    }
    /// Every entry contract in column order.
    #[must_use]
    pub fn entries(&self) -> &[EntryContract] {
        &self.entries
    }
    /// Complete frozen declaration owner, including unused original checks.
    #[must_use]
    pub fn declarations(&self) -> &CheckedDeclarations {
        &self.declarations
    }
    /// Scope of the retained structural assertions.
    #[must_use]
    pub const fn structure_scope(&self) -> StructureScope {
        self.scope
    }
    /// Descriptor recipe, which implies no numerical execution.
    #[must_use]
    pub const fn recipe(&self) -> ShapeRecipe {
        self.recipe
    }
    /// Operands in operation order; they own their original recipes.
    #[must_use]
    pub fn parents(&self) -> &[Rc<Self>] {
        &self.parents
    }
    /// Explicit operand entry maps in operation order.
    #[must_use]
    pub fn entry_maps(&self) -> &[EntryMap] {
        &self.maps
    }
    /// Explicit axis alignments and exact conversions.
    #[must_use]
    pub fn alignments(&self) -> &[AxisAlignment] {
        &self.alignments
    }

    /// Inspect one entry by its logical row and column.
    ///
    /// # Errors
    /// Refuses an out-of-range logical coordinate.
    pub fn entry(&self, row: usize, column: usize) -> Result<&EntryContract, ShapeError> {
        self.entries
            .get(self.extent.index(row, column)?)
            .ok_or(ShapeError::EntryIndex)
    }
}
