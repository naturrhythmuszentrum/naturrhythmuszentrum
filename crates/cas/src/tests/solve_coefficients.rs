// SPDX-License-Identifier: AGPL-3.0-only
//! Sealed arithmetic laws, formal normalization and guarded specialization.

use crate::limits::{Budget, BudgetKind, Limits};
use crate::solving::coefficients::interface::{
    ExactCoefficients, FractionCoefficients, GaussianCoefficients, RationalCoefficients,
};
use crate::solving::coefficients::model::{
    CoefficientFailure, Element, EvaluationScope, UnitDecision, Value, ZeroDecision,
};
use crate::solving::coefficients::scalar::Scalar;
use crate::solving::{
    CoefficientMapData, CoefficientValueData, CoefficientValueObject, MapMode, MapTarget,
    SolveDraft,
};
use crate::syntax::Draft;
use crate::testing::common::TestResult;
use crate::testing::solve_coefficients::Fixture;
use crate::testing::solve_graph::admit;
use crate::{FaultKind, Rational, Reference};

/// Read a small rational through its canonical public scalar representation.
fn ratio(value: &Element, budget: &mut Budget) -> TestResult<(i64, u64)> {
    let Value::Scalar(Scalar::Rational(scalar)) = &value.value else {
        return Err("wrong rational representation".into());
    };
    scalar
        .small_ratio(budget)?
        .ok_or_else(|| "oracle overflow".into())
}

/// Every sealed rational operation agrees with independent small-integer arithmetic.
/// ´test:crate:coefficient-rational-laws´
#[test]
fn coefficient_rational_laws() -> TestResult {
    let mut fixture = Fixture::new()?;
    let left = fixture.integer(3)?;
    let right = fixture.integer(-2)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let field = RationalCoefficients::new(&fixture.cas.state, &fixture.rational, &mut budget)?;
    let a = field.read(&left, &mut budget)?;
    let b = field.read(&right, &mut budget)?;
    assert_eq!(field.domain(), fixture.rational);
    assert_eq!(ratio(&field.zero(&mut budget)?, &mut budget)?, (0, 1));
    assert_eq!(ratio(&field.one(&mut budget)?, &mut budget)?, (1, 1));
    assert_eq!(ratio(&field.copy(&a, &mut budget)?, &mut budget)?, (3, 1));
    assert_eq!(ratio(&field.neg(&a, &mut budget)?, &mut budget)?, (-3, 1));
    assert_eq!(
        ratio(&field.add(&a, &b, &mut budget)?, &mut budget)?,
        (1, 1)
    );
    assert_eq!(
        ratio(&field.sub(&a, &b, &mut budget)?, &mut budget)?,
        (5, 1)
    );
    assert_eq!(
        ratio(&field.mul(&a, &b, &mut budget)?, &mut budget)?,
        (-6, 1)
    );
    assert_eq!(
        ratio(&field.pow(&b, 3, &mut budget)?, &mut budget)?,
        (-8, 1)
    );
    assert_eq!(ratio(&field.reduce(&a, &mut budget)?, &mut budget)?, (3, 1));
    let scope = EvaluationScope {
        context: &fixture.context,
        region: &fixture.region,
        premises: &[],
        localization: &[],
    };
    let UnitDecision::Unit { inverse, .. } = field.inverse(&a, &scope, &mut budget)? else {
        return Err("rational unit missing".into());
    };
    assert_eq!(ratio(&inverse, &mut budget)?, (1, 3));
    assert!(matches!(
        field.zero_test(&field.zero(&mut budget)?, &scope, &mut budget)?,
        ZeroDecision::Zero(_)
    ));
    assert!(matches!(
        field.inverse(&field.zero(&mut budget)?, &scope, &mut budget)?,
        UnitDecision::Zero(_)
    ));
    Ok(())
}

/// Gaussian products retain the independent integer identity (2+i)^3 = 2+11i.
/// ´test:crate:coefficient-gaussian-product´
#[test]
fn coefficient_gaussian_product() -> TestResult {
    let mut fixture = Fixture::new()?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let value = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
            domain: Reference::Existing(fixture.gaussian.clone()),
            value: CoefficientValueData::Gaussian {
                real: Rational::from_ratio(2, 1, &mut budget)?,
                imaginary: Rational::one(),
            },
        })),
    )?;
    let field = GaussianCoefficients::new(&fixture.cas.state, &fixture.gaussian, &mut budget)?;
    let cube = field.pow(&field.read(&value, &mut budget)?, 3, &mut budget)?;
    let Value::Scalar(Scalar::Gaussian(result)) = cube.value else {
        return Err("wrong Gaussian representation".into());
    };
    assert_eq!(
        result.real(&mut budget)?.small_ratio(&mut budget)?,
        Some((2, 1))
    );
    assert_eq!(
        result.imaginary(&mut budget)?.small_ratio(&mut budget)?,
        Some((11, 1))
    );
    Ok(())
}

/// A nonzero formal parameter remains a pending regional unit, and evaluating it at zero is allowed.
/// ´test:crate:coefficient-specialization-guard´
#[test]
fn coefficient_specialization_guard() -> TestResult {
    let mut fixture = Fixture::new()?;
    let top = fixture.polynomial(2, &[(vec![1, 0], 1)])?;
    let bottom = fixture.polynomial(2, &[(vec![0, 0], 1)])?;
    let proposed = fixture.fraction(&top, &bottom);
    let value = admit(&mut fixture.cas, &proposed)?;
    let zero = fixture.integer(0)?;
    let one = fixture.integer(1)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let field = FractionCoefficients::new(&fixture.cas.state, &fixture.fractions, &mut budget)?;
    let parameter = field.read(&value, &mut budget)?;
    let scope = EvaluationScope {
        context: &fixture.context,
        region: &fixture.region,
        premises: &[],
        localization: &[],
    };
    assert!(matches!(
        field.inverse(&parameter, &scope, &mut budget)?,
        UnitDecision::Undetermined(_)
    ));
    let mut mapping = CoefficientMapData {
        source: fixture.fractions.clone(),
        target: MapTarget::CoefficientDomain {
            domain: fixture.rational.clone(),
        },
        mode: MapMode::Specialization,
        region: fixture.region.clone(),
        images: vec![zero, one],
        denominators: Vec::new(),
    };
    assert_eq!(
        ratio(
            &field.map(&parameter, &mapping, &scope, &mut budget)?,
            &mut budget
        )?,
        (0, 1)
    );
    mapping.mode = MapMode::Embedding;
    assert!(
        field
            .map(&parameter, &mapping, &scope, &mut budget)
            .is_err()
    );
    Ok(())
}

/// A normalized product cancels a polynomial factor through exact multivariate gcd work.
/// ´test:crate:coefficient-fraction-product´
#[test]
fn coefficient_fraction_product() -> TestResult {
    let mut fixture = Fixture::new()?;
    let x = fixture.polynomial(2, &[(vec![1, 0], 1)])?;
    let y = fixture.polynomial(2, &[(vec![0, 1], 1)])?;
    let first = fixture.fraction(&x, &y);
    let second = fixture.fraction(&y, &x);
    let left = fixture
        .cas
        .admit(
            std::slice::from_ref(&first),
            &mut Budget::new(Limits::blind_cubic())?,
        )?
        .into_iter()
        .next()
        .ok_or("missing admitted fraction")?;
    let right = fixture
        .cas
        .admit(
            std::slice::from_ref(&second),
            &mut Budget::new(Limits::blind_cubic())?,
        )?
        .into_iter()
        .next()
        .ok_or("missing admitted fraction")?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let field = FractionCoefficients::new(&fixture.cas.state, &fixture.fractions, &mut budget)?;
    let result = field.mul(
        &field.read(&left, &mut budget)?,
        &field.read(&right, &mut budget)?,
        &mut budget,
    )?;
    let Value::Fraction(value) = result.value else {
        return Err("wrong fraction representation".into());
    };
    let one = value.numerator.one(&mut budget)?;
    assert!(value.numerator.equals(&one, &mut budget)?);
    assert!(value.denominator.equals(&one, &mut budget)?);
    Ok(())
}

/// Exact operation limits are reproducible and arithmetic failure cannot mutate the graph.
/// ´test:crate:coefficient-operation-boundary´
#[test]
fn coefficient_operation_boundary() -> TestResult {
    let mut fixture = Fixture::new()?;
    let stored = fixture.integer(3)?;
    let mut setup = Budget::new(Limits::blind_cubic())?;
    let field = RationalCoefficients::new(&fixture.cas.state, &fixture.rational, &mut setup)?;
    let value = field.read(&stored, &mut setup)?;
    let mut measured = Budget::new(Limits::blind_cubic())?;
    field.pow(&value, 5, &mut measured)?;
    let count = measured.usage().coefficient_ops;
    let before = fixture.cas.counts();
    field.pow(
        &value,
        5,
        &mut Budget::new(Limits {
            coefficient_ops: count,
            ..Limits::blind_cubic()
        })?,
    )?;
    let failure = field
        .pow(
            &value,
            5,
            &mut Budget::new(Limits {
                coefficient_ops: count - 1,
                ..Limits::blind_cubic()
            })?,
        )
        .err()
        .ok_or("short budget accepted")?;
    assert!(matches!(
        failure,
        CoefficientFailure::Fault(crate::Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::CoefficientOps,
                ..
            },
            ..
        })
    ));
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Recursive content and primitive factors cancel according to independently expanded products.
/// ´test:crate:coefficient-multivariate-normalization´
#[test]
fn coefficient_multivariate_normalization() -> TestResult {
    let mut fixture = Fixture::new()?;
    // (x+y)(x+1) and (x+y)(y+1), expanded independently over small integers.
    let top = fixture.polynomial(
        2,
        &[
            (vec![2, 0], 1),
            (vec![1, 1], 1),
            (vec![1, 0], 1),
            (vec![0, 1], 1),
        ],
    )?;
    let bottom = fixture.polynomial(
        2,
        &[
            (vec![1, 1], 1),
            (vec![0, 2], 1),
            (vec![1, 0], 1),
            (vec![0, 1], 1),
        ],
    )?;
    let expected_top = fixture.polynomial(2, &[(vec![1, 0], 1), (vec![0, 0], 1)])?;
    let expected_bottom = fixture.polynomial(2, &[(vec![0, 1], 1), (vec![0, 0], 1)])?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let state = &fixture.cas.state;
    let numerator = crate::solving::coefficients::read::sparse(state, &top, &mut budget)?;
    let denominator = crate::solving::coefficients::read::sparse(state, &bottom, &mut budget)?;
    let normalized = crate::solving::coefficients::fraction::Fraction::new(
        &numerator,
        &denominator,
        &mut budget,
    )?;
    assert!(normalized.numerator.equals(
        &crate::solving::coefficients::read::sparse(state, &expected_top, &mut budget)?,
        &mut budget
    )?);
    assert!(normalized.denominator.equals(
        &crate::solving::coefficients::read::sparse(state, &expected_bottom, &mut budget)?,
        &mut budget
    )?);
    Ok(())
}

/// Scalar arithmetic succeeds exactly at its measured dependency-call limit and refuses one below it.
/// ´test:crate:coefficient-arithmetic-boundary´
#[test]
fn coefficient_arithmetic_boundary() -> TestResult {
    let mut fixture = Fixture::new()?;
    let stored = fixture.integer(7)?;
    let mut setup = Budget::new(Limits::blind_cubic())?;
    let field = RationalCoefficients::new(&fixture.cas.state, &fixture.rational, &mut setup)?;
    let value = field.read(&stored, &mut setup)?;
    let mut measured = Budget::new(Limits::blind_cubic())?;
    field.mul(&value, &value, &mut measured)?;
    let count = measured.usage().arithmetic_steps;
    assert!(count > 0);
    let before = fixture.cas.counts();
    field.mul(
        &value,
        &value,
        &mut Budget::new(Limits {
            arithmetic_steps: count,
            ..Limits::blind_cubic()
        })?,
    )?;
    let failure = field
        .mul(
            &value,
            &value,
            &mut Budget::new(Limits {
                arithmetic_steps: count - 1,
                ..Limits::blind_cubic()
            })?,
        )
        .err()
        .ok_or("short arithmetic budget accepted")?;
    assert!(matches!(
        failure,
        CoefficientFailure::Fault(crate::Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::ArithmeticSteps,
                ..
            },
            ..
        })
    ));
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Distinct coordinate renaming is injective; identifying two independent parameters is refused.
/// ´test:crate:coefficient-renaming-injectivity´
#[test]
fn coefficient_renaming_injectivity() -> TestResult {
    let mut fixture = Fixture::new()?;
    let first = fixture.polynomial(2, &[(vec![1, 0], 1)])?;
    let second = fixture.polynomial(2, &[(vec![0, 1], 1)])?;
    let unit = fixture.polynomial(2, &[(vec![0, 0], 1)])?;
    let first_draft = fixture.fraction(&first, &unit);
    let second_draft = fixture.fraction(&second, &unit);
    let first_value = admit(&mut fixture.cas, &first_draft)?;
    let second_value = admit(&mut fixture.cas, &second_draft)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let engine = FractionCoefficients::new(&fixture.cas.state, &fixture.fractions, &mut budget)?;
    let scope = EvaluationScope {
        context: &fixture.context,
        region: &fixture.region,
        premises: &[],
        localization: &[],
    };
    let mut mapping = CoefficientMapData {
        source: fixture.fractions.clone(),
        target: MapTarget::CoefficientDomain {
            domain: fixture.fractions.clone(),
        },
        mode: MapMode::Embedding,
        region: fixture.region.clone(),
        images: vec![second_value.clone(), first_value.clone()],
        denominators: Vec::new(),
    };
    let element = engine.read(&first_value, &mut budget)?;
    let renamed = engine.map(&element, &mapping, &scope, &mut budget)?;
    let Value::Fraction(actual) = &renamed.value else {
        return Err("wrong mapped fraction".into());
    };
    let Value::Fraction(expected) = engine.read(&second_value, &mut budget)?.value else {
        return Err("wrong independent fraction".into());
    };
    assert!(actual.numerator.equals(&expected.numerator, &mut budget)?);
    assert!(
        actual
            .denominator
            .equals(&expected.denominator, &mut budget)?
    );
    *mapping.images.get_mut(1).ok_or("missing second image")? = second_value;
    assert!(engine.map(&element, &mapping, &scope, &mut budget).is_err());
    Ok(())
}

/// A finite localization keeps a denominator guard even after the represented result has become one.
/// ´test:crate:coefficient-localization-pole´
#[test]
fn coefficient_localization_pole() -> TestResult {
    let mut fixture = Fixture::new()?;
    let denominator = fixture.polynomial(2, &[(vec![0, 1], 1)])?;
    let unit = fixture.polynomial(2, &[(vec![0, 0], 1)])?;
    let draft = fixture.fraction(&unit, &unit);
    let value = admit(&mut fixture.cas, &draft)?;
    let one = fixture.integer(1)?;
    let zero = fixture.integer(0)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let engine = FractionCoefficients::new(&fixture.cas.state, &fixture.fractions, &mut budget)?;
    let element = engine.read(&value, &mut budget)?;
    let mut mapping = CoefficientMapData {
        source: fixture.fractions.clone(),
        target: MapTarget::CoefficientDomain {
            domain: fixture.rational.clone(),
        },
        mode: MapMode::Specialization,
        region: fixture.region.clone(),
        images: vec![one.clone(), zero],
        denominators: vec![denominator.clone()],
    };
    let scope = EvaluationScope {
        context: &fixture.context,
        region: &fixture.region,
        premises: &[],
        localization: std::slice::from_ref(&denominator),
    };
    let before = fixture.cas.counts();
    assert!(matches!(
        engine.map(&element, &mapping, &scope, &mut budget),
        Err(CoefficientFailure::Unsupported(
            crate::solving::SolveReason::SpecializationGuardPending
        ))
    ));
    assert_eq!(fixture.cas.counts(), before);
    *mapping
        .images
        .get_mut(1)
        .ok_or("missing denominator image")? = one;
    assert_eq!(
        ratio(
            &engine.map(&element, &mapping, &scope, &mut budget)?,
            &mut budget
        )?,
        (1, 1)
    );
    Ok(())
}

/// Seal an explicit x≠0 context assumption without using any solving checker.
fn coefficient_assumption(fixture: &mut Fixture) -> TestResult<crate::proof::VerifiedWitness> {
    use crate::domain::{AssumptionDraft, ContextDraft, HypothesisRef};
    use crate::proof::{ApplicationDraft, Certificate, Primitive, Statement};
    use crate::syntax::{AtomicSort, Field, Operator, PredicateKind, Sort};
    let parameter = admit(
        &mut fixture.cas,
        &crate::testing::solve_graph::expression(
            Reference::Existing(fixture.header.clone()),
            Operator::Free("x".into()),
            Sort::Value(AtomicSort::Scalar(Field::Real)),
            Vec::new(),
        ),
    )?;
    let predicate = admit(
        &mut fixture.cas,
        &Draft::Condition {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::NonZero,
            arguments: vec![Reference::Existing(parameter)],
        },
    )?;
    fixture.context = admit(
        &mut fixture.cas,
        &Draft::Context(ContextDraft::Root {
            header: Reference::Existing(fixture.header.clone()),
            admissible: Reference::Existing(fixture.region.clone()),
            assumptions: vec![AssumptionDraft {
                key: "nonzero-x".into(),
                condition: Reference::Existing(predicate.clone()),
            }],
        }),
    )?;
    let step = ApplicationDraft {
        rule: Primitive::ContextHypothesis,
        context: fixture.context.clone(),
        conclusion: Statement::PredicateFact {
            region: fixture.region.clone(),
            predicate,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Hypothesis(HypothesisRef {
            context: fixture.context.clone(),
            slot: 0,
        }),
    };
    Ok(fixture.cas.check_proof(
        &fixture.context,
        std::slice::from_ref(&step),
        &mut Budget::new(Limits::blind_cubic())?,
    )?)
}

/// A sealed nonzero parameter hypothesis permits regional inversion without treating formal nonzero as sufficient.
/// ´test:crate:coefficient-scoped-nonzero´
#[test]
fn coefficient_scoped_nonzero() -> TestResult {
    let mut fixture = Fixture::new()?;
    let numerator = fixture.polynomial(2, &[(vec![1, 0], 1)])?;
    let denominator = fixture.polynomial(2, &[(vec![0, 0], 1)])?;
    let draft = fixture.fraction(&numerator, &denominator);
    let handle = admit(&mut fixture.cas, &draft)?;
    let premise = coefficient_assumption(&mut fixture)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let engine = FractionCoefficients::new(&fixture.cas.state, &fixture.fractions, &mut budget)?;
    let value = engine.read(&handle, &mut budget)?;
    let scope = EvaluationScope {
        context: &fixture.context,
        region: &fixture.region,
        premises: std::slice::from_ref(&premise),
        localization: &[],
    };
    let UnitDecision::Unit { inverse, .. } = engine.inverse(&value, &scope, &mut budget)? else {
        return Err("scoped inverse missing".into());
    };
    let Value::Fraction(product) = engine.mul(&value, &inverse, &mut budget)?.value else {
        return Err("wrong product".into());
    };
    assert!(
        product
            .numerator
            .equals(&product.denominator, &mut budget)?
    );
    assert_eq!(premise.hypotheses().len(), 1);
    Ok(())
}

/// Construct Q(y), its zero and its independent y without calling the arithmetic implementation.
fn smaller_fraction_domain(
    fixture: &mut Fixture,
) -> TestResult<(crate::Handle, crate::Handle, crate::Handle)> {
    use crate::solving::{CoefficientDomainData, Coordinate, CoordinateRole, CoordinateSpaceData};
    use crate::syntax::Field;
    let coordinates = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            header: Reference::Existing(fixture.header.clone()),
            coordinates: vec![Coordinate {
                key: "y".into(),
                field: Field::Real,
                role: CoordinateRole::Coefficient,
            }],
        })),
    )?;
    let domain = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Fraction {
                base: Reference::Existing(fixture.rational.clone()),
                coordinates: Reference::Existing(coordinates),
            },
        )),
    )?;
    let one = fixture.polynomial(1, &[(vec![0], 1)])?;
    let variable = fixture.polynomial(1, &[(vec![1], 1)])?;
    let zero = fixture.polynomial(1, &[])?;
    let mut values = Vec::new();
    for numerator in [variable, zero] {
        values.push(admit(
            &mut fixture.cas,
            &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
                domain: Reference::Existing(domain.clone()),
                value: CoefficientValueData::Fraction {
                    numerator: Reference::Existing(numerator),
                    denominator: Reference::Existing(one.clone()),
                },
            })),
        )?);
    }
    Ok((
        domain,
        values.first().ok_or("missing parameter")?.clone(),
        values.get(1).ok_or("missing zero")?.clone(),
    ))
}

/// Specialization can lower the parameter field while retaining a guard for a denominator sent to zero.
/// ´test:crate:coefficient-partial-parameter-specialization´
#[test]
fn coefficient_partial_parameter_specialization() -> TestResult {
    let mut fixture = Fixture::new()?;
    let numerator = fixture.polynomial(2, &[(vec![1, 0], 1), (vec![0, 1], 1)])?;
    let denominator = fixture.polynomial(2, &[(vec![0, 0], 1)])?;
    let excluded = fixture.polynomial(2, &[(vec![1, 0], 1)])?;
    let draft = fixture.fraction(&numerator, &denominator);
    let value = admit(&mut fixture.cas, &draft)?;
    let (target, parameter, zero) = smaller_fraction_domain(&mut fixture)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let source = FractionCoefficients::new(&fixture.cas.state, &fixture.fractions, &mut budget)?;
    let destination = FractionCoefficients::new(&fixture.cas.state, &target, &mut budget)?;
    let element = source.read(&value, &mut budget)?;
    let mut mapping = CoefficientMapData {
        source: fixture.fractions.clone(),
        target: MapTarget::CoefficientDomain {
            domain: target.clone(),
        },
        mode: MapMode::Specialization,
        region: fixture.region.clone(),
        images: vec![zero, parameter.clone()],
        denominators: Vec::new(),
    };
    let scope = EvaluationScope {
        context: &fixture.context,
        region: &fixture.region,
        premises: &[],
        localization: &[],
    };
    let Value::Fraction(actual) = source.map(&element, &mapping, &scope, &mut budget)?.value else {
        return Err("wrong specialized fraction".into());
    };
    let Value::Fraction(expected) = destination.read(&parameter, &mut budget)?.value else {
        return Err("wrong expected fraction".into());
    };
    assert!(actual.numerator.equals(&expected.numerator, &mut budget)?);
    assert!(
        actual
            .denominator
            .equals(&expected.denominator, &mut budget)?
    );
    mapping.denominators.push(excluded.clone());
    let guarded = EvaluationScope {
        context: &fixture.context,
        region: &fixture.region,
        premises: &[],
        localization: std::slice::from_ref(&excluded),
    };
    assert!(matches!(
        source.map(&element, &mapping, &guarded, &mut budget),
        Err(CoefficientFailure::Unsupported(
            crate::solving::SolveReason::SpecializationGuardPending
        ))
    ));
    Ok(())
}
