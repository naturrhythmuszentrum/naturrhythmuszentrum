// SPDX-License-Identifier: AGPL-3.0-only
//! Complete expected replay retains source/target coordinates, inverse maps and hand-derived weights.

use crate::testing::budgets::derivation_budget as budget;
use crate::testing::common::TestResult;
use crate::testing::pullbacks::{Fixture, roots};
use nrz_cas::request::{Outcome, PullbackKind};

/// Positive and negative affine scales use the absolute Jacobian and retain both exact spaces.
/// ´test:integration:affine-pullback-lifecycle´
#[test]
fn affine_pullback_lifecycle() -> TestResult {
    for (scale, offset, point, weight) in [("2", "-1", "1/2", "1/2"), ("-3", "6", "2", "1/3")] {
        let bytes = affine_bytes(scale, offset, false)?;
        assert_eq!(bytes, affine_bytes(scale, offset, true)?);
        let mut receiver = Fixture::new()?;
        receiver.base.number("17")?;
        let source = receiver.source()?;
        let (map, inverse) = receiver.affine(scale, offset)?;
        let output = receiver.weighted(&[(point, weight)])?;
        let expected = receiver.expected(source, map, Some(inverse), output);
        let proof = receiver
            .base
            .cas
            .replay(&bytes, &expected, &mut budget()?)?;
        assert_eq!(
            receiver.base.cas.export_witness(&proof, &mut budget()?)?,
            bytes
        );
    }
    Ok(())
}

/// Generate bytes under different irrelevant allocation orders and budget ceilings.
fn affine_bytes(scale: &str, offset: &str, extra: bool) -> TestResult<Vec<u8>> {
    let mut fixture = Fixture::new()?;
    if extra {
        fixture.base.number("71")?;
    }
    let source = fixture.source()?;
    let (map, _) = fixture.affine(scale, offset)?;
    let mut request = fixture
        .base
        .request(fixture.task(PullbackKind::Affine), &[source, map])?;
    if extra {
        request.limits.pullback_zeros = Some(1);
    }
    let Outcome::Proved { result, usage, .. } = fixture.base.cas.run(&request) else {
        return Err("affine pullback".into());
    };
    assert!(result.replay_usage.work <= usage.work.saturating_mul(10));
    Ok(fixture
        .base
        .cas
        .export_witness(&result.witness, &mut budget()?)?)
}

/// A complete factorization gives two half-weight deltas and portable expected replay.
/// ´test:integration:simple-zero-pullback-lifecycle´
#[test]
fn simple_zero_pullback_lifecycle() -> TestResult {
    let (bytes, work) = simple_bytes(false)?;
    assert_eq!(bytes, simple_bytes(true)?.0);
    let mut receiver = Fixture::new()?;
    let source = receiver.source()?;
    let map = receiver.quadratic(false)?;
    let output = receiver.weighted(&[("-1", "1/2"), ("1", "1/2")])?;
    let expected = receiver.expected(source, map, None, output);
    let mut ledger = budget()?;
    let proof = receiver.base.cas.replay(&bytes, &expected, &mut ledger)?;
    assert_eq!(
        receiver.base.cas.export_witness(&proof, &mut budget()?)?,
        bytes
    );
    assert!(ledger.usage().work <= work.saturating_mul(10));
    Ok(())
}

/// Run the factorized map with root intent supplied independently of its syntax.
fn simple_bytes(extra: bool) -> TestResult<(Vec<u8>, u64)> {
    let mut fixture = Fixture::new()?;
    if extra {
        fixture.base.number("29")?;
    }
    let source = fixture.source()?;
    let map = fixture.quadratic(false)?;
    let mut request = fixture
        .base
        .request(fixture.task(roots(&["-1", "1"])?), &[source, map])?;
    if extra {
        request.limits.pullback_zeros = Some(2);
    }
    let Outcome::Proved { result, usage, .. } = fixture.base.cas.run(&request) else {
        return Err("simple-zero pullback".into());
    };
    assert!(result.replay_usage.work <= usage.work.saturating_mul(10));
    Ok((
        fixture
            .base
            .cas
            .export_witness(&result.witness, &mut budget()?)?,
        usage.work,
    ))
}
