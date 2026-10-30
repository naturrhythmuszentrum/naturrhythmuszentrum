// SPDX-License-Identifier: AGPL-3.0-only
//! Independent integer identities, route corruption and transactional resource controls.

use super::*;
use std::cmp::Ordering;

fn power_policy() -> RefinementPolicy {
    RefinementPolicy {
        checking_work: 0,
        reduction_ceiling: 1,
        ..policy()
    }
}

fn power_case(n: i64, d: u64, p: i64, q: u64) -> TestResult<(Fixture, EnclosureRequest)> {
    let mut fixture = Fixture::new()?;
    let base = fixture.rational(n, d)?;
    let exponent = fixture.rational(p, q)?;
    let target = fixture.term(Operator::PositivePower, &[base, exponent])?;
    let request = fixture.request(target);
    Ok((fixture, request))
}

/// Exact integer identities and independently cubed endpoints check all three routes.
/// ´test:unit:enclosure-power-routes´
/// ´claim:cas:enclosure-power-routes´
#[test]
fn enclosure_power_routes() -> TestResult {
    for (n, d, p, q, a, b) in [
        (2, 1, 3, 1, 8, 1),
        (2, 1, -3, 1, 1, 8),
        (2, 1, 0, 1, 1, 1),
        (16, 1, 3, 4, 8, 1),
        (81, 1, 1, 4, 3, 1),
        (4, 1, -1, 2, 1, 2),
        (1, 16, 1, 2, 1, 4),
        (1, 16, -1, 2, 4, 1),
        (1, 1, 1, 3, 1, 1),
    ] {
        let (mut f, request) = power_case(n, d, p, q)?;
        let checked = f
            .cas
            .enclose(
                &request,
                RefinementPolicy {
                    precision: 0,
                    reduction_ceiling: 0,
                    ..power_policy()
                },
                &mut ledger()?,
                &mut |_| false,
            )?
            .checked
            .ok_or("power bound")?;
        let expected = Interval::singleton(&number(a, b)?, &mut ledger()?)?;
        assert!(checked.interval.contains(&expected, &mut ledger()?)?);
        assert_eq!(checked.formal_schema().name, "enclosure.positive-power");
        let bytes = checked.certificate.to_bytes(&mut ledger()?)?;
        let decoded = EnclosureCertificate::from_bytes(&bytes, &mut ledger()?)?;
        let (mut receiver, expected) = power_case(n, d, p, q)?;
        let replay = receiver.cas.check_enclosure(
            &expected,
            RefinementPolicy {
                precision: 0,
                reduction_ceiling: 0,
                ..power_policy()
            },
            &decoded,
            &mut ledger()?,
            &mut |_| false,
        )?;
        assert_eq!(replay.checked.ok_or("replay")?.interval, checked.interval);
    }
    analytic_routes()?;
    Ok(())
}

/// Independent cubing checks both signs of the analytic route, including fresh replay.
fn analytic_routes() -> TestResult {
    for (n, d, exponent) in [(3, 2, 1), (5, 4, 1), (3, 2, -1), (5, 4, -1)] {
        let (mut f, request) = power_case(n, d, exponent, 3)?;
        let report = f.cas.enclose(
            &request,
            RefinementPolicy {
                precision: 0,
                reduction_ceiling: 0,
                ..power_policy()
            },
            &mut ledger()?,
            &mut |_| false,
        )?;
        let checked = report.checked.ok_or("cube-root bound")?;
        let bound = checked.certificate.records.last().ok_or("record")?;
        let expected_cube = if exponent > 0 {
            number(n, d)?
        } else {
            number(i64::try_from(d)?, u64::try_from(n)?)?
        };
        assert_ne!(
            bound
                .lower()
                .pow(3, &mut ledger()?)?
                .compare(&expected_cube, &mut ledger()?)?,
            Ordering::Greater
        );
        assert_ne!(
            bound
                .upper()
                .pow(3, &mut ledger()?)?
                .compare(&expected_cube, &mut ledger()?)?,
            Ordering::Less
        );
        let (mut receiver, request) = power_case(n, d, exponent, 3)?;
        let decoded = EnclosureCertificate::from_bytes(
            &checked.certificate.to_bytes(&mut ledger()?)?,
            &mut ledger()?,
        )?;
        let replay = receiver.cas.check_enclosure(
            &request,
            RefinementPolicy {
                precision: 0,
                reduction_ceiling: 0,
                ..power_policy()
            },
            &decoded,
            &mut ledger()?,
            &mut |_| false,
        )?;
        assert_eq!(
            replay.checked.ok_or("analytic replay")?.interval,
            checked.interval
        );
    }
    Ok(())
}

/// A finite analytic budget stops without publishing an unchecked partial enclosure.
/// ´test:unit:enclosure-power-exhaustion´
/// ´claim:cas:enclosure-power-exhaustion´
#[test]
fn enclosure_power_exhaustion() -> TestResult {
    let (mut f, request) = power_case(2, 1, 1, 3)?;
    let before = f.cas.canonical_snapshot(&mut ledger()?)?;
    let report = f
        .cas
        .enclose(&request, power_policy(), &mut ledger()?, &mut |_| false)?;
    assert_eq!(report.outcome, EnclosureOutcome::Exhausted);
    assert!(report.checked.is_none());
    assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    Ok(())
}

/// An unfixed exponent has no power law in this profile and spends no evaluation Work.
/// ´test:unit:enclosure-power-preflight´
/// ´claim:cas:enclosure-power-preflight´
#[test]
fn enclosure_power_preflight() -> TestResult {
    let mut f = Fixture::new()?;
    let one = f.rational(1, 1)?;
    let exponent = f.term(Operator::Free("x".into()), &[])?;
    let target = f.term(Operator::PositivePower, &[one, exponent])?;
    let request = f.request(target);
    let before = f.cas.canonical_snapshot(&mut ledger()?)?;
    let report = f
        .cas
        .enclose(&request, power_policy(), &mut ledger()?, &mut |_| false)?;
    assert_eq!(report.outcome, EnclosureOutcome::Unsupported);
    assert_eq!(report.work, 0);
    assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    Ok(())
}

/// Original positive-base domains survive exponent zero, negative powers and hidden operands.
/// ´test:unit:enclosure-power-domains´
/// ´claim:cas:enclosure-power-domains´
#[test]
fn enclosure_power_domains() -> TestResult {
    for base in [-1, 0] {
        for exponent in [-1, 0, 1] {
            let (mut f, request) = power_case(base, 1, exponent, 1)?;
            let before = f.cas.canonical_snapshot(&mut ledger()?)?;
            assert!(
                f.cas
                    .enclose(&request, power_policy(), &mut ledger()?, &mut |_| false)
                    .is_err()
            );
            assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
        }
    }
    let mut f = Fixture::new()?;
    let one = f.rational(1, 1)?;
    let zero = f.rational(0, 1)?;
    let bad = f.term(Operator::Div, &[one, zero.clone()])?;
    let target = f.term(Operator::PositivePower, &[bad, zero])?;
    let request = f.request(target);
    assert!(
        f.cas
            .enclose(&request, power_policy(), &mut ledger()?, &mut |_| false)
            .is_err()
    );
    Ok(())
}

/// Route, original argument, component bounds and missing premises refuse independently.
/// ´test:unit:enclosure-power-corruptions´
/// ´claim:cas:enclosure-power-corruptions´
#[test]
fn enclosure_power_corruptions() -> TestResult {
    for denominator in [2, 3] {
        let (mut f, request) = power_case(3, 2, 1, denominator)?;
        let checked = f
            .cas
            .enclose(
                &request,
                RefinementPolicy {
                    precision: 0,
                    reduction_ceiling: 0,
                    ..power_policy()
                },
                &mut ledger()?,
                &mut |_| false,
            )?
            .checked
            .ok_or("bound")?;
        for mutation in 0..8 {
            let mut certificate = checked.certificate.clone();
            match mutation {
                0 => certificate.powers[0].route = PowerRoute::Integer,
                1 => certificate.powers[0].record = 0,
                2 => certificate.powers.clear(),
                3 => certificate.records[0] = interval(3, 3, 1)?,
                4 => certificate.records[2] = interval(0, 4, 1)?,
                5 => certificate.scope[0] ^= 1,
                6 => {
                    if denominator == 2 {
                        certificate.powers[0].roots[0] = interval(1, 1, 1)?;
                    } else {
                        certificate.powers[0].logarithm = Some(interval(0, 0, 1)?);
                    }
                }
                _ => {
                    if denominator == 2 {
                        certificate.powers[0].roots.clear();
                    } else {
                        certificate.exponentials.clear();
                    }
                }
            }
            let (mut receiver, expected) = power_case(3, 2, 1, denominator)?;
            let before = receiver.cas.canonical_snapshot(&mut ledger()?)?;
            assert!(
                receiver
                    .cas
                    .check_enclosure(
                        &expected,
                        RefinementPolicy {
                            precision: 0,
                            reduction_ceiling: 0,
                            ..power_policy()
                        },
                        &certificate,
                        &mut ledger()?,
                        &mut |_| false
                    )
                    .is_err(),
                "mutation {mutation}"
            );
            assert_eq!(receiver.cas.canonical_snapshot(&mut ledger()?)?, before);
        }
    }
    Ok(())
}

/// Negative powers reverse endpoint order and wide boxes keep an honest width outcome.
/// ´test:unit:enclosure-power-boxes´
/// ´claim:cas:enclosure-power-boxes´
#[test]
fn enclosure_power_boxes() -> TestResult {
    let mut f = Fixture::new()?;
    let x = f.term(Operator::Free("x".into()), &[])?;
    let exponent = f.rational(-2, 1)?;
    let target = f.term(Operator::PositivePower, &[x.clone(), exponent])?;
    let mut request = f.request(target);
    request.coordinates.push(CoordinateBound {
        coordinate: x,
        interval: interval(1, 4, 1)?,
    });
    request.goal = EnclosureGoal::AtMost(number(1, 2)?);
    let report = f.cas.enclose(
        &request,
        RefinementPolicy {
            rounds: 1,
            ..power_policy()
        },
        &mut ledger()?,
        &mut |_| false,
    )?;
    assert_eq!(
        report.outcome,
        EnclosureOutcome::Undecided(UndecidedReason::Straddle)
    );
    assert_eq!(
        report.checked.ok_or("bound")?.interval,
        interval(1, 16, 16)?
    );
    Ok(())
}

/// Every selected interruption boundary leaves the earlier store unchanged.
/// ´test:unit:enclosure-power-interruption´
/// ´claim:cas:enclosure-power-interruption´
#[test]
fn enclosure_power_interruption() -> TestResult {
    for stop in 1..=12 {
        let (mut f, request) = power_case(16, 1, 1, 4)?;
        let before = f.cas.canonical_snapshot(&mut ledger()?)?;
        let mut calls = 0;
        let report = f
            .cas
            .enclose(&request, power_policy(), &mut ledger()?, &mut |_| {
                calls += 1;
                calls == stop
            })?;
        assert_eq!(report.outcome, EnclosureOutcome::Interrupted);
        assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// Mixed original families retain exactly the schemas independently encountered by replay.
/// ´test:unit:enclosure-power-composition-inventory´
/// ´claim:cas:enclosure-power-composition-inventory´
#[test]
fn enclosure_power_composition_inventory() -> TestResult {
    let mut f = Fixture::new()?;
    let zero = f.rational(0, 1)?;
    let half = f.term(Operator::Sigmoid, &[zero])?;
    let two = f.rational(2, 1)?;
    let target = f.term(Operator::PositivePower, &[half, two])?;
    let request = f.request(target);
    let checked = f
        .cas
        .enclose(&request, power_policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("mixed bound")?;
    assert_eq!(checked.interval, interval(1, 1, 4)?);
    assert_eq!(
        checked
            .formal_schemas()
            .iter()
            .map(|s| s.name)
            .collect::<Vec<_>>(),
        [
            "enclosure.rational-inclusion",
            "enclosure.exp-taylor",
            "enclosure.sigmoid-compose",
            "enclosure.positive-power"
        ]
    );
    let decoded = EnclosureCertificate::from_bytes(
        &checked.certificate.to_bytes(&mut ledger()?)?,
        &mut ledger()?,
    )?;
    assert_eq!(decoded, checked.certificate);
    Ok(())
}

/// Existing integer-exponent ceilings bound both magnitude and root repetition.
/// ´test:unit:enclosure-power-exponent-pins´
/// ´claim:cas:enclosure-power-exponent-pins´
#[test]
fn enclosure_power_exponent_pins() -> TestResult {
    for (p, d, pin) in [(256, 1, 256), (1, 1_u64 << 63, 63)] {
        for (available, passes) in [(pin, true), (pin - 1, false)] {
            let (mut f, request) = power_case(1, 1, p, d)?;
            let mut budget = Budget::new(Limits {
                integer_exponent: available,
                ..Limits::default()
            })?;
            let before = f.cas.canonical_snapshot(&mut ledger()?)?;
            let report = f
                .cas
                .enclose(&request, power_policy(), &mut budget, &mut |_| false)?;
            assert_eq!(
                report.outcome,
                if passes {
                    EnclosureOutcome::Bound
                } else {
                    EnclosureOutcome::Exhausted
                }
            );
            if !passes {
                assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
            }
        }
    }
    Ok(())
}

/// Generation, replay and retained lookup report distinct measured Work.
/// ´test:unit:enclosure-power-work-pins´
/// ´claim:cas:enclosure-power-work-pins´
#[test]
fn enclosure_power_work_pins() -> TestResult {
    let (mut f, request) = power_case(4, 1, 1, 2)?;
    let cold = f
        .cas
        .enclose(&request, power_policy(), &mut ledger()?, &mut |_| false)?;
    let warm = f
        .cas
        .enclose(&request, power_policy(), &mut ledger()?, &mut |_| false)?;
    let checked = cold.checked.ok_or("bound")?;
    let (mut receiver, expected) = power_case(4, 1, 1, 2)?;
    let replay = receiver.cas.check_enclosure(
        &expected,
        power_policy(),
        &checked.certificate,
        &mut ledger()?,
        &mut |_| false,
    )?;
    assert_eq!(
        (cold.work, warm.work, replay.work),
        (70_169, 20_323, 42_985)
    );
    for (is_warm, pin) in [(false, 70_169), (true, 20_323)] {
        for (available, passes) in [(pin, true), (pin - 1, false)] {
            let (mut store, request) = power_case(4, 1, 1, 2)?;
            if is_warm {
                store
                    .cas
                    .enclose(&request, power_policy(), &mut ledger()?, &mut |_| false)?;
            }
            let before = store.cas.canonical_snapshot(&mut ledger()?)?;
            let report = store.cas.enclose(
                &request,
                power_policy(),
                &mut precharged(available)?,
                &mut |_| false,
            )?;
            assert_eq!(
                report.outcome,
                if passes {
                    EnclosureOutcome::Bound
                } else {
                    EnclosureOutcome::Exhausted
                }
            );
            if !passes {
                assert_eq!(store.cas.canonical_snapshot(&mut ledger()?)?, before);
            }
        }
    }
    for (available, passes) in [(42_985, true), (42_984, false)] {
        let (mut store, request) = power_case(4, 1, 1, 2)?;
        let before = store.cas.canonical_snapshot(&mut ledger()?)?;
        let report = store.cas.check_enclosure(
            &request,
            power_policy(),
            &checked.certificate,
            &mut precharged(available)?,
            &mut |_| false,
        )?;
        assert_eq!(
            report.outcome,
            if passes {
                EnclosureOutcome::Bound
            } else {
                EnclosureOutcome::Exhausted
            }
        );
        if !passes {
            assert_eq!(store.cas.canonical_snapshot(&mut ledger()?)?, before);
        }
    }
    Ok(())
}

/// Isolated production and checking retain their own literal resource boundaries.
/// ´test:unit:enclosure-power-phase-pins´
/// ´claim:cas:enclosure-power-phase-pins´
#[test]
fn enclosure_power_phase_pins() -> TestResult {
    let (f, request) = power_case(4, 1, 1, 2)?;
    let target = f.cas.state.expression(f.cas.checked(&request.target)?)?;
    let plans = planning::Plans::new(target, power_policy()).ok_or("plan")?;
    let grid = plans.grid(12);
    let scope = scope::key(&f.cas, &request, &mut ledger()?)?;
    let mut production = ledger()?;
    let certificate = producer::generate_planned(
        &f.cas,
        &request,
        scope,
        (grid, 6),
        plans.at_grid(grid),
        &mut production,
        &mut |_| false,
    )?;
    let mut replay = ledger()?;
    let checked = checker::check(&f.cas, &request, &certificate, &mut replay, &mut |_| false)?;
    let bytes = certificate.to_bytes(&mut ledger()?)?;
    let prefix = service::prefix(&certificate.scope, grid, 6, &mut ledger()?)?;
    let key = service::retention_key(&prefix, &certificate, &mut ledger()?)?;
    assert_eq!(
        (
            production.usage().work,
            replay.usage().work,
            bytes.len(),
            footprint(&checked) + u64::try_from(key.len())?,
            grid
        ),
        (12_587, 32_398, 688, 3224, 12)
    );
    for (pin, generation) in [(12_587, true), (32_398, false)] {
        for (available, passes) in [(pin, true), (pin - 1, false)] {
            let mut budget = precharged(available)?;
            let result = if generation {
                producer::generate_planned(
                    &f.cas,
                    &request,
                    certificate.scope.clone(),
                    (grid, 6),
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
        }
    }

    Ok(())
}

/// Wire, retention, degree and grid boundaries refuse at exactly one less.
/// ´test:unit:enclosure-power-size-pins´
/// ´claim:cas:enclosure-power-size-pins´
#[test]
fn enclosure_power_size_pins() -> TestResult {
    let (mut f, request) = power_case(4, 1, 1, 2)?;
    let checked = f
        .cas
        .enclose(&request, power_policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("bound")?;
    let certificate = &checked.certificate;
    let bytes = certificate.to_bytes(&mut ledger()?)?;
    for (available, passes) in [(688, true), (687, false)] {
        let limits = Limits {
            witness_bytes: available,
            ..Limits::default()
        };
        assert_eq!(
            certificate
                .to_bytes(&mut Budget::new(limits.clone())?)
                .is_ok(),
            passes
        );
        assert_eq!(
            EnclosureCertificate::from_bytes(&bytes, &mut Budget::new(limits)?).is_ok(),
            passes
        );
    }
    for (available, passes) in [(3224, true), (3223, false)] {
        for resource in 0..3 {
            let (mut f, expected) = power_case(4, 1, 1, 2)?;
            let checked =
                checker::check(&f.cas, &expected, certificate, &mut ledger()?, &mut |_| {
                    false
                })?;
            let prefix = service::prefix(&certificate.scope, 12, 6, &mut ledger()?)?;
            let key = service::retention_key(&prefix, certificate, &mut ledger()?)?;
            let limits = match resource {
                0 => Limits {
                    output_bytes: available,
                    ..Limits::default()
                },
                1 => Limits {
                    staged_payload_bytes: available,
                    ..Limits::default()
                },
                _ => Limits {
                    work: available + 7,
                    ..Limits::default()
                },
            };
            let mut budget = Budget::new(limits)?;
            budget.work(7, AT)?;
            let before = f.cas.canonical_snapshot(&mut ledger()?)?;
            assert_eq!(
                f.cas.retain_enclosure(key, checked, &mut budget).is_ok(),
                passes
            );
            if !passes {
                assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
            }
        }
    }
    for (grid, passes) in [(12, true), (11, false)] {
        let (mut f, request) = power_case(4, 1, 1, 2)?;
        let report = f.cas.enclose(
            &request,
            RefinementPolicy {
                grid_ceiling: grid,
                ..power_policy()
            },
            &mut ledger()?,
            &mut |_| false,
        )?;
        assert_eq!(
            report.outcome,
            if passes {
                EnclosureOutcome::Bound
            } else {
                EnclosureOutcome::Unsupported
            }
        );
        if !passes {
            assert_eq!(report.work, 0);
        }
    }
    Ok(())
}

/// Seven earlier units remain charged while testing the new operation's allowance.
fn precharged(available: u64) -> Result<Budget, Fault> {
    let mut budget = Budget::new(Limits {
        work: available + 7,
        ..Limits::default()
    })?;
    budget.work(7, AT)?;
    Ok(budget)
}
