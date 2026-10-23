// SPDX-License-Identifier: AGPL-3.0-only
//! Original sign facts, projection evidence and conjugation survive independent replay.

use super::{request, stratum};
use crate::limits::{Budget, Limits};
use crate::proof::{ApplicationDraft, Certificate, Statement, VerifiedWitness};
use crate::request::{Outcome, Request, Task};
use crate::solving::{
    SolutionBranchKind, SolveCertificate, SolveDraft, SolveSchema, SolveStatement,
};
use crate::testing::common::TestResult;
use crate::testing::sorts::{Fixture, admit};
use crate::{Cas, Handle};

/// Solve the original equation on one unspecialized Real stratum.
fn solved(sign: i8) -> TestResult<(Fixture, Request, VerifiedWitness)> {
    let (mut fixture, mut request) = request(2)?;
    stratum(&mut fixture, &mut request, sign)?;
    let outcome = fixture.cas.run(&request);
    let Outcome::Solved { answer, .. } = outcome else {
        return Err(format!("family piece {outcome:?}").into());
    };
    let proof = answer.coverage.ok_or("missing family coverage")?;
    Ok((fixture, request, proof))
}

/// Decode returned tuple values directly from the original complete-set theorem.
fn entries(cas: &Cas, proof: &VerifiedWitness) -> TestResult<Vec<Handle>> {
    let Statement::Solve(statement) = &proof.judgment().statement else {
        return Err("coverage statement".into());
    };
    let SolveStatement::Coverage(value) = statement.as_ref() else {
        return Err("coverage claim".into());
    };
    let SolveDraft::SolutionSet(set) =
        crate::solving::graph::object(&cas.state, value.set.logical_id())?
    else {
        return Err("solution set".into());
    };
    let mut values = Vec::new();
    for branch in &set.branches {
        let SolveDraft::SolutionBranch(branch) =
            crate::solving::graph::object(&cas.state, branch.logical_id())?
        else {
            return Err("solution branch".into());
        };
        let SolutionBranchKind::Finite { tuples } = &branch.kind else {
            return Err("finite piece".into());
        };
        for tuple in tuples {
            let SolveDraft::Tuple(tuple) =
                crate::solving::graph::object(&cas.state, tuple.logical_id())?
            else {
                return Err("tuple".into());
            };
            values.extend(tuple.entries.iter().cloned());
        }
    }
    Ok(values)
}

/// Both actual positive-stratum roots are fixed by the existing standard conjugation theorem.
/// ´test:crate:mixed-positive-roots-conjugation-fixed´
#[test]
fn mixed_positive_roots_conjugation_fixed() -> TestResult {
    let (mut fixture, request, proof) = solved(1)?;
    let values = entries(&fixture.cas, &proof)?;
    assert_eq!(values.len(), 2);
    let region = fixture.cas.handle(
        fixture
            .cas
            .state
            .context(request.context.logical_id())?
            .admissible,
    );
    let mut construction = crate::solving::radicals::construction::Construction {
        header: fixture.header.clone(),
        context: request.context.clone(),
        region: region.clone(),
        steps: crate::testing::solve_blind::family_applications(&fixture.cas, &proof)?,
    };
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let ground = admit(
        &mut fixture.cas,
        crate::syntax::Draft::Solve(SolveDraft::CoefficientDomain(
            crate::solving::CoefficientDomainData::Rational,
        )),
    )?;
    let map = construction.mapping(&mut fixture.cas, &ground, &[], &mut budget)?;
    construction.push(
        SolveSchema::ConjugationTransport,
        SolveStatement::RealValues(crate::solving::StatementRealValues {
            values: values.clone(),
            region: region.clone(),
        }),
        SolveCertificate::ConjugationTransport(crate::solving::CertificateConjugationTransport {
            map: map.clone(),
            conjugate_map: map,
            images: Vec::new(),
            values,
            relations: Vec::new(),
            fixed_field: Vec::new(),
            region,
        }),
        &mut budget,
    )?;
    let witness = fixture.cas.check_proof(
        &request.context,
        &construction.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let bytes = fixture
        .cas
        .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
    assert!(
        Cas::new(Limits::blind_cubic())?
            .inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?
            .is_discharged()
    );
    Ok(())
}

/// A changed application must fail without damaging the original independently checked witness.
fn rejected(
    fixture: &mut Fixture,
    request: &Request,
    original: &VerifiedWitness,
    changed: &[ApplicationDraft],
) -> TestResult {
    assert!(
        fixture
            .cas
            .check_proof(
                &request.context,
                changed,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    let bytes = fixture
        .cas
        .export_witness(original, &mut Budget::new(Limits::blind_cubic())?)?;
    assert!(
        Cas::new(Limits::blind_cubic())?
            .inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?
            .is_discharged()
    );
    Ok(())
}

/// A pulled-back Real root cannot be used after removing its explicit projection premise.
/// ´test:crate:mixed-root-pullback-requires-subset´
#[test]
fn mixed_root_pullback_requires_subset() -> TestResult {
    let (mut fixture, request, proof) = solved(-1)?;
    let mut changed = crate::testing::solve_blind::family_applications(&fixture.cas, &proof)?;
    let step = changed.iter_mut().find(|step| step.rule == crate::rule::Primitive::ContextWeaken
        && matches!(&step.conclusion, Statement::Solve(value) if matches!(value.as_ref(), SolveStatement::RootExistence(_)))).ok_or("root pullback")?;
    assert_eq!(step.premises.len(), 2);
    let original = step.premises.clone();
    step.premises.remove(1);
    assert_ne!(step.premises, original);
    rejected(&mut fixture, &request, &proof, &changed)
}

/// A positive sibling sign cannot inhabit the original negative base stratum.
/// ´test:crate:mixed-negative-piece-rejects-sibling-sign´
#[test]
fn mixed_negative_piece_rejects_sibling_sign() -> TestResult {
    let (mut fixture, request, proof) = solved(-1)?;
    let mut changed = crate::testing::solve_blind::family_applications(&fixture.cas, &proof)?;
    let sign = changed
        .iter_mut()
        .find_map(|step| {
            let Certificate::Solve(certificate) = &mut step.certificate else {
                return None;
            };
            let SolveCertificate::RealSign(value) = certificate.as_mut() else {
                return None;
            };
            value.steps.iter_mut().find(|entry| {
                entry.rule == crate::solving::RealSignRule::Hypothesis
                    && entry.conclusion == crate::solving::RealSignConclusion::Positive
                    && entry.premises.is_empty()
            })
        })
        .ok_or("original positive orientation")?;
    let original = sign.subject.clone();
    assert_eq!(
        fixture
            .cas
            .state
            .expression(original.logical_id())?
            .operator,
        crate::model::Operator::Neg
    );
    assert_eq!(
        fixture
            .cas
            .state
            .expression(original.logical_id())?
            .operands,
        [fixture.real.logical_id()]
    );
    sign.subject.clone_from(&fixture.real);
    assert_ne!(sign.subject, original);
    rejected(&mut fixture, &request, &proof, &changed)
}

/// A family parameter map cannot replace the original Real declaration by a sampled point.
/// ´test:crate:mixed-family-map-rejects-sampled-base´
#[test]
fn mixed_family_map_rejects_sampled_base() -> TestResult {
    let (mut fixture, request, _) = solved(1)?;
    let original = fixture.real.clone();
    let replacement = crate::testing::sorts::expression(
        &mut fixture.cas,
        &fixture.header,
        crate::syntax::Field::Real,
        crate::syntax::Operator::Rational(crate::Rational::one()),
        Vec::new(),
        Vec::new(),
    )?;
    assert_eq!(
        fixture
            .cas
            .state
            .expression(original.logical_id())?
            .operator,
        crate::model::Operator::Free("r".into())
    );
    assert_ne!(replacement, original);
    let Task::Solve(specification) = &request.task else {
        return Err("solve request".into());
    };
    let original_problem =
        crate::solving::graph::problem(&fixture.cas.state, specification.problem.logical_id())?
            .clone();
    let region = fixture.cas.handle(
        fixture
            .cas
            .state
            .context(request.context.logical_id())?
            .admissible,
    );
    let family = fixture.cas.handle(
        fixture
            .cas
            .state
            .region(region.logical_id())?
            .lies_over
            .as_ref()
            .ok_or("family")?
            .family,
    );
    let mut map = crate::solving::CoefficientMapData {
        source: original_problem.coefficient_domain,
        target: crate::solving::MapTarget::Family { family },
        mode: crate::solving::MapMode::Specialization,
        region,
        images: vec![original.clone(), fixture.complex.clone()],
        denominators: Vec::new(),
    };
    crate::solving::family::mapping::parameters(
        &fixture.cas,
        &map,
        &original_problem.coefficients,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(map.images[0], original);
    map.images[0] = replacement;
    assert_ne!(map.images[0], original);
    assert!(
        crate::solving::family::mapping::parameters(
            &fixture.cas,
            &map,
            &original_problem.coefficients,
            &mut Budget::new(Limits::blind_cubic())?
        )
        .is_err()
    );
    Ok(())
}

/// Piece completeness cannot be widened to the original whole-family region.
/// ´test:crate:mixed-piece-rejects-whole-region-widening´
#[test]
fn mixed_piece_rejects_whole_region_widening() -> TestResult {
    let (mut fixture, request, proof) = solved(-1)?;
    let Task::Solve(specification) = &request.task else {
        return Err("solve request".into());
    };
    let whole =
        crate::solving::graph::problem(&fixture.cas.state, specification.problem.logical_id())?
            .region
            .clone();
    let mut changed = crate::testing::solve_blind::family_applications(&fixture.cas, &proof)?;
    let step = changed.last_mut().ok_or("coverage application")?;
    let Statement::Solve(statement) = &mut step.conclusion else {
        return Err("coverage statement".into());
    };
    let SolveStatement::Coverage(value) = statement.as_mut() else {
        return Err("coverage claim".into());
    };
    let original = value.region.clone();
    assert_eq!(
        original.logical_id(),
        fixture
            .cas
            .state
            .context(request.context.logical_id())?
            .admissible
    );
    assert_ne!(whole, original);
    value.region = whole;
    assert_ne!(value.region, original);
    rejected(&mut fixture, &request, &proof, &changed)
}

/// Construct the canonical mixed map with each original image in its own declaration sort.
fn family_map(
    fixture: &Fixture,
    request: &Request,
) -> TestResult<crate::solving::CoefficientMapData> {
    let Task::Solve(specification) = &request.task else {
        return Err("solve request".into());
    };
    let original =
        crate::solving::graph::problem(&fixture.cas.state, specification.problem.logical_id())?;
    let region = fixture.cas.handle(
        fixture
            .cas
            .state
            .context(request.context.logical_id())?
            .admissible,
    );
    let family = fixture.cas.handle(
        fixture
            .cas
            .state
            .region(region.logical_id())?
            .lies_over
            .as_ref()
            .ok_or("family")?
            .family,
    );
    Ok(crate::solving::CoefficientMapData {
        source: crate::Reference::Existing(original.coefficient_domain.clone()),
        target: crate::solving::MapTarget::Family {
            family: crate::Reference::Existing(family),
        },
        mode: crate::solving::MapMode::Specialization,
        region: crate::Reference::Existing(region),
        images: vec![
            crate::Reference::Existing(fixture.real.clone()),
            crate::Reference::Existing(fixture.complex.clone()),
        ],
        denominators: Vec::new(),
    })
}

/// A new family map never licenses putting its Real image into an old Complex scalar slot.
/// ´test:crate:mixed-map-rejects-scalar-complex-coercion´
#[test]
fn mixed_map_rejects_scalar_complex_coercion() -> TestResult {
    let (mut fixture, request) = request(1)?;
    let mut map = family_map(&fixture, &request)?;
    admit(
        &mut fixture.cas,
        crate::syntax::Draft::Solve(SolveDraft::CoefficientMap(map.clone())),
    )?;
    let original = map.target.clone();
    assert!(matches!(original, crate::solving::MapTarget::Family { .. }));
    assert_eq!(
        map.images[0],
        crate::Reference::Existing(fixture.real.clone())
    );
    map.target = crate::solving::MapTarget::ScalarAmbient {
        header: crate::Reference::Existing(fixture.header.clone()),
        field: crate::syntax::Field::Complex,
    };
    assert_ne!(map.target, original);
    assert!(
        admit(
            &mut fixture.cas,
            crate::syntax::Draft::Solve(SolveDraft::CoefficientMap(map))
        )
        .is_err()
    );
    Ok(())
}

/// Changing only F cannot reinterpret the original total region and its source declarations.
/// ´test:crate:mixed-map-rejects-changed-family´
#[test]
fn mixed_map_rejects_changed_family() -> TestResult {
    let (mut fixture, request) = request(1)?;
    let mut map = family_map(&fixture, &request)?;
    admit(
        &mut fixture.cas,
        crate::syntax::Draft::Solve(SolveDraft::CoefficientMap(map.clone())),
    )?;
    let crate::solving::MapTarget::Family {
        family: crate::Reference::Existing(original),
    } = &map.target
    else {
        return Err("family target".into());
    };
    let SolveDraft::SpaceFamily(shape) =
        crate::solving::graph::object(&fixture.cas.state, original.logical_id())?
    else {
        return Err("family shape".into());
    };
    let shape = shape.clone();
    let unknowns = fixture.coordinates(&[])?;
    assert_ne!(unknowns, shape.unknowns);
    let alternate = admit(
        &mut fixture.cas,
        crate::syntax::Draft::Solve(SolveDraft::SpaceFamily(crate::solving::SpaceFamilyData {
            base: crate::Reference::Existing(shape.base),
            fibre_space: crate::Reference::Existing(shape.fibre_space),
            coefficients: crate::Reference::Existing(shape.coefficients),
            unknowns: crate::Reference::Existing(unknowns),
        })),
    )?;
    let previous = map.target.clone();
    assert_eq!(
        previous,
        crate::solving::MapTarget::Family {
            family: crate::Reference::Existing(original.clone())
        }
    );
    map.target = crate::solving::MapTarget::Family {
        family: crate::Reference::Existing(alternate),
    };
    assert_ne!(map.target, previous);
    assert!(
        admit(
            &mut fixture.cas,
            crate::syntax::Draft::Solve(SolveDraft::CoefficientMap(map))
        )
        .is_err()
    );
    Ok(())
}

/// The independent full exclusion search agrees with every original success and refusal in this fixture family.
/// ´test:crate:mixed-properties-oracle-agreement´
#[test]
fn mixed_properties_oracle_agreement() -> TestResult {
    crate::testing::exclusion_oracle::run(|| {
        mixed_positive_roots_conjugation_fixed()?;
        mixed_root_pullback_requires_subset()?;
        mixed_negative_piece_rejects_sibling_sign()?;
        mixed_family_map_rejects_sampled_base()?;
        mixed_piece_rejects_whole_region_widening()?;
        mixed_map_rejects_scalar_complex_coercion()?;
        mixed_map_rejects_changed_family()?;
        Ok(())
    })
}
