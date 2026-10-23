// SPDX-License-Identifier: AGPL-3.0-only
//! Fresh expected-judgment replay checks exact ordered source partials.

use crate::testing::common::TestResult;
use crate::testing::jacobians::{coordinates, expected, product};
use crate::testing::map_inverses::ledger;

/// Ordered product partials replay against independently written `(y,x)`.
/// ´test:integration:source-jacobian-product-replays-fresh´
#[test]
fn source_jacobian_product_replays_fresh() -> TestResult {
    let (mut source, original, _) = product()?;
    let key = source.cas.source_jacobian_key(
        &source.context,
        &original,
        &coordinates(),
        &mut ledger()?,
    )?;
    let result = source.cas.source_jacobian(&key, &mut ledger()?)?;
    assert!(result.witness().is_discharged());
    assert!(result.representation().is_none());
    let bytes = source
        .cas
        .export_witness(result.witness(), &mut ledger()?)?;
    let (mut receiver, original, oracle) = product()?;
    let target = expected(&mut receiver, &original, &oracle)?;
    let replayed = receiver.cas.replay(&bytes, &target, &mut ledger()?)?;
    assert_eq!(
        receiver.cas.export_witness(&replayed, &mut ledger()?)?,
        bytes
    );
    let other = receiver.cas.source_jacobian_key(
        &receiver.context,
        &original,
        &coordinates(),
        &mut ledger()?,
    )?;
    assert_eq!(key, other);
    Ok(())
}

/// Changing the source order changes the key and rejects the old ordered theorem.
/// ´test:integration:source-jacobian-replay-retains-source-order´
#[test]
fn source_jacobian_replay_retains_source_order() -> TestResult {
    let (mut source, original, _) = product()?;
    let key = source.cas.source_jacobian_key(
        &source.context,
        &original,
        &coordinates(),
        &mut ledger()?,
    )?;
    let result = source.cas.source_jacobian(&key, &mut ledger()?)?;
    let bytes = source
        .cas
        .export_witness(result.witness(), &mut ledger()?)?;
    let (mut receiver, original, oracle) = product()?;
    let mut directions = coordinates();
    directions.sources.reverse();
    let changed = receiver.cas.source_jacobian_key(
        &receiver.context,
        &original,
        &directions,
        &mut ledger()?,
    )?;
    assert_ne!(key, changed);
    let mut target = expected(&mut receiver, &original, &oracle)?;
    if let nrz_cas::proof::Statement::Jacobian { variables, .. } = &mut target.statement {
        variables.reverse();
    }
    assert!(
        receiver
            .cas
            .replay(&bytes, &target, &mut ledger()?)
            .is_err()
    );
    Ok(())
}
