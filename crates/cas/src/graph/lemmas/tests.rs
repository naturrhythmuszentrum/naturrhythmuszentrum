// SPDX-License-Identifier: AGPL-3.0-only
//! Mixed retention measures each authority owner and its caller reservation separately.

mod fixture;
mod identity;
mod retained_graph;

use crate::budget::{BudgetKind, Limits};
use crate::decision_kernel::cache::retained;
use crate::graph::Vertex;
use crate::lemma_cache::AnonymousLemmaCache;
use crate::testing::common::TestResult;
use crate::{Fault, FaultKind, Rational};
use fixture::{Fixture, ledger};

/// Every checked graph kind contributes once; portable proof retention has a separate owner.
/// ´test:unit:retained-judgment-mixed-payload´
#[test]
fn retained_judgment_mixed_payload() -> TestResult {
    let mut fixture = Fixture::new()?;
    let before = fixture.cas.counts();
    let mut budget = ledger()?;
    budget.with_retained_payload(31, super::AT, |budget| fixture.retain_graph(budget))?;
    assert_eq!(
        fixture.cas.counts().payload_bytes - before.payload_bytes,
        4_600
    );
    assert_eq!(budget.checked_records.bytes, 4_664);
    let mut cache = AnonymousLemmaCache::new();
    cache.replay(
        &mut fixture.cas,
        &fixture.witness.bytes,
        fixture.witness.judgment(),
        &mut budget,
    )?;
    let payloads = fixture
        .cas
        .state
        .graph
        .node_weights()
        .filter_map(|node| {
            if let Vertex::AnonymousLemma(node) = node {
                Some(node.payload().expect("checked payload"))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        (
            payloads,
            fixture.cas.retained_judgment_count(),
            fixture.cas.retained_judgment_bytes(),
            fixture.cas.counts().payload_bytes - before.payload_bytes,
            cache.len(),
            cache.retained_bytes()
        ),
        (vec![520, 904, 936, 2_240], 4, 4_600, 5_548, 1, 5_672)
    );
    Ok(())
}

/// The complete graph/cache/caller aggregate has an exact boundary after prior Work.
/// ´test:unit:retained-judgment-mixed-reservation´
#[test]
fn retained_judgment_mixed_reservation() -> TestResult {
    for short in [0, 1] {
        mixed_reservation(short)?;
    }
    Ok(())
}

/// A refreshed shared reservation must not reserve the graph aggregate twice.
fn mixed_reservation(short: u64) -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.retain_graph(&mut ledger()?)?;
    let mut cache = AnonymousLemmaCache::new();
    cache.replay(
        &mut fixture.cas,
        &fixture.witness.bytes,
        fixture.witness.judgment(),
        &mut ledger()?,
    )?;
    let before = fixture.cas.counts();
    assert_eq!(fixture.cas.retained_judgment_bytes(), 4_600);
    assert_eq!(cache.retained_bytes(), 5_672);
    let mut budget = crate::budget::Budget::new(Limits {
        staged_payload_bytes: 10_367 - short,
        work: 11,
        ..Limits::default()
    })?;
    budget.work(7, super::AT)?;
    retained(&fixture.cas, &mut budget, |_| Ok(()))?;
    let result = budget.with_retained_payload(5_672 + 31, super::AT, |budget| {
        budget.check(BudgetKind::StagedPayloadBytes, 0, super::AT)?;
        fixture
            .cas
            .prepare_constant(&fixture.context, &fixture.one, budget)
            .map(|_| ())
    });
    if short == 0 {
        result?;
        assert_eq!(budget.usage().work, 11);
    } else {
        assert!(matches!(
            result.err().ok_or("short aggregate accepted")?.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::StagedPayloadBytes,
                limit: 10_366,
                needed: 10_367,
                ..
            }
        ));
        assert_eq!(budget.usage().work, 7);
    }
    assert_eq!(budget.checked_records.bytes, 4_664);
    assert_eq!(fixture.cas.counts(), before);
    assert_eq!(cache.len(), 1);
    Ok(())
}

/// A constant added after scalar admission refreshes the existing registration immediately.
/// ´test:unit:retained-judgment-registration-growth´
#[test]
fn retained_judgment_registration_growth() -> TestResult {
    let mut fixture = Fixture::new()?;
    let mut budget = ledger()?;
    fixture
        .cas
        .check_rational_enclosure(&fixture.scalar, &mut budget)?;
    assert_eq!(fixture.cas.retained_judgment_bytes(), 904);
    assert_eq!(budget.checked_records.bytes, 968);
    fixture
        .cas
        .prepare_constant(&fixture.context, &fixture.one, &mut budget)?;
    assert_eq!(fixture.cas.retained_judgment_count(), 2);
    assert_eq!(fixture.cas.retained_judgment_bytes(), 1_424);
    assert_eq!(budget.checked_records.bytes, 1_488);
    Ok(())
}

/// An enclosure citation borrows the same shared account beside portable and caller payload.
/// ´test:unit:retained-judgment-enclosure-reservation´
#[test]
fn retained_judgment_enclosure_reservation() -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.retain_graph(&mut ledger()?)?;
    let mut cache = AnonymousLemmaCache::new();
    cache.replay(
        &mut fixture.cas,
        &fixture.witness.bytes,
        fixture.witness.judgment(),
        &mut ledger()?,
    )?;
    let key = fixture
        .cas
        .state
        .graph
        .node_weights()
        .find_map(|vertex| {
            if let Vertex::AnonymousLemma(node) = vertex
                && let super::AnonymousLemma::Enclosure(node) = node.as_ref()
            {
                Some(node.inventory.key.as_slice())
            } else {
                None
            }
        })
        .ok_or("enclosure key")?;
    let mut budget = crate::budget::Budget::new(Limits {
        staged_payload_bytes: 10_367,
        ..Limits::default()
    })?;
    retained(&fixture.cas, &mut budget, |_| Ok(()))?;
    budget.with_retained_payload(5_672 + 31, super::AT, |budget| -> Result<(), Fault> {
        assert!(fixture.cas.retained_enclosure(key, budget)?.is_some());
        Ok(())
    })?;
    assert_eq!(budget.checked_records.bytes, 4_664);
    assert_eq!(cache.retained_bytes(), 5_672);
    Ok(())
}

/// Warm public callers retain one payload per kind and one registration for their shared owner.
/// ´test:unit:retained-judgment-mixed-warm-callers´
#[test]
fn retained_judgment_mixed_warm_callers() -> TestResult {
    let mut fixture = Fixture::new()?;
    let mut budget = ledger()?;
    fixture.retain_graph(&mut budget)?;
    let before = fixture.cas.canonical_snapshot(&mut ledger()?)?;
    budget.with_retained_payload(31, super::AT, |budget| fixture.retain_graph(budget))?;
    assert_eq!(fixture.cas.retained_judgment_count(), 4);
    assert_eq!(fixture.cas.retained_judgment_bytes(), 4_600);
    assert_eq!(budget.checked_records.bytes, 4_664);
    assert_eq!(fixture.cas.canonical_snapshot(&mut ledger()?)?, before);
    Ok(())
}

/// Refreshing one owner preserves other owners and charges every visited registration.
/// ´test:unit:retained-judgment-multiple-owners´
#[test]
fn retained_judgment_multiple_owners() -> TestResult {
    let mut fixtures = [Fixture::new()?, Fixture::new()?];
    let mut budget = crate::budget::Budget::new(Limits {
        staged_payload_bytes: 9_359,
        ..Limits::default()
    })?;
    for fixture in &mut fixtures {
        fixture.retain_graph(&mut ledger()?)?;
        retained(&fixture.cas, &mut budget, |_| Ok(()))?;
    }
    budget.with_retained_payload(31, super::AT, |budget| -> Result<(), Fault> {
        for fixture in &mut fixtures {
            fixture
                .cas
                .prepare_constant(&fixture.context, &fixture.one, budget)?;
        }
        Ok(())
    })?;
    assert_eq!(budget.checked_records.bytes, 9_328);
    assert_eq!(budget.usage().work, 9);
    Ok(())
}

/// Late enclosing refusal removes the new judgment while preserving earlier evidence and Work.
/// ´test:unit:retained-judgment-late-refusal´
#[test]
fn retained_judgment_late_refusal() -> TestResult {
    let mut fixture = Fixture::new()?;
    let mut budget = ledger()?;
    fixture.retain_graph(&mut budget)?;
    let header = fixture.cas.expression_header(&fixture.one, &mut budget)?;
    let two = crate::testing::native::term(
        &mut fixture.cas,
        &header,
        crate::model::Operator::Rational(Rational::from_ratio(2, 1, &mut ledger()?)?),
        &[],
    )?;
    let before = fixture.cas.canonical_snapshot(&mut ledger()?)?;
    let mut spent = 0;
    let result: Result<(), Fault> = fixture.cas.transaction(&mut budget, |cas, budget| {
        budget.with_retained_payload(31, super::AT, |budget| {
            cas.prepare_constant(&fixture.context, &two, budget)?;
            assert_eq!(budget.checked_records.bytes, 5_184);
            spent = budget.usage().work;
            budget.work(budget.limits().work, super::AT)
        })
    });
    assert!(matches!(
        result.err().ok_or("late refusal accepted")?.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::Work,
            ..
        }
    ));
    assert_eq!(budget.usage().work, spent);
    assert_eq!(fixture.cas.retained_judgment_count(), 4);
    assert_eq!(fixture.cas.retained_judgment_bytes(), 4_600);
    assert_eq!(fixture.cas.canonical_snapshot(&mut ledger()?)?, before);
    // A real graph rollback invalidates this owner's old ledger registration.
    assert_eq!(budget.checked_records.bytes, 0);
    retained(&fixture.cas, &mut budget, |_| Ok(()))?;
    assert_eq!(budget.checked_records.bytes, 4_664);
    Ok(())
}
