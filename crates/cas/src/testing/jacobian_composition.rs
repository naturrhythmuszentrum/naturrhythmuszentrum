// SPDX-License-Identifier: AGPL-3.0-only
//! Independent exact chain-rule fixtures with shared source coordinates.

use super::common::{TestResult, real};
use super::jacobians::{integer, rectangle};
use super::map_inverses::{constant_map, fixture, ledger};
use super::readings::{Fixture, matrix};
use crate::test_api::Handle;
use crate::test_api::jacobians::{JacobianReadings, SourceCoordinates};
use crate::test_api::syntax::Operator;

/// Inputs and independently supplied derivative coefficients for one exact reading family.
pub(crate) struct CompositionCase {
    /// Original declaration graph and mathematical context.
    pub(crate) fixture: Fixture,
    /// Original expression in reading and source variables.
    pub(crate) original: Handle,
    /// Ordered source/fixed contract.
    pub(crate) coordinates: SourceCoordinates,
    /// Exact simultaneous reading map.
    pub(crate) readings: JacobianReadings,
    /// Independent derivative matrix.
    pub(crate) oracle: Handle,
}

/// Hand-computed linear derivative of a quadratic in two source coordinates.
fn linear(case: &mut Fixture, coefficients: [i64; 3]) -> TestResult<Handle> {
    let x = case.symbol("x")?;
    let y = case.symbol("y")?;
    let first = integer(case, coefficients[0])?;
    let second = integer(case, coefficients[1])?;
    let offset = integer(case, coefficients[2])?;
    let left = case.term(Operator::Mul, real(), &[first, x])?;
    let right = case.term(Operator::Mul, real(), &[second, y])?;
    let sum = case.term(Operator::Add, real(), &[left, right])?;
    case.term(Operator::Add, real(), &[sum, offset])
}

/// `v*w+x` gives both indirect reading derivatives and a direct source contribution.
pub(crate) fn nonlinear(kind: u32) -> TestResult<CompositionCase> {
    let mut case = fixture(&["x", "y", "v", "w", "r"])?;
    let (table, values): ([i64; 4], [&str; 4]) = match kind {
        0 => ([5, 0, 0, 10], ["5", "0", "0", "10"]),
        1 => ([1, 2, 0, 1], ["1", "2", "0", "1"]),
        _ => ([2, 3, 5, 7], ["2", "3", "5", "7"]),
    };
    let map = constant_map(&mut case, 2, &values, &["11", "13"])?;
    let voltage = case.symbol("v")?;
    let current = case.symbol("w")?;
    let source = case.symbol("x")?;
    let product = case.term(Operator::Mul, real(), &[voltage, current])?;
    let original = case.term(Operator::Add, real(), &[product, source])?;
    let [a, b, c, d] = table;
    let mixed = a * d + b * c;
    let first = linear(&mut case, [2 * a * c, mixed, 13 * a + 11 * c + 1])?;
    let second = linear(&mut case, [mixed, 2 * b * d, 13 * b + 11 * d])?;
    let oracle = case.term(
        Operator::Matrix {
            rows: 1,
            columns: 2,
        },
        matrix(1, 2),
        &[first, second],
    )?;
    Ok(CompositionCase {
        fixture: case,
        original,
        coordinates: SourceCoordinates {
            sources: vec!["x".into(), "y".into()],
            fixed: vec!["r".into()],
        },
        readings: JacobianReadings {
            variables: vec!["v".into(), "w".into()],
            map,
        },
        oracle,
    })
}

/// A component and a two-entry state read the very same first source coordinate.
pub(crate) fn overlapping() -> TestResult<CompositionCase> {
    let mut case = fixture(&["u0", "u1", "x0", "x1", "v"])?;
    let mut coefficients = Vec::new();
    for value in [1, 0, 0, 1, 1, 0] {
        coefficients.push(integer(&mut case, value)?);
    }
    let zero = integer(&mut case, 0)?;
    let map = case.cas.exact_reading_map(
        &case.context,
        (2, 3),
        &coefficients,
        &[zero.clone(), zero.clone(), zero],
        &mut ledger()?,
    )?;
    let v = case.symbol("v")?;
    let state_first = case.symbol("x0")?;
    let state_second = case.symbol("x1")?;
    let product = case.term(Operator::Mul, real(), &[v, state_first])?;
    let original = case.term(Operator::Add, real(), &[product, state_second])?;
    let first = case.symbol("u0")?;
    let two = integer(&mut case, 2)?;
    let derivative = case.term(Operator::Mul, real(), &[two, first])?;
    let one = integer(&mut case, 1)?;
    let oracle = case.term(
        Operator::Matrix {
            rows: 1,
            columns: 2,
        },
        matrix(1, 2),
        &[derivative, one],
    )?;
    Ok(CompositionCase {
        fixture: case,
        original,
        coordinates: SourceCoordinates {
            sources: vec!["u0".into(), "u1".into()],
            fixed: Vec::new(),
        },
        readings: JacobianReadings {
            variables: vec!["x0".into(), "x1".into(), "v".into()],
            map,
        },
        oracle,
    })
}

/// A source-dependent coefficient exercises the inner map derivative, not just its literal table.
pub(crate) fn varying_coefficient() -> TestResult<CompositionCase> {
    let mut case = fixture(&["x", "v"])?;
    let x = case.symbol("x")?;
    let zero = integer(&mut case, 0)?;
    let map = case.cas.exact_reading_map(
        &case.context,
        (1, 1),
        std::slice::from_ref(&x),
        &[zero],
        &mut ledger()?,
    )?;
    let original = case.symbol("v")?;
    let two = integer(&mut case, 2)?;
    let derivative = case.term(Operator::Mul, real(), &[two, x])?;
    let oracle = case.term(
        Operator::Matrix {
            rows: 1,
            columns: 1,
        },
        matrix(1, 1),
        &[derivative],
    )?;
    Ok(CompositionCase {
        fixture: case,
        original,
        coordinates: SourceCoordinates {
            sources: vec!["x".into()],
            fixed: Vec::new(),
        },
        readings: JacobianReadings {
            variables: vec!["v".into()],
            map,
        },
        oracle,
    })
}

/// An exact reciprocal reading is defined on the positive source family after substitution.
pub(crate) fn reciprocal() -> TestResult<CompositionCase> {
    let mut case = fixture(&["x", "v"])?;
    super::jacobians::positive_family(&mut case, &["x"])?;
    let map = constant_map(&mut case, 1, &["2"], &["0"])?;
    let one = integer(&mut case, 1)?;
    let v = case.symbol("v")?;
    let original = case.term(Operator::Div, real(), &[one, v])?;
    let derivative = super::jacobians::reciprocal(&mut case, "-1/2", "x", 2)?;
    let oracle = case.term(
        Operator::Matrix {
            rows: 1,
            columns: 1,
        },
        matrix(1, 1),
        &[derivative],
    )?;
    Ok(CompositionCase {
        fixture: case,
        original,
        coordinates: SourceCoordinates {
            sources: vec!["x".into()],
            fixed: Vec::new(),
        },
        readings: JacobianReadings {
            variables: vec!["v".into()],
            map,
        },
        oracle,
    })
}

/// Empty readings and empty sources still retain their complete original map.
pub(crate) fn empty() -> TestResult<CompositionCase> {
    let mut case = fixture(&[])?;
    let original = integer(&mut case, 1)?;
    let map = constant_map(&mut case, 0, &[], &[])?;
    let oracle = rectangle(&mut case, 1, 0, &[])?;
    Ok(CompositionCase {
        fixture: case,
        original,
        coordinates: SourceCoordinates::default(),
        readings: JacobianReadings {
            variables: Vec::new(),
            map,
        },
        oracle,
    })
}
