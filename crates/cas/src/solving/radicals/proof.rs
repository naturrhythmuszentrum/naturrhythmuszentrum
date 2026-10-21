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
    // Regional replay independently matches the original equation and every factor to this shared input.
    let original_region = &crate::solving::elementary::proof::problem(cas, problem)?.region;
    let right = if *original_region == construction.region {
        &product
    } else {
        &input
    };
    let product = identity(construction, [&input, right], &values.map, budget)?;
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

/// Explicit representation obligations name their independently closed child presentations.
pub(crate) fn present(
    construction: &mut Construction,
    set: &Handle,
    form: SolveForm,
    compatibility: Vec<u32>,
    budget: &mut Budget,
) -> Result<(), Fault> {
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
            compatibility,
            form: Some(form),
        }),
        budget,
    )?;
    Ok(())
}

/// A checked answer survives a later resource failure while the first fault remains explicit.
pub(crate) struct Produced {
    /// Only independently replayed facts, with the achieved representation strength.
    pub(crate) answer: SolveAnswer,
    /// A subsequent failed obligation does not revoke earlier checked coverage.
    pub(crate) fault: Option<Fault>,
}

/// Retain independently checked coverage even when representation preparation or replay exhausts.
pub(crate) fn finish(
    cas: &mut Cas,
    construction: &Construction,
    set: Handle,
    form: SolveForm,
    coverage: crate::proof_model::VerifiedWitness,
    preparation: Result<Option<usize>, Fault>,
    checker: &mut Budget,
) -> Produced {
    let representation = preparation.and_then(|prefix| match prefix {
        Some(count) => super::continuation::extend(cas, construction, count, &coverage, checker),
        None => cas.check_proof(&construction.context, &construction.steps, checker),
    });
    let mut answer = SolveAnswer {
        set,
        facts: Vec::new(),
        coverage: Some(coverage),
        form: SolveForm::AnyExact,
        unresolved_regions: vec![construction.region.clone()],
    };
    match representation {
        Ok(presentation) => {
            if answer
                .coverage
                .as_ref()
                .is_some_and(crate::proof_model::VerifiedWitness::is_discharged)
                && presentation.is_discharged()
            {
                answer.unresolved_regions.clear();
            }
            answer.form = form;
            answer.facts.push(presentation);
            Produced {
                answer,
                fault: None,
            }
        }
        Err(error)
            if matches!(
                error.kind,
                crate::error::FaultKind::Invalid(crate::error::InvalidCode::Rule)
            ) =>
        {
            Produced {
                answer,
                fault: None,
            }
        }
        Err(error) => Produced {
            answer,
            fault: Some(error),
        },
    }
}
