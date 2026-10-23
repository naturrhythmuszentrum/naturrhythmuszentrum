// SPDX-License-Identifier: AGPL-3.0-only
//! Reversible exact elimination, independent rank bounds and original affine systems.
//!
//! # Linear and family certificates
//!
//! The supplied linear and family checks enable tags 9 and 16. Reversible row operations reconstruct the full matrix, nonzero echelon pivots certify a triangular minor, and all remaining rows must vanish for the rank upper bound. A pivot or scale may use an earlier checked scalar inverse identity with the same normalized pivot, exact coefficient map, context and region; both endpoints remain defined and their product is checked through the root relations. Canonically embedded nonzero ground constants in fractions and constant tower vectors are units, but formal parameter numerators and arbitrary quotient representatives are not. Affine systems retain their original equations and augmented ranks. Binomial coverage and linear-system claims must stay inside the original coefficient region, as do product covers, cover elimination, transport and family certificates. Family membership substitutes arbitrary bound parameters; coverage additionally checks an exact reverse-space recovery map, its full domain and the identity on every original solution. The zero polynomial therefore admits its whole-domain identity family. Automatic linear-family production remains separate.

mod matrix;
mod system;

use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::proof_model::{ApplicationDraft, CheckedApplication, ScopedConditions, Statement};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::elementary::evaluate::Evaluation;
use crate::solving::elementary::proof::{conditions, invalid, problem};
use crate::solving::{
    CertificateExactLinearEquivalence, SolveCertificate, SolveSchema, SolveStatement,
};

/// Supplied linear certificates are checked; automatic family search remains a separate leaf.
pub(crate) const fn availability() -> super::Availability {
    super::Availability::Available
}

/// Only the exact reversible linear schema belongs to this leaf.
pub(crate) const fn schema_availability(tag: u8) -> super::Availability {
    if tag == 9 {
        availability()
    } else {
        super::Availability::Unavailable
    }
}

/// The statement fixes the coefficient algebra; no floating matrix interpretation is admitted.
fn domain<'a>(cas: &'a Cas, statement: &'a SolveStatement) -> Result<&'a Handle, Fault> {
    match statement {
        SolveStatement::Rank(value) => Ok(&value.domain),
        SolveStatement::LinearSystem(value) => {
            Ok(&problem(cas, &value.problem)?.coefficient_domain)
        }
        SolveStatement::Equivalent(value) => Ok(&problem(cas, &value.source)?.coefficient_domain),
        _ => Err(invalid()),
    }
}

/// Exact dimensions and both rank bounds remain part of the proposition, not certificate hints.
fn conclusion(
    evaluation: &Evaluation<'_>,
    draft: &ApplicationDraft,
    certificate: &CertificateExactLinearEquivalence,
    statement: &SolveStatement,
    original: &matrix::Matrix,
    reduced: &matrix::Matrix,
    budget: &mut Budget,
) -> Result<(), Fault> {
    match statement {
        SolveStatement::Rank(value) => {
            if (
                &value.matrix,
                &value.pivots,
                &value.minor,
                &value.elimination,
                value.rank,
                certificate.augmented_columns,
            ) != (
                &certificate.matrix,
                &certificate.pivots,
                &certificate.minor,
                &certificate.reduced,
                u32::try_from(certificate.pivots.len()).map_err(|_| invalid())?,
                0,
            ) {
                return Err(invalid());
            }
            Ok(())
        }
        SolveStatement::LinearSystem(value) => {
            system::check(evaluation, value, certificate, original, budget)
        }
        SolveStatement::Equivalent(value) => {
            if certificate.augmented_columns != 1 {
                return Err(invalid());
            }
            system::reconstruct(evaluation, &value.source, original, budget)?;
            system::reconstruct(evaluation, &value.target, reduced, budget)?;
            let transport = crate::solving::CertificateSolutionTransport {
                source: value.source.clone(),
                target: value.target.clone(),
                forward: value.forward.clone(),
                backward: value.backward.clone(),
                region: value.region.clone(),
                definedness: Vec::new(),
                equations: Vec::new(),
                forward_inverse: Vec::new(),
                backward_inverse: Vec::new(),
            };
            crate::solving::elementary::transport::check(
                evaluation.cas,
                draft,
                &transport,
                evaluation.premises,
                budget,
            )
        }
        _ => Err(invalid()),
    }
}

/// Execute every row operation, compare the whole result and check the lower and upper rank bounds.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let crate::proof_model::Certificate::Solve(certificate) = &draft.certificate else {
        return Err(invalid());
    };
    let (
        crate::rule::Primitive::Solve(SolveSchema::ExactLinearEquivalence),
        SolveCertificate::ExactLinearEquivalence(value),
    ) = (draft.rule, certificate.as_ref())
    else {
        return Err(invalid());
    };
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let (inherited, _) = conditions(cas, draft, premises, budget)?;
    let context = cas.state.context(cas.checked(&draft.context)?)?;
    if context.admissible != value.region.logical_id() {
        return Err(invalid());
    }
    let evaluation = Evaluation {
        exclusions: &[],
        cas,
        engine: Engine {
            state: &cas.state,
            domain: domain(cas, statement)?,
        },
        region: &value.region,
        premises,
        free: &[],
        bound: &[],
    };
    let original = matrix::Matrix::read(&evaluation, &value.matrix, budget)?;
    let reduced = matrix::Matrix::read(&evaluation, &value.reduced, budget)?;
    let mut transformed = matrix::Matrix::read(&evaluation, &value.matrix, budget)?;
    for operation in &value.operations {
        transformed.operation(&evaluation, &draft.context, operation, budget)?;
    }
    transformed.equal(&evaluation, &reduced, budget)?;
    reduced.rank(
        &evaluation,
        &draft.context,
        &value.pivots,
        &value.minor,
        budget,
    )?;
    conclusion(
        &evaluation,
        draft,
        value,
        statement,
        &original,
        &reduced,
        budget,
    )?;
    Ok(inherited)
}
