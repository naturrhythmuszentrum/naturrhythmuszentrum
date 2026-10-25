// SPDX-License-Identifier: AGPL-3.0-only
//! Evidence borrows the original constant allocation without copying its numerical fields.

use super::region::Region;
use nrz_scalar::{PiBounds, Precision, Scalar};
use std::{fmt::Debug, rc::Rc};

/// Read-only inspection of the original allocation across sealed formats.
trait View: Debug {
    /// The exact rational theorem bounds.
    fn bounds(&self) -> PiBounds;
    /// The original pi endpoints widened exactly for inspection.
    fn enclosure(&self) -> [f64; 2];
    /// The original reciprocal endpoints widened exactly for inspection.
    fn reciprocal(&self) -> [f64; 2];
    /// Constant algorithm revision.
    fn revision(&self) -> u32;
    /// Scalar arithmetic revision.
    fn arithmetic(&self) -> u32;
    /// Original scalar format.
    fn precision(&self) -> Precision;
}

impl<P: Scalar> View for Region<P> {
    /// The exact rational theorem bounds.
    fn bounds(&self) -> PiBounds {
        self.pi.rational_bounds()
    }
    /// The original pi endpoints widened exactly for inspection.
    fn enclosure(&self) -> [f64; 2] {
        [
            self.pi.enclosure().lower().get().to_f64(),
            self.pi.enclosure().upper().get().to_f64(),
        ]
    }
    /// The original reciprocal endpoints widened exactly for inspection.
    fn reciprocal(&self) -> [f64; 2] {
        [
            self.reciprocal.lower().get().to_f64(),
            self.reciprocal.upper().get().to_f64(),
        ]
    }
    /// Constant algorithm revision.
    fn revision(&self) -> u32 {
        self.pi.revision()
    }
    /// Scalar arithmetic revision.
    fn arithmetic(&self) -> u32 {
        self.arithmetic
    }
    /// Original scalar format.
    fn precision(&self) -> Precision {
        P::PRECISION
    }
}

/// The original immutable numerical constant region retained by published conversion evidence.
#[derive(Clone, Debug)]
pub struct PiConstantEvidence {
    /// Shared allocation, with only private read-only format erasure.
    region: Rc<dyn View>,
}

impl Eq for PiConstantEvidence {}

impl PartialEq for PiConstantEvidence {
    fn eq(&self, other: &Self) -> bool {
        self.same_region(other)
    }
}

impl PiConstantEvidence {
    /// Erase only the view type while retaining the original allocation.
    pub(super) fn new<P: Scalar + 'static>(region: Rc<Region<P>>) -> Self {
        Self { region }
    }

    /// Checked rational bounds of the mathematical pi definition.
    #[must_use]
    pub fn rational_bounds(&self) -> PiBounds {
        self.region.bounds()
    }

    /// Original outward pi endpoints, exactly widened for inspection.
    #[must_use]
    pub fn enclosure(&self) -> [f64; 2] {
        self.region.enclosure()
    }

    /// Original outward reciprocal endpoints, exactly widened for inspection.
    #[must_use]
    pub fn reciprocal(&self) -> [f64; 2] {
        self.region.reciprocal()
    }

    /// Retained constant theorem revision.
    #[must_use]
    pub fn constant_revision(&self) -> u32 {
        self.region.revision()
    }

    /// Retained arithmetic theorem revision.
    #[must_use]
    pub fn arithmetic_revision(&self) -> u32 {
        self.region.arithmetic()
    }

    /// Original numerical format; widening the inspector does not change it.
    #[must_use]
    pub fn precision(&self) -> Precision {
        self.region.precision()
    }

    /// Whether two operation records retain the same immutable numerical region.
    #[must_use]
    pub fn same_region(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.region, &other.region)
    }
}
