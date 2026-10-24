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
        (65_588, 832, 1_463_735)
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

/// A changed voltage/current similarity cannot borrow the old operator correspondence.
/// ´test:integration:lc-spectral-refuses-mismatched-state-similarity´
#[test]
fn lc_spectral_refuses_mismatched_state_similarity() -> TestResult {
    use nrz_bindings::readings::{CoordinateNormalization, LcSpectralLaw, SpectralLawError};
    let mut fixture = prepared(["5", "1/10", "1/2000"])?;
    for references in [["10", "1/10"], ["5", "1/5"]] {
        let entries = [
            crate::testing::spectral::meaning(
                "voltage",
                [2, 1, -3, -1, 0, 0, 0],
                references[0],
                &mut ledger()?,
            )?,
            crate::testing::spectral::meaning(
                "current",
                [0, 0, 0, 1, 0, 0, 0],
                references[1],
                &mut ledger()?,
            )?,
        ];
        let changed = CoordinateNormalization::new(
            &mut fixture.source.cas,
            fixture.recipe.frame().context(),
            &entries,
            &mut ledger()?,
        )?;
        let result = LcSpectralLaw::check(
            &mut fixture.source.cas,
            fixture.coverage.clone(),
            fixture.parameters.clone(),
            fixture.recipe.clone(),
            changed,
            fixture.time.clone(),
            &mut ledger()?,
        );
        assert!(matches!(result, Err(SpectralLawError::Transport)));
    }
    Ok(())
}

/// A valid general reciprocal operator still cannot claim the restricted LC spectral theorem.
/// ´test:integration:lc-spectral-refuses-general-reciprocal-operators´
#[test]
fn lc_spectral_refuses_general_reciprocal_operators() -> TestResult {
    use nrz_bindings::readings::{LcSpectralLaw, SpectralLawError};
    let mut fixture = prepared(["5", "1/10", "1/2000"])?;
    for changed in 0..3 {
        let operator = changed_operator(&mut fixture, changed)?;
        let result = LcSpectralLaw::check(
            &mut fixture.source.cas,
            fixture.coverage.clone(),
            fixture.parameters.clone(),
            operator,
            fixture.state.clone(),
            fixture.time.clone(),
            &mut ledger()?,
        );
        if changed == 0 {
            assert!(matches!(result, Err(SpectralLawError::Sign)));
        } else {
            assert!(matches!(result, Err(SpectralLawError::NonzeroDiagonal)));
        }
    }
    Ok(())
}

/// Each altered original is independently admitted as a two-by-two native map before correspondence.
fn changed_operator(
    fixture: &mut crate::testing::spectral::SpectralFixture,
    changed: usize,
) -> TestResult<nrz_bindings::readings::PositiveReciprocalMap> {
    use nrz_bindings::readings::{PositiveReciprocalMap, ReciprocalMapDefinition};
    let mut budget = ledger()?;
    let (map, values) = changed_coefficients(fixture, changed, &mut budget)?;
    let zero = values[3].clone();
    let original = fixture.source.cas.exact_reading_map(
        fixture.recipe.frame().context(),
        (2, 2),
        &values,
        &[zero.clone(), zero],
        &mut budget,
    )?;
    Ok(PositiveReciprocalMap::prove(
        &mut fixture.source.cas,
        fixture.recipe.frame(),
        ReciprocalMapDefinition {
            inputs: fixture.recipe.inputs().to_vec(),
            shape: [2, 2],
            map,
            original,
        },
        &mut budget,
    )?)
}

/// Wrong signs, constant nonzero diagonals and diagonals zero only at the reference point are distinct cases.
fn changed_coefficients(
    fixture: &mut crate::testing::spectral::SpectralFixture,
    changed: usize,
    budget: &mut nrz_bindings::Budget,
) -> TestResult<(nrz_cas::readings::AffineMap, [nrz_cas::Handle; 4])> {
    use nrz_cas::{Rational, readings::AffineMap, syntax::Operator};
    let mut coefficients = fixture.recipe.map().coefficients().to_vec();
    let mut offsets = fixture.recipe.map().offsets().to_vec();
    let zero = altered_term(fixture, Operator::Rational(Rational::zero()), &[], budget)?;
    let one = altered_term(fixture, Operator::Rational(Rational::one()), &[], budget)?;
    let reciprocal = fixture.recipe.reciprocals()[0].clone();
    let upper = fixture.recipe.reciprocals()[1].clone();
    let lower = if changed == 0 {
        coefficients[2] = Rational::one();
        reciprocal.clone()
    } else {
        altered_term(
            fixture,
            Operator::Neg,
            std::slice::from_ref(&reciprocal),
            budget,
        )?
    };
    let diagonal = match changed {
        0 => zero.clone(),
        1 => {
            offsets[0] = Rational::one();
            one
        }
        _ => {
            coefficients[0] = Rational::one();
            offsets[0] = Rational::from_ratio(-1, 1, budget)?;
            altered_term(fixture, Operator::Sub, &[reciprocal, one], budget)?
        }
    };
    let map = AffineMap::new(2, 4, coefficients, offsets, budget)?;
    Ok((map, [diagonal, upper, lower, zero]))
}

/// Altered original terms keep the same physical input header and Real field.
fn altered_term(
    fixture: &mut crate::testing::spectral::SpectralFixture,
    operator: nrz_cas::syntax::Operator,
    operands: &[nrz_cas::Handle],
    budget: &mut nrz_bindings::Budget,
) -> TestResult<nrz_cas::Handle> {
    crate::testing::lc_roots::expression(
        &mut fixture.source,
        nrz_cas::syntax::Field::Real,
        operator,
        operands,
        budget,
    )
}
