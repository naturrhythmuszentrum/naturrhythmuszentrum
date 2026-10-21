// SPDX-License-Identifier: AGPL-3.0-only
//! Empty-cache generic discovery interaction tests, independent of the public solve lifecycle.

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
            |_, _, _| Ok(()),
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
    assert_eq!(crate::solving::active_schema_bitmap() & 0x5fff, 0x5fff);
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
            |_, _, _| Ok(()),
        )?
        .ok_or("forward descent missing")?;
    let second = backward
        .next(
            &fixture.frame,
            &reversed,
            &fixture.base.cas.state,
            &mut budget,
            |_, _, _| Ok(()),
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

/// Both declaration namings retain the original coefficient occurrences and reject changed expected sources.
/// ´test:crate:blind-symbolic-source-retains-parameters´
#[test]
fn blind_symbolic_source_retains_parameters() -> TestResult {
    crate::testing::solve_blind::retained_source()
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
                    &mut budget,
                    |_, _, _| Ok(())
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

/// Independent boundary inventories retain declaration fields, initial maps and original forms.
/// The Complex boundaries reconstruct checked proof DAGs; the separate Real refusal remains earlier.
/// ´test:crate:blind-symbolic-acceptance-audit´
#[test]
fn blind_symbolic_acceptance_audit() -> TestResult {
    let revision = "blind-cubic-audit-fixture-revision";
    let record = crate::testing::solve_blind::symbolic_audit(revision)?;
    assert!(record.starts_with(&format!("source_revision={revision}\n")));
    assert!(record.contains("policy=logical-v10"));
    assert_eq!(record.matches("result=Solved(Radicals)").count(), 7);
    assert_eq!(record.matches("fresh_coverage=true").count(), 7);
    assert_eq!(record.matches("fresh_presentation=true").count(), 7);
    assert!(record.contains("refusal=RealIsolationUnavailable"));
    assert!(record.contains("interning_comparisons="));
    assert!(record.contains("phases="));
    assert_eq!(record.matches("declaration_field=Complex\n").count(), 7);
    assert_eq!(record.matches("declaration_field=Real\n").count(), 1);
    assert_eq!(record.matches("original_form=Radicals\n").count(), 8);
    assert!(record.contains("initial_map=None (propose refuses Real before coefficient_images)"));
    Ok(())
}

/// Replay exhaustion after coverage preserves its valid graph handles and portable theorem.
/// ´test:crate:blind-retained-coverage-replay-boundary´
#[test]
fn blind_retained_coverage_replay_boundary() -> TestResult {
    crate::testing::solve_blind::retained_replay_boundary()
}

/// Original full-factor coverage rejects missing, replaced and wrongly powered factors.
/// ´test:crate:blind-false-factor-certificates´
#[test]
fn blind_false_factor_certificates() -> TestResult {
    crate::testing::solve_blind::false_factors(&[8, 0, 0, 1])
}

/// Repeated-root factor occurrences remain necessary even when set members coincide.
/// ´test:crate:blind-repeated-factor-mutations´
#[test]
fn blind_repeated_factor_mutations() -> TestResult {
    crate::testing::solve_blind::false_factors(&[2, -3, 0, 1])
}

/// Every generated zero/nonzero split of accepted coverage rejects either missing child.
/// ´test:crate:blind-missing-stratum-mutations´
#[test]
fn blind_missing_stratum_mutations() -> TestResult {
    crate::testing::solve_blind::false_strata()
}

/// Inverting the primitive orientation transports the full checked recovered set.
/// ´test:crate:blind-recovered-orientation-transport´
#[test]
fn blind_recovered_orientation_transport() -> TestResult {
    crate::testing::solve_blind::transported_choices(true, 0)
}

/// A compatible alternative lifted radical choice permutes the full checked recovered set.
/// ´test:crate:blind-recovered-choice-transport´
#[test]
fn blind_recovered_choice_transport() -> TestResult {
    crate::testing::solve_blind::transported_choices(false, 1)
}

/// Independent declaration renaming preserves sampled factors and the complete stratum inventory.
/// ´test:crate:blind-complete-coefficient-renaming´
#[test]
fn blind_complete_coefficient_renaming() -> TestResult {
    crate::testing::solve_blind::renamed_complete_set()
}

/// Exercised tower, orbit, gcd, identity and recovery counters have isolated exact boundaries.
/// ´test:crate:blind-measured-resource-boundaries´
#[test]
fn blind_measured_resource_boundaries() -> TestResult {
    crate::testing::solve_blind::measured_boundaries()
}

/// Actual normalized graph members distinguish polynomial powers from cyclic rotations.
/// ´test:crate:blind-distinct-seed-power-orbits´
#[test]
fn blind_distinct_seed_power_orbits() -> TestResult {
    let mut fixture = Fixture::new(3)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let mut previous = Vec::new();
    for (power, cardinality) in [(1, 6), (2, 6), (3, 2)] {
        let polynomial = crate::solving::symmetric::seeds::generate(
            &fixture.frame,
            &Seed::Cyclic { index: 1, power },
            Some(&fixture.unity),
            &fixture.base.cas.state,
            &mut budget,
        )?;
        let orbit = crate::solving::orbits::produce::enumerate(
            &polynomial,
            &fixture.base.cas.state,
            &mut budget,
        )?;
        assert_eq!(orbit.members.len(), cardinality);
        let mut current = Vec::new();
        for member in &orbit.members {
            assert!(!member.form.terms.is_empty());
            for term in &member.form.terms {
                let degree: u64 = term
                    .powers
                    .iter()
                    .map(|(_, exponent)| u64::from(*exponent))
                    .sum();
                assert_eq!(degree, u64::from(power));
            }
            let handle = member.stage(&mut fixture.base.cas, &mut budget)?;
            assert!(!previous.contains(&handle));
            assert!(!current.contains(&handle));
            current.push(handle);
        }
        previous.extend(current);
        if cardinality >= 3 {
            let before = budget.usage().coefficient_ops;
            assert!(
                crate::solving::symmetric::resolvent::smaller(
                    &orbit,
                    3,
                    0,
                    &fixture.base.cas.state,
                    &mut budget
                )?
                .is_none()
            );
            assert_eq!(budget.usage().coefficient_ops, before);
        }
    }
    Ok(())
}

/// The symbolic linear denominator baseline identifies its actual first refused scoped rule.
/// ´test:crate:blind-symbolic-linear-prefix-replay´
#[test]
fn blind_symbolic_linear_prefix_replay() -> TestResult {
    crate::testing::solve_blind::symbolic_prefixes(&["b", "a"])
}

/// Individual radical powers remain valid when an incompatible character rotation destroys the product.
/// ´test:crate:blind-independent-character-choice-refused´
#[test]
fn blind_independent_character_choice_refused() -> TestResult {
    crate::testing::solve_blind::unlinked_choices()
}

/// The blind radical path consumes its scoped expansion only after exact parent closure and fresh replay.
/// ´test:crate:blind-formal-root-consumer´
#[test]
fn blind_formal_root_consumer() -> TestResult {
    crate::testing::solve_blind::formal_consumer()
}

/// The actual blind producer uses a checked real odd root and then proves its complex selection.
/// ´test:crate:blind-constructive-real-choice´
#[test]
fn blind_constructive_real_choice() -> TestResult {
    crate::testing::solve_blind::real_choice_consumer()
}

/// One Complex parameter and its zero cell retain the original immutable declaration frame.
fn complex_zero_scope() -> TestResult<(crate::Cas, crate::Handle, crate::Handle, crate::Handle)> {
    use crate::domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
    use crate::graph::{Draft, Reference};
    use crate::testing::solve_graph::admit;
    let (mut fixture, _) = crate::testing::solve_blind::symbolic(&["a"])?;
    let environment = &fixture
        .cas
        .state
        .header(fixture.header.logical_id())?
        .environment;
    assert!(environment.declarations.iter().all(|declaration| {
        declaration.sort
            == crate::syntax::Sort::Value(crate::syntax::AtomicSort::Scalar(
                crate::syntax::Field::Complex,
            ))
    }));
    let parameter = fixture.free("a")?;
    let zero = fixture.integer(0, false)?;
    let predicate = fixture.equation(&parameter, &zero)?;
    let region = admit(
        &mut fixture.cas,
        &Draft::Region(RegionDraft {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Product {
                coordinates: fixture.coefficients,
            },
            expression: RegionExpr::GuardTrue {
                base: Reference::Existing(fixture.region),
                predicate: Reference::Existing(predicate),
            },
        }),
    )?;
    let context = admit(
        &mut fixture.cas,
        &Draft::Context(ContextDraft::Root {
            header: Reference::Existing(fixture.header),
            admissible: Reference::Existing(region.clone()),
            assumptions: Vec::new(),
        }),
    )?;
    Ok((fixture.cas, fixture.algebra, context, region))
}

/// The only fraction coordinate is the declared Complex parameter a; its denominator is one.
fn complex_parameter(
    engine: &crate::solving::coefficients::arithmetic::Engine<'_>,
    budget: &mut Budget,
) -> TestResult<crate::solving::coefficients::model::Element> {
    use crate::solving::coefficients::{
        fraction::Fraction,
        model::Value,
        polynomial::{Polynomial, Term},
        scalar::Scalar,
    };
    let (ground, width, _) = engine.fraction_shape()?;
    assert_eq!(width, 1);
    let numerator = Polynomial::new(
        width,
        ground,
        vec![Term {
            powers: vec![1],
            coefficient: Scalar::constant(ground, true, budget)?,
        }],
        budget,
    )?;
    let denominator = numerator.one(budget)?;
    Ok(engine.element(Value::Fraction(Fraction::new(
        &numerator,
        &denominator,
        budget,
    )?)))
}

/// A regionally zero divisor is refused distinctly, and the gcd terminates on its zero branch.
/// ´test:crate:blind-complex-regional-zero-divisor´
#[test]
fn blind_complex_regional_zero_divisor() -> TestResult {
    use crate::solving::coefficients::{
        arithmetic::Engine,
        model::{EvaluationScope, UnitDecision, ZeroDecision},
    };
    use crate::solving::towers::{dense::Dense, euclid};
    let (cas, domain, context, region) = complex_zero_scope()?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let engine = Engine {
        state: &cas.state,
        domain: &domain,
    };
    let parameter = complex_parameter(&engine, &mut budget)?;
    let scope = EvaluationScope {
        context: &context,
        region: &region,
        premises: &[],
        localization: &[],
    };
    assert!(!engine.formal_zero(&parameter, &mut budget)?);
    assert!(matches!(
        engine.zero_test(&parameter, &scope, &mut budget)?,
        ZeroDecision::Zero(_)
    ));
    assert!(matches!(
        engine.inverse(&parameter, &scope, &mut budget)?,
        UnitDecision::Zero(_)
    ));
    let divisor = Dense::new(&engine, vec![parameter], &mut budget)?;
    assert_eq!(divisor.coefficients.len(), 1);
    let dividend = Dense::one(&engine, &mut budget)?;
    assert!(matches!(
        euclid::divide(&dividend, &divisor, &cas.state, &scope, &mut budget)?,
        Err(euclid::Pending::ZeroDivisor)
    ));
    let result = euclid::gcd(&dividend, &divisor, &cas.state, &scope, &mut budget)?
        .map_err(|pending| format!("regional gcd: {pending:?}"))?;
    assert!(result.gcd.equals(&dividend, &cas.state, &mut budget)?);
    Ok(())
}

/// A leading zero lowers the divisor degree without erasing the original coefficient polynomial.
/// ´test:crate:blind-complex-regional-degree-drop´
#[test]
fn blind_complex_regional_degree_drop() -> TestResult {
    use crate::solving::coefficients::{arithmetic::Engine, model::EvaluationScope};
    use crate::solving::towers::{dense::Dense, euclid};
    let (cas, domain, context, region) = complex_zero_scope()?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let engine = Engine {
        state: &cas.state,
        domain: &domain,
    };
    let scope = EvaluationScope {
        context: &context,
        region: &region,
        premises: &[],
        localization: &[],
    };
    let dividend = Dense::new(
        &engine,
        vec![
            engine.constant(true, &mut budget)?,
            engine.constant(true, &mut budget)?,
        ],
        &mut budget,
    )?;
    let divisor = Dense::new(
        &engine,
        vec![
            engine.constant(true, &mut budget)?,
            complex_parameter(&engine, &mut budget)?,
        ],
        &mut budget,
    )?;
    let (quotient, remainder) =
        euclid::divide(&dividend, &divisor, &cas.state, &scope, &mut budget)?
            .map_err(|pending| format!("regional division: {pending:?}"))?;
    assert!(quotient.equals(&dividend, &cas.state, &mut budget)?);
    assert_eq!(remainder.coefficients.len(), 0);
    assert_eq!(divisor.coefficients.len(), 2);
    Ok(())
}

/// Independently supplied regional Bezout data admits its exact zero-cell unit.
/// ´test:crate:blind-regional-unit-identity´
#[test]
fn blind_regional_unit_identity() -> TestResult {
    let (mut fixture, step) = crate::testing::solve_blind::regional_unit(true)?;
    let witness = fixture.cas.check_proof(
        &fixture.context,
        &[step],
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert!(witness.is_discharged());
    Ok(())
}

/// Construct only declared scalar guards q²+p³=0 and p≠0, with optional sibling mutations.
fn consequence_scope(
    relation_zero: bool,
    parameter_nonzero: bool,
) -> TestResult<(crate::Cas, crate::Handle, crate::Handle, crate::Handle)> {
    use crate::domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
    use crate::graph::{Draft, Reference};
    use crate::syntax::Operator;
    use crate::testing::solve_graph::admit;
    let (mut fixture, _) = crate::testing::solve_blind::symbolic(&["q", "p"])?;
    let first = fixture.free("q")?;
    let second = fixture.free("p")?;
    let square = fixture.expression(Operator::IntegerPower(2), &[first], false)?;
    let cube = fixture.expression(
        Operator::IntegerPower(3),
        std::slice::from_ref(&second),
        false,
    )?;
    let sum = fixture.expression(Operator::Add, &[square, cube], false)?;
    let origin = fixture.integer(0, false)?;
    for (value, zero) in [(sum, relation_zero), (second, !parameter_nonzero)] {
        let predicate = Reference::Existing(fixture.equation(&value, &origin)?);
        let base = Reference::Existing(fixture.region.clone());
        let expression = if zero {
            RegionExpr::GuardTrue { base, predicate }
        } else {
            RegionExpr::GuardFalse { base, predicate }
        };
        fixture.region = admit(
            &mut fixture.cas,
            &Draft::Region(RegionDraft {
                header: Reference::Existing(fixture.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Product {
                    coordinates: fixture.coefficients.clone(),
                },
                expression,
            }),
        )?;
    }
    fixture.context = admit(
        &mut fixture.cas,
        &Draft::Context(ContextDraft::Root {
            header: Reference::Existing(fixture.header),
            admissible: Reference::Existing(fixture.region.clone()),
            assumptions: Vec::new(),
        }),
    )?;
    Ok((
        fixture.cas,
        fixture.algebra,
        fixture.context,
        fixture.region,
    ))
}

/// The square identity proves q nonzero only on its exact zero relation and p≠0 cell.
/// ´test:crate:blind-regional-unit-consequence´
#[test]
fn blind_regional_unit_consequence() -> TestResult {
    use crate::solving::coefficients::{
        arithmetic::Engine,
        fraction::Fraction,
        model::{EvaluationScope, Value, ZeroDecision},
        polynomial::{Polynomial, Term},
        scalar::Scalar,
    };
    for (relation_zero, parameter_nonzero) in [(true, true), (false, true), (true, false)] {
        let (mut cas, domain, context, region) =
            consequence_scope(relation_zero, parameter_nonzero)?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let engine = Engine {
            state: &cas.state,
            domain: &domain,
        };
        let (ground, width, _) = engine.fraction_shape()?;
        assert_eq!(width, 2);
        let numerator = Polynomial::new(
            width,
            ground,
            vec![Term {
                powers: vec![1, 0],
                coefficient: Scalar::constant(ground, true, &mut budget)?,
            }],
            &mut budget,
        )?;
        let denominator = numerator.one(&mut budget)?;
        let value = engine.element(Value::Fraction(Fraction::new(
            &numerator,
            &denominator,
            &mut budget,
        )?));
        let scope = EvaluationScope {
            context: &context,
            region: &region,
            premises: &[],
            localization: &[],
        };
        let ZeroDecision::Undetermined(obligation) =
            engine.zero_test(&value, &scope, &mut budget)?
        else {
            return Err("uncertified consequence was used".into());
        };
        let mut construction = crate::solving::radicals::construction::Construction {
            header: cas.handle(cas.state.context(context.logical_id())?.header_id),
            context: context.clone(),
            region: region.clone(),
            steps: Vec::new(),
        };
        let theorem = crate::solving::radicals::consequence::certify(
            &mut cas,
            &mut construction,
            &obligation,
            &mut budget,
        )?;
        assert_eq!(theorem.is_some(), relation_zero && parameter_nonzero);
        if let Some(witness) = theorem {
            assert!(witness.is_discharged());
            let engine = Engine {
                state: &cas.state,
                domain: &domain,
            };
            let scope = EvaluationScope {
                premises: std::slice::from_ref(&witness),
                ..scope
            };
            assert!(matches!(
                engine.zero_test(&value, &scope, &mut budget)?,
                ZeroDecision::Nonzero(_)
            ));
        }
    }
    Ok(())
}

/// Long division checks the regional cancellation when the leading unit is not formally invertible.
/// ´test:crate:blind-complex-regional-cancellation´
#[test]
fn blind_complex_regional_cancellation() -> TestResult {
    use crate::proof::Statement;
    use crate::solving::{
        SolveStatement,
        coefficients::{arithmetic::Engine, model::EvaluationScope},
        towers::{dense::Dense, euclid},
    };
    let (fixture, step) = crate::testing::solve_blind::regional_unit(true)?;
    let Statement::Solve(statement) = step.conclusion else {
        return Err("unit statement".into());
    };
    let SolveStatement::Unit(unit) = *statement else {
        return Err("unit payload".into());
    };
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let engine = Engine {
        state: &fixture.cas.state,
        domain: &unit.domain,
    };
    let scope = EvaluationScope {
        context: &fixture.context,
        region: &fixture.region,
        premises: &[],
        localization: &[],
    };
    let dividend = Dense::new(
        &engine,
        vec![
            engine.constant(false, &mut budget)?,
            engine.constant(true, &mut budget)?,
        ],
        &mut budget,
    )?;
    let element = engine.read(&unit.value, &mut budget)?;
    assert!(matches!(
        engine.inverse(&element, &scope, &mut budget)?,
        crate::solving::coefficients::model::UnitDecision::Unit {
            certificate: crate::solving::coefficients::model::CoefficientCertificate::Regional(_),
            ..
        }
    ));
    let divisor = Dense::new(
        &engine,
        vec![engine.constant(true, &mut budget)?, element],
        &mut budget,
    )?;
    let (quotient, remainder) =
        euclid::divide(&dividend, &divisor, &fixture.cas.state, &scope, &mut budget)?
            .map_err(|pending| format!("regional cancellation: {pending:?}"))?;
    let one = Dense::one(&engine, &mut budget)?;
    assert!(quotient.equals_on(&one, &fixture.cas.state, &fixture.region, &mut budget)?);
    assert_eq!(remainder.coefficients.len(), 1);
    assert!(
        remainder
            .sum(&one, false, &fixture.cas.state, &mut budget)?
            .equals_on(
                &Dense::zero(&engine, &mut budget)?,
                &fixture.cas.state,
                &fixture.region,
                &mut budget
            )?
    );
    Ok(())
}

/// Nonreal source coefficients agree with retained factors without projection to Real.
/// ´test:crate:blind-nonreal-independent-oracle´
#[test]
fn blind_nonreal_independent_oracle() -> TestResult {
    let original = [(0, 2), (-2, 1), (-1, -1), (1, 0)];
    crate::testing::solve_blind::independent_gaussian_product(&original)?;
    // Multiply (x-i) by (x-2)(x+1)=x²-x-2 independently.
    let quadratic = [-2_i64, -1, 1];
    let mut product = [(0_i64, 0_i64); 4];
    for (power, coefficient) in quadratic.iter().enumerate() {
        product[power].1 -= coefficient;
        product[power + 1].0 += coefficient;
    }
    assert_eq!(product, original);
    Ok(())
}

/// A supplied power identity proves its scoped fact without invoking producer search.
/// ´test:crate:blind-supplied-nonzero-consequence´
#[test]
fn blind_supplied_nonzero_consequence() -> TestResult {
    let (mut fixture, step) = crate::testing::solve_blind::nonzero_consequence(true)?;
    let witness = fixture.cas.check_proof(
        &fixture.context,
        &[step],
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert!(witness.is_discharged());
    Ok(())
}

/// A symbolic fraction never enters an unrelated ground field through the constant fallback.
/// ´test:crate:blind-unrelated-fraction-is-not-ground´
#[test]
fn blind_unrelated_fraction_is_not_ground() -> TestResult {
    use crate::solving::coefficients::{arithmetic::Engine, maps};
    let (cas, domain, _, _) = complex_zero_scope()?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let engine = Engine {
        state: &cas.state,
        domain: &domain,
    };
    let parameter = complex_parameter(&engine, &mut budget)?;
    let crate::solving::CoefficientDomainData::Fraction { base, .. } =
        crate::solving::coefficients::storage::domain(&cas.state, &domain)?
    else {
        return Err("fraction source".into());
    };
    assert_ne!(domain, *base);
    assert!(maps::canonical(&cas.state, &parameter, base, &mut budget).is_err());
    assert!(maps::constant(&cas.state, &parameter, base, &mut budget).is_err());
    Ok(())
}

/// Projecting a Complex parameter changes its sort and cannot replace its exact-Free map image.
/// ´test:crate:blind-complex-parameter-real-part-refused´
#[test]
fn blind_complex_parameter_real_part_refused() -> TestResult {
    use crate::Reference;
    use crate::solving::{CoefficientMapData, MapTarget, SolveView};
    use crate::syntax::{AtomicSort, Draft, ExpressionDraft, Field, Operator, Sort};
    let (mut fixture, step) = crate::testing::solve_blind::nonzero_consequence(true)?;
    let crate::proof::Certificate::Solve(certificate) = &step.certificate else {
        return Err("map source certificate".into());
    };
    let crate::solving::SolveCertificate::CoefficientIdentity(identity) = certificate.as_ref()
    else {
        return Err("map source identity".into());
    };
    let SolveView::CoefficientMap(original) = fixture.cas.solve_view(&identity.map)? else {
        return Err("map source".into());
    };
    let original = original.clone();
    crate::solving::coefficients::homomorphism::check(
        &fixture.cas,
        &original,
        &[],
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let [parameter] = original.images.as_slice() else {
        return Err("map parameter inventory".into());
    };
    assert_eq!(
        fixture.cas.expression(parameter)?.0,
        &Operator::Free("a".into())
    );
    let projected = crate::testing::solve_graph::admit(
        &mut fixture.cas,
        &Draft::Expression(ExpressionDraft {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            operator: Operator::RealPart,
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            operands: vec![Reference::Existing(parameter.clone())],
            space: None,
            binding: None,
        }),
    )?;
    assert_ne!(projected, *parameter);
    let changed = CoefficientMapData {
        source: original.source,
        target: MapTarget::ScalarAmbient {
            header: fixture.header.clone(),
            field: Field::Complex,
        },
        mode: original.mode,
        region: original.region,
        images: vec![projected],
        denominators: original.denominators,
    };
    assert!(
        crate::solving::coefficients::homomorphism::check(
            &fixture.cas,
            &changed,
            &[],
            &mut Budget::new(Limits::blind_cubic())?
        )
        .is_err()
    );
    Ok(())
}

/// Returned symbolic factors independently specialize across all degrees and to a nonreal cubic.
/// ´test:crate:blind-symbolic-independent-specializations´
#[test]
fn blind_symbolic_independent_specializations() -> TestResult {
    crate::testing::solve_blind::independent_symbolic_product()
}

/// Exact graph, region, ground algebra, stored fraction and canonical parameter map.
type MappedDenominator = (
    crate::Cas,
    crate::Handle,
    crate::Handle,
    crate::Handle,
    crate::Handle,
);

/// Construct 1/p² over the declared q,p frame, retaining the independently chosen p cell.
fn mapped_denominator(nonzero: bool) -> TestResult<MappedDenominator> {
    use crate::solving::coefficients::{
        arithmetic::Engine,
        fraction::Fraction,
        model::Value,
        polynomial::{Polynomial, Term},
        scalar::Scalar,
    };
    let (mut cas, domain, context, region) = consequence_scope(false, nonzero)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let engine = Engine {
        state: &cas.state,
        domain: &domain,
    };
    let (ground, width, base) = engine.fraction_shape()?;
    assert_eq!(width, 2);
    let base = base.clone();
    let denominator = Polynomial::new(
        width,
        ground,
        vec![Term {
            powers: vec![0, 2],
            coefficient: Scalar::constant(ground, true, &mut budget)?,
        }],
        &mut budget,
    )?;
    let numerator = denominator.one(&mut budget)?;
    let element = engine.element(Value::Fraction(Fraction::new(
        &numerator,
        &denominator,
        &mut budget,
    )?));
    let stored = crate::solving::towers::materialize::value(&mut cas, &element, &mut budget)?;
    let construction = crate::solving::radicals::construction::Construction {
        header: cas.handle(cas.state.context(context.logical_id())?.header_id),
        context,
        region: region.clone(),
        steps: Vec::new(),
    };
    let images = crate::solving::solve_search::coefficient_images(
        &mut cas,
        &construction,
        &domain,
        &mut budget,
    )?;
    let map = construction.mapping(&mut cas, &domain, &images, &mut budget)?;
    Ok((cas, region, base, stored, map))
}

/// A canonical scalar map proves a power denominator on p≠0 and refuses it on p=0.
/// ´test:crate:blind-mapped-parameter-denominator´
#[test]
fn blind_mapped_parameter_denominator() -> TestResult {
    use crate::solving::{
        SolveDraft,
        coefficients::{arithmetic::Engine, forms::Variable},
    };
    for nonzero in [true, false] {
        let (cas, region, base, stored, map) = mapped_denominator(nonzero)?;
        let SolveDraft::CoefficientMap(mapping) =
            crate::solving::graph::object(&cas.state, map.logical_id())?
        else {
            return Err("parameter map".into());
        };
        for (image, name) in mapping.images.iter().zip(["q", "p"]) {
            let crate::model::Operator::Free(key) =
                &cas.state.expression(image.logical_id())?.operator
            else {
                return Err("free parameter image".into());
            };
            assert_eq!(key, name);
        }
        assert_eq!(mapping.images.len(), 2);
        let parameter = cas.state.expression(mapping.images[1].logical_id())?;
        let header = cas.state.header(parameter.header)?;
        let slot = u32::try_from(
            header
                .environment
                .declarations
                .iter()
                .position(|declaration| declaration.key == "p")
                .ok_or("p declaration")?,
        )?;
        let evaluation = crate::solving::elementary::evaluate::Evaluation {
            cas: &cas,
            engine: Engine {
                state: &cas.state,
                domain: &base,
            },
            region: &region,
            premises: &[],
            free: &[],
            bound: &[],
        };
        let result = evaluation.mapped(&stored, mapping, &mut Budget::new(Limits::blind_cubic())?);
        assert_eq!(result.is_ok(), nonzero);
        if let Ok(value) = result {
            assert_eq!(
                value
                    .denominator
                    .terms
                    .iter()
                    .map(|term| term.powers.as_slice())
                    .collect::<Vec<_>>(),
                vec![[(Variable::Slot(slot), 2)].as_slice()]
            );
        }
        if nonzero {
            mapped_provenance(&evaluation, &stored, mapping, &map)?;
        }
    }
    Ok(())
}

/// An unused parameter's explicit map provenance survives its zero exponent.
fn mapped_provenance(
    evaluation: &crate::solving::elementary::evaluate::Evaluation<'_>,
    stored: &crate::Handle,
    mapping: &crate::solving::CoefficientMapData<crate::Handle>,
    map: &crate::Handle,
) -> TestResult {
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let mut replacement =
        crate::solving::elementary::value::Value::integer(2, &evaluation.engine, &mut budget)?;
    replacement.maps.push(map.logical_id());
    let free = [("q", replacement)];
    let substituted = crate::solving::elementary::evaluate::Evaluation {
        free: &free,
        ..evaluation.unbound()
    };
    let mapped = substituted.mapped(stored, mapping, &mut budget)?;
    assert_eq!(mapped.maps, [map.logical_id()]);
    Ok(())
}

/// Canonical fractions retain their exact parameter denominator in the identical algebra after its regional proof.
/// ´test:crate:blind-mapped-parameter-field-denominator´
#[test]
fn blind_mapped_parameter_field_denominator() -> TestResult {
    use crate::solving::{
        SolveDraft,
        coefficients::{arithmetic::Engine, model::Value},
    };
    for nonzero in [true, false] {
        let (cas, region, _, stored, map) = mapped_denominator(nonzero)?;
        let SolveDraft::CoefficientMap(mapping) =
            crate::solving::graph::object(&cas.state, map.logical_id())?
        else {
            return Err("parameter map".into());
        };
        let evaluation = crate::solving::elementary::evaluate::Evaluation {
            cas: &cas,
            engine: Engine {
                state: &cas.state,
                domain: &mapping.source,
            },
            region: &region,
            premises: &[],
            free: &[],
            bound: &[],
        };
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let result = evaluation.mapped(&stored, mapping, &mut budget);
        assert_eq!(result.is_ok(), nonzero);
        if let Ok(value) = result {
            assert_eq!(value.numerator.terms.len(), 1);
            assert_eq!(
                value.numerator.terms[0].powers,
                [] as [(crate::solving::coefficients::forms::Variable, u32); 0]
            );
            let Value::Fraction(coefficient) = &value.numerator.terms[0].coefficient.value else {
                return Err("retained fraction".into());
            };
            assert_eq!(coefficient.numerator.terms.len(), 1);
            assert_eq!(coefficient.numerator.terms[0].powers, [0, 0]);
            assert_eq!(coefficient.denominator.terms.len(), 1);
            assert_eq!(coefficient.denominator.terms[0].powers, [0, 2]);
            let unit = crate::solving::elementary::value::Value::integer(
                1,
                &evaluation.engine,
                &mut budget,
            )?;
            assert_eq!(value.denominator.terms.len(), 1);
            assert_eq!(
                value.denominator.terms[0].powers,
                [] as [(crate::solving::coefficients::forms::Variable, u32); 0]
            );
            assert!(evaluation.engine.formal_equal(
                &value.denominator.terms[0].coefficient,
                &unit.numerator.terms[0].coefficient,
                &mut budget
            )?);
            assert_eq!(value.maps, [] as [crate::graph::LogicalId; 0]);
            mapped_provenance(&evaluation, &stored, mapping, &map)?;
        }
    }
    Ok(())
}

/// A zero numerator is usable only on its own zero cell with its actual denominator independently nonzero.
/// ´test:crate:blind-parameter-fraction-cell-zero´
#[test]
fn blind_parameter_fraction_cell_zero() -> TestResult {
    use crate::solving::coefficients::{
        arithmetic::Engine,
        fraction::Fraction,
        model::Value,
        polynomial::{Polynomial, Term},
        scalar::Scalar,
    };
    for (relation_zero, parameter_nonzero, expected) in [
        (true, true, true),
        (true, false, false),
        (false, true, false),
    ] {
        let (cas, domain, _, region) = consequence_scope(relation_zero, parameter_nonzero)?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let engine = Engine {
            state: &cas.state,
            domain: &domain,
        };
        let (ground, width, _) = engine.fraction_shape()?;
        assert_eq!(width, 2);
        let numerator = Polynomial::new(
            width,
            ground,
            vec![
                Term {
                    powers: vec![2, 0],
                    coefficient: Scalar::constant(ground, true, &mut budget)?,
                },
                Term {
                    powers: vec![0, 3],
                    coefficient: Scalar::constant(ground, true, &mut budget)?,
                },
            ],
            &mut budget,
        )?;
        let denominator = Polynomial::new(
            width,
            ground,
            vec![Term {
                powers: vec![0, 2],
                coefficient: Scalar::constant(ground, true, &mut budget)?,
            }],
            &mut budget,
        )?;
        let coefficient = engine.element(Value::Fraction(Fraction::new(
            &numerator,
            &denominator,
            &mut budget,
        )?));
        let evaluation = crate::solving::elementary::evaluate::Evaluation {
            cas: &cas,
            engine,
            region: &region,
            premises: &[],
            free: &[],
            bound: &[],
        };
        assert_eq!(
            evaluation.parameter_zero(&coefficient, &mut budget)?,
            expected
        );
    }
    Ok(())
}

/// Refused factor comparisons are skipped only when a different exact entry supplies the match.
/// ´test:crate:blind-symbolic-inclusion-candidates´
#[test]
fn blind_symbolic_inclusion_candidates() -> TestResult {
    crate::testing::solve_blind::inclusion_candidates()
}
