// SPDX-License-Identifier: AGPL-3.0-only
//! Derived symmetry and explicit substitution recipes, sealed only by existing replay checkers.

use super::AT;
use super::algebra::Polynomial;
use super::reduction::Reduction;
use super::render::Rendering;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::proof_model::{ApplicationDraft, Certificate, Statement, VerifiedWitness};
use crate::solving::coefficients::storage::{copied, invalid, reserve, vector};
use crate::solving::{
    CertificateCoefficientIdentity, IdentityMode, SolveCertificate, SolveSchema, SolveStatement,
    StatementCoefficientEquality,
};

/// Every evaluation names its context and coefficient map; root images and parameter images are separate.
#[derive(Debug)]
pub(crate) struct Evaluation<'a> {
    /// Complete caller scope, including any required map hypotheses.
    pub(crate) context: &'a Handle,
    /// Explicit scalar coefficient map; no quotient field property is inferred.
    pub(crate) map: &'a Handle,
    /// Ordered root expressions used to state the universal expansion.
    pub(crate) roots: &'a [Handle],
    /// Ordered coefficient expressions used in the subsequent formal substitution.
    pub(crate) parameters: &'a [Handle],
    /// Earlier untrusted applications, independently checked as part of this proof DAG.
    pub(crate) prefix: Vec<ApplicationDraft>,
}

/// A sealed derived recipe plus its graph-owned elementary expression and substituted coefficients.
#[derive(Debug)]
pub(crate) struct CertifiedReduction {
    /// Canonical polynomial in elementary-symmetric slots.
    pub(crate) expression: Handle,
    /// Canonical polynomial after the caller's explicit coefficient substitution.
    pub(crate) substituted: Handle,
    /// Contains orbit invariance, expansion and substitution applications in its reachable closure.
    pub(crate) witness: VerifiedWitness,
}

/// Every earlier application remains reachable, retaining all residuals and map hypotheses.
fn premise_slots(count: usize, budget: &mut Budget) -> Result<Vec<u32>, Fault> {
    let mut result = vector(count, budget)?;
    for index in 0..count {
        result.push(u32::try_from(index).map_err(|_| invalid())?);
    }
    Ok(result)
}

/// Submit an exact identity obligation; it is never sealed by a producer-computed flag.
fn identity(
    rendering: &Rendering,
    context: &Handle,
    endpoints: [Handle; 2],
    premises: Vec<u32>,
    cas: &Cas,
    budget: &mut Budget,
) -> Result<ApplicationDraft, Fault> {
    budget.solving(BudgetKind::SolveIdentities, 1, AT)?;
    let denominators = map_denominators(cas, &rendering.map, budget)?;
    let [left, right] = endpoints;
    Ok(ApplicationDraft {
        rule: crate::rule::Primitive::Solve(SolveSchema::CoefficientIdentity),
        context: context.clone(),
        conclusion: Statement::Solve(Box::new(SolveStatement::CoefficientEquality(
            StatementCoefficientEquality {
                left: left.clone(),
                right: right.clone(),
                map: rendering.map.clone(),
                region: rendering.region.clone(),
            },
        ))),
        premises,
        substitutions: Vec::new(),
        certificate: Certificate::Solve(Box::new(SolveCertificate::CoefficientIdentity(
            CertificateCoefficientIdentity {
                mode: IdentityMode::Normalize,
                left,
                right,
                map: rendering.map.clone(),
                region: rendering.region.clone(),
                combinations: Vec::new(),
                denominators,
            },
        ))),
    })
}

/// Denominator obligations are copied exactly, without cancellation or inferred regional units.
fn map_denominators(cas: &Cas, map: &Handle, budget: &mut Budget) -> Result<Vec<Handle>, Fault> {
    let crate::solving::SolveDraft::CoefficientMap(data) =
        crate::solving::graph::object(&cas.state, cas.checked(map)?)?
    else {
        return Err(invalid());
    };
    copied(&data.denominators, budget)
}

/// Render the visibly nested elementary expansion, so the checker independently multiplies it out.
fn expansion(
    cas: &mut Cas,
    reduction: &Reduction,
    rendering: &Rendering,
    roots: &[Handle],
    budget: &mut Budget,
) -> Result<[Handle; 2], Fault> {
    let left = rendering.polynomial(cas, &reduction.source, roots, budget)?;
    let mut elementary = vector(reduction.basis.len(), budget)?;
    for value in &reduction.basis {
        elementary.push(rendering.polynomial(cas, value, roots, budget)?);
    }
    let right = rendering.polynomial(cas, &reduction.expression, &elementary, budget)?;
    Ok([left, right])
}

/// Formal substitution has its own explicit expression identity, including any coincident images.
fn substitution(
    cas: &mut Cas,
    expression: &Polynomial,
    images: &[Polynomial],
    rendering: &Rendering,
    parameters: &[Handle],
    budget: &mut Budget,
) -> Result<(Polynomial, [Handle; 2]), Fault> {
    let substituted = expression.compose(images, &cas.state, budget)?;
    let mut evaluated = vector(images.len(), budget)?;
    for image in images {
        evaluated.push(rendering.polynomial(cas, image, parameters, budget)?);
    }
    let left = rendering.polynomial(cas, expression, &evaluated, budget)?;
    let right = rendering.polynomial(cas, &substituted, parameters, budget)?;
    Ok((substituted, [left, right]))
}

/// Generate finite symmetry, expansion and substitution applications in an unpublished candidate.
fn prepare(
    cas: &mut Cas,
    reduction: &Reduction,
    images: &[Polynomial],
    evaluation: Evaluation<'_>,
    budget: &mut Budget,
    checker: &mut Budget,
) -> Result<CertifiedReduction, Fault> {
    let rendering = Rendering::new(cas, evaluation.map, budget)?;
    let mut steps = evaluation.prefix;
    reserve(&mut steps, 3, budget)?;
    let orbit = crate::solving::orbits::produce::enumerate(&reduction.source, &cas.state, budget)?;
    if orbit.members.len() != 1 {
        return Err(invalid());
    }
    steps.push(orbit.application(cas, evaluation.context, budget)?);
    let endpoints = expansion(cas, reduction, &rendering, evaluation.roots, budget)?;
    let premises = premise_slots(steps.len(), budget)?;
    steps.push(identity(
        &rendering,
        evaluation.context,
        endpoints,
        premises,
        cas,
        budget,
    )?);
    let (substituted, endpoints) = substitution(
        cas,
        &reduction.expression,
        images,
        &rendering,
        evaluation.parameters,
        budget,
    )?;
    let premises = premise_slots(steps.len(), budget)?;
    steps.push(identity(
        &rendering,
        evaluation.context,
        endpoints,
        premises,
        cas,
        budget,
    )?);
    let expression = reduction.expression.stage(cas, budget)?;
    let substituted = substituted.stage(cas, budget)?;
    let witness = cas.check_proof(evaluation.context, &steps, checker)?;
    Ok(CertifiedReduction {
        expression,
        substituted,
        witness,
    })
}

/// Publish the entire derived result only after independent replay of every finite certificate.
pub(crate) fn certify(
    cas: &mut Cas,
    reduction: &Reduction,
    images: &[Polynomial],
    evaluation: Evaluation<'_>,
    budget: &mut Budget,
    checker: &mut Budget,
) -> Result<CertifiedReduction, Fault> {
    let mut candidate = cas.fork(budget)?;
    let result = prepare(
        &mut candidate,
        reduction,
        images,
        evaluation,
        budget,
        checker,
    )?;
    cas.state = candidate.state;
    Ok(result)
}
