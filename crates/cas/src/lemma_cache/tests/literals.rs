// SPDX-License-Identifier: AGPL-3.0-only
//! Reviewed literal costs complement the adaptive threshold properties.

use super::{Change, proof};
use crate::budget::{BudgetKind, Limits};
use crate::lemma_cache::{AT, AnonymousLemmaCache};
use crate::testing::common::TestResult;
use crate::{Cas, FaultKind};

/// The caller has spent Work and retains unrelated payload before replay begins.
const PRIOR: u64 = 7;
/// Unrelated caller payload must coexist with keys, retained witnesses and replay scratch.
const CALLER: u64 = 31;

/// Cold replay visits no entries; the single-entry warm hit visits exactly one complete key.
/// ´test:unit:lemma-cache-literal-work´
#[test]
fn lemma_cache_literal_work() -> TestResult {
    for (warm, required) in [(false, 39_265), (true, 7_784)] {
        for short in [0, 1] {
            work_case(warm, required, short)?;
        }
    }
    Ok(())
}

/// Literal N and N-minus-one include earlier Work without deriving N from this execution.
fn work_case(warm: bool, required: u64, short: u64) -> TestResult {
    let mut cas = Cas::new(Limits::default())?;
    let witness = proof(&mut cas, Change::None)?;
    let mut cache = AnonymousLemmaCache::new();
    if warm {
        cache.replay(
            &mut cas,
            &witness.bytes,
            witness.judgment(),
            &mut crate::budget::Budget::new(Limits::default())?,
        )?;
    }
    let before = cas.counts();
    let mut ledger = crate::budget::Budget::new(Limits {
        work: PRIOR + required - short,
        ..Limits::default()
    })?;
    ledger.work(PRIOR, AT)?;
    let result = ledger.with_retained_payload(CALLER, AT, |ledger| {
        cache
            .replay(&mut cas, &witness.bytes, witness.judgment(), ledger)
            .map(|_| ())
    });
    if short == 0 {
        result?;
        assert_eq!(ledger.usage().work, PRIOR + required);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.retained_bytes(), 5_864);
    } else {
        assert!(matches!(
            result.err().ok_or("short literal Work accepted")?.kind,
            FaultKind::Exhausted { budget: BudgetKind::Work, limit, needed, .. }
                if limit == PRIOR + required - 1 && needed == PRIOR + required
        ));
        assert!(ledger.usage().work > PRIOR);
        assert!(ledger.usage().work < PRIOR + required);
        assert_eq!(cache.len(), usize::from(warm));
        assert_eq!(cache.retained_bytes(), if warm { 5_864 } else { 0 });
        assert_eq!(cas.counts(), before);
    }
    Ok(())
}

/// Literal storage includes the exact entry envelope and separate cold/warm scratch peaks.
/// ´test:unit:lemma-cache-literal-storage´
#[test]
fn lemma_cache_literal_storage() -> TestResult {
    for warm in [false, true] {
        for short in [0, 1] {
            storage_case(warm, BudgetKind::OutputBytes, 5_864, short)?;
            storage_case(
                warm,
                BudgetKind::StagedPayloadBytes,
                CALLER + if warm { 11_672 } else { 13_823 },
                short,
            )?;
        }
    }
    Ok(())
}

/// Every storage refusal preserves earlier cache entries and the complete graph occupancy.
fn storage_case(warm: bool, kind: BudgetKind, required: u64, short: u64) -> TestResult {
    let mut cas = Cas::new(Limits::default())?;
    let witness = proof(&mut cas, Change::None)?;
    assert_eq!(witness.bytes.len(), 326);
    let key = crate::wire_build::judgment_key(
        &cas,
        witness.judgment(),
        &mut crate::budget::Budget::new(Limits::default())?,
    )?;
    assert_eq!(key.len(), 279);
    let mut cache = AnonymousLemmaCache::new();
    if warm {
        cache.replay(
            &mut cas,
            &witness.bytes,
            witness.judgment(),
            &mut crate::budget::Budget::new(Limits::default())?,
        )?;
    }
    let before = cas.counts();
    let mut limits = Limits::default();
    match kind {
        BudgetKind::OutputBytes => limits.output_bytes = required - short,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = required - short,
        _ => return Err("storage pin kind".into()),
    }
    let mut ledger = crate::budget::Budget::new(limits)?;
    ledger.work(PRIOR, AT)?;
    let result = ledger.with_retained_payload(CALLER, AT, |ledger| {
        cache
            .replay(&mut cas, &witness.bytes, witness.judgment(), ledger)
            .map(|_| ())
    });
    if short == 0 {
        result?;
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.retained_bytes(), 5_864);
    } else {
        let fault = result.err().ok_or("short literal storage accepted")?;
        assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind));
        assert_eq!(cache.len(), usize::from(warm));
        assert_eq!(cache.retained_bytes(), if warm { 5_864 } else { 0 });
        assert_eq!(cas.counts(), before);
        assert!(ledger.usage().work >= PRIOR);
    }
    Ok(())
}

/// Foreign entries remain charged in insertion order, and the first local match ends the scan.
/// ´test:unit:lemma-cache-literal-scan-growth´
#[test]
fn lemma_cache_literal_scan_growth() -> TestResult {
    let mut stores = [
        Cas::new(Limits::default())?,
        Cas::new(Limits::default())?,
        Cas::new(Limits::default())?,
    ];
    let witnesses = stores
        .iter_mut()
        .map(|cas| proof(cas, Change::None))
        .collect::<TestResult<Vec<_>>>()?;
    let mut cache = AnonymousLemmaCache::new();
    for (cas, witness) in stores.iter_mut().zip(&witnesses) {
        cache.replay(
            cas,
            &witness.bytes,
            witness.judgment(),
            &mut crate::budget::Budget::new(Limits::default())?,
        )?;
    }
    assert_eq!(cache.len(), 3);
    assert_eq!(cache.retained_bytes(), 17_592);
    for ((cas, witness), (work, scan)) in
        stores
            .iter_mut()
            .zip(&witnesses)
            .zip([(7_784, 606), (8_390, 1_212), (8_996, 1_818)])
    {
        let before = cas.counts();
        let mut ledger = crate::budget::Budget::new(Limits::default())?;
        let (result, rows) = crate::testing::work_trace::measured(|| {
            cache.replay(cas, &witness.bytes, witness.judgment(), &mut ledger)
        })?;
        result?;
        assert_eq!(ledger.usage().work, work);
        assert_eq!(
            rows.iter()
                .find(|row| row.location == "anonymous lemma cache")
                .ok_or("scan charge")?
                .work,
            scan
        );
        assert!(rows.iter().all(|row| row.rule == "replay"));
        assert_eq!(cas.counts(), before);
    }
    Ok(())
}
