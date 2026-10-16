// SPDX-License-Identifier: AGPL-3.0-only
//! Scoped solve proof facts retain distinct inverse, residual and forward claims.

use crate::solve_verification::{CenterCertificate, FamilyCertificate, InverseWitness};
use crate::{Coverage, Extent, OutputReservation};
use nrz_accuracy::BudgetAssessment;
use nrz_scalar::{Precision, Scalar};

/// Independent common-operator solve certificate with separately scoped numerical evidence.
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
    /// `r_A`, `r_B`, upper `g r_A`, lower margin and the normalized total radius.
    family: Option<[f64; 5]>,
    /// Total and numerical-only Frobenius bounds after restoration.
    restored: Option<[f64; 2]>,
    /// The exact requested threshold and conservative comparison decision.
    budget: Option<BudgetAssessment<f64>>,
    /// Checked restoration map, coefficient enclosures and arithmetic revision.
    restoration: Option<Box<crate::ConversionEvidence>>,
    /// Explicit inverse output has independently checked identity RHS semantics.
    inverse_output: bool,
    /// Independent certificates of every logical differential solve, before restoration.
    pub(crate) sensitivities: Vec<super::SensitivityEvidence>,
    /// Complete bounded attempt history, including the explicit stopping reason.
    pub(crate) recovery: Option<super::SolveRecoveryEvidence>,
    /// Origin and checked reuse key, independent of the current RHS proof.
    pub(crate) factor: Option<super::FactorUse>,
}

impl SolveEvidence {
    /// Retain a valid stored-center proof even when the later input-family margin fails.
    pub(crate) fn represented<P: Scalar>(
        contract: &super::SolveContract,
        reservation: OutputReservation,
        witness: &InverseWitness<P>,
        certificate: &CenterCertificate<P>,
        revision: u32,
    ) -> Self {
        Self {
            extent: reservation.extent(),
            coverage: Coverage::RepresentedInputs,
            work: contract.work,
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
            family: None,
            restored: None,
            budget: None,
            restoration: None,
            inverse_output: contract.identity.is_some(),
            sensitivities: Vec::new(),
            recovery: None,
            factor: None,
        }
    }
    /// Add the complete selected input scope only after its strict common-operator theorem succeeds.
    pub(crate) fn include_family<P: Scalar>(
        &mut self,
        family: &FamilyCertificate<P>,
        coverage: Coverage,
    ) {
        self.family = Some([
            family.operator_radius.to_f64(),
            family.rhs_radius.to_f64(),
            family.product.to_f64(),
            family.margin.to_f64(),
            family.radius.to_f64(),
        ]);
        self.coverage = coverage;
    }
    /// Complete the final coordinate and budget interpretation without discarding earlier evidence.
    pub(crate) fn complete<P: Scalar>(
        &mut self,
        restored: &super::restoration::Restored<P>,
        budget: BudgetAssessment<f64>,
    ) {
        self.revision = restored.revision;
        self.restored = Some([
            restored.total_norm.to_f64(),
            restored.numerical_norm.to_f64(),
        ]);
        self.budget = Some(budget);
        self.restoration = restored.evidence.clone().map(Box::new);
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

impl SolveEvidence {
    /// Whether complete input-family, restoration and budget evidence is available.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.family.is_some() && self.restored.is_some() && self.budget.is_some()
    }
    /// Frobenius upper bound of the complete selected operator total, absent on a partial center proof.
    #[must_use]
    pub fn operator_radius(&self) -> Option<f64> {
        self.family.map(|values| values[0])
    }
    /// Joint Frobenius upper bound of all RHS total entries.
    #[must_use]
    pub fn rhs_radius(&self) -> Option<f64> {
        self.family.map(|values| values[1])
    }
    /// Upper bound on `g r_A`; every successful family has this strictly below one.
    #[must_use]
    pub fn perturbation_product(&self) -> Option<f64> {
        self.family.map(|values| values[2])
    }
    /// Positive lower bound on `1 - g r_A`, shared across all RHS columns.
    #[must_use]
    pub fn perturbation_margin(&self) -> Option<f64> {
        self.family.map(|values| values[3])
    }
    /// Normalized joint total radius before conservative entrywise restoration.
    #[must_use]
    pub fn total_radius(&self) -> Option<f64> {
        self.family.map(|values| values[4])
    }
    /// Joint total Frobenius radius in the actual output difference coordinates.
    #[must_use]
    pub fn output_radius(&self) -> Option<f64> {
        self.restored.map(|values| values[0])
    }
    /// Overlapping joint numerical-only bound after restoration.
    #[must_use]
    pub fn output_numerical_radius(&self) -> Option<f64> {
        self.restored.map(|values| values[1])
    }
    /// Completed budget assessment, absent before restoration; retained on budget refusal.
    #[must_use]
    pub const fn budget(&self) -> Option<BudgetAssessment<f64>> {
        self.budget
    }
    /// Exact restoration definition and its separately enclosed coefficients.
    #[must_use]
    pub fn restoration(&self) -> Option<&crate::ConversionEvidence> {
        self.restoration.as_deref()
    }
    /// Whether this result solves the separately derived typed identity to publish an inverse.
    #[must_use]
    pub const fn is_inverse(&self) -> bool {
        self.inverse_output
    }
}

impl SolveEvidence {
    /// Per-direction residual and forward bounds for the exact nominal differential.
    #[must_use]
    pub fn sensitivities(&self) -> &[super::SensitivityEvidence] {
        &self.sensitivities
    }
}

impl SolveEvidence {
    /// Bounded recovery attempts and stop; absent for the ordinary single-attempt policy.
    #[must_use]
    pub const fn recovery(&self) -> Option<&super::SolveRecoveryEvidence> {
        self.recovery.as_ref()
    }
}

impl SolveEvidence {
    /// Factor origin and dependency-specific reuse decision for this exact solve execution.
    #[must_use]
    pub const fn factor_use(&self) -> Option<&super::FactorUse> {
        self.factor.as_ref()
    }
}
