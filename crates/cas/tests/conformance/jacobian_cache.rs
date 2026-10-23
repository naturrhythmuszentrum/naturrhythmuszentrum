// SPDX-License-Identifier: AGPL-3.0-only
//! Exact-key retention spends no derivative or replay Work on a cache hit.

use crate::testing::common::TestResult;
use crate::testing::jacobians::{coordinates, lc, lc_coordinates, product};
use crate::testing::map_inverses::ledger;
use nrz_cas::jacobians::SourceJacobianCache;
use nrz_cas::{Fault, FaultKind, InvalidCode};

/// Repeating an exact product key returns the identical proof and charges only byte comparison.
/// ´test:integration:source-jacobian-cache-hit-only-compares-key´
#[test]
fn source_jacobian_cache_hit_only_compares_key() -> TestResult {
    let (mut case, original, _) = product()?;
    let key =
        case.cas
            .source_jacobian_key(&case.context, &original, &coordinates(), &mut ledger()?)?;
    let mut cache = SourceJacobianCache::new();
    assert_eq!(cache.len(), 0);
    let witness = cache
        .get_or_derive(&mut case.cas, &key, &mut ledger()?)?
        .witness()
        .proof()
        .clone();
    let before = case.cas.counts();
    let mut budget = ledger()?;
    let repeated = cache.get_or_derive(&mut case.cas, &key, &mut budget)?;
    assert_eq!(repeated.witness().proof(), &witness);
    assert_eq!(
        budget.usage().work,
        u64::try_from(key.as_bytes().len())? + 1
    );
    assert_eq!(budget.usage().arithmetic_steps, 0);
    assert_eq!(budget.usage().condition_steps, 0);
    assert_eq!(case.cas.counts(), before);
    assert_eq!(cache.len(), 1);
    Ok(())
}

/// LC keying twice retains one exact derivative witness independently of numerical operating points.
/// ´test:integration:source-jacobian-lc-cache-retains-one-witness´
#[test]
fn source_jacobian_lc_cache_retains_one_witness() -> TestResult {
    let (mut case, original, oracle) = lc(true)?;
    let mut cache = SourceJacobianCache::new();
    let first = case.cas.source_jacobian_key(
        &case.context,
        &original,
        &lc_coordinates(),
        &mut ledger()?,
    )?;
    let witness = cache
        .get_or_derive(&mut case.cas, &first, &mut ledger()?)?
        .witness()
        .proof()
        .clone();
    let second = case.cas.source_jacobian_key(
        &case.context,
        &original,
        &lc_coordinates(),
        &mut ledger()?,
    )?;
    assert_eq!(first, second);
    let result = cache.get_or_derive(&mut case.cas, &second, &mut ledger()?)?;
    assert_eq!(result.witness().proof(), &witness);
    case.cas
        .prove_reading_equality(&case.context, result.matrix(), &oracle, &mut ledger()?)?;
    assert_eq!(cache.len(), 1);
    Ok(())
}

/// Source reordering retains a separate exact theorem; foreign keys never borrow local authority.
/// ´test:integration:source-jacobian-cache-rejects-stale-directions´
#[test]
fn source_jacobian_cache_rejects_stale_directions() -> TestResult {
    let (mut case, original, _) = product()?;
    let mut cache = SourceJacobianCache::new();
    let first =
        case.cas
            .source_jacobian_key(&case.context, &original, &coordinates(), &mut ledger()?)?;
    let witness = cache
        .get_or_derive(&mut case.cas, &first, &mut ledger()?)?
        .witness()
        .proof()
        .clone();
    let mut contract = coordinates();
    contract.sources.reverse();
    let second =
        case.cas
            .source_jacobian_key(&case.context, &original, &contract, &mut ledger()?)?;
    let other = cache.get_or_derive(&mut case.cas, &second, &mut ledger()?)?;
    assert_ne!(other.witness().proof(), &witness);
    assert_eq!(cache.len(), 2);
    let (mut receiver, original, _) = product()?;
    assert!(matches!(
        cache.get_or_derive(&mut receiver.cas, &first, &mut ledger()?),
        Err(Fault {
            kind: FaultKind::Invalid(InvalidCode::ForeignHandle),
            ..
        })
    ));
    let local = receiver.cas.source_jacobian_key(
        &receiver.context,
        &original,
        &coordinates(),
        &mut ledger()?,
    )?;
    assert_eq!(first, local);
    cache.get_or_derive(&mut receiver.cas, &local, &mut ledger()?)?;
    assert_eq!(cache.len(), 3);
    Ok(())
}
