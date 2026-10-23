// SPDX-License-Identifier: AGPL-3.0-only
//! Exact original expressions and independent Jacobian coefficient oracles.

use super::common::{TestResult, real};
use super::map_inverses::{fixture, ledger};
use super::readings::{Fixture, matrix};
use crate::test_api::jacobians::SourceCoordinates;
use crate::test_api::proof::{Judgment, Statement};
use crate::test_api::syntax::Operator;
use crate::test_api::{Handle, Rational};

/// Build a literal coefficient independently of differentiation.
pub(crate) fn integer(case: &mut Fixture, value: i64) -> TestResult<Handle> {
    case.term(
        Operator::Rational(Rational::from_ratio(value, 1, &mut ledger()?)?),
        real(),
        &[],
    )
}

/// Explicit source order, with one fixed reference declaration.
pub(crate) fn coordinates() -> SourceCoordinates {
    SourceCoordinates {
        sources: vec!["x".into(), "y".into()],
        fixed: vec!["r".into()],
    }
}

/// Scalar product and its hand-written ordered partials `(y,x)`.
pub(crate) fn product() -> TestResult<(Fixture, Handle, Handle)> {
    let mut case = fixture(&["x", "y", "r"])?;
    let x = case.symbol("x")?;
    let y = case.symbol("y")?;
    let original = case.term(Operator::Mul, real(), &[x.clone(), y.clone()])?;
    let oracle = case.term(
        Operator::Matrix {
            rows: 1,
            columns: 2,
        },
        matrix(1, 2),
        &[y, x],
    )?;
    Ok((case, original, oracle))
}

/// Build the full expected scalar-output Jacobian without inspecting producer claims.
pub(crate) fn expected(
    case: &mut Fixture,
    original: &Handle,
    oracle: &Handle,
) -> TestResult<Judgment> {
    let input = case.term(
        Operator::Matrix {
            rows: 1,
            columns: 1,
        },
        matrix(1, 1),
        std::slice::from_ref(original),
    )?;
    Ok(Judgment {
        context: case.context.clone(),
        layer: crate::test_api::domain::Layer::Pointwise,
        space: None,
        statement: Statement::Jacobian {
            input,
            output: oracle.clone(),
            variables: coordinates().sources,
            domain: case.cas.admissible_region(&case.context)?,
        },
        residuals: Vec::new(),
    })
}

/// Add positive exact parameter assumptions without specializing either coordinate.
pub(crate) fn positive_family(case: &mut Fixture, keys: &[&str]) -> TestResult {
    use crate::test_api::Reference;
    use crate::test_api::domain::{AssumptionDraft, ContextDraft};
    use crate::test_api::syntax::{Draft, PredicateKind};
    let header = case.cas.context_header(&case.context)?;
    let domain = case.cas.admissible_region(&case.context)?;
    let mut assumptions = Vec::new();
    for key in keys {
        let term = case.symbol(key)?;
        let predicate = super::builders::admit(
            &mut case.cas,
            Draft::Condition {
                kind: PredicateKind::Positive,
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                arguments: vec![Reference::Existing(term)],
            },
            &mut ledger()?,
        )?;
        assumptions.push(AssumptionDraft {
            key: (*key).into(),
            condition: Reference::Existing(predicate),
        });
    }
    case.context = super::builders::admit(
        &mut case.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header),
            admissible: Reference::Existing(domain),
            assumptions,
        }),
        &mut ledger()?,
    )?;
    Ok(())
}

/// Exact literal rectangle in native row-major syntax.
pub(crate) fn rectangle(
    case: &mut Fixture,
    rows: u32,
    columns: u32,
    values: &[&str],
) -> TestResult<Handle> {
    let mut entries = Vec::new();
    for text in values {
        entries.push(case.term(
            Operator::Rational(Rational::parse(text, &mut ledger()?)?),
            real(),
            &[],
        )?);
    }
    case.term(
        Operator::Matrix { rows, columns },
        matrix(rows, columns),
        &entries,
    )
}

/// Independent LC derivative oracle in logical output order `(B00,B10,B01,B11)`.
pub(crate) fn lc(guarded: bool) -> TestResult<(Fixture, Handle, Handle)> {
    let mut case = fixture(&["L", "C"])?;
    if guarded {
        positive_family(&mut case, &["L", "C"])?;
    }
    let zero = integer(&mut case, 0)?;
    let upper = reciprocal(&mut case, "1", "C", 1)?;
    let lower = reciprocal(&mut case, "-1", "L", 1)?;
    let physical = case.term(
        Operator::Matrix {
            rows: 2,
            columns: 2,
        },
        matrix(2, 2),
        &[zero.clone(), upper, lower, zero.clone()],
    )?;
    let state = rectangle(&mut case, 2, 2, &["5", "0", "0", "1/10"])?;
    let inverse = rectangle(&mut case, 2, 2, &["1/5", "0", "0", "10"])?;
    let time = rectangle(&mut case, 2, 2, &["1/2000", "0", "0", "1/2000"])?;
    let right = case.term(Operator::MatrixMultiply, matrix(2, 2), &[physical, state])?;
    let similar = case.term(Operator::MatrixMultiply, matrix(2, 2), &[inverse, right])?;
    let original = case.term(Operator::MatrixMultiply, matrix(2, 2), &[time, similar])?;
    let inductance = reciprocal(&mut case, "1/40", "L", 2)?;
    let capacitance = reciprocal(&mut case, "-1/100000", "C", 2)?;
    let oracle = case.term(
        Operator::Matrix {
            rows: 4,
            columns: 2,
        },
        matrix(4, 2),
        &[
            zero.clone(),
            zero.clone(),
            inductance,
            zero.clone(),
            zero.clone(),
            capacitance,
            zero.clone(),
            zero,
        ],
    )?;
    Ok((case, original, oracle))
}

/// Direct rational formula independent of any symbolic derivative producer.
pub(crate) fn reciprocal(
    case: &mut Fixture,
    coefficient: &str,
    name: &str,
    power: i64,
) -> TestResult<Handle> {
    let numerator = case.term(
        Operator::Rational(Rational::parse(coefficient, &mut ledger()?)?),
        real(),
        &[],
    )?;
    let symbol = case.symbol(name)?;
    let denominator = if power == 1 {
        symbol
    } else {
        case.term(Operator::IntegerPower(power), real(), &[symbol])?
    };
    case.term(Operator::Div, real(), &[numerator, denominator])
}

/// LC source order is independent of canonical declaration sorting.
pub(crate) fn lc_coordinates() -> SourceCoordinates {
    SourceCoordinates {
        sources: vec!["L".into(), "C".into()],
        fixed: Vec::new(),
    }
}
