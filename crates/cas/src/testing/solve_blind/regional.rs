// SPDX-License-Identifier: AGPL-3.0-only
//! Hand-authored regional unit identities, shared by the public conformance harness.

use super::Fixture;
use super::input::admit;
use crate::test_api::domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
use crate::test_api::proof::{ApplicationDraft, Certificate, Primitive, Statement};
use crate::test_api::solving::{
    CertificateExtensionUnit, CoefficientDomainData, CoefficientValueData, CoefficientValueObject,
    ExtensionLevelData, PolynomialData, PolynomialTerm, SolveCertificate, SolveDraft, SolveSchema,
    SolveView, SquareFreeRequest, StatementUnit,
};
use crate::test_api::syntax::{AtomicSort, Draft, Field};
use crate::test_api::{Handle, Rational, Reference};
use crate::testing::common::TestResult;

/// Keep both sibling cells available without changing the original Complex declaration.
fn on_cell(fixture: &mut Fixture, zero: bool) -> TestResult {
    let parameter = fixture.free("a")?;
    let origin = fixture.integer(0, false)?;
    let predicate = Reference::Existing(fixture.equation(&parameter, &origin)?);
    let base = Reference::Existing(fixture.region.clone());
    let mut expression = RegionExpr::GuardTrue {
        base: base.clone(),
        predicate: predicate.clone(),
    };
    if !zero {
        assert!(matches!(expression, RegionExpr::GuardTrue { .. }));
        let replacement = RegionExpr::GuardFalse { base, predicate };
        assert_ne!(expression, replacement);
        expression = replacement;
    }
    fixture.region = admit(
        &mut fixture.cas,
        &Draft::Region(RegionDraft {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Product {
                coordinates: fixture.coefficients.clone(),
            },
            expression,
        }),
    )?;
    fixture.context = admit(
        &mut fixture.cas,
        &Draft::Context(ContextDraft::Root {
            header: Reference::Existing(fixture.header.clone()),
            admissible: Reference::Existing(fixture.region.clone()),
            assumptions: Vec::new(),
        }),
    )?;
    Ok(())
}

/// Supply one formal polynomial directly from its independent coefficient inventory.
fn polynomial(
    fixture: &mut Fixture,
    domain: &Handle,
    terms: &[(u32, Handle)],
) -> TestResult<Handle> {
    admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::Polynomial(PolynomialData {
            domain: Reference::Existing(domain.clone()),
            variables: vec![Field::Complex],
            terms: terms
                .iter()
                .rev()
                .map(|(exponent, coefficient)| PolynomialTerm {
                    exponents: vec![*exponent],
                    coefficient: Reference::Existing(coefficient.clone()),
                })
                .collect(),
        })),
    )
}

/// A supplied coefficient is graph syntax, with no arithmetic or inverse computation in this fixture.
fn coefficient(
    fixture: &mut Fixture,
    domain: &Handle,
    value: CoefficientValueData<Reference>,
) -> TestResult<Handle> {
    admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
            domain: Reference::Existing(domain.clone()),
            value,
        })),
    )
}

/// The exact fractions a and 1 retain the original Complex coefficient frame.
fn parameters(fixture: &mut Fixture) -> TestResult<(Handle, Handle)> {
    let SolveView::CoefficientDomain(CoefficientDomainData::Fraction { base, .. }) =
        fixture.cas.solve_view(&fixture.algebra)?
    else {
        return Err("fraction domain".into());
    };
    let rational = base.clone();
    let integer = coefficient(
        fixture,
        &rational,
        CoefficientValueData::Rational {
            value: Rational::one(),
        },
    )?;
    let constant = polynomial(fixture, &rational, &[(0, integer.clone())])?;
    let variable = polynomial(fixture, &rational, &[(1, integer)])?;
    let domain = fixture.algebra.clone();
    let parameter = coefficient(
        fixture,
        &domain,
        CoefficientValueData::Fraction {
            numerator: Reference::Existing(variable),
            denominator: Reference::Existing(constant.clone()),
        },
    )?;
    let one = coefficient(
        fixture,
        &domain,
        CoefficientValueData::Fraction {
            numerator: Reference::Existing(constant.clone()),
            denominator: Reference::Existing(constant),
        },
    )?;
    Ok((parameter, one))
}

/// The monic relation T²+1 supplies a genuine earlier tower parent, without a root assumption.
fn quotient(fixture: &mut Fixture, modulus: &Handle) -> TestResult<Handle> {
    let level = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::ExtensionLevel(ExtensionLevelData {
            parent: Reference::Existing(fixture.algebra.clone()),
            polynomial: Reference::Existing(modulus.clone()),
            generator: vec![AtomicSort::Scalar(Field::Complex)],
            square_free: SquareFreeRequest::Required,
        })),
    )?;
    admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Quotient {
                level: Reference::Existing(level),
            },
        )),
    )
}

/// On a=0, (1+aT)*1=1 and gcd(1+aT,T²+1)=1, with explicit independent Bezout data.
pub(crate) fn unit(zero: bool) -> TestResult<(Fixture, ApplicationDraft)> {
    let (mut fixture, _) = super::symbolic(&["a"])?;
    on_cell(&mut fixture, zero)?;
    let (parameter, one) = parameters(&mut fixture)?;
    let parent = fixture.algebra.clone();
    let modulus = polynomial(&mut fixture, &parent, &[(0, one.clone()), (2, one.clone())])?;
    let domain = quotient(&mut fixture, &modulus)?;
    let element = coefficient(
        &mut fixture,
        &domain,
        CoefficientValueData::Tower {
            coefficients: vec![
                Reference::Existing(one.clone()),
                Reference::Existing(parameter.clone()),
            ],
        },
    )?;
    let inverse = coefficient(
        &mut fixture,
        &domain,
        CoefficientValueData::Tower {
            coefficients: vec![Reference::Existing(one.clone())],
        },
    )?;
    let divisor = polynomial(&mut fixture, &parent, &[(0, one.clone())])?;
    let dividend = polynomial(&mut fixture, &parent, &[(0, one), (1, parameter)])?;
    let empty = polynomial(&mut fixture, &parent, &[])?;
    let step = ApplicationDraft {
        rule: Primitive::Solve(SolveSchema::ExtensionUnit),
        context: fixture.context.clone(),
        conclusion: Statement::Solve(Box::new(crate::test_api::solving::SolveStatement::Unit(
            StatementUnit {
                value: element.clone(),
                inverse,
                domain,
                region: fixture.region.clone(),
            },
        ))),
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Solve(Box::new(SolveCertificate::ExtensionUnit(
            CertificateExtensionUnit {
                element,
                modulus: modulus.clone(),
                gcd: divisor.clone(),
                quotients: vec![dividend, modulus],
                bezout: vec![divisor, empty],
                region: fixture.region.clone(),
            },
        ))),
    };
    Ok((fixture, step))
}

/// The independently supplied cell is a²-1=0, or its explicitly selected nonzero sibling.
fn root_cell(fixture: &mut Fixture, zero: bool) -> TestResult {
    use crate::test_api::syntax::Operator;
    let parameter = fixture.free("a")?;
    let square = fixture.expression(Operator::IntegerPower(2), &[parameter], false)?;
    let one = fixture.integer(1, false)?;
    let difference = fixture.expression(Operator::Sub, &[square, one], false)?;
    let origin = fixture.integer(0, false)?;
    let predicate = Reference::Existing(fixture.equation(&difference, &origin)?);
    let base = Reference::Existing(fixture.region.clone());
    let expression = if zero {
        RegionExpr::GuardTrue { base, predicate }
    } else {
        RegionExpr::GuardFalse { base, predicate }
    };
    fixture.region = admit(
        &mut fixture.cas,
        &Draft::Region(RegionDraft {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Product {
                coordinates: fixture.coefficients.clone(),
            },
            expression,
        }),
    )?;
    fixture.context = admit(
        &mut fixture.cas,
        &Draft::Context(ContextDraft::Root {
            header: Reference::Existing(fixture.header.clone()),
            admissible: Reference::Existing(fixture.region.clone()),
            assumptions: Vec::new(),
        }),
    )?;
    Ok(())
}

/// Hand-authored total coefficient a²-1 has no arithmetic normalization dependency.
fn root_relation(fixture: &mut Fixture) -> TestResult<Handle> {
    let SolveView::CoefficientDomain(CoefficientDomainData::Fraction { base, .. }) =
        fixture.cas.solve_view(&fixture.algebra)?
    else {
        return Err("fraction domain".into());
    };
    let base = base.clone();
    let one = coefficient(
        fixture,
        &base,
        CoefficientValueData::Rational {
            value: Rational::one(),
        },
    )?;
    let minus = Rational::from_ratio(
        -1,
        1,
        &mut crate::test_api::limits::Budget::new(crate::test_api::limits::Limits::blind_cubic())?,
    )?;
    let negative = coefficient(
        fixture,
        &base,
        CoefficientValueData::Rational { value: minus },
    )?;
    let numerator = polynomial(fixture, &base, &[(0, negative), (2, one.clone())])?;
    let denominator = polynomial(fixture, &base, &[(0, one)])?;
    let source = fixture.algebra.clone();
    coefficient(
        fixture,
        &source,
        CoefficientValueData::Fraction {
            numerator: Reference::Existing(numerator),
            denominator: Reference::Existing(denominator),
        },
    )
}

/// The nonzero conclusion names precisely the supplied coefficient and its original interpretation.
fn nonzero_statement(fixture: &mut Fixture, value: &Handle, map: &Handle) -> TestResult<Statement> {
    let literal = admit(
        &mut fixture.cas,
        &Draft::ExactCoefficient {
            header: Reference::Existing(fixture.header.clone()),
            value: Reference::Existing(value.clone()),
            map: Reference::Existing(map.clone()),
        },
    )?;
    let predicate = admit(
        &mut fixture.cas,
        &Draft::Condition {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            kind: crate::test_api::syntax::PredicateKind::NonZero,
            arguments: vec![Reference::Existing(literal)],
        },
    )?;
    Ok(Statement::PredicateFact {
        predicate,
        region: fixture.region.clone(),
    })
}

/// Supply a²-1=1*(a²-1), with nonzero product 1, before any exponent or branch search.
pub(crate) fn consequence(zero: bool) -> TestResult<(Fixture, ApplicationDraft)> {
    use crate::test_api::solving::{
        CertificateCoefficientIdentity, CoefficientMapData, IdentityMode, MapMode, MapTarget,
        RelationMultiple,
    };
    let (mut fixture, _) = super::symbolic(&["a"])?;
    root_cell(&mut fixture, zero)?;
    let (parameter, one) = parameters(&mut fixture)?;
    let relation = root_relation(&mut fixture)?;
    let image = fixture.free("a")?;
    let map = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
            source: Reference::Existing(fixture.algebra.clone()),
            target: MapTarget::ScalarAmbient {
                header: Reference::Existing(fixture.header.clone()),
                field: Field::Complex,
            },
            mode: MapMode::Specialization,
            region: Reference::Existing(fixture.region.clone()),
            images: vec![Reference::Existing(image)],
            denominators: Vec::new(),
        })),
    )?;
    let conclusion = nonzero_statement(&mut fixture, &parameter, &map)?;
    let certificate = CertificateCoefficientIdentity {
        mode: IdentityMode::NonzeroPower(2),
        left: parameter,
        right: one.clone(),
        map,
        region: fixture.region.clone(),
        combinations: vec![RelationMultiple {
            multiplier: one.clone(),
            relation,
            hypothesis: None,
        }],
        denominators: vec![one],
    };
    let step = ApplicationDraft {
        rule: Primitive::Solve(SolveSchema::CoefficientIdentity),
        context: fixture.context.clone(),
        conclusion,
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Solve(Box::new(SolveCertificate::CoefficientIdentity(
            certificate,
        ))),
    };
    Ok((fixture, step))
}
