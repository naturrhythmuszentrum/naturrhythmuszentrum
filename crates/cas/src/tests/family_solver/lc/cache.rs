// SPDX-License-Identifier: AGPL-3.0-only
//! Complete LC theorem retention across independent caller ledgers.

use crate::request::Outcome;
use crate::testing::common::TestResult;

/// A complete original LC theorem is checked once and retained across independent request ledgers.
/// ´test:crate:lc-anonymous-lemma-cache-retains-coverage´
#[test]
fn lc_anonymous_lemma_cache_retains_coverage() -> TestResult {
    let (mut source, request) = super::fixture::request([1, 1], true)?;
    let Outcome::Solved { answer, .. } = source.cas.run(&request) else {
        return Err("LC coverage".into());
    };
    let witness = answer.coverage.as_ref().ok_or("LC coverage")?;
    let bytes = source.cas.export_witness(witness, &mut super::ledger()?)?;
    let (mut target, request) = super::fixture::request([1, 1], true)?;
    let set = crate::testing::replay_answer::proposal(&source.cas, &answer.set, &mut target.cas)?;
    let expected = super::replay::expected(&target, &request, &set)?;
    let mut cache = crate::proof::AnonymousLemmaCache::new();
    let mut costs = Vec::new();
    for warm in [false, true] {
        let before = target.cas.counts();
        let mut ledger = super::ledger()?;
        let witness = cache.replay(&mut target.cas, &bytes, &expected, &mut ledger)?;
        assert!(witness.is_discharged());
        assert_eq!(witness.judgment(), &expected);
        if warm {
            assert_eq!(target.cas.counts(), before);
        }
        costs.push(ledger.usage().work);
    }
    assert_eq!(cache.len(), 1);
    costs.push(cache.retained_bytes());
    assert_eq!(costs, [1_563_060, 116_711, 74_056]);
    Ok(())
}
