// SPDX-License-Identifier: AGPL-3.0-only
//! Empty-cache generic discovery interaction tests, independent of the public solve lifecycle.

use crate::limits::{Budget, Limits};
use crate::solving::radicals::discovery::Discovery;
use crate::solving::symmetric::cursor::Seed;
use crate::testing::common::TestResult;
use crate::testing::solve_blind::{derivative, multiply};
use crate::testing::solve_orbits::Fixture;

/// The first useful prefix follows generic character powers and skips nondecreasing orbit sizes.
/// ´test:crate:blind-discovery-reachable-prefix´
#[test]
fn blind_discovery_reachable_prefix() -> TestResult {
    let fixture = Fixture::new(3)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let mut discovery = Discovery::new(3, 0, &mut budget)?;
    let candidate = discovery
        .next(
            &fixture.frame,
            &fixture.unity,
            &fixture.base.cas.state,
            &mut budget,
        )?
        .ok_or("no strict descent")?;
    assert_eq!(candidate.resolvent.coefficients.len(), 3);
    assert_eq!(candidate.resolvent.depth, 1);
    let enumeration: Vec<_> = discovery
        .attempts
        .iter()
        .map(|attempt| (&attempt.seed, attempt.cardinality))
        .collect();
    assert_eq!(
        enumeration,
        [
            (&Seed::Cyclic { index: 1, power: 1 }, 6),
            (&Seed::Cyclic { index: 1, power: 2 }, 6),
            (&Seed::Cyclic { index: 1, power: 3 }, 2),
        ]
    );
    assert!(budget.usage().coefficient_ops > 0);
    assert_eq!(crate::solving::active_schema_bitmap() & 0x7fff, 0x7fff);
    Ok(())
}

/// Reversing primitive-root orientation independently reaches the same strict-degree prefix.
/// ´test:crate:blind-reversed-primitive-orientation´
#[test]
fn blind_reversed_primitive_orientation() -> TestResult {
    let fixture = Fixture::new(3)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let engine = crate::solving::coefficients::arithmetic::Engine {
        state: &fixture.base.cas.state,
        domain: &fixture.frame.domain,
    };
    let reversed = engine.pow(&fixture.unity, 2, &mut budget)?;
    let mut forward = Discovery::new(3, 0, &mut budget)?;
    let mut backward = Discovery::new(3, 0, &mut budget)?;
    let first = forward
        .next(
            &fixture.frame,
            &fixture.unity,
            &fixture.base.cas.state,
            &mut budget,
        )?
        .ok_or("forward descent missing")?;
    let second = backward
        .next(
            &fixture.frame,
            &reversed,
            &fixture.base.cas.state,
            &mut budget,
        )?
        .ok_or("reverse descent missing")?;
    for (left, right) in first
        .resolvent
        .coefficients
        .iter()
        .zip(&second.resolvent.coefficients)
    {
        assert!(
            left.expression
                .equals(&right.expression, &fixture.base.cas.state, &mut budget)?
        );
    }
    Ok(())
}

/// Pairing is discovered from cross-character products rather than an input constraint or cached lemma.
/// ´test:crate:blind-discovered-linked-product´
#[test]
fn blind_discovered_linked_product() -> TestResult {
    let fixture = Fixture::new(3)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let pairing = crate::solving::radicals::pairing::discover(
        &fixture.frame,
        &fixture.unity,
        3,
        &fixture.base.cas.state,
        &mut budget,
    )?
    .ok_or("pairing missing")?;
    assert_eq!(pairing.powers, [1, 1]);
    let expanded = pairing.reduction.expression.compose(
        &pairing.reduction.basis,
        &fixture.base.cas.state,
        &mut budget,
    )?;
    assert!(expanded.equals(
        &pairing.reduction.source,
        &fixture.base.cas.state,
        &mut budget
    )?);
    Ok(())
}

/// Named radical identities are evaluated only after the empty-cache search returns its complete product.
/// ´test:crate:blind-bombelli-independent-oracle´
#[test]
fn blind_bombelli_independent_oracle() -> TestResult {
    crate::testing::solve_blind::independent_product(&[-4, -15, 0, 1])?;
    let (real, imaginary) = (2_i64, 1_i64);
    assert_eq!(
        (
            real.pow(3) - 3 * real * imaginary.pow(2),
            3 * real.pow(2) * imaginary - imaginary.pow(3)
        ),
        (2, 11)
    );
    assert_eq!(real.pow(2) + imaginary.pow(2), 5);
    // The two irrational roots are -2 plus/minus sqrt(3); their quadratic is x²+4x+1.
    assert_eq!(multiply(&[-4, 1], &[1, 4, 1])?, [-4, -15, 0, 1]);
    assert_eq!(derivative(&[-4, -15, 0, 1])?, [-15, 0, 3]);
    Ok(())
}

/// Rational sign intervals and the rational-root test independently certify the named real cubic.
/// ´test:crate:blind-casus-independent-oracle´
#[test]
fn blind_casus_independent_oracle() -> TestResult {
    crate::testing::solve_blind::independent_product(&[1, -3, 0, 1])?;
    let evaluate = |value: i64| value.pow(3) - 3 * value + 1;
    for (left, right) in [(-2, -1), (0, 1), (1, 2)] {
        assert!(evaluate(left) * evaluate(right) < 0);
    }
    for candidate in [-1, 1] {
        assert_ne!(evaluate(candidate), 0);
    }
    // Substituting x=t+t^-1 and clearing t³ gives t⁶+t³+1.
    let cube = multiply(&multiply(&[1, 0, 1], &[1, 0, 1])?, &[1, 0, 1])?;
    let correction = [0, 0, -3, 1, -3, 0, 0];
    let result: Vec<_> = cube
        .iter()
        .zip(correction)
        .map(|(left, right)| left + right)
        .collect();
    assert_eq!(result, [1, 0, 0, 1, 0, 0, 1]);
    Ok(())
}

/// Derivative multiplicities and a cleared-denominator Bezout identity are independent of search arithmetic.
/// ´test:crate:blind-repeated-independent-oracle´
#[test]
fn blind_repeated_independent_oracle() -> TestResult {
    crate::testing::solve_blind::independent_product(&[2, -3, 0, 1])?;
    let original = |value: i64| value.pow(3) - 3 * value + 2;
    let first_derivative = |value: i64| 3 * value.pow(2) - 3;
    assert_eq!((original(1), first_derivative(1)), (0, 0));
    assert_eq!(
        crate::testing::solve_blind::derivative(&crate::testing::solve_blind::derivative(&[
            2, -3, 0, 1
        ])?)?,
        [0, 6]
    );
    assert_eq!((original(-2), first_derivative(-2)), (0, 9));
    // -3 f + x f' = 6(x-1), so gcd(f,f')=x-1 with exact rational Bezout coefficients.
    let original_coefficients = [2, -3, 0, 1];
    let shifted = multiply(
        &[0, 1],
        &crate::testing::solve_blind::derivative(&original_coefficients)?,
    )?;
    let bezout: Vec<_> = original_coefficients
        .iter()
        .zip(shifted)
        .map(|(value, differentiated)| -3 * value + differentiated)
        .collect();
    assert_eq!(bezout, [-6, 6, 0, 0]);
    assert_eq!(
        multiply(&multiply(&[-1, 1], &[-1, 1])?, &[2, 1])?,
        original_coefficients
    );
    Ok(())
}

/// The zero-pairing case is covered without inverting a zero selected character.
/// ´test:crate:blind-zero-pairing-independent-oracle´
#[test]
fn blind_zero_pairing_independent_oracle() -> TestResult {
    crate::testing::solve_blind::independent_product(&[8, 0, 0, 1])?;
    assert_eq!(multiply(&[2, 1], &[4, -2, 1])?, [8, 0, 0, 1]);
    assert_eq!(derivative(&[8, 0, 0, 1])?, [0, 0, 3]);
    Ok(())
}

/// Independent source parameters require exact regional guards; no formal-nonzero assumption is inferred.
/// ´test:crate:blind-symbolic-source-retains-parameters´
#[test]
fn blind_symbolic_source_retains_parameters() -> TestResult {
    for names in [["d", "c", "b", "a"], ["delta", "gamma", "beta", "alpha"]] {
        let (mut fixture, request) = crate::testing::solve_blind::symbolic(&names)?;
        let outcome = fixture.cas.run(&request);
        assert!(matches!(
            outcome,
            crate::request::Outcome::Unresolved {
                solve_reason: Some(crate::solving::SolveReason::SpecializationGuardPending),
                ..
            }
        ));
    }
    Ok(())
}

/// Resuming the cursor reaches the other character without restarting or caching a formula.
/// ´test:crate:blind-complete-cyclic-prefix´
#[test]
fn blind_complete_cyclic_prefix() -> TestResult {
    let fixture = Fixture::new(3)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let mut discovery = Discovery::new(3, 0, &mut budget)?;
    for _ in 0..2 {
        assert!(
            discovery
                .next(
                    &fixture.frame,
                    &fixture.unity,
                    &fixture.base.cas.state,
                    &mut budget
                )?
                .is_some()
        );
    }
    let enumeration: Vec<_> = discovery
        .attempts
        .iter()
        .map(|attempt| (&attempt.seed, attempt.cardinality))
        .collect();
    assert_eq!(
        enumeration,
        [
            (&Seed::Cyclic { index: 1, power: 1 }, 6),
            (&Seed::Cyclic { index: 1, power: 2 }, 6),
            (&Seed::Cyclic { index: 1, power: 3 }, 2),
            (&Seed::Cyclic { index: 2, power: 1 }, 6),
            (&Seed::Cyclic { index: 2, power: 2 }, 6),
            (&Seed::Cyclic { index: 2, power: 3 }, 2),
        ]
    );
    Ok(())
}

/// A computed primitive root stages each relation, selection, quotient and scalar map through admission.
/// ´test:crate:blind-staged-root-objects´
#[test]
fn blind_staged_root_objects() -> TestResult {
    use crate::solving::radicals::construction::Construction;
    let (mut fixture, _) = crate::testing::solve_blind::polynomial(&[1, 1])?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let mut construction = Construction {
        header: fixture.header.clone(),
        context: fixture.context.clone(),
        region: fixture.region.clone(),
        steps: Vec::new(),
    };
    let map = construction.mapping(&mut fixture.cas, &fixture.algebra, &[], &mut budget)?;
    let crate::solving::SolveView::CoefficientMap(mapping) = fixture.cas.solve_view(&map)? else {
        return Err("staged scalar map".into());
    };
    assert_eq!(mapping.mode, crate::solving::MapMode::Specialization);
    let engine = crate::solving::coefficients::arithmetic::Engine {
        state: &fixture.cas.state,
        domain: &fixture.algebra,
    };
    let defining = crate::solving::towers::cyclotomic::polynomial(&engine, 3, &mut budget)?;
    let extension = construction
        .adjoin(&mut fixture.cas, &defining, &map, Some(3), &mut budget)
        .map_err(|error| format!("staged adjunction: {error:?}"))?;
    assert!(matches!(
        fixture.cas.solve_view(&extension.domain)?,
        crate::solving::SolveView::CoefficientDomain(
            crate::solving::CoefficientDomainData::Quotient { .. }
        )
    ));
    assert!(matches!(
        fixture.cas.solve_view(&extension.selection)?,
        crate::solving::SolveView::RootSelection(_)
    ));
    assert!(matches!(
        fixture.cas.expression(&extension.root)?.0,
        crate::syntax::Operator::RootValue { .. }
    ));
    Ok(())
}

/// The first refused primitive is reported before later coverage or presentation can obscure its cause.
/// ´test:crate:blind-incremental-proof-replay´
#[test]
fn blind_incremental_proof_replay() -> TestResult {
    crate::testing::solve_blind::prefixes(&[-4, -15, 0, 1])
}

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
