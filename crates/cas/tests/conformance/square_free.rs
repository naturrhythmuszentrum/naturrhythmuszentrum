// SPDX-License-Identifier: AGPL-3.0-only
//! Public-contract checks for square-free.

use nrz_cas::{
    AtomicSort, Budget, BudgetKind, Cas, Constant, Declaration, Draft, Environment, FaultKind,
    Field, Limits, LimitsOverride, Operator, Outcome, Primitive, Rational, Relation, Sort,
    UnsupportedReason,
};

use crate::testing::budgets::derivation_budget as budget;
use crate::testing::common::TestResult;
use crate::testing::square_free::{Fixture, proposal, rationals};

/// Nonzero constants have no multiplicity layers; zero has no finite such decomposition.
/// ´test:integration:square-free-constants-and-zero´
#[test]
fn square_free_constants_and_zero() -> TestResult {
    for text in ["1", "-7", "3/5"] {
        let mut fixture = Fixture::new(Field::Rational)?;
        let input = fixture.number(text)?;
        let result = fixture.decompose(&input)?;
        let certificate = fixture.cas.square_free_certificate(&result.witness)?;
        assert_eq!(certificate.scalar, Rational::parse(text, &mut budget()?)?);
        assert_eq!(certificate.layers, [] as [nrz_cas::SquareFreeLayer; 0]);
        assert_eq!(certificate.rounds, [] as [nrz_cas::GcdCertificate; 0]);
    }
    let mut fixture = Fixture::new(Field::Real)?;
    let zero = fixture.number("0")?;
    let request = fixture.request(&zero)?;
    assert!(matches!(
        fixture.cas.run(&request),
        Outcome::Unsupported {
            reason: UnsupportedReason::ZeroPolynomialMultiplicity,
            ..
        }
    ));
    Ok(())
}

/// A rational square-free factor need not split into linear or irreducible factors.
/// ´test:integration:square-free-unsplit-quadratic´
#[test]
fn square_free_unsplit_quadratic() -> TestResult {
    let mut fixture = Fixture::new(Field::Real)?;
    let input = fixture.polynomial(&[1, 0, 1])?;
    let result = fixture.decompose(&input)?;
    let certificate = fixture.cas.square_free_certificate(&result.witness)?;
    assert_eq!(certificate.layers.len(), 1);
    let layer = certificate.layers.first().ok_or("quadratic layer")?;
    assert_eq!(layer.multiplicity, 1);
    assert_eq!(layer.coefficients, rationals(&[1, 0, 1])?);
    Ok(())
}

/// Independent binomial coefficients test a repeated single factor and negative leading scalar.
/// ´test:integration:square-free-single-repeated-factor´
#[test]
fn square_free_single_repeated_factor() -> TestResult {
    let mut fixture = Fixture::new(Field::Rational)?;
    let input = fixture.polynomial(&[-162, 216, -108, 24, -2])?;
    let result = fixture.decompose(&input)?;
    let certificate = fixture.cas.square_free_certificate(&result.witness)?;
    assert_eq!(
        certificate.scalar,
        Rational::from_ratio(-2, 1, &mut budget()?)?
    );
    assert_eq!(certificate.layers.len(), 1);
    let layer = certificate.layers.first().ok_or("repeated factor")?;
    assert_eq!(layer.multiplicity, 4);
    assert_eq!(layer.coefficients, rationals(&[-3, 1])?);
    Ok(())
}

/// Scalar division keeps rational coefficients exact, including a nonintegral monic factor.
/// ´test:integration:square-free-rational-coefficients´
#[test]
fn square_free_rational_coefficients() -> TestResult {
    let mut fixture = Fixture::new(Field::Rational)?;
    let coordinate = fixture.term(Operator::Free("x".into()), &[])?;
    let half = fixture.number("1/2")?;
    let linear = fixture.term(Operator::Sub, &[coordinate, half])?;
    let square = fixture.term(Operator::IntegerPower(2), &[linear])?;
    let third = fixture.number("3")?;
    let input = fixture.term(Operator::Div, &[square, third])?;
    let result = fixture.decompose(&input)?;
    let certificate = fixture.cas.square_free_certificate(&result.witness)?;
    assert_eq!(certificate.scalar, Rational::parse("1/3", &mut budget()?)?);
    let layer = certificate.layers.first().ok_or("rational layer")?;
    assert_eq!(layer.multiplicity, 2);
    assert_eq!(
        layer.coefficients,
        vec![Rational::parse("-1/2", &mut budget()?)?, Rational::one()]
    );
    Ok(())
}

/// Different grammar/field exclusions are explicit and never certify a multiplicity theorem.
/// ´test:integration:square-free-fragment-refusals´
#[test]
fn square_free_fragment_refusals() -> TestResult {
    let mut fixture = Fixture::new(Field::Real)?;
    let coordinate = fixture.term(Operator::Free("x".into()), &[])?;
    let other = fixture.term(Operator::Free("y".into()), &[])?;
    let multivariate = fixture.term(Operator::Add, &[coordinate.clone(), other])?;
    let request = fixture.request(&multivariate)?;
    assert!(matches!(
        fixture.cas.run(&request),
        Outcome::Unsupported {
            reason: UnsupportedReason::PolynomialFragment,
            ..
        }
    ));
    let named = fixture.term(Operator::Constant(Constant::Pi), &[])?;
    let symbolic = fixture.term(Operator::Mul, &[named, coordinate.clone()])?;
    let request = fixture.request(&symbolic)?;
    assert!(matches!(
        fixture.cas.run(&request),
        Outcome::Unsupported {
            reason: UnsupportedReason::CoefficientField,
            ..
        }
    ));
    let inverse = fixture.term(Operator::IntegerPower(-1), &[coordinate])?;
    let request = fixture.request(&inverse)?;
    assert!(matches!(
        fixture.cas.run(&request),
        Outcome::Unsupported {
            reason: UnsupportedReason::PolynomialFragment,
            ..
        }
    ));
    for field in [Field::Complex, Field::PositiveCharacteristic(5)] {
        let mut fixture = Fixture::new(field)?;
        let input = fixture.term(Operator::Free("x".into()), &[])?;
        let request = fixture.request(&input)?;
        assert!(matches!(
            fixture.cas.run(&request),
            Outcome::Unsupported {
                reason: UnsupportedReason::Field,
                ..
            }
        ));
    }
    Ok(())
}

/// Independently selected false certificate fields must all refuse without publishing a prefix.
/// ´test:integration:square-free-certificate-tampering´
#[test]
fn square_free_certificate_tampering() -> TestResult {
    let mut fixture = Fixture::new(Field::Rational)?;
    let input = fixture.polynomial(&[-4, 8, -1, -5, 1, 1])?;
    let result = fixture.decompose(&input)?;
    let original = fixture
        .cas
        .square_free_certificate(&result.witness)?
        .clone();
    for mutation in 0..10 {
        let mut certificate = original.clone();
        match mutation {
            0 => certificate.scalar = Rational::zero(),
            1 => {
                certificate
                    .layers
                    .first_mut()
                    .ok_or("first layer")?
                    .multiplicity = 1;
            }
            2 => {
                let _removed = certificate.layers.pop();
            }
            3 => certificate.initial_gcd.bezout_left.clear(),
            4 => certificate.initial_gcd.gcd = rationals(&[1])?,
            5 => {
                let layer = certificate.layers.first_mut().ok_or("first layer")?;
                layer.coefficients = rationals(&[4, 2])?;
            }
            6 => certificate.coprime.clear(),
            7 => certificate.initial_gcd.left_quotient.push(Rational::zero()),
            8 => certificate
                .square_free
                .first_mut()
                .ok_or("square-free gcd")?
                .bezout_right
                .clear(),
            _ => {
                let _removed = certificate.rounds.pop();
            }
        }
        let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
        assert!(
            fixture
                .cas
                .check_proof(
                    &fixture.context,
                    &[proposal(&result, certificate)],
                    &mut budget()?
                )
                .is_err()
        );
        assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    }
    Ok(())
}

/// Genuine coefficients cannot authenticate a different endpoint, field or variable judgment.
/// ´test:integration:square-free-claim-tampering´
#[test]
fn square_free_claim_tampering() -> TestResult {
    let mut fixture = Fixture::new(Field::Rational)?;
    let input = fixture.polynomial(&[1, 0, 1])?;
    let result = fixture.decompose(&input)?;
    let certificate = fixture
        .cas
        .square_free_certificate(&result.witness)?
        .clone();
    let different = fixture.number("1")?;
    for mutation in 0..4 {
        let mut step = proposal(&result, certificate.clone());
        match mutation {
            0 => step.right = different.clone(),
            1 => {
                step.relation = Relation::SquareFree {
                    variable: "y".into(),
                    field: Field::Rational,
                }
            }
            2 => {
                step.relation = Relation::SquareFree {
                    variable: "x".into(),
                    field: Field::Real,
                }
            }
            _ => step.primitive = Primitive::Reflexivity,
        }
        let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
        assert!(
            fixture
                .cas
                .check_proof(&fixture.context, &[step], &mut budget()?)
                .is_err()
        );
        assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    }
    Ok(())
}

/// Degree, coefficient size and arithmetic fuel refuse as resources, not false mathematics.
/// ´test:integration:square-free-resource-refusals´
#[test]
fn square_free_resource_refusals() -> TestResult {
    for (kind, limits) in [
        (
            BudgetKind::PolynomialDegree,
            LimitsOverride {
                polynomial_degree: Some(4),
                ..LimitsOverride::default()
            },
        ),
        (
            BudgetKind::ComponentBits,
            LimitsOverride {
                component_bits: Some(3),
                ..LimitsOverride::default()
            },
        ),
        (
            BudgetKind::ArithmeticSteps,
            LimitsOverride {
                arithmetic_steps: Some(0),
                ..LimitsOverride::default()
            },
        ),
    ] {
        let mut fixture = Fixture::new(Field::Rational)?;
        let input = fixture.polynomial(&[-4, 8, -1, -5, 1, 1])?;
        let mut request = fixture.request(&input)?;
        request.limits.polynomial_degree = limits.polynomial_degree;
        request.limits.component_bits = limits.component_bits;
        request.limits.arithmetic_steps = limits.arithmetic_steps;
        let outcome = fixture.cas.run(&request);
        assert!(matches!(outcome, Outcome::Exhausted {
            fault: nrz_cas::Fault { kind: FaultKind::Exhausted { budget, .. }, .. }, best: None, ..
        } if budget == kind));
    }
    Ok(())
}

/// Repeated builds produce identical witnesses; exact witness size N passes and N-1 refuses.
/// ´test:integration:square-free-portable-boundaries´
#[test]
fn square_free_portable_boundaries() -> TestResult {
    let mut outputs = Vec::new();
    for _ in 0..2 {
        let mut fixture = Fixture::new(Field::Rational)?;
        let input = fixture.polynomial(&[-4, 8, -1, -5, 1, 1])?;
        let result = fixture.decompose(&input)?;
        outputs.push(
            fixture
                .cas
                .export_witness(&result.witness, &mut budget()?)?,
        );
    }
    let bytes = outputs.first().ok_or("missing export")?;
    assert_eq!(Some(bytes), outputs.get(1));
    let length = u64::try_from(bytes.len())?;
    let mut receiver = Cas::new(Limits::default())?;
    let mut exact = Budget::new(Limits {
        witness_bytes: length,
        work: 2_000_000_000,
        replay_work: 2_000_000_000,
        ..Limits::default()
    })?;
    let witness = receiver.inspect(bytes, &mut exact)?;
    assert_eq!(receiver.export_witness(&witness, &mut budget()?)?, *bytes);
    let mut short = Budget::new(Limits {
        witness_bytes: length.checked_sub(1).ok_or("empty proof")?,
        work: 2_000_000_000,
        ..Limits::default()
    })?;
    let before = receiver.canonical_snapshot(&mut budget()?)?;
    assert!(receiver.inspect(bytes, &mut short).is_err());
    let mut truncated = bytes.clone();
    let _removed = truncated.pop();
    assert!(receiver.inspect(&truncated, &mut budget()?).is_err());
    assert_eq!(receiver.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// Several shifted powers use independent binomial coefficient lists as their input oracle.
/// ´test:integration:square-free-small-binomial-family´
#[test]
fn square_free_small_binomial_family() -> TestResult {
    for (coefficients, shift, multiplicity) in [
        (vec![2, 1], 2, 1),
        (vec![4, 4, 1], 2, 2),
        (vec![8, 12, 6, 1], 2, 3),
        (vec![-2, 1], -2, 1),
        (vec![4, -4, 1], -2, 2),
        (vec![-8, 12, -6, 1], -2, 3),
        (vec![0, 0, 0, 1], 0, 3),
    ] {
        let mut fixture = Fixture::new(Field::Rational)?;
        let input = fixture.polynomial(&coefficients)?;
        let result = fixture.decompose(&input)?;
        let certificate = fixture.cas.square_free_certificate(&result.witness)?;
        assert_eq!(certificate.layers.len(), 1);
        let layer = certificate.layers.first().ok_or("binomial layer")?;
        assert_eq!(layer.coefficients, rationals(&[shift, 1])?);
        assert_eq!(layer.multiplicity, multiplicity);
    }
    Ok(())
}

/// A declared unknown function is valid syntax but cannot supply polynomial coefficients.
/// ´test:integration:square-free-unknown-function-refusal´
#[test]
fn square_free_unknown_function_refusal() -> TestResult {
    let mut fixture = Fixture::new(Field::Real)?;
    let scalar = AtomicSort::Scalar(Field::Real);
    let handles = fixture.cas.admit(
        &[Draft::Context {
            environment: Environment {
                declarations: vec![
                    Declaration {
                        key: "x".into(),
                        display_name: "x".into(),
                        sort: fixture.sort.clone(),
                    },
                    Declaration {
                        key: "f".into(),
                        display_name: "f".into(),
                        sort: Sort::Function {
                            inputs: vec![scalar.clone()],
                            output: scalar,
                        },
                    },
                ],
                ..Environment::default()
            },
            assumptions: Vec::new(),
        }],
        &mut budget()?,
    )?;
    fixture.context = handles.first().cloned().ok_or("function context")?;
    let coordinate = fixture.term(Operator::Free("x".into()), &[])?;
    let input = fixture.term(Operator::Call("f".into()), &[coordinate])?;
    let request = fixture.request(&input)?;
    assert!(matches!(
        fixture.cas.run(&request),
        Outcome::Unsupported {
            reason: UnsupportedReason::PolynomialFragment,
            ..
        }
    ));
    Ok(())
}

/// Dense zero padding is bounded by degree and bytes; the term ceiling counts nonzero terms.
/// ´test:integration:square-free-sparse-term-boundary´
#[test]
fn square_free_sparse_term_boundary() -> TestResult {
    let mut fixture = Fixture::new(Field::Rational)?;
    let coordinate = fixture.term(Operator::Free("x".into()), &[])?;
    let input = fixture.term(Operator::IntegerPower(3), &[coordinate])?;
    let mut request = fixture.request(&input)?;
    request.limits.polynomial_terms = Some(1);
    let Outcome::Verified { result, .. } = fixture.cas.run(&request) else {
        return Err("dense padding spent the nonzero term allowance".into());
    };
    let certificate = fixture.cas.square_free_certificate(&result.witness)?;
    let layer = certificate.layers.first().ok_or("monomial layer")?;
    assert_eq!(layer.coefficients, rationals(&[0, 1])?);
    assert_eq!(layer.multiplicity, 3);
    Ok(())
}
