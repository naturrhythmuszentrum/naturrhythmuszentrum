// SPDX-License-Identifier: AGPL-3.0-only
//! Sealed bytes survive invalidation, while reuse requires the current receiving namespace.

use super::{Change, proof};
use crate::budget::{Budget, BudgetKind, Limits};
use crate::lemma_cache::{AT, AnonymousLemmaCache};
use crate::testing::common::TestResult;
use crate::{Cas, Fault, FaultKind, InvalidCode};

/// A changed expected scope cannot borrow the original proof, even after a valid cache admission.
/// ´test:unit:lemma-cache-scope-refusal´
#[test]
fn lemma_cache_scope_refusal() -> TestResult {
    let mut cas = Cas::new(Limits::default())?;
    let original = proof(&mut cas, Change::None)?;
    let changed = proof(&mut cas, Change::Context)?;
    let mut cache = AnonymousLemmaCache::new();
    cache.replay(
        &mut cas,
        &original.bytes,
        original.judgment(),
        &mut ledger()?,
    )?;
    let before = cas.canonical_snapshot(&mut ledger()?)?;
    let mut spent = ledger()?;
    let fault = cache
        .replay(&mut cas, &original.bytes, changed.judgment(), &mut spent)
        .err()
        .ok_or("changed scope borrowed an earlier proof")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.retained_bytes(), 5_864);
    assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
    assert!(spent.usage().work > 0);
    rechecked(&mut cache, &mut cas, &original)?;
    Ok(())
}

/// A foreign expected judgment refuses before another receiving store can cite its handles.
/// ´test:unit:lemma-cache-owner-refusal´
#[test]
fn lemma_cache_owner_refusal() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    let mut first = Cas::new(Limits::default())?;
    let original = proof(&mut first, Change::None)?;
    let mut second = Cas::new(Limits::default())?;
    let local = proof(&mut second, Change::None)?;
    let mut cache = AnonymousLemmaCache::new();
    cache.replay(
        &mut first,
        &original.bytes,
        original.judgment(),
        &mut ledger()?,
    )?;
    let before = second.counts();
    let fault = cache
        .replay(
            &mut second,
            &original.bytes,
            original.judgment(),
            &mut ledger()?,
        )
        .err()
        .ok_or("foreign judgment hit")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::ForeignHandle));
    assert_eq!(second.counts(), before);
    assert_eq!(cache.len(), 1);
    drop(first);
    let (result, rows) = crate::testing::work_trace::measured(|| {
        cache
            .replay(
                &mut second,
                &original.bytes,
                local.judgment(),
                &mut ledger().expect("ledger"),
            )
            .map(|witness| witness.proof().clone())
    })?;
    second.checked(&result?, &mut budget)?;
    assert!(rows.iter().any(|row| row.rule != "replay"));
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.retained_bytes(), 11_728);
    Ok(())
}

/// Rollback and saturated memo generations disable hits without discarding sealed evidence.
/// ´test:unit:lemma-cache-committed-generation-lifetime´
#[test]
fn lemma_cache_committed_generation_lifetime() -> TestResult {
    let mut cas = Cas::new(Limits::default())?;
    let witness = proof(&mut cas, Change::None)?;
    let mut cache = AnonymousLemmaCache::new();
    cache.replay(&mut cas, &witness.bytes, witness.judgment(), &mut ledger()?)?;
    for saturated in [false, true] {
        if saturated {
            cas.memo_epoch = u64::MAX;
        }
        let before = cas.canonical_snapshot(&mut ledger()?)?;
        let epoch = cas.memo_epoch;
        let mut spent = ledger()?;
        let result: Result<(), Fault> = cas.transaction(&mut spent, |_, budget| {
            budget.work(7, AT)?;
            Err(Fault::invalid(InvalidCode::Reference, AT))
        });
        assert!(result.is_err());
        assert_eq!(cas.memo_epoch, epoch.saturating_add(1));
        assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
        assert_eq!(spent.usage().work, 263);
        rechecked(&mut cache, &mut cas, &witness)?;
        if saturated {
            rechecked(&mut cache, &mut cas, &witness)?;
        }
    }
    assert_eq!(cache.len(), 4);
    Ok(())
}

/// A copied graph has its own memo owner even when immutable source handles are shared.
/// ´test:unit:lemma-cache-copied-owner-refusal´
#[test]
fn lemma_cache_copied_owner_refusal() -> TestResult {
    let mut cas = Cas::new(Limits::default())?;
    let witness = proof(&mut cas, Change::None)?;
    let mut cache = AnonymousLemmaCache::new();
    cache.replay(&mut cas, &witness.bytes, witness.judgment(), &mut ledger()?)?;
    let mut copied = cas.full_copy_for_test();
    rechecked(&mut cache, &mut copied, &witness)?;
    assert_eq!(cache.len(), 2);
    Ok(())
}

/// A late replay Work refusal keeps earlier entries, graph bytes and spent cumulative Work.
/// ´test:unit:lemma-cache-late-refusal´
#[test]
fn lemma_cache_late_refusal() -> TestResult {
    let required = late_case(None)?;
    late_case(Some(required - 1))?;
    Ok(())
}

/// Independently reconstruct both starting states; the probe cannot prewarm the refused attempt.
fn late_case(cap: Option<u64>) -> TestResult<u64> {
    let mut cas = Cas::new(Limits::default())?;
    let original = proof(&mut cas, Change::None)?;
    let changed = proof(&mut cas, Change::Premises)?;
    let mut cache = AnonymousLemmaCache::new();
    cache.replay(
        &mut cas,
        &original.bytes,
        original.judgment(),
        &mut ledger()?,
    )?;
    let before = cas.canonical_snapshot(&mut ledger()?)?;
    let mut limits = Limits::default();
    if let Some(cap) = cap {
        limits.work = cap;
    }
    let mut spent = Budget::new(limits)?;
    spent.work(7, AT)?;
    let result = cache.replay(&mut cas, &changed.bytes, changed.judgment(), &mut spent);
    if let Some(cap) = cap {
        assert!(matches!(result.err().ok_or("late refusal accepted")?.kind,
            FaultKind::Exhausted { budget: BudgetKind::Work, limit, needed, .. }
                if limit == cap && needed == cap + 1));
        assert!(spent.usage().work > 7);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.retained_bytes(), 5_864);
        assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
        rechecked(&mut cache, &mut cas, &original)?;
    } else {
        result?;
        assert_eq!(cache.len(), 2);
    }
    Ok(spent.usage().work)
}

/// An invalidated entry remains owned, but its replacement requires independent rule replay.
fn rechecked(
    cache: &mut AnonymousLemmaCache,
    cas: &mut Cas,
    witness: &crate::proof::VerifiedWitness,
) -> TestResult {
    let before = cas.counts();
    let mut spent = ledger()?;
    let (result, rows) = crate::testing::work_trace::measured(|| {
        cache.replay(cas, &witness.bytes, witness.judgment(), &mut spent)
    })?;
    assert_eq!(&result?.bytes, &witness.bytes);
    assert!(spent.usage().work > 7_784);
    assert!(rows.iter().any(|row| row.rule != "replay"));
    assert_eq!(cas.counts(), before);
    Ok(())
}

/// Ordinary limits leave lifecycle checks independent of raised fixture ceilings.
fn ledger() -> Result<Budget, Fault> {
    Budget::new(Limits::default())
}
