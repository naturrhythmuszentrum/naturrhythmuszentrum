// SPDX-License-Identifier: AGPL-3.0-only
//! Original root coverage acquires physical meaning only through the complete retained transport.

use crate::testing::{
    lc_roots::TestResult,
    spectral::{ledger, prepared},
};

/// The entire original theorem and both simple roots survive the declared LC similarity.
/// ´test:integration:lc-spectral-original-transport´
#[test]
fn lc_spectral_original_transport() -> TestResult {
    let mut fixture = prepared(["5", "1/10", "1/2000"])?;
    let mut budget = ledger()?;
    let law = fixture.check(&mut budget)?;
    assert!(law.coverage().witness().is_unconditional());
    assert!(!law.is_unconditional());
    let names = law
        .transport_hypotheses()
        .iter()
        .map(|reference| fixture.source.cas.hypothesis_name(reference))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(names, vec![Some("positive-L"), Some("positive-C")]);
    assert_eq!(law.multiplicities(), [1, 1]);
    assert_ne!(law.roots()[0], law.roots()[1]);
    // One original physical construction, native hypothesis authentication and the owned law record.
    assert_eq!(
        (
            budget.usage().work,
            law.retained_payload_bytes(),
            fixture.construction_work
        ),
        (66_696, 832, 1_557_249)
    );
    Ok(())
}

/// Changed positive references and time scales require a new complete exact transport.
/// ´test:integration:lc-spectral-changed-reference-and-time-transport´
#[test]
fn lc_spectral_changed_reference_and_time_transport() -> TestResult {
    let mut fixture = prepared(["10", "1/5", "1/1000"])?;
    let law = fixture.check(&mut ledger()?)?;
    assert_eq!(law.multiplicities(), [1, 1]);
    let wrong_time = crate::testing::spectral::meaning(
        "duration",
        [0, 0, 1, 0, 0, 0, 0],
        "1/2000",
        &mut ledger()?,
    )?;
    let refused = nrz_bindings::readings::LcSpectralLaw::check(
        &mut fixture.source.cas,
        fixture.coverage.clone(),
        fixture.parameters.clone(),
        fixture.recipe.clone(),
        fixture.state.clone(),
        wrong_time,
        &mut ledger()?,
    );
    assert!(matches!(
        refused,
        Err(nrz_bindings::readings::SpectralLawError::Transport)
    ));
    Ok(())
}

/// The explicit original-to-real parameter relation cannot reorder, duplicate or replace a slot.
/// ´test:integration:lc-spectral-parameter-transport-preserves-identities´
#[test]
fn lc_spectral_parameter_transport_preserves_identities() -> TestResult {
    use nrz_bindings::readings::LcSpectralLaw;
    let mut fixture = prepared(["5", "1/10", "1/2000"])?;
    for parameters in [
        [fixture.parameters[1].clone(), fixture.parameters[0].clone()],
        [fixture.parameters[0].clone(), fixture.parameters[0].clone()],
        [
            fixture.recipe.inputs()[0].clone(),
            fixture.recipe.inputs()[1].clone(),
        ],
    ] {
        assert!(
            LcSpectralLaw::check(
                &mut fixture.source.cas,
                fixture.coverage.clone(),
                parameters,
                fixture.recipe.clone(),
                fixture.state.clone(),
                fixture.time.clone(),
                &mut ledger()?
            )
            .is_err()
        );
    }
    let law = fixture.check(&mut ledger()?)?;
    assert_eq!(law.parameters(), &fixture.parameters);
    assert_ne!(law.parameters().as_slice(), law.operator().inputs());
    Ok(())
}
