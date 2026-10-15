// SPDX-License-Identifier: AGPL-3.0-only
//! Exact split calculus from original occurrence receipts through expected fresh replay.

use crate::testing::boundary::named;
use crate::testing::budgets::derivation_budget as budget;
use crate::testing::cases::Fixture;
use crate::testing::common::TestResult;
use crate::testing::split::{expected, odd, task};
use nrz_cas::proof::SplitKind;
use nrz_cas::request::Outcome;
use nrz_cas::syntax::CaseName;

/// The absolute-value integral has orientation and the hand value (1+4)/2.
/// ´test:integration:split-absolute-integral-lifecycle´
#[test]
fn split_absolute_integral_lifecycle() -> TestResult {
    for (lower, upper, value) in [("-1", "2", "5/2"), ("2", "-1", "-5/2")] {
        let mut fixture = Fixture::new()?;
        let function = named(&mut fixture, CaseName::Abs)?;
        let request = fixture.request(
            task(SplitKind::Integral, lower, upper)?,
            std::slice::from_ref(&function),
        )?;
        let Outcome::Proved { result, usage, .. } = fixture.cas.run(&request) else {
            return Err("integral proof".into());
        };
        assert_eq!(
            *result.witness.judgment(),
            expected(
                &mut fixture,
                &function,
                SplitKind::Integral,
                (lower, upper),
                value
            )?
        );
        let bytes = fixture
            .cas
            .export_witness(&result.witness, &mut budget()?)?;
        let mut receiver = Fixture::new()?;
        receiver.number("99")?;
        let incoming = named(&mut receiver, CaseName::Abs)?;
        let claim = expected(
            &mut receiver,
            &incoming,
            SplitKind::Integral,
            (lower, upper),
            value,
        )?;
        let mut ledger = budget()?;
        let proof = receiver.cas.replay(&bytes, &claim, &mut ledger)?;
        assert_eq!(receiver.cas.export_witness(&proof, &mut budget()?)?, bytes);
        assert!(ledger.usage().work <= usage.work.saturating_mul(10));
        assert!(result.replay_usage.work <= usage.work.saturating_mul(10));
    }
    Ok(())
}

/// F(k)=k²-2k has forward difference 2k-1; endpoint cancellation gives n².
/// ´test:integration:split-odd-sum-lifecycle´
#[test]
fn split_odd_sum_lifecycle() -> TestResult {
    for split in [false, true] {
        let mut fixture = Fixture::new()?;
        let function = odd(&mut fixture, split)?;
        let request = fixture.request(
            task(SplitKind::Sum, "1", "9")?,
            std::slice::from_ref(&function),
        )?;
        let Outcome::Proved { result, usage, .. } = fixture.cas.run(&request) else {
            return Err("sum proof".into());
        };
        let bytes = fixture
            .cas
            .export_witness(&result.witness, &mut budget()?)?;
        let mut receiver = Fixture::new()?;
        let incoming = odd(&mut receiver, split)?;
        let claim = expected(&mut receiver, &incoming, SplitKind::Sum, ("1", "9"), "81")?;
        let mut ledger = budget()?;
        let proof = receiver.cas.replay(&bytes, &claim, &mut ledger)?;
        assert_eq!(receiver.cas.export_witness(&proof, &mut budget()?)?, bytes);
        assert!(ledger.usage().work <= usage.work.saturating_mul(10));
    }
    Ok(())
}
