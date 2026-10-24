// SPDX-License-Identifier: AGPL-3.0-only
//! Selected radius channels retain their own storage maps and exact target definitions.

use super::SelectedReading;
use crate::{CompanionAdapter, Coverage, HeapError, Layout};
use nrz_bindings::{CompanionKind, readings::CompanionReadingKind};

impl SelectedReading {
    /// Construct only the selected identity companions under the caller's exact ledger.
    pub(super) fn companion_symbols(
        &self,
        budget: &mut nrz_bindings::Budget,
    ) -> Result<nrz_bindings::BindingSymbols, HeapError> {
        let symbols = self.symbols().ok_or(HeapError::MissingBinding)?.clone();
        let companions = self
            .companions()
            .map(CompanionAdapter::companion)
            .collect::<Vec<_>>();
        if companions.is_empty() {
            Ok(symbols)
        } else {
            Ok(symbols.with_reading_companions(&companions, budget)?)
        }
    }

    /// Exact selected value definition.
    pub(crate) fn reading(&self) -> &nrz_bindings::readings::PhysicalReading {
        self.group.coverage.readings()[self.index].reading()
    }

    /// Every original companion targeting this selection, including both covariance sides.
    pub(crate) fn companions(&self) -> impl Iterator<Item = &CompanionAdapter<usize>> {
        self.group.companions.iter().filter(|companion| {
            companion
                .targets()
                .iter()
                .any(|target| target.reading() == self.reading())
        })
    }

    /// All actual repeated copies and their discrepancies in the selected exact family.
    pub(crate) fn copies(&self) -> impl Iterator<Item = &super::super::CopyDiscrepancy> {
        self.group
            .copies
            .iter()
            .filter(|copy| copy.family() == self.reading().family().node())
    }

    /// Find one exact radius channel; ambiguous declarations cannot choose by order.
    pub(crate) fn radius(
        &self,
        kind: CompanionKind,
    ) -> Result<Option<&CompanionAdapter<usize>>, HeapError> {
        let reading = self
            .group
            .coverage
            .readings()
            .get(self.index)
            .ok_or(HeapError::MissingBinding)?;
        let mut found = None;
        for companion in &self.group.companions {
            let targeted = companion
                .targets()
                .iter()
                .any(|target| target.reading() == reading.reading());
            if targeted && companion.companion().kind() == CompanionReadingKind::Radius(kind) {
                if found.is_some() {
                    return Err(HeapError::MissingCoverage);
                }
                found = Some(companion);
            }
        }
        Ok(found)
    }

    /// Separate physical assurance follows the selected declaration and original producer.
    pub(crate) fn coverage(&self, fallback: Coverage) -> Coverage {
        let declared = self
            .radius(CompanionKind::DeterministicBound)
            .is_ok_and(|value| value.is_some());
        let inherited = self
            .group
            .phasor_inputs
            .is_none_or(|inputs| inputs == [Coverage::DeclaredBounds; 2]);
        if declared && inherited {
            Coverage::DeclaredBounds
        } else {
            fallback
        }
    }

    /// Apply the parameter's own ordered selection, never the value's base indices.
    pub(crate) fn radius_layout(
        &self,
        kind: CompanionKind,
        limit: usize,
    ) -> Result<Option<(&super::super::RegionTicket, Layout)>, HeapError> {
        let Some(companion) = self.radius(kind)? else {
            return Ok(None);
        };
        let first = companion
            .entries()
            .first()
            .ok_or(HeapError::MissingCoverage)?;
        if companion
            .entries()
            .iter()
            .any(|entry| entry.region != first.region)
        {
            return Err(HeapError::InvalidLayout);
        }
        let ticket = self
            .group
            .tickets
            .get(first.region)
            .ok_or(HeapError::InvalidLayout)?;
        let symbols = self.symbols().ok_or(HeapError::MissingBinding)?;
        let [rows, columns] = symbols.value().shape();
        let extent = crate::Extent::new(
            usize::try_from(rows).map_err(|_| HeapError::ExtentOverflow)?,
            usize::try_from(columns).map_err(|_| HeapError::ExtentOverflow)?,
        )?;
        let indices = companion
            .entries()
            .iter()
            .map(|entry| usize::try_from(entry.index).map_err(|_| HeapError::InvalidLayout))
            .collect::<Result<Vec<_>, _>>()?;
        let layout = Layout::gather(extent, ticket.layout.extent().entries(), &indices, limit)?;
        Ok(Some((ticket, layout)))
    }
}
