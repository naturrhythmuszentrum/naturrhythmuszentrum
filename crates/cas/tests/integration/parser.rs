// SPDX-License-Identifier: AGPL-3.0-only
//! Parsed original inspection and portable equality lifecycles under default budgets.

use std::collections::BTreeMap;

use nrz_cas::limits::Limits;
use nrz_cas::proof::{DomainContract, Judgment, Primitive, Statement};
use nrz_cas::source::{OriginalForm, SyntaxRole, Visit, WalkEvent, parse_original};
use nrz_cas::syntax::{Draft, Field, OccurrenceDraft, Span};
use nrz_cas::{Cas, Handle, Reference};

use crate::testing::builders::{core_context, environment};
use crate::testing::common::{TestResult, default_budget};
use crate::testing::witness::equality;

/// Source and theorem construction both retain production defaults.
fn setup(keys: &[&str]) -> TestResult<(Cas, Handle, Handle)> {
    let mut cas = Cas::new(Limits::default())?;
    let context = core_context(
        &mut cas,
        environment(Field::Real, keys),
        Vec::new(),
        &mut default_budget()?,
    )?;
    let header = cas.context_header(&context)?;
    Ok((cas, context, header))
}

/// Every returned visit is an original occurrence, with no implicit generated materialization.
fn visits(cas: &Cas, root: &Handle) -> TestResult<Vec<Visit>> {
    Ok(cas
        .walk_original(root, &mut default_budget()?)?
        .events
        .into_iter()
        .filter_map(|event| match event {
            WalkEvent::Visit(visit) => Some(visit),
            WalkEvent::EnterBinder { .. } => None,
        })
        .collect())
}

/// Nested grouping, signed powers and rational exponent operands retain their exact byte spans.
/// ´test:integration:parsed-nested-spans´
#[test]
fn parsed_nested_spans() -> TestResult {
    let (mut cas, _, header) = setup(&["x", "y"])?;
    let original = parse_original(
        &mut cas,
        "nested",
        "  -(x + y) * x^(1/2)  ",
        &header,
        &mut default_budget()?,
    )?;
    let actual: Vec<_> = visits(&cas, &original.root)?
        .into_iter()
        .map(|visit| {
            let source = visit.source.ok_or("missing original source")?;
            assert_eq!(source.key, "nested");
            Ok((visit.path, source.span.ok_or("missing original span")?))
        })
        .collect::<TestResult<_>>()?;
    let expected = [
        (vec![0, 0, 0], 4, 5),
        (vec![0, 0, 1], 8, 9),
        (vec![0, 0], 3, 10),
        (vec![0], 2, 10),
        (vec![1, 0], 13, 14),
        (vec![1, 1], 15, 20),
        (vec![1], 13, 20),
        (vec![], 2, 20),
    ];
    assert_eq!(
        actual,
        expected
            .into_iter()
            .map(|(path, start, end)| (path, Span { start, end }))
            .collect::<Vec<_>>()
    );
    Ok(())
}

/// Reconstruct ordinary occurrence drafts using descriptor dependency order and walked locations.
fn reconstruct(cas: &mut Cas, original: &OriginalForm, text: &str) -> TestResult<Handle> {
    let walked = visits(cas, &original.root)?;
    let mut drafts = vec![Draft::Source {
        key: "roundtrip".into(),
        text: text.into(),
    }];
    let mut children = BTreeMap::new();
    for visit in walked {
        let descriptor = cas.describe_syntax(&visit.expression, &mut default_budget()?)?;
        let mut references = Vec::new();
        for (index, dependency) in descriptor.dependencies.iter().enumerate() {
            let index = u32::try_from(index)?;
            assert_eq!(dependency.role, SyntaxRole::Operand(index));
            let mut path = visit.path.clone();
            path.push(index);
            let (reference, syntax) = children.get(&path).ok_or("missing earlier child")?;
            assert_eq!(syntax, &dependency.target);
            references.push(Reference::Local(*reference));
        }
        let index = u32::try_from(drafts.len())?;
        drafts.push(Draft::Occurrence(OccurrenceDraft {
            expression: Reference::Existing(visit.expression.clone()),
            source: Some(Reference::Local(0)),
            span: visit.source.ok_or("original source")?.span,
            children: references,
            provenance: None,
            binder_names: Vec::new(),
        }));
        children.insert(visit.path, (index, visit.expression));
    }
    cas.admit(&drafts, &mut default_budget()?)?
        .last()
        .cloned()
        .ok_or_else(|| "rebuilt root missing".into())
}

/// Parse, walk, inspect and reconstruct all ordered occurrences without changing their spans.
/// ´test:integration:parsed-inspection-roundtrip´
#[test]
fn parsed_inspection_roundtrip() -> TestResult {
    let (mut cas, _, header) = setup(&["x", "y"])?;
    let text = "+((x + y)/(x - y))^(-1/2)";
    let original = parse_original(&mut cas, "roundtrip", text, &header, &mut default_budget()?)?;
    let rebuilt = reconstruct(&mut cas, &original, text)?;
    let first = visits(&cas, &original.root)?;
    let second = visits(&cas, &rebuilt)?;
    assert_eq!(first.len(), second.len());
    for (left, right) in first.iter().zip(&second) {
        assert_ne!(left.occurrence, right.occurrence);
        assert_eq!(left.expression, right.expression);
        assert_eq!(left.path, right.path);
        assert_eq!(left.source, right.source);
        assert_eq!(left.scope, right.scope);
    }
    Ok(())
}

/// Export a theorem over parsed syntax, then replay against an independently parsed expectation.
fn portable_parse() -> TestResult<(Vec<u8>, Vec<u8>)> {
    let (mut cas, context, header) = setup(&["x"])?;
    let text = " x + 0.5*x ";
    let original = parse_original(&mut cas, "portable", text, &header, &mut default_budget()?)?;
    let snapshot = cas.canonical_snapshot(&mut default_budget()?)?;
    let domain = cas.admissible_region(&context)?;
    let proposal = equality(
        &context,
        &domain,
        Primitive::Reflexivity,
        &original.syntax,
        &original.syntax,
    );
    let witness = cas.check_proof(&context, &[proposal], &mut default_budget()?)?;
    let bytes = cas.export_witness(&witness, &mut default_budget()?)?;
    let (mut fresh, context, header) = setup(&["x"])?;
    let expected_form = parse_original(
        &mut fresh,
        "portable",
        text,
        &header,
        &mut default_budget()?,
    )?;
    let expected = Judgment {
        context: context.clone(),
        layer: nrz_cas::domain::Layer::Pointwise,
        space: None,
        statement: Statement::Equality {
            left: expected_form.syntax.clone(),
            right: expected_form.syntax,
            domain: fresh.admissible_region(&context)?,
            domain_contract: DomainContract::DefinedAndEqual,
        },
        residuals: Vec::new(),
    };
    let checked = fresh.replay(&bytes, &expected, &mut default_budget()?)?;
    assert_eq!(
        fresh.export_witness(&checked, &mut default_budget()?)?,
        bytes
    );
    let source_spans: Vec<_> = visits(&cas, &original.root)?
        .into_iter()
        .map(|visit| (visit.path, visit.source))
        .collect();
    let replay_spans: Vec<_> = visits(&fresh, &expected_form.root)?
        .into_iter()
        .map(|visit| (visit.path, visit.source))
        .collect();
    assert_eq!(source_spans, replay_spans);
    Ok((snapshot, bytes))
}

/// Repeated parses produce identical source graphs and portable mathematical witness bytes.
/// ´test:integration:parsed-deterministic-replay´
#[test]
fn parsed_deterministic_replay() -> TestResult {
    assert_eq!(portable_parse()?, portable_parse()?);
    Ok(())
}
