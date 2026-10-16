// SPDX-License-Identifier: AGPL-3.0-only
//! Retained constant lowering facts, separate from the output's numerical error region.

use crate::coordinate_numeric::LoweredMap;
use crate::{CopyEvidence, Coverage};
use nrz_scalar::{DecodedLiteral, Precision, Scalar};
use nrz_units::CheckedMap;

/// Constant candidate, enclosure and evaluation error, widened exactly for inspection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstantEvidence {
    /// Represented coefficient candidate.
    center: f64,
    /// Outward endpoints enclosing the exact descriptor definition.
    enclosure: [f64; 2],
    /// Outward bound on the coefficient candidate's displacement.
    error: f64,
}

impl ConstantEvidence {
    /// Retain exact binary widening; no arithmetic on descriptor values occurs here.
    fn from_decoded<P: Scalar>(decoded: DecodedLiteral<P>) -> Self {
        Self {
            center: decoded.center().get().to_f64(),
            enclosure: [
                decoded.enclosure().lower().get().to_f64(),
                decoded.enclosure().upper().get().to_f64(),
            ],
            error: decoded.displacement().get().to_f64(),
        }
    }
    /// Candidate coefficient in its original format, exactly widened to binary64.
    #[must_use]
    pub const fn center(self) -> f64 {
        self.center
    }
    /// Exact-definition enclosure endpoints, exactly widened to binary64.
    #[must_use]
    pub const fn enclosure(self) -> [f64; 2] {
        self.enclosure
    }
    /// Coefficient evaluation-error bound; this is not physical scale uncertainty.
    #[must_use]
    pub const fn error(self) -> f64 {
        self.error
    }
}

/// Proof recipe for one converted region and its separately published error companion.
#[derive(Clone, Debug, PartialEq)]
pub struct ConversionEvidence {
    /// Source/output position in the atomic transaction.
    pub(crate) slot: usize,
    /// Number of coordinates covered by each companion region.
    pub(crate) entries: usize,
    /// Physical scope retained from the input, excluding invented physical exactness.
    pub(crate) coverage: Coverage,
    /// Full exact coefficient definitions and snapshot identity.
    map: CheckedMap,
    /// Scale and shift lowering results, in that order.
    constants: [ConstantEvidence; 2],
    /// Sealed arithmetic format used by the conversion.
    precision: Precision,
    /// Admitted scalar arithmetic revision.
    revision: u32,
}

impl ConversionEvidence {
    /// Capture the coefficients actually used, without re-exporting or lowering them.
    pub(crate) fn new<P: Scalar>(
        slot: usize,
        entries: usize,
        coverage: Coverage,
        map: &LoweredMap<P>,
    ) -> Self {
        Self {
            slot,
            entries,
            coverage,
            map: map.exact.clone(),
            constants: [
                ConstantEvidence::from_decoded(map.scale),
                ConstantEvidence::from_decoded(map.shift),
            ],
            precision: P::PRECISION,
            revision: map.revision,
        }
    }
    /// Exact map identifying the coefficient definitions and declared origins/references.
    #[must_use]
    pub const fn map(&self) -> &CheckedMap {
        &self.map
    }
    /// Scale and shift candidates with their own numerical enclosures and errors.
    #[must_use]
    pub const fn constants(&self) -> &[ConstantEvidence; 2] {
        &self.constants
    }
    /// Sealed arithmetic format of the actual kernel.
    #[must_use]
    pub const fn precision(&self) -> Precision {
        self.precision
    }
    /// Scalar rounding proof revision.
    #[must_use]
    pub const fn arithmetic_revision(&self) -> u32 {
        self.revision
    }
}

/// Independently established evidence for a sealed native recipe.
#[derive(Clone, Debug, PartialEq)]
pub enum RecipeEvidence {
    /// Independent inverse-defect and residual certificate for a square solve.
    Solve(crate::SolveEvidence),
    /// Complete deterministic propagation and independently checked numerical error.
    Kernel(crate::KernelEvidence),
    /// Exact represented-bit identity and retention of the complete companion set.
    Copy(CopyEvidence),
    /// Outward affine evaluation and propagated deterministic companions.
    Conversion(Box<ConversionEvidence>),
    /// New representation with retained prior bounds and a separate center displacement.
    Precision(PrecisionEvidence),
}

impl RecipeEvidence {
    /// Source and output position covered by this claim.
    #[must_use]
    pub const fn slot(&self) -> usize {
        match self {
            Self::Solve(_) | Self::Kernel(_) => 0,
            Self::Copy(claim) => claim.slot(),
            Self::Conversion(claim) => claim.slot,
            Self::Precision(claim) => claim.slot,
        }
    }
    /// Number of logical center entries covered by this claim.
    #[must_use]
    pub const fn entries(&self) -> usize {
        match self {
            Self::Solve(claim) => claim.extent.entries(),
            Self::Kernel(claim) => claim.entries,
            Self::Copy(claim) => claim.entries(),
            Self::Conversion(claim) => claim.entries,
            Self::Precision(claim) => claim.entries,
        }
    }
    /// Retained physical or represented-input scope.
    #[must_use]
    pub const fn coverage(&self) -> Coverage {
        match self {
            Self::Solve(claim) => claim.coverage,
            Self::Kernel(claim) => claim.coverage,
            Self::Copy(claim) => claim.coverage(),
            Self::Conversion(claim) => claim.coverage,
            Self::Precision(claim) => claim.coverage,
        }
    }
}

/// Numerical coverage of one explicit representation change, with no unit-map evaluation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrecisionEvidence {
    /// Position in the connected conversion bundle.
    pub(crate) slot: usize,
    /// Number of logical entries covered by the total and displacement regions.
    pub(crate) entries: usize,
    /// Original deterministic problem scope.
    pub(crate) coverage: Coverage,
    /// Source arena precision.
    pub(crate) source: Precision,
    /// Newly allocated arena precision.
    pub(crate) target: Precision,
    /// Scalar environment/rounding proof revision.
    pub(crate) revision: u32,
}

impl PrecisionEvidence {
    /// Original arena format, which remains unchanged and readable.
    #[must_use]
    pub const fn source_precision(&self) -> Precision {
        self.source
    }
    /// Format of the fresh output representation.
    #[must_use]
    pub const fn target_precision(&self) -> Precision {
        self.target
    }
    /// Scalar rounding proof revision for endpoint and displacement conversion.
    #[must_use]
    pub const fn arithmetic_revision(&self) -> u32 {
        self.revision
    }
}
