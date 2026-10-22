// SPDX-License-Identifier: AGPL-3.0-only
//! Whole-family expansions preserve every repeated, negated and zero coordinate.

use super::{EntryMeaning, ReadingError, meaning, support};
use nrz_cas::{Cas, Handle, limits::Budget, readings::AffineMap, syntax::Operator};
use std::rc::Rc;

/// Exact ordered coordinate family; it contains no numerical input or enclosure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoordinateFamily {
    /// Exact scope and admissible family domain.
    pub(super) context: Handle,
    /// Original ordered free-coordinate declaration.
    pub(super) root: Handle,
    /// Original declaration or complete expansion definition.
    pub(super) node: Handle,
    /// Literal expanded rectangle in logical column order.
    pub(super) shape: [u32; 2],
    /// Meanings of every expanded coordinate.
    pub(super) entries: Rc<[EntryMeaning]>,
    /// Immutable meanings of the original free coordinates.
    pub(super) root_entries: Rc<[EntryMeaning]>,
}

impl CoordinateFamily {
    /// Admit every original physical coordinate of a declared free Real matrix.
    /// # Errors
    /// Refuses foreign/scoped/non-free symbols, wrong shapes and invalid original meanings.
    pub fn new(
        cas: &Cas,
        context: &Handle,
        symbol: &Handle,
        entries: Vec<EntryMeaning>,
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        meaning::validate(&entries, budget)?;
        let (sort, operator, _, header) = support::inspect(cas, symbol, budget)?;
        if !matches!(operator, Operator::Free(_)) || header != cas.context_header(context)? {
            return Err(ReadingError::Definition);
        }
        if support::coordinates(&sort)? != entries.len() {
            return Err(ReadingError::Shape);
        }
        let entries: Rc<[EntryMeaning]> = entries.into();
        Ok(Self {
            context: context.clone(),
            root: symbol.clone(),
            node: symbol.clone(),
            shape: support::shape(&sort)?,
            root_entries: Rc::clone(&entries),
            entries,
        })
    }

    /// Define a complete expansion under another declared matrix symbol.
    /// # Errors
    /// Refuses a wrong scope, shape or physical map; all original coordinates are checked.
    pub fn expand(
        &self,
        cas: &mut Cas,
        symbol: &Handle,
        map: &AffineMap,
        entries: Vec<EntryMeaning>,
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        meaning::map_meanings(map, &self.entries, &entries, budget)?;
        let (sort, _, _, header) = support::inspect(cas, symbol, budget)?;
        if header != cas.context_header(&self.context)?
            || support::coordinates(&sort)? != entries.len()
        {
            return Err(ReadingError::Definition);
        }
        let shape = support::shape(&sort)?;
        let node = cas.define_reading(symbol, &self.node, map, budget)?;
        Ok(Self {
            context: self.context.clone(),
            root: self.root.clone(),
            node,
            shape,
            entries: entries.into(),
            root_entries: Rc::clone(&self.root_entries),
        })
    }

    /// Exact original free-coordinate declaration shared by derived readings.
    #[must_use]
    pub const fn root(&self) -> &Handle {
        &self.root
    }
    /// Complete expansion definition, or the original free matrix.
    #[must_use]
    pub const fn node(&self) -> &Handle {
        &self.node
    }
    /// Scope and family domain retained by every proof.
    #[must_use]
    pub const fn context(&self) -> &Handle {
        &self.context
    }
    /// Ordered physical coordinates, including all repeated storage entries.
    #[must_use]
    pub fn entries(&self) -> &[EntryMeaning] {
        &self.entries
    }
    /// Complete original free-coordinate meanings, including coordinates omitted by an expansion.
    #[must_use]
    pub fn original_entries(&self) -> &[EntryMeaning] {
        &self.root_entries
    }
    /// Literal source rectangle in logical column order.
    #[must_use]
    pub const fn shape(&self) -> [u32; 2] {
        self.shape
    }
}
