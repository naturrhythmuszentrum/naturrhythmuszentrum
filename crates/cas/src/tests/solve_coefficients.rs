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

/// Quotient multiplication reduces t² to one, while t-1 requires a genuine component split.
/// ´test:crate:tower-unit-and-split-decisions´
#[test]
fn tower_unit_and_split_decisions() -> TestResult {
    use crate::solving::coefficients::interface::TowerCoefficients;
    use crate::testing::solve_coefficients::UnitFixture;
    let mut fixture = UnitFixture::new()?;
    let negative = fixture.base.integer(-1)?;
    let one = fixture.base.integer(1)?;
    let divisor = fixture
        .base
        .tower_value(&fixture.domain, &[negative, one])?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let field = TowerCoefficients::new(&fixture.base.cas.state, &fixture.domain, &mut budget)?;
    let generator = field.read(&fixture.generator, &mut budget)?;
    let square = field.mul(&generator, &generator, &mut budget)?;
    let Value::Tower(coefficients) = square.value else {
        return Err("wrong tower representation".into());
    };
    assert_eq!(coefficients.len(), 1);
    assert_eq!(
        ratio(coefficients.first().ok_or("missing constant")?, &mut budget)?,
        (1, 1)
    );
    let scope = EvaluationScope {
        context: &fixture.base.context,
        region: &fixture.base.region,
        premises: &[],
        localization: &[],
    };
    assert!(matches!(
        field.inverse(&generator, &scope, &mut budget)?,
        UnitDecision::Unit { .. }
    ));
    assert!(matches!(
        field.inverse(
            &field.read(&fixture.zero, &mut budget)?,
            &scope,
            &mut budget
        )?,
        UnitDecision::Zero(_)
    ));
    let UnitDecision::Split(split) =
        field.inverse(&field.read(&divisor, &mut budget)?, &scope, &mut budget)?
    else {
        return Err("zero divisor was treated as a unit".into());
    };
    assert_eq!(split.level, fixture.level);
    assert_eq!(split.left.coefficients.len(), 2);
    assert_eq!(split.right.coefficients.len(), 2);
    assert_eq!(
        ratio(
            split
                .left
                .coefficients
                .first()
                .ok_or("missing left constant")?,
            &mut budget
        )?,
        (-1, 1)
    );
    assert_eq!(
        ratio(
            split
                .right
                .coefficients
                .first()
                .ok_or("missing right constant")?,
            &mut budget
        )?,
        (1, 1)
    );
    Ok(())
}

/// A child inverse cannot bypass a nonunit leading coefficient in its parent algebra.
/// ´test:crate:tower-parent-before-child-split´
#[test]
fn tower_parent_before_child_split() -> TestResult {
    use crate::solving::coefficients::interface::TowerCoefficients;
    use crate::testing::solve_coefficients::UnitFixture;
    let mut fixture = UnitFixture::new()?;
    let negative = fixture.base.integer(-1)?;
    let one = fixture.base.integer(1)?;
    let parent_one = fixture
        .base
        .tower_value(&fixture.domain, std::slice::from_ref(&one))?;
    let parent_divisor = fixture
        .base
        .tower_value(&fixture.domain, &[negative, one])?;
    let polynomial = fixture
        .base
        .univariate(&fixture.domain, &[(0, parent_one.clone()), (2, parent_one)])?;
    let (_, child) = fixture.base.tower(&fixture.domain, &polynomial)?;
    let value = fixture
        .base
        .tower_value(&child, std::slice::from_ref(&parent_divisor))?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let field = TowerCoefficients::new(&fixture.base.cas.state, &child, &mut budget)?;
    let scope = EvaluationScope {
        context: &fixture.base.context,
        region: &fixture.base.region,
        premises: &[],
        localization: &[],
    };
    let before = fixture.base.cas.counts();
    let UnitDecision::Split(split) =
        field.inverse(&field.read(&value, &mut budget)?, &scope, &mut budget)?
    else {
        return Err("parent split missing".into());
    };
    assert_eq!(split.level, fixture.level);
    assert_eq!(fixture.base.cas.counts(), before);
    Ok(())
}

/// A parent obstruction beneath u²+t retains the exact earlier level and dependent coefficient value.
fn rebuild_fixture() -> TestResult<(
    crate::testing::solve_coefficients::UnitFixture,
    crate::Handle,
    crate::Handle,
    Box<crate::solving::towers::SplitCertificate>,
)> {
    use crate::solving::coefficients::interface::TowerCoefficients;
    let mut fixture = crate::testing::solve_coefficients::UnitFixture::new()?;
    let one = fixture.base.integer(1)?;
    let negative = fixture.base.integer(-1)?;
    let parent_one = fixture
        .base
        .tower_value(&fixture.domain, std::slice::from_ref(&one))?;
    let obstructed = fixture
        .base
        .tower_value(&fixture.domain, &[negative, one])?;
    let polynomial = fixture.base.univariate(
        &fixture.domain,
        &[(0, fixture.generator.clone()), (2, parent_one)],
    )?;
    let (_, upper) = fixture.base.tower(&fixture.domain, &polynomial)?;
    let value = fixture
        .base
        .tower_value(&upper, std::slice::from_ref(&obstructed))?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let engine = TowerCoefficients::new(&fixture.base.cas.state, &upper, &mut budget)?;
    let element = engine.read(&value, &mut budget)?;
    let scope = EvaluationScope {
        context: &fixture.base.context,
        region: &fixture.base.region,
        premises: &[],
        localization: &[],
    };
    let UnitDecision::Split(proposal) = engine.inverse(&element, &scope, &mut budget)? else {
        return Err("missing parent split".into());
    };
    Ok((fixture, upper, value, proposal))
}

/// Parent splitting rebuilds higher levels on both children and atomically refuses one below its arithmetic cost.
/// ´test:crate:tower-parent-rebuild-boundary´
#[test]
fn tower_parent_rebuild_boundary() -> TestResult {
    use crate::solving::coefficients::arithmetic::Engine;
    use crate::solving::towers::staging::stage;
    let (mut fixture, upper, value, proposal) = rebuild_fixture()?;
    let mut measured = Budget::new(Limits::blind_cubic())?;
    let result = stage(
        &mut fixture.base.cas,
        &fixture.domain,
        &proposal,
        &[upper, value],
        &fixture.base.region,
        &[],
        &mut measured,
    )?;
    let count = measured.usage().arithmetic_steps;
    for (images, expected) in result.dependents.iter().zip([0, -2]) {
        let domain = images.first().ok_or("missing upper domain")?;
        let value = images.get(1).ok_or("missing upper value")?;
        let engine = Engine {
            state: &fixture.base.cas.state,
            domain,
        };
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let independent = Rational::from_ratio(expected, 1, &mut budget)?;
        assert!(engine.formal_equal(
            &engine.read(value, &mut budget)?,
            &engine.rational(&independent, &mut budget)?,
            &mut budget
        )?);
    }
    assert_ne!(result.children.first(), result.children.get(1));
    assert!(count > 0);
    for (limit, succeeds) in [(count, true), (count - 1, false)] {
        let (mut peer, upper, value, proposal) = rebuild_fixture()?;
        let before = peer.base.cas.counts();
        let mut budget = Budget::new(Limits {
            arithmetic_steps: limit,
            ..Limits::blind_cubic()
        })?;
        let staged = stage(
            &mut peer.base.cas,
            &peer.domain,
            &proposal,
            &[upper, value],
            &peer.base.region,
            &[],
            &mut budget,
        );
        if succeeds {
            assert!(staged.is_ok());
        } else {
            let error = staged.err().ok_or("short rebuild budget accepted")?;
            assert!(matches!(
                error.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::ArithmeticSteps,
                    ..
                }
            ));
            assert_eq!(peer.base.cas.counts(), before);
        }
    }
    Ok(())
}

/// Canonical parent inclusions and both component projections execute through the same sealed map boundary.
/// ´test:crate:tower-canonical-map-laws´
#[test]
fn tower_canonical_map_laws() -> TestResult {
    use crate::solving::coefficients::arithmetic::Engine;
    let mut fixture = crate::testing::solve_coefficients::SplitFixture::new()?;
    let one = fixture.unit.base.integer(1)?;
    let scope = EvaluationScope {
        context: &fixture.unit.base.context,
        region: &fixture.unit.base.region,
        premises: &[],
        localization: &[],
    };
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let parent = RationalCoefficients::new(
        &fixture.unit.base.cas.state,
        &fixture.unit.base.rational,
        &mut budget,
    )?;
    let map = CoefficientMapData {
        source: fixture.unit.base.rational.clone(),
        target: MapTarget::CoefficientDomain {
            domain: fixture.unit.domain.clone(),
        },
        mode: MapMode::Embedding,
        region: fixture.unit.base.region.clone(),
        images: Vec::new(),
        denominators: Vec::new(),
    };
    let embedded = parent.map(&parent.read(&one, &mut budget)?, &map, &scope, &mut budget)?;
    let tower = Engine {
        state: &fixture.unit.base.cas.state,
        domain: &fixture.unit.domain,
    };
    assert!(tower.formal_equal(&embedded, &tower.constant(true, &mut budget)?, &mut budget)?);
    projection_laws(&fixture, &scope, &mut budget)?;
    Ok(())
}

/// Both independently supplied component maps send t to the correct signed constant.
fn projection_laws(
    fixture: &crate::testing::solve_coefficients::SplitFixture,
    scope: &EvaluationScope<'_>,
    budget: &mut Budget,
) -> TestResult {
    use crate::proof::{ApplicationDraft, Certificate};
    use crate::solving::coefficients::arithmetic::Engine;
    use crate::solving::coefficients::interface::TowerCoefficients;
    use crate::solving::{SolveCertificate, SolveView};
    let interface =
        TowerCoefficients::new(&fixture.unit.base.cas.state, &fixture.unit.domain, budget)?;
    let generator = interface.read(&fixture.unit.generator, budget)?;
    let ApplicationDraft {
        certificate: Certificate::Solve(certificate),
        ..
    } = fixture.steps.last().ok_or("missing split")?
    else {
        return Err("wrong certificate".into());
    };
    let SolveCertificate::ComponentSplit(certificate) = certificate.as_ref() else {
        return Err("wrong split".into());
    };
    let SolveView::ComponentSplit(split) = fixture.unit.base.cas.solve_view(&certificate.split)?
    else {
        return Err("wrong split view".into());
    };
    for (handle, expected) in [&split.left_map, &split.right_map].into_iter().zip([1, -1]) {
        let SolveView::CoefficientMap(projection) = fixture.unit.base.cas.solve_view(handle)?
        else {
            return Err("wrong map".into());
        };
        let MapTarget::CoefficientDomain { domain } = &projection.target else {
            return Err("wrong target".into());
        };
        let image = interface.map(&generator, projection, scope, budget)?;
        let destination = Engine {
            state: &fixture.unit.base.cas.state,
            domain,
        };
        let independent = Rational::from_ratio(expected, 1, budget)?;
        assert!(destination.formal_equal(
            &image,
            &destination.rational(&independent, budget)?,
            budget
        )?);
    }
    Ok(())
}

/// Repeated support cannot manufacture a disjoint split or an inverse for a nilpotent.
/// ´test:crate:tower-repeated-support-refusal´
#[test]
fn tower_repeated_support_refusal() -> TestResult {
    use crate::solving::coefficients::interface::TowerCoefficients;
    let mut fixture = Fixture::new()?;
    let polynomial = fixture.polynomial(1, &[(vec![2], 1), (vec![1], -2), (vec![0], 1)])?;
    let parent = fixture.rational.clone();
    let (_, domain) = fixture.tower(&parent, &polynomial)?;
    let negative = fixture.integer(-1)?;
    let one = fixture.integer(1)?;
    let value = fixture.tower_value(&domain, &[negative, one])?;
    let before = fixture.cas.counts();
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let engine = TowerCoefficients::new(&fixture.cas.state, &domain, &mut budget)?;
    let element = engine.read(&value, &mut budget)?;
    let scope = EvaluationScope {
        context: &fixture.context,
        region: &fixture.region,
        premises: &[],
        localization: &[],
    };
    assert!(matches!(
        engine.inverse(&element, &scope, &mut budget),
        Err(CoefficientFailure::Unsupported(
            crate::solving::SolveReason::ComponentChoicePending
        ))
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
            scalar_spaces: Vec::new(),
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

/// A fresh split decides only its exact cell and never loses a localization denominator.
/// ´test:crate:coefficient-regional-decisions´
#[test]
fn coefficient_regional_decisions() -> TestResult {
    use crate::syntax::Operator;
    let mut fixture = Fixture::new()?;
    let ambient = fixture.region.clone();
    let expression = fixture.guard_expression(Operator::Free("x".into()), &[])?;
    let cells = fixture.guard_cells(&expression)?;
    let top = fixture.polynomial(2, &[(vec![1, 0], 1)])?;
    let other = fixture.polynomial(2, &[(vec![0, 1], 1)])?;
    let one = fixture.polynomial(2, &[(vec![0, 0], 1)])?;
    let draft = fixture.fraction(&top, &one);
    let parameter = admit(&mut fixture.cas, &draft)?;
    for (region, expected) in [
        (&ambient, None),
        (&cells[0], Some(true)),
        (&cells[1], Some(false)),
    ] {
        fixture.on_region(region)?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let field = FractionCoefficients::new(&fixture.cas.state, &fixture.fractions, &mut budget)?;
        let value = field.read(&parameter, &mut budget)?;
        let scope = EvaluationScope {
            context: &fixture.context,
            region,
            premises: &[],
            localization: &[],
        };
        match (
            expected,
            field.zero_test(&value, &scope, &mut budget)?,
            field.inverse(&value, &scope, &mut budget)?,
        ) {
            (None, ZeroDecision::Undetermined(_), UnitDecision::Undetermined(_))
            | (Some(true), ZeroDecision::Zero(_), UnitDecision::Zero(_))
            | (Some(false), ZeroDecision::Nonzero(_), UnitDecision::Unit { .. }) => {}
            _ => return Err("incorrect regional decision".into()),
        }
        let localized = EvaluationScope {
            localization: std::slice::from_ref(&other),
            ..scope
        };
        assert!(matches!(
            field.zero_test(&value, &localized, &mut budget)?,
            ZeroDecision::Undetermined(_)
        ));
    }
    Ok(())
}

/// Canonical arithmetic guards descend through meets and bases, without deciding unions or positivity.
/// ´test:crate:coefficient-regional-polynomial-scope´
#[test]
fn coefficient_regional_polynomial_scope() -> TestResult {
    use crate::domain::RegionExpr;
    use crate::syntax::Operator;
    let mut fixture = Fixture::new()?;
    let ambient = fixture.region.clone();
    let x = fixture.guard_expression(Operator::Free("x".into()), &[])?;
    let y = fixture.guard_expression(Operator::Free("y".into()), &[])?;
    let sum = fixture.guard_expression(Operator::Add, &[y, x])?;
    let cells = fixture.guard_cells(&sum)?;
    let top = fixture.polynomial(2, &[(vec![1, 0], 1), (vec![0, 1], 1)])?;
    let one = fixture.polynomial(2, &[(vec![0, 0], 1)])?;
    let draft = fixture.fraction(&top, &one);
    let parameter = admit(&mut fixture.cas, &draft)?;
    let meet = fixture.region_expression(RegionExpr::Meet(vec![
        Reference::Existing(ambient),
        Reference::Existing(cells[1].clone()),
    ]))?;
    let union = fixture.region_expression(RegionExpr::Union(
        cells.iter().cloned().map(Reference::Existing).collect(),
    ))?;
    for (region, nonzero) in [(meet, true), (union, false)] {
        fixture.on_region(&region)?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let field = FractionCoefficients::new(&fixture.cas.state, &fixture.fractions, &mut budget)?;
        let value = field.read(&parameter, &mut budget)?;
        let scope = EvaluationScope {
            context: &fixture.context,
            region: &region,
            premises: &[],
            localization: std::slice::from_ref(&top),
        };
        assert_eq!(
            matches!(
                field.inverse(&value, &scope, &mut budget)?,
                UnitDecision::Unit { .. }
            ),
            nonzero
        );
        let wrong = EvaluationScope {
            region: &cells[0],
            ..scope
        };
        assert!(field.zero_test(&value, &wrong, &mut budget).is_err());
    }
    Ok(())
}

/// Explicit nonzero guards and inherited bases work; positivity and partial syntax are not generalized.
/// ´test:crate:coefficient-explicit-regional-guards´
#[test]
fn coefficient_explicit_regional_guards() -> TestResult {
    use crate::domain::RegionExpr;
    use crate::syntax::{Operator, PredicateKind};
    let mut fixture = Fixture::new()?;
    let x = fixture.guard_expression(Operator::Free("x".into()), &[])?;
    let top = fixture.polynomial(2, &[(vec![1, 0], 1)])?;
    let one = fixture.polynomial(2, &[(vec![0, 0], 1)])?;
    let draft = fixture.fraction(&top, &one);
    let parameter = admit(&mut fixture.cas, &draft)?;
    for (kind, decided) in [
        (PredicateKind::NonZero, true),
        (PredicateKind::Positive, false),
    ] {
        let predicate = admit(
            &mut fixture.cas,
            &Draft::Condition {
                header: Reference::Existing(fixture.header.clone()),
                scope: Vec::new(),
                kind,
                arguments: vec![Reference::Existing(x.clone())],
            },
        )?;
        let region = fixture.region_expression(RegionExpr::GuardTrue {
            base: Reference::Existing(fixture.region.clone()),
            predicate: Reference::Existing(predicate),
        })?;
        let context = fixture.context.clone();
        let original = fixture.region.clone();
        fixture.on_region(&region)?;
        let y = fixture.guard_expression(Operator::Free("y".into()), &[])?;
        let nested = fixture.guard_cells(&y)?;
        fixture.on_region(&nested[1])?;
        assert_regional_inverse(&fixture, &parameter, decided)?;
        fixture.region = original;
        fixture.context = context;
    }
    let partial = fixture.guard_expression(Operator::Div, &[x.clone(), x])?;
    let cells = fixture.guard_cells(&partial)?;
    fixture.on_region(&cells[1])?;
    assert_regional_inverse(&fixture, &parameter, false)
}

/// Inspect the crate-private decision without adding a public arithmetic API.
fn assert_regional_inverse(
    fixture: &Fixture,
    parameter: &crate::Handle,
    expected: bool,
) -> TestResult {
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let field = FractionCoefficients::new(&fixture.cas.state, &fixture.fractions, &mut budget)?;
    let value = field.read(parameter, &mut budget)?;
    let scope = EvaluationScope {
        context: &fixture.context,
        region: &fixture.region,
        premises: &[],
        localization: &[],
    };
    assert_eq!(
        matches!(
            field.inverse(&value, &scope, &mut budget)?,
            UnitDecision::Unit { .. }
        ),
        expected
    );
    Ok(())
}

/// Same parameter spellings under a different full header cannot reuse regional guards.
/// ´test:crate:coefficient-regional-header-identity´
#[test]
fn coefficient_regional_header_identity() -> TestResult {
    use crate::domain::HeaderDraft;
    use crate::solving::{Coordinate, CoordinateRole, CoordinateSpaceData};
    use crate::syntax::{AtomicSort, Branch, Declaration, Environment, Field, Operator, Sort};
    let mut fixture = Fixture::new()?;
    let x = fixture.guard_expression(Operator::Free("x".into()), &[])?;
    let cells = fixture.guard_cells(&x)?;
    fixture.on_region(&cells[1])?;
    let header = admit(
        &mut fixture.cas,
        &Draft::Header(HeaderDraft::Core(Environment {
            field: Field::Real,
            branch: Branch::StrictReal,
            declarations: ["x", "y"]
                .into_iter()
                .map(|key| Declaration {
                    key: key.into(),
                    display_name: "changed".into(),
                    sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                })
                .collect(),
        })),
    )?;
    let coordinates = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            scalar_spaces: Vec::new(),
            header: Reference::Existing(header),
            coordinates: ["x", "y"]
                .into_iter()
                .map(|key| Coordinate {
                    key: key.into(),
                    field: Field::Real,
                    role: CoordinateRole::Coefficient,
                })
                .collect(),
        })),
    )?;
    fixture.fractions = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoefficientDomain(
            crate::solving::CoefficientDomainData::Fraction {
                base: Reference::Existing(fixture.rational.clone()),
                coordinates: Reference::Existing(coordinates),
            },
        )),
    )?;
    let top = fixture.polynomial(2, &[(vec![1, 0], 1)])?;
    let one = fixture.polynomial(2, &[(vec![0, 0], 1)])?;
    let draft = fixture.fraction(&top, &one);
    let parameter = admit(&mut fixture.cas, &draft)?;
    assert_regional_inverse(&fixture, &parameter, false)
}

/// Scratch zero and inverse decisions close only proved factors and retain actual denominators.
/// ´test:crate:coefficient-nonzero-product-decisions´
#[test]
fn coefficient_nonzero_product_decisions() -> TestResult {
    for case in 0..10 {
        let (fixture, handle, expected) =
            crate::testing::solve_coefficients::nonzero_closure_fixture(case)?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let field = FractionCoefficients::new(&fixture.cas.state, &fixture.fractions, &mut budget)?;
        let value = field.read(&handle, &mut budget)?;
        let scope = EvaluationScope {
            context: &fixture.context,
            region: &fixture.region,
            premises: &[],
            localization: &[],
        };
        match (
            expected,
            field.zero_test(&value, &scope, &mut budget)?,
            field.inverse(&value, &scope, &mut budget)?,
        ) {
            (true, ZeroDecision::Nonzero(_), UnitDecision::Unit { inverse, .. }) => {
                let Value::Fraction(product) = field.mul(&value, &inverse, &mut budget)?.value
                else {
                    return Err("wrong reciprocal product".into());
                };
                assert!(
                    product
                        .numerator
                        .equals(&product.denominator, &mut budget)?
                );
            }
            (false, ZeroDecision::Undetermined(_), UnitDecision::Undetermined(_)) => {}
            _ => return Err("wrong product closure decision".into()),
        }
    }
    Ok(())
}

/// The zero cell refuses every reciprocal power; a missing localization remains an exact obligation.
/// ´test:crate:coefficient-nonzero-power-obligations´
#[test]
fn coefficient_nonzero_power_obligations() -> TestResult {
    use crate::syntax::Operator;
    for exponent in 1..=3 {
        let mut fixture = Fixture::new()?;
        let x = fixture.guard_expression(Operator::Free("x".into()), &[])?;
        let children = fixture.guard_cells(&x)?;
        let handle = fixture.reciprocal(&[(vec![exponent, 0], 1)])?;
        let other = fixture.polynomial(2, &[(vec![0, 1], 1)])?;
        for (cell, localization) in [
            (&children[0], &[][..]),
            (&children[1], std::slice::from_ref(&other)),
        ] {
            fixture.on_region(cell)?;
            let mut budget = Budget::new(Limits::blind_cubic())?;
            let field =
                FractionCoefficients::new(&fixture.cas.state, &fixture.fractions, &mut budget)?;
            let value = field.read(&handle, &mut budget)?;
            let scope = EvaluationScope {
                context: &fixture.context,
                region: cell,
                premises: &[],
                localization,
            };
            let ZeroDecision::Undetermined(obligation) =
                field.zero_test(&value, &scope, &mut budget)?
            else {
                return Err("missing exact denominator obligation".into());
            };
            assert_eq!(obligation.region, *cell);
            assert_eq!(obligation.context, fixture.context);
            assert_eq!(obligation.localization, localization);
            assert!(matches!(
                field.inverse(&value, &scope, &mut budget)?,
                UnitDecision::Undetermined(_)
            ));
        }
    }
    Ok(())
}

/// Discharged sealed premises use the same closure after scratch multiplication loses graph origin.
/// ´test:crate:coefficient-nonzero-premise-powers´
#[test]
fn coefficient_nonzero_premise_powers() -> TestResult {
    let mut fixture = Fixture::new()?;
    let handle = fixture.reciprocal(&[(vec![1, 0], 1)])?;
    let premise = coefficient_assumption(&mut fixture)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let field = FractionCoefficients::new(&fixture.cas.state, &fixture.fractions, &mut budget)?;
    let reciprocal = field.read(&handle, &mut budget)?;
    let value = field.pow(&reciprocal, 3, &mut budget)?;
    let scope = EvaluationScope {
        context: &fixture.context,
        region: &fixture.region,
        premises: std::slice::from_ref(&premise),
        localization: &[],
    };
    assert!(matches!(
        field.inverse(&value, &scope, &mut budget)?,
        UnitDecision::Unit { .. }
    ));
    let unproved = EvaluationScope {
        premises: &[],
        ..scope
    };
    assert!(matches!(
        field.inverse(&value, &unproved, &mut budget)?,
        UnitDecision::Undetermined(_)
    ));
    Ok(())
}

/// A retained power denominator is proved through the same closure as the actual value denominator.
/// ´test:crate:coefficient-power-localization´
#[test]
fn coefficient_power_localization() -> TestResult {
    let (mut fixture, handle, _) = crate::testing::solve_coefficients::nonzero_closure_fixture(2)?;
    let polynomial = fixture.polynomial(2, &[(vec![3, 0], 1)])?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let field = FractionCoefficients::new(&fixture.cas.state, &fixture.fractions, &mut budget)?;
    let value = field.read(&handle, &mut budget)?;
    let scope = EvaluationScope {
        context: &fixture.context,
        region: &fixture.region,
        premises: &[],
        localization: std::slice::from_ref(&polynomial),
    };
    assert!(matches!(
        field.inverse(&value, &scope, &mut budget)?,
        UnitDecision::Unit { .. }
    ));
    Ok(())
}

/// The exact paired erasure walk charges a reproducible work boundary without enlarging any profile.
/// ´test:crate:coefficient-formal-erasure-work-boundary´
#[test]
fn coefficient_formal_erasure_work_boundary() -> TestResult {
    use crate::domain_model::{RegionDraft, RegionExpr};
    use crate::graph::{Draft, Reference};
    use crate::model::{AtomicSort, Field};
    use crate::solving::coefficients::formal_shape;
    let mut fixture = crate::testing::solve_coefficients::Fixture::new()?;
    let scope = vec![vec![AtomicSort::Scalar(Field::Complex); 3]];
    let ambient = fixture
        .cas
        .state
        .region(fixture.region.logical_id())?
        .ambient
        .clone();
    let source = crate::testing::solve_graph::admit(
        &mut fixture.cas,
        &Draft::Region(RegionDraft {
            header: Reference::Existing(fixture.header.clone()),
            scope: scope.clone(),
            ambient,
            expression: RegionExpr::Ambient,
        }),
    )?;
    let mut measured = Budget::new(Limits::blind_cubic())?;
    formal_shape::paired(
        &fixture.cas.state,
        source.logical_id(),
        fixture.region.logical_id(),
        &scope,
        &mut measured,
    )?;
    let required = measured.usage().work;
    assert_eq!(required, 127);
    for cap in [127, 126] {
        let mut limits = Limits::blind_cubic();
        limits.work = cap;
        let mut budget = Budget::new(limits)?;
        let result = formal_shape::paired(
            &fixture.cas.state,
            source.logical_id(),
            fixture.region.logical_id(),
            &scope,
            &mut budget,
        );
        if cap == required {
            result?;
            assert_eq!(budget.usage().work, required);
        } else {
            let error = result.err().ok_or("erasure accepted one below")?;
            assert!(
                matches!(error.kind, crate::error::FaultKind::Exhausted { budget: crate::budget::BudgetKind::Work, limit, needed, .. } if limit == cap && needed > cap)
            );
            assert_eq!(error.location, formal_shape::AT);
        }
    }
    Ok(())
}

/// Independently supplied exact terms build the arithmetic inputs without using gcd normalization.
fn monomial_gcd_polynomial(
    ground: crate::solving::coefficients::scalar::Ground,
    variables: usize,
    entries: &[(&[u32], &str)],
    budget: &mut Budget,
) -> TestResult<crate::solving::coefficients::polynomial::Polynomial> {
    use crate::solving::coefficients::{
        polynomial::{Polynomial, Term},
        scalar::Ground,
    };
    let mut terms = Vec::new();
    for (powers, text) in entries {
        let value = Rational::parse(text, budget)?;
        let coefficient = if ground == Ground::Rational {
            Scalar::Rational(value)
        } else {
            Scalar::Gaussian(crate::solving::numeric::Gaussian::from_parts(
                &value,
                &Rational::one(),
                budget,
            )?)
        };
        terms.push(Term {
            powers: powers.to_vec(),
            coefficient,
        });
    }
    Ok(Polynomial::new(variables, ground, terms, budget)?)
}

/// Exact gcd exponents include every term, both operand orders, zeros, units and Gaussian scalars.
/// ´test:crate:coefficient-monomial-gcd´
#[test]
fn coefficient_monomial_gcd() -> TestResult {
    use crate::solving::coefficients::{gcd::gcd, scalar::Ground};
    for ground in [Ground::Rational, Ground::Gaussian] {
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let monomial = monomial_gcd_polynomial(ground, 2, &[(&[3, 4], "2")], &mut budget)?;
        let mixed =
            monomial_gcd_polynomial(ground, 2, &[(&[5, 2], "3"), (&[1, 5], "-7")], &mut budget)?;
        let coprime =
            monomial_gcd_polynomial(ground, 2, &[(&[0, 5], "3"), (&[4, 0], "-7")], &mut budget)?;
        let zero = monomial_gcd_polynomial(ground, 2, &[], &mut budget)?;
        let unit = monomial_gcd_polynomial(ground, 2, &[(&[0, 0], "-5")], &mut budget)?;
        for (other, expected) in [
            (&mixed, [1, 2]),
            (&coprime, [0, 0]),
            (&zero, [3, 4]),
            (&unit, [0, 0]),
        ] {
            for (left, right) in [(&monomial, other), (other, &monomial)] {
                let result = gcd(left, right, &mut budget)?;
                let [term] = result.terms.as_slice() else {
                    return Err("nonmonomial gcd".into());
                };
                assert_eq!(term.powers, expected);
                assert!(
                    term.coefficient
                        .equals(&Scalar::constant(ground, true, &mut budget)?, &mut budget)?
                );
            }
        }
    }
    Ok(())
}

/// The exact arithmetic regression cubes a small fraction over a monomial denominator under 64 bits.
/// ´test:crate:coefficient-monomial-cube-range´
#[test]
fn coefficient_monomial_cube_range() -> TestResult {
    use crate::solving::coefficients::{fraction::Fraction, scalar::Ground};
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let numerator = monomial_gcd_polynomial(
        Ground::Rational,
        4,
        &[(&[0, 1, 0, 1], "1"), (&[0, 0, 2, 0], "-1/3")],
        &mut budget,
    )?;
    let denominator =
        monomial_gcd_polynomial(Ground::Rational, 4, &[(&[0, 0, 0, 2], "1")], &mut budget)?;
    let input = Fraction::new(&numerator, &denominator, &mut budget)?;
    let square = input.mul(&input, &mut budget)?;
    let cube = input.mul(&square, &mut budget)?;
    let expected = monomial_gcd_polynomial(
        Ground::Rational,
        4,
        &[
            (&[0, 3, 0, 3], "1"),
            (&[0, 2, 2, 2], "-1"),
            (&[0, 1, 4, 1], "1/3"),
            (&[0, 0, 6, 0], "-1/27"),
        ],
        &mut budget,
    )?;
    let bottom =
        monomial_gcd_polynomial(Ground::Rational, 4, &[(&[0, 0, 0, 6], "1")], &mut budget)?;
    assert!(cube.numerator.equals(&expected, &mut budget)?);
    assert!(cube.denominator.equals(&bottom, &mut budget)?);
    assert_eq!(budget.limits().component_bits, 64);
    Ok(())
}

/// Monomial detection follows, and never weakens, the full coefficient frame checks.
/// ´test:crate:coefficient-monomial-frame-refusal´
#[test]
fn coefficient_monomial_frame_refusal() -> TestResult {
    use crate::solving::coefficients::{gcd::gcd, scalar::Ground};
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let source = monomial_gcd_polynomial(Ground::Rational, 2, &[(&[2, 1], "1")], &mut budget)?;
    let width = monomial_gcd_polynomial(Ground::Rational, 1, &[(&[2], "1")], &mut budget)?;
    let field = monomial_gcd_polynomial(Ground::Gaussian, 2, &[(&[2, 1], "1")], &mut budget)?;
    assert!(gcd(&source, &width, &mut budget).is_err());
    assert!(gcd(&source, &field, &mut budget).is_err());
    Ok(())
}
