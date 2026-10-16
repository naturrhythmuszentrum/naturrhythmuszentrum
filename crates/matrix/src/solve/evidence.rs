// SPDX-License-Identifier: AGPL-3.0-only
//! Scoped solve proof facts retain distinct inverse, residual and forward claims.

use crate::solve_verification::{CenterCertificate, InverseWitness};
use crate::{Coverage, Extent};
use nrz_scalar::{Precision, Scalar};

/// Independent represented-center certificate; its private inverse witness is not an inverse value.
#[derive(Clone, Debug, PartialEq)]
pub struct SolveEvidence {
    /// Complete solution rectangle, including all common right-hand sides.
    pub(crate) extent: Extent,
    /// Named deterministic problem covered by the total companion.
    pub(crate) coverage: Coverage,
    /// Candidate and verifier format.
    precision: Precision,
    /// Scalar arithmetic proof revision.
    revision: u32,
    /// Charged conservative complete-phase work/scratch count.
    work: usize,
    /// `q`, `h`, lower `1-q`, and `g`, widened exactly for inspection.
    inverse: [f64; 4],
    /// `rho`, `z`, and the numerical-only forward radius.
    center: [f64; 3],
}

impl SolveEvidence {
    /// Capture already established facts without performing floating verification in metadata.
    pub(crate) fn new<P: Scalar>(
        extent: Extent,
        coverage: Coverage,
        work: usize,
        witness: &InverseWitness<P>,
        certificate: &CenterCertificate<P>,
        revision: u32,
    ) -> Self {
        Self {
            extent,
            coverage,
            work,
            precision: P::PRECISION,
            revision,
            inverse: [
                witness.defect.to_f64(),
                witness.magnitude.to_f64(),
                witness.margin.to_f64(),
                witness.inverse.to_f64(),
            ],
            center: [
                certificate.residual.to_f64(),
                certificate.magnitude.to_f64(),
                certificate.radius.to_f64(),
            ],
        }
    }
    /// Candidate and independent verifier precision.
    #[must_use]
    pub const fn precision(&self) -> Precision {
        self.precision
    }
    /// Scalar rounding proof revision.
    #[must_use]
    pub const fn arithmetic_revision(&self) -> u32 {
        self.revision
    }
    /// Conservative work and scratch admission count.
    #[must_use]
    pub const fn work(&self) -> usize {
        self.work
    }
    /// Frobenius upper bound on `I - Z A` in the saved normalized coordinates.
    #[must_use]
    pub const fn inverse_defect(&self) -> f64 {
        self.inverse[0]
    }
    /// Frobenius upper bound on the private inverse candidate.
    #[must_use]
    pub const fn witness_norm(&self) -> f64 {
        self.inverse[1]
    }
    /// Positive lower bound on `1 - q`.
    #[must_use]
    pub const fn inverse_margin(&self) -> f64 {
        self.inverse[2]
    }
    /// Proved spectral inverse-norm upper bound.
    #[must_use]
    pub const fn inverse_norm(&self) -> f64 {
        self.inverse[3]
    }
    /// Frobenius upper bound on the exact represented-center residual.
    #[must_use]
    pub const fn residual_norm(&self) -> f64 {
        self.center[0]
    }
    /// Frobenius upper bound on all candidate solution columns together.
    #[must_use]
    pub const fn candidate_norm(&self) -> f64 {
        self.center[1]
    }
    /// Numerical-only normalized forward radius, already included in the total.
    #[must_use]
    pub const fn numerical_radius(&self) -> f64 {
        self.center[2]
    }
}
