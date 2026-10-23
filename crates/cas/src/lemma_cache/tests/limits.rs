// SPDX-License-Identifier: AGPL-3.0-only
//! Complete-key preparation, miss checking and retained hits have exact resource boundaries.

use super::{Change, proof};
use crate::budget::{Budget, BudgetKind};
use crate::lemma_cache::AnonymousLemmaCache;
use crate::testing::common::TestResult;
use crate::testing::native::budget;
use crate::{Cas, FaultKind};

/// Both cold checking and warm comparison obey N and N-minus-one cumulative Work ceilings.
/// ´test:unit:lemma-cache-work-boundaries´
#[test]
fn lemma_cache_work_boundaries() -> TestResult {
    for warm in [false, true] {
        let mut cas = Cas::new(budget()?.limits().clone())?;
        let witness = proof(&mut cas, Change::None)?;
        let mut cache = AnonymousLemmaCache::new();
        if warm {
            cache.replay(&mut cas, &witness.bytes, witness.judgment(), &mut budget()?)?;
        }
        let mut ledger = budget()?;
        cache.replay(&mut cas, &witness.bytes, witness.judgment(), &mut ledger)?;
        let required = ledger.usage().work;
        for short in [0, 1] {
            work_case(warm, required, short)?;
        }
    }
    Ok(())
}

fn work_case(warm: bool, required: u64, short: u64) -> TestResult {
    let mut cas = Cas::new(budget()?.limits().clone())?;
    let witness = proof(&mut cas, Change::None)?;
    let mut cache = AnonymousLemmaCache::new();
    if warm {
        cache.replay(&mut cas, &witness.bytes, witness.judgment(), &mut budget()?)?;
    }
    let before = cas.counts();
    let mut limits = budget()?.limits().clone();
    limits.work = required - short;
    let mut ledger = Budget::new(limits)?;
    let result = cache.replay(&mut cas, &witness.bytes, witness.judgment(), &mut ledger);
    if short == 0 {
        result?;
        assert_eq!(ledger.usage().work, required);
    } else {
        let fault = result.err().ok_or("short cache Work accepted")?;
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            }
        ));
        assert_eq!(cache.len(), usize::from(warm));
        assert_eq!(cas.counts(), before);
        cache.replay(&mut cas, &witness.bytes, witness.judgment(), &mut budget()?)?;
        assert_eq!(cache.len(), 1);
    }
    Ok(())
}

/// Retained payload is checked on every hit and a storage refusal preserves the existing lemma.
/// ´test:unit:lemma-cache-storage-boundaries´
#[test]
fn lemma_cache_storage_boundaries() -> TestResult {
    let mut cas = Cas::new(budget()?.limits().clone())?;
    let witness = proof(&mut cas, Change::None)?;
    let mut cache = AnonymousLemmaCache::new();
    cache.replay(&mut cas, &witness.bytes, witness.judgment(), &mut budget()?)?;
    let retained = cache.retained_bytes();
    for short in [0, 1] {
        let mut limits = budget()?.limits().clone();
        limits.output_bytes = retained - short;
        let result = cache.replay(
            &mut cas,
            &witness.bytes,
            witness.judgment(),
            &mut Budget::new(limits)?,
        );
        assert_eq!(result.is_ok(), short == 0);
        assert_eq!(cache.len(), 1);
    }
    let mut lower = 0;
    let mut upper = budget()?.limits().staged_payload_bytes;
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if staged(&mut cache, &mut cas, &witness, middle)? {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    assert!(lower >= retained);
    assert!(staged(&mut cache, &mut cas, &witness, lower)?);
    assert!(!staged(&mut cache, &mut cas, &witness, lower - 1)?);
    Ok(())
}

fn staged(
    cache: &mut AnonymousLemmaCache,
    cas: &mut Cas,
    witness: &crate::proof::VerifiedWitness,
    cap: u64,
) -> TestResult<bool> {
    let mut limits = budget()?.limits().clone();
    limits.staged_payload_bytes = cap;
    let before = cas.counts();
    let result = cache.replay(
        cas,
        &witness.bytes,
        witness.judgment(),
        &mut Budget::new(limits)?,
    );
    let accepted = result.is_ok();
    if let Err(fault) = result {
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::StagedPayloadBytes,
                ..
            }
        ));
    }
    assert_eq!(cas.counts(), before);
    assert_eq!(cache.len(), 1);
    Ok(accepted)
}

/// A cold retention failure rolls back both the graph and cache at the exact staged boundary.
/// ´test:unit:lemma-cache-cold-storage-boundaries´
#[test]
fn lemma_cache_cold_storage_boundaries() -> TestResult {
    let mut lower = 0;
    let mut upper = budget()?.limits().staged_payload_bytes;
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if cold_staged(middle)? {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    assert!(lower > 0);
    assert!(cold_staged(lower)?);
    assert!(!cold_staged(lower - 1)?);
    Ok(())
}

fn cold_staged(cap: u64) -> TestResult<bool> {
    let mut cas = Cas::new(budget()?.limits().clone())?;
    let witness = proof(&mut cas, Change::None)?;
    let mut cache = AnonymousLemmaCache::new();
    let mut limits = budget()?.limits().clone();
    limits.staged_payload_bytes = cap;
    let before = cas.counts();
    match cache.replay(
        &mut cas,
        &witness.bytes,
        witness.judgment(),
        &mut Budget::new(limits)?,
    ) {
        Ok(_) => {
            assert_eq!(cache.len(), 1);
            Ok(true)
        }
        Err(fault) => {
            assert!(matches!(
                fault.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::StagedPayloadBytes,
                    ..
                }
            ));
            assert!(cache.is_empty());
            assert_eq!(cas.counts(), before);
            Ok(false)
        }
    }
}
