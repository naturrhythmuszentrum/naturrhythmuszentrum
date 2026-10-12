// SPDX-License-Identifier: AGPL-3.0-only
//! Public grammar, location and formatting evidence.

use nrz_algebra::{Limits, Node, NormalForm, ParseFailure, Rational, parse};

/// Fallible setup failures retain their diagnostic.
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Exact precedence and numeric literals never pass through floating arithmetic.
/// ´test:integration:syntax-precedence-and-exact-literals´
#[test]
fn syntax_precedence_and_exact_literals() -> TestResult {
    for (source, numerator, denominator) in [
        ("-2^2", -4, 1),
        ("(-2)^2", 4, 1),
        ("1/2/3", 1, 6),
        ("1-2-3", -4, 1),
        ("0.125e2", 25, 2),
        ("1.25e-2", 1, 80),
        ("-9223372036854775808", i64::MIN, 1),
        ("2^-3", 1, 8),
    ] {
        let expression = parse(source, Limits::default())?;
        assert_eq!(
            NormalForm::new(&expression, Limits::default())?.constant(),
            Some(Rational::new(numerator, denominator)?)
        );
    }
    let division = parse("1/2", Limits::default())?;
    assert!(matches!(division.node(), Node::Div(_, _)));
    let half = parse("x^(2/4)", Limits::default())?;
    assert!(matches!(half.node(), Node::Power(_, p) if *p == Rational::new(1, 2)?));
    Ok(())
}

/// Formatting round trips preserve the collection form and function argument order.
/// ´test:integration:syntax-formatting-round-trips´
#[test]
fn syntax_formatting_round_trips() -> TestResult {
    for source in [
        "x+y*z",
        "f(x^(1/2),-3.5,g())",
        "-(a/b)",
        "1.25e-2",
        "(x^2)^(1/2)",
    ] {
        let original = parse(source, Limits::default())?;
        let restored = parse(&original.to_string(), Limits::default())?;
        assert_eq!(
            NormalForm::new(&original, Limits::default())?,
            NormalForm::new(&restored, Limits::default())?
        );
    }
    let left = parse("f(x,y)", Limits::default())?;
    let right = parse("f(y,x)", Limits::default())?;
    assert_ne!(
        NormalForm::new(&left, Limits::default())?,
        NormalForm::new(&right, Limits::default())?
    );
    Ok(())
}

/// Unsupported constructs and incomplete tokens report exact byte positions.
/// ´test:integration:syntax-refusals-and-locations´
#[test]
fn syntax_refusals_and_locations() -> TestResult {
    for source in [
        "",
        "2x",
        "1 m",
        "x^y",
        "x^2^3",
        "x^(1/0)",
        "x[0]",
        "a=b",
        "f(x,)",
        "1.",
        "1e 2",
        "1e",
        "1e999",
        "9223372036854775808",
        "sum(x,i=1..n)",
    ] {
        assert!(
            parse(source, Limits::default()).is_err(),
            "accepted {source}"
        );
    }
    let error = parse("x + )", Limits::default())
        .err()
        .ok_or("missing parser error")?;
    assert_eq!((error.start, error.end), (4, 5));
    assert!(matches!(error.failure, ParseFailure::Syntax(_)));
    let expression = parse("x + y", Limits::default())?;
    let span = expression.span().ok_or("missing source span")?;
    assert_eq!((span.start(), span.end()), (0, 5));
    assert!(
        parse(
            "(((x)))",
            Limits {
                depth: 2,
                ..Limits::default()
            }
        )
        .is_err()
    );
    assert!(
        parse(
            "x",
            Limits {
                work: 0,
                ..Limits::default()
            }
        )
        .is_err()
    );
    Ok(())
}
