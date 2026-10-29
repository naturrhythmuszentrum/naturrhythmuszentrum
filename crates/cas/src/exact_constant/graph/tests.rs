// SPDX-License-Identifier: AGPL-3.0-only
//! Literal baseline costs for repeated closed admission, normalization and identities.

use super::*;
use crate::budget::Limits;
use crate::model::{Branch, Constant};
use crate::number::Rational;

/// Independent preparation, normalization and identity costs in one source store.
struct Measurement {
    /// Exact original fixture.
    name: &'static str,
    /// Two admissions on a cumulative ledger.
    admission: u64,
    /// Two polynomial normalization passes in the source store.
    normalization: u64,
    /// Two multiplications of the retained constant by literal one.
    multiplication: u64,
    /// Two additions of literal zero to the retained constant.
    addition: u64,
}

/// One ordinary default ledger.
fn ledger() -> Result<Budget, Fault> {
    Budget::new(Limits::default())
}

/// Measure two operations on the same monotone ledger.
fn twice<T>(mut operation: impl FnMut(&mut Budget) -> Result<T, Fault>) -> Result<[u64; 2], Fault> {
    let mut budget = ledger()?;
    operation(&mut budget)?;
    let first = budget.usage().work;
    operation(&mut budget)?;
    Ok([first, budget.usage().work - first])
}

/// Closed original fixture in an independently owned source store.
fn fixture(name: &str) -> Result<(Cas, Handle), Fault> {
    let mut budget = ledger()?;
    let (mut cas, header) = empty(&mut budget)?;
    let operator = match name {
        "pi" | "two-pi" => Operator::Constant(Constant::Pi),
        "rational" => Operator::Rational(Rational::from_ratio(3, 7, &mut budget)?),
        "unit-scale" => Operator::Rational(Rational::one()),
        "zero-origin" => Operator::Rational(Rational::zero()),
        _ => return Err(Fault::invalid(InvalidCode::Reference, AT)),
    };
    let mut root = cas.generated(header, operator, real(), &[], &mut budget)?;
    if name == "two-pi" {
        let two = cas.generated(
            header,
            Operator::Rational(Rational::from_ratio(2, 1, &mut budget)?),
            real(),
            &[],
            &mut budget,
        )?;
        root = cas.generated(header, Operator::Mul, real(), &[two, root], &mut budget)?;
    }
    let root = cas.handle(root);
    Ok((cas, root))
}

/// Measured single-operation costs before any caller opts into reuse.
const BASELINES: [Measurement; 5] = [
    Measurement {
        name: "two-pi",
        admission: 27_938,
        normalization: 6_509,
        multiplication: 37_111,
        addition: 38_270,
    },
    Measurement {
        name: "pi",
        admission: 10_888,
        normalization: 3_343,
        multiplication: 18_633,
        addition: 19_792,
    },
    Measurement {
        name: "rational",
        admission: 6_741,
        normalization: 2_570,
        multiplication: 544,
        addition: 448,
    },
    Measurement {
        name: "unit-scale",
        admission: 6_453,
        normalization: 2_474,
        multiplication: 448,
        addition: 384,
    },
    Measurement {
        name: "zero-origin",
        admission: 3_956,
        normalization: 1_532,
        multiplication: 448,
        addition: 384,
    },
];

/// Pin both repetitions and the literal single-operation N/N−1 boundary.
fn pin<T>(
    mut operation: impl FnMut(&mut Budget) -> Result<T, Fault>,
    work: u64,
) -> Result<(), Fault> {
    assert_eq!(twice(&mut operation)?, [work, work]);
    operation(&mut Budget::new(Limits {
        work,
        ..Limits::default()
    })?)?;
    let fault = operation(&mut Budget::new(Limits {
        work: work - 1,
        ..Limits::default()
    })?)
    .err()
    .expect("literal Work boundary");
    assert!(matches!(
        fault.kind,
        crate::FaultKind::Exhausted {
            budget: BudgetKind::Work,
            ..
        }
    ));
    Ok(())
}

/// Ordinary admission, normalization and identities repeat their full original costs.
/// ´test:unit:constant-preparation-costs´
/// ´claim:cas:constant-preparation-costs´
#[test]
fn constant_preparation_costs() -> Result<(), Fault> {
    for expected in BASELINES {
        let (mut cas, root) = fixture(expected.name)?;
        let value = admit(&cas, &root, &mut ledger()?)?;
        let id = cas.checked(&root)?;
        pin(|budget| admit(&cas, &root, budget), expected.admission)?;
        pin(
            |budget| crate::polynomial::normalize(&mut cas, id, budget),
            expected.normalization,
        )?;
        pin(
            |budget| value.mul(&ExactConstant::one(), budget),
            expected.multiplication,
        )?;
        pin(
            |budget| value.add(&ExactConstant::zero(), budget),
            expected.addition,
        )?;
    }
    Ok(())
}

/// The positive LC square root now admits through exact normalization.
/// ´test:unit:constant-square-root-admission-costs´
/// ´claim:cas:constant-square-root-admission-costs´
#[test]
fn constant_square_root_admission_costs() -> Result<(), Fault> {
    let mut budget = ledger()?;
    let (mut cas, header) = empty(&mut budget)?;
    let input = cas.generated(
        header,
        Operator::Rational(Rational::from_ratio(1, 4_000_000, &mut budget)?),
        real(),
        &[],
        &mut budget,
    )?;
    let half = cas.generated(
        header,
        Operator::Rational(Rational::from_ratio(1, 2, &mut budget)?),
        real(),
        &[],
        &mut budget,
    )?;
    let root = cas.generated(
        header,
        Operator::GeneralPower(Branch::StrictReal),
        real(),
        &[input, half],
        &mut budget,
    )?;
    let mut measured = ledger()?;
    let result = admit(&cas, &cas.handle(root), &mut measured)?;
    assert_eq!(
        result,
        ExactConstant::from(Rational::from_ratio(1, 2_000, &mut ledger()?)?)
    );
    assert_eq!(measured.usage().work, 20_490);
    pin(|budget| admit(&cas, &cas.handle(root), budget), 20_490)?;
    Ok(())
}
