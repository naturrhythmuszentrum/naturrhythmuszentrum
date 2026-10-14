// SPDX-License-Identifier: AGPL-3.0-only
//! Complete public-API client flows for square-free.

use nrz_cas::{
    Cas, Field, Limits, Operator, Primitive, ProofStep, Rational, Relation, SquareFreeCertificate,
};

use crate::testing::budgets::derivation_budget as budget;
use crate::testing::common::TestResult;
use crate::testing::square_free::{Fixture, rationals};

/// The degree-five oracle is independently expanded in the accepted design.
/// ´test:integration:square-free-degree-five´
#[test]
fn square_free_degree_five() -> TestResult {
    let mut fixture = Fixture::new(Field::Rational)?;
    let input = fixture.polynomial(&[-4, 8, -1, -5, 1, 1])?;
    let result = fixture.decompose(&input)?;
    let certificate = fixture.cas.square_free_certificate(&result.witness)?;
    assert_eq!(certificate.scalar, Rational::one());
    assert_eq!(certificate.initial_gcd.gcd, rationals(&[2, -3, 0, 1])?);
    assert_eq!(
        certificate.initial_gcd.left_quotient,
        rationals(&[-2, 1, 1])?
    );
    assert_eq!(certificate.rounds.len(), 3);
    assert_eq!(certificate.layers.len(), 2);
    assert_eq!(
        certificate
            .layers
            .first()
            .ok_or("first layer")?
            .multiplicity,
        2
    );
    assert_eq!(
        certificate
            .layers
            .first()
            .ok_or("first layer")?
            .coefficients,
        rationals(&[2, 1])?
    );
    assert_eq!(
        certificate
            .layers
            .get(1)
            .ok_or("second layer")?
            .multiplicity,
        3
    );
    assert_eq!(
        certificate
            .layers
            .get(1)
            .ok_or("second layer")?
            .coefficients,
        rationals(&[-1, 1])?
    );
    assert_eq!(certificate.square_free.len(), 2);
    assert_eq!(certificate.coprime.len(), 1);
    assert_eq!(result.cost, 8);
    assert_eq!(
        result.witness.claim().relation,
        Relation::SquareFree {
            variable: "x".into(),
            field: Field::Rational
        }
    );
    let bytes = fixture
        .cas
        .export_witness(&result.witness, &mut budget()?)?;
    let mut fresh = Cas::new(Limits::default())?;
    let witness = fresh.inspect(&bytes, &mut budget()?)?;
    assert_eq!(fresh.export_witness(&witness, &mut budget()?)?, bytes);
    assert_eq!(fresh.square_free_certificate(&witness)?, certificate);
    assert!(fresh.square_free_certificate(&result.witness).is_err());
    Ok(())
}

/// A hand-written Bezout identity proves x²+1 without calling the multiplicity producer.
/// ´test:integration:square-free-independent-certificate´
#[test]
fn square_free_independent_certificate() -> TestResult {
    let mut fixture = Fixture::new(Field::Rational)?;
    let input = fixture.polynomial(&[1, 0, 1])?;
    let one = fixture.number("1")?;
    let power = fixture.term(Operator::IntegerPower(1), std::slice::from_ref(&input))?;
    let output = fixture.term(Operator::Mul, &[one, power])?;
    // (x²+1) + (-x/2)(2x) = 1, with both divisions by one exact.
    let derivative_gcd = nrz_cas::GcdCertificate {
        gcd: rationals(&[1])?,
        left_quotient: rationals(&[1, 0, 1])?,
        right_quotient: rationals(&[0, 2])?,
        bezout_left: rationals(&[1])?,
        bezout_right: vec![Rational::zero(), Rational::parse("-1/2", &mut budget()?)?],
    };
    let round = nrz_cas::GcdCertificate {
        gcd: rationals(&[1])?,
        left_quotient: rationals(&[1, 0, 1])?,
        right_quotient: rationals(&[1])?,
        bezout_left: Vec::new(),
        bezout_right: rationals(&[1])?,
    };
    let certificate = SquareFreeCertificate {
        scalar: Rational::one(),
        initial_gcd: derivative_gcd.clone(),
        rounds: vec![round],
        layers: vec![nrz_cas::SquareFreeLayer {
            multiplicity: 1,
            coefficients: rationals(&[1, 0, 1])?,
        }],
        square_free: vec![derivative_gcd],
        coprime: Vec::new(),
    };
    let step = ProofStep {
        primitive: Primitive::SquareFree,
        relation: Relation::SquareFree {
            variable: "x".into(),
            field: Field::Rational,
        },
        left: input,
        right: output,
        premises: Vec::new(),
        path: Vec::new(),
        substitutions: Vec::new(),
        certificate: Some(certificate.clone()),
    };
    let witness = fixture
        .cas
        .check_proof(&fixture.context, &[step], &mut budget()?)?;
    assert_eq!(fixture.cas.square_free_certificate(&witness)?, &certificate);
    let bytes = fixture.cas.export_witness(&witness, &mut budget()?)?;
    let mut fresh = Cas::new(Limits::default())?;
    let replayed = fresh.inspect(&bytes, &mut budget()?)?;
    assert_eq!(fresh.square_free_certificate(&replayed)?, &certificate);
    Ok(())
}
