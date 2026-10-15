// SPDX-License-Identifier: AGPL-3.0-only
//! Original-domain elementary certificates replay in independently constructed stores.

use crate::testing::common::TestResult;
use crate::testing::solve_elementary::{Fixture, hole, translation};
use nrz_cas::limits::{Budget, Limits};
use nrz_cas::proof::ApplicationDraft;

/// Set up each independent certificate from its displayed input mathematics.
fn supplied(case: u8) -> TestResult<(Fixture, ApplicationDraft)> {
    match case {
        0 => {
            let (mut fixture, problem) = hole()?;
            let entry = fixture.integer(-1, false)?;
            let step = fixture.member(&problem, &entry)?;
            Ok((fixture, step))
        }
        1 => translation(-1),
        _ => {
            let mut fixture = Fixture::new()?;
            let coefficient = fixture.free("a")?;
            let step = fixture.split(&coefficient)?;
            Ok((fixture, step))
        }
    }
}

/// Membership, reversible translation and full degree partitions all survive fresh canonical replay.
/// ´test:integration:elementary-guarded-fresh-replay´
#[test]
fn elementary_guarded_fresh_replay() -> TestResult {
    for case in 0..3 {
        let (mut source, step) = supplied(case)?;
        let witness = source.cas.check_proof(
            &source.context,
            std::slice::from_ref(&step),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let bytes = source
            .cas
            .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
        let (mut target, expected_step) = supplied(case)?;
        let expected = target.cas.check_proof(
            &target.context,
            std::slice::from_ref(&expected_step),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let checked = target.cas.replay(
            &bytes,
            expected.judgment(),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        assert_eq!(checked.judgment(), expected.judgment());
        assert_eq!(checked.judgment().residuals, []);
        assert_eq!(checked.hypotheses(), []);
        assert_eq!(
            target
                .cas
                .export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?,
            bytes
        );
    }
    Ok(())
}
