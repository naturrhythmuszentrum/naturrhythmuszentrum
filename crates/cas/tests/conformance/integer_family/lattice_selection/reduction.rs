// SPDX-License-Identifier: AGPL-3.0-only
//! Supplied reductions transport nearest candidates and ties back to the original index order.

use super::{Selection, TestResult, column, exact, ledger, number, record_count, skew};
use nrz_cas::lattice::{LatticeEnclosure, LatticeReduction};
use nrz_cas::limits::Limits;
use nrz_cas::proof::VerifiedWitness;
use nrz_cas::{Cas, FaultKind, InvalidCode, Rational};

fn fixture(ties: bool) -> TestResult<Selection> {
    let mut source = skew()?;
    if ties {
        tie_target(&mut source)?;
    }
    source.expected.values[0] = number(&mut source.source, 1, 1, true)?;
    let mut transform = Vec::new();
    let mut inverse = Vec::new();
    for value in [1, -1, 0, 0, 1, 0, 0, 0, 1] {
        transform.push(Rational::from_ratio(value, 1, &mut ledger()?)?);
    }
    for value in [1, 1, 0, 0, 1, 0, 0, 0, 1] {
        inverse.push(Rational::from_ratio(value, 1, &mut ledger()?)?);
    }
    source.enclosure = LatticeEnclosure::Reduced(Box::new(LatticeReduction {
        transform,
        inverse,
        values: source.expected.values.clone(),
        enclosure: source.enclosure,
    }));
    Ok(source)
}

fn tie_target(source: &mut Selection) -> TestResult {
    let mut coordinates = Vec::new();
    let mut targets = Vec::new();
    for ratio in [(1, 2), (1, 2), (1, 10)] {
        let target = number(&mut source.source, ratio.0, ratio.1, false)?;
        coordinates.push(exact(source, target.clone(), ratio)?);
        targets.push(target);
    }
    source.expected.target = column(&mut source.source, &targets)?;
    let LatticeEnclosure::Original(enclosure) = &mut source.enclosure else {
        return Err("missing source box".into());
    };
    enclosure
        .original
        .target
        .clone_from(&source.expected.target);
    enclosure.original.coordinates = coordinates;
    enclosure.bounds.clear();
    for ratio in [(0, 1), (1, 2), (1, 10)] {
        let value = Rational::from_ratio(ratio.0, ratio.1, &mut ledger()?)?;
        enclosure.bounds.push((value.clone(), value));
    }
    Ok(())
}

fn selected(source: &mut Selection) -> TestResult<VerifiedWitness> {
    Ok(source.source.frame.cas.prove_lattice_nearest_selection(
        &source.cover,
        &source.expected,
        &source.enclosure,
        &mut ledger()?,
    )?)
}

/// U=A proves B*U=I, original k=(1,0,0), and the exact expected representative.
/// ´test:integration:lattice-reduced-skew´
#[test]
fn lattice_reduced_skew() -> TestResult {
    let mut source = fixture(false)?;
    let witness = selected(&mut source)?;
    let outputs = source
        .source
        .frame
        .cas
        .lattice_selection_outputs(&witness, &mut ledger()?)?;
    assert_eq!(outputs.tuple(), source.expected.values);
    let representative = outputs.representative().clone();
    assert!(outputs.identity().is_discharged());
    let mut entries = Vec::new();
    for ratio in [(-2, 5), (2, 5), (1, 10)] {
        entries.push(number(&mut source.source, ratio.0, ratio.1, false)?);
    }
    let expected = column(&mut source.source, &entries)?;
    assert!(
        source
            .source
            .frame
            .cas
            .prove_reading_equality(
                &source.expected.cover.index.basis.context,
                &representative,
                &expected,
                &mut ledger()?
            )?
            .is_discharged()
    );
    let bytes = source
        .source
        .frame
        .cas
        .export_witness(&witness, &mut ledger()?)?;
    let mut receiver = Cas::new(Limits::default())?;
    let replayed = receiver.inspect(&bytes, &mut ledger()?)?;
    assert!(replayed.is_discharged());
    assert_eq!(receiver.export_witness(&replayed, &mut ledger()?)?, bytes);
    let citations = receiver.proof_citations(replayed.proof(), &mut ledger()?)?;
    assert_eq!(
        citations
            .last()
            .ok_or("missing citation")?
            .auxiliary_schema(&receiver)?
            .ok_or("missing schema")?
            .name,
        "decision.lattice-reduced-nearest-selection"
    );
    Ok(())
}

/// Simultaneous reduced ties follow original k-order; silent reduced-coordinate rounding refuses.
/// ´test:integration:lattice-reduced-tie-order´
#[test]
fn lattice_reduced_tie_order() -> TestResult {
    let mut source = fixture(true)?;
    selected(&mut source)?;
    let zero = number(&mut source.source, 0, 1, true)?;
    let one = number(&mut source.source, 1, 1, true)?;
    source.expected.values = vec![zero.clone(), one.clone(), zero];
    let LatticeEnclosure::Reduced(reduction) = &mut source.enclosure else {
        return Err("missing reduction".into());
    };
    reduction.values[1] = one;
    refuses(
        &mut source,
        InvalidCode::DecisionBoundaryIntersection,
        "lattice tuple selection",
    )
}

fn refuses(source: &mut Selection, expected: InvalidCode, owner: &str) -> TestResult {
    let before = source.source.frame.cas.counts();
    let original = source
        .source
        .frame
        .cas
        .export_witness(&source.cover, &mut ledger()?)?;
    let mut budget = ledger()?;
    let fault = source
        .source
        .frame
        .cas
        .prove_lattice_nearest_selection(
            &source.cover,
            &source.expected,
            &source.enclosure,
            &mut budget,
        )
        .err()
        .ok_or("false reduction selected")?;
    assert_eq!(fault.kind, FaultKind::Invalid(expected));
    assert_eq!(fault.location.operation, owner);
    assert!(budget.usage().work > 0);
    assert_eq!(source.source.frame.cas.counts(), before);
    assert_eq!(
        source
            .source
            .frame
            .cas
            .export_witness(&source.cover, &mut ledger()?)?,
        original
    );
    Ok(())
}

/// Each missing, nonintegral, false-inverse or false-candidate mutation names its rejecting checker.
/// ´test:integration:lattice-reduced-refusals´
#[test]
fn lattice_reduced_refusals() -> TestResult {
    for mutation in 0..5 {
        let mut source = fixture(false)?;
        let zero = number(&mut source.source, 0, 1, true)?;
        let LatticeEnclosure::Reduced(reduction) = &mut source.enclosure else {
            return Err("missing reduction".into());
        };
        match mutation {
            0 => {
                reduction.inverse.pop();
            }
            1 => reduction.transform[0] = Rational::from_ratio(1, 2, &mut ledger()?)?,
            2 => reduction.inverse[1] = Rational::zero(),
            3 => reduction.values[0] = zero,
            4 => {
                reduction.transform[1] = Rational::zero();
                reduction.inverse[1] = Rational::zero();
            }
            _ => unreachable!("five explicit mutations"),
        }
        refuses(
            &mut source,
            InvalidCode::UnsupportedCertificate,
            if mutation == 4 {
                "lattice orthogonal nearest"
            } else {
                "lattice reduction nearest"
            },
        )?;
    }
    Ok(())
}

/// Named figures keep the five owners and two sealed record inventories distinct.
struct Costs {
    /// Portable selected theorem construction.
    proof: u64,
    /// Fresh inspection of the selected theorem.
    replay: u64,
    /// Complete local evidence preparation.
    preparation: u64,
    /// Exact representative and reconstruction construction.
    outputs: u64,
    /// Fresh reconstruction identity inspection.
    identity_replay: u64,
    /// Complete selected theorem byte length.
    selection_bytes: usize,
    /// Selected theorem's sealed record count.
    selection_records: usize,
    /// Reconstruction identity byte length.
    identity_bytes: usize,
    /// Reconstruction identity's sealed record count.
    identity_records: usize,
}

/// Original skew followed by its simultaneous reduced-coordinate ties.
const COSTS: [Costs; 2] = [
    Costs {
        proof: 695_582,
        replay: 1_644_023,
        preparation: 311_228,
        outputs: 132_441,
        identity_replay: 205_491,
        selection_bytes: 4_905,
        selection_records: 83,
        identity_bytes: 1_604,
        identity_records: 33,
    },
    Costs {
        proof: 684_538,
        replay: 1_625_876,
        preparation: 303_578,
        outputs: 130_563,
        identity_replay: 200_146,
        selection_bytes: 4_853,
        selection_records: 82,
        identity_bytes: 1_554,
        identity_records: 32,
    },
];

/// Every reduction owner succeeds at N and refuses at N-1 on the monotone caller ledger.
/// ´test:integration:lattice-reduced-work´
#[test]
fn lattice_reduced_work() -> TestResult {
    for (ties, cost) in [false, true].into_iter().zip(COSTS) {
        let mut source = fixture(ties)?;
        let cover = source
            .source
            .frame
            .cas
            .export_witness(&source.cover, &mut ledger()?)?;
        super::costs::preparation(&mut source, cost.preparation + 36, false)?;
        super::costs::preparation(&mut source, cost.preparation + 37, true)?;
        super::costs::proof(&mut source, cost.proof + 36, false)?;
        let witness = super::costs::proof(&mut source, cost.proof + 37, true)?
            .ok_or("missing reduced tuple")?;
        let encoded = source
            .source
            .frame
            .cas
            .export_witness(&witness, &mut ledger()?)?;
        assert_eq!(encoded.len(), cost.selection_bytes);
        assert_eq!(record_count(&encoded)?, cost.selection_records);
        super::super::costs::replay_boundary(&encoded, cost.replay + 36, false)?;
        super::super::costs::replay_boundary(&encoded, cost.replay + 37, true)?;
        super::costs::outputs(&mut source, &witness, cost.outputs + 36, false)?;
        let encoded = super::costs::outputs(&mut source, &witness, cost.outputs + 37, true)?
            .ok_or("missing reduced outputs")?;
        assert_eq!(encoded.len(), cost.identity_bytes);
        assert_eq!(record_count(&encoded)?, cost.identity_records);
        super::super::costs::replay_boundary(&encoded, cost.identity_replay + 36, false)?;
        super::super::costs::replay_boundary(&encoded, cost.identity_replay + 37, true)?;
        assert_eq!(
            source
                .source
                .frame
                .cas
                .export_witness(&source.cover, &mut ledger()?)?,
            cover
        );
    }
    Ok(())
}

/// Reduced original and direct boxes retain exact hypotheses and actual Machin citations on fresh replay.
/// ´test:integration:lattice-reduced-replay-scope´
#[test]
fn lattice_reduced_replay_scope() -> TestResult {
    for (mut source, hypotheses, pi) in [
        (super::prepared(4)?, 2, true),
        (super::super::direct::direct(5)?, 0, false),
    ] {
        let floor = super::super::selected(&mut source, &mut ledger()?)?;
        let floor_bytes = source
            .source
            .frame
            .cas
            .export_witness(&floor, &mut ledger()?)?;
        let count = source.expected.values.len();
        let mut transform = Vec::new();
        for row in 0..count {
            for column in 0..count {
                transform.push(if row == column {
                    Rational::one()
                } else {
                    Rational::zero()
                });
            }
        }
        source.enclosure = LatticeEnclosure::Reduced(Box::new(LatticeReduction {
            transform: transform.clone(),
            inverse: transform,
            values: source.expected.values.clone(),
            enclosure: source.enclosure,
        }));
        let witness = selected(&mut source)?;
        assert_eq!(witness.hypotheses(), floor.hypotheses());
        assert_eq!(witness.hypotheses().len(), hypotheses);
        let bytes = source
            .source
            .frame
            .cas
            .export_witness(&witness, &mut ledger()?)?;
        assert_eq!(record_count(&bytes)?, record_count(&floor_bytes)?);
        let mut receiver = Cas::new(Limits::default())?;
        let replayed = receiver.inspect(&bytes, &mut ledger()?)?;
        assert_eq!(replayed.hypotheses().len(), hypotheses);
        assert!(replayed.is_discharged());
        let citations = receiver.proof_citations(replayed.proof(), &mut ledger()?)?;
        let schema = citations
            .last()
            .ok_or("missing reduction citation")?
            .constant_schema(&receiver, &mut ledger()?)?;
        assert_eq!(
            schema.map(|value| (value.name, value.version)),
            pi.then_some(("constant.pi-machin-bounds", 1))
        );
    }
    Ok(())
}
