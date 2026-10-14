// SPDX-License-Identifier: AGPL-3.0-only
//! Text grammar, exact limits and declaration refusals through the public source facade.

use nrz_cas::domain::HeaderDraft;
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::source::{ParseFailure, SyntaxCode, WalkEvent, parse_original};
use nrz_cas::syntax::{
    AtomicSort, Branch, Declaration, Draft, Environment, Field, Operator, Sort, Span,
};
use nrz_cas::{Cas, FaultKind, Handle, InvalidCode, Rational};

use crate::testing::builders::{at, environment};
use crate::testing::common::{TestResult, default_budget, real};

/// A minimal declaration header with real names and zero-, one- and two-argument functions.
fn declared() -> Environment {
    let mut result = environment(Field::Real, &["x", "y", "a", "b"]);
    for (key, arity) in [("z", 0), ("g", 1), ("f", 2)] {
        result.declarations.push(Declaration {
            key: key.into(),
            display_name: key.into(),
            sort: Sort::Function {
                inputs: vec![AtomicSort::Scalar(Field::Real); arity],
                output: AtomicSort::Scalar(Field::Real),
            },
        });
    }
    result
}

/// Header setup uses a separate default ledger from the parse being tested.
fn store(declarations: Environment) -> TestResult<(Cas, Handle)> {
    let mut cas = Cas::new(Limits::default())?;
    let header = at(
        &cas.admit(
            &[Draft::Header(HeaderDraft::Core(declarations))],
            &mut default_budget()?,
        )?,
        0,
    )?;
    Ok((cas, header))
}

/// The oracle is a hand-written postorder operator list; no formatter or simplifier participates.
fn operators(text: &str) -> TestResult<Vec<Operator>> {
    let (mut cas, header) = store(declared())?;
    let original = parse_original(&mut cas, "grammar", text, &header, &mut default_budget()?)
        .map_err(|error| error.to_string())?;
    cas.walk_original(&original.root, &mut default_budget()?)?
        .events
        .iter()
        .filter_map(|event| match event {
            WalkEvent::Visit(visit) => Some(visit),
            WalkEvent::EnterBinder { .. } => None,
        })
        .map(|visit| Ok(cas.expression(&visit.expression)?.0.clone()))
        .collect()
}

/// Every arithmetic production retains precedence, associativity and exact original order.
/// ´test:integration:parsed-arithmetic-grammar´
#[test]
fn parsed_arithmetic_grammar() -> TestResult {
    let x = Operator::Free("x".into());
    let y = Operator::Free("y".into());
    assert_eq!(
        operators("x+y*x-y/x")?,
        vec![
            x.clone(),
            y.clone(),
            x.clone(),
            Operator::Mul,
            Operator::Add,
            y.clone(),
            x.clone(),
            Operator::Div,
            Operator::Sub
        ]
    );
    assert_eq!(
        operators("x-y-x")?,
        vec![
            x.clone(),
            y.clone(),
            Operator::Sub,
            x.clone(),
            Operator::Sub
        ]
    );
    assert_eq!(
        operators("x/y*x")?,
        vec![x.clone(), y, Operator::Div, x, Operator::Mul]
    );
    Ok(())
}

/// Signs bind below powers and transparent parentheses retain that precedence.
/// ´test:integration:parsed-unary-precedence´
#[test]
fn parsed_unary_precedence() -> TestResult {
    let x = Operator::Free("x".into());
    assert_eq!(
        operators("-x^2")?,
        vec![x.clone(), Operator::IntegerPower(2), Operator::Neg]
    );
    assert_eq!(
        operators("(-x)^2")?,
        vec![x.clone(), Operator::Neg, Operator::IntegerPower(2)]
    );
    assert_eq!(operators("+((x))")?, vec![x.clone()]);
    assert_eq!(
        operators("x^-2/3")?,
        vec![
            x,
            Operator::IntegerPower(-2),
            Operator::Rational(Rational::parse("3", &mut default_budget()?)?),
            Operator::Div
        ]
    );
    Ok(())
}

/// Integers, signed literals, exact decimals and scientific notation never pass through floats.
/// ´test:integration:parsed-exact-literals´
#[test]
fn parsed_exact_literals() -> TestResult {
    for (text, exact) in [
        ("0", "0"),
        ("0012", "12"),
        ("- 2", "-2"),
        ("+2", "2"),
        ("0.5", "1/2"),
        ("12.50e-1", "5/4"),
        ("2E+3", "2000"),
        ("-9223372036854775808", "-9223372036854775808"),
    ] {
        assert_eq!(
            operators(text)?,
            vec![Operator::Rational(Rational::parse(
                exact,
                &mut default_budget()?
            )?)]
        );
    }
    assert_eq!(
        operators("--2")?,
        vec![
            Operator::Rational(Rational::parse("-2", &mut default_budget()?)?),
            Operator::Neg
        ]
    );
    assert_eq!(
        operators("1/2")?,
        vec![
            Operator::Rational(Rational::one()),
            Operator::Rational(Rational::parse("2", &mut default_budget()?)?),
            Operator::Div
        ]
    );
    Ok(())
}

/// Calls retain every written argument, including zero-argument functions and nested calls.
/// ´test:integration:parsed-call-grammar´
#[test]
fn parsed_call_grammar() -> TestResult {
    assert_eq!(
        operators("f(g(x), z())")?,
        vec![
            Operator::Free("x".into()),
            Operator::Call("g".into()),
            Operator::Call("z".into()),
            Operator::Call("f".into())
        ]
    );
    assert_eq!(
        operators("g (x + y)")?,
        vec![
            Operator::Free("x".into()),
            Operator::Free("y".into()),
            Operator::Add,
            Operator::Call("g".into())
        ]
    );
    Ok(())
}

/// Reduced rational exponents choose the fixed payload exactly when it can represent them.
/// ´test:integration:parsed-power-conventions´
#[test]
fn parsed_power_conventions() -> TestResult {
    let x = Operator::Free("x".into());
    for text in ["x^2", "x^(+2)", "x^(4/2)"] {
        assert_eq!(operators(text)?, vec![x.clone(), Operator::IntegerPower(2)]);
    }
    for (text, exponent) in [
        ("x^(-1/2)", "-1/2"),
        ("x^9223372036854775808", "9223372036854775808"),
    ] {
        assert_eq!(
            operators(text)?,
            vec![
                x.clone(),
                Operator::Rational(Rational::parse(exponent, &mut default_budget()?)?),
                Operator::GeneralPower(Branch::StrictReal)
            ]
        );
    }
    for branch in [
        Branch::PrincipalComplex,
        Branch::Declared("chosen-root".into()),
    ] {
        let mut declarations = environment(Field::Complex, &["x"]);
        declarations.branch.clone_from(&branch);
        let (mut cas, header) = store(declarations)?;
        let original = parse_original(
            &mut cas,
            "branch",
            "x^(1/2)",
            &header,
            &mut default_budget()?,
        )?;
        assert_eq!(
            cas.expression(&original.syntax)?.0,
            &Operator::GeneralPower(branch)
        );
    }
    Ok(())
}

/// Every stable syntax code has a precise byte interval and deterministic cursor position.
/// ´test:integration:parsed-syntax-errors´
#[test]
fn parsed_syntax_errors() -> TestResult {
    for &(text, code, start, end, progress) in SYNTAX_ERRORS {
        let (mut cas, header) = store(declared())?;
        let before = cas.canonical_snapshot(&mut default_budget()?)?;
        let error = parse_original(&mut cas, "bad", text, &header, &mut default_budget()?)
            .err()
            .ok_or("invalid grammar must refuse")?;
        assert_eq!(error.failure, ParseFailure::Syntax(code), "{text}");
        assert_eq!(error.span, Span { start, end }, "{text}");
        assert_eq!(error.parsed_to, progress, "{text}");
        assert_eq!(error.source_key, "bad");
        assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    Ok(())
}

/// No implicit multiplication, exponent expression, chained power or relaxed decimal slips in.
/// ´test:integration:parsed-grammar-exclusions´
#[test]
fn parsed_grammar_exclusions() -> TestResult {
    for text in [
        "2x",
        "x y",
        "x^2^3",
        "x^(1+2)",
        "x^0.5",
        ".5",
        "1.e2",
        "1e",
        "1e-",
        "()",
        "f(x y)",
        "f(x,)",
        "x)",
        "x + )",
        "x\u{a0}+y",
    ] {
        let (mut cas, header) = store(declared())?;
        let error = parse_original(&mut cas, "excluded", text, &header, &mut default_budget()?)
            .err()
            .ok_or("unsupported grammar must refuse")?;
        assert!(
            matches!(error.failure, ParseFailure::Syntax(_)),
            "{text}: {error:?}"
        );
    }
    Ok(())
}

/// Name and arity diagnostics retain the exact name span, independently of call extent.
/// ´test:integration:parsed-declaration-refusals´
#[test]
fn parsed_declaration_refusals() -> TestResult {
    for &(text, failure, span) in DECLARATION_ERRORS {
        let (mut cas, header) = store(declared())?;
        let before = cas.canonical_snapshot(&mut default_budget()?)?;
        let error = parse_original(&mut cas, "names", text, &header, &mut default_budget()?)
            .err()
            .ok_or("expected a declaration refusal")?;
        assert_eq!(error.failure, failure);
        assert_eq!(error.span, span);
        assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    Ok(())
}

/// Shared display spellings cannot alias distinct declared identities or create new names.
/// ´test:integration:parsed-declaration-keys´
#[test]
fn parsed_declaration_keys() -> TestResult {
    let declarations = Environment {
        declarations: ["left", "right"]
            .iter()
            .map(|key| Declaration {
                key: (*key).into(),
                display_name: "same".into(),
                sort: real(),
            })
            .collect(),
        ..Environment::default()
    };
    let (mut cas, header) = store(declarations)?;
    let original = parse_original(
        &mut cas,
        "keys",
        "left+right",
        &header,
        &mut default_budget()?,
    )?;
    let children = cas.operands(&original.syntax, &mut default_budget()?)?;
    assert_ne!(children.first(), children.get(1));
    let error = parse_original(&mut cas, "display", "same", &header, &mut default_budget()?)
        .err()
        .ok_or("display names must not resolve")?;
    assert_eq!(error.failure, ParseFailure::UnknownName { name: "same" });
    Ok(())
}

/// Only the named ceiling changes; all other parser and admission budgets remain default.
fn limited(kind: BudgetKind, ceiling: u64) -> TestResult<Budget> {
    let mut limits = Limits::default();
    match kind {
        BudgetKind::Depth => limits.depth = ceiling,
        BudgetKind::OriginalOccurrences => limits.original_occurrences = ceiling,
        BudgetKind::LiteralBytes => limits.literal_bytes = ceiling,
        BudgetKind::Work => limits.work = ceiling,
        _ => return Err("unexpected parser boundary resource".into()),
    }
    Ok(Budget::new(limits)?)
}

/// Depth, distinct uses and literal size each accept the exact limit and refuse one below.
/// ´test:integration:parsed-budget-boundaries´
#[test]
fn parsed_budget_boundaries() -> TestResult {
    for &(text, kind, needed, span, progress) in BUDGET_BOUNDARIES {
        let (mut cas, header) = store(declared())?;
        parse_original(
            &mut cas,
            "boundary",
            text,
            &header,
            &mut limited(kind, needed)?,
        )
        .map_err(|error| error.to_string())?;
        let (mut refused, header) = store(declared())?;
        let before = refused.canonical_snapshot(&mut default_budget()?)?;
        let error = parse_original(
            &mut refused,
            "boundary",
            text,
            &header,
            &mut limited(kind, needed - 1)?,
        )
        .err()
        .ok_or("one-below boundary must refuse")?;
        assert!(
            matches!(error.failure, ParseFailure::Fault(nrz_cas::Fault {
            kind: FaultKind::Exhausted { budget, limit, needed: actual, .. }, ..
        }) if budget == kind && limit == needed - 1 && actual == needed),
            "{text}: {error:?}"
        );
        assert_eq!(error.span, span, "{text}");
        assert_eq!(error.parsed_to, progress, "{text}");
        assert_eq!(refused.canonical_snapshot(&mut default_budget()?)?, before);
    }
    Ok(())
}

/// A shared work ledger accepts precisely the measured complete transaction cost.
/// ´test:integration:parsed-work-boundary´
#[test]
fn parsed_work_boundary() -> TestResult {
    let (mut cas, header) = store(declared())?;
    let mut measured = default_budget()?;
    parse_original(&mut cas, "work", "f(x, -2.5)", &header, &mut measured)?;
    let needed = measured.usage().work;
    for allowance in [needed, needed - 1, 0] {
        let (mut candidate, header) = store(declared())?;
        let before = candidate.canonical_snapshot(&mut default_budget()?)?;
        let mut budget = limited(BudgetKind::Work, allowance)?;
        let result = parse_original(&mut candidate, "work", "f(x, -2.5)", &header, &mut budget);
        assert_eq!(result.is_ok(), allowance == needed);
        if let Err(error) = result {
            assert!(matches!(
                error.failure,
                ParseFailure::Fault(nrz_cas::Fault {
                    kind: FaultKind::Exhausted {
                        budget: BudgetKind::Work,
                        ..
                    },
                    ..
                })
            ));
            assert!(error.parsed_to <= 10);
            assert_eq!(
                candidate.canonical_snapshot(&mut default_budget()?)?,
                before
            );
        }
        assert!(budget.usage().work <= allowance);
    }
    Ok(())
}

/// Raised grammar depth uses heap continuations, and a store ceiling still constrains a call.
/// ´test:integration:parsed-raised-depth´
#[test]
fn parsed_raised_depth() -> TestResult {
    let limits = Limits {
        depth: 513,
        ..Limits::default()
    };
    let mut cas = Cas::new(limits.clone())?;
    let header = at(
        &cas.admit(
            &[Draft::Header(HeaderDraft::Core(Environment::default()))],
            &mut default_budget()?,
        )?,
        0,
    )?;
    let text = format!("{}1{}", "(".repeat(512), ")".repeat(512));
    let original = parse_original(
        &mut cas,
        "deep",
        &text,
        &header,
        &mut Budget::new(limits.clone())?,
    )
    .map_err(|error| error.to_string())?;
    assert_eq!(
        cas.walk_original(&original.root, &mut default_budget()?)?
            .receipt
            .visited(),
        1
    );
    let mut lower = limits.clone();
    lower.depth = 512;
    let error = parse_original(&mut cas, "deep", &text, &header, &mut Budget::new(lower)?)
        .err()
        .ok_or("lower nesting ceiling must refuse")?;
    assert!(matches!(
        error.failure,
        ParseFailure::Fault(nrz_cas::Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::Depth,
                limit: 512,
                needed: 513,
                ..
            },
            ..
        })
    ));
    let (mut shallow, header) = store(Environment::default())?;
    let error = parse_original(
        &mut shallow,
        "deep",
        &text,
        &header,
        &mut Budget::new(limits)?,
    )
    .err()
    .ok_or("request cannot raise the store depth")?;
    assert!(matches!(
        error.failure,
        ParseFailure::Fault(nrz_cas::Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::Depth,
                limit: 128,
                ..
            },
            ..
        })
    ));
    Ok(())
}

/// Source conflicts, foreign headers and impossible exponent fractions stay typed, atomic faults.
/// ´test:integration:parsed-admission-refusals´
#[test]
fn parsed_admission_refusals() -> TestResult {
    let (mut cas, header) = store(declared())?;
    parse_original(&mut cas, "shared", "x", &header, &mut default_budget()?)?;
    let before = cas.canonical_snapshot(&mut default_budget()?)?;
    let error = parse_original(&mut cas, "shared", "y", &header, &mut default_budget()?)
        .err()
        .ok_or("source conflict must refuse")?;
    assert!(matches!(
        error.failure,
        ParseFailure::Fault(nrz_cas::Fault {
            kind: FaultKind::Invalid(InvalidCode::SourceConflict),
            ..
        })
    ));
    let (_, foreign) = store(declared())?;
    let error = parse_original(&mut cas, "foreign", "x", &foreign, &mut default_budget()?)
        .err()
        .ok_or("foreign header must refuse")?;
    assert!(matches!(
        error.failure,
        ParseFailure::Fault(nrz_cas::Fault {
            kind: FaultKind::Invalid(InvalidCode::ForeignHandle),
            ..
        })
    ));
    let error = parse_original(&mut cas, "zero", "x^(1/0)", &header, &mut default_budget()?)
        .err()
        .ok_or("zero exponent denominator must refuse")?;
    assert_eq!(error.span, Span { start: 5, end: 6 });
    assert!(matches!(
        error.failure,
        ParseFailure::Fault(nrz_cas::Fault {
            kind: FaultKind::Invalid(InvalidCode::ZeroDenominator),
            ..
        })
    ));
    assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
    Ok(())
}

/// Grammar failures and their independent offending spans and progress offsets.
const SYNTAX_ERRORS: &[(&str, SyntaxCode, u64, u64, u64)] = &[
    ("", SyntaxCode::UnexpectedToken, 0, 0, 0),
    ("x + β", SyntaxCode::UnexpectedToken, 4, 6, 4),
    ("(x", SyntaxCode::UnclosedParenthesis, 0, 1, 2),
    ("f(x, y", SyntaxCode::UnclosedParenthesis, 1, 2, 6),
    ("x^(", SyntaxCode::UnclosedParenthesis, 2, 3, 3),
    ("x + ", SyntaxCode::TrailingOperator, 2, 3, 4),
    ("-", SyntaxCode::TrailingOperator, 0, 1, 1),
    ("1.", SyntaxCode::InvalidLiteral, 2, 2, 2),
    ("1e +2", SyntaxCode::InvalidLiteral, 2, 3, 2),
    ("x^y", SyntaxCode::InvalidExponent, 2, 3, 2),
    ("x^(1/-2)", SyntaxCode::InvalidExponent, 5, 6, 5),
];

/// Declaration errors point at names rather than entire calls.
const DECLARATION_ERRORS: &[(&str, ParseFailure<'static>, Span)] = &[
    (
        "lost",
        ParseFailure::UnknownName { name: "lost" },
        Span { start: 0, end: 4 },
    ),
    (
        "x + lost(y)",
        ParseFailure::UnknownName { name: "lost" },
        Span { start: 4, end: 8 },
    ),
    (
        "x(y)",
        ParseFailure::NotCallable { name: "x" },
        Span { start: 0, end: 1 },
    ),
    (
        "f(x)",
        ParseFailure::Arity {
            name: "f",
            expected: 2,
            actual: 1,
        },
        Span { start: 0, end: 1 },
    ),
    (
        "x+g(x,y)",
        ParseFailure::Arity {
            name: "g",
            expected: 1,
            actual: 2,
        },
        Span { start: 2, end: 3 },
    ),
    (
        "f()",
        ParseFailure::Arity {
            name: "f",
            expected: 2,
            actual: 0,
        },
        Span { start: 0, end: 1 },
    ),
];

/// Exact parser boundary fixtures and the expected one-below refusal locations.
const BUDGET_BOUNDARIES: &[(&str, BudgetKind, u64, Span, u64)] = &[
    ("x", BudgetKind::Depth, 1, Span { start: 0, end: 1 }, 0),
    (
        "x",
        BudgetKind::OriginalOccurrences,
        1,
        Span { start: 0, end: 1 },
        1,
    ),
    (
        "7",
        BudgetKind::LiteralBytes,
        1,
        Span { start: 0, end: 1 },
        0,
    ),
    ("((x))", BudgetKind::Depth, 3, Span { start: 2, end: 3 }, 2),
    ("x+x+x", BudgetKind::Depth, 3, Span { start: 0, end: 5 }, 5),
    (
        "x+x",
        BudgetKind::OriginalOccurrences,
        3,
        Span { start: 0, end: 3 },
        3,
    ),
    (
        "x^(1/2)",
        BudgetKind::Depth,
        2,
        Span { start: 0, end: 7 },
        7,
    ),
    (
        "x^(1/2)",
        BudgetKind::OriginalOccurrences,
        3,
        Span { start: 0, end: 7 },
        7,
    ),
    (
        "12.5",
        BudgetKind::LiteralBytes,
        4,
        Span { start: 0, end: 4 },
        3,
    ),
    (
        "-12",
        BudgetKind::LiteralBytes,
        3,
        Span { start: 0, end: 3 },
        3,
    ),
    (
        "x^(1/2)",
        BudgetKind::LiteralBytes,
        3,
        Span { start: 3, end: 6 },
        6,
    ),
];

/// Parsing preserves partial original operations and every occurrence beneath a zero factor.
/// ´test:integration:parsed-partial-operations´
#[test]
fn parsed_partial_operations() -> TestResult {
    let x = Operator::Free("x".into());
    assert_eq!(
        operators("x/0")?,
        vec![
            x.clone(),
            Operator::Rational(Rational::zero()),
            Operator::Div
        ]
    );
    assert_eq!(
        operators("x^0")?,
        vec![x.clone(), Operator::IntegerPower(0)]
    );
    assert_eq!(
        operators("0*(x/x)")?,
        vec![
            Operator::Rational(Rational::zero()),
            x.clone(),
            x,
            Operator::Div,
            Operator::Mul
        ]
    );
    Ok(())
}

/// Call spans include the written callee and delimiters while child spans exclude separators.
/// ´test:integration:parsed-call-spans´
#[test]
fn parsed_call_spans() -> TestResult {
    let (mut cas, header) = store(declared())?;
    let original = parse_original(
        &mut cas,
        "calls",
        "f( x, g(y) )",
        &header,
        &mut default_budget()?,
    )?;
    let spans: Vec<_> = cas
        .walk_original(&original.root, &mut default_budget()?)?
        .events
        .into_iter()
        .filter_map(|event| match event {
            WalkEvent::Visit(visit) => Some(visit),
            WalkEvent::EnterBinder { .. } => None,
        })
        .map(|visit| {
            visit
                .source
                .ok_or("source missing")?
                .span
                .ok_or("span missing")
        })
        .collect::<Result<_, _>>()?;
    assert_eq!(
        spans,
        vec![
            Span { start: 3, end: 4 },
            Span { start: 8, end: 9 },
            Span { start: 6, end: 10 },
            Span { start: 0, end: 12 }
        ]
    );
    Ok(())
}
