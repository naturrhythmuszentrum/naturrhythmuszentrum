// SPDX-License-Identifier: AGPL-3.0-only
//! Branchwise controls distinguish a generalized hull from a classical derivative.

use super::*;

/// Generalized queries reject complex operands at admission while ordinary derivatives retain them.
/// ´test:unit:enclosure-clamp-real-sort´
/// ´claim:cas:enclosure-clamp-real-sort´
#[test]
fn enclosure_clamp_real_sort() -> TestResult {
    let sort = Sort::Value(AtomicSort::Scalar(Field::Complex));
    let mut cas = Cas::new(Limits::default())?;
    let header = cas
        .admit(
            &[Draft::Header(HeaderDraft::Core(Environment {
                declarations: vec![Declaration {
                    key: "z".into(),
                    display_name: "z".into(),
                    sort: sort.clone(),
                }],
                ..Environment::default()
            }))],
            &mut ledger()?,
        )?
        .remove(0);
    let operand = cas
        .admit(
            &[Draft::Expression(ExpressionDraft {
                space: None,
                operator: Operator::Free("z".into()),
                sort: sort.clone(),
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                binding: None,
                operands: Vec::new(),
            })],
            &mut ledger()?,
        )?
        .remove(0);
    for generalized in [false, true] {
        let result = cas.admit(
            &[Draft::Expression(ExpressionDraft {
                space: None,
                operator: if generalized {
                    Operator::GeneralizedDerivative("z".into())
                } else {
                    Operator::Derivative("z".into())
                },
                sort: sort.clone(),
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                binding: None,
                operands: vec![Reference::Existing(operand.clone())],
            })],
            &mut ledger()?,
        );
        assert_eq!(result.is_err(), generalized);
    }
    Ok(())
}

fn clamp_policy() -> RefinementPolicy {
    RefinementPolicy {
        checking_work: 0,
        rounds: 1,
        reduction_ceiling: 0,
        ..policy()
    }
}

fn clamp_case(lo: i64, hi: i64, derivative: bool) -> TestResult<(Fixture, EnclosureRequest)> {
    let mut f = Fixture::new()?;
    let x = f.term(Operator::Free("x".into()), &[])?;
    let wall = f.rational(1, 1)?;
    let floor = f.term(Operator::ClampFloor, &[x.clone(), wall])?;
    let value = f.term(Operator::Mul, &[floor.clone(), floor])?;
    let target = if derivative {
        f.term(Operator::GeneralizedDerivative("x".into()), &[value])?
    } else {
        value
    };
    let mut request = f.request(target);
    request.coordinates.push(CoordinateBound {
        coordinate: x,
        interval: interval(lo, hi, 1)?,
    });
    Ok((f, request))
}

/// Below, above, touching and crossing boxes retain the exact branch coverage and fresh replay.
/// ´test:unit:enclosure-clamp-branches´
/// ´claim:cas:enclosure-clamp-branches´
#[test]
fn enclosure_clamp_branches() -> TestResult {
    for (lo, hi, value, slope) in [
        (-2, 0, (1, 1), (0, 0)),
        (2, 3, (4, 9), (4, 6)),
        (0, 1, (1, 1), (0, 2)),
        (0, 2, (1, 4), (0, 4)),
        (1, 1, (1, 1), (0, 2)),
    ] {
        for derivative in [false, true] {
            let (mut f, request) = clamp_case(lo, hi, derivative)?;
            let checked = f
                .cas
                .enclose(&request, clamp_policy(), &mut ledger()?, &mut |_| false)?
                .checked
                .ok_or("clamp")?;
            let (a, b) = if derivative { slope } else { value };
            assert_eq!(checked.interval, interval(a, b, 1)?);
            assert_eq!(checked.formal_schema().name, "enclosure.clamp-generalized");
            assert!(
                !checked
                    .formal_schemas()
                    .iter()
                    .any(|schema| schema.name == "enclosure.smooth-derivative")
            );
            let decoded = EnclosureCertificate::from_bytes(
                &checked.certificate.to_bytes(&mut ledger()?)?,
                &mut ledger()?,
            )?;
            let (mut receiver, request) = clamp_case(lo, hi, derivative)?;
            let replay = receiver.cas.check_enclosure(
                &request,
                clamp_policy(),
                &decoded,
                &mut ledger()?,
                &mut |_| false,
            )?;
            assert_eq!(replay.checked.ok_or("replay")?.interval, checked.interval);
        }
    }
    Ok(())
}

/// The complete uncertain wall region is covered at fixed wall, not differentiated as a coordinate.
/// ´test:unit:enclosure-clamp-uncertain-wall´
/// ´claim:cas:enclosure-clamp-uncertain-wall´
#[test]
fn enclosure_clamp_uncertain_wall() -> TestResult {
    let mut f = Fixture::with_names(&["x", "a"])?;
    let x = f.term(Operator::Free("x".into()), &[])?;
    let a = f.term(Operator::Free("a".into()), &[])?;
    let floor = f.term(Operator::ClampFloor, &[x.clone(), a.clone()])?;
    let target = f.term(
        Operator::GeneralizedDerivative("x".into()),
        std::slice::from_ref(&floor),
    )?;
    let mut request = f.request(target);
    request.coordinates = vec![
        CoordinateBound {
            coordinate: x,
            interval: interval(2, 2, 1)?,
        },
        CoordinateBound {
            coordinate: a,
            interval: interval(1, 3, 1)?,
        },
    ];
    let bound = f
        .cas
        .enclose(&request, clamp_policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("wall hull")?;
    assert_eq!(bound.interval, interval(0, 1, 1)?);
    request.target = floor;
    let bound = f
        .cas
        .enclose(&request, clamp_policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("wall value")?;
    assert_eq!(bound.interval, interval(2, 3, 1)?);
    Ok(())
}

/// Only the original single floor construction grants generalized derivative premises.
/// ´test:unit:enclosure-clamp-source´
/// ´claim:cas:enclosure-clamp-source´
#[test]
fn enclosure_clamp_source() -> TestResult {
    for variant in 0..3 {
        let mut f = Fixture::with_names(&["x", "y"])?;
        let x = f.term(Operator::Free("x".into()), &[])?;
        let one = f.rational(1, 1)?;
        let floor = f.term(Operator::ClampFloor, &[x.clone(), one])?;
        let source = match variant {
            0 => f.term(Operator::Add, &[floor, x.clone()])?,
            1 => {
                let two = f.rational(2, 1)?;
                let other = f.term(Operator::ClampFloor, &[x.clone(), two])?;
                f.term(Operator::Add, &[floor, other])?
            }
            _ => floor,
        };
        let coordinate = if variant == 2 { "y" } else { "x" };
        let target = f.term(
            Operator::GeneralizedDerivative(coordinate.into()),
            &[source],
        )?;
        let mut request = f.request(target);
        request.coordinates.push(CoordinateBound {
            coordinate: x,
            interval: interval(0, 2, 1)?,
        });
        let before = f.cas.canonical_snapshot(&mut ledger()?)?;
        assert!(
            f.cas
                .enclose(&request, clamp_policy(), &mut ledger()?, &mut |_| false)
                .is_err()
        );
        assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// Generalized bounds quantify over all slopes and cannot become an ordinary scalar operand.
/// ´test:unit:enclosure-clamp-straddle´
/// ´claim:cas:enclosure-clamp-straddle´
#[test]
fn enclosure_clamp_straddle() -> TestResult {
    let (mut f, mut request) = clamp_case(0, 2, true)?;
    request.goal = EnclosureGoal::AtMost(number(1, 1)?);
    let report = f
        .cas
        .enclose(&request, clamp_policy(), &mut ledger()?, &mut |_| false)?;
    assert_eq!(
        report.outcome,
        EnclosureOutcome::Undecided(UndecidedReason::Straddle)
    );
    request.goal = EnclosureGoal::AtMost(number(4, 1)?);
    assert_eq!(
        f.cas
            .enclose(&request, clamp_policy(), &mut ledger()?, &mut |_| false)?
            .outcome,
        EnclosureOutcome::Proved
    );
    let one = f.rational(1, 1)?;
    request.target = f.term(Operator::Add, &[request.target, one])?;
    let report = f
        .cas
        .enclose(&request, clamp_policy(), &mut ledger()?, &mut |_| false)?;
    assert_eq!(report.outcome, EnclosureOutcome::Unsupported);
    assert_eq!(report.work, 0);
    Ok(())
}

/// Positive inner quantities support fourth-root activation; a singular wall grants values only.
/// ´test:unit:enclosure-clamp-regularity´
/// ´claim:cas:enclosure-clamp-regularity´
#[test]
fn enclosure_clamp_regularity() -> TestResult {
    for wall in [0, 1] {
        let mut f = Fixture::new()?;
        let x = f.term(Operator::Free("x".into()), &[])?;
        let a = f.rational(wall, 1)?;
        let floor = f.term(Operator::ClampFloor, &[x.clone(), a])?;
        let half = f.rational(1, 2)?;
        let root = f.term(
            Operator::GeneralPower(crate::model::Branch::StrictReal),
            &[floor, half.clone()],
        )?;
        let fourth = f.term(
            Operator::GeneralPower(crate::model::Branch::StrictReal),
            &[root, half],
        )?;
        let target = f.term(
            Operator::GeneralizedDerivative("x".into()),
            std::slice::from_ref(&fourth),
        )?;
        let smooth = f.term(
            Operator::Derivative("x".into()),
            std::slice::from_ref(&fourth),
        )?;
        let mut request = f.request(fourth);
        request.coordinates.push(CoordinateBound {
            coordinate: x,
            interval: interval(wall, wall, 1)?,
        });
        assert!(
            f.cas
                .enclose(&request, clamp_policy(), &mut ledger()?, &mut |_| false)?
                .checked
                .is_some()
        );
        request.target = target;
        let before = f.cas.canonical_snapshot(&mut ledger()?)?;
        let outcome = f
            .cas
            .enclose(&request, clamp_policy(), &mut ledger()?, &mut |_| false);
        if wall == 0 {
            assert!(outcome.is_err());
            assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
        } else {
            assert_eq!(
                outcome?.checked.ok_or("fourth")?.interval,
                interval(0, 1, 4)?
            );
        }
        request.target = smooth;
        let report = f
            .cas
            .enclose(&request, clamp_policy(), &mut ledger()?, &mut |_| false)?;
        assert_eq!(report.outcome, EnclosureOutcome::Unsupported);
        assert_eq!(report.work, 0);
    }
    Ok(())
}

/// Fourth-root activation encloses exact responsive derivatives across a positive complete box.
/// ´test:unit:enclosure-clamp-activation-box´
/// ´claim:cas:enclosure-clamp-activation-box´
#[test]
fn enclosure_clamp_activation_box() -> TestResult {
    let mut f = Fixture::new()?;
    let x = f.term(Operator::Free("x".into()), &[])?;
    let wall = f.rational(1, 1)?;
    let floor = f.term(Operator::ClampFloor, &[x.clone(), wall])?;
    let half = f.rational(1, 2)?;
    let root = f.term(
        Operator::GeneralPower(crate::model::Branch::StrictReal),
        &[floor, half.clone()],
    )?;
    let fourth = f.term(
        Operator::GeneralPower(crate::model::Branch::StrictReal),
        &[root, half],
    )?;
    let target = f.term(Operator::GeneralizedDerivative("x".into()), &[fourth])?;
    let mut request = f.request(target);
    request.coordinates.push(CoordinateBound {
        coordinate: x,
        interval: interval(0, 16, 1)?,
    });
    let bound = f
        .cas
        .enclose(&request, clamp_policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("activation")?;
    for expected in [number(0, 1)?, number(1, 4)?, number(1, 32)?] {
        assert!(bound.interval.contains(
            &Interval::singleton(&expected, &mut ledger()?)?,
            &mut ledger()?
        )?);
    }
    Ok(())
}

/// Branch identities, original wall, coverage, source and derivative conclusions cannot be forged.
/// ´test:unit:enclosure-clamp-corruptions´
/// ´claim:cas:enclosure-clamp-corruptions´
#[test]
fn enclosure_clamp_corruptions() -> TestResult {
    let (mut f, request) = clamp_case(0, 2, true)?;
    let checked = f
        .cas
        .enclose(&request, clamp_policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("clamp")?;
    for mutation in 0..10 {
        let mut certificate = checked.certificate.clone();
        match mutation {
            0 => certificate.derivatives[0].steps[2].rule = DerivativeRule::ClampResponsive,
            1 => certificate.derivatives[0].steps[2].bound = interval(1, 1, 1)?,
            2 => certificate.records[1] = interval(0, 0, 1)?,
            3 => certificate.derivatives[0].coordinate = "a".into(),
            4 => certificate.law = EnclosureLaw::SmoothDerivative,
            5 => certificate.derivatives.clear(),
            6 => certificate.scope[0] ^= 1,
            7 => certificate.records[2] = interval(2, 2, 1)?,
            8 => certificate.derivatives[0].steps[2].rule = DerivativeRule::Coordinate,
            _ => certificate.derivatives[0]
                .steps
                .pop()
                .ok_or("step")
                .map(|_| ())?,
        }
        let (mut receiver, request) = clamp_case(0, 2, true)?;
        let before = receiver.cas.canonical_snapshot(&mut ledger()?)?;
        assert!(
            receiver
                .cas
                .check_enclosure(
                    &request,
                    clamp_policy(),
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

/// Interruption at deterministic charged boundaries publishes no partial clamp evidence.
/// ´test:unit:enclosure-clamp-interruption´
/// ´claim:cas:enclosure-clamp-interruption´
#[test]
fn enclosure_clamp_interruption() -> TestResult {
    for stop in 1..=12 {
        let (mut f, request) = clamp_case(1, 1, true)?;
        let before = f.cas.canonical_snapshot(&mut ledger()?)?;
        let mut calls = 0;
        let report = f
            .cas
            .enclose(&request, clamp_policy(), &mut ledger()?, &mut |_| {
                calls += 1;
                calls == stop
            })?;
        assert_eq!(report.outcome, EnclosureOutcome::Interrupted);
        assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// Fresh replay interruption cannot retain a prefix of generalized derivative evidence.
/// ´test:unit:enclosure-clamp-replay-interruption´
/// ´claim:cas:enclosure-clamp-replay-interruption´
#[test]
fn enclosure_clamp_replay_interruption() -> TestResult {
    let (mut f, request) = clamp_case(0, 2, true)?;
    let checked = f
        .cas
        .enclose(&request, clamp_policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("clamp")?;
    for stop in 1..=12 {
        let (mut receiver, request) = clamp_case(0, 2, true)?;
        let before = receiver.cas.canonical_snapshot(&mut ledger()?)?;
        let mut calls = 0;
        let report = receiver.cas.check_enclosure(
            &request,
            clamp_policy(),
            &checked.certificate,
            &mut ledger()?,
            &mut |_| {
                calls += 1;
                calls == stop
            },
        )?;
        assert_eq!(report.outcome, EnclosureOutcome::Interrupted);
        assert_eq!(receiver.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// Generation, replay and retained lookup report distinct measured Work.
/// ´test:unit:enclosure-clamp-work-pins´
/// ´claim:cas:enclosure-clamp-work-pins´
#[test]
fn enclosure_clamp_work_pins() -> TestResult {
    let (mut f, request) = clamp_case(0, 0, true)?;
    let cold = f
        .cas
        .enclose(&request, clamp_policy(), &mut ledger()?, &mut |_| false)?;
    let warm = f
        .cas
        .enclose(&request, clamp_policy(), &mut ledger()?, &mut |_| false)?;
    let checked = cold.checked.ok_or("bound")?;
    let (mut receiver, expected) = clamp_case(0, 0, true)?;
    let replay = receiver.cas.check_enclosure(
        &expected,
        clamp_policy(),
        &checked.certificate,
        &mut ledger()?,
        &mut |_| false,
    )?;
    assert_eq!(
        (cold.work, warm.work, replay.work),
        (185_515, 33_774, 102_228)
    );
    for (is_warm, pin) in [(false, 185_515), (true, 33_774)] {
        for (available, passes) in [(pin, true), (pin - 1, false)] {
            let (mut store, request) = clamp_case(0, 0, true)?;
            if is_warm {
                store
                    .cas
                    .enclose(&request, clamp_policy(), &mut ledger()?, &mut |_| false)?;
            }
            let before = store.cas.canonical_snapshot(&mut ledger()?)?;
            let report = store.cas.enclose(
                &request,
                clamp_policy(),
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
    for (available, passes) in [(102_228, true), (102_227, false)] {
        let (mut store, request) = clamp_case(0, 0, true)?;
        let before = store.cas.canonical_snapshot(&mut ledger()?)?;
        let report = store.cas.check_enclosure(
            &request,
            clamp_policy(),
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
/// ´test:unit:enclosure-clamp-phase-pins´
/// ´claim:cas:enclosure-clamp-phase-pins´
#[test]
fn enclosure_clamp_phase_pins() -> TestResult {
    let (f, request) = clamp_case(0, 0, true)?;
    let target = f.cas.state.expression(f.cas.checked(&request.target)?)?;
    let plans = planning::Plans::new(target, clamp_policy()).ok_or("plan")?;
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
        (57_957, 84_235, 1195, 6194, 12)
    );
    for (pin, generation) in [(57_957, true), (84_235, false)] {
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
/// ´test:unit:enclosure-clamp-size-pins´
/// ´claim:cas:enclosure-clamp-size-pins´
#[test]
fn enclosure_clamp_size_pins() -> TestResult {
    let (mut f, request) = clamp_case(0, 0, true)?;
    let checked = f
        .cas
        .enclose(&request, clamp_policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("bound")?;
    let certificate = &checked.certificate;
    let bytes = certificate.to_bytes(&mut ledger()?)?;
    for (available, passes) in [(1195, true), (1194, false)] {
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
    for (available, passes) in [(6194, true), (6193, false)] {
        for resource in 0..3 {
            let (mut f, expected) = clamp_case(0, 0, true)?;
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
        let (mut f, request) = clamp_case(0, 0, true)?;
        let report = f.cas.enclose(
            &request,
            RefinementPolicy {
                grid_ceiling: grid,
                ..clamp_policy()
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
