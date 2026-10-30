// SPDX-License-Identifier: AGPL-3.0-only
//! Caller-only Work measurements preserve all other resource ceilings.

use super::*;

/// Explicit caller profile; defaults and permanent stores are not modified after admission.
fn limits() -> Limits {
    Limits {
        work: 10_000_000,
        ..Limits::default()
    }
}
fn large() -> Result<Budget, Fault> {
    Budget::new(limits())
}
fn configured_case(n: i64, d: u64, p: i64) -> TestResult<(Fixture, EnclosureRequest)> {
    let mut f = Fixture::with_names_and_limits(&["x"], limits())?;
    let base = f.rational(n, d)?;
    let exponent = f.rational(p, 3)?;
    let target = f.term(Operator::PositivePower, &[base, exponent])?;
    let request = f.request(target);
    Ok((f, request))
}

/// Base, exponent numerator, precision, and cold/generator/checker/fresh-replay Work.
const CASES: [(i64, u64, i64, u32, [u64; 4]); 12] = [
    (1, 2, 1, 4, [2_242_506, 1_067_071, 1_138_358, 1_160_840]),
    (1, 2, 1, 8, [2_989_208, 1_434_223, 1_517_362, 1_540_390]),
    (1, 2, 1, 16, [4_596_440, 2_234_765, 2_322_924, 2_347_080]),
    (2, 1, 1, 4, [2_146_333, 1_041_219, 1_068_026, 1_090_519]),
    (2, 1, 1, 8, [2_874_660, 1_402_415, 1_434_614, 1_457_650]),
    (2, 1, 1, 16, [4_467_864, 2_200_941, 2_228_204, 2_252_328]),
    (1, 2, -2, 4, [2_429_362, 1_160_211, 1_231_910, 1_254_556]),
    (1, 2, -2, 8, [3_210_096, 1_545_591, 1_626_710, 1_649_910]),
    (1, 2, -2, 16, [4_999_232, 2_441_037, 2_519_268, 2_543_600]),
    (2, 1, -2, 4, [2_327_756, 1_134_763, 1_155_806, 1_178_398]),
    (2, 1, -2, 8, [3_095_056, 1_513_943, 1_543_318, 1_566_518]),
    (2, 1, -2, 16, [4_863_500, 2_405_549, 2_419_048, 2_443_356]),
];

/// All twelve cases attain their width in one round and pass independent exact cubing and fresh replay.
/// ´test:unit:enclosure-power-useful-precision´
/// ´claim:cas:enclosure-power-useful-precision´
#[test]
fn enclosure_power_useful_precision() -> TestResult {
    for (n, d, p, precision, pins) in CASES {
        let (mut f, request) = configured_case(n, d, p)?;
        let policy = RefinementPolicy {
            precision,
            ..power_policy()
        };
        let report = f
            .cas
            .enclose(&request, policy, &mut large()?, &mut |_| false)?;
        assert_eq!(report.outcome, EnclosureOutcome::Bound);
        assert!(report.precision_met);
        assert_eq!(report.completed_rounds, 1);
        assert_eq!(report.work, pins[0]);
        let checked = report.checked.ok_or("bound")?;
        cube_oracle(&checked.interval, n, d, p)?;
        let decoded = EnclosureCertificate::from_bytes(
            &checked.certificate.to_bytes(&mut large()?)?,
            &mut large()?,
        )?;
        let (mut receiver, expected) = configured_case(n, d, p)?;
        let replay = receiver.cas.check_enclosure(
            &expected,
            policy,
            &decoded,
            &mut large()?,
            &mut |_| false,
        )?;
        assert_eq!(replay.work, pins[3]);
        assert_eq!(replay.checked.ok_or("replay")?.interval, checked.interval);
    }
    Ok(())
}

/// Cubing is monotone; use scalar products only, never the power, log, exp or interval algorithms.
fn cube_oracle(bound: &Interval, n: i64, d: u64, p: i64) -> TestResult {
    let mut budget = large()?;
    let base = number(n, d)?;
    let expected = if p == 1 {
        base
    } else {
        Rational::one().div(&base.mul(&base, &mut budget)?, &mut budget)?
    };
    for (endpoint, forbidden) in [
        (bound.lower(), Ordering::Greater),
        (bound.upper(), Ordering::Less),
    ] {
        let cube = endpoint
            .mul(endpoint, &mut budget)?
            .mul(endpoint, &mut budget)?;
        assert_ne!(cube.compare(&expected, &mut budget)?, forbidden);
    }
    Ok(())
}

/// Work alone stops the old request; the enlarged store/caller Work profile completes with unchanged policy.
/// ´test:unit:enclosure-power-limiting-resource´
/// ´claim:cas:enclosure-power-limiting-resource´
#[test]
fn enclosure_power_limiting_resource() -> TestResult {
    let (mut f, request) = power_case(2, 1, 1, 3)?;
    let report = f
        .cas
        .enclose(&request, power_policy(), &mut large()?, &mut |_| false)?;
    assert_eq!(report.outcome, EnclosureOutcome::Exhausted);
    assert!(matches!(
        report.resource_fault.ok_or("fault")?.kind,
        crate::FaultKind::Exhausted {
            budget: crate::budget::BudgetKind::Work,
            limit: 2_000_000,
            ..
        }
    ));
    let (mut f, request) = configured_case(2, 1, 1)?;
    let report = f
        .cas
        .enclose(&request, power_policy(), &mut large()?, &mut |_| false)?;
    assert_eq!(report.outcome, EnclosureOutcome::Bound);
    assert_eq!(report.work, 2_146_333);
    assert_eq!(report.completed_rounds, 1);
    assert!(report.precision_met);
    Ok(())
}

/// Cold and fresh replay pass at N and refuse at N minus one without publication, retaining prior Work.
/// ´test:unit:enclosure-power-analytic-work-pins´
/// ´claim:cas:enclosure-power-analytic-work-pins´
#[test]
fn enclosure_power_analytic_work_pins() -> TestResult {
    for (n, d, p, precision, pins) in CASES {
        let policy = RefinementPolicy {
            precision,
            ..power_policy()
        };
        let (mut origin, request) = configured_case(n, d, p)?;
        let certificate = origin
            .cas
            .enclose(&request, policy, &mut large()?, &mut |_| false)?
            .checked
            .ok_or("bound")?
            .certificate;
        for replay in [false, true] {
            let pin = pins[if replay { 3 } else { 0 }];
            for (available, passes) in [(pin, true), (pin - 1, false)] {
                let (mut f, request) = configured_case(n, d, p)?;
                let before = f.cas.canonical_snapshot(&mut large()?)?;
                let mut budget = precharged(available)?;
                let report = if replay {
                    f.cas.check_enclosure(
                        &request,
                        policy,
                        &certificate,
                        &mut budget,
                        &mut |_| false,
                    )?
                } else {
                    f.cas
                        .enclose(&request, policy, &mut budget, &mut |_| false)?
                };
                assert_eq!(
                    report.outcome,
                    if passes {
                        EnclosureOutcome::Bound
                    } else {
                        EnclosureOutcome::Exhausted
                    }
                );
                if passes {
                    assert_eq!(budget.usage().work, pin + 7);
                } else {
                    assert_eq!(f.cas.canonical_snapshot(&mut large()?)?, before);
                }
            }
        }
    }
    Ok(())
}

/// Isolated generation and independent checking retain separate literal boundaries for every requested case.
/// ´test:unit:enclosure-power-analytic-phase-pins´
/// ´claim:cas:enclosure-power-analytic-phase-pins´
#[test]
fn enclosure_power_analytic_phase_pins() -> TestResult {
    for (n, d, p, precision, pins) in CASES {
        let policy = RefinementPolicy {
            precision,
            ..power_policy()
        };
        let (f, request) = configured_case(n, d, p)?;
        let target = f.cas.state.expression(f.cas.checked(&request.target)?)?;
        let plans = planning::Plans::new(target, policy).ok_or("plan")?;
        let grid = plans.grid(precision + 8);
        let scope = scope::key(&f.cas, &request, &mut large()?)?;
        let certificate = producer::generate_planned(
            &f.cas,
            &request,
            scope,
            (grid, precision + 2),
            plans.at_grid(grid),
            &mut large()?,
            &mut |_| false,
        )?;
        for generation in [true, false] {
            let pin = pins[if generation { 1 } else { 2 }];
            for (available, passes) in [(pin, true), (pin - 1, false)] {
                let mut budget = precharged(available)?;
                let result = if generation {
                    producer::generate_planned(
                        &f.cas,
                        &request,
                        certificate.scope.clone(),
                        (grid, precision + 2),
                        plans.at_grid(grid),
                        &mut budget,
                        &mut |_| false,
                    )
                    .map(|_| ())
                } else {
                    checker::check(&f.cas, &request, &certificate, &mut budget, &mut |_| false)
                        .map(|_| ())
                };
                assert_eq!(result.is_ok(), passes);
                if passes {
                    assert_eq!(budget.usage().work, pin + 7);
                }
            }
        }
    }
    Ok(())
}
