// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed affine statistics refuse nonlinear denominators and invented independent voltage copies.

use super::blocks;
use crate::phasor_solve_testing::{budget, policy};
use crate::reading_testing::{Fixture, TestResult, matrix, meaning};
use nrz_bindings::{
    BinaryOp, Budget, ExponentPolicy, Registry,
    readings::{
        CompanionParameters, CompanionReadingKind, PhasorBlockCorrespondence, PhasorConvention,
        PhasorCovarianceLaw, PhasorSolveDefinition, PhysicalReading, ReadingError,
    },
};

/// Neither a varying denominator, a pole nor a second voltage declaration fits the shared affine law.
/// ´test:integration:shared-voltage-phasor-covariance-refuses-unproved-affine-laws´
#[test]
fn shared_voltage_phasor_covariance_refuses_unproved_affine_laws() -> TestResult {
    for (reactance, independent) in [(None, false), (Some(0), false), (Some(75), true)] {
        let (fixture, equations, targets) = prepare(reactance, independent)?;
        let result = PhasorCovarianceLaw::new(&fixture.cas, equations, targets, &mut budget()?);
        assert!(matches!(
            result,
            Err(ReadingError::Unsupported | ReadingError::Unrelated)
        ));
    }
    Ok(())
}

/// A diagonal covariance over repeated copies is not covariance over the one original voltage source.
/// ´test:integration:shared-voltage-phasor-refuses-independent-repeated-entry-covariance´
#[test]
fn shared_voltage_phasor_refuses_independent_repeated_entry_covariance() -> TestResult {
    let (fixture, equations, targets) = prepare(Some(75), false)?;
    let voltage = equations[0].voltage().reading();
    let wrong = fixture.symbol("independent_copies")?;
    assert!(
        CompanionParameters::new(
            &fixture.cas,
            &fixture.context,
            &wrong,
            CompanionReadingKind::Covariance,
            &[voltage.clone(), voltage.clone()],
            &mut budget()?
        )
        .is_err()
    );
    let law = PhasorCovarianceLaw::new(&fixture.cas, equations, targets, &mut budget()?)?;
    assert_eq!(law.map().inputs(), 1);
    assert_eq!(law.map().outputs(), 1);
    Ok(())
}

/// Both original equations and ordered scalar current selections are retained for exact refusal tests.
type ExactCase = (Fixture, [PhasorSolveDefinition; 2], [PhysicalReading; 2]);

/// Declare every original source and result before deriving either physical equation.
fn prepare(reactance: Option<i64>, independent: bool) -> TestResult<ExactCase> {
    let mut names = Vec::new();
    for prefix in ["below", "above", "voltage", "other"] {
        blocks::declare(&mut names, prefix, 1, false);
    }
    for prefix in ["first", "second"] {
        blocks::declare(&mut names, prefix, 2, true);
    }
    names.push(("independent_copies".to_owned(), matrix(2, 2)));
    let borrowed = names
        .iter()
        .map(|(name, sort)| (name.as_str(), sort.clone()))
        .collect::<Vec<_>>();
    let mut fixture = Fixture::with_limits(&borrowed, policy())?;
    let impedance = meaning("impedance", [2, 1, -3, -2, 0, 0, 0])?;
    let voltage = meaning("voltage", [2, 1, -3, -1, 0, 0, 0])?;
    let current = meaning("current", [0, 0, 0, 1, 0, 0, 0])?;
    let below = if let Some(value) = reactance {
        blocks::input(&mut fixture, "below", impedance.clone(), Some(value), 0)?
    } else {
        blocks::varying(&mut fixture, "below", impedance.clone(), 0)?
    };
    let above = blocks::input(&mut fixture, "above", impedance, Some(75), 1)?;
    let source = blocks::input(&mut fixture, "voltage", voltage.clone(), None, 2)?;
    let other = blocks::input(&mut fixture, "other", voltage, None, 3)?;
    let first = blocks::output(&mut fixture, "first", current.clone())?;
    let second = blocks::output(&mut fixture, "second", current)?;
    let mut resources = budget()?;
    let equations = [
        equation(&mut fixture, [&below, &first, &source], &mut resources)?,
        equation(
            &mut fixture,
            [&above, &second, if independent { &other } else { &source }],
            &mut resources,
        )?,
    ];
    let targets = [
        first.adapters[2].reading().clone(),
        second.adapters[2].reading().clone(),
    ];
    Ok((fixture, equations, targets))
}

/// The exact physical product registry is checked independently for each original equation.
fn equation(
    fixture: &mut Fixture,
    operands: [&blocks::Block; 3],
    budget: &mut Budget,
) -> TestResult<PhasorSolveDefinition> {
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_binary(
        BinaryOp::Multiply,
        operands[0].representation.reading().entries()[0]
            .descriptor()
            .kind()
            .clone(),
        operands[1].representation.reading().entries()[0]
            .descriptor()
            .kind()
            .clone(),
        operands[2].representation.reading().entries()[0]
            .descriptor()
            .kind()
            .clone(),
    )?;
    let law = PhasorBlockCorrespondence::prove(
        &mut fixture.cas,
        [
            operands[0].representation.clone(),
            operands[1].representation.clone(),
        ],
        PhasorConvention::PositiveTimePassive,
        budget,
    )?;
    Ok(PhasorSolveDefinition::new(
        &fixture.cas,
        law,
        operands[2].representation.clone(),
        &registry,
        budget,
    )?)
}
