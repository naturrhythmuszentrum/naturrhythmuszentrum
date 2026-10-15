// SPDX-License-Identifier: AGPL-3.0-only
//! Standalone limits and polynomial series replay against independent complete claims.

use crate::testing::boundary::named;
use crate::testing::budgets::derivation_budget as budget;
use crate::testing::cases::Fixture;
use crate::testing::common::TestResult;
use crate::testing::germs::{at, cubic, expected, task};
use nrz_cas::domain::Side;
use nrz_cas::proof::GermMode;
use nrz_cas::request::Outcome;
use nrz_cas::syntax::CaseName;

/// Sign has independent side limits +1 and -1; each standalone witness survives expected replay.
/// ´test:integration:case-sign-limit-lifecycle´
#[test]
fn case_sign_limit_lifecycle() -> TestResult {
    for (side, value) in [(Side::Above, "1"), (Side::Below, "-1")] {
        let mut fixture = Fixture::new()?;
        let function = named(&mut fixture, CaseName::Sign)?;
        let approach = at(&mut fixture, "0", Some(side))?;
        let request = fixture.request(task(&approach, GermMode::Limit), &[function])?;
        let Outcome::Proved { result, usage, .. } = fixture.cas.run(&request) else {
            return Err("limit proof".into());
        };
        let bytes = fixture
            .cas
            .export_witness(&result.witness, &mut budget()?)?;
        let mut receiver = Fixture::new()?;
        receiver.number("17")?;
        let function = named(&mut receiver, CaseName::Sign)?;
        let approach = at(&mut receiver, "0", Some(side))?;
        let claim = expected(&receiver, &function, &approach, GermMode::Limit, &[value])?;
        let mut ledger = budget()?;
        let proof = receiver.cas.replay(&bytes, &claim, &mut ledger)?;
        assert_eq!(receiver.cas.export_witness(&proof, &mut budget()?)?, bytes);
        assert!(ledger.usage().work <= usage.work.saturating_mul(10));
        assert!(result.replay_usage.work <= usage.work.saturating_mul(10));
    }
    Ok(())
}

/// Ramp's right germ is exactly h, hence coefficients [0,1,0] and zero remainder at order two.
/// ´test:integration:case-ramp-side-series-lifecycle´
#[test]
fn case_ramp_side_series_lifecycle() -> TestResult {
    let mode = GermMode::Series {
        order: 2,
        analytic: false,
    };
    let mut fixture = Fixture::new()?;
    let function = named(&mut fixture, CaseName::Ramp)?;
    let approach = at(&mut fixture, "0", Some(Side::Above))?;
    let request = fixture.request(task(&approach, mode), &[function])?;
    let Outcome::Proved { result, usage, .. } = fixture.cas.run(&request) else {
        return Err("side series".into());
    };
    let bytes = fixture
        .cas
        .export_witness(&result.witness, &mut budget()?)?;
    let mut receiver = Fixture::new()?;
    let function = named(&mut receiver, CaseName::Ramp)?;
    let approach = at(&mut receiver, "0", Some(Side::Above))?;
    let claim = expected(&receiver, &function, &approach, mode, &["0", "1", "0"])?;
    let mut ledger = budget()?;
    let proof = receiver.cas.replay(&bytes, &claim, &mut ledger)?;
    assert_eq!(receiver.cas.export_witness(&proof, &mut budget()?)?, bytes);
    assert!(ledger.usage().work <= usage.work.saturating_mul(10));
    Ok(())
}

/// A genuine analytic polynomial germ verifies the nonzero centre and its exact cubic remainder.
/// ´test:integration:case-analytic-series-remainder-lifecycle´
#[test]
fn case_analytic_series_remainder_lifecycle() -> TestResult {
    let mode = GermMode::Series {
        order: 2,
        analytic: true,
    };
    let mut fixture = Fixture::new()?;
    let function = cubic(&mut fixture)?;
    let approach = at(&mut fixture, "1", None)?;
    let request = fixture.request(task(&approach, mode), &[function])?;
    let result = fixture.proved(&request)?;
    let bytes = fixture
        .cas
        .export_witness(&result.witness, &mut budget()?)?;
    let mut receiver = Fixture::new()?;
    let function = cubic(&mut receiver)?;
    let approach = at(&mut receiver, "1", None)?;
    let claim = expected(&receiver, &function, &approach, mode, &["3", "5", "3"])?;
    receiver.cas.replay(&bytes, &claim, &mut budget()?)?;
    Ok(())
}
