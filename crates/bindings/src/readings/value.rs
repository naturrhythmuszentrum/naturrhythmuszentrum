// SPDX-License-Identifier: AGPL-3.0-only
//! One defined reading retains its exact source selection and ordered affine map.

use super::{CoordinateFamily, EntryMeaning, ReadingError, meaning, support};
use nrz_cas::{
    Cas, Handle,
    limits::Budget,
    readings::{AffineMap, EntrySelection, SelectionSpec},
    syntax::{AtomicSort, Field, Sort},
};
use std::rc::Rc;

/// Proposed physical reading of a selected coordinate family.
#[derive(Clone, Debug)]
pub struct ReadingDefinition {
    /// Actual declared result symbol, never resolved by display name.
    pub symbol: Handle,
    /// Ordered source entries, retaining selected-copy identity.
    pub selection: SelectionSpec,
    /// Exact selected-coordinate to result-coordinate map.
    pub map: AffineMap,
    /// Complete ordered result meanings.
    pub entries: Vec<EntryMeaning>,
}

/// Sealed physical descriptor over a CAS defined reading.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalReading {
    /// Complete source family and original physical declarations.
    pub(super) family: CoordinateFamily,
    /// Original declared result symbol, without a defining equation.
    pub(super) symbol: Handle,
    /// Retained CAS affine definition used as the reading identity.
    pub(super) node: Handle,
    /// Exact CAS selected-source expression.
    pub(super) selected: Handle,
    /// Checked source entries, including order and repeated-copy identity.
    pub(super) selection: EntrySelection,
    /// Complete rational map from selected entries to result coordinates.
    pub(super) map: AffineMap,
    /// Immutable mathematical result declaration.
    pub(super) sort: Sort,
    /// Physical result meanings in logical coordinate order.
    pub(super) entries: Rc<[EntryMeaning]>,
}

impl PhysicalReading {
    /// Check physical meanings and construct the retained CAS definition.
    /// # Errors
    /// Refuses missing declarations, wrong order/extent, kind/reference/origin changes and limits.
    pub fn define(
        cas: &mut Cas,
        family: &CoordinateFamily,
        definition: ReadingDefinition,
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        meaning::validate(&family.entries, budget)?;
        let (source_sort, _, _, _) = support::inspect(cas, family.node(), budget)?;
        let selection = EntrySelection::new(&source_sort, &definition.selection, budget)?;
        let inputs = selected_meanings(family, &selection, budget)?;
        meaning::map_meanings(&definition.map, &inputs, &definition.entries, budget)?;
        let (sort, _, _, header) = support::inspect(cas, &definition.symbol, budget)?;
        if header != cas.context_header(family.context())?
            || support::coordinates(&sort)? != definition.entries.len()
        {
            return Err(ReadingError::Definition);
        }
        if sort == Sort::Value(AtomicSort::Scalar(Field::Complex))
            && definition.entries.first() != definition.entries.get(1)
        {
            return Err(ReadingError::Meaning);
        }
        let selected = cas.select_entries(family.node(), &definition.selection, budget)?;
        let node = cas.define_reading(&definition.symbol, &selected, &definition.map, budget)?;
        Ok(Self {
            family: family.clone(),
            symbol: definition.symbol,
            node,
            selected,
            selection,
            map: definition.map,
            sort,
            entries: definition.entries.into(),
        })
    }

    /// The returned CAS definition is this reading's identity.
    #[must_use]
    pub const fn node(&self) -> &Handle {
        &self.node
    }
    /// Original declared symbol; it alone does not establish the reading equation.
    #[must_use]
    pub const fn symbol(&self) -> &Handle {
        &self.symbol
    }
    /// Original complete coordinate family and domain.
    #[must_use]
    pub const fn family(&self) -> &CoordinateFamily {
        &self.family
    }
    /// Selected source expression, including exact entry copies and order.
    #[must_use]
    pub const fn selected(&self) -> &Handle {
        &self.selected
    }
    /// Checked finite selection.
    #[must_use]
    pub const fn selection(&self) -> &EntrySelection {
        &self.selection
    }
    /// Exact reading map, with offsets retained separately from linear coefficients.
    #[must_use]
    pub const fn map(&self) -> &AffineMap {
        &self.map
    }
    /// Immutable declaration sort; it chooses no numerical layout.
    #[must_use]
    pub const fn sort(&self) -> &Sort {
        &self.sort
    }
    /// Per-result-coordinate physical meanings in logical order.
    #[must_use]
    pub fn entries(&self) -> &[EntryMeaning] {
        &self.entries
    }
}

/// Project source meanings in exactly the checked entry order.
fn selected_meanings(
    family: &CoordinateFamily,
    selection: &EntrySelection,
    budget: &mut Budget,
) -> Result<Vec<EntryMeaning>, ReadingError> {
    let mut entries = support::reserve(selection.indices().len(), budget)?;
    for index in selection.indices() {
        let position = u64::from(index.column) * u64::from(family.shape[0]) + u64::from(index.row);
        entries.push(
            family
                .entries
                .get(usize::try_from(position).map_err(|_| ReadingError::Shape)?)
                .ok_or(ReadingError::Shape)?
                .clone(),
        );
    }
    Ok(entries)
}
