// SPDX-License-Identifier: AGPL-3.0-only
//! Point, jet and derivative-map laws with independently specified expected values.

use crate::testing::boundary::{derivative_fact, facts, literal, named, point, quadratic, whole};
use crate::testing::budgets::derivation_budget as budget;
use crate::testing::cases::Fixture;
use crate::testing::common::TestResult;
use nrz_cas::domain::Side;
use nrz_cas::proof::{NonexistenceReason, Ownership, Statement};
use nrz_cas::request::{BoundaryFact, Outcome, ResultPayload, Task};
use nrz_cas::syntax::{CaseName, NamedCase};

/// Absolute value and positive part have unequal left/right slopes at their owned zero.
/// ´test:integration:abs-ramp-boundary-nonexistence´
#[test]
fn abs_ramp_boundary_nonexistence() -> TestResult {
    for (name, slopes) in [(CaseName::Abs, ["-1", "1"]), (CaseName::Ramp, ["0", "1"])] {
        let mut fixture = Fixture::new()?;
        let function = named(&mut fixture, name)?;
        let Outcome::Nonexistent { result, .. } = point(&mut fixture, &function, 1)? else {
            return Err("missing witnessed nonexistence".into());
        };
        assert!(
            matches!(derivative_fact(&result, 1)?, Statement::DerivativeDoesNotExist(value)
            if value.function == function && value.reason == NonexistenceReason::UnequalSlopes)
        );
        for fact in facts(&result)? {
            if let Statement::Jet { side, values, .. } = &fact.judgment().statement {
                let expected = if *side == Side::Below {
                    slopes.first()
                } else {
                    slopes.get(1)
                }
                .ok_or("slope")?;
                literal(&mut fixture, values.get(1).ok_or("jet slope")?, expected)?;
                literal(&mut fixture, values.first().ok_or("jet value")?, "0")?;
            }
        }
    }
    Ok(())
}

/// J fails continuity although both derivative traces are zero.
/// ´test:integration:jump-owned-value-precedes-slope-test´
#[test]
fn jump_owned_value_precedes_slope_test() -> TestResult {
    let mut fixture = Fixture::new()?;
    let function = quadratic(&mut fixture, true)?;
    let Outcome::Nonexistent { result, .. } = point(&mut fixture, &function, 1)? else {
        return Err("J derivative was not rejected".into());
    };
    assert!(
        matches!(derivative_fact(&result, 1)?, Statement::DerivativeDoesNotExist(value)
        if value.reason == NonexistenceReason::Discontinuous(Side::Below))
    );
    for fact in facts(&result)? {
        match &fact.judgment().statement {
            Statement::Ownership {
                function: owner,
                value: Ownership::Present(value),
                ..
            } if *owner == function => literal(&mut fixture, value, "1")?,
            Statement::Jet { side, values, .. } => {
                literal(
                    &mut fixture,
                    values.first().ok_or("trace")?,
                    if *side == Side::Below { "0" } else { "1" },
                )?;
                literal(&mut fixture, values.get(1).ok_or("slope")?, "0")?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Q has a true first derivative at zero but unequal slopes of that actual derivative.
/// ´test:integration:quadratic-first-and-second-point-derivatives´
#[test]
fn quadratic_first_and_second_point_derivatives() -> TestResult {
    let mut fixture = Fixture::new()?;
    let function = quadratic(&mut fixture, false)?;
    let Outcome::Proved { result, .. } = point(&mut fixture, &function, 1)? else {
        return Err("Q first derivative missing".into());
    };
    let Statement::DerivativeAt(value) = derivative_fact(&result, 1)? else {
        return Err("point derivative expected".into());
    };
    literal(&mut fixture, &value.value, "0")?;
    let result = match point(&mut fixture, &function, 2)? {
        Outcome::Nonexistent { result, .. } => result,
        other => return Err(format!("Q second derivative obstruction missing: {other:?}").into()),
    };
    assert!(
        matches!(derivative_fact(&result, 2)?, Statement::DerivativeDoesNotExist(value)
        if value.function == function && value.reason == NonexistenceReason::UnequalSlopes)
    );
    let mut slopes = Vec::new();
    for fact in facts(&result)? {
        if let Statement::Jet {
            function: current,
            side,
            values,
            ..
        } = &fact.judgment().statement
            && *current != function
        {
            literal(&mut fixture, values.first().ok_or("derivative trace")?, "0")?;
            literal(
                &mut fixture,
                values.get(1).ok_or("second trace")?,
                if *side == Side::Below { "2" } else { "4" },
            )?;
            slopes.push(*side);
        }
    }
    assert_eq!(slopes, [Side::Below, Side::Above]);
    Ok(())
}

/// Mixed whole-domain maps retain both correct open derivatives and the point obstruction.
/// ´test:integration:piecewise-partial-derivative-map´
#[test]
fn piecewise_partial_derivative_map() -> TestResult {
    for (name, expected) in [(CaseName::Abs, ["-1", "1"]), (CaseName::Ramp, ["0", "1"])] {
        let mut fixture = Fixture::new()?;
        let function = named(&mut fixture, name)?;
        let Outcome::Partial { result, .. } = whole(&mut fixture, &function, 1)? else {
            return Err("partial derivative map expected".into());
        };
        let Some(ResultPayload::DerivativeMap(map)) = result.payload.as_deref() else {
            return Err("map payload missing".into());
        };
        assert_eq!(map.function(), &function);
        assert!(map.complete().is_none());
        assert!(map.defined().is_some());
        assert_eq!(map.pending(), []);
        let Statement::BoundaryPartition(partition) = &map.partition().judgment().statement else {
            return Err("partition".into());
        };
        let mut index = 0;
        for fact in map.facts() {
            if let Statement::CaseDerivative(value) = &fact.judgment().statement
                && partition.cells.contains(&value.domain)
            {
                literal(
                    &mut fixture,
                    &value.derivative,
                    expected.get(index).ok_or("extra derivative cell")?,
                )?;
                index += 1;
            }
        }
        assert_eq!(index, 2);
        assert!(matches!(
            derivative_fact(&result, 1)?,
            Statement::DerivativeDoesNotExist(_)
        ));
    }
    Ok(())
}

/// The jump recipe produces right-minus-left independently of the step's point height.
/// ´test:integration:named-sign-step-and-jump-recipe´
#[test]
fn named_sign_step_and_jump_recipe() -> TestResult {
    let mut fixture = Fixture::new()?;
    let sign = named(&mut fixture, CaseName::Sign)?;
    let height = nrz_cas::Rational::from_ratio(3, 2, &mut budget()?)?;
    let step = fixture.cas.named_case(
        &fixture.domain,
        &NamedCase {
            name: CaseName::Step,
            coordinate: "x".into(),
            parameter: Some(height),
        },
        &mut budget()?,
    )?;
    for (function, expected, owned) in [(sign, "2", "0"), (step, "1", "3/2")] {
        let location = fixture.boundary(&fixture.zero.clone())?;
        let request = fixture.request(
            Task::Boundary {
                coordinate: "x".into(),
                point: location,
                requested_facts: vec![BoundaryFact::Ownership, BoundaryFact::Jump { order: 0 }],
            },
            &[function],
        )?;
        let result = fixture.proved(&request)?;
        let Statement::Jump {
            value, order: 0, ..
        } = &result.witness.judgment().statement
        else {
            return Err("jump missing".into());
        };
        literal(&mut fixture, value, expected)?;
        let owner = facts(&result)?
            .iter()
            .find_map(|proof| match &proof.judgment().statement {
                Statement::Ownership {
                    value: Ownership::Present(value),
                    ..
                } => Some(value),
                _ => None,
            })
            .ok_or("ownership missing")?;
        literal(&mut fixture, owner, owned)?;
    }
    Ok(())
}

/// Independent formulas give J's two open derivatives as twice x, and Q's as twice/four times x.
/// ´test:integration:quadratic-open-cell-derivative-formulas´
#[test]
fn quadratic_open_cell_derivative_formulas() -> TestResult {
    for jump in [true, false] {
        let mut fixture = Fixture::new()?;
        let function = quadratic(&mut fixture, jump)?;
        let result = match whole(&mut fixture, &function, 1)? {
            Outcome::Partial { result, .. } if jump => result,
            Outcome::Proved { result, .. } if !jump => result,
            other => return Err(format!("unexpected quadratic map: {other:?}").into()),
        };
        let Some(ResultPayload::DerivativeMap(map)) = result.payload.as_deref() else {
            return Err("map".into());
        };
        let Statement::BoundaryPartition(partition) = &map.partition().judgment().statement else {
            return Err("partition".into());
        };
        let mut index = 0;
        for fact in map.facts() {
            if let Statement::CaseDerivative(value) = &fact.judgment().statement
                && partition.cells.contains(&value.domain)
            {
                let coefficient = fixture.number(if index == 0 || jump { "2" } else { "4" })?;
                let expected = fixture.term(
                    nrz_cas::syntax::Operator::Mul,
                    &[coefficient, fixture.x.clone()],
                )?;
                assert_eq!(value.derivative, expected);
                index += 1;
            }
        }
        assert_eq!(index, 2);
    }
    Ok(())
}
