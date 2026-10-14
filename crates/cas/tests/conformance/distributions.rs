// SPDX-License-Identifier: AGPL-3.0-only
//! Hand-computed distribution identities and their portable native proofs.

use crate::testing::boundary::{named, quadratic};
use crate::testing::budgets::derivation_budget as budget;
use crate::testing::common::TestResult;
use crate::testing::distributions::{Fixture, output, replay};
use nrz_cas::Handle;
use nrz_cas::proof::DomainContract;
use nrz_cas::request::{ActionMode, Task};
use nrz_cas::source::SyntaxRole;
use nrz_cas::syntax::{CaseName, Operator};

/// Both the transparent name and its explicit case definition have second derivative two deltas.
/// ´test:integration:distribution-abs-named-and-expanded´
#[test]
fn distribution_abs_named_and_expanded() -> TestResult {
    for expanded in [false, true] {
        let mut fixture = Fixture::new()?;
        let mut function = named(&mut fixture.base, CaseName::Abs)?;
        if expanded {
            function = fixture
                .base
                .cas
                .describe_syntax(&function, &mut budget()?)?
                .dependencies
                .into_iter()
                .find(|dependency| dependency.role == SyntaxRole::Operand(0))
                .ok_or("named definition")?
                .target;
        }
        let (embedded, embedding) = fixture.embed(&function)?;
        replay(&fixture, &embedding)?;
        let result = fixture.derivative(&embedded, 2)?;
        let delta = fixture.delta("0", 0)?;
        let expected = fixture.scale("2", &delta)?;
        assert_eq!(output(&result)?, expected);
        replay(&fixture, &result)?;
    }
    Ok(())
}

/// J has regular derivative 2x and value jump one at zero.
/// ´test:integration:distribution-quadratic-jump´
#[test]
fn distribution_quadratic_jump() -> TestResult {
    let mut fixture = Fixture::new()?;
    let function = quadratic(&mut fixture.base, true)?;
    let (embedded, _) = fixture.embed(&function)?;
    let result = fixture.derivative(&embedded, 1)?;
    let two = fixture.base.number("2")?;
    let scalar = fixture
        .base
        .term(Operator::Mul, &[two, fixture.base.x.clone()])?;
    let regular = fixture.node(Operator::RegularEmbed, &[scalar])?;
    let delta = fixture.delta("0", 0)?;
    let expected = fixture.node(Operator::DistributionAdd, &[regular, delta])?;
    equal(&mut fixture, &output(&result)?, &expected)?;
    replay(&fixture, &result)?;
    Ok(())
}

/// Q is continuous with continuous first derivative; its second derivative jumps from two to four.
/// ´test:integration:distribution-quadratic-third-derivative´
#[test]
fn distribution_quadratic_third_derivative() -> TestResult {
    let mut fixture = Fixture::new()?;
    let function = quadratic(&mut fixture.base, false)?;
    let (embedded, _) = fixture.embed(&function)?;
    let result = fixture.derivative(&embedded, 3)?;
    let delta = fixture.delta("0", 0)?;
    let expected = fixture.scale("2", &delta)?;
    assert_eq!(output(&result)?, expected);
    replay(&fixture, &result)?;
    Ok(())
}

/// The supported polynomial has value seven and first derivative four at two.
/// ´test:integration:distribution-supported-delta-actions´
#[test]
fn distribution_supported_delta_actions() -> TestResult {
    let mut fixture = Fixture::new()?;
    let coordinate = fixture.base.x.clone();
    let square = fixture
        .base
        .term(Operator::IntegerPower(2), &[coordinate])?;
    let three = fixture.base.number("3")?;
    let polynomial = fixture.base.term(Operator::Add, &[square, three])?;
    for (order, expected) in [(0, "7"), (1, "-4")] {
        let delta = fixture.delta("2", order)?;
        let result = fixture.pair(&delta, &polynomial)?;
        assert_eq!(output(&result)?, fixture.base.number(expected)?);
        replay(&fixture, &result)?;
    }
    Ok(())
}

/// Leibniz signs give x times delta equal to zero and x times delta-prime equal to minus delta.
/// ´test:integration:distribution-smooth-delta-products´
#[test]
fn distribution_smooth_delta_products() -> TestResult {
    let mut fixture = Fixture::new()?;
    for order in [0, 1] {
        let delta = fixture.delta("0", order)?;
        let request = fixture.base.request(
            Task::SmoothMultiply {
                space: fixture.space.clone(),
            },
            &[fixture.base.x.clone(), delta],
        )?;
        let result = fixture.base.proved(&request)?;
        let expected = if order == 0 {
            fixture.node(Operator::DistributionZero, &[])?
        } else {
            let delta = fixture.delta("0", 0)?;
            fixture.scale("-1", &delta)?
        };
        assert_eq!(output(&result)?, expected);
        replay(&fixture, &result)?;
    }
    Ok(())
}

/// Rational coefficients survive regular embedding, two derivatives and enclosing linearity.
/// ´test:integration:distribution-scaled-ramp´
#[test]
fn distribution_scaled_ramp() -> TestResult {
    let mut fixture = Fixture::new()?;
    let ramp = named(&mut fixture.base, CaseName::Ramp)?;
    let coefficient = fixture.base.number("3/2")?;
    let function = fixture.base.term(Operator::Mul, &[coefficient, ramp])?;
    let (embedded, _) = fixture.embed(&function)?;
    let derivative = fixture.node(Operator::DistributionDerivative { order: 2 }, &[embedded])?;
    let left = fixture.scale("2", &derivative)?;
    let delta = fixture.delta("0", 0)?;
    let expected = fixture.scale("3", &delta)?;
    equal(&mut fixture, &left, &expected)?;
    Ok(())
}

/// An ordinary declared test yields the exact symbolic signed derivative evaluation.
/// ´test:integration:distribution-symbolic-test-action´
#[test]
fn distribution_symbolic_test_action() -> TestResult {
    let mut fixture = Fixture::new()?;
    let test = fixture.test()?;
    let delta = fixture.delta("2", 1)?;
    let request = fixture.base.request(
        Task::Pair {
            space: fixture.space.clone(),
            action_mode: ActionMode::Test,
        },
        &[delta, test.clone()],
    )?;
    let result = fixture.base.proved(&request)?;
    let derivative = fixture.node(Operator::TestDerivative { order: 1 }, &[test])?;
    let point = fixture.base.number("2")?;
    let value = fixture.node(Operator::TestApply, &[derivative, point])?;
    let expected = fixture.node(Operator::Neg, &[value])?;
    assert_eq!(output(&result)?, expected);
    replay(&fixture, &result)?;
    Ok(())
}

/// The second-order multiplier formula exposes the independently expected binomial factor two.
/// ´test:integration:distribution-second-order-leibniz´
#[test]
fn distribution_second_order_leibniz() -> TestResult {
    let mut fixture = Fixture::new()?;
    let square = fixture
        .base
        .term(Operator::IntegerPower(2), &[fixture.base.x.clone()])?;
    let three = fixture.base.number("3")?;
    let polynomial = fixture.base.term(Operator::Add, &[square, three])?;
    let delta = fixture.delta("2", 2)?;
    let request = fixture.base.request(
        Task::SmoothMultiply {
            space: fixture.space.clone(),
        },
        &[polynomial, delta],
    )?;
    let result = fixture.base.proved(&request)?;
    let mut terms = Vec::new();
    for (order, coefficient) in [(2, "7"), (1, "-8"), (0, "2")] {
        let delta = fixture.delta("2", order)?;
        terms.push(fixture.scale(coefficient, &delta)?);
    }
    let [first, second, third] = terms.as_slice() else {
        return Err("three Leibniz terms".into());
    };
    let prefix = fixture.node(Operator::DistributionAdd, &[first.clone(), second.clone()])?;
    let expected = fixture.node(Operator::DistributionAdd, &[prefix, third.clone()])?;
    equal(&mut fixture, &output(&result)?, &expected)?;
    replay(&fixture, &result)?;
    Ok(())
}

/// Integration by parts retains a symbolic negative action on the differentiated declared test.
/// ´test:integration:distribution-integration-by-parts-recipe´
#[test]
fn distribution_integration_by_parts_recipe() -> TestResult {
    let mut fixture = Fixture::new()?;
    let test = fixture.test()?;
    let (embedded, _) = fixture.embed(&fixture.base.x.clone())?;
    let derivative = fixture.node(
        Operator::DistributionDerivative { order: 1 },
        std::slice::from_ref(&embedded),
    )?;
    let request = fixture.base.request(
        Task::Pair {
            space: fixture.space.clone(),
            action_mode: ActionMode::Test,
        },
        &[derivative, test.clone()],
    )?;
    let result = fixture.base.proved(&request)?;
    let differentiated = fixture.node(Operator::TestDerivative { order: 1 }, &[test])?;
    let pair = fixture.node(Operator::Pair, &[embedded, differentiated])?;
    let expected = fixture.node(Operator::Neg, &[pair])?;
    assert_eq!(output(&result)?, expected);
    replay(&fixture, &result)?;
    Ok(())
}

/// Compare independently built sums through the native equality checker.
fn equal(fixture: &mut Fixture, left: &Handle, right: &Handle) -> TestResult {
    let request = fixture.base.request(
        Task::Equal {
            domain: fixture.base.domain.clone(),
            domain_contract: DomainContract::DefinedAndEqual,
        },
        &[left.clone(), right.clone()],
    )?;
    let result = fixture.base.proved(&request)?;
    assert_eq!(output(&result)?, *right);
    replay(fixture, &result)?;
    Ok(())
}
