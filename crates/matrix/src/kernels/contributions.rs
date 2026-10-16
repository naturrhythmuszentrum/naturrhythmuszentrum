// SPDX-License-Identifier: AGPL-3.0-only
//! Product derivation terms are independently stored companions already covered by the total.

use crate::heap::identity::Key;
use crate::heap::records::{Location, Meaning, Record};
use crate::storage::{Format, Region};
use crate::{
    Companion, CompanionView, Extent, Handle, Heap, HeapError, Layout, PublishedBundle, Value,
    ValueStamp, ValueView,
};
use nrz_scalar::Scalar;

/// A nonnegative product contribution; none is independent of the result's total bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductTerm {
    /// `abs(A) R_B`, the contribution linear in the right operand's radius.
    RightInput,
    /// `R_A abs(B)`, the contribution linear in the left operand's radius.
    LeftInput,
    /// `R_A R_B`, the union of both operands' deterministic sources.
    Interaction,
}

impl ProductTerm {
    /// Stable region order used by storage, candidates and exact-oracle tests.
    pub(crate) const ALL: [Self; 3] = [Self::RightInput, Self::LeftInput, Self::Interaction];
    /// Convert the closed term inventory to its checked fixed array position.
    const fn index(self) -> usize {
        match self {
            Self::RightInput => 0,
            Self::LeftInput => 1,
            Self::Interaction => 2,
        }
    }
}

impl<P: Scalar> PublishedBundle<P> {
    /// Inspect a separately stored product contribution, already included in `bound`.
    #[must_use]
    pub fn contribution(self, term: ProductTerm) -> Option<Handle<Companion, P>> {
        self.contributions
            .and_then(|handles| handles.get(term.index()).copied())
    }
}

impl<P: Scalar> ValueView<'_, P> {
    /// Whole-base product contribution; use the heap's selected view for an alias.
    #[must_use]
    pub fn contribution(&self, term: ProductTerm) -> Option<Handle<Companion, P>> {
        self.contributions
            .and_then(|handles| handles.get(term.index()).copied())
    }
}

impl Heap {
    /// Resolve a product term using exactly the target value's selected logical entry map.
    /// # Errors
    /// Refuses stale values, target versions, invalid selections and corrupted reverse edges.
    pub fn selected_contribution<P: Scalar + 'static>(
        &self,
        handle: Handle<Value, P>,
        term: ProductTerm,
    ) -> Result<Option<CompanionView<'_, P>>, HeapError> {
        let value = self.value(handle)?;
        let Some(bound) = value.contribution(term) else {
            return Ok(None);
        };
        let mut companion = self.companion(bound)?;
        companion.region = self.mapped_region(self.record(bound)?, &self.record(handle)?.layout)?;
        Ok(Some(companion))
    }
}

/// All immutable addresses needed to prepare contribution descriptors before publication.
#[derive(Debug)]
pub(crate) struct ContributionReservation<'a> {
    /// Owning heap scope.
    pub(crate) scope: u64,
    /// Result rectangle shared by each contribution.
    pub(crate) extent: Extent,
    /// Reserved full value and companion descriptor keys.
    pub(crate) keys: &'a [Key],
    /// Corresponding private backend arena keys.
    pub(crate) arenas: &'a [Key],
    /// Optional radius schema for the three contribution regions.
    pub(crate) schema: Option<usize>,
    /// Reserved result identity and version.
    pub(crate) stamp: ValueStamp,
    /// Physical epoch of the private destination arenas.
    pub(crate) epoch: u64,
}

/// Prepare three same-target bound regions without adding them to total coverage twice.
fn prepare<P: Scalar + 'static>(
    plan: &ContributionReservation<'_>,
    records: &mut Vec<Record>,
) -> Result<Option<[Handle<Companion, P>; 3]>, HeapError> {
    let Some(schema) = plan.schema else {
        return Ok(None);
    };
    let target = *plan.keys.first().ok_or(HeapError::Capacity)?;
    let mut handles = crate::binding::buffer(3)?;
    for position in 3..6 {
        let key = *plan.keys.get(position).ok_or(HeapError::Capacity)?;
        let arena = *plan.arenas.get(position).ok_or(HeapError::Capacity)?;
        records.push(Record {
            schema,
            location: Location::Owned(Region {
                epoch: plan.epoch,
                arena,
                format: Format::of::<P>(),
                offset: 0,
                length: plan.extent.entries(),
            }),
            layout: Layout::contiguous(plan.extent)?,
            meaning: Meaning::Bound {
                target,
                stamp: plan.stamp,
            },
        });
        handles.push(Handle::new(plan.scope, key));
    }
    Ok(Some(handles.try_into().map_err(|_| HeapError::Capacity)?))
}

/// Capture all existing selected contribution views as one all-or-none inventory.
pub(crate) fn views<P: Scalar + 'static>(
    heap: &Heap,
    handle: Handle<Value, P>,
) -> Result<Option<[CompanionView<'_, P>; 3]>, HeapError> {
    let first = heap.selected_contribution(handle, ProductTerm::RightInput)?;
    let second = heap.selected_contribution(handle, ProductTerm::LeftInput)?;
    let interaction = heap.selected_contribution(handle, ProductTerm::Interaction)?;
    match (first, second, interaction) {
        (Some(first), Some(second), Some(interaction)) => Ok(Some([first, second, interaction])),
        (None, None, None) => Ok(None),
        _ => Err(HeapError::StaleEvidence),
    }
}

impl<P: Scalar + 'static> crate::NormalizedView<'_, P> {
    /// Read a carried product derivation term in the exact same selected coordinates.
    /// # Errors
    /// Refuses out-of-range coordinates or stale companion storage.
    pub fn contribution(
        &self,
        term: ProductTerm,
        row: usize,
        column: usize,
    ) -> Result<Option<P>, HeapError> {
        self.contributions
            .as_ref()
            .and_then(|views| views.get(term.index()))
            .map(|view| view.region().get(row, column))
            .transpose()
    }
}

impl Heap {
    /// Validate direct contribution ownership without granting upstream mathematical assurance.
    pub(crate) fn contribution_regions<P: Scalar + 'static>(
        &self,
        keys: Option<[Key; 3]>,
        target: Key,
        stamp: ValueStamp,
        extent: Extent,
    ) -> Result<Option<[(Key, Region); 3]>, HeapError> {
        let Some(keys) = keys else {
            return Ok(None);
        };
        let [first, second, interaction] = keys.map(|key| {
            let record = self.record(Handle::<Companion, P>::new(self.scope, key))?;
            if !matches!(record.meaning, Meaning::Bound { target: owner, stamp: version } if owner == target && version == stamp)
                || self.region_view::<P>(record)?.extent() != extent
            { return Err(HeapError::StaleEvidence); }
            Ok((key, self.owning_region(record)?))
        });
        Ok(Some([first?, second?, interaction?]))
    }
}

/// Complete all reverse companion links before the value descriptor can become visible.
pub(crate) fn finish<P: Scalar + 'static>(
    plan: &ContributionReservation<'_>,
    bound: Option<Key>,
    evaluation: Option<Handle<Companion, P>>,
    records: &mut Vec<Record>,
) -> Result<PublishedBundle<P>, HeapError> {
    let contributions = prepare(plan, records)?;
    let target = *plan.keys.first().ok_or(HeapError::Capacity)?;
    let record = records.first_mut().ok_or(HeapError::Capacity)?;
    let Meaning::Value {
        evaluation: edge,
        contributions: terms,
        ..
    } = &mut record.meaning
    else {
        return Err(HeapError::CategoryMismatch);
    };
    *edge = evaluation.map(|handle| handle.key);
    *terms = contributions.map(|handles| handles.map(|handle| handle.key));
    Ok(PublishedBundle {
        value: Handle::new(plan.scope, target),
        bound: bound.map(|key| Handle::new(plan.scope, key)),
        evaluation,
        contributions,
    })
}
