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
