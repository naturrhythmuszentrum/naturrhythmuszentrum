// SPDX-License-Identifier: AGPL-3.0-only
//! Exact independent small-grid and original-tree contract evidence.

use std::collections::BTreeMap;

use nrz_algebra::{ArithmeticError, Error, Expr, Limits, Node, NormalForm, Rational, Symbol};

/// Fallible tests preserve the error that invalidates their setup.
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Construct a symbol without a parser dependency.
fn symbol(name: &str) -> Result<Expr, Error> {
    Expr::new(Node::Symbol(Symbol::new(name)?), Limits::default())
}

/// Construct an exact integer node.
fn integer(value: i64) -> Result<Expr, Error> {
    Expr::new(Node::Constant(Rational::integer(value)), Limits::default())
}

/// Preserve a binary operation in its original order.
fn pair(op: fn(Box<Expr>, Box<Expr>) -> Node, a: Expr, b: Expr) -> Result<Expr, Error> {
    Expr::new(op(Box::new(a), Box::new(b)), Limits::default())
}

/// Construct an original power.
fn power(base: Expr, n: i64, d: i64) -> Result<Expr, Error> {
    Expr::new(
        Node::Power(Box::new(base), Rational::new(n, d)?),
        Limits::default(),
    )
}

/// Normalize under the standard finite budgets.
fn normal(expression: &Expr) -> Result<NormalForm, Error> {
    NormalForm::new(expression, Limits::default())
}

/// Exhaustive small fractions obey exact identities and independently computed products.
/// ´test:integration:rational-small-grid´
#[test]
fn rational_small_grid() -> TestResult {
    for a in -6..=6 {
        for b in 1..=6 {
            let x = Rational::new(a, b)?;
            for c in -6..=6 {
                for d in 1..=6 {
                    let y = Rational::new(c, d)?;
                    assert_eq!(x.checked_mul(y)?, Rational::new(a * c, b * d)?);
                    assert_eq!(x.checked_add(y)?, Rational::new(a * d + b * c, b * d)?);
                    assert_eq!(x.checked_add(y)?.checked_sub(y)?, x);
                    if c != 0 {
                        assert_eq!(x.checked_mul(y)?.checked_div(y)?, x);
                    }
                    assert_eq!(x.cmp(&y), (a * d).cmp(&(c * b)));
                }
            }
        }
    }
    Ok(())
}

/// Reduction precedes narrowing and all finite-range failures are explicit.
/// ´test:integration:rational-boundaries´
#[test]
fn rational_boundaries() -> TestResult {
    let maximum = Rational::integer(i64::MAX);
    let minimum = Rational::integer(i64::MIN);
    assert_eq!(Rational::new(i64::MIN, i64::MIN)?, Rational::ONE);
    assert_eq!(Rational::new(0, i64::MIN)?, Rational::ZERO);
    assert_eq!(minimum.checked_sub(minimum)?, Rational::ZERO);
    assert_eq!(
        Rational::new(i64::MAX, 2)?.checked_add(Rational::new(i64::MAX, 2)?)?,
        maximum
    );
    assert_eq!(
        maximum.checked_mul(Rational::new(1, i64::MAX)?)?,
        Rational::ONE
    );
    assert_eq!(
        maximum.checked_add(Rational::ONE),
        Err(ArithmeticError::Overflow)
    );
    assert_eq!(minimum.checked_neg(), Err(ArithmeticError::Overflow));
    assert_eq!(
        minimum.checked_div(Rational::integer(-1)),
        Err(ArithmeticError::Overflow)
    );
    assert_eq!(Rational::new(1, 0), Err(ArithmeticError::ZeroDenominator));
    assert_eq!(
        Rational::integer(2).checked_pow(63),
        Err(ArithmeticError::Overflow)
    );
    assert_eq!(
        Rational::ZERO.checked_pow(0),
        Err(ArithmeticError::UndefinedPower)
    );
    assert_eq!(Rational::integer(-1).checked_pow(i64::MIN)?, Rational::ONE);
    assert_eq!(
        Rational::new(-2, 3)?.checked_pow(-3)?,
        Rational::new(-27, 8)?
    );
    Ok(())
}

/// Sum and product permutations and parenthesizations share one form.
/// ´test:integration:collection-is-associative-and-commutative´
#[test]
fn collection_is_associative_and_commutative() -> TestResult {
    let values = [symbol("x")?, symbol("y")?, integer(3)?];
    for op in [Node::Add as fn(Box<Expr>, Box<Expr>) -> Node, Node::Mul] {
        let reference = normal(&pair(
            op,
            pair(op, values[0].clone(), values[1].clone())?,
            values[2].clone(),
        )?)?;
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let [a, b, c] = order;
            let left = pair(
                op,
                pair(op, values[a].clone(), values[b].clone())?,
                values[c].clone(),
            )?;
            let right = pair(
                op,
                values[a].clone(),
                pair(op, values[b].clone(), values[c].clone())?,
            )?;
            assert_eq!(normal(&left)?, reference);
            assert_eq!(normal(&right)?, reference);
        }
        assert_eq!(
            normal(&reference.expression(Limits::default())?)?,
            reference
        );
    }
    assert_eq!(
        normal(&pair(Node::Add, symbol("x")?, symbol("x")?)?)?,
        normal(&pair(Node::Mul, integer(2)?, symbol("x")?)?)?
    );
    Ok(())
}

/// Rational content of sums preserves both addition and multiplication regrouping.
///
/// ´test:integration:scaled-sum-associativity´
#[test]
fn scaled_sum_associativity() -> TestResult {
    let sum = pair(Node::Add, symbol("x")?, symbol("y")?)?;
    let values = [
        pair(Node::Mul, integer(2)?, sum.clone())?,
        pair(Node::Mul, integer(-1)?, sum.clone())?,
        symbol("z")?,
    ];
    for op in [Node::Add as fn(Box<Expr>, Box<Expr>) -> Node, Node::Mul] {
        let reference = normal(&pair(
            op,
            pair(op, values[0].clone(), values[1].clone())?,
            values[2].clone(),
        )?)?;
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let [a, b, c] = order;
            assert_eq!(
                normal(&pair(
                    op,
                    values[a].clone(),
                    pair(op, values[b].clone(), values[c].clone())?
                )?)?,
                reference
            );
            assert_eq!(
                normal(&pair(
                    op,
                    pair(op, values[a].clone(), values[b].clone())?,
                    values[c].clone()
                )?)?,
                reference
            );
        }
        assert_eq!(
            normal(&reference.expression(Limits::default())?)?,
            reference
        );
    }
    let factored = pair(Node::Mul, integer(2)?, sum)?;
    let collected = pair(
        Node::Add,
        pair(Node::Mul, integer(2)?, symbol("x")?)?,
        pair(Node::Mul, integer(2)?, symbol("y")?)?,
    )?;
    assert_eq!(normal(&factored)?, normal(&collected)?);
    let left = pair(Node::Mul, factored, symbol("z")?)?;
    let right = pair(
        Node::Mul,
        integer(2)?,
        pair(
            Node::Mul,
            pair(Node::Add, symbol("x")?, symbol("y")?)?,
            symbol("z")?,
        )?,
    )?;
    assert_eq!(normal(&left)?, normal(&right)?);
    Ok(())
}

/// Rational powers collect while fractional outer powers preserve their base.
/// ´test:integration:powers-and-equivalence-boundary´
#[test]
fn powers_and_equivalence_boundary() -> TestResult {
    assert_eq!(
        normal(&pair(
            Node::Mul,
            power(symbol("x")?, 1, 2)?,
            power(symbol("x")?, 1, 3)?
        )?)?,
        normal(&power(symbol("x")?, 5, 6)?)?
    );
    assert_eq!(
        normal(&power(power(symbol("x")?, 1, 2)?, 2, 1)?)?,
        normal(&symbol("x")?)?
    );
    assert_ne!(
        normal(&power(power(symbol("x")?, 2, 1)?, 1, 2)?)?,
        normal(&symbol("x")?)?
    );
    let xy = pair(Node::Mul, symbol("x")?, symbol("y")?)?;
    assert_ne!(
        normal(&power(xy.clone(), 1, 2)?)?,
        normal(&pair(
            Node::Mul,
            power(symbol("x")?, 1, 2)?,
            power(symbol("y")?, 1, 2)?
        )?)?
    );
    assert_eq!(
        normal(&power(xy, 2, 1)?)?,
        normal(&pair(
            Node::Mul,
            power(symbol("x")?, 2, 1)?,
            power(symbol("y")?, 2, 1)?
        )?)?
    );
    let root = power(integer(2)?, 1, 3)?;
    assert_eq!(
        normal(&pair(Node::Mul, power(integer(2)?, 1, 1)?, root)?)?,
        normal(&power(integer(2)?, 4, 3)?)?
    );
    let a = pair(
        Node::Mul,
        symbol("x")?,
        pair(Node::Add, symbol("y")?, symbol("z")?)?,
    )?;
    let b = pair(
        Node::Add,
        pair(Node::Mul, symbol("x")?, symbol("y")?)?,
        pair(Node::Mul, symbol("x")?, symbol("z")?)?,
    )?;
    assert_ne!(normal(&a)?, normal(&b)?);
    assert_ne!(normal(&symbol("x")?)?, normal(&symbol("y")?)?);
    Ok(())
}

/// Domain failures survive annihilators and simultaneous substitution preserves source operations.
/// ´test:integration:source-domains-and-substitution´
#[test]
fn source_domains_and_substitution() -> TestResult {
    let quotient = pair(Node::Div, symbol("x")?, symbol("x")?)?;
    assert_eq!(normal(&quotient)?.constant(), Some(Rational::ONE));
    let replacements = BTreeMap::from([(Symbol::new("x")?, integer(0)?)]);
    let substituted = quotient.substitute(&replacements, Limits::default())?;
    assert!(matches!(substituted.node(), Node::Div(_, _)));
    assert!(matches!(
        normal(&substituted),
        Err(Error::Arithmetic(ArithmeticError::ZeroDenominator))
    ));
    let hidden = pair(
        Node::Mul,
        integer(0)?,
        pair(Node::Div, integer(1)?, integer(0)?)?,
    )?;
    assert!(normal(&hidden).is_err());
    assert!(normal(&power(integer(0)?, 0, 1)?).is_err());
    let map = BTreeMap::from([
        (Symbol::new("x")?, symbol("y")?),
        (Symbol::new("y")?, integer(7)?),
    ]);
    assert_eq!(
        symbol("x")?.substitute(&map, Limits::default())?.symbols(),
        symbol("y")?.symbols()
    );
    let call = Expr::new(
        Node::Call(Symbol::new("f")?, vec![symbol("x")?]),
        Limits::default(),
    )?;
    assert_eq!(call.symbols(), symbol("x")?.symbols());
    assert_ne!(normal(&call)?, normal(&symbol("x")?)?);
    Ok(())
}

/// Construction, normalization and substitution all enforce explicit finite budgets.
/// ´test:integration:resource-refusals´
#[test]
fn resource_refusals() -> TestResult {
    let x = symbol("x")?;
    let limits = Limits {
        depth: 1,
        ..Limits::default()
    };
    assert_eq!(
        Expr::new(Node::Neg(Box::new(x.clone())), limits),
        Err(Error::Limit("depth"))
    );
    assert_eq!(
        NormalForm::new(
            &x,
            Limits {
                work: 0,
                ..Limits::default()
            }
        ),
        Err(Error::Limit("normalization work"))
    );
    assert!(
        x.substitute(
            &BTreeMap::new(),
            Limits {
                work: 0,
                ..Limits::default()
            }
        )
        .is_err()
    );
    assert_eq!(x.symbols().len(), 1);
    Ok(())
}
