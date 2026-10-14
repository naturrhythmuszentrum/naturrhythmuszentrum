// SPDX-License-Identifier: AGPL-3.0-only
//! Independent exact multiplicity, certificate tampering and refusal fixtures.

use nrz_cas::{
    AtomicSort, Budget, BudgetKind, Cas, Constant, Declaration, Draft, Environment,
    ExpressionDraft, FaultKind, Field, Handle, Limits, LimitsOverride, Operation, Operator,
    Outcome, Primitive, ProofStep, Rational, Reference, Relation, Request, SearchResult, Sort,
    SquareFreeCertificate, UnsupportedReason,
};

/// Test setup returns errors instead of hiding failures in unchecked access.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Setup and adversarial replay have explicit independent work allowances.
fn budget() -> TestResult<Budget> {
    Ok(Budget::new(Limits {
        work: 2_000_000_000,
        replay_work: 2_000_000_000,
        ..Limits::default()
    })?)
}

/// Convert an independent integer coefficient fixture to canonical rationals.
fn rationals(values: &[i64]) -> TestResult<Vec<Rational>> {
    let mut result = Vec::new();
    for value in values {
        result.push(Rational::from_ratio(*value, 1, &mut budget()?)?);
    }
    Ok(result)
}

/// A generic field fixture with named coordinates x and y.
struct Fixture {
    /// Store under test.
    cas: Cas,
    /// Immutable context.
    context: Handle,
    /// Scalar sort matching the declared field.
    sort: Sort,
}

impl Fixture {
    /// Declare coordinates without asserting any polynomial identities.
    fn new(field: Field) -> TestResult<Self> {
        let sort = Sort::Value(AtomicSort::Scalar(field.clone()));
        let mut cas = Cas::new(Limits::default())?;
        let handles = cas.admit(
            &[Draft::Context {
                environment: Environment {
                    field,
                    declarations: ["x", "y"]
                        .into_iter()
                        .map(|key| Declaration {
                            key: key.into(),
                            display_name: key.into(),
                            sort: sort.clone(),
                        })
                        .collect(),
                    ..Environment::default()
                },
                assumptions: Vec::new(),
            }],
            &mut budget()?,
        )?;
        let context = handles.first().cloned().ok_or("missing context")?;
        Ok(Self { cas, context, sort })
    }

    /// Construct ordered syntax without invoking a polynomial producer.
    fn term(&mut self, operator: Operator, operands: &[Handle]) -> TestResult<Handle> {
        let handles = self.cas.admit(
            &[Draft::Expression(ExpressionDraft {
                operator,
                sort: self.sort.clone(),
                context: Reference::Existing(self.context.clone()),
                scope: Vec::new(),
                binding: None,
                operands: operands.iter().cloned().map(Reference::Existing).collect(),
            })],
            &mut budget()?,
        )?;
        handles
            .first()
            .cloned()
            .ok_or_else(|| "missing term".into())
    }

    /// Exact independent literal construction.
    fn number(&mut self, text: &str) -> TestResult<Handle> {
        self.term(
            Operator::Rational(Rational::parse(text, &mut budget()?)?),
            &[],
        )
    }

    /// Spell a coefficient fixture by Horner's identity, independently of the output formatter.
    fn polynomial(&mut self, coefficients: &[i64]) -> TestResult<Handle> {
        let coordinate = self.term(Operator::Free("x".into()), &[])?;
        let mut result = self.number("0")?;
        for coefficient in coefficients.iter().rev() {
            let product = self.term(Operator::Mul, &[result, coordinate.clone()])?;
            let constant = self.term(
                Operator::Rational(Rational::from_ratio(*coefficient, 1, &mut budget()?)?),
                &[],
            )?;
            result = self.term(Operator::Add, &[product, constant])?;
        }
        Ok(result)
    }

    /// Each input is covered completely before requesting multiplicities.
    fn request(&mut self, expression: &Handle) -> TestResult<Request> {
        let root = self
            .cas
            .materialize_derived(expression, expression, &mut budget()?)?;
        let receipt = self.cas.walk_derived(&root, &mut budget()?)?.receipt;
        Ok(Request {
            operation: Operation::SquareFree("x".into()),
            context: self.context.clone(),
            inputs: vec![root],
            receipts: vec![receipt],
            limits: LimitsOverride {
                work: Some(2_000_000_000),
                replay_work: Some(2_000_000_000),
                ..LimitsOverride::default()
            },
        })
    }

    /// Only a discharged, complete generated certificate satisfies these polynomial fixtures.
    fn decompose(&mut self, input: &Handle) -> TestResult<SearchResult> {
        let request = self.request(input)?;
        match self.cas.run(&request) {
            Outcome::Verified { result, .. } => Ok(result),
            other => Err(format!("expected square-free proof, got {other:?}").into()),
        }
    }
}

/// Copy a checked theorem's endpoints into a new untrusted proposal for adversarial replay.
fn proposal(result: &SearchResult, certificate: SquareFreeCertificate) -> ProofStep {
    ProofStep {
        primitive: Primitive::SquareFree,
        relation: result.witness.claim().relation.clone(),
        left: result.witness.claim().left.clone(),
        right: result.witness.claim().right.clone(),
        premises: Vec::new(),
        path: Vec::new(),
        substitutions: Vec::new(),
        certificate: Some(certificate),
    }
}

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
