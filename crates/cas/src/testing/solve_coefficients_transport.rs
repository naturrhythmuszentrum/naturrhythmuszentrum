// SPDX-License-Identifier: AGPL-3.0-only
//! Independently supplied quotient-specialization identities for public replay tests.

use super::Fixture;
use crate::test_api::proof::{ApplicationDraft, Certificate, Statement};
use crate::test_api::solving::{
    CoefficientMapData, MapMode, MapTarget, PolynomialData, PolynomialTerm, SolveCertificate,
    SolveDraft, SolveStatement,
};
use crate::test_api::syntax::{Draft, Field};
use crate::test_api::{Handle, Reference};
use crate::testing::common::TestResult;
use crate::testing::solve_graph::admit;

/// Each alteration preserves syntax admission while changing one required proof obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpecializationMutation {
    /// The exact source and target unity relation.
    Valid,
    /// One does not satisfy the third-root relation.
    Generator,
    /// A target third root does not satisfy the fourth-root relation.
    Modulus,
    /// A target coefficient differs by one.
    TargetCoefficient,
    /// The conclusion changes its exact regional claim.
    Region,
    /// An unused formal slot changes its scalar field.
    Frame,
    /// The certificate omits the map's finite localization.
    Denominator,
    /// An actual target denominator has no nonzero evidence.
    UndefinedTarget,
}

/// A literal fraction constant in the fixture's two-parameter base.
fn constant(fixture: &mut Fixture, integer: i64) -> TestResult<Handle> {
    let terms = if integer == 0 {
        Vec::new()
    } else {
        vec![(vec![0, 0], integer)]
    };
    let numerator = fixture.polynomial(2, &terms)?;
    let denominator = fixture.polynomial(2, &[(vec![0, 0], 1)])?;
    let draft = fixture.fraction(&numerator, &denominator);
    admit(&mut fixture.cas, &draft)
}

/// The variable frame retains an unused second slot, so normalization cannot hide its mutation.
fn polynomial(
    fixture: &mut Fixture,
    domain: &Handle,
    coefficient: &Handle,
    altered_frame: bool,
) -> TestResult<Handle> {
    admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::Polynomial(PolynomialData {
            domain: Reference::Existing(domain.clone()),
            variables: vec![
                Field::Real,
                if altered_frame {
                    Field::Complex
                } else {
                    Field::Real
                },
            ],
            terms: vec![PolynomialTerm {
                exponents: vec![2, 0],
                coefficient: Reference::Existing(coefficient.clone()),
            }],
        })),
    )
}

/// Independently encode Q[w]/(w²+w+1), with a wrong-modulus alternative for replay.
fn source(fixture: &mut Fixture, mutation: SpecializationMutation) -> TestResult<(Handle, Handle)> {
    let rational = fixture.rational.clone();
    let one = fixture.integer(1)?;
    let zero = fixture.integer(0)?;
    let entries = if mutation == SpecializationMutation::Modulus {
        vec![(0, one.clone()), (2, one.clone())]
    } else {
        vec![(0, one.clone()), (1, one.clone()), (2, one.clone())]
    };
    let modulus = fixture.univariate(&rational, &entries)?;
    let (_, domain) = fixture.tower(&rational, &modulus)?;
    let coefficient = fixture.tower_value(&domain, &[zero, one])?;
    Ok((domain, coefficient))
}

/// The same unity over Fraction(x,y), including an independently constructed changed coefficient.
fn target(
    fixture: &mut Fixture,
    mutation: SpecializationMutation,
) -> TestResult<(Handle, Handle, Handle)> {
    let fractions = fixture.fractions.clone();
    let one = constant(fixture, 1)?;
    let zero = constant(fixture, 0)?;
    let modulus = fixture.univariate(
        &fractions,
        &[(0, one.clone()), (1, one.clone()), (2, one.clone())],
    )?;
    let (_, domain) = fixture.tower(&fractions, &modulus)?;
    let unity = fixture.tower_value(&domain, &[zero, one.clone()])?;
    let coefficient = match mutation {
        SpecializationMutation::TargetCoefficient => {
            fixture.tower_value(&domain, &[one.clone(), one.clone()])?
        }
        SpecializationMutation::UndefinedTarget => {
            let numerator = fixture.polynomial(2, &[(vec![0, 0], 1)])?;
            let denominator = fixture.polynomial(2, &[(vec![1, 0], 1)])?;
            let draft = fixture.fraction(&numerator, &denominator);
            let reciprocal = admit(&mut fixture.cas, &draft)?;
            fixture.tower_value(&domain, &[reciprocal, one.clone()])?
        }
        _ => unity.clone(),
    };
    let image = if mutation == SpecializationMutation::Generator {
        fixture.tower_value(&domain, std::slice::from_ref(&one))?
    } else {
        unity
    };
    Ok((domain, coefficient, image))
}

/// The explicit quotient map retains a concrete, nonzero localization polynomial.
fn map(
    fixture: &mut Fixture,
    source: &Handle,
    target: &Handle,
    image: &Handle,
) -> TestResult<(Handle, Handle)> {
    let denominator = fixture.polynomial(0, &[(Vec::new(), 1)])?;
    let result = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
            source: Reference::Existing(source.clone()),
            target: MapTarget::CoefficientDomain {
                domain: Reference::Existing(target.clone()),
            },
            mode: MapMode::Specialization,
            region: Reference::Existing(fixture.region.clone()),
            images: vec![Reference::Existing(image.clone())],
            denominators: vec![Reference::Existing(denominator.clone())],
        })),
    )?;
    Ok((result, denominator))
}

/// Repeat the certificate's localization and optionally change its exact region in both payloads.
fn evidence(
    fixture: &mut Fixture,
    step: &mut ApplicationDraft,
    denominator: Handle,
    mutation: SpecializationMutation,
) -> TestResult {
    let Certificate::Solve(certificate) = &mut step.certificate else {
        return Err("missing identity certificate".into());
    };
    let SolveCertificate::CoefficientIdentity(identity) = certificate.as_mut() else {
        return Err("wrong identity certificate".into());
    };
    if mutation != SpecializationMutation::Denominator {
        identity.denominators.push(denominator);
    }
    if mutation == SpecializationMutation::Region {
        let x = fixture.free("x")?;
        let cells = fixture.guard_cells(&x)?;
        identity.region.clone_from(&cells[0]);
        let Statement::Solve(statement) = &mut step.conclusion else {
            return Err("missing coefficient statement".into());
        };
        let SolveStatement::CoefficientEquality(equality) = statement.as_mut() else {
            return Err("wrong coefficient statement".into());
        };
        equality.region.clone_from(&cells[0]);
    }
    Ok(())
}

/// Build only public syntax; the expected equality is unity times the same formal monomial.
pub(crate) fn specialization_identity(
    mutation: SpecializationMutation,
    use_polynomial: bool,
) -> TestResult<(Fixture, ApplicationDraft)> {
    let mut fixture = Fixture::new()?;
    let (source_domain, source_value) = source(&mut fixture, mutation)?;
    let (target_domain, target_value, image) = target(&mut fixture, mutation)?;
    let (mapping, denominator) = map(&mut fixture, &source_domain, &target_domain, &image)?;
    let (left, right) = if use_polynomial {
        (
            polynomial(&mut fixture, &source_domain, &source_value, false)?,
            polynomial(
                &mut fixture,
                &target_domain,
                &target_value,
                mutation == SpecializationMutation::Frame,
            )?,
        )
    } else {
        (source_value, target_value)
    };
    let mut step = fixture.identity(&left, &right, &mapping);
    evidence(&mut fixture, &mut step, denominator, mutation)?;
    Ok((fixture, step))
}
