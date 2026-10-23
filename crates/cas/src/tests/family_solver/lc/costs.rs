// SPDX-License-Identifier: AGPL-3.0-only
//! Exclusive Work buckets separate constructive roots from universal coverage.

use crate::request::Outcome;
use crate::testing::common::TestResult;
use crate::testing::work_trace::Row;

/// Exclusive categories are existence, exclusion, rule validation and surrounding reconstruction.
pub(super) fn totals<'a>(rows: impl Iterator<Item = &'a Row>) -> TestResult<[u64; 4]> {
    let mut result = [0_u64; 4];
    for row in rows {
        let index = if row.rule == "replay" {
            3
        } else if row.phase.starts_with("exclusion.") {
            1
        } else if row.phase.starts_with("existence") {
            0
        } else {
            2
        };
        result[index] = result[index]
            .checked_add(row.work)
            .ok_or("Work attribution overflow")?;
    }
    Ok(result)
}

/// The exact LC replay charges reconcile with their primitive and operation locations.
/// ´test:crate:lc-replay-work-attribution´
#[test]
fn lc_replay_work_attribution() -> TestResult {
    let (mut source, request) = super::fixture::request([1, 1], true)?;
    let Outcome::Solved { answer, .. } = source.cas.run(&request) else {
        return Err("LC coverage unavailable".into());
    };
    let witness = answer.coverage.as_ref().ok_or("LC coverage")?;
    let bytes = source.cas.export_witness(witness, &mut super::ledger()?)?;
    let (mut receiver, original) = super::fixture::request([1, 1], true)?;
    let set = crate::testing::replay_answer::proposal(&source.cas, &answer.set, &mut receiver.cas)?;
    let expected = super::replay::expected(&receiver, &original, &set)?;
    let mut budget = super::ledger()?;
    let (result, rows) = crate::testing::work_trace::measured(|| {
        receiver.cas.replay(&bytes, &expected, &mut budget)
    })?;
    assert!(result?.is_discharged());
    assert_eq!(
        rows.iter().map(|row| row.work).sum::<u64>(),
        budget.usage().work
    );
    assert_eq!(
        [
            totals(rows.iter())?,
            totals(rows.iter().filter(|row| row.rule == "root.select"))?,
            totals(rows.iter().filter(|row| row.rule == "solve.product"))?
        ],
        [
            [221_241, 75_578, 74_839, 1_062_125],
            [190_520, 129, 672, 0],
            [30_721, 61_906, 800, 0]
        ]
    );
    Ok(())
}
