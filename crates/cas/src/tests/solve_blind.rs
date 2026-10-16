// SPDX-License-Identifier: AGPL-3.0-only
//! Reserved blind solving tests for the owning lane.

/// Recording generic observer events changes neither the candidate nor non-observer work.
/// ´test:crate:blind-resolvent-observer-equivalence´
#[test]
fn blind_resolvent_observer_equivalence() -> crate::testing::common::TestResult {
    let fixture = crate::testing::solve_orbits::Fixture::new(3)?;
    let state = &fixture.base.cas.state;
    let mut setup = crate::limits::Budget::new(crate::limits::Limits::blind_cubic())?;
    let seed = crate::solving::symmetric::cursor::Seed::Cyclic { index: 1, power: 3 };
    let polynomial = crate::solving::symmetric::seeds::generate(
        &fixture.frame,
        &seed,
        Some(&fixture.unity),
        state,
        &mut setup,
    )?;
    let orbit = crate::solving::orbits::produce::enumerate(&polynomial, state, &mut setup)?;
    let mut plain = crate::limits::Budget::new(crate::limits::Limits::blind_cubic())?;
    let first = crate::solving::symmetric::resolvent::smaller(&orbit, 3, 0, state, &mut plain)?
        .ok_or("plain descent")?;
    let mut observed = crate::limits::Budget::new(crate::limits::Limits::blind_cubic())?;
    let mut events = Vec::new();
    let second = crate::solving::symmetric::resolvent::smaller_observed(
        &orbit,
        3,
        0,
        state,
        &mut observed,
        |phase, index, budget| {
            crate::solving::coefficients::storage::reserve(&mut events, 1, budget)?;
            events.push((phase, index));
            Ok(())
        },
    )?
    .ok_or("observed descent")?;
    let expected_events = 2 * orbit.members.len()
        + 6 * first.coefficients.len()
        + 2 * usize::try_from(plain.usage().symmetric_steps)?;
    assert_eq!(events.len(), expected_events);
    let mut actual = observed.usage();
    actual.work = actual
        .work
        .checked_sub(u64::try_from(events.len())?)
        .ok_or("observer work")?;
    assert_eq!(actual, plain.usage());
    assert_eq!(first.depth, second.depth);
    assert_eq!(first.coefficients.len(), second.coefficients.len());
    for (left, right) in first.coefficients.iter().zip(&second.coefficients) {
        assert!(left.source.equals(&right.source, state, &mut setup)?);
        assert!(
            left.expression
                .equals(&right.expression, state, &mut setup)?
        );
        assert_eq!(left.basis.len(), right.basis.len());
        for (left, right) in left.basis.iter().zip(&right.basis) {
            assert!(left.equals(right, state, &mut setup)?);
        }
    }
    Ok(())
}

/// Algebraic substitution reconstructs the action independently of exponent permutation.
fn permutation_matches(
    fixture: &mut crate::testing::solve_orbits::Fixture,
    source: &crate::solving::symmetric::algebra::Polynomial,
    permutation: &[u32],
) -> crate::testing::common::TestResult {
    let mut budget = crate::limits::Budget::new(crate::limits::Limits::blind_cubic())?;
    let transformed = source.permute(permutation, &fixture.base.cas.state, &mut budget)?;
    let mut images = Vec::new();
    for slot in permutation {
        images.push(source.variable(*slot, &fixture.base.cas.state, &mut budget)?);
    }
    let expected = source.compose(&images, &fixture.base.cas.state, &mut budget)?;
    assert!(transformed.equals(&expected, &fixture.base.cas.state, &mut budget)?);
    let actual = transformed.stage(&mut fixture.base.cas, &mut budget)?;
    let expected = expected.stage(&mut fixture.base.cas, &mut budget)?;
    assert_eq!(actual, expected);
    Ok(())
}

/// Every actual cyclic power and both unity orientations preserve exact permutation substitution.
/// ´test:crate:blind-permutation-substitution-equivalence´
#[test]
fn blind_permutation_substitution_equivalence() -> crate::testing::common::TestResult {
    let mut fixture = crate::testing::solve_orbits::Fixture::new(3)?;
    let mut budget = crate::limits::Budget::new(crate::limits::Limits::blind_cubic())?;
    let engine = crate::solving::coefficients::arithmetic::Engine {
        state: &fixture.base.cas.state,
        domain: &fixture.frame.domain,
    };
    let reversed = engine.pow(&fixture.unity, 2, &mut budget)?;
    let permutations = crate::solving::orbits::produce::adjacent(3, &mut budget)?;
    for reverse in [false, true] {
        for power in 1..=3 {
            let unity = if reverse { &reversed } else { &fixture.unity };
            let source = crate::solving::symmetric::seeds::generate(
                &fixture.frame,
                &crate::solving::symmetric::cursor::Seed::Cyclic { index: 1, power },
                Some(unity),
                &fixture.base.cas.state,
                &mut budget,
            )?;
            for permutation in &permutations {
                permutation_matches(&mut fixture, &source, permutation)?;
            }
        }
    }
    let first = fixture
        .frame
        .variable(0, &fixture.base.cas.state, &mut budget)?;
    let second = fixture
        .frame
        .variable(1, &fixture.base.cas.state, &mut budget)?
        .pow(2, &fixture.base.cas.state, &mut budget)?;
    let mixed = first.add(&second, &fixture.base.cas.state, &mut budget)?;
    for permutation in &permutations {
        permutation_matches(&mut fixture, &mixed, permutation)?;
    }
    Ok(())
}
