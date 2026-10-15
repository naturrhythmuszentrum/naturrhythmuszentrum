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

/// Primitive order, linked roots and the complete product all replay together in a new store.
/// ´test:integration:binomial-fresh-replay´
#[test]
fn binomial_fresh_replay() -> TestResult {
    use crate::testing::solve_elementary::BinomialFixture;
    for translated in [false, true] {
        let mut source = if translated {
            BinomialFixture::translated()?
        } else {
            BinomialFixture::new(2, 2)?
        };
        let checked = source.base.cas.check_proof(
            &source.base.context,
            &source.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let bytes = source
            .base
            .cas
            .export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?;
        let mut target = if translated {
            BinomialFixture::translated()?
        } else {
            BinomialFixture::new(2, 2)?
        };
        let expected = target.base.cas.check_proof(
            &target.base.context,
            &target.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let replayed = target.base.cas.replay(
            &bytes,
            expected.judgment(),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        assert_eq!(replayed.judgment(), expected.judgment());
        assert_eq!(replayed.hypotheses(), []);
        assert_eq!(replayed.judgment().residuals, []);
        assert_eq!(
            target
                .base
                .cas
                .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
            bytes
        );
    }
    Ok(())
}

/// Constructively refined selections retain their exact target and uniqueness strength in fresh replay.
/// ´test:integration:root-refinement-fresh-replay´
#[test]
fn root_refinement_fresh_replay() -> TestResult {
    for unique in [false, true] {
        let (mut source, step) = crate::testing::solve_elementary::root_refinement(2, unique)?;
        let checked = source.cas.check_proof(
            &source.context,
            std::slice::from_ref(&step),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let bytes = source
            .cas
            .export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?;
        let (mut target, expected_step) =
            crate::testing::solve_elementary::root_refinement(2, unique)?;
        let expected = target.cas.check_proof(
            &target.context,
            std::slice::from_ref(&expected_step),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let replayed = target.cas.replay(
            &bytes,
            expected.judgment(),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        assert_eq!(replayed.judgment(), expected.judgment());
        assert_eq!(
            target
                .cas
                .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
            bytes
        );
    }
    Ok(())
}
