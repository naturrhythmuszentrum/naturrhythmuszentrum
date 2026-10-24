// SPDX-License-Identifier: AGPL-3.0-only
//! Closed allocation bindings project original reciprocal declarations without new exact graphs.

use super::PositiveReciprocalMap;
use crate::readings::EntryMeaning;
use nrz_cas::{Handle, Rational};

/// One finite buffer role in the checked positive-reciprocal schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReciprocalStage {
    /// Imported physical references, each with its own original meaning.
    References,
    /// Row-major dimensionless affine coefficients.
    Coefficients,
    /// Column-ordered dimensionless affine offsets.
    Offsets,
    /// Positive normalized reciprocal candidates and both interval images.
    Images,
    /// Contracted result centers, total radii and numerical radii.
    Contracted,
    /// Separate candidate center coordinates.
    Centers,
    /// Separate candidate total radii.
    Totals,
    /// Separate candidate numerical-only radii.
    Errors,
}

impl ReciprocalStage {
    /// Complete ordered buffer inventory of this closed numerical schedule.
    pub const ALL: [Self; 8] = [
        Self::References,
        Self::Coefficients,
        Self::Offsets,
        Self::Images,
        Self::Contracted,
        Self::Centers,
        Self::Totals,
        Self::Errors,
    ];

    /// Ordered floating components of each logical buffer entry.
    #[must_use]
    pub const fn components(self) -> &'static [ReciprocalComponent] {
        use ReciprocalComponent as Part;
        match self {
            Self::References | Self::Coefficients | Self::Offsets => {
                &[Part::Center, Part::Lower, Part::Upper, Part::ImportError]
            }
            Self::Images => &[
                Part::Center,
                Part::TotalLower,
                Part::TotalUpper,
                Part::NominalLower,
                Part::NominalUpper,
            ],
            Self::Contracted => &[Part::Center, Part::TotalRadius, Part::NumericalRadius],
            Self::Centers => &[Part::Center],
            Self::Totals => &[Part::TotalRadius],
            Self::Errors => &[Part::NumericalRadius],
        }
    }
}

/// Interpretation of a floating component relative to its exact logical target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReciprocalComponent {
    /// Rounded candidate for the exact target.
    Center,
    /// Lower endpoint of an exact constant enclosure.
    Lower,
    /// Upper endpoint of an exact constant enclosure.
    Upper,
    /// Absolute displacement of the imported candidate from the exact constant.
    ImportError,
    /// Lower endpoint over the complete admitted physical input family.
    TotalLower,
    /// Upper endpoint over the complete admitted physical input family.
    TotalUpper,
    /// Lower endpoint over the numerical-only input enclosure.
    NominalLower,
    /// Upper endpoint over the numerical-only input enclosure.
    NominalUpper,
    /// Absolute radius over the complete admitted family, including numerical error.
    TotalRadius,
    /// Absolute radius for the original nominal target.
    NumericalRadius,
}

/// Exact target of one logical entry; component roles supply its numerical interpretation.
#[derive(Clone, Copy, Debug)]
pub enum ReciprocalEntry<'a> {
    /// Original independently typed physical reference and its coherent magnitude.
    Reference(&'a EntryMeaning),
    /// Exact dimensionless affine coefficient or offset.
    Constant(&'a Rational),
    /// Exact normalized reciprocal expression over its original formal input.
    Image {
        /// Original input symbol in the checked graph.
        input: &'a Handle,
        /// Checked expression for reference divided by that input.
        expression: &'a Handle,
        /// Original physical reference, with independent definition identity.
        reference: &'a EntryMeaning,
    },
    /// Column-ordered selection of the original checked result expression.
    Output {
        /// Complete dimensionless operator expression in the checked graph.
        expression: &'a Handle,
        /// Column-ordered logical position in that expression.
        index: usize,
    },
}

/// Storage-neutral allocating binding derived only from a checked reciprocal declaration.
///
/// This retains the original exact graph and whole-family witness. A numerical owner must
/// separately supply an opaque allocation ticket, representation and live coverage checks.
#[derive(Clone, Debug)]
pub struct ReciprocalAllocation {
    /// Shared original graph-backed declaration, never reconstructed from numerical values.
    definition: PositiveReciprocalMap,
    /// Closed operation stage and component interpretation.
    stage: ReciprocalStage,
}

impl PositiveReciprocalMap {
    /// Plan one complete buffer meaning by projecting this already checked declaration.
    ///
    /// No new CAS expression, independent physical declaration or coefficient export is needed.
    #[must_use]
    pub fn allocation(&self, stage: ReciprocalStage) -> ReciprocalAllocation {
        ReciprocalAllocation {
            definition: self.clone(),
            stage,
        }
    }
}

impl ReciprocalAllocation {
    /// Original checked operation, including its physical references and exact correspondence.
    #[must_use]
    pub const fn definition(&self) -> &PositiveReciprocalMap {
        &self.definition
    }

    /// Closed schedule position; it is not a caller-supplied descriptive name.
    #[must_use]
    pub const fn stage(&self) -> ReciprocalStage {
        self.stage
    }

    /// Number of logical entries covered by this binding.
    #[must_use]
    pub fn entries(&self) -> usize {
        match self.stage {
            ReciprocalStage::References | ReciprocalStage::Images => self.definition.inputs().len(),
            ReciprocalStage::Coefficients => self.definition.map().coefficients().len(),
            ReciprocalStage::Offsets
            | ReciprocalStage::Contracted
            | ReciprocalStage::Centers
            | ReciprocalStage::Totals
            | ReciprocalStage::Errors => self.definition.map().offsets().len(),
        }
    }

    /// Check declaration identity and stage without rebuilding or comparing exact proof payloads.
    #[must_use]
    pub fn same_binding(&self, other: &Self) -> bool {
        self.stage == other.stage && std::rc::Rc::ptr_eq(&self.definition.0, &other.definition.0)
    }

    /// Exact meaning of one logical entry; an out-of-inventory selection has no meaning.
    #[must_use]
    pub fn entry(&self, index: usize) -> Option<ReciprocalEntry<'_>> {
        if index >= self.entries() {
            return None;
        }
        match self.stage {
            ReciprocalStage::References => self
                .definition
                .frame()
                .entries()
                .get(index)
                .map(ReciprocalEntry::Reference),
            ReciprocalStage::Coefficients => self
                .definition
                .map()
                .coefficients()
                .get(index)
                .map(ReciprocalEntry::Constant),
            ReciprocalStage::Offsets => self
                .definition
                .map()
                .offsets()
                .get(index)
                .map(ReciprocalEntry::Constant),
            ReciprocalStage::Images => Some(ReciprocalEntry::Image {
                input: self.definition.inputs().get(index)?,
                expression: self.definition.reciprocals().get(index)?,
                reference: self.definition.frame().entries().get(index)?,
            }),
            ReciprocalStage::Contracted
            | ReciprocalStage::Centers
            | ReciprocalStage::Totals
            | ReciprocalStage::Errors => Some(ReciprocalEntry::Output {
                expression: self.definition.image().expression(),
                index,
            }),
        }
    }
}
