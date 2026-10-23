// SPDX-License-Identifier: AGPL-3.0-only
//! Finite strict-sign proposals reuse the existing ordered-field proof rules.

mod build;
mod guards;

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle};
use crate::proof_model::{ApplicationDraft, Certificate, Statement, VerifiedWitness};
use crate::solving::{
    CertificateRealSign, SolveCertificate, SolveSchema, SolveStatement, StatementRealSign,
};

/// Shared location for multiplicative sign proposals.
const AT: Location = Location::new("strict sign transport");

/// Missing strict evidence is a capability refusal, never an inferred sign.
const fn unavailable() -> Fault {
    Fault::invalid(InvalidCode::StrictSignUnavailable, AT)
}

/// Conservative simultaneous proposal arrays, including binary argument slots.
pub(crate) fn payload(cas: &Cas, subject: &Handle) -> Result<u64, Fault> {
    Ok(cas
        .state
        .expression(cas.checked(subject)?)?
        .unfolded
        .saturating_mul(256)
        .saturating_add(512))
}

/// Propose only rational signs, declared strict guards and their multiplicative closure.
pub(crate) fn certificate(
    cas: &Cas,
    subject: &Handle,
    region: &Handle,
    budget: &mut Budget,
) -> Result<CertificateRealSign, Fault> {
    let bytes = payload(cas, subject)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.with_retained_payload(bytes, AT, |budget| {
        build::certificate(cas, subject, region, budget)
    })
}

impl Cas {
    /// Prove the strict sign of products, quotients, negations and positive integer powers.
    ///
    /// Factors need exact rational signs or explicit strict guards in the context's
    /// region. A negative factor uses a positive guard on its negation. Denominators
    /// must be strictly signed; samples and merely nonnegative guards do not qualify.
    /// The ordinary witness retains the complete region and replays with `Cas::replay`.
    ///
    /// # Errors
    /// Returns `InvalidCode::StrictSignUnavailable` for an unsigned factor, zero,
    /// unsupported syntax or mismatched Real frame, and propagates resource refusals.
    pub fn prove_strict_sign(
        &mut self,
        context: &Handle,
        subject: &Handle,
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        self.proof_limits(budget, |cas, budget| {
            let region = cas.admissible_region(context)?;
            let certificate = certificate(cas, subject, &region, budget)?;
            let conclusion = certificate.steps.last().ok_or_else(unavailable)?.conclusion;
            let step = ApplicationDraft {
                rule: crate::rule::Primitive::Solve(SolveSchema::RealSign),
                context: context.clone(),
                conclusion: Statement::Solve(Box::new(SolveStatement::RealSign(
                    StatementRealSign {
                        subject: subject.clone(),
                        region,
                        conclusion,
                    },
                ))),
                premises: Vec::new(),
                substitutions: Vec::new(),
                certificate: Certificate::Solve(Box::new(SolveCertificate::RealSign(certificate))),
            };
            budget.with_retained_payload(payload(cas, subject)?, AT, |budget| {
                cas.transaction(budget, |candidate, budget| {
                    let witness = candidate.check_proof(context, &[step], budget)?;
                    budget.check(
                        BudgetKind::OutputBytes,
                        crate::graph::size(witness.exported_byte_len(), AT)?,
                        AT,
                    )?;
                    Ok(witness)
                })
            })
        })
    }
}
