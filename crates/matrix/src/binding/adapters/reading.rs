// SPDX-License-Identifier: AGPL-3.0-only
//! Represented exact readings and companion parameters retain ordered ticket inventories.

use super::{EntryTicket, ReadingAdapterError, entry};
use nrz_bindings::{
    Budget,
    readings::{CompanionReading, PhysicalReading, Representation, RepresentationKind},
};

/// One checked exact representation associated with all its selected numerical positions.
#[derive(Clone, Debug)]
pub struct ReadingAdapter<T> {
    /// Full-family reconstruction or exact no-storage constancy evidence.
    representation: Representation,
    /// Ordered selected storage coordinates, including repeated/negative physical copies.
    entries: Vec<EntryTicket<T>>,
}

impl<T: Eq> ReadingAdapter<T> {
    /// Match every selected storage coordinate to an opaque ticket, or no tickets for None.
    /// # Errors
    /// Refuses missing/extra entries and repeated positions; live numerical checks remain external.
    pub fn new(
        representation: Representation,
        entries: Vec<EntryTicket<T>>,
        budget: &mut Budget,
    ) -> Result<Self, ReadingAdapterError> {
        let count = if representation.kind() == RepresentationKind::None {
            0
        } else {
            representation.reading().selection().indices().len()
        };
        entry::check(&entries, count, budget)?;
        Ok(Self {
            representation,
            entries,
        })
    }
}

impl<T> ReadingAdapter<T> {
    /// Exact physical reading identity with all original declarations retained.
    #[must_use]
    pub const fn reading(&self) -> &PhysicalReading {
        self.representation.reading()
    }
    /// Complete reconstruction or no-storage constancy proof.
    #[must_use]
    pub const fn representation(&self) -> &Representation {
        &self.representation
    }
    /// Original selected-coordinate order, never inferred from equal values.
    #[must_use]
    pub fn entries(&self) -> &[EntryTicket<T>] {
        &self.entries
    }
}

/// One exact companion reading with all target adapters and parameter storage retained.
#[derive(Clone, Debug)]
pub struct CompanionAdapter<T> {
    /// Actual companion definition with checked kind, units and target maps.
    companion: CompanionReading,
    /// Ordered output parameter positions; these are no physical input uncertainty by substitution.
    entries: Vec<EntryTicket<T>>,
    /// Exact value adapters targeted by this radius or ordered covariance block.
    targets: Vec<ReadingAdapter<T>>,
}

impl<T: Eq> CompanionAdapter<T> {
    /// Check companion target identities and complete ordered parameter storage.
    /// # Errors
    /// Refuses substituted targets, numerical companions of None, wrong extents and duplicates.
    pub fn new(
        companion: CompanionReading,
        entries: Vec<EntryTicket<T>>,
        targets: Vec<ReadingAdapter<T>>,
        budget: &mut Budget,
    ) -> Result<Self, ReadingAdapterError> {
        super::capacity::<ReadingAdapter<T>>(targets.len(), budget)?;
        if targets.len() != companion.targets().len() {
            return Err(ReadingAdapterError::Target);
        }
        for (target, expected) in targets.iter().zip(companion.targets()) {
            super::reading_work(expected, budget)?;
            super::reading_work(target.reading(), budget)?;
            if target.reading() != expected
                || target.representation().kind() == RepresentationKind::None
            {
                return Err(ReadingAdapterError::Target);
            }
        }
        entry::check(
            &entries,
            usize::try_from(companion.map().outputs()).map_err(|_| ReadingAdapterError::Extent)?,
            budget,
        )?;
        Ok(Self {
            companion,
            entries,
            targets,
        })
    }
}

impl<T> CompanionAdapter<T> {
    /// Exact companion definition, including both covariance targets and the entire map.
    #[must_use]
    pub const fn companion(&self) -> &CompanionReading {
        &self.companion
    }
    /// Complete ordered companion parameter positions.
    #[must_use]
    pub fn entries(&self) -> &[EntryTicket<T>] {
        &self.entries
    }
    /// Exact target adapters which the numerical owner must resolve together.
    #[must_use]
    pub fn targets(&self) -> &[ReadingAdapter<T>] {
        &self.targets
    }
}
