// SPDX-License-Identifier: AGPL-3.0-only
//! Independent closed derivatives, full-box controls and transactional replay boundaries.

use super::*;

/// Ordered chain, sum and difference rules keep independent coordinate identities.
/// ´test:unit:enclosure-derivative-coordinates´
/// ´claim:cas:enclosure-derivative-coordinates´
#[test]
fn enclosure_derivative_coordinates() -> TestResult {
    for coordinate in ["x", "y"] {
        let mut f = Fixture::with_names(&["x", "y"])?;
        let x = f.term(Operator::Free("x".into()), &[])?;
        let y = f.term(Operator::Free("y".into()), &[])?;
        let sum = f.term(Operator::Add, &[x.clone(), y.clone()])?;
        let product = f.term(Operator::Mul, &[sum.clone(), sum])?;
        let difference = f.term(Operator::Sub, &[product, x.clone()])?;
        let source = f.term(Operator::Neg, &[difference])?;
        let target = f.term(Operator::Derivative(coordinate.into()), &[source])?;
        let mut request = f.request(target);
        request.coordinates = vec![
            CoordinateBound {
                coordinate: x,
                interval: interval(1, 2, 1)?,
            },
            CoordinateBound {
                coordinate: y,
                interval: interval(3, 4, 1)?,
            },
        ];
        let bound = f
            .cas
            .enclose(&request, derivative_policy(), &mut ledger()?, &mut |_| {
                false
            })?
            .checked
            .ok_or("chain")?;
        for a in 1..=2 {
            for b in 3..=4 {
                let expected = -2 * (a + b) + i64::from(coordinate == "x");
                assert!(
                    bound
                        .interval
                        .contains(&interval(expected, expected, 1)?, &mut ledger()?)?
                );
            }
        }
        request.coordinates[0].interval = interval(1, 3, 1)?;
        assert!(
            f.cas
                .check_enclosure(
                    &request,
                    derivative_policy(),
                    &bound.certificate,
                    &mut ledger()?,
                    &mut |_| false
                )
                .is_err()
        );
    }
    Ok(())
}

/// A nontrivial analytic chain encloses its full-box derivative, not its midpoint.
/// ´test:unit:enclosure-derivative-chain-box´
/// ´claim:cas:enclosure-derivative-chain-box´
#[test]
fn enclosure_derivative_chain_box() -> TestResult {
    let mut f = Fixture::new()?;
    let x = f.term(Operator::Free("x".into()), &[])?;
    let two = f.rational(2, 1)?;
    let inner = f.term(Operator::Mul, &[two, x.clone()])?;
    let logarithm = f.term(Operator::NaturalLog, &[inner])?;
    let target = f.term(Operator::Derivative("x".into()), &[logarithm])?;
    let mut request = f.request(target);
    request.coordinates.push(CoordinateBound {
        coordinate: x,
        interval: interval(1, 2, 1)?,
    });
    let bound = f
        .cas
        .enclose(
            &request,
            RefinementPolicy {
                reduction_ceiling: 2,
                ..derivative_policy()
            },
            &mut ledger()?,
            &mut |_| false,
        )?
        .checked
        .ok_or("log chain")?;
    for denominator in 4..=8 {
        let expected = number(4, denominator)?;
        assert!(bound.interval.contains(
            &Interval::singleton(&expected, &mut ledger()?)?,
            &mut ledger()?
        )?);
    }
    assert_ne!(bound.interval.lower(), bound.interval.upper());
    Ok(())
}

fn derivative_policy() -> RefinementPolicy {
    RefinementPolicy {
        checking_work: 0,
        rounds: 1,
        reduction_ceiling: 0,
        ..policy()
    }
}

fn derivative_case(quotient: bool, lo: i64, hi: i64) -> TestResult<(Fixture, EnclosureRequest)> {
    let mut f = Fixture::new()?;
    let x = f.term(Operator::Free("x".into()), &[])?;
    let source = if quotient {
        let one = f.rational(1, 1)?;
        let two = f.rational(2, 1)?;
        let sum = f.term(Operator::Add, &[x.clone(), two])?;
        f.term(Operator::Div, &[one, sum])?
    } else {
        f.term(Operator::Mul, &[x.clone(), x.clone()])?
    };
    let target = f.term(Operator::Derivative("x".into()), &[source])?;
    let mut request = f.request(target);
    request.coordinates.push(CoordinateBound {
        coordinate: x,
        interval: interval(lo, hi, 1)?,
    });
    Ok((f, request))
}

/// Independent polynomial and rational derivatives are contained across the entire box.
/// ´test:unit:enclosure-derivative-oracles´
/// ´claim:cas:enclosure-derivative-oracles´
#[test]
fn enclosure_derivative_oracles() -> TestResult {
    for quotient in [false, true] {
        let (mut f, request) = derivative_case(quotient, -1, 1)?;
        let checked = f
            .cas
            .enclose(&request, derivative_policy(), &mut ledger()?, &mut |_| {
                false
            })?
            .checked
            .ok_or("bound")?;
        for numerator in -4..=4 {
            let expected = if quotient {
                number(-16, u64::try_from((numerator + 8) * (numerator + 8))?)?
            } else {
                number(numerator, 2)?
            };
            assert!(checked.interval.contains(
                &Interval::singleton(&expected, &mut ledger()?)?,
                &mut ledger()?
            )?);
        }
        let decoded = EnclosureCertificate::from_bytes(
            &checked.certificate.to_bytes(&mut ledger()?)?,
            &mut ledger()?,
        )?;
        let (mut receiver, request) = derivative_case(quotient, -1, 1)?;
        let replay = receiver.cas.check_enclosure(
            &request,
            derivative_policy(),
            &decoded,
            &mut ledger()?,
            &mut |_| false,
        )?;
        assert_eq!(replay.checked.ok_or("replay")?.interval, checked.interval);
        assert_eq!(checked.formal_schema().name, "enclosure.smooth-derivative");
    }
    Ok(())
}

/// Every closed analytic derivative uses its checked original value and fixed exponent.
/// ´test:unit:enclosure-derivative-identities´
/// ´claim:cas:enclosure-derivative-identities´
#[test]
fn enclosure_derivative_identities() -> TestResult {
    for (operator, point, n, d) in [
        (Operator::Exponential, 0, 1, 1),
        (Operator::NaturalLog, 1, 1, 1),
        (Operator::Sigmoid, 0, 1, 4),
        (Operator::Tanh, 0, 1, 1),
        (Operator::PositivePower, 4, 1, 4),
        (
            Operator::GeneralPower(crate::model::Branch::StrictReal),
            4,
            1,
            4,
        ),
    ] {
        let mut f = Fixture::new()?;
        let x = f.term(Operator::Free("x".into()), &[])?;
        let mut operands = vec![x.clone()];
        if matches!(
            operator,
            Operator::PositivePower | Operator::GeneralPower(_)
        ) {
            operands.push(f.rational(1, 2)?);
        }
        let value = f.term(operator, &operands)?;
        let target = f.term(Operator::Derivative("x".into()), &[value])?;
        let mut request = f.request(target);
        request.coordinates.push(CoordinateBound {
            coordinate: x,
            interval: interval(point, point, 1)?,
        });
        let checked = f
            .cas
            .enclose(&request, derivative_policy(), &mut ledger()?, &mut |_| {
                false
            })?
            .checked
            .ok_or("analytic")?;
        assert!(checked.interval.contains(
            &Interval::singleton(&number(n, d)?, &mut ledger()?)?,
            &mut ledger()?
        )?);
        let decoded = EnclosureCertificate::from_bytes(
            &checked.certificate.to_bytes(&mut ledger()?)?,
            &mut ledger()?,
        )?;
        assert_eq!(decoded, checked.certificate);
    }
    Ok(())
}

/// A valid zero root value grants no smooth derivative; higher derivatives stay unsupported.
/// ´test:unit:enclosure-derivative-regularity´
/// ´claim:cas:enclosure-derivative-regularity´
#[test]
fn enclosure_derivative_regularity() -> TestResult {
    let mut f = Fixture::new()?;
    let x = f.term(Operator::Free("x".into()), &[])?;
    let half = f.rational(1, 2)?;
    let root = f.term(
        Operator::GeneralPower(crate::model::Branch::StrictReal),
        &[x.clone(), half],
    )?;
    let derivative = f.term(
        Operator::Derivative("x".into()),
        std::slice::from_ref(&root),
    )?;
    let second = f.term(
        Operator::Derivative("x".into()),
        std::slice::from_ref(&derivative),
    )?;
    let mut request = f.request(root);
    request.coordinates.push(CoordinateBound {
        coordinate: x,
        interval: interval(0, 0, 1)?,
    });
    assert!(
        f.cas
            .enclose(&request, derivative_policy(), &mut ledger()?, &mut |_| {
                false
            })?
            .checked
            .is_some()
    );
    request.target = derivative;
    let before = f.cas.canonical_snapshot(&mut ledger()?)?;
    assert!(
        f.cas
            .enclose(&request, derivative_policy(), &mut ledger()?, &mut |_| {
                false
            })
            .is_err()
    );
    assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    request.target = second;
    let report = f
        .cas
        .enclose(&request, derivative_policy(), &mut ledger()?, &mut |_| {
            false
        })?;
    assert_eq!(report.outcome, EnclosureOutcome::Unsupported);
    assert_eq!(report.work, 0);
    Ok(())
}

/// Symbolic identities, coordinates, complete boxes and every constituent are checked separately.
/// ´test:unit:enclosure-derivative-corruptions´
/// ´claim:cas:enclosure-derivative-corruptions´
#[test]
fn enclosure_derivative_corruptions() -> TestResult {
    let (mut f, request) = derivative_case(false, 1, 2)?;
    let checked = f
        .cas
        .enclose(&request, derivative_policy(), &mut ledger()?, &mut |_| {
            false
        })?
        .checked
        .ok_or("bound")?;
    for mutation in 0..10 {
        let mut certificate = checked.certificate.clone();
        match mutation {
            0 => certificate.derivatives[0].steps[0].rule = DerivativeRule::Constant,
            1 => certificate.derivatives[0].steps[0].bound = interval(0, 0, 1)?,
            2 => certificate.derivatives[0].first = 1,
            3 => certificate.derivatives[0].record = 0,
            4 => certificate.derivatives[0].coordinate = "y".into(),
            5 => {
                certificate.derivatives[0].steps.pop();
            }
            6 => certificate.derivatives.clear(),
            7 => certificate.scope[0] ^= 1,
            8 => certificate.records[0] = interval(1, 1, 1)?,
            _ => certificate.records[3] = interval(0, 10, 1)?,
        }
        let (mut receiver, request) = derivative_case(false, 1, 2)?;
        let before = receiver.cas.canonical_snapshot(&mut ledger()?)?;
        assert!(
            receiver
                .cas
                .check_enclosure(
                    &request,
                    derivative_policy(),
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

/// Deterministic charged interruption leaves no partial derivative judgment.
/// ´test:unit:enclosure-derivative-interruption´
/// ´claim:cas:enclosure-derivative-interruption´
#[test]
fn enclosure_derivative_interruption() -> TestResult {
    for stop in 1..=8 {
        let (mut f, request) = derivative_case(false, 1, 1)?;
        let before = f.cas.canonical_snapshot(&mut ledger()?)?;
        let mut calls = 0;
        let report = f
            .cas
            .enclose(&request, derivative_policy(), &mut ledger()?, &mut |_| {
                calls += 1;
                calls == stop
            })?;
        assert_eq!(report.outcome, EnclosureOutcome::Interrupted);
        assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// Generation, replay and retained lookup report distinct measured Work.
/// ´test:unit:enclosure-derivative-work-pins´
/// ´claim:cas:enclosure-derivative-work-pins´
#[test]
fn enclosure_derivative_work_pins() -> TestResult {
    let (mut f, request) = derivative_case(false, 1, 1)?;
    let cold = f
        .cas
        .enclose(&request, derivative_policy(), &mut ledger()?, &mut |_| {
            false
        })?;
    let warm = f
        .cas
        .enclose(&request, derivative_policy(), &mut ledger()?, &mut |_| {
            false
        })?;
    let checked = cold.checked.ok_or("bound")?;
    let (mut receiver, expected) = derivative_case(false, 1, 1)?;
    let replay = receiver.cas.check_enclosure(
        &expected,
        derivative_policy(),
        &checked.certificate,
        &mut ledger()?,
        &mut |_| false,
    )?;
    assert_eq!(
        (cold.work, warm.work, replay.work),
        (133_343, 26_819, 75_545)
    );
    for (is_warm, pin) in [(false, 133_343), (true, 26_819)] {
        for (available, passes) in [(pin, true), (pin - 1, false)] {
            let (mut store, request) = derivative_case(false, 1, 1)?;
            if is_warm {
                store
                    .cas
                    .enclose(&request, derivative_policy(), &mut ledger()?, &mut |_| {
                        false
                    })?;
            }
            let before = store.cas.canonical_snapshot(&mut ledger()?)?;
            let report = store.cas.enclose(
                &request,
                derivative_policy(),
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
    for (available, passes) in [(75_545, true), (75_544, false)] {
        let (mut store, request) = derivative_case(false, 1, 1)?;
        let before = store.cas.canonical_snapshot(&mut ledger()?)?;
        let report = store.cas.check_enclosure(
            &request,
            derivative_policy(),
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
/// ´test:unit:enclosure-derivative-phase-pins´
/// ´claim:cas:enclosure-derivative-phase-pins´
#[test]
fn enclosure_derivative_phase_pins() -> TestResult {
    let (f, request) = derivative_case(false, 1, 1)?;
    let target = f.cas.state.expression(f.cas.checked(&request.target)?)?;
    let plans = planning::Plans::new(target, derivative_policy()).ok_or("plan")?;
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
        (38_034, 61_556, 936, 4609, 12)
    );
    for (pin, generation) in [(38_034, true), (61_556, false)] {
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
/// ´test:unit:enclosure-derivative-size-pins´
/// ´claim:cas:enclosure-derivative-size-pins´
#[test]
fn enclosure_derivative_size_pins() -> TestResult {
    let (mut f, request) = derivative_case(false, 1, 1)?;
    let checked = f
        .cas
        .enclose(&request, derivative_policy(), &mut ledger()?, &mut |_| {
            false
        })?
        .checked
        .ok_or("bound")?;
    let certificate = &checked.certificate;
    let bytes = certificate.to_bytes(&mut ledger()?)?;
    for (available, passes) in [(936, true), (935, false)] {
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
    for (available, passes) in [(4609, true), (4608, false)] {
        for resource in 0..3 {
            let (mut f, expected) = derivative_case(false, 1, 1)?;
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
        let (mut f, request) = derivative_case(false, 1, 1)?;
        let report = f.cas.enclose(
            &request,
            RefinementPolicy {
                grid_ceiling: grid,
                ..derivative_policy()
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
