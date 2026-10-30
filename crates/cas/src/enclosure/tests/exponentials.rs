// SPDX-License-Identifier: AGPL-3.0-only
//! Independent product bounds and adversarial finite exponential evidence.

use super::*;
use std::cmp::Ordering;

fn exp_policy() -> RefinementPolicy {
    RefinementPolicy {
        checking_work: 0,
        reduction_ceiling: 4,
        ..policy()
    }
}

fn exp_case(n: i64, d: u64) -> TestResult<(Fixture, EnclosureRequest)> {
    let mut f = Fixture::new()?;
    let input = f.rational(n, d)?;
    let target = f.term(Operator::Exponential, &[input])?;
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

/// Taylor enclosures lie inside independently computed product brackets for both signs.
/// ´test:unit:enclosure-exp-product-oracle´
/// ´claim:cas:enclosure-exp-product-oracle´
#[test]
fn enclosure_exp_product_oracle() -> TestResult {
    for (n, d) in [(0, 1), (1, 2), (-1, 2), (1, 1), (-1, 1), (2, 1), (-2, 1)] {
        let (mut f, request) = exp_case(n, d)?;
        let report = f
            .cas
            .enclose(&request, exp_policy(), &mut ledger()?, &mut |_| false)?;
        let checked = report.checked.ok_or("missing exponential bound")?;
        let oracle = product_oracle(n, d)?;
        // Check the working bound, before presentation can widen a narrow oracle overlap.
        let actual = checked.certificate.records.last().ok_or("record")?;
        assert_ne!(
            actual.lower().compare(oracle.lower(), &mut ledger()?)?,
            Ordering::Less
        );
        assert_ne!(
            actual.upper().compare(oracle.upper(), &mut ledger()?)?,
            Ordering::Greater
        );
        if n == 0 {
            assert_eq!(actual, &interval(1, 1, 1)?);
        }
        let bytes = checked.certificate.to_bytes(&mut ledger()?)?;
        let decoded = EnclosureCertificate::from_bytes(&bytes, &mut ledger()?)?;
        let (mut receiver, expected) = exp_case(n, d)?;
        let replay = receiver.cas.check_enclosure(
            &expected,
            exp_policy(),
            &decoded,
            &mut ledger()?,
            &mut |_| false,
        )?;
        assert_eq!(replay.checked.ok_or("replay")?.interval, checked.interval);
    }
    Ok(())
}

/// Every finite proposal is authenticated, including reduction, tail and original scope.
/// ´test:unit:enclosure-exp-corruptions´
/// ´claim:cas:enclosure-exp-corruptions´
#[test]
fn enclosure_exp_corruptions() -> TestResult {
    let (mut f, request) = exp_case(1, 1)?;
    let checked = f
        .cas
        .enclose(&request, exp_policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("bound")?;
    for mutation in 0..10 {
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
            _ => certificate.records[1] = interval(1, 1, 1)?,
        }
        let (mut receiver, expected) = exp_case(1, 1)?;
        let before = receiver.cas.canonical_snapshot(&mut ledger()?)?;
        assert!(
            receiver
                .cas
                .check_enclosure(
                    &expected,
                    exp_policy(),
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

/// Integer factorial bounds and binary searches select the least adequate degree.
/// ´test:unit:enclosure-exp-sizing´
/// ´claim:cas:enclosure-exp-sizing´
#[test]
fn enclosure_exp_sizing() -> TestResult {
    for m in 1..=4096_u32 {
        let direct: u64 = (1..=m).map(|j| u64::from(j.ilog2())).sum();
        assert_eq!(exp::sizing::factorial_bits(m), Some(direct));
    }
    for p in 0..32_u32 {
        for k in 0..=4_u32 {
            let policy = RefinementPolicy {
                precision: p,
                ..exp_policy()
            };
            if let Some(plan) = exp::plan(policy, k) {
                let t = p + 4 + k + (1 << k);
                let m = plan.terms + 1;
                assert!(
                    u64::from(m) + exp::sizing::factorial_bits(m).ok_or("factorial")?
                        >= u64::from(t + 1)
                );
                if m > 2 {
                    assert!(
                        u64::from(m - 1) + exp::sizing::factorial_bits(m - 1).ok_or("factorial")?
                            < u64::from(t + 1)
                    );
                }
                assert!(plan.grid >= t + 4);
            }
        }
    }
    for policy in [
        RefinementPolicy {
            precision: u32::MAX,
            ..exp_policy()
        },
        RefinementPolicy {
            term_ceiling: 0,
            ..exp_policy()
        },
        RefinementPolicy {
            grid_ceiling: 1,
            ..exp_policy()
        },
        RefinementPolicy {
            reduction_ceiling: 0,
            ..exp_policy()
        },
    ] {
        let (mut f, request) = exp_case(1, 1)?;
        let report = f
            .cas
            .enclose(&request, policy, &mut ledger()?, &mut |_| false)?;
        assert_eq!(report.outcome, EnclosureOutcome::Unsupported);
        assert_eq!(report.work, 0);
    }
    assert!(
        exp::plan(
            RefinementPolicy {
                reduction_ceiling: u32::MAX,
                ..exp_policy()
            },
            32
        )
        .is_none()
    );
    Ok(())
}

/// Charged interruption rolls back every speculative exponential judgment.
/// ´test:unit:enclosure-exp-interruption´
/// ´claim:cas:enclosure-exp-interruption´
#[test]
fn enclosure_exp_interruption() -> TestResult {
    for stop in 1..=12 {
        let (mut f, request) = exp_case(1, 2)?;
        let before = f.cas.canonical_snapshot(&mut ledger()?)?;
        let mut calls = 0;
        let report = f
            .cas
            .enclose(&request, exp_policy(), &mut ledger()?, &mut |_| {
                calls += 1;
                calls == stop
            })?;
        assert_eq!(report.outcome, EnclosureOutcome::Interrupted);
        assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// Generation, replay and retained lookup report distinct measured Work.
/// ´test:unit:enclosure-exp-work-pins´
/// ´claim:cas:enclosure-exp-work-pins´
#[test]
fn enclosure_exp_work_pins() -> TestResult {
    let (mut f, request) = exp_case(1, 2)?;
    let cold = f
        .cas
        .enclose(&request, exp_policy(), &mut ledger()?, &mut |_| false)?;
    let warm = f
        .cas
        .enclose(&request, exp_policy(), &mut ledger()?, &mut |_| false)?;
    let checked = cold.checked.ok_or("bound")?;
    let (mut receiver, expected) = exp_case(1, 2)?;
    let replay = receiver.cas.check_enclosure(
        &expected,
        exp_policy(),
        &checked.certificate,
        &mut ledger()?,
        &mut |_| false,
    )?;
    assert_eq!(
        (cold.work, warm.work, replay.work),
        (589_409, 18_833, 312_011)
    );
    for (is_warm, pin) in [(false, 589_409), (true, 18_833)] {
        for (available, passes) in [(pin, true), (pin - 1, false)] {
            let (mut store, request) = exp_case(1, 2)?;
            if is_warm {
                store
                    .cas
                    .enclose(&request, exp_policy(), &mut ledger()?, &mut |_| false)?;
            }
            let before = store.cas.canonical_snapshot(&mut ledger()?)?;
            let report = store.cas.enclose(
                &request,
                exp_policy(),
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
    for (available, passes) in [(312_011, true), (312_010, false)] {
        let (mut store, request) = exp_case(1, 2)?;
        let before = store.cas.canonical_snapshot(&mut ledger()?)?;
        let report = store.cas.check_enclosure(
            &request,
            exp_policy(),
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
/// ´test:unit:enclosure-exp-phase-pins´
/// ´claim:cas:enclosure-exp-phase-pins´
#[test]
fn enclosure_exp_phase_pins() -> TestResult {
    let (f, request) = exp_case(1, 2)?;
    let target = f.cas.state.expression(f.cas.checked(&request.target)?)?;
    let plans = planning::Plans::new(target, exp_policy()).ok_or("plan")?;
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
        (265_427, 298_692, 771, 4335, 18, 5)
    );
    for (pin, generation) in [(265_427, true), (298_692, false)] {
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

/// One extra term changes the checked recurrence, never its analytic authority.
/// ´test:unit:enclosure-exp-degree´
/// ´claim:cas:enclosure-exp-degree´
#[test]
fn enclosure_exp_degree() -> TestResult {
    let input = interval(1, 1, 2)?;
    let plan = exp::plan(exp_policy(), 1).ok_or("plan")?;
    let (a, first) = exp::produce(&input, 0, plan, &mut ledger()?, &mut |_| false)?;
    let (b, second) = exp::produce(
        &input,
        0,
        log::Plan {
            terms: plan.terms + 1,
            ..plan
        },
        &mut ledger()?,
        &mut |_| false,
    )?;
    exp::check(&input, &first, &a, plan.grid, &mut ledger()?, &mut |_| {
        false
    })?;
    exp::check(&input, &second, &b, plan.grid, &mut ledger()?, &mut |_| {
        false
    })?;
    let mut forged = first;
    forged.lower.series.degree += 1;
    assert!(
        exp::check(&input, &forged, &a, plan.grid, &mut ledger()?, &mut |_| {
            false
        })
        .is_err()
    );
    Ok(())
}

/// Box variation straddles honestly and both signs respect the finite reduction ceiling.
/// ´test:unit:enclosure-exp-ranges´
/// ´claim:cas:enclosure-exp-ranges´
#[test]
fn enclosure_exp_ranges() -> TestResult {
    let mut f = Fixture::new()?;
    let x = f.term(Operator::Free("x".into()), &[])?;
    let target = f.term(Operator::Exponential, std::slice::from_ref(&x))?;
    let mut request = f.request(target);
    request.coordinates.push(CoordinateBound {
        coordinate: x,
        interval: interval(0, 1, 1)?,
    });
    request.goal = EnclosureGoal::AtMost(number(3, 2)?);
    let report = f.cas.enclose(
        &request,
        RefinementPolicy {
            rounds: 1,
            ..exp_policy()
        },
        &mut ledger()?,
        &mut |_| false,
    )?;
    assert_eq!(
        report.outcome,
        EnclosureOutcome::Undecided(UndecidedReason::Straddle)
    );
    for x in [-7, 7] {
        let (mut f, request) = exp_case(x, 1)?;
        let report = f.cas.enclose(
            &request,
            RefinementPolicy {
                precision: 0,
                rounds: 1,
                ..exp_policy()
            },
            &mut ledger()?,
            &mut |_| false,
        )?;
        assert!(report.checked.is_some(), "{x}: {:?}", report.resource_fault);
        let checked = report.checked.ok_or("extreme bound")?;
        let actual = checked.certificate.records.last().ok_or("record")?;
        let oracle = product_oracle(x, 1)?;
        assert_ne!(
            actual.lower().compare(oracle.lower(), &mut ledger()?)?,
            Ordering::Less
        );
        assert_ne!(
            actual.upper().compare(oracle.upper(), &mut ledger()?)?,
            Ordering::Greater
        );
        let (mut f, request) = exp_case(x, 1)?;
        let before = f.cas.canonical_snapshot(&mut ledger()?)?;
        let report = f.cas.enclose(
            &request,
            exp_policy(),
            &mut Budget::new(Limits {
                output_bytes: 1,
                ..Limits::default()
            })?,
            &mut |_| false,
        )?;
        assert_eq!(report.outcome, EnclosureOutcome::Exhausted);
        assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// Wire, retention, degree and grid boundaries refuse at exactly one less.
/// ´test:unit:enclosure-exp-size-pins´
/// ´claim:cas:enclosure-exp-size-pins´
#[test]
fn enclosure_exp_size_pins() -> TestResult {
    let (mut f, request) = exp_case(1, 2)?;
    let checked = f
        .cas
        .enclose(&request, exp_policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("bound")?;
    let certificate = &checked.certificate;
    let bytes = certificate.to_bytes(&mut ledger()?)?;
    for (available, passes) in [(771, true), (770, false)] {
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
    for (available, passes) in [(4335, true), (4334, false)] {
        for resource in 0..3 {
            let (mut f, expected) = exp_case(1, 2)?;
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
        let (mut f, request) = exp_case(1, 2)?;
        let report = f.cas.enclose(
            &request,
            RefinementPolicy {
                term_ceiling: terms,
                grid_ceiling: grid,
                ..exp_policy()
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

/// Component and intermediate ceilings bound every rounded recurrence operation.
/// ´test:unit:enclosure-exp-arithmetic-size-pins´
/// ´claim:cas:enclosure-exp-arithmetic-size-pins´
#[test]
fn enclosure_exp_arithmetic_size_pins() -> TestResult {
    let input = interval(1, 1, 2)?;
    let plan = exp::plan(exp_policy(), 1).ok_or("plan")?;
    for (component, intermediate, passes) in [(37, 38, true), (36, 38, false), (37, 37, false)] {
        let mut budget = Budget::new(Limits {
            component_bits: component,
            intermediate_bits: intermediate,
            ..Limits::default()
        })?;
        assert_eq!(
            exp::produce(&input, 0, plan, &mut budget, &mut |_| false).is_ok(),
            passes
        );
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
