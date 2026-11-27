// SPDX-License-Identifier: AGPL-3.0-only
//! Exact clock-phase branch controls.

use super::*;
use crate::budget::{Budget, BudgetKind, Limits};
use crate::{Cas, Fault, FaultKind, InvalidCode};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Construct the wide finite caller ledger.
fn wide() -> Result<Budget, Fault> {
    Budget::new(Limits::blind_cubic())
}
/// Construct an exact small integer rational.
fn q(value: i64, b: &mut Budget) -> Result<Rational, Fault> {
    Rational::from_ratio(value, 1, b)
}
/// Construct the checked Machin pi certificate fixture.
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
/// Construct the complete one-observation fixture.
fn request() -> Result<ClockPhaseRequest, Fault> {
    let b = &mut wide()?;
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
        ClockPhaseSourceKind::Rate,
        ClockPhaseSourceKind::Initial,
        ClockPhaseSourceKind::Clock,
        ClockPhaseSourceKind::Offset,
        ClockPhaseSourceKind::Time(0),
        ClockPhaseSourceKind::Measured(0),
        ClockPhaseSourceKind::Physical(0),
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

/// Embed four integer coordinates as exact affine values.
fn point(values: [i64; 4]) -> Result<[PiAffine; 4], Fault> {
    let b = &mut wide()?;
    Ok(values.map(|value| PiAffine::rational(q(value, b).expect("small integer"))))
}

#[test]
/// Compare the complete vertex, active-row and edge oracle with the checked fibre.
#[expect(
    clippy::too_many_lines,
    reason = "the exact external vertex and edge oracle is kept together"
)]
fn lc_affine_clock_phase_expected_claim() -> TestResult {
    let original = request()?;
    let cas = Cas::new(Limits::blind_cubic())?;
    let mut construction = wide()?;
    let receipt = cas.construct_lc_affine_clock_phase(&original, &mut construction)?;
    let topology = receipt.topology(&cas, &mut wide()?)?;
    assert_eq!(topology.dimension, Some(4));
    // Enumerated outside Rust from the 14 rational inequalities by independent
    // Fraction Gaussian elimination of every four-row subset, then row evaluation.
    let expected = [
        [
            PiAffine::rational(q(1, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(-3, 4, &mut wide()?)?),
            PiAffine::rational(q(1, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(1, 4, &mut wide()?)?),
        ],
        [
            PiAffine::rational(q(1, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(-1, 2, &mut wide()?)?),
            PiAffine::rational(q(1, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(1, 2, &mut wide()?)?),
        ],
        [
            PiAffine::rational(q(1, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(1, 4, &mut wide()?)?),
            PiAffine::rational(q(0, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(1, 4, &mut wide()?)?),
        ],
        [
            PiAffine::rational(q(1, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(1, 4, &mut wide()?)?),
            PiAffine::rational(q(1, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(1, 4, &mut wide()?)?),
        ],
        [
            PiAffine::rational(q(1, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(1, 2, &mut wide()?)?),
            PiAffine::rational(q(0, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(1, 2, &mut wide()?)?),
        ],
        [
            PiAffine::rational(q(1, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(1, 2, &mut wide()?)?),
            PiAffine::rational(q(1, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(1, 2, &mut wide()?)?),
        ],
        [
            PiAffine::rational(q(1, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(5, 4, &mut wide()?)?),
            PiAffine::rational(q(0, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(1, 4, &mut wide()?)?),
        ],
        [
            PiAffine::rational(q(1, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(3, 2, &mut wide()?)?),
            PiAffine::rational(q(0, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(1, 2, &mut wide()?)?),
        ],
        [
            PiAffine::rational(q(2, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(-1, 2, &mut wide()?)?),
            PiAffine::rational(q(1, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(1, 2, &mut wide()?)?),
        ],
        [
            PiAffine::rational(q(2, &mut wide()?)?),
            PiAffine::rational(q(0, &mut wide()?)?),
            PiAffine::rational(q(1, &mut wide()?)?),
            PiAffine::rational(q(1, &mut wide()?)?),
        ],
        [
            PiAffine::rational(q(2, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(1, 2, &mut wide()?)?),
            PiAffine::rational(q(0, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(1, 2, &mut wide()?)?),
        ],
        [
            PiAffine::rational(q(2, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(1, 2, &mut wide()?)?),
            PiAffine::rational(q(1, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(1, 2, &mut wide()?)?),
        ],
        [
            PiAffine::rational(q(2, &mut wide()?)?),
            PiAffine::rational(q(1, &mut wide()?)?),
            PiAffine::rational(q(0, &mut wide()?)?),
            PiAffine::rational(q(1, &mut wide()?)?),
        ],
        [
            PiAffine::rational(q(2, &mut wide()?)?),
            PiAffine::rational(q(1, &mut wide()?)?),
            PiAffine::rational(q(1, &mut wide()?)?),
            PiAffine::rational(q(1, &mut wide()?)?),
        ],
        [
            PiAffine::rational(q(2, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(3, 2, &mut wide()?)?),
            PiAffine::rational(q(0, &mut wide()?)?),
            PiAffine::rational(Rational::from_ratio(1, 2, &mut wide()?)?),
        ],
        [
            PiAffine::rational(q(2, &mut wide()?)?),
            PiAffine::rational(q(2, &mut wide()?)?),
            PiAffine::rational(q(0, &mut wide()?)?),
            PiAffine::rational(q(1, &mut wide()?)?),
        ],
    ];
    let active = [
        vec![0, 2, 7, 8],
        vec![0, 2, 7, 9],
        vec![0, 2, 6, 8],
        vec![0, 3, 7, 8],
        vec![0, 2, 6, 9],
        vec![0, 3, 7, 9],
        vec![0, 3, 6, 8],
        vec![0, 3, 6, 9],
        vec![1, 2, 7, 8],
        vec![1, 2, 7, 9],
        vec![1, 2, 6, 8],
        vec![1, 3, 7, 8],
        vec![1, 2, 6, 9],
        vec![1, 3, 7, 9, 13],
        vec![1, 3, 6, 8],
        vec![1, 3, 6, 9, 11, 13],
    ];
    let edges = [
        ([0, 1], vec![0, 2, 7]),
        ([0, 2], vec![0, 2, 8]),
        ([0, 3], vec![0, 7, 8]),
        ([0, 8], vec![2, 7, 8]),
        ([1, 4], vec![0, 2, 9]),
        ([1, 5], vec![0, 7, 9]),
        ([1, 9], vec![2, 7, 9]),
        ([2, 4], vec![0, 2, 6]),
        ([2, 6], vec![0, 6, 8]),
        ([2, 10], vec![2, 6, 8]),
        ([3, 5], vec![0, 3, 7]),
        ([3, 6], vec![0, 3, 8]),
        ([3, 11], vec![3, 7, 8]),
        ([4, 7], vec![0, 6, 9]),
        ([4, 12], vec![2, 6, 9]),
        ([5, 7], vec![0, 3, 9]),
        ([5, 13], vec![3, 7, 9]),
        ([6, 7], vec![0, 3, 6]),
        ([6, 14], vec![3, 6, 8]),
        ([7, 15], vec![3, 6, 9]),
        ([8, 9], vec![1, 2, 7]),
        ([8, 10], vec![1, 2, 8]),
        ([8, 11], vec![1, 7, 8]),
        ([9, 12], vec![1, 2, 9]),
        ([9, 13], vec![1, 7, 9]),
        ([10, 12], vec![1, 2, 6]),
        ([10, 14], vec![1, 6, 8]),
        ([11, 13], vec![1, 3, 7]),
        ([11, 14], vec![1, 3, 8]),
        ([12, 15], vec![1, 6, 9]),
        ([13, 15], vec![1, 3, 9, 13]),
        ([14, 15], vec![1, 3, 6]),
    ];
    assert_eq!(topology.vertices.len(), expected.len());
    for ((vertex, coordinates), rows) in topology.vertices.iter().zip(expected).zip(active) {
        assert_eq!(vertex.point, coordinates);
        assert_eq!(vertex.active, rows);
    }
    assert_eq!(topology.edges.len(), edges.len());
    for (edge, (ends, supports)) in topology.edges.iter().zip(edges) {
        assert_eq!(edge.ends, ends);
        assert_eq!(edge.supports, supports);
    }
    assert_eq!(receipt.formal_schema(), &SCHEMA);
    let certificate = receipt.certificate(&cas, &mut wide()?)?;
    let mut receiver = Cas::new(Limits::blind_cubic())?;
    let mut replay = wide()?;
    let checked_receipt =
        receiver.check_lc_affine_clock_phase(&original, &certificate, &mut replay)?;
    assert_eq!(checked_receipt.topology(&receiver, &mut wide()?)?, topology);
    let mut apply = wide()?;
    let (same, _) = receipt.apply(&cas, &original, &mut apply)?;
    assert!(matches!(same, std::borrow::Cow::Borrowed(_)));
    assert_eq!(construction.usage().work, 483_564_796);
    assert_eq!(replay.usage().work, 282_267_194);
    assert_eq!(apply.usage().work, 3_310);

    let mut by_pass = wide()?;
    let proposed = geometry::propose(&original, &mut by_pass)?;
    let proposal_work = by_pass.usage().work;
    check::verify(&original, &proposed, &mut by_pass)?;
    let verification_work = by_pass.usage().work - proposal_work;
    let scope = wire::scope(&original, &mut by_pass)?;
    let scope_work = by_pass.usage().work - proposal_work - verification_work;
    let bytes = wire::encode(&scope, &proposed, &mut by_pass)?;
    let encoding_work = by_pass.usage().work - proposal_work - verification_work - scope_work;
    let signs = geometry::predicates(&original, &mut by_pass)?;
    let predicate_work =
        by_pass.usage().work - proposal_work - verification_work - scope_work - encoding_work;
    let storage_work = construction.usage().work - by_pass.usage().work;
    let inventory_bytes = service::storage_bytes(&original, &proposed, &signs, bytes.len());
    assert_eq!(storage_work, inventory_bytes);
    assert_eq!(
        [
            proposal_work,
            verification_work,
            scope_work,
            encoding_work,
            predicate_work,
            storage_work
        ],
        [
            201_326_688,
            36_765_183,
            1_390,
            7_082,
            244_363_852,
            1_100_601
        ]
    );
    let mut receive_pass = wide()?;
    let scope2 = wire::scope(&original, &mut receive_pass)?;
    let receive_scope = receive_pass.usage().work;
    drop(wire::decode(&bytes, &scope2, &mut receive_pass)?);
    let decode_work = receive_pass.usage().work - receive_scope;
    check::verify(&original, &proposed, &mut receive_pass)?;
    let receive_verify = receive_pass.usage().work - receive_scope - decode_work;
    drop(geometry::predicates(&original, &mut receive_pass)?);
    let receive_predicates =
        receive_pass.usage().work - receive_scope - decode_work - receive_verify;
    let receive_storage = replay.usage().work - receive_pass.usage().work;
    assert_eq!(receive_storage, inventory_bytes + 256);
    assert_eq!(
        [
            receive_scope,
            decode_work,
            receive_verify,
            receive_predicates,
            receive_storage
        ],
        [1_390, 35_912, 36_765_183, 244_363_852, 1_100_857]
    );
    Ok(())
}

#[test]
/// Restore one point with nonzero common phase and clock offsets.
fn lc_affine_clock_phase_joint_restoration() -> TestResult {
    let original = request()?;
    let cas = Cas::new(Limits::blind_cubic())?;
    let receipt = cas.construct_lc_affine_clock_phase(&original, &mut wide()?)?;
    let witness = receipt.restore(&cas, &original, &point([2, 1, 1, 1])?, &mut wide()?)?;
    assert_eq!(witness.rate, PiAffine::rational(q(2, &mut wide()?)?));
    assert_eq!(witness.initial, PiAffine::rational(q(1, &mut wide()?)?));
    assert_eq!(
        witness.clock_offset.numerator,
        PiAffine::rational(q(1, &mut wide()?)?)
    );
    assert_eq!(witness.clock_offset.denominator, witness.rate);
    assert_eq!(witness.measured[0], PiAffine::rational(q(3, &mut wide()?)?));
    assert_eq!(witness.physical[0], PiAffine::rational(q(4, &mut wide()?)?));
    assert_eq!(
        witness.measured_intercept,
        PiAffine::rational(q(1, &mut wide()?)?)
    );
    assert_eq!(
        witness.phase_offset,
        PiAffine::rational(q(1, &mut wide()?)?)
    );
    assert!(
        receipt
            .restore(&cas, &original, &point([2, 3, 1, 2])?, &mut wide()?)
            .is_err()
    );
    Ok(())
}

/// An owned topology copy reserves its vectors after both exact byte preflights.
#[test]
fn lc_affine_clock_phase_copy_limits() -> TestResult {
    let original = request()?;
    let cas = Cas::new(Limits::blind_cubic())?;
    let receipt = cas.construct_lc_affine_clock_phase(&original, &mut wide()?)?;
    let topology = receipt.topology(&cas, &mut wide()?)?;
    let mut budget = wide()?;
    assert_eq!(&topology.copy_charged(&mut budget)?, topology);
    let bytes = budget.usage().work;
    let short_output = Limits {
        output_bytes: bytes - 1,
        ..Limits::blind_cubic()
    };
    let fault = topology
        .copy_charged(&mut Budget::new(short_output)?)
        .expect_err("one below output bytes");
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::OutputBytes,
            ..
        }
    ));
    let short_staging = Limits {
        staged_payload_bytes: 2 * bytes - 1,
        ..Limits::blind_cubic()
    };
    let fault = topology
        .copy_charged(&mut Budget::new(short_staging)?)
        .expect_err("one below staging bytes");
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::StagedPayloadBytes,
            ..
        }
    ));
    Ok(())
}

#[test]
/// Refuse stale versions, changed strata, incomplete turns and default Work.
fn lc_affine_clock_phase_refusals() -> TestResult {
    let original = request()?;
    let cas = Cas::new(Limits::blind_cubic())?;
    let receipt = cas.construct_lc_affine_clock_phase(&original, &mut wide()?)?;
    let mut stale = original.clone();
    stale.clock[1] = q(0, &mut wide()?)?;
    stale.clock_declarations[0][1] = stale.clock[1].clone();
    let fault = receipt
        .apply(&cas, &stale, &mut wide()?)
        .expect_err("stale source");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedCertificate)
    );
    assert_eq!(fault.location, AT);
    let mut changed = original.clone();
    changed.source_versions[2] += 1;
    let mut version_only = original.clone();
    version_only.source_versions[2] += 1;
    let (live, signs) = receipt.apply(&cas, &version_only, &mut wide()?)?;
    assert!(matches!(live, std::borrow::Cow::Owned(_)));
    assert!(!signs.is_empty());
    changed.clock[1] = q(0, &mut wide()?)?;
    changed.clock_declarations[0][1] = changed.clock[1].clone();
    let result = receipt.apply(&cas, &changed, &mut wide()?);
    let fault = result.expect_err("changed topology refuses");
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::PendingPremise));
    assert_eq!(fault.location, TOPOLOGY_AT);
    let mut changed_sign = original.clone();
    changed_sign.source_versions[5] += 1;
    changed_sign.observations[0].measured_declarations[0][0] = q(-1, &mut wide()?)?;
    let fault = receipt
        .apply(&cas, &changed_sign, &mut wide()?)
        .err()
        .ok_or("changed exact sign stratum must refuse")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::PendingPremise));
    assert_eq!(fault.location, SIGN_AT);
    let mut bad = original.clone();
    bad.observations[0].lift_domain = [1, 1];
    let fault = cas
        .construct_lc_affine_clock_phase(&bad, &mut wide()?)
        .expect_err("selected turn is outside its lift domain");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedCertificate)
    );
    assert_eq!(fault.location, AT);
    let default =
        cas.construct_lc_affine_clock_phase(&original, &mut Budget::new(Limits::default())?);
    let fault = default.expect_err("default Work refusal");
    assert_eq!(fault.location, PROPOSAL_AT);
    let mut incomplete = original.clone();
    incomplete.initial[1] = q(7, &mut wide()?)?;
    incomplete.initial_declarations[0][1] = incomplete.initial[1].clone();
    let fault = cas
        .construct_lc_affine_clock_phase(&incomplete, &mut wide()?)
        .expect_err("adjacent turn intersects outer physical image");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedCertificate)
    );
    assert_eq!(fault.location, AT);
    let mut incomplete_lower = original.clone();
    incomplete_lower.initial[0] = q(-4, &mut wide()?)?;
    incomplete_lower.initial_declarations[0][0] = incomplete_lower.initial[0].clone();
    let fault = cas
        .construct_lc_affine_clock_phase(&incomplete_lower, &mut wide()?)
        .expect_err("lower adjacent turn intersects outer physical image");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedCertificate)
    );
    assert_eq!(fault.location, AT);
    let certificate = receipt.certificate(&cas, &mut wide()?)?;
    let mut receiver = Cas::new(Limits::blind_cubic())?;
    let fault = receiver
        .check_lc_affine_clock_phase(
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
