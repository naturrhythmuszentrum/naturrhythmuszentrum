// SPDX-License-Identifier: AGPL-3.0-only
//! Complete timing, clock and phase branch controls.

use super::*;
use crate::budget::{Budget, BudgetKind, Limits};
use crate::{Cas, Fault, FaultKind, InvalidCode};
use std::mem::size_of;
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn limits() -> Limits {
    Limits {
        work: 20_000_000_000,
        replay_work: 20_000_000_000,
        arithmetic_steps: 20_000_000_000,
        witness_bytes: 32_000_000,
        output_bytes: 32_000_000,
        staged_payload_bytes: 64_000_000,
        matrix_entries: 1_000_000,
        ..Limits::blind_cubic()
    }
}
fn wide() -> Result<Budget, Fault> {
    Budget::new(limits())
}
fn q(n: i64, d: u64, b: &mut Budget) -> Result<Rational, Fault> {
    Rational::from_ratio(n, d, b)
}
fn integer(n: i64, b: &mut Budget) -> Result<Rational, Fault> {
    q(n, 1, b)
}
fn pi(b: &mut Budget) -> Result<PiBoundCertificate, Fault> {
    Ok(PiBoundCertificate {
        version: 1,
        terms: [2, 1],
        sums: [q(74, 375, b)?, q(1, 239, b)?],
        lower: q(128, 41, b)?,
        upper: q(640, 203, b)?,
    })
}
fn request() -> Result<TimingClockPhaseRequest, Fault> {
    let b = &mut wide()?;
    let rate = [integer(1, b)?, integer(2, b)?];
    let initial = [integer(0, b)?, integer(1, b)?];
    let clock = [integer(0, b)?, integer(1, b)?];
    let offset = [integer(0, b)?, integer(1, b)?];
    let error = [integer(0, b)?, q(1, 4, b)?];
    let time = [q(5, 4, b)?, q(7, 4, b)?];
    let measured = [integer(0, b)?, integer(5, b)?];
    let physical = [integer(1, b)?, integer(5, b)?];
    let kinds = [
        TimingClockPhaseSourceKind::Rate,
        TimingClockPhaseSourceKind::Initial,
        TimingClockPhaseSourceKind::Clock,
        TimingClockPhaseSourceKind::Offset,
        TimingClockPhaseSourceKind::Error(0),
        TimingClockPhaseSourceKind::Time(0),
        TimingClockPhaseSourceKind::Measured(0),
        TimingClockPhaseSourceKind::Physical(0),
    ];
    Ok(TimingClockPhaseRequest {
        rate: rate.clone(),
        rate_declarations: vec![rate],
        initial: initial.clone(),
        initial_declarations: vec![initial],
        clock: clock.clone(),
        clock_declarations: vec![clock],
        offset: offset.clone(),
        offset_declarations: vec![offset],
        observations: vec![TimingClockPhaseObservation {
            reported: integer(1, b)?,
            error: error.clone(),
            error_declarations: vec![error],
            time_declarations: vec![time],
            measured_declarations: vec![measured],
            physical_declarations: vec![physical],
            lift: 0,
            lift_domain: [0, 0],
        }],
        epoch: "joint epoch".into(),
        source_order: kinds
            .map(|kind| TimingClockPhaseSourceSlot {
                kind,
                declaration: 0,
            })
            .to_vec(),
        source_versions: vec![1; 8],
        pi: pi(b)?,
        revision: 1,
    })
}
fn fraction(text: &str, b: &mut Budget) -> Result<Rational, Box<dyn std::error::Error>> {
    let (n, d) = text.split_once('/').unwrap_or((text, "1"));
    Ok(q(n.parse()?, d.parse()?, b)?)
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "full independent rational vertex and edge oracle is kept together"
)]
fn lc_affine_timing_clock_phase_expected_claim() -> TestResult {
    let original = request()?;
    let cas = Cas::new(limits())?;
    let receipt = cas.construct_lc_affine_timing_clock_phase(&original, &mut wide()?)?;
    assert_eq!(
        receipt.formal_schema().label,
        "thm:cas:lc-affine-fibre-timing-clock-phase"
    );
    let topology = receipt.topology(&cas, &mut wide()?)?;
    assert_eq!(topology.dimension, Some(5));
    // Independent Fraction Gaussian elimination over all 4368 five-row subsets
    // of the sixteen exact rows, followed by all-row feasibility and normal rank.
    let vertices: [([&str; 5], &[u32]); 32] = [
        (["1", "-1", "1", "0", "1/4"], &[0, 2, 4, 7, 9, 10]),
        (["1", "-3/4", "1", "1/4", "0"], &[0, 2, 7, 8, 10]),
        (["1", "-1/2", "1", "1/2", "1/4"], &[0, 2, 7, 9, 11]),
        (["1", "-1/4", "1", "3/4", "0"], &[0, 2, 7, 8, 11]),
        (["1", "0", "0", "0", "1/4"], &[0, 2, 4, 6, 9, 10]),
        (["1", "0", "1", "0", "1/4"], &[0, 3, 4, 7, 9, 10]),
        (["1", "1/4", "0", "1/4", "0"], &[0, 2, 6, 8, 10]),
        (["1", "1/4", "1", "1/4", "0"], &[0, 3, 7, 8, 10]),
        (["1", "1/2", "0", "1/2", "1/4"], &[0, 2, 6, 9, 11]),
        (["1", "1/2", "1", "1/2", "1/4"], &[0, 3, 7, 9, 11]),
        (["1", "3/4", "0", "3/4", "0"], &[0, 2, 6, 8, 11]),
        (["1", "3/4", "1", "3/4", "0"], &[0, 3, 7, 8, 11]),
        (["1", "1", "0", "0", "1/4"], &[0, 3, 4, 6, 9, 10]),
        (["1", "5/4", "0", "1/4", "0"], &[0, 3, 6, 8, 10]),
        (["1", "3/2", "0", "1/2", "1/4"], &[0, 3, 6, 9, 11]),
        (["1", "7/4", "0", "3/4", "0"], &[0, 3, 6, 8, 11]),
        (["2", "-1", "1", "0", "1/2"], &[1, 2, 4, 7, 9, 10]),
        (["2", "-1/2", "1", "1/2", "0"], &[1, 2, 7, 8, 10]),
        (["2", "0", "0", "0", "1/2"], &[1, 2, 4, 6, 9, 10]),
        (["2", "0", "1", "0", "1/2"], &[1, 3, 4, 7, 9, 10]),
        (["2", "0", "1", "1", "1/2"], &[1, 2, 7, 9, 11]),
        (["2", "1/2", "0", "1/2", "0"], &[1, 2, 6, 8, 10]),
        (["2", "1/2", "1", "1/2", "0"], &[1, 3, 7, 8, 10]),
        (["2", "1/2", "1", "3/2", "0"], &[1, 2, 7, 8, 11]),
        (["2", "1", "0", "0", "1/2"], &[1, 3, 4, 6, 9, 10]),
        (["2", "1", "0", "1", "1/2"], &[1, 2, 6, 9, 11]),
        (["2", "1", "1", "1", "1/2"], &[1, 3, 7, 9, 11]),
        (["2", "3/2", "0", "1/2", "0"], &[1, 3, 6, 8, 10]),
        (["2", "3/2", "0", "3/2", "0"], &[1, 2, 6, 8, 11]),
        (["2", "3/2", "1", "3/2", "0"], &[1, 3, 7, 8, 11]),
        (["2", "2", "0", "1", "1/2"], &[1, 3, 6, 9, 11]),
        (["2", "5/2", "0", "3/2", "0"], &[1, 3, 6, 8, 11]),
    ];
    assert_eq!(topology.vertices.len(), vertices.len());
    for (actual, (coords, active)) in topology.vertices.iter().zip(vertices) {
        assert_eq!(actual.active, active);
        for (value, expected) in actual.point.iter().zip(coords) {
            assert_eq!(
                *value,
                PiAffine::rational(fraction(expected, &mut wide()?)?)
            );
        }
    }
    let edges: [([u32; 2], &[u32]); 80] = [
        ([0, 1], &[0, 2, 7, 10]),
        ([0, 2], &[0, 2, 7, 9]),
        ([0, 4], &[0, 2, 4, 9, 10]),
        ([0, 5], &[0, 4, 7, 9, 10]),
        ([0, 16], &[2, 4, 7, 9, 10]),
        ([1, 3], &[0, 2, 7, 8]),
        ([1, 6], &[0, 2, 8, 10]),
        ([1, 7], &[0, 7, 8, 10]),
        ([1, 17], &[2, 7, 8, 10]),
        ([2, 3], &[0, 2, 7, 11]),
        ([2, 8], &[0, 2, 9, 11]),
        ([2, 9], &[0, 7, 9, 11]),
        ([2, 20], &[2, 7, 9, 11]),
        ([3, 10], &[0, 2, 8, 11]),
        ([3, 11], &[0, 7, 8, 11]),
        ([3, 23], &[2, 7, 8, 11]),
        ([4, 6], &[0, 2, 6, 10]),
        ([4, 8], &[0, 2, 6, 9]),
        ([4, 12], &[0, 4, 6, 9, 10]),
        ([4, 18], &[2, 4, 6, 9, 10]),
        ([5, 7], &[0, 3, 7, 10]),
        ([5, 9], &[0, 3, 7, 9]),
        ([5, 12], &[0, 3, 4, 9, 10]),
        ([5, 19], &[3, 4, 7, 9, 10]),
        ([6, 10], &[0, 2, 6, 8]),
        ([6, 13], &[0, 6, 8, 10]),
        ([6, 21], &[2, 6, 8, 10]),
        ([7, 11], &[0, 3, 7, 8]),
        ([7, 13], &[0, 3, 8, 10]),
        ([7, 22], &[3, 7, 8, 10]),
        ([8, 10], &[0, 2, 6, 11]),
        ([8, 14], &[0, 6, 9, 11]),
        ([8, 25], &[2, 6, 9, 11]),
        ([9, 11], &[0, 3, 7, 11]),
        ([9, 14], &[0, 3, 9, 11]),
        ([9, 26], &[3, 7, 9, 11]),
        ([10, 15], &[0, 6, 8, 11]),
        ([10, 28], &[2, 6, 8, 11]),
        ([11, 15], &[0, 3, 8, 11]),
        ([11, 29], &[3, 7, 8, 11]),
        ([12, 13], &[0, 3, 6, 10]),
        ([12, 14], &[0, 3, 6, 9]),
        ([12, 24], &[3, 4, 6, 9, 10]),
        ([13, 15], &[0, 3, 6, 8]),
        ([13, 27], &[3, 6, 8, 10]),
        ([14, 15], &[0, 3, 6, 11]),
        ([14, 30], &[3, 6, 9, 11]),
        ([15, 31], &[3, 6, 8, 11]),
        ([16, 17], &[1, 2, 7, 10]),
        ([16, 18], &[1, 2, 4, 9, 10]),
        ([16, 19], &[1, 4, 7, 9, 10]),
        ([16, 20], &[1, 2, 7, 9]),
        ([17, 21], &[1, 2, 8, 10]),
        ([17, 22], &[1, 7, 8, 10]),
        ([17, 23], &[1, 2, 7, 8]),
        ([18, 21], &[1, 2, 6, 10]),
        ([18, 24], &[1, 4, 6, 9, 10]),
        ([18, 25], &[1, 2, 6, 9]),
        ([19, 22], &[1, 3, 7, 10]),
        ([19, 24], &[1, 3, 4, 9, 10]),
        ([19, 26], &[1, 3, 7, 9]),
        ([20, 23], &[1, 2, 7, 11]),
        ([20, 25], &[1, 2, 9, 11]),
        ([20, 26], &[1, 7, 9, 11]),
        ([21, 27], &[1, 6, 8, 10]),
        ([21, 28], &[1, 2, 6, 8]),
        ([22, 27], &[1, 3, 8, 10]),
        ([22, 29], &[1, 3, 7, 8]),
        ([23, 28], &[1, 2, 8, 11]),
        ([23, 29], &[1, 7, 8, 11]),
        ([24, 27], &[1, 3, 6, 10]),
        ([24, 30], &[1, 3, 6, 9]),
        ([25, 28], &[1, 2, 6, 11]),
        ([25, 30], &[1, 6, 9, 11]),
        ([26, 29], &[1, 3, 7, 11]),
        ([26, 30], &[1, 3, 9, 11]),
        ([27, 31], &[1, 3, 6, 8]),
        ([28, 31], &[1, 6, 8, 11]),
        ([29, 31], &[1, 3, 8, 11]),
        ([30, 31], &[1, 3, 6, 11]),
    ];
    assert_eq!(topology.edges.len(), edges.len());
    for (actual, (ends, supports)) in topology.edges.iter().zip(edges) {
        assert_eq!(actual.ends, ends);
        assert_eq!(actual.supports, supports);
    }
    Ok(())
}

#[test]
fn lc_affine_timing_clock_phase_all_rows() -> TestResult {
    let original = request()?;
    let (rows, _) = claim::rows(&original, &mut wide()?)?;
    let expected: [([&str; 5], &str); 16] = [
        (["-1", "0", "0", "0", "0"], "-1"),
        (["1", "0", "0", "0", "0"], "2"),
        (["0", "-1", "-1", "1", "0"], "0"),
        (["0", "1", "1", "-1", "0"], "1"),
        (["0", "0", "0", "-1", "0"], "0"),
        (["-1", "0", "0", "1", "0"], "0"),
        (["0", "0", "-1", "0", "0"], "0"),
        (["0", "0", "1", "0", "0"], "1"),
        (["0", "0", "0", "0", "-1"], "0"),
        (["-1/4", "0", "0", "0", "1"], "0"),
        (["1/4", "0", "0", "-1", "-1"], "0"),
        (["-3/4", "0", "0", "1", "1"], "0"),
        (["-1", "-1", "0", "0", "-1"], "0"),
        (["1", "1", "0", "0", "1"], "5"),
        (["-1", "-1", "-1", "0", "-1"], "-1"),
        (["1", "1", "1", "0", "1"], "5"),
    ];
    assert_eq!(rows.len(), expected.len());
    for (row, (coefficients, bound)) in rows.iter().zip(expected) {
        for (actual, text) in row.coefficient.iter().zip(coefficients) {
            assert_eq!(*actual, fraction(text, &mut wide()?)?);
        }
        assert_eq!(
            row.bound,
            PiAffine::rational(fraction(bound, &mut wide()?)?)
        );
    }
    let cas = Cas::new(limits())?;
    let receipt = cas.construct_lc_affine_timing_clock_phase(&original, &mut wide()?)?;
    let certificate = receipt.certificate(&cas, &mut wide()?)?;
    let mut receiver = Cas::new(limits())?;
    let replay =
        receiver.check_lc_affine_timing_clock_phase(&original, &certificate, &mut wide()?)?;
    assert_eq!(
        replay.topology(&receiver, &mut wide()?)?,
        receipt.topology(&cas, &mut wide()?)?
    );
    let mut changed = original.clone();
    changed.source_versions[4] += 1;
    changed.observations[0].error[1] = integer(0, &mut wide()?)?;
    changed.observations[0].error_declarations[0][1] = changed.observations[0].error[1].clone();
    let fault = receipt
        .apply(&cas, &changed, &mut wide()?)
        .expect_err("changed w row must refuse");
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::PendingPremise));
    assert_eq!(fault.location, TOPOLOGY_AT);
    let mut changed_sign = original.clone();
    changed_sign.source_versions[6] += 1;
    changed_sign.observations[0].measured_declarations[0][0] = integer(-1, &mut wide()?)?;
    let fault = receipt
        .apply(&cas, &changed_sign, &mut wide()?)
        .expect_err("changed exact residual sign must refuse");
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::PendingPremise));
    assert_eq!(fault.location, SIGN_AT);
    Ok(())
}

#[test]
fn lc_affine_timing_clock_phase_restoration() -> TestResult {
    let original = request()?;
    let cas = Cas::new(limits())?;
    let receipt = cas.construct_lc_affine_timing_clock_phase(&original, &mut wide()?)?;
    let b = &mut wide()?;
    let point = [
        integer(2, b)?,
        integer(0, b)?,
        integer(1, b)?,
        integer(1, b)?,
        q(1, 2, b)?,
    ]
    .map(PiAffine::rational);
    let witness = receipt.restore(&cas, &original, &point, b)?;
    assert_eq!(witness.initial, PiAffine::rational(integer(0, b)?));
    assert_eq!(
        witness.clock_offset.numerator,
        PiAffine::rational(integer(1, b)?)
    );
    assert_eq!(witness.errors[0].numerator, PiAffine::rational(q(1, 2, b)?));
    assert_eq!(
        witness.clock_offset.denominator,
        PiAffine::rational(integer(2, b)?)
    );
    assert_eq!(
        witness.errors[0].denominator,
        PiAffine::rational(integer(2, b)?)
    );
    assert_eq!(
        witness.timestamps[0].denominator,
        PiAffine::rational(integer(2, b)?)
    );
    assert_eq!(
        witness.timestamps[0].numerator,
        PiAffine::rational(q(7, 2, b)?)
    );
    assert_eq!(witness.measured[0], PiAffine::rational(q(5, 2, b)?));
    assert_eq!(witness.physical[0], PiAffine::rational(q(7, 2, b)?));
    Ok(())
}

#[test]
fn lc_affine_timing_clock_phase_weak_pi() -> TestResult {
    let mut strong = request()?;
    strong.observations[0].physical_declarations[0][1] = integer(7, &mut wide()?)?;
    let cas = Cas::new(limits())?;
    cas.construct_lc_affine_timing_clock_phase(&strong, &mut wide()?)?;
    let mut weak = strong;
    weak.pi.lower = integer(3, &mut wide()?)?;
    weak.pi.upper = integer(4, &mut wide()?)?;
    let fault = cas
        .construct_lc_affine_timing_clock_phase(&weak, &mut wide()?)
        .expect_err("weak pi cannot separate preceding turn");
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::PendingPremise));
    assert_eq!(fault.location, PI_AT);
    Ok(())
}

#[test]
fn lc_affine_timing_clock_phase_same_version_content() -> TestResult {
    let original = request()?;
    let cas = Cas::new(limits())?;
    let receipt = cas.construct_lc_affine_timing_clock_phase(&original, &mut wide()?)?;
    let mut stale = original;
    stale.observations[0].error[1] = integer(0, &mut wide()?)?;
    stale.observations[0].error_declarations[0][1] = stale.observations[0].error[1].clone();
    let fault = receipt
        .apply(&cas, &stale, &mut wide()?)
        .expect_err("changed content under the same source versions must refuse");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedCertificate)
    );
    assert_eq!(fault.location, AT);
    Ok(())
}

#[test]
fn lc_affine_timing_clock_phase_decided_adjacent_turn() -> TestResult {
    let original = request()?;
    let cas = Cas::new(limits())?;
    cas.construct_lc_affine_timing_clock_phase(&original, &mut wide()?)?;
    let mut adjacent = original;
    adjacent.observations[0].physical_declarations[0][1] = integer(8, &mut wide()?)?;
    let Err(fault) = claim::rows(&adjacent, &mut wide()?) else {
        panic!("the preceding turn is decided to intersect the physical interval");
    };
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedCertificate)
    );
    assert_eq!(fault.location, AT);
    Ok(())
}

#[test]
fn lc_affine_timing_clock_phase_lift_membership() -> TestResult {
    let original = request()?;
    let cas = Cas::new(limits())?;
    cas.construct_lc_affine_timing_clock_phase(&original, &mut wide()?)?;
    let mut outside = original;
    outside.observations[0].lift = 5;
    let Err(fault) = claim::rows(&outside, &mut wide()?) else {
        panic!("selected lift is outside its complete domain");
    };
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedCertificate)
    );
    assert_eq!(fault.location, AT);
    Ok(())
}

#[test]
fn lc_affine_timing_clock_phase_corrupt_replay_atomic() -> TestResult {
    let original = request()?;
    let cas = Cas::new(limits())?;
    let receipt = cas.construct_lc_affine_timing_clock_phase(&original, &mut wide()?)?;
    let certificate = receipt.certificate(&cas, &mut wide()?)?;
    let mut receiver = Cas::new(limits())?;
    receiver.check_lc_affine_timing_clock_phase(&original, &certificate, &mut wide()?)?;
    let before = receiver.counts();
    let mut bytes = certificate.to_bytes(&mut wide()?)?;
    bytes.push(0);
    let corrupt = TimingClockPhaseCertificate::from_bytes(&bytes, &mut wide()?)?;
    let fault = receiver
        .check_lc_affine_timing_clock_phase(&original, &corrupt, &mut wide()?)
        .expect_err("trailing certificate byte must refuse");
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Encoding));
    assert_eq!(fault.location, crate::Location::new("witness decoding"));
    assert_eq!(receiver.counts(), before);
    Ok(())
}

fn rational_bytes(value: &Rational) -> u64 {
    let (n, d) = value.component_bits();
    (n.saturating_add(7) / 8).max(1) + (d.saturating_add(7) / 8).max(1)
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "all measured pass and retained-byte boundaries remain together"
)]
fn lc_affine_timing_clock_phase_resource_boundaries() -> TestResult {
    let original = request()?;
    let cas = Cas::new(limits())?;
    let mut b = wide()?;
    let topology = geometry::propose(&original, &mut b)?;
    let proposal = b.usage().work;
    check::verify(&original, &topology, &mut b)?;
    let verification = b.usage().work - proposal;
    let scope = wire::scope(&original, &mut b)?;
    let scope_work = b.usage().work - proposal - verification;
    let bytes = wire::encode(&scope, &topology, &mut b)?;
    let encoding = b.usage().work - proposal - verification - scope_work;
    let signs = geometry::predicates(&original, &mut b)?;
    let predicates = b.usage().work - proposal - verification - scope_work - encoding;
    let storage = service::storage_bytes(&original, &topology, &signs, bytes.len());
    let header = size_of::<TimingClockPhaseReceipt>() as u64;
    let mut request_bytes = (original.observations.len() * size_of::<TimingClockPhaseObservation>()
        + original.source_order.len() * size_of::<TimingClockPhaseSourceSlot>()
        + original.source_versions.len() * size_of::<u64>()
        + original.epoch.len()) as u64;
    for ranges in [
        &original.rate_declarations,
        &original.initial_declarations,
        &original.clock_declarations,
        &original.offset_declarations,
    ] {
        request_bytes += (ranges.len() * size_of::<[Rational; 2]>()) as u64;
        for range in ranges {
            request_bytes += rational_bytes(&range[0]) + rational_bytes(&range[1]);
        }
    }
    for range in [
        &original.rate,
        &original.initial,
        &original.clock,
        &original.offset,
    ] {
        request_bytes += rational_bytes(&range[0]) + rational_bytes(&range[1]);
    }
    for item in &original.observations {
        request_bytes += rational_bytes(&item.reported)
            + rational_bytes(&item.error[0])
            + rational_bytes(&item.error[1]);
        for ranges in [
            &item.error_declarations,
            &item.time_declarations,
            &item.measured_declarations,
            &item.physical_declarations,
        ] {
            request_bytes += (ranges.len() * size_of::<[Rational; 2]>()) as u64;
            for range in ranges {
                request_bytes += rational_bytes(&range[0]) + rational_bytes(&range[1]);
            }
        }
    }
    for value in original
        .pi
        .sums
        .iter()
        .chain([&original.pi.lower, &original.pi.upper])
    {
        request_bytes += rational_bytes(value);
    }
    let mut topology_bytes = (topology.vertices.len() * size_of::<TimingClockPhaseVertex>()
        + topology.edges.len() * size_of::<TimingClockPhaseEdge>())
        as u64;
    for vertex in &topology.vertices {
        topology_bytes += (vertex.point.len() * size_of::<PiAffine>()
            + vertex.active.len() * size_of::<u32>()) as u64;
        for value in &vertex.point {
            topology_bytes += rational_bytes(&value.base) + rational_bytes(&value.pi);
        }
    }
    for edge in &topology.edges {
        topology_bytes += (edge.supports.len() * size_of::<u32>()) as u64;
    }
    let mut signs_bytes = (signs.len() * size_of::<super::super::SignPredicate>()) as u64;
    for sign in &signs {
        for value in &sign.image {
            signs_bytes += rational_bytes(value);
        }
    }
    assert_eq!(
        storage,
        header + request_bytes + topology_bytes + signs_bytes + bytes.len() as u64
    );
    assert_eq!(
        [
            header,
            request_bytes,
            topology_bytes,
            signs_bytes,
            scope.len() as u64,
            bytes.len() as u64,
            signs.len() as u64
        ],
        [1_192, 1_819, 29_776, 4_147_260, 589, 7_141, 26_585]
    );
    let mut construction_budget = wide()?;
    let receipt =
        cas.construct_lc_affine_timing_clock_phase(&original, &mut construction_budget)?;
    let construction = construction_budget.usage().work;
    assert_eq!(
        construction,
        proposal + verification + scope_work + encoding + predicates + storage
    );
    let certificate = receipt.certificate(&cas, &mut wide()?)?;
    let mut receiver = Cas::new(limits())?;
    let mut replay_budget = wide()?;
    receiver.check_lc_affine_timing_clock_phase(&original, &certificate, &mut replay_budget)?;
    let replay = replay_budget.usage().work;
    let mut receive_pass = wide()?;
    let receiving_scope = wire::scope(&original, &mut receive_pass)?;
    let receive_scope = receive_pass.usage().work;
    drop(wire::decode(
        &bytes,
        &receiving_scope,
        original.observations.len() + 4,
        &mut receive_pass,
    )?);
    let decoding = receive_pass.usage().work - receive_scope;
    check::verify(&original, &topology, &mut receive_pass)?;
    let receive_verify = receive_pass.usage().work - receive_scope - decoding;
    drop(geometry::predicates(&original, &mut receive_pass)?);
    let receive_predicates = receive_pass.usage().work - receive_scope - decoding - receive_verify;
    let receive_storage = replay - receive_pass.usage().work;
    assert_eq!(receive_storage, storage + 256);
    assert_eq!(
        replay,
        receive_scope + decoding + receive_verify + receive_predicates + receive_storage
    );
    let mut unchanged_budget = wide()?;
    receipt.apply(&cas, &original, &mut unchanged_budget)?;
    let unchanged = unchanged_budget.usage().work;
    let mut changed = original.clone();
    changed.source_versions[4] += 1;
    let mut changed_budget = wide()?;
    receipt.apply(&cas, &changed, &mut changed_budget)?;
    let changed_work = changed_budget.usage().work;
    assert_eq!(
        [
            proposal,
            verification,
            scope_work,
            encoding,
            predicates,
            storage,
            construction,
            decoding,
            replay,
            unchanged,
            changed_work
        ],
        [
            177_940_523,
            185_536_133,
            1_581,
            17_381,
            379_317_899,
            4_187_188,
            747_000_705,
            67_355,
            569_110_412,
            3_755,
            742_798_310
        ]
    );
    let default =
        cas.construct_lc_affine_timing_clock_phase(&original, &mut Budget::new(Limits::default())?);
    let fault = default.expect_err("default construction Work refusal");
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::Work,
            ..
        }
    ));
    assert_eq!(fault.location, PROPOSAL_AT);
    let mut default_receiver = Cas::new(limits())?;
    let fault = default_receiver
        .check_lc_affine_timing_clock_phase(
            &original,
            &certificate,
            &mut Budget::new(Limits::default())?,
        )
        .expect_err("default replay Work refusal");
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::Work,
            ..
        }
    ));
    assert_eq!(fault.location, REPLAY_AT);
    Ok(())
}
