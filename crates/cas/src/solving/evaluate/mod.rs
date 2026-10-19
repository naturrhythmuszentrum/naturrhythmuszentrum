// SPDX-License-Identifier: AGPL-3.0-only
//! Finite same-sort specialization, without an injectivity claim.

mod evidence;
mod finite;
mod syntax;

use crate::budget::Budget;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle};
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, ScopedConditions, Statement,
};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::elementary::evaluate::Evaluation;
use crate::solving::{
    CertificateCoefficientEvaluation, CoefficientEvaluationData, SolveCertificate, SolveDraft,
    SolveStatement,
};

/// Stable checker site, distinct from producer coefficient gaps.
const AT: Location = Location::new("finite coefficient evaluation");

/// Reject a false evaluation claim without making a mathematical negative assertion.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Rule, AT)
}

/// Match every statement subject before inspecting any finite arithmetic.
fn subjects<'a>(
    cas: &'a Cas,
    draft: &'a ApplicationDraft,
) -> Result<
    (
        &'a CertificateCoefficientEvaluation,
        &'a CoefficientEvaluationData<Handle>,
    ),
    Fault,
> {
    let Certificate::Solve(certificate) = &draft.certificate else {
        return Err(invalid());
    };
    let SolveCertificate::CoefficientEvaluation(certificate) = certificate.as_ref() else {
        return Err(invalid());
    };
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let SolveStatement::CoefficientEvaluation(statement) = statement.as_ref() else {
        return Err(invalid());
    };
    if (
        &statement.evaluation,
        &statement.left,
        &statement.right,
        &statement.region,
    ) != (
        &certificate.evaluation,
        &certificate.left,
        &certificate.right,
        &certificate.region,
    ) {
        return Err(invalid());
    }
    let SolveDraft::CoefficientEvaluation(evaluation) =
        crate::solving::graph::object(&cas.state, cas.checked(&certificate.evaluation)?)?
    else {
        return Err(invalid());
    };
    let context = cas.state.context(cas.checked(&draft.context)?)?;
    let region = cas.state.region(cas.checked(&certificate.region)?)?;
    if evaluation.region != certificate.region
        || context.admissible != certificate.region.logical_id()
        || context.header_id != region.header
    {
        return Err(invalid());
    }
    Ok((certificate, evaluation))
}

/// Validate support and prerequisites before evaluating any substituted polynomial term.
#[expect(
    clippy::redundant_pub_crate,
    reason = "finite evaluation checking is shared within the crate"
)]
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let (certificate, mapping) = subjects(cas, draft)?;
    let inherited = crate::solving::coefficients::proof::conditions(cas, draft, premises, budget)?;
    let polynomial =
        crate::solving::coefficients::storage::polynomial(&cas.state, &certificate.left)?;
    ground_target(cas, mapping, &polynomial.domain)?;
    let source = crate::solving::graph::coordinates(&cas.state, mapping.source.logical_id())?;
    budget.work(
        crate::graph::size(
            polynomial
                .variables
                .len()
                .saturating_add(polynomial.terms.len()),
            AT,
        )?,
        AT,
    )?;
    if !polynomial
        .variables
        .iter()
        .eq(source.coordinates.iter().map(|slot| &slot.field))
        || !certificate
            .support
            .iter()
            .eq(polynomial.terms.iter().map(|term| &term.coefficient))
    {
        return Err(invalid());
    }
    let evaluation = Evaluation {
        cas,
        engine: Engine {
            state: &cas.state,
            domain: &polynomial.domain,
        },
        region: &certificate.region,
        premises,
        free: &[],
        bound: &[],
    };
    evidence::check(&evaluation, mapping, certificate, budget)?;
    let images = finite::images(&evaluation, mapping, budget)?;
    finite::localization(&evaluation, mapping, certificate, &images, budget)?;
    crate::solving::coefficients::proof::defined(
        cas,
        &certificate.left,
        &certificate.region,
        premises,
        budget,
    )?;
    let left = finite::polynomial(&evaluation, &certificate.left, &images, budget)?;
    let right = syntax::read(&evaluation, &certificate.right, budget)?;
    if !left.equals(&right, &evaluation.engine, budget)? {
        return Err(invalid());
    }
    Ok(inherited)
}

/// Finite support uses only canonical ground constants; symbolic dependencies belong in its slots.
fn ground_target(
    cas: &Cas,
    mapping: &CoefficientEvaluationData<Handle>,
    domain: &Handle,
) -> Result<(), Fault> {
    use crate::solving::CoefficientDomainData;
    let SolveDraft::CoefficientDomain(domain) =
        crate::solving::graph::object(&cas.state, domain.logical_id())?
    else {
        return Err(invalid());
    };
    match domain {
        CoefficientDomainData::Rational => Ok(()),
        CoefficientDomainData::Gaussian => {
            if target_field(cas, &mapping.target)? != crate::model::Field::Complex {
                return Err(invalid());
            }
            Ok(())
        }
        _ => Err(Fault::invalid(
            InvalidCode::UnsupportedFragment,
            Location::new("finite evaluation ground coefficients"),
        )),
    }
}

/// Ground constants must belong to the declared scalar target or family fibre.
fn target_field(
    cas: &Cas,
    target: &crate::solving::EvaluationTarget<Handle>,
) -> Result<crate::model::Field, Fault> {
    use crate::solving::EvaluationTarget;
    let space = match target {
        EvaluationTarget::Scalar { space, .. } => space,
        EvaluationTarget::Family { family } => {
            let SolveDraft::SpaceFamily(shape) =
                crate::solving::graph::object(&cas.state, family.logical_id())?
            else {
                return Err(invalid());
            };
            &shape.fibre_space
        }
    };
    crate::solving::sorts::field(&cas.state, space)
}
