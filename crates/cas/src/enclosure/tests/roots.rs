// SPDX-License-Identifier: AGPL-3.0-only
//! Integer oracles and original-scope controls for the zero-capable grid law.

use super::*;
use crate::budget::BudgetKind;
use crate::model::Branch;

fn root(fixture: &mut Fixture, base: Handle) -> TestResult<Handle> {
    let half = fixture.rational(1, 2)?;
    fixture.term(Operator::GeneralPower(Branch::StrictReal), &[base, half])
}

fn root_case(
    numerator: i64,
    denominator: u64,
    fourth: bool,
) -> TestResult<(Fixture, EnclosureRequest)> {
    let mut fixture = Fixture::new()?;
    let base = fixture.rational(numerator, denominator)?;
    let square = root(&mut fixture, base)?;
    let target = if fourth {
        root(&mut fixture, square)?
    } else {
        square
    };
    let request = fixture.request(target);
    Ok((fixture, request))
}

fn root_policy() -> RefinementPolicy {
    RefinementPolicy {
        checking_work: 0,
        ..policy()
    }
}

fn pair(value: &Rational) -> TestResult<(i128, i128)> {
    let (numerator, denominator) = value.small_ratio(&mut ledger()?)?.ok_or("small oracle")?;
    Ok((i128::from(numerator), i128::from(denominator)))
}

/// Integer cross-products establish both grid neighbors without production root arithmetic.
/// ´test:unit:enclosure-root-integer-oracle´
/// ´claim:cas:enclosure-root-integer-oracle´
#[test]
fn enclosure_root_integer_oracle() -> TestResult {
    for numerator in 0..=32_i64 {
        for denominator in 1..=9_u64 {
            for grid in 0..=5_u32 {
                let value = number(numerator, denominator)?;
                let (lower, upper) = value.square_root_grid(grid, &mut ledger()?)?;
                let (lo, ld) = pair(&lower)?;
                let (hi, hd) = pair(&upper)?;
                let scale = 1_i128 << grid;
                let n = i128::from(numerator);
                let d = i128::from(denominator);
                assert_eq!(lo * scale % ld, 0);
                assert_eq!(hi * scale % hd, 0);
                let k = lo * scale / ld;
                assert!(k * k * d <= n * scale * scale);
                assert!((k + 1) * (k + 1) * d > n * scale * scale);
                if k * k * d == n * scale * scale {
                    assert_eq!(lower, upper);
                } else {
                    assert_eq!(hi * scale / hd, k + 1);
                }
                let input = Interval::singleton(&value, &mut ledger()?)?;
                let proposed = Interval::new(lower, upper, &mut ledger()?)?;
                crate::enclosure::roots::check(&input, &proposed, grid, &mut ledger()?)?;
            }
        }
    }
    Ok(())
}

/// Adjacent squared grid points retain equality and the correct outward neighbor.
/// ´test:unit:enclosure-root-grid-boundaries´
/// ´claim:cas:enclosure-root-grid-boundaries´
#[test]
fn enclosure_root_grid_boundaries() -> TestResult {
    for grid in 1..=5_u32 {
        let scale = 1_i64 << grid;
        for k in 1..=7_i64 {
            for delta in -1..=1_i64 {
                let value = number(2 * k * k + delta, u64::try_from(2 * scale * scale)?)?;
                let (lower, upper) = value.square_root_grid(grid, &mut ledger()?)?;
                let expected = number(k, u64::try_from(scale)?)?;
                match delta {
                    -1 => assert_eq!(upper, expected),
                    0 => {
                        assert_eq!(lower, expected);
                        assert_eq!(upper, lower);
                    }
                    _ => assert_eq!(lower, expected),
                }
            }
        }
    }
    Ok(())
}

/// Nested zero-capable instances enclose fourth roots and replay in a fresh store.
/// ´test:unit:enclosure-fourth-root-containment´
/// ´claim:cas:enclosure-fourth-root-containment´
#[test]
fn enclosure_fourth_root_containment() -> TestResult {
    for (numerator, denominator) in [(0, 1), (1, 1), (16, 1), (81, 16), (2, 1), (7, 5)] {
        let (mut fixture, request) = root_case(numerator, denominator, true)?;
        let result = fixture
            .cas
            .enclose(&request, root_policy(), &mut ledger()?, &mut |_| false)?;
        let checked = result.checked.ok_or("fourth root")?;
        let (lower, lower_d) = pair(checked.interval().lower())?;
        let (upper, upper_d) = pair(checked.interval().upper())?;
        assert!(lower.pow(4) * i128::from(denominator) <= i128::from(numerator) * lower_d.pow(4));
        assert!(upper.pow(4) * i128::from(denominator) >= i128::from(numerator) * upper_d.pow(4));
        assert_eq!(checked.formal_schema().name, "enclosure.root-grid");
        assert_eq!(checked.formal_schemas().len(), 2);
        let wire = checked.certificate().to_bytes(&mut ledger()?)?;
        let decoded = EnclosureCertificate::from_bytes(&wire, &mut ledger()?)?;
        let (mut receiving, expected) = root_case(numerator, denominator, true)?;
        let replay = receiving.cas.check_enclosure(
            &expected,
            root_policy(),
            &decoded,
            &mut ledger()?,
            &mut |_| false,
        )?;
        assert_eq!(
            checked.interval(),
            replay.checked.ok_or("replay")?.interval()
        );
    }
    Ok(())
}

/// Every changed root endpoint, grid and original-scope join refuses without publication.
/// ´test:unit:enclosure-root-mutations´
/// ´claim:cas:enclosure-root-mutations´
#[test]
fn enclosure_root_mutations() -> TestResult {
    let (mut fixture, request) = root_case(2, 1, false)?;
    let certificate = fixture
        .cas
        .enclose(&request, root_policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("certificate")?
        .certificate()
        .clone();
    for control in 0..8 {
        let (mut receiving, expected) = root_case(2, 1, false)?;
        let before = receiving.cas.canonical_snapshot(&mut ledger()?)?;
        let mut changed = certificate.clone();
        let last = changed.records.last_mut().ok_or("last")?;
        match control {
            0 => *last = Interval::singleton(last.lower(), &mut ledger()?)?,
            1 => *last = Interval::singleton(last.upper(), &mut ledger()?)?,
            2 => {
                *last = Interval::new(
                    last.upper().negated(&mut ledger()?)?,
                    last.lower().negated(&mut ledger()?)?,
                    &mut ledger()?,
                )?;
            }
            3 => *last = Interval::new(Rational::zero(), last.upper().clone(), &mut ledger()?)?,
            4 => changed.grid -= 1,
            5 => changed.records[0] = interval(3, 3, 1)?,
            6 => changed.law = EnclosureLaw::RationalInclusion,
            _ => changed.records[1] = interval(1, 1, 3)?,
        }
        let changed =
            EnclosureCertificate::from_bytes(&changed.to_bytes(&mut ledger()?)?, &mut ledger()?)?;
        assert!(
            receiving
                .cas
                .check_enclosure(
                    &expected,
                    root_policy(),
                    &changed,
                    &mut ledger()?,
                    &mut |_| false
                )
                .is_err(),
            "control {control}"
        );
        assert_eq!(receiving.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    let (mut different, expected) = root_case(3, 1, false)?;
    assert!(
        different
            .cas
            .check_enclosure(
                &expected,
                root_policy(),
                &certificate,
                &mut ledger()?,
                &mut |_| false
            )
            .is_err()
    );
    let mut wrong_context = request.clone();
    let added = fixture.cas.admit(
        &[
            Draft::Region(RegionDraft {
                header: Reference::Existing(fixture.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::Empty,
            }),
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(fixture.header.clone()),
                admissible: Reference::Local(0),
                assumptions: Vec::new(),
            }),
        ],
        &mut ledger()?,
    )?;
    wrong_context.context = added[1].clone();
    assert!(
        fixture
            .cas
            .check_enclosure(
                &wrong_context,
                root_policy(),
                &certificate,
                &mut ledger()?,
                &mut |_| false
            )
            .is_err()
    );
    Ok(())
}

/// Negative domains refuse; unsupported powers and excessive grids publish no judgment.
/// ´test:unit:enclosure-root-refusals´
/// ´claim:cas:enclosure-root-refusals´
#[test]
fn enclosure_root_refusals() -> TestResult {
    let (mut zero, zero_request) = root_case(0, 1, false)?;
    assert!(
        crate::exact_constant::ExactConstant::from_expression(
            &zero.cas,
            &zero_request.target,
            &mut ledger()?
        )
        .is_err()
    );
    assert_eq!(
        zero.cas
            .enclose(&zero_request, root_policy(), &mut ledger()?, &mut |_| false)?
            .outcome,
        EnclosureOutcome::Bound
    );
    let (mut fixture, request) = root_case(-1, 1, false)?;
    let before = fixture.cas.canonical_snapshot(&mut ledger()?)?;
    assert!(
        fixture
            .cas
            .enclose(&request, root_policy(), &mut ledger()?, &mut |_| false)
            .is_err()
    );
    assert_eq!(fixture.cas.canonical_snapshot(&mut ledger()?)?, before);
    let (mut fixture, request) = root_case(2, 1, false)?;
    let mut limited = Budget::new(Limits {
        integer_exponent: 23,
        ..Limits::default()
    })?;
    let report = fixture
        .cas
        .enclose(&request, root_policy(), &mut limited, &mut |_| false)?;
    assert_eq!(report.outcome, EnclosureOutcome::Exhausted);
    assert_eq!(report.work, 0);
    let mut bounded = ledger()?;
    assert!(
        number(2, 1)?
            .square_root_grid(u32::MAX, &mut bounded)
            .is_err()
    );
    assert_eq!(bounded.usage().work, 0);
    let third = fixture.rational(1, 3)?;
    let two = fixture.rational(2, 1)?;
    let power = fixture.term(Operator::GeneralPower(Branch::StrictReal), &[two, third])?;
    let result = fixture.cas.enclose(
        &fixture.request(power),
        root_policy(),
        &mut ledger()?,
        &mut |_| false,
    )?;
    assert_eq!(result.outcome, EnclosureOutcome::Unsupported);
    assert_eq!(result.work, 0);
    Ok(())
}

/// Monotone endpoint lifting, nested refinement and zero predicates keep distinct outcomes.
/// ´test:unit:enclosure-root-refinement´
/// ´claim:cas:enclosure-root-refinement´
#[test]
fn enclosure_root_refinement() -> TestResult {
    let mut fixture = Fixture::new()?;
    let coordinate = fixture.term(Operator::Free("x".into()), &[])?;
    let target = root(&mut fixture, coordinate.clone())?;
    let mut request = fixture.request(target);
    request.coordinates.push(CoordinateBound {
        coordinate,
        interval: interval(0, 4, 1)?,
    });
    request.goal = EnclosureGoal::AtMost(number(1, 1)?);
    let result = fixture
        .cas
        .enclose(&request, root_policy(), &mut ledger()?, &mut |_| false)?;
    assert_eq!(
        result.outcome,
        EnclosureOutcome::Undecided(UndecidedReason::Straddle)
    );
    assert_eq!(
        result.checked.ok_or("bound")?.interval(),
        &interval(0, 2, 1)?
    );
    let (mut zero, mut request) = root_case(0, 1, true)?;
    request.goal = EnclosureGoal::Equal(Rational::zero());
    assert_eq!(
        zero.cas
            .enclose(&request, root_policy(), &mut ledger()?, &mut |_| false)?
            .outcome,
        EnclosureOutcome::Proved
    );
    let (mut tiny, request) = root_case(3, 1_u64 << 50, true)?;
    let first = tiny.cas.enclose(
        &request,
        RefinementPolicy {
            precision: 12,
            rounds: 1,
            ..root_policy()
        },
        &mut ledger()?,
        &mut |_| false,
    )?;
    assert_eq!(
        first.outcome,
        EnclosureOutcome::Undecided(UndecidedReason::WidthLimit)
    );
    let refined = tiny.cas.enclose(
        &request,
        RefinementPolicy {
            precision: 12,
            ..root_policy()
        },
        &mut ledger()?,
        &mut |_| false,
    )?;
    assert_eq!(refined.completed_rounds, 2);
    assert_eq!(refined.outcome, EnclosureOutcome::Bound);
    let previous = first.checked.ok_or("first")?;
    let later = refined.checked.ok_or("later")?;
    assert_eq!(
        previous
            .interval()
            .intersection(later.interval(), &mut ledger()?)?,
        *later.interval()
    );
    Ok(())
}

/// Interruption never publishes a partial root judgment or confuses stopping with refutation.
/// ´test:unit:enclosure-root-interruption´
/// ´claim:cas:enclosure-root-interruption´
#[test]
fn enclosure_root_interruption() -> TestResult {
    for stop in 1..=8 {
        let (mut fixture, request) = root_case(2, 1, false)?;
        let before = fixture.cas.canonical_snapshot(&mut ledger()?)?;
        let mut calls = 0;
        let report = fixture
            .cas
            .enclose(&request, root_policy(), &mut ledger()?, &mut |_| {
                calls += 1;
                calls == stop
            })?;
        assert_eq!(report.outcome, EnclosureOutcome::Interrupted);
        assert!(report.checked.is_none());
        assert_eq!(fixture.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// Root generation and independent square replay have separate measured Work boundaries.
/// ´test:unit:enclosure-root-work-pins´
/// ´claim:cas:enclosure-root-work-pins´
#[test]
fn enclosure_root_work_pins() -> TestResult {
    let value = number(2, 1)?;
    let mut generation = ledger()?;
    let (lower, upper) = value.square_root_grid(12, &mut generation)?;
    let input = Interval::singleton(&value, &mut ledger()?)?;
    let proposed = Interval::new(lower, upper, &mut ledger()?)?;
    let mut checking = ledger()?;
    crate::enclosure::roots::check(&input, &proposed, 12, &mut checking)?;
    let (mut fixture, request) = root_case(2, 1, false)?;
    let cold = fixture
        .cas
        .enclose(&request, root_policy(), &mut ledger()?, &mut |_| false)?;
    let warm = fixture
        .cas
        .enclose(&request, root_policy(), &mut ledger()?, &mut |_| false)?;
    let (mut fourth, request) = root_case(2, 1, true)?;
    let fourth_cold = fourth
        .cas
        .enclose(&request, root_policy(), &mut ledger()?, &mut |_| false)?;
    let fourth_warm = fourth
        .cas
        .enclose(&request, root_policy(), &mut ledger()?, &mut |_| false)?;
    assert_eq!(
        (
            generation.usage().work,
            checking.usage().work,
            cold.work,
            warm.work,
            fourth_cold.work,
            fourth_warm.work
        ),
        (1_696, 10_368, 63_739, 20_283, 90_467, 23_510)
    );
    for (fourth, cold, warm) in [(false, 63_739, 20_283), (true, 90_467, 23_510)] {
        request_work_boundary(fourth, false, cold)?;
        request_work_boundary(fourth, true, warm)?;
    }
    for (limit, passes) in [(1_696, true), (1_695, false)] {
        assert_eq!(
            value.square_root_grid(12, &mut work_limit(limit)?).is_ok(),
            passes
        );
    }
    for (limit, passes) in [(10_368, true), (10_367, false)] {
        assert_eq!(
            crate::enclosure::roots::check(&input, &proposed, 12, &mut work_limit(limit)?).is_ok(),
            passes
        );
    }
    Ok(())
}

/// Shift, component, intermediate and retained-byte ceilings are literal refusal boundaries.
/// ´test:unit:enclosure-root-size-pins´
/// ´claim:cas:enclosure-root-size-pins´
#[test]
fn enclosure_root_size_pins() -> TestResult {
    let value = number(2, 1)?;
    for (kind, limit) in [
        (BudgetKind::IntegerExponent, 24),
        (BudgetKind::ComponentBits, 13),
        (BudgetKind::IntermediateBits, 26),
        (BudgetKind::StagedPayloadBytes, 384),
    ] {
        for (available, passes) in [(limit, true), (limit - 1, false)] {
            let mut limits = Limits::default();
            match kind {
                BudgetKind::IntegerExponent => limits.integer_exponent = available,
                BudgetKind::ComponentBits => limits.component_bits = available,
                BudgetKind::IntermediateBits => {
                    limits.component_bits = 13;
                    limits.intermediate_bits = available;
                }
                _ => limits.staged_payload_bytes = available,
            }
            assert_eq!(
                value
                    .square_root_grid(12, &mut Budget::new(limits)?)
                    .is_ok(),
                passes,
                "{kind:?} {available}"
            );
        }
    }
    let (mut fixture, request) = root_case(2, 1, false)?;
    let checked = fixture
        .cas
        .enclose(&request, root_policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("checked")?;
    let certificate = checked.certificate();
    let wire = certificate.to_bytes(&mut ledger()?)?;
    let prefix = service::prefix(&certificate.scope, 12, 6, &mut ledger()?)?;
    let key = service::retention_key(&prefix, certificate, &mut ledger()?)?;
    let retained = footprint(&checked) + u64::try_from(key.len())?;
    assert_eq!((wire.len(), retained), (607, 2945));
    root_byte_boundaries(certificate)?;
    Ok(())
}

fn work_limit(work: u64) -> Result<Budget, Fault> {
    Budget::new(Limits {
        work,
        ..Limits::default()
    })
}

fn root_byte_boundaries(certificate: &EnclosureCertificate) -> TestResult {
    let encoded = certificate.to_bytes(&mut ledger()?)?;
    for (bytes, passes) in [(607, true), (606, false)] {
        let limits = Limits {
            witness_bytes: bytes,
            ..Limits::default()
        };
        assert_eq!(
            certificate
                .to_bytes(&mut Budget::new(limits.clone())?)
                .is_ok(),
            passes
        );
        assert_eq!(
            EnclosureCertificate::from_bytes(&encoded, &mut Budget::new(limits)?).is_ok(),
            passes
        );
    }
    for (bytes, passes) in [(2945, true), (2944, false)] {
        for output in [true, false] {
            let (mut receiving, expected) = root_case(2, 1, false)?;
            let checked = checker::check(
                &receiving.cas,
                &expected,
                certificate,
                &mut ledger()?,
                &mut |_| false,
            )?;
            let prefix = service::prefix(&certificate.scope, 12, 6, &mut ledger()?)?;
            let key = service::retention_key(&prefix, certificate, &mut ledger()?)?;
            let limits = if output {
                Limits {
                    output_bytes: bytes,
                    ..Limits::default()
                }
            } else {
                Limits {
                    staged_payload_bytes: bytes,
                    ..Limits::default()
                }
            };
            let before = receiving.cas.canonical_snapshot(&mut ledger()?)?;
            assert_eq!(
                receiving
                    .cas
                    .retain_enclosure(key, checked, &mut Budget::new(limits)?)
                    .is_ok(),
                passes
            );
            if !passes {
                assert_eq!(receiving.cas.canonical_snapshot(&mut ledger()?)?, before);
            }
        }
    }
    Ok(())
}

fn request_work_boundary(fourth: bool, warm: bool, limit: u64) -> TestResult {
    for (available, passes) in [(limit, true), (limit - 1, false)] {
        let (mut fixture, request) = root_case(2, 1, fourth)?;
        if warm {
            fixture
                .cas
                .enclose(&request, root_policy(), &mut ledger()?, &mut |_| false)?;
        }
        let before = fixture.cas.canonical_snapshot(&mut ledger()?)?;
        let report = fixture.cas.enclose(
            &request,
            root_policy(),
            &mut work_limit(available)?,
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
            assert_eq!(fixture.cas.canonical_snapshot(&mut ledger()?)?, before);
        }
    }
    Ok(())
}
