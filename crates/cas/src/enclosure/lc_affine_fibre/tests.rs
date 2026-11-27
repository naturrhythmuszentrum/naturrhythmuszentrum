// SPDX-License-Identifier: AGPL-3.0-only
//! Independent exact timing topology, restoration and mutation controls.

use super::*;
use crate::budget::{Budget, Limits};
use crate::{Cas, Fault, FaultKind, InvalidCode};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn ledger() -> Result<Budget, Fault> {
    Budget::new(Limits::default())
}

fn wide() -> Result<Budget, Fault> {
    Budget::new(Limits::blind_cubic())
}

fn invalid_code(result: Result<impl Sized, Fault>, code: InvalidCode) {
    assert_eq!(
        result.err().map(|fault| fault.kind),
        Some(FaultKind::Invalid(code))
    );
}

fn topology_refusal(result: Result<impl Sized, Fault>) {
    let fault = result.err().expect("expected topology refusal");
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::PendingPremise));
    assert_eq!(fault.location, TOPOLOGY_AT);
}

fn sign_refusal(result: Result<impl Sized, Fault>) {
    let fault = result.err().expect("expected sign refusal");
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::PendingPremise));
    assert_eq!(fault.location, SIGN_AT);
}

fn q(n: i64, d: u64, b: &mut Budget) -> Result<Rational, Fault> {
    Rational::from_ratio(n, d, b)
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

fn request(
    rate: [i64; 2],
    initial: [i64; 2],
    observations: Vec<([i64; 2], [i64; 2])>,
) -> Result<TimingRequest, Fault> {
    let b = &mut ledger()?;
    let observations = observations
        .into_iter()
        .map(|(time, phase)| {
            Ok(TimingObservation {
                time: [q(time[0], 1, b)?, q(time[1], 1, b)?],
                phase: [q(phase[0], 1, b)?, q(phase[1], 1, b)?],
                time_declarations: vec![[q(time[0], 1, b)?, q(time[1], 1, b)?]],
                phase_declarations: vec![[q(phase[0], 1, b)?, q(phase[1], 1, b)?]],
                lift: 0,
                lift_domain: [0, 0],
            })
        })
        .collect::<Result<Vec<_>, Fault>>()?;
    let mut source_order = vec![
        TimingSourceSlot {
            kind: TimingSourceKind::Rate,
            declaration: 0,
        },
        TimingSourceSlot {
            kind: TimingSourceKind::Initial,
            declaration: 0,
        },
    ];
    for index in 0..observations.len() {
        source_order.push(TimingSourceSlot {
            kind: TimingSourceKind::Time(index),
            declaration: 0,
        });
        source_order.push(TimingSourceSlot {
            kind: TimingSourceKind::Phase(index),
            declaration: 0,
        });
    }
    Ok(TimingRequest {
        epoch: "test epoch".into(),
        nominal: vec![Rational::zero(); observations.len()],
        source_order,
        rate: [q(rate[0], 1, b)?, q(rate[1], 1, b)?],
        initial: [q(initial[0], 1, b)?, q(initial[1], 1, b)?],
        source_versions: vec![1; observations.len() * 2 + 2],
        rate_declarations: vec![[q(rate[0], 1, b)?, q(rate[1], 1, b)?]],
        initial_declarations: vec![[q(initial[0], 1, b)?, q(initial[1], 1, b)?]],
        observations,
        pi: pi(b)?,
        revision: 1,
    })
}

fn exact(value: &PiAffine, expected: i64) -> TestResult {
    assert_eq!(value.base, q(expected, 1, &mut ledger()?)?);
    assert_eq!(value.pi, Rational::zero());
    Ok(())
}

#[test]
fn lc_affine_timing_expected_claim() -> TestResult {
    let original = request([1, 2], [0, 1], vec![([0, 0], [0, 1])])?;
    let cas = Cas::new(Limits::default())?;
    let mut construct_budget = ledger()?;
    let receipt = cas.construct_lc_affine_timing(&original, &mut construct_budget)?;
    assert_eq!(construct_budget.usage().work, 1_174_126);
    assert_eq!(receipt.formal_schema().name, "lc-affine-fibre");
    let topology = receipt.topology(&cas, &mut ledger()?)?;
    assert_eq!(topology.dimension, Dimension::Polygon);
    assert_eq!(topology.vertices.len(), 4);
    assert_eq!(topology.edges.len(), 4);
    let mut copy_budget = ledger()?;
    let copy = topology.copy_charged(&mut copy_budget)?;
    assert_eq!(&copy, topology);
    assert_eq!(copy_budget.usage().work, 1_536);
    let output_short = Limits {
        output_bytes: 1_535,
        ..Limits::default()
    };
    assert!(matches!(
        topology.copy_charged(&mut Budget::new(output_short)?),
        Err(Fault {
            kind: FaultKind::Exhausted {
                budget: crate::budget::BudgetKind::OutputBytes,
                ..
            },
            ..
        })
    ));
    let staged_short = Limits {
        staged_payload_bytes: 3_071,
        ..Limits::default()
    };
    assert!(matches!(
        topology.copy_charged(&mut Budget::new(staged_short)?),
        Err(Fault {
            kind: FaultKind::Exhausted {
                budget: crate::budget::BudgetKind::StagedPayloadBytes,
                ..
            },
            ..
        })
    ));
    for (vertex, [rate, phase]) in topology
        .vertices
        .iter()
        .zip([[1, 0], [1, 1], [2, 0], [2, 1]])
    {
        exact(&vertex.point[0], rate)?;
        exact(&vertex.point[1], phase)?;
    }
    let mut spurious = topology.clone();
    spurious.vertices.push(Vertex {
        point: [
            PiAffine::rational(q(3, 1, &mut ledger()?)?),
            PiAffine::rational(Rational::zero()),
        ],
        active: Vec::new(),
    });
    assert!(check::verify(&original, &spurious, &mut ledger()?).is_err());
    let certificate = receipt.certificate(&cas, &mut ledger()?)?;
    let mut receiver = Cas::new(Limits::default())?;
    let mut replay_budget = ledger()?;
    let replay = receiver.check_lc_affine_timing(&original, &certificate, &mut replay_budget)?;
    assert_eq!(replay_budget.usage().work, 840_730);
    let mut apply_budget = ledger()?;
    receipt.apply(&cas, &original, &mut apply_budget)?;
    assert_eq!(apply_budget.usage().work, 2_542);
    assert_eq!(replay.topology(&receiver, &mut ledger()?)?, topology);
    Ok(())
}

#[test]
fn lc_affine_timing_incidence() -> TestResult {
    for (rate, initial, phase, dimension, vertices, edges) in [
        ([1, 2], [0, 1], [2, 3], Dimension::Empty, 0, 0),
        ([1, 1], [0, 0], [0, 0], Dimension::Point, 1, 0),
        ([1, 2], [0, 0], [0, 0], Dimension::Segment, 2, 1),
        ([1, 2], [0, 1], [0, 1], Dimension::Polygon, 4, 4),
    ] {
        let original = request(rate, initial, vec![([0, 0], phase)])?;
        let cas = Cas::new(Limits::default())?;
        let receipt = cas.construct_lc_affine_timing(&original, &mut ledger()?)?;
        let topology = receipt.topology(&cas, &mut ledger()?)?;
        assert_eq!(topology.dimension, dimension);
        assert_eq!(topology.vertices.len(), vertices);
        assert_eq!(topology.edges.len(), edges);
        if dimension != Dimension::Empty {
            let omitted = Topology {
                dimension: Dimension::Empty,
                vertices: Vec::new(),
                edges: Vec::new(),
            };
            assert!(check::verify(&original, &omitted, &mut ledger()?).is_err());
        }
        let mut missing = topology.clone();
        if !missing.vertices.is_empty() {
            missing.vertices.pop();
            assert!(check::verify(&original, &missing, &mut ledger()?).is_err());
        }
        let mut bad_dimension = topology.clone();
        bad_dimension.dimension = Dimension::Empty;
        if dimension != Dimension::Empty {
            assert!(check::verify(&original, &bad_dimension, &mut ledger()?).is_err());
        }
    }
    Ok(())
}

#[test]
fn lc_affine_timing_restoration() -> TestResult {
    let original = request([1, 2], [0, 1], vec![([0, 2], [1, 2]), ([1, 3], [1, 3])])?;
    let cas = Cas::new(Limits::default())?;
    let mut construct_budget = wide()?;
    let receipt = cas.construct_lc_affine_timing(&original, &mut construct_budget)?;
    assert_eq!(construct_budget.usage().work, 2_693_246);
    let topology = receipt.topology(&cas, &mut wide()?)?;
    for vertex in &topology.vertices {
        let witness = receipt.restore(&cas, &original, &vertex.point, &mut wide()?)?;
        assert_eq!(witness.timestamps.len(), 2);
        assert_eq!(witness.rate, vertex.point[0]);
        assert_eq!(witness.initial, vertex.point[1]);
    }
    let point = [
        PiAffine::rational(Rational::one()),
        PiAffine::rational(Rational::zero()),
    ];
    let witness = receipt.restore(&cas, &original, &point, &mut wide()?)?;
    for time in &witness.timestamps {
        exact(&time.numerator, 1)?;
        exact(&time.denominator, 1)?;
    }
    let mut changed = original.clone();
    changed.initial[1] = Rational::zero();
    changed.initial_declarations[0][1] = Rational::zero();
    changed.source_versions[1] += 1;
    let mut changed_budget = wide()?;
    topology_refusal(receipt.restore(&cas, &changed, &point, &mut changed_budget));
    // This pin distinguishes the topology refusal from a later sign refusal.
    assert_eq!(changed_budget.usage().work, 1_160_202);
    let mut apply_budget = wide()?;
    receipt.apply(&cas, &original, &mut apply_budget)?;
    assert_eq!(apply_budget.usage().work, 3_622);
    let mut default_budget = ledger()?;
    receipt.apply(&cas, &original, &mut default_budget)?;
    assert_eq!(default_budget.usage().work, 3_622);
    let mut version_only = changed;
    version_only.initial = original.initial.clone();
    version_only.initial_declarations = original.initial_declarations.clone();
    assert!(receipt.apply(&cas, &version_only, &mut wide()?).is_ok());
    Ok(())
}

#[test]
fn lc_affine_timing_mutations() -> TestResult {
    let original = request([1, 2], [0, 1], vec![([0, 0], [0, 1])])?;
    let cas = Cas::new(Limits::default())?;
    let receipt = cas.construct_lc_affine_timing(&original, &mut ledger()?)?;
    let bytes = receipt
        .certificate(&cas, &mut ledger()?)?
        .to_bytes(&mut ledger()?)?;
    let mut receiver = Cas::new(Limits::default())?;
    let before = receiver.counts();
    let mut changed = original.clone();
    changed.source_versions[0] += 1;
    assert!(
        receiver
            .check_lc_affine_timing(
                &changed,
                &Certificate::from_bytes(&bytes, &mut ledger()?)?,
                &mut ledger()?
            )
            .is_err()
    );
    assert_eq!(receiver.counts(), before);
    let mut corrupt = bytes;
    corrupt.push(0);
    assert!(
        receiver
            .check_lc_affine_timing(
                &original,
                &Certificate::from_bytes(&corrupt, &mut ledger()?)?,
                &mut ledger()?
            )
            .is_err()
    );
    assert_eq!(receiver.counts(), before);
    let mut incomplete = original.clone();
    incomplete.observations[0].lift_domain = [1, 1];
    assert!(
        cas.construct_lc_affine_timing(&incomplete, &mut ledger()?)
            .is_err()
    );
    let omitted_feasible = request([1, 1], [0, 0], vec![([0, 0], [0, 7])])?;
    assert!(
        cas.construct_lc_affine_timing(&omitted_feasible, &mut ledger()?)
            .is_err()
    );
    Ok(())
}

#[test]
fn lc_affine_timing_stale_same_version() -> TestResult {
    let original = request([1, 2], [0, 1], vec![([0, 0], [0, 1])])?;
    let cas = Cas::new(Limits::default())?;
    let receipt = cas.construct_lc_affine_timing(&original, &mut ledger()?)?;
    let mut stale = original;
    stale.rate[1] = q(3, 1, &mut ledger()?)?;
    stale.rate_declarations[0][1] = q(3, 1, &mut ledger()?)?;
    invalid_code(
        receipt.apply(&cas, &stale, &mut ledger()?),
        InvalidCode::UnsupportedCertificate,
    );
    stale.source_versions[0] += 1;
    let mut changed_budget = ledger()?;
    assert!(receipt.apply(&cas, &stale, &mut changed_budget).is_ok());
    assert_eq!(changed_budget.usage().work, 1_160_692);
    Ok(())
}

#[test]
fn lc_affine_timing_changed_live_incidence() -> TestResult {
    let original = request([1, 2], [0, 1], vec![([0, 0], [0, 1])])?;
    let cas = Cas::new(Limits::default())?;
    let receipt = cas.construct_lc_affine_timing(&original, &mut ledger()?)?;
    let mut changed = original;
    changed.initial = [Rational::one(), Rational::one()];
    changed.initial_declarations[0] = changed.initial.clone();
    changed.source_versions[1] += 1;
    let mut b = ledger()?;
    topology_refusal(receipt.apply(&cas, &changed, &mut b));
    // This pin distinguishes the topology refusal from a later sign refusal.
    assert_eq!(b.usage().work, 659_438);
    Ok(())
}

#[test]
fn lc_affine_timing_sign_stratum() -> TestResult {
    let mut original = request([1, 2], [0, 1], vec![([1, 1], [0, 4]), ([2, 2], [0, 6])])?;
    let a_upper = q(7, 2, &mut wide()?)?;
    let b_upper = q(27, 5, &mut wide()?)?;
    original.observations[0].phase[1] = a_upper.clone();
    original.observations[0].phase_declarations[0][1] = a_upper;
    original.observations[1].phase[1] = b_upper.clone();
    original.observations[1].phase_declarations[0][1] = b_upper;
    let cas = Cas::new(Limits::default())?;
    let mut budget = wide()?;
    let receipt = cas.construct_lc_affine_timing(&original, &mut budget)?;
    assert_eq!(budget.usage().work, 2_648_010);
    let certificate = receipt.certificate(&cas, &mut wide()?)?;
    let mut receiver = Cas::new(Limits::default())?;
    let mut replay_budget = wide()?;
    receiver.check_lc_affine_timing(&original, &certificate, &mut replay_budget)?;
    assert_eq!(replay_budget.usage().work, 2_091_129);
    assert!(matches!(
        receiver.check_lc_affine_timing(&original, &certificate, &mut ledger()?),
        Err(Fault {
            kind: FaultKind::Exhausted {
                budget: crate::budget::BudgetKind::Work,
                ..
            },
            ..
        })
    ));
    assert!(matches!(
        cas.construct_lc_affine_timing(&original, &mut ledger()?),
        Err(Fault {
            kind: FaultKind::Exhausted {
                budget: crate::budget::BudgetKind::Work,
                ..
            },
            ..
        })
    ));
    let mut budget = wide()?;
    receipt.apply(&cas, &original, &mut budget)?;
    assert_eq!(budget.usage().work, 3_616);
    let mut same_stratum = original.clone();
    let close = q(53, 10, &mut wide()?)?;
    same_stratum.observations[1].phase[1] = close.clone();
    same_stratum.observations[1].phase_declarations[0][1] = close;
    same_stratum.source_versions[5] += 1;
    let mut budget = wide()?;
    receipt.apply(&cas, &same_stratum, &mut budget)?;
    assert_eq!(budget.usage().work, 2_613_552);
    let mut changed = original.clone();
    let flip = q(28, 5, &mut wide()?)?;
    changed.observations[1].phase[1] = flip.clone();
    changed.observations[1].phase_declarations[0][1] = flip;
    changed.source_versions[5] += 1;
    assert_eq!(
        claim::propose(&original, &mut wide()?)?,
        claim::propose(&changed, &mut wide()?)?
    );
    let two = q(2, 1, &mut wide()?)?;
    let before = q(27, 5, &mut wide()?)?
        .sub(&q(7, 2, &mut wide()?)?, &mut wide()?)?
        .sub(&two, &mut wide()?)?;
    let after = q(28, 5, &mut wide()?)?
        .sub(&q(7, 2, &mut wide()?)?, &mut wide()?)?
        .sub(&two, &mut wide()?)?;
    assert_eq!(before, q(-1, 10, &mut wide()?)?);
    assert_eq!(after, q(1, 10, &mut wide()?)?);
    let old_signs = claim::predicates(&original, &mut wide()?)?;
    let new_signs = claim::predicates(&changed, &mut wide()?)?;
    assert_eq!(old_signs.len(), new_signs.len());
    let flipped_index = old_signs
        .iter()
        .zip(&new_signs)
        .position(|(old, new)| {
            old.image == [before.clone(), before.clone()]
                && new.image == [after.clone(), after.clone()]
        })
        .expect("a <= 2 residual at the upper-row intersection");
    assert_eq!(flipped_index, 113);
    assert!(
        old_signs
            .iter()
            .zip(&new_signs)
            .any(|(old, new)| old.required != new.required)
    );
    sign_refusal(receipt.apply(&cas, &changed, &mut wide()?));
    Ok(())
}

#[test]
fn lc_affine_timing_pi_lift_and_boundaries() -> TestResult {
    let mut lifted = request([1, 2], [0, 1], vec![([0, 0], [-6, -6])])?;
    lifted.observations[0].lift = 1;
    lifted.observations[0].lift_domain = [1, 1];
    let cas = Cas::new(Limits::default())?;
    let mut construct_budget = ledger()?;
    let receipt = cas.construct_lc_affine_timing(&lifted, &mut construct_budget)?;
    let construction_work = construct_budget.usage().work;
    assert_eq!(construction_work, 1_175_886);
    let topology = receipt.topology(&cas, &mut ledger()?)?;
    assert_eq!(topology.dimension, Dimension::Segment);
    assert_eq!(topology.vertices.len(), 2);
    for vertex in &topology.vertices {
        assert_eq!(vertex.point[1].base, q(-6, 1, &mut ledger()?)?);
        assert_eq!(vertex.point[1].pi, q(2, 1, &mut ledger()?)?);
        let witness = receipt.restore(&cas, &lifted, &vertex.point, &mut ledger()?)?;
        assert_eq!(witness.timestamps.len(), 1);
        assert_eq!(
            witness.timestamps[0].numerator,
            PiAffine::rational(Rational::zero())
        );
    }
    let (rows, _) = claim::rows(&lifted, &mut ledger()?)?;
    let two = q(2, 1, &mut ledger()?)?;
    assert!(rows.iter().any(|row| row.bound.pi == two));
    let exact_limit = Limits {
        work: construction_work,
        ..Limits::default()
    };
    let mut exact_budget = Budget::new(exact_limit)?;
    cas.construct_lc_affine_timing(&lifted, &mut exact_budget)?;
    assert_eq!(exact_budget.usage().work, construction_work);
    let one_below = Limits {
        work: construction_work - 1,
        ..Limits::default()
    };
    assert!(matches!(
        cas.construct_lc_affine_timing(&lifted, &mut Budget::new(one_below)?),
        Err(Fault {
            kind: FaultKind::Exhausted {
                budget: crate::budget::BudgetKind::Work,
                ..
            },
            ..
        })
    ));
    let mut undecided = lifted;
    let near_pi = q(-44, 7, &mut ledger()?)?;
    undecided.observations[0].phase = [near_pi.clone(), near_pi.clone()];
    undecided.observations[0].phase_declarations[0] = [near_pi.clone(), near_pi];
    undecided.initial = [Rational::zero(), Rational::zero()];
    undecided.initial_declarations[0] = undecided.initial.clone();
    invalid_code(
        cas.construct_lc_affine_timing(&undecided, &mut ledger()?),
        InvalidCode::PendingPremise,
    );
    Ok(())
}
