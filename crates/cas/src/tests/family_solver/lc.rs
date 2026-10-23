// SPDX-License-Identifier: AGPL-3.0-only
//! The original determinant retains strict divisors through checked coefficient transport.

mod expected;
mod fixture;
mod identity;
mod refusals;
mod replay;
mod theorem;
mod transport;

use crate::limits::{Budget, Limits};
use crate::request::Outcome;
use crate::testing::common::TestResult;

fn ledger() -> TestResult<Budget> {
    Ok(Budget::new(Limits::blind_cubic())?)
}

/// Checked transport equates the original strict determinant with an independently built exact coefficient.
/// ´test:crate:lc-original-coefficient-equality-replays´
#[test]
fn lc_original_coefficient_equality_replays() -> TestResult {
    let (mut source, request) = fixture::request([1, 1], true)?;
    let claim = expected::identity(&mut source, &request)?;
    let mut generation = ledger()?;
    let mut checking = ledger()?;
    let witness = identity::prove(&mut source, &claim, &mut generation, &mut checking)?;
    assert_eq!(witness.judgment(), &claim);
    let bytes = source.cas.export_witness(&witness, &mut ledger()?)?;
    let (mut receiver, original) = fixture::request([1, 1], true)?;
    let claim = expected::identity(&mut receiver, &original)?;
    let mut replay = ledger()?;
    assert!(
        receiver
            .cas
            .replay(&bytes, &claim, &mut replay)?
            .is_discharged()
    );
    assert_eq!(
        (
            generation.usage().work,
            checking.usage().work,
            replay.usage().work,
            bytes.len()
        ),
        (236_088, 1_210_414, 825_134, 2_479)
    );
    Ok(())
}

/// Source coverage refuses a missing divisor inventory before any root proposal can be published.
/// ´test:crate:lc-original-missing-domain-refuses´
#[test]
fn lc_original_missing_domain_refuses() -> TestResult {
    let (mut source, request) = fixture::request([1, 1], false)?;
    let outcome = source.cas.run(&request);
    let Outcome::Invalid { fault, usage } = outcome else {
        return Err(format!("missing LC divisor inventory: {outcome:?}").into());
    };
    assert_eq!(
        fault,
        crate::Fault::invalid(
            crate::InvalidCode::Sort,
            crate::Location::new("solving object admission")
        )
    );
    assert_eq!(usage.work, 8_563);
    for (signs, work) in [([0, 1], 447_073), ([1, 0], 444_665)] {
        let (mut source, request) = fixture::request(signs, true)?;
        let outcome = source.cas.run(&request);
        let Outcome::Unresolved {
            solve_reason,
            usage,
            ..
        } = outcome
        else {
            return Err(format!("missing LC sign: {outcome:?}").into());
        };
        assert_eq!(
            solve_reason,
            Some(crate::solving::SolveReason::MissingUnitProof)
        );
        assert_eq!(usage.work, work);
    }
    Ok(())
}

/// The transported original coefficient identity cannot move to an unsigned or sibling stratum.
/// ´test:crate:lc-coefficient-replay-retains-signs´
#[test]
fn lc_coefficient_replay_retains_signs() -> TestResult {
    let (mut source, request) = fixture::request([1, 1], true)?;
    let claim = expected::identity(&mut source, &request)?;
    let witness = identity::prove(&mut source, &claim, &mut ledger()?, &mut ledger()?)?;
    let bytes = source.cas.export_witness(&witness, &mut ledger()?)?;
    for signs in [[0, 1], [1, 0], [-1, 1], [1, -1]] {
        let (mut receiver, request) = fixture::request(signs, true)?;
        let claim = expected::identity(&mut receiver, &request)?;
        let before = receiver.cas.counts();
        assert!(receiver.cas.replay(&bytes, &claim, &mut ledger()?).is_err());
        assert_eq!(receiver.cas.counts(), before);
    }
    Ok(())
}
