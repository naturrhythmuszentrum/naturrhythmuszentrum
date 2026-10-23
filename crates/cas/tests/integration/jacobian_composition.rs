// SPDX-License-Identifier: AGPL-3.0-only
//! Independent chain-rule oracles and fresh whole-family composition replay.

use crate::testing::common::TestResult;
use crate::testing::jacobian_composition::{
    CompositionCase, empty, nonlinear, overlapping, reciprocal, varying_coefficient,
};
use crate::testing::map_inverses::ledger;
use nrz_cas::jacobians::ComposedJacobian;

fn compose(case: &mut CompositionCase) -> TestResult<ComposedJacobian> {
    Ok(case.fixture.cas.compose_reading_jacobian(
        &case.fixture.context,
        &case.original,
        &case.coordinates,
        &case.readings,
        &mut ledger()?,
    )?)
}

fn check(mut source: CompositionCase, mut receiver: CompositionCase) -> TestResult {
    let result = compose(&mut source)?;
    source.fixture.cas.prove_reading_equality(
        &source.fixture.context,
        result.direct().matrix(),
        &source.oracle,
        &mut ledger()?,
    )?;
    let equality = source
        .fixture
        .cas
        .export_witness(result.equality(), &mut ledger()?)?;
    let map = source
        .fixture
        .cas
        .export_witness(result.map_representation(), &mut ledger()?)?;
    let derivative = source
        .fixture
        .cas
        .export_witness(result.direct().witness(), &mut ledger()?)?;
    let rebuilt = compose(&mut receiver)?;
    receiver
        .fixture
        .cas
        .replay(&equality, rebuilt.equality().judgment(), &mut ledger()?)?;
    receiver.fixture.cas.replay(
        &map,
        rebuilt.map_representation().judgment(),
        &mut ledger()?,
    )?;
    receiver.fixture.cas.replay(
        &derivative,
        rebuilt.direct().witness().judgment(),
        &mut ledger()?,
    )?;
    receiver.fixture.cas.prove_reading_equality(
        &receiver.fixture.context,
        rebuilt.direct().matrix(),
        &receiver.oracle,
        &mut ledger()?,
    )?;
    assert_eq!(result.original_key(), rebuilt.original_key());
    assert_eq!(result.readings().variables, rebuilt.readings().variables);
    assert_eq!(result.direct().key(), rebuilt.direct().key());
    Ok(())
}

/// Diagonal, triangular and general maps obey chain rule for nonlinear and direct source use.
/// ´test:integration:reading-jacobian-chain-rule-oracles´
#[test]
fn reading_jacobian_chain_rule_oracles() -> TestResult {
    for kind in 0..3 {
        check(nonlinear(kind)?, nonlinear(kind)?)?;
    }
    Ok(())
}

/// Source-dependent map coefficients contribute their own exact derivatives.
/// ´test:integration:reading-jacobian-varying-map-coefficients´
#[test]
fn reading_jacobian_varying_map_coefficients() -> TestResult {
    check(varying_coefficient()?, varying_coefficient()?)
}

/// A positive source discharges the reciprocal domain after checked zero-map simplification.
/// ´test:integration:reading-jacobian-reciprocal-domain´
#[test]
fn reading_jacobian_reciprocal_domain() -> TestResult {
    check(reciprocal()?, reciprocal()?)
}

/// Empty readings and source axes produce an exact empty derivative matrix.
/// ´test:integration:reading-jacobian-empty-map´
#[test]
fn reading_jacobian_empty_map() -> TestResult {
    check(empty()?, empty()?)
}

/// The state and component readings share two directions, including the repeated first entry.
/// ´test:integration:reading-jacobian-overlap-introduces-no-direction´
#[test]
fn reading_jacobian_overlap_introduces_no_direction() -> TestResult {
    let mut case = overlapping()?;
    let result = compose(&mut case)?;
    assert_eq!(result.direct().key().coordinates().sources, ["u0", "u1"]);
    assert_eq!(
        case.fixture.cas.expression(result.direct().matrix())?.0,
        &nrz_cas::syntax::Operator::Matrix {
            rows: 1,
            columns: 2
        }
    );
    check(overlapping()?, overlapping()?)
}

/// Changed reading coefficients cannot replay the old map or composed-equality theorem.
/// ´test:integration:reading-jacobian-replay-rejects-changed-map´
#[test]
fn reading_jacobian_replay_rejects_changed_map() -> TestResult {
    let mut source = nonlinear(0)?;
    let result = compose(&mut source)?;
    let equality = source
        .fixture
        .cas
        .export_witness(result.equality(), &mut ledger()?)?;
    let map = source
        .fixture
        .cas
        .export_witness(result.map_representation(), &mut ledger()?)?;
    let mut receiver = nonlinear(1)?;
    let changed = compose(&mut receiver)?;
    assert_ne!(result.direct().key(), changed.direct().key());
    let before = receiver.fixture.cas.counts();
    assert!(
        receiver
            .fixture
            .cas
            .replay(&equality, changed.equality().judgment(), &mut ledger()?)
            .is_err()
    );
    assert!(
        receiver
            .fixture
            .cas
            .replay(
                &map,
                changed.map_representation().judgment(),
                &mut ledger()?
            )
            .is_err()
    );
    assert_eq!(receiver.fixture.cas.counts(), before);
    Ok(())
}
