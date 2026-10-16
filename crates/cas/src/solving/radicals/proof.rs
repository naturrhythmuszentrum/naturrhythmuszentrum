// SPDX-License-Identifier: AGPL-3.0-only
//! Independently checked full-factor coverage of the original polynomial and requested presentation.

use super::construction::Construction;
use super::roots::Values;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle, Reference};
use crate::model::Operator;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::maps::canonical;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{invalid, vector};
use crate::solving::towers::{dense::Dense, materialize};
use crate::solving::{
    CertificateCoefficientIdentity, CertificateProductCompleteness,
    CertificateRootSelectionRefinement, IdentityMode, RefinementMode, RefinementTargetKind,
    RootFactor, SolutionBranchData, SolutionBranchKind, SolutionSetData, SolveAnswer,
    SolveCertificate, SolveDraft, SolveForm, SolveSchema, SolveStatement,
    StatementCoefficientEquality, StatementCoverage, StatementPresentation, TupleData,
};

/// Emit exact normalization with the same endpoints and scalar interpretation in statement and certificate.
pub(crate) fn identity(
    construction: &mut Construction,
    endpoints: [&Handle; 2],
    map: &Handle,
    budget: &mut Budget,
) -> Result<u32, Fault> {
    let [left, right] = endpoints;
    construction.push(
        SolveSchema::CoefficientIdentity,
        SolveStatement::CoefficientEquality(StatementCoefficientEquality {
            left: left.clone(),
            right: right.clone(),
            map: map.clone(),
            region: construction.region.clone(),
        }),
        SolveCertificate::CoefficientIdentity(CertificateCoefficientIdentity {
            mode: IdentityMode::Normalize,
            left: left.clone(),
            right: right.clone(),
            map: map.clone(),
            region: construction.region.clone(),
            combinations: Vec::new(),
            denominators: Vec::new(),
        }),
        budget,
    )
}

/// The replay checker expands the discovered symmetric expression independently of the producer reduction.
pub(crate) fn reduction(
    cas: &mut Cas,
    construction: &mut Construction,
    reduction: &crate::solving::symmetric::reduction::Reduction,
    map: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let expanded = reduction
        .expression
        .compose(&reduction.basis, &cas.state, budget)?;
    let left = reduction.source.stage(cas, budget)?;
    let right = expanded.stage(cas, budget)?;
    identity(construction, [&left, &right], map, budget)?;
    Ok(())
}

/// Replay the actual elementary-slot replacement as a visible nested scalar expression.
pub(crate) fn substitution(
    cas: &mut Cas,
    construction: &mut Construction,
    expression: &crate::solving::symmetric::algebra::Polynomial,
    images: &[crate::solving::symmetric::algebra::Polynomial],
    map: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let rendering = crate::solving::symmetric::render::Rendering::new(cas, map, budget)?;
    let mut evaluated = vector(images.len(), budget)?;
    for image in images {
        evaluated.push(rendering.polynomial(cas, image, &[], budget)?);
    }
    let left = rendering.polynomial(cas, expression, &evaluated, budget)?;
    let composed = expression.compose(images, &cas.state, budget)?;
    let right = rendering.polynomial(cas, &composed, &[], budget)?;
    identity(construction, [&left, &right], map, budget)?;
    Ok(())
}

/// A complete candidate set has one exact original-domain branch and deduplicated tuple syntax.
fn set(
    cas: &mut Cas,
    construction: &Construction,
    problem: &Handle,
    entries: &[Handle],
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let original = crate::solving::elementary::proof::problem(cas, problem)?;
    let (coordinates, domain) = (original.unknowns.clone(), original.domain.clone());
    let mut tuples = vector(entries.len(), budget)?;
    for entry in entries {
        let tuple = materialize::admit(
            cas,
            SolveDraft::Tuple(TupleData {
                header: Reference::Existing(construction.header.clone()),
                scope: Vec::new(),
                coordinates: Reference::Existing(coordinates.clone()),
                entries: vec![Reference::Existing(entry.clone())],
            }),
            budget,
        )?;
        budget.work(
            crate::graph::size(tuples.len(), super::AT)?.max(1),
            super::AT,
        )?;
        let reference = Reference::Existing(tuple);
        if !tuples.contains(&reference) {
            tuples.push(reference);
        }
    }
    let kind = if tuples.is_empty() {
        SolutionBranchKind::Empty
    } else {
        SolutionBranchKind::Finite { tuples }
    };
    let branch = materialize::admit(
        cas,
        SolveDraft::SolutionBranch(SolutionBranchData {
            problem: Reference::Existing(problem.clone()),
            region: Reference::Existing(construction.region.clone()),
            domain: Reference::Existing(domain),
            kind,
        }),
        budget,
    )?;
    materialize::admit(
        cas,
        SolveDraft::SolutionSet(SolutionSetData {
            problem: Reference::Existing(problem.clone()),
            branches: vec![Reference::Existing(branch)],
        }),
        budget,
    )
}

/// Render the original coefficient vector in the final algebra, retaining the original unknown expression.
fn original(
    cas: &mut Cas,
    construction: &Construction,
    polynomial: &Dense,
    values: &Values,
    unknown: &Handle,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let engine = Engine {
        state: &cas.state,
        domain: &values.domain,
    };
    let zero = engine.constant(false, budget).map_err(|e| fault(&e))?;
    let mut result = construction.literal(cas, &zero, &values.map, budget)?;
    for coefficient in polynomial.coefficients.iter().rev() {
        let value =
            canonical(&cas.state, coefficient, &values.domain, budget).map_err(|e| fault(&e))?;
        let literal = construction.literal(cas, &value, &values.map, budget)?;
        let product = construction.expression(
            cas,
            Operator::Mul,
            &[result, unknown.clone()],
            false,
            budget,
        )?;
        result = construction.expression(cas, Operator::Add, &[product, literal], false, budget)?;
    }
    Ok(result)
}

/// Every original factor occurrence is retained; coincident roots are not mistaken for multiplicity one.
fn factors(
    cas: &mut Cas,
    construction: &Construction,
    values: &Values,
    scalar: &Handle,
    unknown: &Handle,
    budget: &mut Budget,
) -> Result<(Vec<Handle>, Handle), Fault> {
    let mut entries = vector(values.roots.len(), budget)?;
    let mut product = scalar.clone();
    for root in &values.roots {
        let literal = construction.literal(cas, root, &values.map, budget)?;
        let factor = construction.expression(
            cas,
            Operator::Sub,
            &[unknown.clone(), literal.clone()],
            false,
            budget,
        )?;
        product = construction.expression(cas, Operator::Mul, &[product, factor], false, budget)?;
        entries.push(literal);
    }
    Ok((entries, product))
}

/// Full factor multiplication is a separate identity obligation before any zero-product coverage theorem.
pub(crate) fn coverage(
    cas: &mut Cas,
    construction: &mut Construction,
    problem: &Handle,
    polynomial: &Dense,
    values: &Values,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let original_problem = crate::solving::elementary::proof::problem(cas, problem)?;
    let coordinates =
        crate::solving::elementary::proof::coordinates(cas, &original_problem.unknowns)?;
    let [coordinate] = coordinates.coordinates.as_slice() else {
        return Err(invalid());
    };
    let key = coordinate.key.clone();
    let unknown = construction.expression(cas, Operator::Free(key), &[], false, budget)?;
    let leading = polynomial.coefficients.last().ok_or_else(invalid)?;
    let scale = canonical(&cas.state, leading, &values.domain, budget).map_err(|e| fault(&e))?;
    let scalar = construction.literal(cas, &scale, &values.map, budget)?;
    let (entries, product) = factors(cas, construction, values, &scalar, &unknown, budget)?;
    let input = original(cas, construction, polynomial, values, &unknown, budget)?;
    let product = identity(construction, [&input, &product], &values.map, budget)?;
    let set = set(cas, construction, problem, &entries, budget)?;
    let mut factors = vector(entries.len(), budget)?;
    factors.extend(entries.into_iter().map(|root| RootFactor {
        root,
        multiplicity: 1,
    }));
    construction.push(
        SolveSchema::ProductCompleteness,
        SolveStatement::Coverage(StatementCoverage {
            problem: problem.clone(),
            set: set.clone(),
            region: construction.region.clone(),
        }),
        SolveCertificate::ProductCompleteness(CertificateProductCompleteness {
            problem: problem.clone(),
            set: set.clone(),
            scalar,
            factors,
            product,
            existence: Vec::new(),
            region: construction.region.clone(),
        }),
        budget,
    )?;
    Ok(set)
}

/// Only sealed, discharged full coverage and presentation can become a solved answer.
pub(crate) fn finish(
    cas: &mut Cas,
    construction: &mut Construction,
    proposal: &Proposal<'_>,
    budget: &mut Budget,
    checker: &mut Budget,
) -> Result<SolveAnswer, Fault> {
    let (problem, polynomial, values, form) = (
        proposal.problem,
        proposal.polynomial,
        proposal.values,
        proposal.form,
    );
    let set = coverage(cas, construction, problem, polynomial, values, budget)?;
    seal(cas, construction, set, form, budget, checker)
}

/// Coverage and requested presentation are checked separately before publication.
pub(crate) fn seal(
    cas: &mut Cas,
    construction: &mut Construction,
    set: Handle,
    form: SolveForm,
    budget: &mut Budget,
    checker: &mut Budget,
) -> Result<SolveAnswer, Fault> {
    let coverage = cas.check_proof(&construction.context, &construction.steps, checker)?;
    construction.push(
        SolveSchema::RootSelectionRefinement,
        SolveStatement::Presentation(StatementPresentation {
            set: set.clone(),
            form,
            region: construction.region.clone(),
        }),
        SolveCertificate::RootSelectionRefinement(CertificateRootSelectionRefinement {
            mode: RefinementMode::Presentation,
            target_kind: RefinementTargetKind::SolutionSet,
            target: set.clone(),
            witnesses: Vec::new(),
            compatibility: Vec::new(),
            form: Some(form),
        }),
        budget,
    )?;
    let presentation = match cas.check_proof(&construction.context, &construction.steps, checker) {
        Ok(presentation) => presentation,
        Err(error)
            if matches!(
                error.kind,
                crate::error::FaultKind::Invalid(crate::error::InvalidCode::Rule)
            ) =>
        {
            return Ok(SolveAnswer {
                set,
                facts: Vec::new(),
                coverage: Some(coverage),
                form: SolveForm::AnyExact,
                unresolved_regions: vec![construction.region.clone()],
            });
        }
        Err(error) => return Err(error),
    };
    let unresolved_regions = if coverage.is_discharged() && presentation.is_discharged() {
        Vec::new()
    } else {
        vec![construction.region.clone()]
    };
    Ok(SolveAnswer {
        set,
        facts: vec![presentation],
        coverage: Some(coverage),
        form,
        unresolved_regions,
    })
}

/// A finite proposal retains the original problem and requested representation until replay.
pub(crate) struct Proposal<'a> {
    /// Exact original problem, including all original denominator obligations.
    pub(crate) problem: &'a Handle,
    /// Original polynomial coefficient vector.
    pub(crate) polynomial: &'a Dense,
    /// Computed roots in a shared scalar interpretation.
    pub(crate) values: &'a Values,
    /// Requested exact representation.
    pub(crate) form: SolveForm,
}
