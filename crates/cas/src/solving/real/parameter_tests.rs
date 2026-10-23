// SPDX-License-Identifier: AGPL-3.0-only
//! Same-Real evaluation retains regional division guards and replacement provenance.

use crate::budget::{Budget, Limits};
use crate::domain_model::{AmbientDomain, RegionDraft, RegionExpr};
use crate::graph::{Draft, Handle, Reference};
use crate::model::{Field, Operator};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::elementary::{evaluate::Evaluation, value::Value};
use crate::testing::common::TestResult;
use crate::testing::solve_blind::{Fixture, symbolic_in_field};

/// Local evaluation reads the actual coefficient algebra and its exact current region.
fn evaluation(fixture: &Fixture) -> Evaluation<'_> {
    Evaluation {
        exclusions: &[],
        cas: &fixture.cas,
        engine: Engine {
            state: &fixture.cas.state,
            domain: &fixture.algebra,
        },
        region: &fixture.region,
        premises: &[],
        free: &[],
        bound: &[],
    }
}

/// The source declaration remains unchanged when its coefficient cell is restricted.
fn cell(fixture: &mut Fixture, parameter: &Handle, zero: bool) -> TestResult {
    let origin = fixture.integer(0, false)?;
    let predicate = Reference::Existing(fixture.equation(parameter, &origin)?);
    let base = Reference::Existing(fixture.region.clone());
    let expression = if zero {
        RegionExpr::GuardTrue { base, predicate }
    } else {
        RegionExpr::GuardFalse { base, predicate }
    };
    let draft = Draft::Region(RegionDraft {
        header: Reference::Existing(fixture.header.clone()),
        scope: Vec::new(),
        ambient: AmbientDomain::Product {
            coordinates: fixture.coefficients.clone(),
        },
        expression,
    });
    fixture.region = fixture
        .cas
        .admit(&[draft], &mut Budget::new(Limits::blind_cubic())?)?
        .into_iter()
        .next()
        .ok_or("Real parameter cell")?;
    Ok(())
}

/// Strict division proves its actual divisor nonzero before either cancellation is evaluated.
/// ´test:unit:real-parameter-division´
#[test]
fn real_parameter_division() -> TestResult {
    for zero in [true, false] {
        let (mut fixture, _) = symbolic_in_field(&["a"], &Field::Real)?;
        let parameter = fixture.free("a")?;
        let quotient = fixture.expression(
            Operator::Div,
            &[parameter.clone(), parameter.clone()],
            false,
        )?;
        let square = fixture.expression(
            Operator::IntegerPower(2),
            std::slice::from_ref(&parameter),
            false,
        )?;
        let cancelled = fixture.expression(Operator::Div, &[square, parameter.clone()], false)?;
        assert_ne!(quotient, cancelled);
        cell(&mut fixture, &parameter, zero)?;
        let evaluator = evaluation(&fixture);
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let first = evaluator.expression(quotient.logical_id(), true, &mut budget);
        let second = evaluator.expression(cancelled.logical_id(), true, &mut budget);
        assert_eq!(first.is_ok(), !zero);
        assert_eq!(second.is_ok(), !zero);
        if !zero {
            assert!(first?.equals(
                &Value::integer(1, &evaluator.engine, &mut budget)?,
                &evaluator.engine,
                &mut budget
            )?);
            let expected = evaluator.expression(parameter.logical_id(), true, &mut budget)?;
            assert!(second?.equals(&expected, &evaluator.engine, &mut budget)?);
        }
    }
    Ok(())
}

/// Both original zero cells are required; a nonzero constant cannot disappear into their ideal.
/// ´test:unit:real-parameter-joint-zeros´
#[test]
fn real_parameter_joint_zeros() -> TestResult {
    let (mut fixture, _) = symbolic_in_field(&["p", "q"], &Field::Real)?;
    let first = fixture.free("p")?;
    let second = fixture.free("q")?;
    let square = fixture.expression(
        Operator::IntegerPower(2),
        std::slice::from_ref(&first),
        false,
    )?;
    let cube = fixture.expression(
        Operator::IntegerPower(3),
        std::slice::from_ref(&second),
        false,
    )?;
    let sum = fixture.expression(Operator::Add, &[square, cube], false)?;
    for (parameter, expected) in [(&first, false), (&second, true)] {
        cell(&mut fixture, parameter, true)?;
        let evaluator = evaluation(&fixture);
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let value = evaluator.expression(sum.logical_id(), true, &mut budget)?;
        assert_eq!(evaluator.zero(&value, &mut budget).is_ok(), expected);
        let changed = value.add(
            &Value::integer(1, &evaluator.engine, &mut budget)?,
            &evaluator.engine,
            &mut budget,
        )?;
        assert!(!value.equals(&changed, &evaluator.engine, &mut budget)?);
        assert!(evaluator.zero(&changed, &mut budget).is_err());
    }
    Ok(())
}

/// Simultaneous replacement retains the actual map provenance of the supplied value.
/// ´test:unit:real-parameter-replacement-provenance´
#[test]
fn real_parameter_replacement_provenance() -> TestResult {
    let (mut fixture, _) = symbolic_in_field(&["a"], &Field::Real)?;
    let parameter = fixture.free("a")?;
    let frame = super::construction::Frame {
        inner: crate::solving::radicals::construction::Construction {
            header: fixture.header.clone(),
            context: fixture.context.clone(),
            region: fixture.region.clone(),
            steps: Vec::new(),
        },
    };
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let map = frame.mapping(
        &mut fixture.cas,
        &fixture.algebra,
        std::slice::from_ref(&parameter),
        &mut budget,
    )?;
    let evaluator = evaluation(&fixture);
    let mut replacement = Value::integer(7, &evaluator.engine, &mut budget)?;
    replacement.maps.push(map.logical_id());
    let free = [("a", replacement)];
    let evaluator = Evaluation {
        free: &free,
        ..evaluator
    };
    let actual = evaluator.expression(parameter.logical_id(), true, &mut budget)?;
    assert_eq!(actual.maps, [map.logical_id()]);
    assert!(actual.equals(
        &Value::integer(7, &evaluator.engine, &mut budget)?,
        &evaluator.engine,
        &mut budget
    )?);
    Ok(())
}

/// A later zero generator may expose a multiple of an earlier one, requiring another exact pass.
/// ´test:unit:real-parameter-revisits-generators´
#[test]
fn real_parameter_revisits_generators() -> TestResult {
    let (mut fixture, _) = symbolic_in_field(&["p", "q"], &Field::Real)?;
    let first = fixture.free("p")?;
    let second = fixture.free("q")?;
    let difference = fixture.expression(Operator::Sub, &[first.clone(), second.clone()], false)?;
    assert_ne!(first, difference);
    cell(&mut fixture, &difference, true)?;
    cell(&mut fixture, &second, true)?;
    let evaluator = evaluation(&fixture);
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let value = evaluator.expression(first.logical_id(), true, &mut budget)?;
    evaluator.zero(&value, &mut budget)?;
    let changed = value.add(
        &Value::integer(1, &evaluator.engine, &mut budget)?,
        &evaluator.engine,
        &mut budget,
    )?;
    assert!(evaluator.zero(&changed, &mut budget).is_err());
    Ok(())
}

/// A separately declared Complex symbol cannot occupy the same-named Real parameter slot.
/// ´test:unit:real-parameter-keeps-complex-sort´
#[test]
fn real_parameter_keeps_complex_sort() -> TestResult {
    use crate::model::{AtomicSort, Branch, Declaration, Environment, Sort};
    let (mut fixture, _) = symbolic_in_field(&["a"], &Field::Real)?;
    let parameter = fixture.free("a")?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let sort = Sort::Value(AtomicSort::Scalar(Field::Complex));
    let header = super::construction::admit(
        &mut fixture.cas,
        &Draft::Header(crate::domain_model::HeaderDraft::Core(Environment {
            field: Field::Complex,
            branch: Branch::PrincipalComplex,
            declarations: vec![Declaration {
                key: "a".into(),
                display_name: "a".into(),
                sort: sort.clone(),
            }],
        })),
        &mut budget,
    )?;
    let complex = super::construction::admit(
        &mut fixture.cas,
        &Draft::Expression(crate::graph::ExpressionDraft {
            header: Reference::Existing(header),
            scope: Vec::new(),
            space: None,
            binding: None,
            operator: Operator::Free("a".into()),
            operands: Vec::new(),
            sort: sort.clone(),
        }),
        &mut budget,
    )?;
    assert_eq!(
        fixture.cas.state.expression(complex.logical_id())?.sort,
        sort
    );
    assert_ne!(parameter, complex);
    let evaluator = evaluation(&fixture);
    assert!(
        evaluator
            .parameter(parameter.logical_id(), &mut budget)?
            .is_some()
    );
    assert!(
        evaluator
            .parameter(complex.logical_id(), &mut budget)?
            .is_none()
    );
    Ok(())
}
