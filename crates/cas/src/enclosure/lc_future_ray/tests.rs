// SPDX-License-Identifier: AGPL-3.0-only
//! Exact future-ray controls with an external rational polygon oracle.

use super::*;
use crate::budget::{Budget, Limits};
use crate::enclosure::lc_affine_fibre::{TimingObservation, TimingSourceKind, TimingSourceSlot};
use crate::pi_bounds::PiBoundCertificate;
use crate::{Cas, Fault, FaultKind, InvalidCode};

type TestResult = Result<(), Box<dyn std::error::Error>>;
fn wide() -> Result<Budget, Fault> {
    Budget::new(Limits::blind_cubic())
}
fn q(n: i64, b: &mut Budget) -> Result<Rational, Fault> {
    Rational::from_ratio(n, 1, b)
}
fn fraction(text: &str, b: &mut Budget) -> Result<Rational, Box<dyn std::error::Error>> {
    let (numerator, denominator) = text.split_once('/').unwrap_or((text, "1"));
    Ok(Rational::from_ratio(
        numerator.parse()?,
        denominator.parse()?,
        b,
    )?)
}
fn pi(b: &mut Budget) -> Result<PiBoundCertificate, Fault> {
    Ok(PiBoundCertificate {
        version: 1,
        terms: [2, 1],
        sums: [
            Rational::from_ratio(74, 375, b)?,
            Rational::from_ratio(1, 239, b)?,
        ],
        lower: Rational::from_ratio(128, 41, b)?,
        upper: Rational::from_ratio(640, 203, b)?,
    })
}
fn timing() -> Result<TimingRequest, Fault> {
    let b = &mut wide()?;
    let first_observation = 0;
    let rate = [q(1, b)?, q(2, b)?];
    let initial = [q(0, b)?, q(1, b)?];
    let time = [q(0, b)?, q(0, b)?];
    let phase = [q(0, b)?, q(1, b)?];
    Ok(TimingRequest {
        epoch: "future epoch".into(),
        nominal: vec![q(0, b)?],
        source_order: vec![
            TimingSourceSlot {
                kind: TimingSourceKind::Rate,
                declaration: 0,
            },
            TimingSourceSlot {
                kind: TimingSourceKind::Initial,
                declaration: 0,
            },
            TimingSourceSlot {
                kind: TimingSourceKind::Time(first_observation),
                declaration: 0,
            },
            TimingSourceSlot {
                kind: TimingSourceKind::Phase(first_observation),
                declaration: 0,
            },
        ],
        rate: rate.clone(),
        initial: initial.clone(),
        source_versions: vec![1; 4],
        rate_declarations: vec![rate],
        initial_declarations: vec![initial],
        observations: vec![TimingObservation {
            time: time.clone(),
            phase: phase.clone(),
            time_declarations: vec![time],
            phase_declarations: vec![phase],
            lift: 0,
            lift_domain: [0, 0],
        }],
        pi: pi(b)?,
        revision: 1,
    })
}
fn request(cas: &Cas) -> Result<FutureRayRequest, Fault> {
    let source = timing()?;
    let certificate = cas
        .construct_lc_affine_timing(&source, &mut wide()?)?
        .certificate(cas, &mut wide()?)?;
    Ok(FutureRayRequest {
        form: FutureRayForm::Plain,
        reference: q(1, &mut wide()?)?,
        safe_reference: PiAffine::rational(q(1, &mut wide()?)?),
        narrowed: false,
        epoch: source.epoch.clone(),
        branches: vec![FutureRayBranch {
            id: 0,
            shift: 0,
            source: FutureRaySource::Timing(source, certificate),
        }],
    })
}
#[test]
fn lc_future_ray_plain() -> TestResult {
    let mut cas = Cas::new(Limits::blind_cubic())?;
    let request = request(&cas)?;
    let receipt = cas.construct_lc_future_ray(&request, &mut wide()?)?;
    assert_eq!(receipt.formal_schema().label, "thm:cas:lc-future-ray");
    assert_eq!(receipt.original(&cas, &mut wide()?)?, &request);
    let proposal = receipt.proposal(&cas, &mut wide()?)?;
    // Fraction enumeration outside Rust: constraints 1<=a<=2, 0<=alpha<=1.
    // Lexicographic polygon vertices (a,alpha) and R=1 ray differences.
    let expected = [[1, 0], [1, 1], [2, 0], [2, 1]];
    assert_eq!(proposal.vertices.len(), expected.len());
    for (vertex, pair) in proposal.vertices.iter().zip(expected) {
        assert_eq!(
            vertex.point[0],
            PiAffine::rational(q(pair[0], &mut wide()?)?)
        );
        assert_eq!(
            vertex.point[1],
            PiAffine::rational(q(pair[1], &mut wide()?)?)
        );
    }
    assert_eq!(proposal.hull, [0, 2, 3, 1]);
    assert_eq!((proposal.low, proposal.high), (0, 3));
    let oracle = [[0, 0, 1, 2], [0, 1, 1, 1], [1, 1, 0, 1], [1, 2, 0, 0]];
    for (row, values) in proposal.comparisons.iter().zip(oracle) {
        for (value, expected) in row.iter().zip(values) {
            assert_eq!(*value, PiAffine::rational(q(expected, &mut wide()?)?));
        }
    }
    assert_eq!(proposal.copy_charged(&mut wide()?)?, *proposal);
    let copy_bytes = wire::encode(&[], proposal, &mut wide()?)?.len() as u64;
    let short_copy = Limits {
        output_bytes: copy_bytes - 1,
        ..Limits::blind_cubic()
    };
    let fault = proposal
        .copy_charged(&mut Budget::new(short_copy)?)
        .expect_err("copy output limit");
    assert!(
        matches!(fault.kind, FaultKind::Exhausted { budget: crate::budget::BudgetKind::OutputBytes, limit, needed, .. } if limit == copy_bytes - 1 && needed == copy_bytes)
    );
    assert_eq!(fault.location, AT);
    let cert = receipt.certificate(&cas, &mut wide()?)?;
    let replay = cas.check_lc_future_ray(&request, &cert, &mut wide()?)?;
    assert_eq!(replay.proposal(&cas, &mut wide()?)?, proposal);
    match receipt.interpolate(
        &cas,
        &Rational::from_ratio(1, 2, &mut wide()?)?,
        &mut wide()?,
    )? {
        FutureRayWitness::Timing(witness) => {
            assert_eq!(
                witness.rate,
                PiAffine::rational(fraction("3/2", &mut wide()?)?)
            );
            assert_eq!(
                witness.initial,
                PiAffine::rational(fraction("1/2", &mut wide()?)?)
            );
        }
        _ => panic!("wrong original witness"),
    }
    let fault = receipt
        .interpolate(&cas, &q(2, &mut wide()?)?, &mut wide()?)
        .expect_err("weight outside unit interval");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedCertificate)
    );
    assert_eq!(fault.location, AT);
    Ok(())
}
#[test]
fn lc_future_ray_reference() -> TestResult {
    let mut cas = Cas::new(Limits::blind_cubic())?;
    let mut input = request(&cas)?;
    input.reference = Rational::zero();
    input.safe_reference = PiAffine::rational(Rational::zero());
    let fault = cas
        .construct_lc_future_ray(&input, &mut wide()?)
        .expect_err("named future-ray refusal");
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::PendingPremise));
    assert_eq!(fault.location, REFERENCE_AT);
    Ok(())
}
#[test]
fn lc_future_ray_signed_start() -> TestResult {
    let mut cas = Cas::new(Limits::blind_cubic())?;
    let mut original = timing()?;
    let time = [q(-2, &mut wide()?)?, q(-2, &mut wide()?)?];
    original.observations[0].time = time.clone();
    original.observations[0].time_declarations = vec![time];
    original.nominal[0] = q(-2, &mut wide()?)?;
    let rate = [q(1, &mut wide()?)?, q(2, &mut wide()?)?];
    let initial = [Rational::zero(), q(2, &mut wide()?)?];
    let exact_phase = [q(-2, &mut wide()?)?, q(-2, &mut wide()?)?];
    original.rate = rate.clone();
    original.rate_declarations = vec![rate];
    original.initial = initial.clone();
    original.initial_declarations = vec![initial];
    original.observations[0].phase = exact_phase.clone();
    original.observations[0].phase_declarations = vec![exact_phase];
    let certificate = cas
        .construct_lc_affine_timing(&original, &mut wide()?)?
        .certificate(&cas, &mut wide()?)?;
    let input = FutureRayRequest {
        form: FutureRayForm::Plain,
        reference: q(-1, &mut wide()?)?,
        safe_reference: PiAffine::rational(q(-1, &mut wide()?)?),
        narrowed: false,
        epoch: original.epoch.clone(),
        branches: vec![FutureRayBranch {
            id: 0,
            shift: 0,
            source: FutureRaySource::Timing(original, certificate),
        }],
    };
    let receipt = cas.construct_lc_future_ray(&input, &mut wide()?)?;
    let proposal = receipt.proposal(&cas, &mut wide()?)?.clone();
    // At t=-2, a*t+alpha=-2 gives alpha=2*a-2. With a in [1,2],
    // the segment endpoints are (1,0) and (2,2). At R=-1 the latter
    // exceeds the former by one, yielding the four signed ray differences.
    assert_eq!(proposal.vertices.len(), 2);
    for (vertex, pair) in proposal.vertices.iter().zip([[1, 0], [2, 2]]) {
        for (value, number) in vertex.point.iter().zip(pair) {
            assert_eq!(*value, PiAffine::rational(q(number, &mut wide()?)?));
        }
    }
    assert_eq!((proposal.low, proposal.high), (0, 1));
    assert_eq!(proposal.hull, [0, 1]);
    for (row, expected) in proposal
        .comparisons
        .iter()
        .zip([[0, 0, 1, 1], [1, 1, 0, 0]])
    {
        for (value, number) in row.iter().zip(expected) {
            assert_eq!(*value, PiAffine::rational(q(number, &mut wide()?)?));
        }
    }
    let certificate = receipt.certificate(&cas, &mut wide()?)?;
    assert_eq!(
        cas.check_lc_future_ray(&input, &certificate, &mut wide()?)?
            .proposal(&cas, &mut wide()?)?,
        &proposal
    );
    Ok(())
}
#[test]
fn lc_future_ray_non_equivalent_refusal() -> TestResult {
    let mut cas = Cas::new(Limits::blind_cubic())?;
    let input = gauge_request(&cas, true)?;
    let fault = cas
        .construct_lc_future_ray(&input, &mut wide()?)
        .expect_err("named future-ray refusal");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedCertificate)
    );
    assert_eq!(fault.location, UNION_AT);
    Ok(())
}

fn clock_phase() -> Result<ClockPhaseRequest, Fault> {
    use crate::enclosure::lc_affine_fibre::{
        ClockPhaseObservation, ClockPhaseSourceKind as Kind, ClockPhaseSourceSlot,
    };
    let b = &mut wide()?;
    let first_observation = 0;
    let rate = [q(1, b)?, q(2, b)?];
    let initial = [q(0, b)?, q(1, b)?];
    let clock = [q(0, b)?, q(1, b)?];
    let offset = [q(0, b)?, q(1, b)?];
    let time = [
        Rational::from_ratio(5, 4, b)?,
        Rational::from_ratio(3, 2, b)?,
    ];
    let measured = [q(0, b)?, q(4, b)?];
    let physical = [q(1, b)?, q(4, b)?];
    let kinds = [
        Kind::Rate,
        Kind::Initial,
        Kind::Clock,
        Kind::Offset,
        Kind::Time(first_observation),
        Kind::Measured(first_observation),
        Kind::Physical(first_observation),
    ];
    Ok(ClockPhaseRequest {
        rate: rate.clone(),
        rate_declarations: vec![rate],
        initial: initial.clone(),
        initial_declarations: vec![initial],
        clock: clock.clone(),
        clock_declarations: vec![clock],
        offset: offset.clone(),
        offset_declarations: vec![offset],
        observations: vec![ClockPhaseObservation {
            reported: q(1, b)?,
            time_declarations: vec![time],
            measured_declarations: vec![measured],
            physical_declarations: vec![physical],
            lift: 0,
            lift_domain: [0, 0],
        }],
        epoch: "joint epoch".into(),
        source_order: kinds
            .map(|kind| ClockPhaseSourceSlot {
                kind,
                declaration: 0,
            })
            .to_vec(),
        source_versions: vec![1; 7],
        pi: pi(b)?,
        revision: 1,
    })
}
fn plain_at_three(proposal: &FutureRayProposal) -> TestResult {
    // Independent Fraction projection of the complete original vertices.
    let expected = [[1, 0], [1, 1], [2, 0], [2, 1]];
    assert_eq!(proposal.vertices.len(), expected.len());
    for (vertex, pair) in proposal.vertices.iter().zip(expected) {
        assert_eq!(
            vertex.point[0],
            PiAffine::rational(q(pair[0], &mut wide()?)?)
        );
        assert_eq!(
            vertex.point[1],
            PiAffine::rational(q(pair[1], &mut wide()?)?)
        );
    }
    assert_eq!(proposal.hull, [0, 2, 3, 1]);
    assert_eq!((proposal.low, proposal.high), (0, 3));
    for (row, expected) in
        proposal
            .comparisons
            .iter()
            .zip([[0, 0, 1, 4], [0, 1, 1, 3], [1, 3, 0, 1], [1, 4, 0, 0]])
    {
        for (value, number) in row.iter().zip(expected) {
            assert_eq!(*value, PiAffine::rational(q(number, &mut wide()?)?));
        }
    }
    Ok(())
}
#[test]
fn lc_future_ray_plain_clock_phase() -> TestResult {
    let mut cas = Cas::new(Limits::blind_cubic())?;
    let original = clock_phase()?;
    let certificate = cas
        .construct_lc_affine_clock_phase(&original, &mut wide()?)?
        .certificate(&cas, &mut wide()?)?;
    let mut input = FutureRayRequest {
        form: FutureRayForm::Plain,
        reference: q(3, &mut wide()?)?,
        safe_reference: PiAffine::rational(q(3, &mut wide()?)?),
        narrowed: false,
        epoch: original.epoch.clone(),
        branches: vec![FutureRayBranch {
            id: 0,
            shift: 0,
            source: FutureRaySource::ClockPhase(original, certificate),
        }],
    };
    let receipt = cas.construct_lc_future_ray(&input, &mut wide()?)?;
    plain_at_three(receipt.proposal(&cas, &mut wide()?)?)?;
    let proposal = receipt.proposal(&cas, &mut wide()?)?.clone();
    let (_, points) = service::sources(&mut cas, &input, &mut wide()?)?;
    input.reference = q(2, &mut wide()?)?;
    input.safe_reference = PiAffine::rational(q(2, &mut wide()?)?);
    let check_fault = check::verify(&input, &points, &proposal, &mut wide()?)
        .expect_err("checker R at clock-phase window");
    assert_eq!(
        (check_fault.kind, check_fault.location),
        (
            FaultKind::Invalid(InvalidCode::PendingPremise),
            REFERENCE_AT
        )
    );
    let fault = cas
        .construct_lc_future_ray(&input, &mut wide()?)
        .expect_err("R at window");
    assert_eq!(
        (fault.kind, fault.location),
        (
            FaultKind::Invalid(InvalidCode::PendingPremise),
            REFERENCE_AT
        )
    );
    Ok(())
}
#[test]
fn lc_future_ray_plain_timing_clock_phase() -> TestResult {
    let mut cas = Cas::new(Limits::blind_cubic())?;
    let original = crate::enclosure::lc_affine_fibre::timing_clock_phase_test_request()?;
    let certificate = cas
        .construct_lc_affine_timing_clock_phase(&original, &mut wide()?)?
        .certificate(&cas, &mut wide()?)?;
    let input = FutureRayRequest {
        form: FutureRayForm::Plain,
        reference: q(3, &mut wide()?)?,
        safe_reference: PiAffine::rational(q(3, &mut wide()?)?),
        narrowed: false,
        epoch: original.epoch.clone(),
        branches: vec![FutureRayBranch {
            id: 0,
            shift: 0,
            source: FutureRaySource::TimingClockPhase(original, certificate),
        }],
    };
    let receipt = cas.construct_lc_future_ray(&input, &mut wide()?)?;
    plain_at_three(receipt.proposal(&cas, &mut wide()?)?)?;
    assert!(matches!(
        receipt.interpolate(&cas, &fraction("1/2", &mut wide()?)?, &mut wide()?)?,
        FutureRayWitness::TimingClockPhase(_)
    ));
    let proposal = receipt.proposal(&cas, &mut wide()?)?.clone();
    let (_, points) = service::sources(&mut cas, &input, &mut wide()?)?;
    let mut shortened = input.clone();
    shortened.reference = fraction("17/8", &mut wide()?)?;
    shortened.safe_reference = PiAffine::rational(shortened.reference.clone());
    let fault = check::verify(&shortened, &points, &proposal, &mut wide()?)
        .expect_err("checker full clock and error window");
    assert_eq!(
        (fault.kind, fault.location),
        (
            FaultKind::Invalid(InvalidCode::PendingPremise),
            REFERENCE_AT
        )
    );
    let fault = cas
        .construct_lc_future_ray(&shortened, &mut wide()?)
        .expect_err("full clock and error window");
    assert_eq!(
        (fault.kind, fault.location),
        (
            FaultKind::Invalid(InvalidCode::PendingPremise),
            REFERENCE_AT
        )
    );
    Ok(())
}
#[test]
fn lc_future_ray_narrow() -> TestResult {
    let mut cas = Cas::new(Limits::blind_cubic())?;
    let original = clock_phase()?;
    let certificate = cas
        .construct_lc_affine_clock_phase(&original, &mut wide()?)?
        .certificate(&cas, &mut wide()?)?;
    let input = FutureRayRequest {
        form: FutureRayForm::Narrow,
        reference: q(3, &mut wide()?)?,
        safe_reference: PiAffine::rational(q(3, &mut wide()?)?),
        narrowed: false,
        epoch: original.epoch.clone(),
        branches: vec![FutureRayBranch {
            id: 0,
            shift: 0,
            source: FutureRaySource::ClockPhase(original, certificate),
        }],
    };
    let receipt = cas.construct_lc_future_ray(&input, &mut wide()?)?;
    let proposal = receipt.proposal(&cas, &mut wide()?)?;
    // Project the independent 16-vertex Fraction oracle for the original
    // 14-row four-dimensional fibre by beta=gamma+s, then deduplicate exactly.
    let polygon = [
        ["1", "1/4"],
        ["1", "1/2"],
        ["1", "5/4"],
        ["1", "3/2"],
        ["2", "1/2"],
        ["2", "1"],
        ["2", "3/2"],
        ["2", "2"],
    ];
    assert_eq!(proposal.vertices.len(), polygon.len());
    for (vertex, pair) in proposal.vertices.iter().zip(polygon) {
        for (value, text) in vertex.point.iter().zip(pair) {
            assert_eq!(*value, PiAffine::rational(fraction(text, &mut wide()?)?));
        }
    }
    assert_eq!(proposal.hull, [0, 4, 7, 3]);
    assert_eq!((proposal.low, proposal.high), (0, 7));
    let oracle = [
        ["0", "0", "1", "19/4"],
        ["0", "1/4", "1", "9/2"],
        ["0", "1", "1", "15/4"],
        ["0", "5/4", "1", "7/2"],
        ["1", "13/4", "0", "3/2"],
        ["1", "15/4", "0", "1"],
        ["1", "17/4", "0", "1/2"],
        ["1", "19/4", "0", "0"],
    ];
    for (row, expected) in proposal.comparisons.iter().zip(oracle) {
        for (value, text) in row.iter().zip(expected) {
            assert_eq!(*value, PiAffine::rational(fraction(text, &mut wide()?)?));
        }
    }
    match receipt.interpolate(
        &cas,
        &Rational::from_ratio(1, 2, &mut wide()?)?,
        &mut wide()?,
    )? {
        FutureRayWitness::ClockPhase(_) => (),
        _ => panic!("wrong original witness"),
    }
    let certificate = receipt.certificate(&cas, &mut wide()?)?;
    let replay = cas.check_lc_future_ray(&input, &certificate, &mut wide()?)?;
    assert_eq!(replay.proposal(&cas, &mut wide()?)?, proposal);
    Ok(())
}

fn gauge_request(cas: &Cas, altered: bool) -> Result<FutureRayRequest, Fault> {
    let mut first = timing()?;
    first.initial = [q(0, &mut wide()?)?, q(10, &mut wide()?)?];
    first.initial_declarations = vec![first.initial.clone()];
    first.observations[0].lift_domain = [0, 1];
    let mut second = first.clone();
    second.observations[0].lift = 1;
    if altered {
        second.initial[1] = q(7, &mut wide()?)?;
        second.initial_declarations = vec![second.initial.clone()];
    }
    let first_cert = cas
        .construct_lc_affine_timing(&first, &mut wide()?)?
        .certificate(cas, &mut wide()?)?;
    let second_cert = cas
        .construct_lc_affine_timing(&second, &mut wide()?)?
        .certificate(cas, &mut wide()?)?;
    Ok(FutureRayRequest {
        form: FutureRayForm::Plain,
        reference: q(1, &mut wide()?)?,
        safe_reference: PiAffine::rational(q(1, &mut wide()?)?),
        narrowed: false,
        epoch: first.epoch.clone(),
        branches: vec![
            FutureRayBranch {
                id: 0,
                shift: 0,
                source: FutureRaySource::Timing(first, first_cert),
            },
            FutureRayBranch {
                id: 1,
                shift: 1,
                source: FutureRaySource::Timing(second, second_cert),
            },
        ],
    })
}

#[test]
fn lc_future_ray_gauge_union() -> TestResult {
    let mut cas = Cas::new(Limits::blind_cubic())?;
    let input = gauge_request(&cas, false)?;
    let receipt = cas.construct_lc_future_ray(&input, &mut wide()?)?;
    assert_eq!(receipt.proposal(&cas, &mut wide()?)?.vertices.len(), 4);
    let certificate = receipt.certificate(&cas, &mut wide()?)?;
    assert_eq!(
        cas.check_lc_future_ray(&input, &certificate, &mut wide()?)?
            .proposal(&cas, &mut wide()?)?,
        receipt.proposal(&cas, &mut wide()?)?
    );
    Ok(())
}
#[test]
fn lc_future_ray_incomplete_union() -> TestResult {
    let mut cas = Cas::new(Limits::blind_cubic())?;
    let mut input = gauge_request(&cas, false)?;
    input.branches.pop();
    let fault = cas
        .construct_lc_future_ray(&input, &mut wide()?)
        .expect_err("missing full lift branch");
    assert_eq!(
        (fault.kind, fault.location),
        (
            FaultKind::Invalid(InvalidCode::UnsupportedCertificate),
            UNION_AT
        )
    );
    let full = gauge_request(&cas, false)?;
    let receipt = cas.construct_lc_future_ray(&full, &mut wide()?)?;
    let proposal = receipt.proposal(&cas, &mut wide()?)?.clone();
    let (_, mut points) = service::sources(&mut cas, &full, &mut wide()?)?;
    points.pop();
    let fault = check::verify(&input, &points, &proposal, &mut wide()?)
        .expect_err("checker missing complete lift branch");
    assert_eq!(
        (fault.kind, fault.location),
        (
            FaultKind::Invalid(InvalidCode::UnsupportedCertificate),
            UNION_AT
        )
    );
    Ok(())
}
#[test]
fn lc_future_ray_gauge_map_tamper() -> TestResult {
    let mut cas = Cas::new(Limits::blind_cubic())?;
    let input = gauge_request(&cas, false)?;
    let receipt = cas.construct_lc_future_ray(&input, &mut wide()?)?;
    let proposal = receipt.proposal(&cas, &mut wide()?)?.clone();
    let (_, mut points) = service::sources(&mut cas, &input, &mut wide()?)?;
    points[1][0][1].pi = points[1][0][1].pi.add(&q(1, &mut wide()?)?, &mut wide()?)?;
    let fault = check::verify(&input, &points, &proposal, &mut wide()?)
        .expect_err("broken original gauge map");
    assert_eq!(
        (fault.kind, fault.location),
        (
            FaultKind::Invalid(InvalidCode::UnsupportedCertificate),
            UNION_AT
        )
    );
    Ok(())
}

#[test]
fn lc_future_ray_checker_tamper() -> TestResult {
    let mut cas = Cas::new(Limits::blind_cubic())?;
    let input = request(&cas)?;
    let receipt = cas.construct_lc_future_ray(&input, &mut wide()?)?;
    let valid = receipt.proposal(&cas, &mut wide()?)?.clone();
    let (_, points) = service::sources(&mut cas, &input, &mut wide()?)?;
    for kind in 0..4 {
        let mut changed = valid.clone();
        match kind {
            0 => changed.hull[0] = 2,
            1 => changed.vertices[0].point[1].base = q(2, &mut wide()?)?,
            2 => changed.comparisons[0][0].base = q(1, &mut wide()?)?,
            _ => changed.low = 99,
        }
        let fault =
            check::verify(&input, &points, &changed, &mut wide()?).expect_err("tampered proposal");
        assert_eq!(
            (fault.kind, fault.location),
            (FaultKind::Invalid(InvalidCode::UnsupportedCertificate), AT)
        );
    }
    Ok(())
}

#[test]
fn lc_future_ray_extremum_sides() -> TestResult {
    let cas = Cas::new(Limits::blind_cubic())?;
    let input = request(&cas)?;
    let points: Vec<claim::SourcePoints> = vec![
        [[1, 0], [1, 1], [2, 0], [2, 1]]
            .into_iter()
            .map(|pair| {
                pair.into_iter()
                    .map(|value| {
                        PiAffine::rational(q(value, &mut wide().expect("budget")).expect("integer"))
                    })
                    .collect()
            })
            .collect(),
    ];
    let valid = claim::propose(&input, &points, &mut wide()?)?;
    for (low, high, vertex, side) in [(2, 3, 0, 0), (1, 3, 0, 1), (0, 1, 2, 2), (0, 2, 3, 3)] {
        let mut changed = valid.clone();
        changed.low = low;
        changed.high = high;
        for (index, row) in changed.comparisons.iter_mut().enumerate() {
            *row = claim::differences(
                &changed.vertices[index].point,
                &changed.vertices[low as usize].point,
                &changed.vertices[high as usize].point,
                &input.reference,
                &mut wide()?,
            )?;
        }
        let forbidden = &changed.comparisons[vertex][side];
        assert!(forbidden.base.is_negative());
        assert!(forbidden.pi.is_zero());
        let fault = check::verify(&input, &points, &changed, &mut wide()?)
            .expect_err("named future-ray refusal");
        assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::PendingPremise));
        assert_eq!(fault.location, EXTREMUM_AT);
    }
    Ok(())
}
#[test]
fn lc_future_ray_topology_undecidable() -> TestResult {
    let cas = Cas::new(Limits::blind_cubic())?;
    let input = request(&cas)?;
    let zero = PiAffine::rational(Rational::zero());
    let one = PiAffine::rational(q(1, &mut wide()?)?);
    let uncertain = PiAffine {
        base: Rational::from_ratio(-22, 7, &mut wide()?)?,
        pi: q(1, &mut wide()?)?,
    };
    let points = vec![vec![vec![one.clone(), zero], vec![one, uncertain]]];
    let fault = claim::propose(&input, &points, &mut wide()?)
        .expect_err("undecidable projected deduplication");
    assert_eq!(
        (fault.kind, fault.location),
        (FaultKind::Invalid(InvalidCode::PendingPremise), TOPOLOGY_AT)
    );
    Ok(())
}

#[test]
fn lc_future_ray_reference_sides() -> TestResult {
    let mut cas = Cas::new(Limits::blind_cubic())?;
    let mut input = request(&cas)?;
    input.safe_reference = PiAffine::rational(q(2, &mut wide()?)?);
    let fault = cas
        .construct_lc_future_ray(&input, &mut wide()?)
        .expect_err("named future-ray refusal");
    assert_eq!(
        (fault.kind, fault.location),
        (
            FaultKind::Invalid(InvalidCode::PendingPremise),
            REFERENCE_AT
        )
    );
    input.reference = q(3, &mut wide()?)?;
    let fault = cas
        .construct_lc_future_ray(&input, &mut wide()?)
        .expect_err("named future-ray refusal");
    assert_eq!(
        (fault.kind, fault.location),
        (
            FaultKind::Invalid(InvalidCode::PendingPremise),
            REFERENCE_AT
        )
    );
    input.narrowed = true;
    assert!(cas.construct_lc_future_ray(&input, &mut wide()?).is_ok());
    input.safe_reference = PiAffine {
        base: Rational::zero(),
        pi: q(1, &mut wide()?)?,
    };
    let fault = cas
        .construct_lc_future_ray(&input, &mut wide()?)
        .expect_err("named future-ray refusal");
    assert_eq!(
        (fault.kind, fault.location),
        (
            FaultKind::Invalid(InvalidCode::PendingPremise),
            REFERENCE_AT
        )
    );
    input.reference = q(4, &mut wide()?)?;
    assert!(cas.construct_lc_future_ray(&input, &mut wide()?).is_ok());
    input.reference = Rational::from_ratio(22, 7, &mut wide()?)?;
    let fault = cas
        .construct_lc_future_ray(&input, &mut wide()?)
        .expect_err("undecidable pi reference");
    assert_eq!(
        (fault.kind, fault.location),
        (
            FaultKind::Invalid(InvalidCode::PendingPremise),
            REFERENCE_AT
        )
    );
    Ok(())
}

#[test]
fn lc_future_ray_apply() -> TestResult {
    let mut cas = Cas::new(Limits::blind_cubic())?;
    let input = request(&cas)?;
    let receipt = cas.construct_lc_future_ray(&input, &mut wide()?)?;
    assert!(matches!(
        receipt.apply(&mut cas, &input, &mut wide()?)?,
        std::borrow::Cow::Borrowed(_)
    ));
    let mut changed = input.clone();
    changed.narrowed = true;
    let applied = receipt.apply(&mut cas, &changed, &mut wide()?)?;
    assert!(matches!(applied, std::borrow::Cow::Owned(_)));
    assert_eq!(
        receipt.proposal(&cas, &mut wide()?)?.vertices,
        applied.proposal(&cas, &mut wide()?)?.vertices
    );
    Ok(())
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "end-to-end cumulative pass measurements stay together"
)]
fn lc_future_ray_work_passes() -> TestResult {
    let mut cas = Cas::new(Limits::blind_cubic())?;
    let input = request(&cas)?;
    let mut b = wide()?;
    let (sources, points) = service::sources(&mut cas, &input, &mut b)?;
    let source_work = b.usage().work;
    let proposal = claim::propose(&input, &points, &mut b)?;
    let proposal_work = b.usage().work - source_work;
    check::verify(&input, &points, &proposal, &mut b)?;
    let verify_work = b.usage().work - source_work - proposal_work;
    let scope = wire::scope(&input, &mut b)?;
    let scope_work = b.usage().work - source_work - proposal_work - verify_work;
    let bytes = wire::encode(&scope, &proposal, &mut b)?;
    let encode_work = b.usage().work - source_work - proposal_work - verify_work - scope_work;
    assert_eq!(scope.len(), 806);
    assert_eq!(bytes.len(), 1_382);
    assert_eq!(input.branches.len(), 1);
    assert_eq!(points.iter().map(Vec::len).sum::<usize>(), 4);
    assert_eq!(points.iter().flatten().map(Vec::len).sum::<usize>(), 8);
    assert_eq!(
        (
            proposal.vertices.len(),
            proposal.hull.len(),
            proposal.comparisons.len()
        ),
        (4, 4, 4)
    );
    let inventory = service::storage_inventory(&input, &proposal, &sources, &points, bytes.len());
    assert_eq!(
        inventory,
        service::StorageInventory {
            shell: 432,
            wire: 1_382,
            original: 1_921,
            epoch: 12,
            source_receipts: 1_200,
            point_vectors: 120,
            branches: 1_128,
            projected: 1_184,
            comparisons: 2_304,
            hull: 16,
            coordinates: 1_152,
            limbs: 128,
        }
    );
    let before_retain = b.usage().work;
    let _receipt = service::retain(&cas, &input, proposal, sources, points, &bytes, &mut b)?;
    let retain_work = b.usage().work - before_retain;
    let construction = b.usage().work;
    assert_eq!(
        [
            source_work,
            proposal_work,
            verify_work,
            scope_work,
            encode_work,
            retain_work
        ],
        [1_262_720, 99_730, 80_611, 1_625, 2_918, 10_979]
    );
    assert_eq!(construction, 1_458_583);
    assert_eq!(retain_work, 10_979);
    let mut full = wide()?;
    let actual = cas.construct_lc_future_ray(&input, &mut full)?;
    assert_eq!(full.usage().work, construction);
    let mut rb = wide()?;
    let rscope = wire::scope(&input, &mut rb)?;
    let receive_scope = rb.usage().work;
    let proposed = wire::decode(&bytes, &rscope, &mut rb)?;
    let decode_work = rb.usage().work - receive_scope;
    let (rreceipts, rpoints) = service::sources(&mut cas, &input, &mut rb)?;
    let receive_sources = rb.usage().work - receive_scope - decode_work;
    check::verify(&input, &rpoints, &proposed, &mut rb)?;
    let receive_verify = rb.usage().work - receive_scope - decode_work - receive_sources;
    let before_storage = rb.usage().work;
    assert_eq!(
        service::storage_inventory(&input, &proposed, &rreceipts, &rpoints, bytes.len()),
        inventory
    );
    let _retained = service::retain(&cas, &input, proposed, rreceipts, rpoints, &bytes, &mut rb)?;
    let receive_retain = rb.usage().work - before_storage;
    let replay = rb.usage().work;
    assert_eq!(
        [
            receive_scope,
            decode_work,
            receive_sources,
            receive_verify,
            receive_retain
        ],
        [1_625, 12_694, 1_262_720, 80_611, 10_979]
    );
    assert_eq!(replay, 1_368_629);
    let mut full_replay = wide()?;
    let certificate = actual.certificate(&cas, &mut wide()?)?;
    let _rechecked = cas.check_lc_future_ray(&input, &certificate, &mut full_replay)?;
    assert_eq!(full_replay.usage().work, replay);
    assert_eq!(
        [
            receive_scope,
            receive_sources,
            receive_verify,
            receive_retain
        ],
        [scope_work, source_work, verify_work, retain_work]
    );
    let mut unchanged = wide()?;
    assert!(matches!(
        actual.apply(&mut cas, &input, &mut unchanged)?,
        std::borrow::Cow::Borrowed(_)
    ));
    let mut changed_input = input.clone();
    changed_input.reference = q(2, &mut wide()?)?;
    changed_input.narrowed = true;
    let mut changed = wide()?;
    assert!(matches!(
        actual.apply(&mut cas, &changed_input, &mut changed)?,
        std::borrow::Cow::Owned(_)
    ));
    assert_eq!(unchanged.usage().work, 84_623);
    assert_eq!(changed.usage().work, 1_458_459);
    assert_eq!(bytes.len(), 1_382);
    let exact_limits = Limits {
        work: construction,
        ..Limits::blind_cubic()
    };
    assert!(
        cas.construct_lc_future_ray(&input, &mut Budget::new(exact_limits)?)
            .is_ok()
    );
    let short_limits = Limits {
        work: construction - 1,
        ..Limits::blind_cubic()
    };
    let fault = cas
        .construct_lc_future_ray(&input, &mut Budget::new(short_limits)?)
        .expect_err("N-1 Work refusal");
    assert!(matches!(fault.kind, FaultKind::Exhausted {
        reason: crate::error::Exhaustion::Limit,
        budget: crate::budget::BudgetKind::Work,
        limit,
        needed,
    } if limit == construction - 1 && needed == construction));
    assert_eq!(fault.location, AT);
    assert!(
        cas.construct_lc_future_ray(&input, &mut Budget::new(Limits::default())?)
            .is_ok()
    );
    Ok(())
}

#[test]
fn lc_future_ray_scope_refusal() -> TestResult {
    let mut cas = Cas::new(Limits::blind_cubic())?;
    let input = request(&cas)?;
    let receipt = cas.construct_lc_future_ray(&input, &mut wide()?)?;
    let certificate = receipt.certificate(&cas, &mut wide()?)?;
    let mut changed = input.clone();
    changed.narrowed = true;
    let fault = cas
        .check_lc_future_ray(&changed, &certificate, &mut wide()?)
        .expect_err("changed narrowed scope");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedCertificate)
    );
    assert_eq!(fault.location, AT);
    Ok(())
}
