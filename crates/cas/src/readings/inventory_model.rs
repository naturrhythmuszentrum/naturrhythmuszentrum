// SPDX-License-Identifier: AGPL-3.0-only
//! Portable, immutable views of a retained reading or proof dependency closure.

use super::{AffineMap, EntryIndex, EntrySelection};
use crate::graph::{Handle, LogicalId};
use crate::model::Declaration;

/// One declaration in a retained immutable header snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyDeclaration {
    /// Canonical record index of the complete header snapshot.
    pub header: u32,
    /// Exact key, display spelling and sort in that snapshot.
    pub declaration: Declaration,
}

/// One original selection, including entries unused by a later projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencySelection {
    /// Canonical record index of the selection expression.
    pub node: u32,
    /// Canonical record index of its exact declared source or defined base.
    pub base: u32,
    /// Original ordered entries and result shape, without normalization.
    pub selection: EntrySelection,
}

/// One retained defining equation `symbol := map(source)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyDefinition {
    /// Canonical record index of the complete defining expression.
    pub node: u32,
    /// Canonical record index of the original declared symbol.
    pub symbol: u32,
    /// Canonical record index of the exact source expression.
    pub source: u32,
    /// Complete original rational affine map, including zero coefficients and offsets.
    pub map: AffineMap,
}

/// Exact entry reads at one named base in the retained closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyBase {
    /// Canonical record index of the base expression.
    pub node: u32,
    /// Original declared symbol key, independent of display spelling.
    pub symbol: String,
    /// Complete portable syntax key of the declaration header snapshot.
    pub snapshot: Vec<u8>,
    /// Complete portable syntax key of the base, including its defining expression.
    pub definition: Vec<u8>,
    /// Union of demanded base entries in logical column order.
    pub entries: Vec<EntryIndex>,
}

/// The retained role of an exact premise; none carries a numerical heap value.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DependencyPremiseKind {
    /// Full immutable context, including every admitted assumption.
    Context,
    /// Exact region or domain syntax.
    Domain,
    /// Exact predicate syntax retained by a context or guarded obligation.
    Predicate,
    /// A free-coordinate family reached through a reading's semantic source.
    Family,
    /// A guarded obligation with its exact context, region and predicate.
    Guard,
    /// A complete typed proof statement.
    Statement,
    /// A checked proof application, including earlier applications and certificates.
    Proof,
}

/// One premise's role and canonical record identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DependencyPremise {
    /// Canonical record index in the retained table.
    pub node: u32,
    /// Meaning of this retained premise.
    pub kind: DependencyPremiseKind,
}

/// Portable inventory contents, ordered by canonical record index and declaration key.
///
/// Record indices name this table only and never authorize a graph lookup. Records use
/// the existing canonical witness schemas. All original selections and equations remain
/// present even when a projection's entry reads are narrower than their full syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyContents {
    /// Complete canonical dependency records, including all typed payloads and premises.
    pub records: Vec<Vec<u8>>,
    /// Canonical index of the reading expression or witnessed conclusion.
    pub root: u32,
    /// Every declaration in every retained header snapshot.
    pub declarations: Vec<DependencyDeclaration>,
    /// Every original selection with its base and ordered entries.
    pub selections: Vec<DependencySelection>,
    /// Every original affine defining equation.
    pub definitions: Vec<DependencyDefinition>,
    /// Exact union of demanded entries per declared or defined base.
    pub bases: Vec<DependencyBase>,
    /// Contexts, domains, conditions, free families and proof records used by the closure.
    pub premises: Vec<DependencyPremise>,
}

/// Sealed dependency data minted from an authenticated reading or verified witness.
///
/// Equality compares portable contents, so fresh replay and different allocation order
/// produce equal inventories. The private owner still prevents use in a foreign `Cas`.
#[derive(Clone, Debug)]
pub struct DependencyInventory {
    /// Exact authenticated root in the producing store.
    pub(super) owner: Handle,
    /// Immutable portable view.
    pub(super) contents: DependencyContents,
    /// Local base identities in exactly the public base-vector order.
    pub(super) bases: Vec<LogicalId>,
}

impl DependencyInventory {
    /// Borrow complete portable contents without permitting mutation of sealed evidence.
    #[must_use]
    pub const fn contents(&self) -> &DependencyContents {
        &self.contents
    }
}

impl PartialEq for DependencyInventory {
    fn eq(&self, other: &Self) -> bool {
        self.contents == other.contents
    }
}

impl Eq for DependencyInventory {}
