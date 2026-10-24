// SPDX-License-Identifier: AGPL-3.0-only
//! Request records preserve canonical bytes across shared nodes, independent stores and rollback.

use crate::domain::HeaderDraft;
use crate::graph::{Cas, Draft, Handle};
use crate::limits::{Budget, Limits};
use crate::syntax::{Declaration, Environment};
use crate::testing::common::{TestResult, default_budget as ledger, real};
use crate::{Fault, InvalidCode, Location};

/// One complete declaration snapshot gives the cache a graph-owned canonical node.
fn header(cas: &mut Cas, name: &str, budget: &mut Budget) -> TestResult<Handle> {
    cas.admit(
        &[Draft::Header(HeaderDraft::Core(Environment {
            declarations: vec![Declaration {
                key: name.into(),
                display_name: name.into(),
                sort: real(),
            }],
            ..Environment::default()
        }))],
        budget,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "header".into())
}

/// A repeated record keeps byte identity and pays only lookup, dependency and encoding work.
/// ´test:crate:canonical-record-memo-reuses-complete-node´
#[test]
fn canonical_record_memo_reuses_complete_node() -> TestResult {
    let mut cas = Cas::new(Limits::default())?;
    let root = header(&mut cas, "original", &mut ledger()?)?;
    let mut budget = ledger()?;
    budget.witness_records.enabled = true;
    let original = crate::wire_build::syntax_key(&cas, root.logical_id(), &mut budget)?;
    let miss = budget.usage().work;
    let repeated = crate::wire_build::syntax_key(&cas, root.logical_id(), &mut budget)?;
    let hit = budget.usage().work - miss;
    assert_eq!(repeated, original);
    assert!(hit < miss);
    // A core header reaches its two registered scalar-space records exactly once each.
    assert_eq!(
        (budget.witness_records.hits, budget.witness_records.misses),
        (3, 3)
    );
    for (limit, success) in [(hit, true), (hit - 1, false)] {
        let mut exact = Budget::new(Limits {
            work: limit,
            ..Limits::default()
        })?;
        exact.witness_records = std::mem::take(&mut budget.witness_records);
        assert_eq!(
            crate::wire_build::syntax_key(&cas, root.logical_id(), &mut exact).is_ok(),
            success
        );
        budget.witness_records = std::mem::take(&mut exact.witness_records);
    }
    Ok(())
}

/// Identical local serials in independent stores cannot share different declarations.
/// ´test:crate:canonical-record-memo-separates-stores´
#[test]
fn canonical_record_memo_separates_stores() -> TestResult {
    let mut first = Cas::new(Limits::default())?;
    let mut second = Cas::new(Limits::default())?;
    let left = header(&mut first, "left", &mut ledger()?)?;
    let right = header(&mut second, "right", &mut ledger()?)?;
    assert_eq!(left.logical_id(), right.logical_id());
    let mut budget = ledger()?;
    budget.witness_records.enabled = true;
    let original = crate::wire_build::syntax_key(&first, left.logical_id(), &mut budget)?;
    let different = crate::wire_build::syntax_key(&second, right.logical_id(), &mut budget)?;
    assert_ne!(original, different);
    assert_eq!(
        different,
        crate::wire_build::syntax_key(&second, right.logical_id(), &mut ledger()?)?
    );
    // A core header reaches its two registered scalar-space records exactly once each.
    assert_eq!(
        (budget.witness_records.hits, budget.witness_records.misses),
        (0, 6)
    );
    Ok(())
}

/// Rolled-back nodes cannot survive as facts when admission reuses their logical serials.
/// ´test:crate:canonical-record-memo-invalidates-rollback´
#[test]
fn canonical_record_memo_invalidates_rollback() -> TestResult {
    let mut cas = Cas::new(Limits::default())?;
    let mut budget = ledger()?;
    budget.witness_records.enabled = true;
    let mut abandoned = None;
    let refused: Result<(), Fault> = cas.transaction(&mut budget, |cas, budget| {
        let value = cas.admit(
            &[Draft::Header(HeaderDraft::Core(Environment::default()))],
            budget,
        )?;
        let root = value
            .first()
            .ok_or_else(|| Fault::invalid(InvalidCode::Reference, Location::new("memo fixture")))?;
        abandoned = Some((
            root.logical_id(),
            crate::wire_build::syntax_key(cas, root.logical_id(), budget)?,
        ));
        Err(Fault::invalid(
            InvalidCode::Rule,
            Location::new("memo fixture"),
        ))
    });
    assert!(refused.is_err());
    assert!(cas.memo_epoch > 0);
    let (former, bytes) = abandoned.ok_or("abandoned root")?;
    let replacement = header(&mut cas, "replacement", &mut ledger()?)?;
    assert_eq!(former, replacement.logical_id());
    let current = crate::wire_build::syntax_key(&cas, replacement.logical_id(), &mut budget)?;
    assert_ne!(current, bytes);
    assert_eq!(
        current,
        crate::wire_build::syntax_key(&cas, replacement.logical_id(), &mut ledger()?)?
    );
    Ok(())
}

/// Ordinary one-shot exports pay no record lookup and leave no new retained cache payload.
/// ´test:crate:canonical-record-memo-is-local-to-reading-services´
#[test]
fn canonical_record_memo_is_local_to_reading_services() -> TestResult {
    let mut cas = Cas::new(Limits::default())?;
    let root = header(&mut cas, "uncached", &mut ledger()?)?;
    let mut budget = ledger()?;
    crate::wire_build::syntax_key(&cas, root.logical_id(), &mut budget)?;
    assert_eq!(
        (
            budget.witness_records.hits,
            budget.witness_records.misses,
            budget.witness_records.bytes
        ),
        (0, 0, 0)
    );
    Ok(())
}
