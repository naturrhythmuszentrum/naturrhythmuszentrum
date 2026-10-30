// SPDX-License-Identifier: AGPL-3.0-only
//! Independent integral brackets and corrupt finite-law controls.

use super::*;
use std::cmp::Ordering;

fn log_case(n: i64, d: u64) -> TestResult<(Fixture, EnclosureRequest)> {
    let mut f = Fixture::new()?;
    let x = f.rational(n, d)?;
    let target = f.term(Operator::NaturalLog, &[x])?;
    let request = f.request(target);
    Ok((f, request))
}

fn log_policy() -> RefinementPolicy {
    RefinementPolicy {
        checking_work: 0,
        reduction_ceiling: 64,
        ..policy()
    }
}

/// Lower and upper rectangle sums for integral(1/x), using only integer divisions.
fn integral(n: i128, d: i128) -> (i128, i128) {
    let scale = 1_i128 << 30;
    let delta = n - d;
    let mut lower = 0;
    let mut upper = 0;
    for i in 0..32_i128 {
        let bottom = d * 32 + i * delta;
        upper += (delta * scale + bottom - 1) / bottom;
        lower += delta * scale / (bottom + delta);
    }
    (lower, upper)
}

/// The integral oracle uses repeated scaling, not the bit-length reduction or atanh series.
fn oracle(mut numerator: i128, mut denominator: i128) -> TestResult<Interval> {
    let mut exponent = 0_i128;
    while numerator < denominator {
        numerator *= 2;
        exponent -= 1;
    }
    while numerator >= 2 * denominator {
        denominator *= 2;
        exponent += 1;
    }
    let (lower, upper) = integral(numerator, denominator);
    let (two_lower, two_upper) = integral(2, 1);
    let (lo, hi) = if exponent < 0 {
        (lower + exponent * two_upper, upper + exponent * two_lower)
    } else {
        (lower + exponent * two_lower, upper + exponent * two_upper)
    };
    Ok(Interval::new(
        number(i64::try_from(lo)?, 1 << 30)?,
        number(i64::try_from(hi)?, 1 << 30)?,
        &mut ledger()?,
    )?)
}

/// Rational quadrature brackets contain the computed bounds across signed reductions.
/// ´test:unit:enclosure-log-integral-oracle´
/// ´claim:cas:enclosure-log-integral-oracle´
#[test]
fn enclosure_log_integral_oracle() -> TestResult {
    for (n, d) in [
        (1, 1),
        (2, 1),
        (3, 2),
        (7, 4),
        (8, 1),
        (1, 2),
        (3, 8),
        (1, 1 << 30),
    ] {
        let (mut f, request) = log_case(n, d)?;
        let report = f.cas.enclose(
            &request,
            RefinementPolicy {
                precision: 8,
                ..log_policy()
            },
            &mut ledger()?,
            &mut |_| false,
        )?;
        let checked = report
            .checked
            .ok_or_else(|| format!("no bound {n}/{d}: {:?}", report.outcome))?;
        let expected = oracle(i128::from(n), i128::from(d))?;
        assert!(
            checked
                .interval()
                .lower()
                .compare(expected.lower(), &mut ledger()?)?
                != Ordering::Less,
            "lower {n}/{d}"
        );
        assert!(
            checked
                .interval()
                .upper()
                .compare(expected.upper(), &mut ledger()?)?
                != Ordering::Greater,
            "upper {n}/{d}"
        );
        if n == 1 && d == 1 {
            assert_eq!(checked.interval(), &interval(0, 0, 1)?);
        }
    }
    Ok(())
}

/// Every finite payload mutation refuses before graph publication, including a changed N.
/// ´test:unit:enclosure-log-mutations´
/// ´claim:cas:enclosure-log-mutations´
#[test]
fn enclosure_log_mutations() -> TestResult {
    let (mut f, request) = log_case(3, 1)?;
    let report = f
        .cas
        .enclose(&request, log_policy(), &mut ledger()?, &mut |_| false)?;
    let cert = report
        .checked
        .ok_or_else(|| format!("bound {:?} {:?}", report.outcome, report.resource_fault))?
        .certificate()
        .clone();
    for mutation in 0..8 {
        let mut bad = cert.clone();
        let point = &mut bad.logarithms[0].lower;
        match mutation {
            0 => point.reduced.tail = interval(0, 0, 1)?,
            1 => point.reduced.terms += 1,
            2 => point.exponent += 1,
            3 => point.argument = number(1, 4)?,
            4 => point.mantissa = number(7, 4)?,
            5 => point.two.tail = interval(0, 0, 1)?,
            6 => point.reduced.power = interval(0, 0, 1)?,
            _ => bad.logarithms[0].record += 1,
        }
        let bytes = bad.to_bytes(&mut ledger()?)?;
        let decoded = EnclosureCertificate::from_bytes(&bytes, &mut ledger()?)?;
        let before = f.cas.canonical_snapshot(&mut ledger()?)?;
        assert!(
            f.cas
                .check_enclosure(
                    &request,
                    log_policy(),
                    &decoded,
                    &mut ledger()?,
                    &mut |_| false
                )
                .is_err(),
            "mutation {mutation}"
        );
        assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// Checked log-two and its exact factor-two composition retain ordinary anonymous judgments.
/// ´test:unit:enclosure-log-retention´
/// ´claim:cas:enclosure-log-retention´
#[test]
fn enclosure_log_retention() -> TestResult {
    let (mut f, request) = log_case(2, 1)?;
    let cold = f
        .cas
        .enclose(&request, log_policy(), &mut ledger()?, &mut |_| false)?;
    let snap = f.cas.canonical_snapshot(&mut ledger()?)?;
    let warm = f
        .cas
        .enclose(&request, log_policy(), &mut ledger()?, &mut |_| false)?;
    assert!(warm.work < cold.work);
    assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, snap);
    let two = f.rational(2, 1)?;
    let doubled = f.term(Operator::Mul, &[two, request.target.clone()])?;
    let twice_request = f.request(doubled);
    let twice = f
        .cas
        .enclose(&twice_request, log_policy(), &mut ledger()?, &mut |_| false)?;
    let checked = twice
        .checked
        .ok_or_else(|| format!("twice bound {:?} {:?}", twice.outcome, twice.resource_fault))?;
    let retained = f.cas.canonical_snapshot(&mut ledger()?)?;
    let reused = f
        .cas
        .enclose(&twice_request, log_policy(), &mut ledger()?, &mut |_| false)?;
    assert!(reused.work < twice.work);
    assert_eq!(
        reused.checked.ok_or("reused twice")?.interval(),
        checked.interval()
    );
    assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, retained);
    let bytes = checked.certificate().to_bytes(&mut ledger()?)?;
    let (mut receiving, original) = log_case(2, 1)?;
    let two = receiving.rational(2, 1)?;
    let target = receiving.term(Operator::Mul, &[two, original.target])?;
    let replay_request = receiving.request(target);
    let replay = receiving.cas.check_enclosure(
        &replay_request,
        log_policy(),
        &EnclosureCertificate::from_bytes(&bytes, &mut ledger()?)?,
        &mut ledger()?,
        &mut |_| false,
    )?;
    assert_eq!(
        replay.checked.ok_or("replay")?.interval(),
        checked.interval()
    );
    Ok(())
}

/// Impossible first schedules and oversized reduction bounds consume no evaluation Work.
/// ´test:unit:enclosure-log-sizing´
/// ´claim:cas:enclosure-log-sizing´
#[test]
fn enclosure_log_sizing() -> TestResult {
    for p in 0..100_u32 {
        let policy = RefinementPolicy {
            precision: p,
            grid_ceiling: 200,
            term_ceiling: 100,
            ..log_policy()
        };
        for k in 0..32_u32 {
            let plan = log::plan(policy, k).ok_or("plan")?;
            let mut ceil = 0;
            while (1_u64 << ceil) < u64::from(k) + 1 {
                ceil += 1;
            }
            let t = p + 4 + ceil;
            assert_eq!(plan.terms, t.div_ceil(3).max(1));
            assert!(3 * plan.terms >= t);
            assert!(plan.terms == 1 || 3 * (plan.terms - 1) < t);
            let mut guard = 0;
            while (1_u64 << guard) < u64::from(plan.terms) + 1 {
                guard += 1;
            }
            assert_eq!(plan.grid, t + 4 + guard);
        }
    }
    assert!(
        log::plan(
            RefinementPolicy {
                reduction_ceiling: u32::MAX,
                ..log_policy()
            },
            u32::MAX
        )
        .is_none()
    );
    let (mut f, request) = log_case(2, 1)?;
    for bad in [
        RefinementPolicy {
            term_ceiling: 0,
            ..log_policy()
        },
        RefinementPolicy {
            grid_ceiling: 1,
            ..log_policy()
        },
        RefinementPolicy {
            precision: u32::MAX,
            ..log_policy()
        },
        RefinementPolicy {
            reduction_ceiling: 0,
            ..log_policy()
        },
    ] {
        let report = f
            .cas
            .enclose(&request, bad, &mut ledger()?, &mut |_| false)?;
        assert_eq!(report.outcome, EnclosureOutcome::Unsupported);
        assert_eq!(report.work, 0);
    }
    Ok(())
}

/// Invalid logarithm domains refuse, while valid boxes preserve a straddling predicate.
/// ´test:unit:enclosure-log-domains´
/// ´claim:cas:enclosure-log-domains´
#[test]
fn enclosure_log_domains() -> TestResult {
    for n in [-1, 0] {
        let (mut f, request) = log_case(n, 1)?;
        let before = f.cas.canonical_snapshot(&mut ledger()?)?;
        assert!(
            f.cas
                .enclose(&request, log_policy(), &mut ledger()?, &mut |_| false)
                .is_err()
        );
        assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    let mut f = Fixture::new()?;
    let x = f.term(Operator::Free("x".into()), &[])?;
    let target = f.term(Operator::NaturalLog, std::slice::from_ref(&x))?;
    let mut request = f.request(target);
    request.coordinates.push(CoordinateBound {
        coordinate: x,
        interval: interval(1, 2, 1)?,
    });
    request.goal = EnclosureGoal::AtMost(number(1, 2)?);
    let report = f.cas.enclose(
        &request,
        RefinementPolicy {
            rounds: 1,
            ..log_policy()
        },
        &mut ledger()?,
        &mut |_| false,
    )?;
    assert_eq!(
        report.outcome,
        EnclosureOutcome::Undecided(UndecidedReason::Straddle)
    );
    assert!(report.checked.is_some());
    request.coordinates[0].interval = interval(0, 2, 1)?;
    assert!(
        f.cas
            .enclose(&request, log_policy(), &mut ledger()?, &mut |_| false)
            .is_err()
    );
    Ok(())
}

/// Interrupting either recurrence or replay leaves no partial anonymous judgment.
/// ´test:unit:enclosure-log-interruption´
/// ´claim:cas:enclosure-log-interruption´
#[test]
fn enclosure_log_interruption() -> TestResult {
    for stop in 1..=12 {
        let (mut f, request) = log_case(2, 1)?;
        let before = f.cas.canonical_snapshot(&mut ledger()?)?;
        let mut calls = 0;
        let report = f
            .cas
            .enclose(&request, log_policy(), &mut ledger()?, &mut |_| {
                calls += 1;
                calls == stop
            })?;
        assert_eq!(report.outcome, EnclosureOutcome::Interrupted, "stop {stop}");
        assert!(report.checked.is_none());
        assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// Measured logarithm generation and replay have separately visible resource costs.
/// ´test:unit:enclosure-log-work-pins´
/// ´claim:cas:enclosure-log-work-pins´
#[test]
fn enclosure_log_work_pins() -> TestResult {
    let (mut f, request) = log_case(2, 1)?;
    let cold = f
        .cas
        .enclose(&request, log_policy(), &mut ledger()?, &mut |_| false)?;
    let warm = f
        .cas
        .enclose(&request, log_policy(), &mut ledger()?, &mut |_| false)?;
    let checked = cold.checked.ok_or("cold")?;
    let certificate = checked.certificate();
    let bytes = certificate.to_bytes(&mut ledger()?)?;
    let (mut receiver, expected) = log_case(2, 1)?;
    let replay = receiver.cas.check_enclosure(
        &expected,
        log_policy(),
        certificate,
        &mut ledger()?,
        &mut |_| false,
    )?;
    let prefix = service::prefix(
        &certificate.scope,
        certificate.grid,
        certificate.presentation,
        &mut ledger()?,
    )?;
    let key = service::retention_key(&prefix, certificate, &mut ledger()?)?;
    assert_eq!(
        (cold.work, warm.work, replay.work),
        (630_588, 19_985, 330_435)
    );
    let (mut cold_store, cold_request) = log_case(2, 1)?;
    let short = cold_store.cas.enclose(
        &cold_request,
        log_policy(),
        &mut limited(19_985)?,
        &mut |_| false,
    )?;
    assert_eq!(short.outcome, EnclosureOutcome::Exhausted);
    assert_eq!(
        (
            bytes.len(),
            footprint(&checked) + u64::try_from(key.len())?,
            certificate.grid,
            certificate.logarithms[0].lower.two.terms
        ),
        (918, 5666, 17, 4)
    );
    for (warm, pin) in [(false, 630_588), (true, 19_985)] {
        for (available, passes) in [(pin, true), (pin - 1, false)] {
            let (mut f, request) = log_case(2, 1)?;
            if warm {
                f.cas
                    .enclose(&request, log_policy(), &mut ledger()?, &mut |_| false)?;
            }
            let before = f.cas.canonical_snapshot(&mut ledger()?)?;
            let report = f.cas.enclose(
                &request,
                log_policy(),
                &mut limited(available)?,
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
                assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
            }
        }
    }
    for (available, passes) in [(330_435, true), (330_434, false)] {
        let (mut f, expected) = log_case(2, 1)?;
        let before = f.cas.canonical_snapshot(&mut ledger()?)?;
        let report = f.cas.check_enclosure(
            &expected,
            log_policy(),
            certificate,
            &mut limited(available)?,
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
            assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
        }
    }
    Ok(())
}

fn limited(work: u64) -> Result<Budget, Fault> {
    Budget::new(Limits {
        work,
        ..Limits::default()
    })
}

/// Literal wire, retention and initial-schedule ceilings refuse at exactly one less.
/// ´test:unit:enclosure-log-size-pins´
/// ´claim:cas:enclosure-log-size-pins´
#[test]
fn enclosure_log_size_pins() -> TestResult {
    let (mut f, request) = log_case(2, 1)?;
    let checked = f
        .cas
        .enclose(&request, log_policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("bound")?;
    let certificate = checked.certificate();
    let bytes = certificate.to_bytes(&mut ledger()?)?;
    for (n, passes) in [(918, true), (917, false)] {
        let limits = Limits {
            witness_bytes: n,
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
    for (n, passes) in [(5666, true), (5665, false)] {
        for output in [true, false] {
            let (mut f, expected) = log_case(2, 1)?;
            let checked =
                checker::check(&f.cas, &expected, certificate, &mut ledger()?, &mut |_| {
                    false
                })?;
            let prefix = service::prefix(&certificate.scope, 17, 6, &mut ledger()?)?;
            let key = service::retention_key(&prefix, certificate, &mut ledger()?)?;
            let limits = if output {
                Limits {
                    output_bytes: n,
                    ..Limits::default()
                }
            } else {
                Limits {
                    staged_payload_bytes: n,
                    ..Limits::default()
                }
            };
            let before = f.cas.canonical_snapshot(&mut ledger()?)?;
            assert_eq!(
                f.cas
                    .retain_enclosure(key, checked, &mut Budget::new(limits)?)
                    .is_ok(),
                passes
            );
            if !passes {
                assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
            }
        }
    }
    for (terms, grid, passes) in [(4, 17, true), (3, 17, false), (4, 16, false)] {
        let (mut f, request) = log_case(2, 1)?;
        let policy = RefinementPolicy {
            term_ceiling: terms,
            grid_ceiling: grid,
            ..log_policy()
        };
        let report = f
            .cas
            .enclose(&request, policy, &mut ledger()?, &mut |_| false)?;
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

/// One extra term is checked as a different finite sum rather than accepted by its count.
/// ´test:unit:enclosure-log-term-count´
/// ´claim:cas:enclosure-log-term-count´
#[test]
fn enclosure_log_term_count() -> TestResult {
    let input = interval(3, 3, 2)?;
    let plan = log::Plan {
        grid: 25,
        ..log::plan(log_policy(), 1).ok_or("plan")?
    };
    let (a, first) = log::produce(&input, 0, plan, &mut ledger()?, &mut |_| false)?;
    let (b, second) = log::produce(
        &input,
        0,
        log::Plan {
            terms: plan.terms + 1,
            ..plan
        },
        &mut ledger()?,
        &mut |_| false,
    )?;
    log::check(&input, &first, &a, plan.grid, &mut ledger()?, &mut |_| {
        false
    })?;
    log::check(&input, &second, &b, plan.grid, &mut ledger()?, &mut |_| {
        false
    })?;
    assert_ne!(first.lower.reduced.power, second.lower.reduced.power);
    let mut forged = first;
    forged.lower.reduced.terms += 1;
    assert!(
        log::check(&input, &forged, &a, plan.grid, &mut ledger()?, &mut |_| {
            false
        })
        .is_err()
    );
    Ok(())
}

/// A rounded computed zero boundary requests refinement instead of a domain verdict.
/// ´test:unit:enclosure-log-domain-refinement´
/// ´claim:cas:enclosure-log-domain-refinement´
#[test]
fn enclosure_log_domain_refinement() -> TestResult {
    let mut f = Fixture::new()?;
    let two = f.rational(2, 1)?;
    let half = f.rational(1, 2)?;
    let root = f.term(
        Operator::GeneralPower(crate::model::Branch::StrictReal),
        &[two, half],
    )?;
    let floor = f.rational(5_931_641, 1 << 22)?;
    let difference = f.term(Operator::Sub, &[root, floor])?;
    let target = f.term(Operator::NaturalLog, &[difference])?;
    let request = f.request(target);
    let before = f.cas.canonical_snapshot(&mut ledger()?)?;
    let report = f.cas.enclose(
        &request,
        RefinementPolicy {
            rounds: 1,
            ..log_policy()
        },
        &mut ledger()?,
        &mut |_| false,
    )?;
    assert_eq!(
        report.outcome,
        EnclosureOutcome::Undecided(UndecidedReason::DomainBoundary)
    );
    assert!(report.checked.is_none());
    assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    let report = f
        .cas
        .enclose(&request, log_policy(), &mut ledger()?, &mut |_| false)?;
    assert_ne!(
        report.outcome,
        EnclosureOutcome::Undecided(UndecidedReason::DomainBoundary)
    );
    assert_ne!(report.outcome, EnclosureOutcome::Refuted);
    Ok(())
}
