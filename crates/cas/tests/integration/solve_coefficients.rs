// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical coefficient closures survive the ordinary typed witness lifecycle.

use crate::testing::common::TestResult;
use nrz_cas::Cas;
use nrz_cas::limits::{Budget, Limits};
use nrz_cas::proof::VerifiedWitness;

/// A checked fraction identity keeps its explicit scoped nonzero hypothesis in the reachable proof.
fn fraction_witness() -> TestResult<(Cas, VerifiedWitness)> {
    let (mut fixture, steps) = crate::testing::solve_coefficients::fraction_identity(true)?;
    let witness = fixture.cas.check_proof(
        &fixture.context,
        &steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    Ok((fixture.cas, witness))
}

/// Fresh replay revalidates multivariate canonical fractions and retains the explicit used hypothesis.
/// ´test:integration:fraction-closure-replay´
#[test]
fn fraction_closure_replay() -> TestResult {
    let (source, witness) = fraction_witness()?;
    let bytes = source.export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
    let (mut target, expected) = fraction_witness()?;
    let replayed = target.replay(
        &bytes,
        expected.judgment(),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(replayed.judgment(), expected.judgment());
    assert_eq!(replayed.hypotheses().len(), 1);
    assert_eq!(
        target.export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}

/// Exported full component cover replays both children, their exact root restrictions and the derivative unit.
/// ´test:integration:tower-components-fresh-replay´
#[test]
fn tower_components_fresh_replay() -> TestResult {
    use crate::testing::solve_coefficients::SplitFixture;
    let mut source = SplitFixture::new()?;
    let witness = source.unit.base.cas.check_proof(
        &source.unit.base.context,
        &source.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let bytes = source
        .unit
        .base
        .cas
        .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
    let mut target = SplitFixture::new()?;
    let expected = target.unit.base.cas.check_proof(
        &target.unit.base.context,
        &target.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let replayed = target.unit.base.cas.replay(
        &bytes,
        expected.judgment(),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(replayed.judgment(), expected.judgment());
    assert_eq!(replayed.hypotheses(), []);
    assert_eq!(replayed.judgment().residuals, []);
    assert_eq!(
        target
            .unit
            .base
            .cas
            .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}

/// The Gaussian identity has a discharged, byte-stable fresh-store replay lifecycle.
/// ´test:integration:coefficient-gaussian-fresh-replay´
#[test]
fn coefficient_gaussian_fresh_replay() -> TestResult {
    use crate::testing::solve_coefficients::gaussian_identity;
    let (mut source, step) = gaussian_identity(11)?;
    let witness = source.cas.check_proof(
        &source.context,
        std::slice::from_ref(&step),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let bytes = source
        .cas
        .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
    let (mut target, expected_step) = gaussian_identity(11)?;
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
    assert_eq!(checked.hypotheses(), []);
    assert_eq!(
        target
            .cas
            .export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}
