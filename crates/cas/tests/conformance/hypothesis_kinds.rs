// SPDX-License-Identifier: AGPL-3.0-only
//! Formal kinds retain exact predicate identities and frame-qualified slots.

use crate::testing::budgets::derivation_budget as budget;
use crate::testing::builders::admit;
use crate::testing::common::TestResult;
use crate::testing::requests::Fixture;
use nrz_cas::domain::{AssumptionDraft, ContextDraft, HypothesisRef};
use nrz_cas::proof::{HYPOTHESIS_KINDS, HYPOTHESIS_SCHEMAS, HypothesisKind, PRIMITIVES};
use nrz_cas::syntax::{Draft, PredicateKind};
use nrz_cas::{Cas, Handle, Reference};

/// Every predicate constructor and compiled premise shape has a named assumed shape.
/// ´test:integration:formal-hypothesis-kinds-are-complete´
#[test]
fn formal_hypothesis_kinds_are_complete() {
    for primitive in PRIMITIVES {
        let kind = primitive.specification().premises.hypothesis_kind();
        assert!(HYPOTHESIS_KINDS.contains(&kind));
    }
    for (predicate, expected) in [
        (PredicateKind::Defined, HypothesisKind::DomainGuard),
        (PredicateKind::NonZero, HypothesisKind::NonZero),
        (PredicateKind::Positive, HypothesisKind::StrictSign),
        (PredicateKind::NonNegative, HypothesisKind::NonNegative),
        (PredicateKind::IntegerRange, HypothesisKind::IntegerRange),
        (
            PredicateKind::Differentiable,
            HypothesisKind::Differentiable,
        ),
        (PredicateKind::Equal, HypothesisKind::Equality),
        (PredicateKind::Symmetric, HypothesisKind::Symmetric),
        (PredicateKind::And, HypothesisKind::Conjunction),
        (PredicateKind::Or, HypothesisKind::Disjunction),
        (PredicateKind::Not, HypothesisKind::Negation),
        (PredicateKind::ForAll, HypothesisKind::Universal),
        (
            PredicateKind::Named("explicit".into()),
            HypothesisKind::Named,
        ),
    ] {
        assert_eq!(predicate.hypothesis_kind(), expected);
    }
    assert_eq!(HYPOTHESIS_KINDS.len(), HYPOTHESIS_SCHEMAS.len());
    for (index, kind) in HYPOTHESIS_KINDS.iter().enumerate() {
        let schema = kind.formal_schema();
        assert_eq!(*schema, HYPOTHESIS_SCHEMAS[index]);
        assert!(schema.label.starts_with("assum:cas:"));
        for earlier in &HYPOTHESIS_SCHEMAS[..index] {
            assert_ne!(schema.name, earlier.name);
            assert_ne!(schema.label, earlier.label);
        }
    }
}

/// Both guides contain precisely one head and exact contract for every formal hypothesis kind.
/// ´test:integration:formal-hypothesis-kinds-are-documented´
#[test]
fn formal_hypothesis_kinds_are_documented() {
    let expected: Vec<_> = HYPOTHESIS_SCHEMAS
        .iter()
        .map(|schema| {
            format!(
                "Formal hypothesis `{}`, version {}. {} Required premises: {}",
                schema.name, schema.version, schema.statement, schema.premises,
            )
        })
        .collect();
    for document in [
        include_str!("../../README.md"),
        include_str!("../../../../templates/crates/cas/README.md.j2"),
    ] {
        let actual: Vec<_> = document
            .lines()
            .filter(|line| line.starts_with("Formal hypothesis `"))
            .collect();
        assert_eq!(actual, expected);
        let heads: Vec<_> = document
            .lines()
            .filter(|line| line.starts_with("**") && line.contains("assum:cas:hyp-"))
            .collect();
        assert_eq!(heads.len(), HYPOTHESIS_SCHEMAS.len());
        for ((schema, paragraph), head) in HYPOTHESIS_SCHEMAS.iter().zip(&expected).zip(heads) {
            assert!(head.starts_with("**Assumption ("));
            assert!(head.ends_with(&format!("· `{}`", schema.label)));
            assert_eq!(
                document.matches(&format!("{head}\n\n{paragraph}")).count(),
                1
            );
        }
    }
}

/// Admit only the exact predicate syntax; no helper asserts that it is true.
fn predicate(
    fixture: &mut Fixture,
    kind: PredicateKind,
    arguments: &[Handle],
) -> TestResult<Handle> {
    let header = fixture.cas.context_header(&fixture.context)?;
    admit(
        &mut fixture.cas,
        Draft::Condition {
            header: Reference::Existing(header),
            scope: Vec::new(),
            kind,
            arguments: arguments.iter().cloned().map(Reference::Existing).collect(),
        },
        &mut budget()?,
    )
}

/// Retain one independently named assumption in a root or local frame.
fn frame(fixture: &mut Fixture, condition: &Handle, local: bool) -> TestResult<Handle> {
    let assumptions = vec![AssumptionDraft {
        key: "retained".into(),
        condition: Reference::Existing(condition.clone()),
    }];
    let draft = if local {
        ContextDraft::Local {
            parent: Reference::Existing(fixture.context.clone()),
            region: Reference::Existing(fixture.domain.clone()),
            hypotheses: assumptions,
        }
    } else {
        ContextDraft::Root {
            header: Reference::Existing(fixture.cas.context_header(&fixture.context)?),
            admissible: Reference::Existing(fixture.domain.clone()),
            assumptions,
        }
    };
    admit(&mut fixture.cas, Draft::Context(draft), &mut budget()?)
}

/// Root predicates, local membership and equal sibling slot numbers keep their exact kind and frame.
/// ´test:integration:qualified-hypothesis-kinds-retain-slots´
#[test]
fn qualified_hypothesis_kinds_retain_slots() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let subject = fixture.variable("x")?;
    let positive = predicate(
        &mut fixture,
        PredicateKind::Positive,
        std::slice::from_ref(&subject),
    )?;
    let nonzero = predicate(
        &mut fixture,
        PredicateKind::NonZero,
        std::slice::from_ref(&subject),
    )?;
    let root = frame(&mut fixture, &positive, false)?;
    let local = frame(&mut fixture, &nonzero, true)?;
    let sibling = frame(&mut fixture, &positive, true)?;
    for (context, slot, kind) in [
        (&root, 0, HypothesisKind::StrictSign),
        (&local, 0, HypothesisKind::RegionMembership),
        (&local, 1, HypothesisKind::NonZero),
        (&sibling, 1, HypothesisKind::StrictSign),
    ] {
        let reference = HypothesisRef {
            context: context.clone(),
            slot,
        };
        let qualified = reference.qualified(&fixture.cas)?;
        assert_eq!(qualified.context, *context);
        assert_eq!(qualified.slot, slot);
        assert_eq!(qualified.kind, kind);
        assert!(
            reference
                .qualified(&Cas::new(budget()?.limits().clone())?)
                .is_err()
        );
    }
    assert_ne!(local, sibling);
    for context in [root, local] {
        assert!(
            HypothesisRef {
                context,
                slot: u32::MAX
            }
            .qualified(&fixture.cas)
            .is_err()
        );
    }
    assert!(fixture.cas.predicate_hypothesis_kind(&subject).is_err());
    Ok(())
}

/// Only a literal-zero equality gains the zero kind; inspection preserves the graph.
/// ´test:integration:hypothesis-kinds-inspect-exact-zero´
#[test]
fn hypothesis_kinds_inspect_exact_zero() -> TestResult {
    let mut fixture = Fixture::new(&["x", "y"])?;
    let left = fixture.variable("x")?;
    let right = fixture.variable("y")?;
    let zero = fixture.number("0")?;
    let equality = predicate(&mut fixture, PredicateKind::Equal, &[left.clone(), right])?;
    let forward = predicate(
        &mut fixture,
        PredicateKind::Equal,
        &[left.clone(), zero.clone()],
    )?;
    let reverse = predicate(&mut fixture, PredicateKind::Equal, &[zero, left])?;
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    assert_eq!(
        fixture.cas.predicate_hypothesis_kind(&equality)?,
        HypothesisKind::Equality
    );
    assert_eq!(
        fixture.cas.predicate_hypothesis_kind(&forward)?,
        HypothesisKind::DeclaredZero
    );
    assert_eq!(
        fixture.cas.predicate_hypothesis_kind(&reverse)?,
        HypothesisKind::DeclaredZero
    );
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// Strict signs, weak signs, zero and unsigned nonzero remain distinct formal premise kinds.
/// ´test:integration:hypothesis-kinds-distinguish-real-signs´
#[test]
fn hypothesis_kinds_distinguish_real_signs() -> TestResult {
    use nrz_cas::proof::Statement;
    use nrz_cas::solving::{RealSignConclusion, SolveStatement, StatementRealSign};
    let mut fixture = Fixture::new(&["x"])?;
    let subject = fixture.variable("x")?;
    for (conclusion, expected) in [
        (RealSignConclusion::Positive, HypothesisKind::StrictSign),
        (RealSignConclusion::Negative, HypothesisKind::StrictSign),
        (RealSignConclusion::NonNegative, HypothesisKind::NonNegative),
        (RealSignConclusion::NonPositive, HypothesisKind::NonPositive),
        (RealSignConclusion::Zero, HypothesisKind::DeclaredZero),
        (RealSignConclusion::NonZero, HypothesisKind::NonZero),
    ] {
        let statement = Statement::Solve(Box::new(SolveStatement::RealSign(StatementRealSign {
            subject: subject.clone(),
            region: fixture.domain.clone(),
            conclusion,
        })));
        assert_eq!(fixture.cas.statement_hypothesis_kind(&statement)?, expected);
    }
    Ok(())
}
