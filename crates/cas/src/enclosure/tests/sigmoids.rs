// SPDX-License-Identifier: AGPL-3.0-only
//! Independent product bounds and adversarial finite exponential evidence.

use super::*;
use std::cmp::Ordering;

fn sigmoid_policy() -> RefinementPolicy {
    RefinementPolicy {
        checking_work: 0,
        reduction_ceiling: 4,
        ..policy()
    }
}

fn sigmoid_case(n: i64, d: u64) -> TestResult<(Fixture, EnclosureRequest)> {
    let mut f = Fixture::new()?;
    let input = f.rational(n, d)?;
    let target = f.term(Operator::Sigmoid, &[input])?;
    let request = f.request(target);
    Ok((f, request))
}

/// Product inequalities use exact rational powers, no series or production reduction.
fn product_oracle(n: i64, d: u64) -> TestResult<Interval> {
    let mut budget = ledger()?;
    let magnitude = number(i64::try_from(n.unsigned_abs())?, d)?;
    let divided = magnitude.div(&number(32, 1)?, &mut budget)?;
    let lo = Rational::one()
        .add(&divided, &mut budget)?
        .pow(32, &mut budget)?;
    let hi = Rational::one()
        .sub(&divided, &mut budget)?
        .pow(-32, &mut budget)?;
    if n < 0 {
        Ok(Interval::new(
            Rational::one().div(&hi, &mut budget)?,
            Rational::one().div(&lo, &mut budget)?,
            &mut budget,
        )?)
    } else {
        Ok(Interval::new(lo, hi, &mut budget)?)
    }
}

/// Exact quotient of independent product bounds, without Taylor or interval operations.
fn sigmoid_oracle(n: i64, d: u64) -> TestResult<Interval> {
    let exp = product_oracle(-n, d)?;
    let mut budget = ledger()?;
    Ok(Interval::new(
        Rational::one().div(&Rational::one().add(exp.upper(), &mut budget)?, &mut budget)?,
        Rational::one().div(&Rational::one().add(exp.lower(), &mut budget)?, &mut budget)?,
        &mut budget,
    )?)
}

/// Checked sigmoid endpoints agree with independent brackets, exact zero and reflection.
/// ´test:unit:enclosure-sigmoid-oracle´
/// ´claim:cas:enclosure-sigmoid-oracle´
#[test]
fn enclosure_sigmoid_oracle() -> TestResult {
    for (n, d) in [(0, 1), (1, 2), (-1, 2), (1, 1), (-1, 1), (2, 1), (-2, 1)] {
        let (mut f, request) = sigmoid_case(n, d)?;
        let report = f
            .cas
            .enclose(&request, sigmoid_policy(), &mut ledger()?, &mut |_| false)?;
        let checked = report.checked.ok_or("sigmoid bound")?;
        let actual = checked.certificate.records.last().ok_or("record")?;
        let oracle = sigmoid_oracle(n, d)?;
        assert_ne!(
            actual.lower().compare(oracle.lower(), &mut ledger()?)?,
            Ordering::Less
        );
        assert_ne!(
            actual.upper().compare(oracle.upper(), &mut ledger()?)?,
            Ordering::Greater
        );
        if n == 0 {
            assert_eq!(actual, &interval(1, 1, 2)?);
        }
        let (mut reflected, other) = sigmoid_case(-n, d)?;
        let reflected = reflected
            .cas
            .enclose(&other, sigmoid_policy(), &mut ledger()?, &mut |_| false)?
            .checked
            .ok_or("reflection")?;
        let reflected = reflected.certificate.records.last().ok_or("record")?;
        let lo = Rational::one().sub(reflected.upper(), &mut ledger()?)?;
        let hi = Rational::one().sub(reflected.lower(), &mut ledger()?)?;
        assert_ne!(
            actual.lower().compare(&hi, &mut ledger()?)?,
            Ordering::Greater
        );
        assert_ne!(actual.upper().compare(&lo, &mut ledger()?)?, Ordering::Less);
        let bytes = checked.certificate.to_bytes(&mut ledger()?)?;
        let decoded = EnclosureCertificate::from_bytes(&bytes, &mut ledger()?)?;
        let (mut receiver, expected) = sigmoid_case(n, d)?;
        let replay = receiver.cas.check_enclosure(
            &expected,
            sigmoid_policy(),
            &decoded,
            &mut ledger()?,
            &mut |_| false,
        )?;
        assert_eq!(replay.checked.ok_or("replay")?.interval, checked.interval);
        assert!(
            checked
                .formal_schemas()
                .iter()
                .any(|s| s.name == "enclosure.exp-taylor" && s.version == 1)
        );
        let mut wrong_version = bytes;
        let name = b"enclosure.exp-taylor";
        let offset = wrong_version
            .windows(name.len())
            .position(|v| v == name)
            .ok_or("schema reference")?
            + name.len();
        wrong_version[offset] = 2;
        assert!(EnclosureCertificate::from_bytes(&wrong_version, &mut ledger()?).is_err());
    }
    Ok(())
}

/// Sign-crossing boxes and near-zero values retain uncertainty rather than force equality.
/// ´test:unit:enclosure-sigmoid-straddles´
/// ´claim:cas:enclosure-sigmoid-straddles´
#[test]
fn enclosure_sigmoid_straddles() -> TestResult {
    let mut f = Fixture::new()?;
    let x = f.term(Operator::Free("x".into()), &[])?;
    let target = f.term(Operator::Sigmoid, std::slice::from_ref(&x))?;
    let mut request = f.request(target);
    request.coordinates.push(CoordinateBound {
        coordinate: x,
        interval: interval(-1, 1, 1)?,
    });
    request.goal = EnclosureGoal::AtMost(number(1, 2)?);
    let report = f.cas.enclose(
        &request,
        RefinementPolicy {
            rounds: 1,
            reduction_ceiling: 1,
            ..sigmoid_policy()
        },
        &mut ledger()?,
        &mut |_| false,
    )?;
    assert_eq!(
        report.outcome,
        EnclosureOutcome::Undecided(UndecidedReason::Straddle)
    );
    for n in [-1, 1] {
        let (mut f, mut request) = sigmoid_case(n, 1 << 20)?;
        request.goal = EnclosureGoal::Equal(number(1, 2)?);
        let report = f.cas.enclose(
            &request,
            RefinementPolicy {
                rounds: 1,
                reduction_ceiling: 1,
                ..sigmoid_policy()
            },
            &mut ledger()?,
            &mut |_| false,
        )?;
        assert_ne!(report.outcome, EnclosureOutcome::Proved);
        let bound = report.checked.ok_or("near-zero bound")?;
        assert_ne!(bound.interval.lower(), bound.interval.upper());
    }
    for (n, endpoint) in [(2, 1), (-2, 0)] {
        let (mut f, mut request) = sigmoid_case(n, 1)?;
        request.goal = EnclosureGoal::Equal(number(endpoint, 1)?);
        let report = f.cas.enclose(
            &request,
            RefinementPolicy {
                precision: 0,
                rounds: 1,
                ..sigmoid_policy()
            },
            &mut ledger()?,
            &mut |_| false,
        )?;
        assert_eq!(
            report.outcome,
            EnclosureOutcome::Undecided(UndecidedReason::Straddle)
        );
        let bound = report.checked.ok_or("limiting presentation")?;
        assert!(
            bound.interval.lower() == &number(endpoint, 1)?
                || bound.interval.upper() == &number(endpoint, 1)?
        );
    }
    Ok(())
}

/// Each composition binds its arguments, exponential premises, range and original scope.
/// ´test:unit:enclosure-sigmoid-corruptions´
/// ´claim:cas:enclosure-sigmoid-corruptions´
#[test]
fn enclosure_sigmoid_corruptions() -> TestResult {
    let (mut f, request) = sigmoid_case(1, 1)?;
    let checked = f
        .cas
        .enclose(&request, sigmoid_policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("bound")?;
    for mutation in 0..12 {
        let mut certificate = checked.certificate.clone();
        let endpoint = &mut certificate.exponentials[0].lower;
        match mutation {
            0 => endpoint.squarings += 1,
            1 => endpoint.argument = number(1, 3)?,
            2 => endpoint.series.degree += 1,
            3 => endpoint.series.tail = interval(0, 0, 1)?,
            4 => endpoint.series.omitted = interval(0, 0, 1)?,
            5 => endpoint.series.sum = interval(1, 1, 1)?,
            6 => certificate.exponentials[0].record = 0,
            7 => certificate.scope[0] ^= 1,
            8 => certificate.version += 1,
            9 => certificate.records[1] = interval(0, 1, 1)?,
            10 => certificate.exponentials.clear(),
            _ => {
                let bound = &certificate.records[1];
                certificate.records[1] = Interval::new(
                    Rational::one().sub(bound.upper(), &mut ledger()?)?,
                    Rational::one().sub(bound.lower(), &mut ledger()?)?,
                    &mut ledger()?,
                )?;
            }
        }
        let (mut receiver, expected) = sigmoid_case(1, 1)?;
        let before = receiver.cas.canonical_snapshot(&mut ledger()?)?;
        assert!(
            receiver
                .cas
                .check_enclosure(
                    &expected,
                    sigmoid_policy(),
                    &certificate,
                    &mut ledger()?,
                    &mut |_| false
                )
                .is_err(),
            "mutation {mutation}"
        );
        assert_eq!(receiver.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// Charged interruption rolls back every speculative sigmoid judgment.
/// ´test:unit:enclosure-sigmoid-interruption´
/// ´claim:cas:enclosure-sigmoid-interruption´
#[test]
fn enclosure_sigmoid_interruption() -> TestResult {
    for stop in 1..=12 {
        let (mut f, request) = sigmoid_case(1, 2)?;
        let before = f.cas.canonical_snapshot(&mut ledger()?)?;
        let mut calls = 0;
        let report = f
            .cas
            .enclose(&request, sigmoid_policy(), &mut ledger()?, &mut |_| {
                calls += 1;
                calls == stop
            })?;
        assert_eq!(report.outcome, EnclosureOutcome::Interrupted);
        assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// Generation, replay and retained lookup report distinct measured Work.
/// ´test:unit:enclosure-sigmoid-work-pins´
/// ´claim:cas:enclosure-sigmoid-work-pins´
#[test]
fn enclosure_sigmoid_work_pins() -> TestResult {
    let (mut f, request) = sigmoid_case(1, 2)?;
    let cold = f
        .cas
        .enclose(&request, sigmoid_policy(), &mut ledger()?, &mut |_| false)?;
    let warm = f
        .cas
        .enclose(&request, sigmoid_policy(), &mut ledger()?, &mut |_| false)?;
    let checked = cold.checked.ok_or("bound")?;
    let (mut receiver, expected) = sigmoid_case(1, 2)?;
    let replay = receiver.cas.check_enclosure(
        &expected,
        sigmoid_policy(),
        &checked.certificate,
        &mut ledger()?,
        &mut |_| false,
    )?;
    assert_eq!(
        (cold.work, warm.work, replay.work),
        (749_495, 18_829, 400_424)
    );
    for (is_warm, pin) in [(false, 749_495), (true, 18_829)] {
        for (available, passes) in [(pin, true), (pin - 1, false)] {
            let (mut store, request) = sigmoid_case(1, 2)?;
            if is_warm {
                store
                    .cas
                    .enclose(&request, sigmoid_policy(), &mut ledger()?, &mut |_| false)?;
            }
            let before = store.cas.canonical_snapshot(&mut ledger()?)?;
            let report = store.cas.enclose(
                &request,
                sigmoid_policy(),
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
    for (available, passes) in [(400_424, true), (400_423, false)] {
        let (mut store, request) = sigmoid_case(1, 2)?;
        let before = store.cas.canonical_snapshot(&mut ledger()?)?;
        let report = store.cas.check_enclosure(
            &request,
            sigmoid_policy(),
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
/// ´test:unit:enclosure-sigmoid-phase-pins´
/// ´claim:cas:enclosure-sigmoid-phase-pins´
#[test]
fn enclosure_sigmoid_phase_pins() -> TestResult {
    let (f, request) = sigmoid_case(1, 2)?;
    let target = f.cas.state.expression(f.cas.checked(&request.target)?)?;
    let plans = planning::Plans::new(target, sigmoid_policy()).ok_or("plan")?;
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
            grid,
            plans.exp.ok_or("exp plan")?.terms
        ),
        (337_100, 387_005, 807, 4367, 18, 5)
    );
    for (pin, generation) in [(337_100, true), (387_005, false)] {
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
/// ´test:unit:enclosure-sigmoid-size-pins´
/// ´claim:cas:enclosure-sigmoid-size-pins´
#[test]
fn enclosure_sigmoid_size_pins() -> TestResult {
    let (mut f, request) = sigmoid_case(1, 2)?;
    let checked = f
        .cas
        .enclose(&request, sigmoid_policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("bound")?;
    let certificate = &checked.certificate;
    let bytes = certificate.to_bytes(&mut ledger()?)?;
    for (available, passes) in [(807, true), (806, false)] {
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
    for (available, passes) in [(4367, true), (4366, false)] {
        for resource in 0..3 {
            let (mut f, expected) = sigmoid_case(1, 2)?;
            let checked =
                checker::check(&f.cas, &expected, certificate, &mut ledger()?, &mut |_| {
                    false
                })?;
            let prefix = service::prefix(&certificate.scope, 18, 6, &mut ledger()?)?;
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
    for (terms, grid, passes) in [(5, 18, true), (4, 18, false), (5, 17, false)] {
        let (mut f, request) = sigmoid_case(1, 2)?;
        let report = f.cas.enclose(
            &request,
            RefinementPolicy {
                term_ceiling: terms,
                grid_ceiling: grid,
                ..sigmoid_policy()
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
