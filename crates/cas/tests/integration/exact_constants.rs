// SPDX-License-Identifier: AGPL-3.0-only
//! Public closed-constant admission, exact cancellation and cumulative proof budget boundaries.

use nrz_cas::domain::HeaderDraft;
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::syntax::{
    AtomicSort, Constant, Declaration, Draft, Environment, ExpressionDraft, Field, Operator, Sort,
};
use nrz_cas::{Cas, ExactConstant, Fault, FaultKind, Handle, InvalidCode, Rational, Reference};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn ledger() -> Result<Budget, Fault> {
    Budget::new(Limits::default())
}

fn rational(numerator: i64, denominator: u64) -> TestResult<ExactConstant> {
    Ok(Rational::from_ratio(numerator, denominator, &mut ledger()?)?.into())
}

fn original(operators: &[(Operator, Vec<u32>)], declared: bool) -> TestResult<(Cas, Handle)> {
    let declarations = if declared {
        vec![Declaration {
            key: "x".into(),
            display_name: "pi".into(),
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
        }]
    } else {
        Vec::new()
    };
    let mut drafts = vec![Draft::Header(HeaderDraft::Core(Environment {
        declarations,
        ..Environment::default()
    }))];
    for (operator, operands) in operators {
        drafts.push(Draft::Expression(ExpressionDraft {
            operator: operator.clone(),
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            header: Reference::Local(0),
            scope: Vec::new(),
            binding: None,
            space: None,
            operands: operands
                .iter()
                .map(|index| Reference::Local(index + 1))
                .collect(),
        }));
    }
    let mut cas = Cas::new(Limits::default())?;
    let root = cas
        .admit(&drafts, &mut ledger()?)?
        .pop()
        .ok_or("missing root")?;
    Ok((cas, root))
}

/// The exact typed constant cancels to an independently specified Rational, without decimal pi.
/// ´test:integration:constant-products-cancel-exactly´
#[test]
fn constant_products_cancel_exactly() -> TestResult {
    let pi = ExactConstant::named(Constant::Pi, &mut ledger()?)?;
    let degree = pi.div(&rational(180, 1)?, &mut ledger()?)?;
    let turn = rational(2, 1)?.mul(&pi, &mut ledger()?)?;
    let quotient = degree.div(&turn, &mut ledger()?)?;
    assert_eq!(quotient.as_rational(), rational(1, 360)?.as_rational());
    assert_eq!(degree.positive().ok_or("positive degree")?.value(), &degree);
    assert_eq!(turn.div(&degree, &mut ledger()?)?, rational(360, 1)?);
    assert_eq!(
        pi.pow(-2, &mut ledger()?)?
            .mul(&pi.pow(2, &mut ledger()?)?, &mut ledger()?)?,
        rational(1, 1)?
    );
    assert_ne!(pi.as_rational(), Some(&Rational::one()));
    let sum = pi.add(&ExactConstant::one(), &mut ledger()?)?;
    assert_eq!(
        sum.div(&sum, &mut ledger()?)?.as_rational(),
        Some(&Rational::one())
    );
    Ok(())
}

/// Closed sign rules prove positive sums but never manufacture a witness for an unresolved sign.
/// ´test:integration:constant-positivity-is-checked´
#[test]
fn constant_positivity_is_checked() -> TestResult {
    let pi = ExactConstant::named(Constant::Pi, &mut ledger()?)?;
    let e = ExactConstant::named(Constant::E, &mut ledger()?)?;
    assert!(pi.add(&e, &mut ledger()?)?.positive().is_some());
    assert_eq!(pi.sub(&rational(1, 1)?, &mut ledger()?)?.positive(), None);
    assert_eq!(pi.negated(&mut ledger()?)?.positive(), None);
    assert_eq!(pi.sub(&pi, &mut ledger()?)?, ExactConstant::zero());
    assert_eq!(ExactConstant::zero().positive(), None);
    assert_eq!(rational(-1, 1)?.positive(), None);
    assert!(
        pi.negated(&mut ledger()?)?
            .pow(2, &mut ledger()?)?
            .positive()
            .is_some()
    );
    Ok(())
}

/// Original undefined and open children remain refusals under a multiplying zero.
/// ´test:integration:constant-original-domains-and-symbols-are-retained´
#[test]
fn constant_original_domains_and_symbols_are_retained() -> TestResult {
    let (cas, root) = original(
        &[
            (Operator::Rational(Rational::zero()), vec![]),
            (Operator::Rational(Rational::one()), vec![]),
            (Operator::Div, vec![1, 0]),
            (Operator::Mul, vec![0, 2]),
        ],
        false,
    )?;
    assert!(matches!(
        ExactConstant::from_expression(&cas, &root, &mut ledger()?),
        Err(Fault {
            kind: FaultKind::Invalid(InvalidCode::RefutedDomain),
            ..
        })
    ));
    let (cas, root) = original(
        &[
            (Operator::Free("x".into()), vec![]),
            (Operator::Rational(Rational::zero()), vec![]),
            (Operator::Mul, vec![0, 1]),
        ],
        true,
    )?;
    assert!(matches!(
        ExactConstant::from_expression(&cas, &root, &mut ledger()?),
        Err(Fault {
            kind: FaultKind::Invalid(InvalidCode::UnsupportedFragment),
            ..
        })
    ));
    assert!(original(&[(Operator::Free("x".into()), vec![])], false).is_err());
    Ok(())
}

/// Equivalent normalized structures keep equality across independent stores and original syntax.
/// ´test:integration:constant-graph-ownership-and-identity´
#[test]
fn constant_graph_ownership_and_identity() -> TestResult {
    let (cas, root) = original(
        &[
            (Operator::Constant(Constant::Pi), vec![]),
            (Operator::Rational(Rational::one()), vec![]),
            (Operator::Mul, vec![1, 0]),
        ],
        false,
    )?;
    let value = ExactConstant::from_expression(&cas, &root, &mut ledger()?)?;
    drop(cas);
    assert_eq!(value, ExactConstant::named(Constant::Pi, &mut ledger()?)?);
    let (owner, normalized) = value.graph().ok_or("missing graph")?;
    assert_eq!(
        owner.expression(normalized)?.0,
        &Operator::Constant(Constant::Pi)
    );
    let other = Cas::new(Limits::default())?;
    assert!(matches!(
        ExactConstant::from_expression(&other, normalized, &mut ledger()?),
        Err(Fault {
            kind: FaultKind::Invalid(InvalidCode::ForeignHandle),
            ..
        })
    ));
    Ok(())
}

fn boundary<T>(operation: impl Fn(&mut Budget) -> Result<T, Fault>) -> TestResult {
    let mut measured = ledger()?;
    operation(&mut measured)?;
    let work = measured.usage().work;
    let limits = Limits {
        work,
        ..Limits::default()
    };
    operation(&mut Budget::new(limits.clone())?)?;
    let result = operation(&mut Budget::new(Limits {
        work: work.checked_sub(1).ok_or("no measured work")?,
        ..limits
    })?);
    assert!(matches!(
        result,
        Err(Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            },
            ..
        })
    ));
    Ok(())
}

/// Every new closed traversal and composition shares its caller's exact N/N-1 work boundary.
/// ´test:integration:constant-proof-work-boundaries´
#[test]
fn constant_proof_work_boundaries() -> TestResult {
    boundary(|budget| ExactConstant::named(Constant::Pi, budget))?;
    let (cas, root) = original(
        &[
            (Operator::Constant(Constant::Pi), vec![]),
            (Operator::Rational(Rational::one()), vec![]),
            (Operator::Add, vec![0, 1]),
        ],
        false,
    )?;
    boundary(|budget| ExactConstant::from_expression(&cas, &root, budget))?;
    let value = ExactConstant::from_expression(&cas, &root, &mut ledger()?)?;
    boundary(|budget| value.div(&value, budget))?;
    boundary(|budget| value.expression_drafts(Reference::Existing(root.clone()), budget))?;
    Ok(())
}

mod cache;

mod roots;
