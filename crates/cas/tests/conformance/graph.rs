// SPDX-License-Identifier: AGPL-3.0-only
//! Public-contract checks for graph.

use nrz_cas::{
    AssumptionDraft, AtomicSort, Binding, Budget, Cas, Declaration, Draft, Environment,
    ExpressionDraft, Field, Limits, LimitsOverride, OccurrenceDraft, Operation, Operator, Outcome,
    PredicateKind, Rational, Reference, Request, Sort, Span, Substitution, WalkEvent,
};

use crate::testing::budgets::graph_budget as budget;
use crate::testing::common::{TestResult, real};
use crate::testing::graph::{admit, expression, fixture, occurrence, shared_original};

/// Shared mathematical syntax never erases either original operand position.
/// ´test:integration:original-sharing´
#[test]
fn original_sharing() -> TestResult {
    let (cas, _, root) = shared_original()?;
    let walk = cas.walk_original(&root, &mut budget()?)?;
    assert_eq!(walk.receipt.visited(), 3);
    assert!(walk.receipt.is_original());
    let visits: Vec<_> = walk
        .events
        .iter()
        .filter_map(|event| match event {
            WalkEvent::Visit(visit) => Some(visit),
            WalkEvent::EnterBinder { .. } => None,
        })
        .collect();
    let paths: Vec<_> = visits.iter().map(|visit| visit.path.clone()).collect();
    assert_eq!(paths, vec![vec![0], vec![1], vec![]]);
    let first = visits.first().ok_or("missing first visit")?;
    let second = visits.get(1).ok_or("missing second visit")?;
    assert_eq!(first.expression, second.expression);
    assert_ne!(first.occurrence, second.occurrence);
    assert_eq!(
        second.source.as_ref().and_then(|source| source.span),
        Some(Span { start: 2, end: 3 }),
    );
    Ok(())
}

/// Identical input bytes and admission order have identical address-free snapshots.
/// ´test:integration:graph-determinism´
#[test]
fn graph_determinism() -> TestResult {
    let (first, _, _) = shared_original()?;
    let (second, _, _) = shared_original()?;
    assert_eq!(
        first.canonical_snapshot(&mut budget()?)?,
        second.canonical_snapshot(&mut budget()?)?,
    );
    Ok(())
}

/// A bad UTF-8 span rolls back an earlier source draft and its namespace serial.
/// ´test:integration:transaction-rollback´
#[test]
fn transaction_rollback() -> TestResult {
    let (mut cas, context) = fixture()?;
    let x = expression(&mut cas, &context, Operator::Free("x".into()), real(), &[])?;
    let before = cas.canonical_snapshot(&mut budget()?)?;
    let counts = cas.counts();
    let rejected = [
        Draft::Source {
            key: "unicode".into(),
            text: "π".into(),
        },
        Draft::Occurrence(OccurrenceDraft {
            expression: Reference::Existing(x),
            source: Some(Reference::Local(0)),
            span: Some(Span { start: 0, end: 1 }),
            children: Vec::new(),
            provenance: None,
            binder_names: Vec::new(),
        }),
    ];
    assert!(cas.admit(&rejected, &mut budget()?).is_err());
    assert_eq!(cas.counts(), counts);
    assert_eq!(cas.canonical_snapshot(&mut budget()?)?, before);
    let source = admit(
        &mut cas,
        Draft::Source {
            key: "unicode".into(),
            text: "π".into(),
        },
    )?;
    assert_eq!(source.logical_id().serial, 0);
    Ok(())
}

/// A foreign handle, a forward reference and a wrong sort fail atomically.
/// ´test:integration:draft-refusals´
#[test]
fn draft_refusals() -> TestResult {
    let (mut cas, context) = fixture()?;
    let (other, foreign) = fixture()?;
    assert!(cas.expression(&foreign).is_err());
    assert!(other.expression(&context).is_err());
    let before = cas.canonical_snapshot(&mut budget()?)?;
    for reference in [Reference::Existing(foreign), Reference::Local(1)] {
        let draft = Draft::Expression(ExpressionDraft {
            operator: Operator::Free("x".into()),
            sort: real(),
            context: reference,
            scope: Vec::new(),
            binding: None,
            operands: Vec::new(),
        });
        assert!(cas.admit(&[draft], &mut budget()?).is_err());
    }
    assert!(
        expression(
            &mut cas,
            &context,
            Operator::Free("x".into()),
            Sort::Value(AtomicSort::Boolean),
            &[],
        )
        .is_err()
    );
    assert_eq!(cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// Coverage is required before a request can start independently checked simplification.
/// ´test:integration:request-coverage´
#[test]
fn request_coverage() -> TestResult {
    let (mut cas, context, root) = shared_original()?;
    let mut tiny = Budget::new(Limits {
        original_occurrences: 2,
        ..Limits::default()
    })?;
    let partial = cas
        .walk_original(&root, &mut tiny)
        .err()
        .ok_or("walk exceeded its limit")?;
    assert_eq!(partial.events.len(), 2);
    let receipt = cas.walk_original(&root, &mut budget()?)?.receipt;
    let mut request = Request {
        operation: Operation::Simplify,
        context,
        inputs: vec![root],
        receipts: Vec::new(),
        limits: LimitsOverride::default(),
    };
    assert!(matches!(cas.run(&request), Outcome::Invalid { .. }));
    request.receipts.push(receipt);
    assert!(matches!(cas.run(&request), Outcome::Verified { .. }));
    Ok(())
}

/// Source-free derived walks retain provenance without inventing original spans.
/// ´test:integration:generated-coverage´
#[test]
fn generated_coverage() -> TestResult {
    let (mut cas, context) = fixture()?;
    let x = expression(&mut cas, &context, Operator::Free("x".into()), real(), &[])?;
    let sum = expression(&mut cas, &context, Operator::Add, real(), &[x.clone(), x])?;
    let root = cas.materialize_derived(&sum, &sum, &mut budget()?)?;
    assert!(cas.walk_original(&root, &mut budget()?).is_err());
    let walk = cas.walk_derived(&root, &mut budget()?)?;
    assert_eq!(walk.receipt.visited(), 3);
    assert!(!walk.receipt.is_original());
    for event in walk.events {
        if let WalkEvent::Visit(visit) = event {
            assert!(visit.source.is_none());
        }
    }
    Ok(())
}

/// Substitution is simultaneous: inserted terms are not substituted a second time.
/// ´test:integration:simultaneous-substitution´
#[test]
fn simultaneous_substitution() -> TestResult {
    let (mut cas, context) = fixture()?;
    let x = expression(&mut cas, &context, Operator::Free("x".into()), real(), &[])?;
    let y = expression(&mut cas, &context, Operator::Free("y".into()), real(), &[])?;
    let one = expression(
        &mut cas,
        &context,
        Operator::Rational(Rational::one()),
        real(),
        &[],
    )?;
    let result = cas.substitute(
        &x,
        &[
            Substitution {
                key: "x".into(),
                replacement: y.clone(),
            },
            Substitution {
                key: "y".into(),
                replacement: one,
            },
        ],
        &mut budget()?,
    )?;
    assert_eq!(result, y);
    Ok(())
}

/// Instantiating a bound slot removes its frame and restores the caller's free identity.
/// ´test:integration:bound-instantiation´
#[test]
fn bound_instantiation() -> TestResult {
    let (mut cas, context) = fixture()?;
    let x = expression(&mut cas, &context, Operator::Free("x".into()), real(), &[])?;
    let bound = admit(
        &mut cas,
        Draft::Expression(ExpressionDraft {
            operator: Operator::Bound { depth: 0, slot: 0 },
            sort: real(),
            context: Reference::Existing(context),
            scope: vec![vec![AtomicSort::Scalar(Field::Real)]],
            binding: None,
            operands: Vec::new(),
        }),
    )?;
    assert_eq!(
        cas.instantiate_bound(&bound, std::slice::from_ref(&x), &mut budget()?)?,
        x
    );
    assert!(cas.instantiate_bound(&bound, &[], &mut budget()?).is_err());
    Ok(())
}

/// An active call cannot override the store's permanent source-byte ceiling.
/// ´test:integration:permanent-storage-limit´
#[test]
fn permanent_storage_limit() -> TestResult {
    let mut cas = Cas::new(Limits {
        source_bytes: 1,
        ..Limits::default()
    })?;
    let mut ledger = budget()?;
    let original_limit = ledger.limits().source_bytes;
    assert!(
        cas.admit(
            &[Draft::Source {
                key: "a".into(),
                text: "ab".into()
            }],
            &mut ledger,
        )
        .is_err()
    );
    assert_eq!(cas.counts().vertices, 0);
    assert_eq!(ledger.limits().source_bytes, original_limit);
    let source = admit(
        &mut cas,
        Draft::Source {
            key: "a".into(),
            text: "a".into(),
        },
    )?;
    assert_eq!(source.logical_id().serial, 0);
    Ok(())
}

/// A recognized atom and its explicit negation cannot enter the same context.
/// ´test:integration:explicit-contradiction´
#[test]
fn explicit_contradiction() -> TestResult {
    let (mut cas, context) = fixture()?;
    let x = expression(&mut cas, &context, Operator::Free("x".into()), real(), &[])?;
    let predicate = expression(
        &mut cas,
        &context,
        Operator::Predicate(PredicateKind::NonZero),
        Sort::Value(AtomicSort::Boolean),
        std::slice::from_ref(&x),
    )?;
    let positive = admit(
        &mut cas,
        Draft::Condition {
            kind: PredicateKind::NonZero,
            context: Reference::Existing(context.clone()),
            scope: Vec::new(),
            arguments: vec![Reference::Existing(x)],
        },
    )?;
    let negative = admit(
        &mut cas,
        Draft::Condition {
            kind: PredicateKind::Not,
            context: Reference::Existing(context),
            scope: Vec::new(),
            arguments: vec![Reference::Existing(predicate)],
        },
    )?;
    let environment = Environment {
        declarations: ["x", "y"]
            .into_iter()
            .map(|key| Declaration {
                key: key.to_owned(),
                display_name: key.to_owned(),
                sort: real(),
            })
            .collect(),
        ..Environment::default()
    };
    let before = cas.canonical_snapshot(&mut budget()?)?;
    assert!(
        admit(
            &mut cas,
            Draft::Context {
                environment,
                assumptions: vec![
                    AssumptionDraft {
                        key: "positive".into(),
                        condition: Reference::Existing(positive)
                    },
                    AssumptionDraft {
                        key: "negative".into(),
                        condition: Reference::Existing(negative)
                    },
                ],
            },
        )
        .is_err()
    );
    assert_eq!(cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// An occurrence can occupy only one original operand position, even under the same parent.
/// ´test:integration:occurrence-parent-refusal´
#[test]
fn occurrence_parent_refusal() -> TestResult {
    let (mut cas, context) = fixture()?;
    let source = admit(
        &mut cas,
        Draft::Source {
            key: "input".into(),
            text: "x+x".into(),
        },
    )?;
    let x = expression(&mut cas, &context, Operator::Free("x".into()), real(), &[])?;
    let sum = expression(
        &mut cas,
        &context,
        Operator::Add,
        real(),
        &[x.clone(), x.clone()],
    )?;
    let leaf = occurrence(&mut cas, &x, &source, None, &[])?;
    let counts = cas.counts();
    assert!(occurrence(&mut cas, &sum, &source, None, &[leaf.clone(), leaf]).is_err());
    assert_eq!(cas.counts(), counts);
    Ok(())
}

/// Unsupported executable fields remain representable, while composite characteristics refuse.
/// ´test:integration:field-boundary´
#[test]
fn field_boundary() -> TestResult {
    let mut cas = Cas::new(Limits::default())?;
    assert!(
        admit(
            &mut cas,
            Draft::Context {
                environment: Environment {
                    field: Field::PositiveCharacteristic(4),
                    ..Environment::default()
                },
                assumptions: Vec::new(),
            },
        )
        .is_err()
    );
    let field = Field::PositiveCharacteristic(5);
    let sort = Sort::Value(AtomicSort::Scalar(field.clone()));
    let context = admit(
        &mut cas,
        Draft::Context {
            environment: Environment {
                field,
                declarations: vec![Declaration {
                    key: "x".into(),
                    display_name: "x".into(),
                    sort: sort.clone(),
                }],
                ..Environment::default()
            },
            assumptions: Vec::new(),
        },
    )?;
    let x = expression(&mut cas, &context, Operator::Free("x".into()), sort, &[])?;
    let source = admit(
        &mut cas,
        Draft::Source {
            key: "input".into(),
            text: "x".into(),
        },
    )?;
    let root = occurrence(&mut cas, &x, &source, None, &[])?;
    let receipt = cas.walk_original(&root, &mut budget()?)?.receipt;
    assert!(matches!(
        cas.run(&Request {
            operation: Operation::Differentiate("x".into()),
            context,
            inputs: vec![root],
            receipts: vec![receipt],
            limits: LimitsOverride::default(),
        }),
        Outcome::Unsupported { .. },
    ));
    Ok(())
}

/// Lifting a replacement shifts references to outer frames while keeping the local index local.
/// ´test:integration:outer-frame-lifting´
#[test]
fn outer_frame_lifting() -> TestResult {
    let (mut cas, context) = fixture()?;
    let outer = vec![vec![AtomicSort::Scalar(Field::Real)]];
    let replacement = admit(
        &mut cas,
        Draft::Expression(ExpressionDraft {
            operator: Operator::Bound { depth: 0, slot: 0 },
            sort: real(),
            context: Reference::Existing(context.clone()),
            scope: outer.clone(),
            binding: None,
            operands: Vec::new(),
        }),
    )?;
    let mut nested = outer.clone();
    nested.push(vec![AtomicSort::Integer]);
    let body = admit(
        &mut cas,
        Draft::Expression(ExpressionDraft {
            operator: Operator::Free("x".into()),
            sort: real(),
            context: Reference::Existing(context.clone()),
            scope: nested,
            binding: None,
            operands: Vec::new(),
        }),
    )?;
    let endpoint = admit(
        &mut cas,
        Draft::Expression(ExpressionDraft {
            operator: Operator::Rational(Rational::one()),
            sort: Sort::Value(AtomicSort::Integer),
            context: Reference::Existing(context.clone()),
            scope: outer.clone(),
            binding: None,
            operands: Vec::new(),
        }),
    )?;
    let sum = admit(
        &mut cas,
        Draft::Expression(ExpressionDraft {
            operator: Operator::Sum,
            sort: real(),
            context: Reference::Existing(context),
            scope: outer,
            binding: Some(Binding {
                sorts: vec![AtomicSort::Integer],
                body_operands: vec![0],
            }),
            operands: vec![
                Reference::Existing(body),
                Reference::Existing(endpoint.clone()),
                Reference::Existing(endpoint),
            ],
        }),
    )?;
    let result = cas.substitute(
        &sum,
        &[Substitution {
            key: "x".into(),
            replacement,
        }],
        &mut budget()?,
    )?;
    let operands = cas.operands(&result, &mut budget()?)?;
    let lifted = operands.first().ok_or("sum lost its body")?;
    assert_eq!(
        cas.expression(lifted)?.0,
        &Operator::Bound { depth: 1, slot: 0 }
    );
    Ok(())
}

/// The first staged draft is also discarded when a later draft exceeds the vertex ceiling.
/// ´test:integration:storage-boundary-rollback´
#[test]
fn storage_boundary_rollback() -> TestResult {
    let mut cas = Cas::new(Limits {
        vertices: 1,
        ..Limits::default()
    })?;
    let drafts = [
        Draft::Source {
            key: "first".into(),
            text: "a".into(),
        },
        Draft::Source {
            key: "second".into(),
            text: "b".into(),
        },
    ];
    let mut ledger = budget()?;
    assert!(cas.admit(&drafts, &mut ledger).is_err());
    assert_eq!(cas.counts().vertices, 0);
    let handle = admit(&mut cas, drafts.first().ok_or("missing draft")?.clone())?;
    assert_eq!(handle.logical_id().serial, 0);
    assert_eq!(cas.counts().vertices, 1);
    Ok(())
}

/// An equality request rejects differently sorted targets despite complete coverage of each.
/// ´test:integration:equality-target-sort´
#[test]
fn equality_target_sort() -> TestResult {
    let (mut cas, context) = fixture()?;
    let source = admit(
        &mut cas,
        Draft::Source {
            key: "input".into(),
            text: "1".into(),
        },
    )?;
    let scalar = expression(
        &mut cas,
        &context,
        Operator::Rational(Rational::one()),
        real(),
        &[],
    )?;
    let integer = expression(
        &mut cas,
        &context,
        Operator::Rational(Rational::one()),
        Sort::Value(AtomicSort::Integer),
        &[],
    )?;
    let left = occurrence(&mut cas, &scalar, &source, None, &[])?;
    let right = occurrence(&mut cas, &integer, &source, None, &[])?;
    let receipts = vec![
        cas.walk_original(&left, &mut budget()?)?.receipt,
        cas.walk_original(&right, &mut budget()?)?.receipt,
    ];
    let before = cas.canonical_snapshot(&mut budget()?)?;
    assert!(matches!(
        cas.run(&Request {
            operation: Operation::Equal,
            context,
            inputs: vec![left, right],
            receipts,
            limits: LimitsOverride::default(),
        }),
        Outcome::Invalid { .. },
    ));
    assert_eq!(cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}
