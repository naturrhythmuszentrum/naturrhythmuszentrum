// SPDX-License-Identifier: AGPL-3.0-only
//! Crate-visible frontier scheduling and dominance contracts.

use crate::Fault;
use crate::frontier::Frontier;
use crate::testing::common::default_budget;
use crate::testing::frontier::metrics;

/// The default epoch is exactly four cost visits followed by one progress visit.
/// ´test:crate:queue-quota-order´
#[test]
fn queue_quota_order() -> Result<(), Fault> {
    let mut frontier = Frontier::default();
    let mut budget = default_budget()?;
    for id in 0_u8..6 {
        frontier.insert(
            usize::from(id),
            vec![id],
            [u64::from(6 - id), 0, 0, 0],
            metrics(u64::from(id), 0, 0),
            &mut budget,
        )?;
    }
    for expected in [0, 1, 2, 3, 5, 4] {
        assert_eq!(frontier.pop(&mut budget)?, Some(expected));
    }
    assert_eq!(
        frontier.visits,
        vec![false, false, false, false, true, false]
    );
    assert_eq!(frontier.pop(&mut budget)?, None);
    Ok(())
}

/// Resumed equal-priority cursors go behind earlier pending visits.
/// ´test:crate:queue-resumption-order´
#[test]
fn queue_resumption_order() -> Result<(), Fault> {
    let mut frontier = Frontier::default();
    let mut budget = default_budget()?;
    frontier.insert(0, vec![0], [0; 4], metrics(1, 0, 0), &mut budget)?;
    frontier.insert(1, vec![0], [0; 4], metrics(1, 0, 1), &mut budget)?;
    assert_eq!(frontier.pop(&mut budget)?, Some(0));
    frontier.resume(0, &mut budget)?;
    assert_eq!(frontier.pop(&mut budget)?, Some(1));
    assert_eq!(frontier.pop(&mut budget)?, Some(0));
    Ok(())
}

/// Neither expansion tradeoffs nor distinct dependency support can be discarded as cheaper.
/// ´test:crate:resource-dominance-laws´
#[test]
fn resource_dominance_laws() {
    let cheap_expanded = metrics(1, 4, 0);
    let dear_compact = metrics(2, 1, 0);
    assert!(!cheap_expanded.dominates(&dear_compact));
    assert!(!dear_compact.dominates(&cheap_expanded));
    assert!(!metrics(1, 1, 0).dominates(&metrics(2, 2, 1)));
    assert!(metrics(1, 1, 0).dominates(&metrics(2, 2, 0)));
}

/// Different theorem classes remain separate, while worse labels lose both queue entries.
/// ´test:crate:queue-condition-classes´
#[test]
fn queue_condition_classes() -> Result<(), Fault> {
    let mut frontier = Frontier::default();
    let mut budget = default_budget()?;
    assert!(frontier.insert(0, vec![0], [0; 4], metrics(3, 2, 0), &mut budget)?);
    assert!(frontier.insert(1, vec![1], [0; 4], metrics(1, 0, 0), &mut budget)?);
    assert!(frontier.insert(2, vec![0], [0; 4], metrics(2, 1, 0), &mut budget)?);
    assert_eq!(frontier.dominated, 1);
    assert_eq!(frontier.pop(&mut budget)?, Some(1));
    assert_eq!(frontier.pop(&mut budget)?, Some(2));
    assert_eq!(frontier.pop(&mut budget)?, None);
    Ok(())
}
