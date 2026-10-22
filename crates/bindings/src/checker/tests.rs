// SPDX-License-Identifier: AGPL-3.0-only
//! Per-check sharing retains occurrence evidence and the first exact depth refusal.

use nrz_cas::limits::{BudgetKind, Limits};
use nrz_cas::syntax::{AtomicSort, Declaration as Mathematical, Environment, Field, Sort, Span};
use nrz_cas::{Fault, FaultKind, Reference};

use super::{Context, Declaration, Descriptor, State};
use crate::{Evidence, ExponentPolicy, Failure, Observable, Registry};

/// These crate tests use only their own setup and the public CAS boundary.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Identical stores for the single-use and repeated-use controls.
fn fixture(body: &str, uses: &str, limits: Limits) -> TestResult<Context> {
    let environment = Environment {
        declarations: ["h", "total", "x"]
            .into_iter()
            .map(|name| Mathematical {
                key: name.into(),
                display_name: name.into(),
                sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            })
            .collect(),
        ..Environment::default()
    };
    let mut context = Context::new(Registry::new(ExponentPolicy::Rational), limits, environment)?;
    context.declare(
        "x".into(),
        Declaration::Input {
            descriptor: Descriptor::ratio(),
            nonzero: false,
            evidence: Evidence::default(),
        },
    )?;
    for (name, text) in [("h", body), ("total", uses)] {
        let expression = context.parse(name, text)?;
        context.declare(
            name.into(),
            Declaration::Derived(Observable {
                descriptor: Descriptor::ratio(),
                expression,
                evidence: Evidence::default(),
            }),
        )?;
    }
    Ok(context)
}

/// Measure the production ledger through publication and the final local draft index.
fn measured(context: &Context, name: &str) -> TestResult<(u64, u32, usize)> {
    let original = context.parse("probe", name)?;
    let mut cas = context.engine()?;
    let mut state = State::new(context, &mut cas)?;
    let (value, _) = state.walk(&original, 1)?;
    let Reference::Local(last) = &value.coordinates.reference else {
        return Err("expected a staged arithmetic root".into());
    };
    let drafts = last + 1;
    state.publish(&value.coordinates)?;
    Ok((state.budget.usage().work, drafts, state.memo.len()))
}

/// Eight uses build the heavy body once and pay only their own additional arithmetic and replay.
/// ´test:unit:repeated-derived-work´
#[test]
fn repeated_derived_work() -> TestResult {
    let body = ["x"; 24].join("+");
    let uses = ["h"; 8].join("+");
    let single = fixture(&body, &uses, Limits::default())?;
    let repeated = fixture(&body, &uses, Limits::default())?;
    let (one_work, one_drafts, one_memos) = measured(&single, "h")?;
    let (many_work, many_drafts, many_memos) = measured(&repeated, "total")?;
    assert_eq!((one_drafts, many_drafts), (23, 30));
    assert_eq!((one_memos, many_memos), (1, 2));
    assert!(
        many_work < 4 * one_work,
        "single {one_work}, repeated {many_work}"
    );
    // A subsequent check starts a new memo even though the CAS retains the prior coordinates.
    let (_, next_drafts, next_memos) = measured(&repeated, "total")?;
    assert_eq!((next_drafts, next_memos), (30, 2));
    Ok(())
}

/// Cached bodies still contribute evidence at both original caller paths and body byte spans.
/// ´test:unit:repeated-derived-locations´
#[test]
fn repeated_derived_locations() -> TestResult {
    let context = fixture("x+x", "h+h", Limits::default())?;
    let original = context.parse("probe", "total")?;
    let checked = context.check(&original)?;
    let roots = checked
        .nodes()
        .iter()
        .filter(|node| {
            node.location.definitions == ["total", "h"]
                && node.location.span == Some(Span { start: 0, end: 3 })
        })
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 2);
    assert_eq!(
        roots.first().ok_or("first body missing")?.location.path,
        [0, 0, 0]
    );
    assert_eq!(
        roots.get(1).ok_or("second body missing")?.location.path,
        [0, 1, 0]
    );
    assert_eq!(checked.nodes().len(), 10);
    assert_eq!(checked.declarations().len(), 3);
    Ok(())
}

/// Reusing a successful body deeper in the expression reports its first overflowing leaf.
/// ´test:unit:repeated-derived-depth-location´
#[test]
fn repeated_derived_depth_location() -> TestResult {
    let context = fixture(
        "x+x",
        "h+(h+x)",
        Limits {
            depth: 5,
            ..Limits::default()
        },
    )?;
    let original = context.parse("probe", "total")?;
    let error = context
        .check(&original)
        .err()
        .ok_or("expected depth refusal")?;
    assert!(matches!(
        error.failure,
        Failure::Algebra(Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::Depth,
                limit: 5,
                needed: 6,
                ..
            },
            ..
        })
    ));
    assert_eq!(error.location.path, [0, 1, 0, 0, 0]);
    assert_eq!(error.location.definitions, ["total", "h"]);
    assert_eq!(error.location.span, Some(Span { start: 0, end: 1 }));
    Ok(())
}

/// Failures are never memoized or shifted to a later occurrence of the declaration.
/// ´test:unit:derived-failure-first-location´
#[test]
fn derived_failure_first_location() -> TestResult {
    let context = fixture("sin(x)", "h+h", Limits::default())?;
    let original = context.parse("probe", "total")?;
    let error = context
        .check(&original)
        .err()
        .ok_or("expected physical refusal")?;
    assert!(matches!(error.failure, Failure::Units(_)));
    assert_eq!(error.location.path, [0, 0, 0, 0]);
    assert_eq!(error.location.definitions, ["total", "h"]);
    assert_eq!(error.location.span, Some(Span { start: 4, end: 5 }));
    Ok(())
}
