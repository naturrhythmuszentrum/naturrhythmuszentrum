// SPDX-License-Identifier: AGPL-3.0-only
//! Original entry recipes, typed structural zeros and retained quotient witnesses.

use super::{AxisEntry, ShapeError};
use nrz_bindings::{
    CheckedDeclarations, CheckedExpression, Conversion, Descriptor, Dimension, Measure,
    PairingWitness,
};

/// One original entry declaration; structural absence still carries a scalar type.
#[derive(Clone, Debug)]
pub enum EntryDraft {
    /// A directly declared scalar coefficient schema, with no expression claimed.
    Declared(Descriptor),
    /// A structurally absent coefficient with its required physical descriptor.
    Zero(Descriptor),
    /// A named original checked in the complete frozen declaration owner.
    Original(String),
    /// A separately checked original from the same semantic snapshot.
    Checked(Box<CheckedExpression>),
}

/// Checked output-over-input entry meaning and optional concrete scalar kind.
#[derive(Clone, Debug)]
pub struct EntryContract {
    /// Formal quotient, checked without inventing a scalar kind.
    pub(crate) pairing: PairingWitness,
    /// Present for actual supplied scalar entries; derived formal contracts may omit it.
    pub(crate) scalar: Option<Descriptor>,
    /// Exact supplied-entry to axis-quotient coordinate map.
    pub(crate) conversion: Option<Conversion>,
    /// A proved structural zero in this contract's stated scope.
    pub(crate) zero: bool,
    /// Original declaration key, resolved in the owning checked shape snapshot.
    pub(crate) original: Option<String>,
    /// Full separately supplied original check, retaining every original node.
    pub(crate) evidence: Option<CheckedExpression>,
}

impl EntryContract {
    /// Quotient dimension; a dual transpose retains this same physical dimension.
    #[must_use]
    pub const fn dimension(&self) -> &Dimension {
        self.pairing.dimension()
    }

    /// Quotient difference measure, distinct from a supplied entry's display measure.
    #[must_use]
    pub const fn measure(&self) -> &Measure {
        self.pairing.measure()
    }

    /// Supplied scalar kind, absent for a formal solve or inverse result.
    #[must_use]
    pub const fn scalar_descriptor(&self) -> Option<&Descriptor> {
        self.scalar.as_ref()
    }

    /// Exact conversion into the ordered axes' quotient coordinates.
    #[must_use]
    pub const fn conversion(&self) -> Option<&Conversion> {
        self.conversion.as_ref()
    }

    /// Whether structure declares this entry exactly zero.
    #[must_use]
    pub const fn is_zero(&self) -> bool {
        self.zero
    }

    /// Original declaration key; the checked shape retains its entire immutable owner.
    #[must_use]
    pub fn original(&self) -> Option<&str> {
        self.original.as_deref()
    }

    /// Complete separately supplied original check; its owner is retained by the shape.
    #[must_use]
    pub const fn checked_expression(&self) -> Option<&CheckedExpression> {
        self.evidence.as_ref()
    }

    /// Bindings evidence for the original primal relation, retained through dual transpose.
    #[must_use]
    pub const fn pairing(&self) -> &PairingWitness {
        &self.pairing
    }

    /// Derive a formal map entry from ordinary scalar axes.
    pub(crate) fn formal(output: &AxisEntry, input: &AxisEntry) -> Result<Self, ShapeError> {
        Ok(Self {
            pairing: PairingWitness::sensitivity(output.scalar()?, input.scalar()?)?,
            scalar: None,
            conversion: None,
            zero: false,
            original: None,
            evidence: None,
        })
    }

    /// Check every original coefficient, including typed structural zeros.
    pub(crate) fn check(
        draft: &EntryDraft,
        output: &AxisEntry,
        input: &AxisEntry,
        owner: &CheckedDeclarations,
        index: usize,
    ) -> Result<Self, ShapeError> {
        let descriptor = match draft {
            EntryDraft::Declared(quantity) | EntryDraft::Zero(quantity) => quantity,
            EntryDraft::Original(name) => owner.checked(name)?.descriptor(),
            EntryDraft::Checked(checked) => {
                owner
                    .validate_checked(checked)
                    .map_err(|_| ShapeError::SnapshotMismatch)?;
                checked.descriptor()
            }
        };
        let mut entry = Self::formal(output, input)?;
        entry.conversion = Some(
            entry
                .pairing
                .check_entry(owner.registry(), descriptor)
                .map_err(|cause| ShapeError::EntryMeaning { index, cause })?,
        );
        entry.scalar = Some(descriptor.clone());
        entry.zero = matches!(draft, EntryDraft::Zero(_));
        if let EntryDraft::Original(name) = draft {
            entry.original = Some(name.clone());
        }
        if let EntryDraft::Checked(checked) = draft {
            entry.evidence = Some(checked.as_ref().clone());
        }
        Ok(entry)
    }
}
