// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed typed coordinate references retain their native exact inverse and both certificates.

mod image;
mod storage;

pub use image::CoordinateImage;

use super::{EntryMeaning, ReadingError, meaning, support};
use nrz_cas::{
    Cas, Handle,
    limits::Budget,
    readings::{CheckedMapInverse, ExactAffineMap, ReuseKey},
};
use std::rc::Rc;

/// Direction between coherent physical coordinates and the retained dimensionless references.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinateDirection {
    /// Subtract each declared point origin and divide by its positive difference reference.
    Normalize,
    /// Multiply by each retained reference and restore point origins only to centers.
    Restore,
}

/// Checked diagonal restoration and normalization of an ordered mixed physical coordinate vector.
#[derive(Clone, Debug)]
pub struct CoordinateNormalization(Rc<Definition>);

/// Shared exact evidence is retained once; cloning a contract preserves definition identities.
#[derive(Debug)]
struct Definition {
    /// Every original physical entry, including its source-free reference and point origin.
    entries: Vec<EntryMeaning>,
    /// Exact declaration context used by both inverse certificates.
    context: Handle,
    /// Native homogeneous matrix for coherent restoration.
    restoration: ExactAffineMap,
    /// Normalization, determinant, nonsingularity and identity certificates kept together.
    inverse: CheckedMapInverse,
    /// Conservative retained metadata charge for nested exact operations.
    bytes: u64,
}

impl CoordinateNormalization {
    /// Construct `y=S*z+o` and request CAS's checked inverse under the caller's ledger.
    ///
    /// Each entry already carries positive typed source-free difference evidence. No
    /// numerical operand, measured calibration or inferred magnitude can supply a reference.
    /// Exact non-rational references remain admitted; numerical export is a separate operation.
    /// # Errors
    /// Refuses noncoherent input measures, invalid original meanings, foreign contexts and exhausted exact resources.
    pub fn new(
        cas: &mut Cas,
        context: &Handle,
        entries: &[EntryMeaning],
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        meaning::validate(entries, budget)?;
        if entries
            .iter()
            .any(|entry| entry.descriptor().measure() != &crate::Measure::coherent())
        {
            return Err(ReadingError::Measure);
        }
        let mut retained = support::reserve(entries.len(), budget)?;
        retained.extend_from_slice(entries);
        let bytes = storage::entries(entries)?;
        let restoration = budget.with_retained_payload(bytes, support::AT, |budget| {
            image::restoration(cas, context, entries, budget)
        })?;
        let inverse = budget.with_retained_payload(bytes, support::AT, |budget| {
            cas.invert_reading_map(context, &restoration, budget)
        })?;
        let bytes = storage::inverse(cas, &inverse, bytes, budget)?;
        Ok(Self(Rc::new(Definition {
            entries: retained,
            context: context.clone(),
            restoration,
            inverse,
            bytes,
        })))
    }

    /// Original ordered physical meanings, references and origins.
    #[must_use]
    pub fn entries(&self) -> &[EntryMeaning] {
        &self.0.entries
    }

    /// Exact declaration and domain context; another equal-looking context cannot substitute.
    #[must_use]
    pub fn context(&self) -> &Handle {
        &self.0.context
    }

    /// Native exact restoration matrix, including each declared point origin.
    #[must_use]
    pub fn restoration(&self) -> &ExactAffineMap {
        &self.0.restoration
    }

    /// Checked normalization inverse, determinant and both retained witnesses.
    #[must_use]
    pub fn inverse(&self) -> &CheckedMapInverse {
        &self.0.inverse
    }

    /// Match every original entry, including independent reference and origin identities.
    /// # Errors
    /// Refuses changed order, kinds, measures, roles, references or origins.
    pub fn check_entries(
        &self,
        entries: &[EntryMeaning],
        budget: &mut Budget,
    ) -> Result<(), ReadingError> {
        meaning::validate(entries, budget)?;
        if entries.len() != self.entries().len() {
            return Err(ReadingError::Shape);
        }
        for (expected, actual) in self.entries().iter().zip(entries) {
            expected.check(actual)?;
        }
        Ok(())
    }

    /// Conservative retained exact metadata and inverse-witness allowance.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }

    /// Key the original restoration expression and its complete mathematical context.
    ///
    /// Consumers additionally retain `Self::entries` identities and numerical dependency,
    /// format and arithmetic revisions. Equal numerical values never supply this key.
    /// # Errors
    /// Refuses foreign graph authority or exhausted cumulative exact resources.
    pub fn reuse_key(&self, cas: &mut Cas, budget: &mut Budget) -> Result<ReuseKey, ReadingError> {
        Ok(
            budget.with_retained_payload(self.0.bytes, support::AT, |budget| {
                cas.expression_reuse_key(self.context(), self.restoration().expression(), budget)
            })?,
        )
    }

    /// Retain a checked exact numerical-image definition for one direction.
    /// # Errors
    /// Refuses foreign graph authority, unproved correspondence and exact resource exhaustion.
    pub fn image(
        &self,
        cas: &mut Cas,
        direction: CoordinateDirection,
        budget: &mut Budget,
    ) -> Result<CoordinateImage, ReadingError> {
        image::direction(cas, self, direction, budget)
    }

    /// Compose another frame's checked inverse after this frame's exact restoration.
    ///
    /// The result is `target.S^(-1)*(self.S*z+self.o-target.o)`. Compatible kinds and
    /// roles are mandatory; references and origins remain explicit on both endpoints.
    /// # Errors
    /// Refuses different contexts, shapes, kinds or roles, and exact resource exhaustion.
    pub fn conversion_to(
        &self,
        cas: &mut Cas,
        target: &Self,
        budget: &mut Budget,
    ) -> Result<CoordinateImage, ReadingError> {
        image::conversion(cas, self, target, budget)
    }
}
