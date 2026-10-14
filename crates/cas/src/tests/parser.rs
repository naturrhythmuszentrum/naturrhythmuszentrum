// SPDX-License-Identifier: AGPL-3.0-only
//! Parser admission invariants checked against the crate-owned graph records.

use crate::domain::HeaderDraft;
use crate::limits::{Budget, BudgetKind, Limits};
use crate::source::{ParseFailure, parse_original};
use crate::syntax::{Declaration, Draft, Environment, Operator, Span};
use crate::testing::common::{TestResult, default_budget, real};
use crate::{Cas, Fault, FaultKind, Handle};

/// One declared real variable; no source or occurrence exists before parsing.
fn declared_store() -> TestResult<(Cas, Handle)> {
    let mut cas = Cas::new(Limits::default())?;
    let handles = cas.admit(
        &[Draft::Header(HeaderDraft::Core(Environment {
            declarations: vec![Declaration {
                key: "x".into(),
                display_name: "coordinate".into(),
                sort: real(),
            }],
            ..Environment::default()
        }))],
        &mut default_budget()?,
    )?;
    Ok((cas, handles.first().cloned().ok_or("header missing")?))
}

/// Repeated names share mathematical syntax while original uses and edges stay distinct.
/// ´test:crate:parsed-graph-occurrences´
#[test]
fn parsed_graph_occurrences() -> TestResult {
    let (mut cas, header) = declared_store()?;
    let original = parse_original(
        &mut cas,
        "formula",
        "(x+x)",
        &header,
        &mut default_budget()?,
    )?;
    let root = cas.state.occurrence(original.root.logical_id())?;
    assert_eq!(root.span, Some(Span { start: 0, end: 5 }));
    assert_eq!(root.expression, original.syntax.logical_id());
    assert_eq!(root.children.len(), 2);
    let first = *root.children.first().ok_or("first use")?;
    let second = *root.children.get(1).ok_or("second use")?;
    assert_ne!(first, second);
    let left = cas.state.occurrence(first)?;
    let right = cas.state.occurrence(second)?;
    assert_eq!(left.expression, right.expression);
    assert_eq!(left.span, Some(Span { start: 1, end: 2 }));
    assert_eq!(right.span, Some(Span { start: 3, end: 4 }));
    assert_eq!(left.source, root.source);
    assert_eq!(right.source, root.source);
    assert!(root.provenance.is_none());
    let expression = cas.state.expression(root.expression)?;
    assert_eq!(expression.operator, Operator::Add);
    assert_eq!(expression.operands, vec![left.expression, right.expression]);
    assert_eq!(expression.header, header.logical_id());
    assert_eq!(cas.counts().original_occurrences, 3);
    Ok(())
}

/// Syntax, numeric and admission failures consume no logical identifiers or source keys.
/// ´test:crate:parsed-rollback-identifiers´
#[test]
fn parsed_rollback_identifiers() -> TestResult {
    let (mut cas, header) = declared_store()?;
    let before = cas.canonical_snapshot(&mut default_budget()?)?;
    for text in ["(x+x", "x+missing", "x^(1/0)", "x+1."] {
        assert!(parse_original(&mut cas, "retry", text, &header, &mut default_budget()?).is_err());
        assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    let actual = parse_original(&mut cas, "retry", "x+x", &header, &mut default_budget()?)?;
    let (mut fresh, header) = declared_store()?;
    let expected = parse_original(&mut fresh, "retry", "x+x", &header, &mut default_budget()?)?;
    assert_eq!(actual.syntax.logical_id(), expected.syntax.logical_id());
    assert_eq!(actual.root.logical_id(), expected.root.logical_id());
    assert_eq!(
        cas.canonical_snapshot(&mut default_budget()?)?,
        fresh.canonical_snapshot(&mut default_budget()?)?
    );
    Ok(())
}

/// The occurrence ceiling includes previous parses, and refusal restores the caller policy.
/// ´test:crate:parsed-existing-occupancy´
#[test]
fn parsed_existing_occupancy() -> TestResult {
    let (mut cas, header) = declared_store()?;
    parse_original(&mut cas, "first", "x+x", &header, &mut default_budget()?)?;
    let before = cas.canonical_snapshot(&mut default_budget()?)?;
    let limits = Limits {
        original_occurrences: 3,
        depth: 512,
        ..Limits::default()
    };
    let mut budget = Budget::new(limits.clone())?;
    let error = parse_original(&mut cas, "next", "x", &header, &mut budget)
        .err()
        .ok_or("the fourth occurrence must refuse")?;
    assert!(matches!(
        error.failure,
        ParseFailure::Fault(Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::OriginalOccurrences,
                limit: 3,
                needed: 4,
                ..
            },
            ..
        })
    ));
    assert_eq!(budget.limits(), &limits);
    assert!(budget.usage().work > 0);
    assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
    let mut budget = Budget::new(Limits {
        original_occurrences: 4,
        ..Limits::default()
    })?;
    parse_original(&mut cas, "next", "x", &header, &mut budget)?;
    assert_eq!(cas.counts().original_occurrences, 4);
    Ok(())
}
