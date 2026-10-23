// SPDX-License-Identifier: AGPL-3.0-only
//! Reading substitutions refuse cycles, extra directions and invalid original maps.

use crate::testing::common::{TestResult, real};
use crate::testing::jacobian_composition::{CompositionCase, nonlinear, overlapping};
use crate::testing::jacobians::integer;
use crate::testing::map_inverses::ledger;
use nrz_cas::syntax::Operator;
use nrz_cas::{Fault, FaultKind, InvalidCode};

fn refuses(case: &mut CompositionCase, code: InvalidCode) -> TestResult {
    let before = case.fixture.cas.counts();
    let result = case.fixture.cas.compose_reading_jacobian(
        &case.fixture.context,
        &case.original,
        &case.coordinates,
        &case.readings,
        &mut ledger()?,
    );
    assert!(
        matches!(result, Err(Fault { kind: FaultKind::Invalid(actual), .. }) if actual == code)
    );
    assert_eq!(case.fixture.cas.counts(), before);
    Ok(())
}

/// A reading declaration never becomes a second source direction or a fixed derivative direction.
/// ´test:integration:reading-jacobian-direction-contract-refusals´
#[test]
fn reading_jacobian_direction_contract_refusals() -> TestResult {
    let mut case = nonlinear(0)?;
    case.readings.variables[0] = "x".into();
    refuses(&mut case, InvalidCode::JacobianDuplicateSource)?;
    let mut case = nonlinear(0)?;
    case.coordinates.fixed.push("v".into());
    refuses(&mut case, InvalidCode::JacobianFixedSource)?;
    let mut case = overlapping()?;
    case.readings.variables.pop();
    refuses(&mut case, InvalidCode::ReadingShape)
}

/// Map coefficients cannot depend on the simultaneous reading symbols themselves.
/// ´test:integration:reading-jacobian-cyclic-coefficients-refuse´
#[test]
fn reading_jacobian_cyclic_coefficients_refuse() -> TestResult {
    let mut case = nonlinear(0)?;
    let reading = case.fixture.symbol("v")?;
    let zero = integer(&mut case.fixture, 0)?;
    case.readings.map = case.fixture.cas.exact_reading_map(
        &case.fixture.context,
        (2, 2),
        &[reading, zero.clone(), zero.clone(), zero.clone()],
        &[zero.clone(), zero],
        &mut ledger()?,
    )?;
    refuses(&mut case, InvalidCode::JacobianReadingCycle)
}

/// An unused reading row retains its invalid coefficient beneath multiplication by zero.
/// ´test:integration:reading-jacobian-unused-map-domain-refuses´
#[test]
fn reading_jacobian_unused_map_domain_refuses() -> TestResult {
    let mut case = nonlinear(0)?;
    case.original = case.fixture.symbol("v")?;
    let zero = integer(&mut case.fixture, 0)?;
    let one = integer(&mut case.fixture, 1)?;
    let invalid = case
        .fixture
        .term(Operator::Div, real(), &[one.clone(), zero.clone()])?;
    let hidden = case
        .fixture
        .term(Operator::Mul, real(), &[zero.clone(), invalid])?;
    case.readings.map = case.fixture.cas.exact_reading_map(
        &case.fixture.context,
        (2, 2),
        &[one, zero.clone(), hidden, zero.clone()],
        &[zero.clone(), zero],
        &mut ledger()?,
    )?;
    refuses(&mut case, InvalidCode::RefutedDomain)
}

/// A fixed symbol contributes its value as a coefficient and contributes no extra column.
/// ´test:integration:source-jacobian-fixed-coefficient-remains-exact´
#[test]
fn source_jacobian_fixed_coefficient_remains_exact() -> TestResult {
    let (mut case, _, _) = crate::testing::jacobians::product()?;
    let reference = case.symbol("r")?;
    let source = case.symbol("x")?;
    let original = case.term(Operator::Mul, real(), &[reference.clone(), source])?;
    let key = case.cas.source_jacobian_key(
        &case.context,
        &original,
        &crate::testing::jacobians::coordinates(),
        &mut ledger()?,
    )?;
    let derivative = case.cas.source_jacobian(&key, &mut ledger()?)?;
    let zero = integer(&mut case, 0)?;
    let oracle = case.term(
        Operator::Matrix {
            rows: 1,
            columns: 2,
        },
        crate::testing::readings::matrix(1, 2),
        &[reference, zero],
    )?;
    case.cas
        .prove_reading_equality(&case.context, derivative.matrix(), &oracle, &mut ledger()?)?;
    Ok(())
}
