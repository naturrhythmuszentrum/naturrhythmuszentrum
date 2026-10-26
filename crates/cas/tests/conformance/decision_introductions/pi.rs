// SPDX-License-Identifier: AGPL-3.0-only
//! The original LC quotient introduces a qualified cell with its pi theorem retained.

use super::{IntroductionFixture, frame_draft, membership, prepared_introduction};
use crate::conformance::decision_baseline::Example;
use crate::conformance::pi_bounds::cycle;
use crate::testing::builders::admit;
use crate::testing::common::TestResult;
use crate::testing::readings::ledger;
use nrz_cas::decision::EnclosureOrigin;
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::proof::{Certificate, VerifiedWitness};
use nrz_cas::{Cas, FaultKind, Handle, InvalidCode, Location};

fn fixture() -> TestResult<IntroductionFixture> {
    let mut fixture = prepared_introduction(Example::Integer, false)?;
    let Certificate::Decision(evidence) = &mut fixture
        .steps
        .last_mut()
        .ok_or("missing introduction")?
        .certificate
    else {
        return Err("missing introduction certificate".into());
    };
    let enclosure = evidence.enclosures.first_mut().ok_or("missing enclosure")?;
    enclosure.origin = EnclosureOrigin::PiBound {
        expression: enclosure.target.clone(),
        certificate: Box::new(cycle()?),
    };
    Ok(fixture)
}

fn pi_citations(cas: &Cas, witness: &VerifiedWitness) -> TestResult<usize> {
    let mut count = 0;
    for citation in cas.proof_citations(witness.proof(), &mut ledger()?)? {
        if let Some(schema) = citation.constant_schema(cas, &mut ledger()?)? {
            assert_eq!(
                (schema.name, schema.version),
                ("constant.pi-machin-bounds", 1)
            );
            count += 1;
        }
    }
    Ok(count)
}

fn introduced() -> TestResult<(IntroductionFixture, Handle)> {
    let mut fixture = fixture()?;
    let source = &mut fixture.partition.source;
    let intro = source
        .cas
        .check_proof(&source.context, &fixture.steps, &mut ledger()?)?;
    assert_eq!(intro.hypotheses().len(), 1);
    assert_eq!(pi_citations(&source.cas, &intro)?, 1);
    let draft = frame_draft(&fixture, &intro);
    let frame = admit(&mut fixture.partition.source.cas, draft, &mut ledger()?)?;
    Ok((fixture, frame))
}

/// The LC target belongs to [3,4) with source correspondence and Machin evidence retained.
/// ´test:integration:decision-pi-certified-membership´
#[test]
fn decision_pi_certified_membership() -> TestResult {
    let (mut fixture, frame) = introduced()?;
    let source = &mut fixture.partition.source;
    let step = membership(&source.cas, &frame, &fixture.option)?;
    let witness = source.cas.check_proof(&frame, &[step], &mut ledger()?)?;
    assert!(witness.is_discharged());
    assert!(!witness.is_unconditional());
    assert_eq!(witness.hypotheses().len(), 2);
    assert_eq!(pi_citations(&source.cas, &witness)?, 1);
    let bytes = source.cas.export_witness(&witness, &mut ledger()?)?;
    let mut receiver = Cas::new(Limits::default())?;
    let replayed = receiver.inspect(&bytes, &mut ledger()?)?;
    assert_eq!(replayed.hypotheses().len(), 2);
    assert_eq!(pi_citations(&receiver, &replayed)?, 1);
    assert_eq!(receiver.export_witness(&replayed, &mut ledger()?)?, bytes);
    Ok(())
}

/// Measured LC introduction and fresh membership replay retain the full constant proof.
/// ´test:integration:decision-pi-membership-costs´
#[test]
fn decision_pi_membership_costs() -> TestResult {
    let mut fixture = fixture()?;
    let source = &mut fixture.partition.source;
    let mut budget = ledger()?;
    let intro = source
        .cas
        .check_proof(&source.context, &fixture.steps, &mut budget)?;
    let work = budget.usage().work;
    let draft = frame_draft(&fixture, &intro);
    let frame = admit(&mut fixture.partition.source.cas, draft, &mut ledger()?)?;
    let source = &mut fixture.partition.source;
    let step = membership(&source.cas, &frame, &fixture.option)?;
    let member = source.cas.check_proof(&frame, &[step], &mut ledger()?)?;
    let bytes = source.cas.export_witness(&member, &mut ledger()?)?;
    let mut receiver = Cas::new(Limits::default())?;
    let mut replay = ledger()?;
    receiver.inspect(&bytes, &mut replay)?;
    assert_eq!(
        (work, replay.usage().work, bytes.len()),
        (381_454, 608_307, 3_073)
    );
    Ok(())
}

fn resource_attempt(replay: bool, amount: u64, prior: u64, accepted: bool) -> TestResult {
    let mut source = fixture()?;
    let bytes = if replay {
        let (mut source, frame) = introduced()?;
        let cas = &mut source.partition.source.cas;
        let step = membership(cas, &frame, &source.option)?;
        let member = cas.check_proof(&frame, &[step], &mut ledger()?)?;
        Some(cas.export_witness(&member, &mut ledger()?)?)
    } else {
        None
    };
    let mut receiver = Cas::new(Limits::default())?;
    let cas = if replay {
        &mut receiver
    } else {
        &mut source.partition.source.cas
    };
    let before = cas.counts();
    let snapshot = cas.canonical_snapshot(&mut ledger()?)?;
    let limits = Limits {
        work: amount,
        ..Limits::default()
    };
    let mut budget = Budget::new(limits.clone())?;
    budget.work(prior, Location::new("prior pi membership work"))?;
    let result = if let Some(bytes) = bytes {
        cas.inspect(&bytes, &mut budget)
    } else {
        cas.check_proof(&source.partition.source.context, &source.steps, &mut budget)
    };
    if accepted {
        assert!(result?.is_discharged());
        assert_eq!(budget.usage().work, amount);
    } else {
        let fault = result.err().ok_or("pi membership exceeded Work")?;
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            }
        ));
        assert_eq!(cas.counts(), before);
        assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    }
    assert!(budget.usage().work > prior);
    assert_eq!(budget.limits(), &limits);
    Ok(())
}

/// N/N−1 introduction and import limits retain atomic publication and spent caller Work.
/// ´test:integration:decision-pi-membership-resources´
#[test]
fn decision_pi_membership_resources() -> TestResult {
    for (replay, work) in [(false, 381_454), (true, 608_307)] {
        resource_attempt(replay, work, 0, true)?;
        resource_attempt(replay, work - 1, 0, false)?;
        resource_attempt(replay, work, 1, false)?;
        resource_attempt(replay, work + 1, 1, true)?;
    }
    Ok(())
}

/// Wire changes to the theorem revision, term inventory, exact sums or bounds refuse on replay.
/// ´test:integration:decision-pi-wire-refusals´
#[test]
fn decision_pi_wire_refusals() -> TestResult {
    let (mut fixture, frame) = introduced()?;
    let cas = &mut fixture.partition.source.cas;
    let step = membership(cas, &frame, &fixture.option)?;
    let witness = cas.check_proof(&frame, &[step], &mut ledger()?)?;
    let bytes = cas.export_witness(&witness, &mut ledger()?)?;
    let [version, terms, sums, lower] = super::wire::pi_offsets(&bytes)?;
    for (index, expected) in [
        (version, InvalidCode::UnsupportedVersion),
        (terms, InvalidCode::EndpointMismatch),
        (sums, InvalidCode::EndpointMismatch),
        (lower, InvalidCode::DecisionBox),
    ] {
        let mut changed = bytes.clone();
        if index == version {
            changed[index..index + 8].copy_from_slice(&2_u64.to_le_bytes());
        } else if index == terms {
            changed[index..index + 8].copy_from_slice(&1_u64.to_le_bytes());
        } else if index == lower {
            // Negate the positive upper endpoint, leaving a reversed claimed interval.
            let mut position = lower + 1;
            for _ in 0..2 {
                let count = u32::from_le_bytes(changed[position..position + 4].try_into()?);
                position += 4 + usize::try_from(count)?;
            }
            changed[position] = 1;
        } else {
            changed[index] = 1;
        }
        let mut receiver = Cas::new(Limits::default())?;
        let before = receiver.counts();
        let snapshot = receiver.canonical_snapshot(&mut ledger()?)?;
        let mut budget = ledger()?;
        let fault = receiver
            .inspect(&changed, &mut budget)
            .err()
            .ok_or("tampered pi certificate accepted")?;
        assert_eq!(fault.kind, FaultKind::Invalid(expected));
        assert_eq!(receiver.counts(), before);
        assert_eq!(receiver.canonical_snapshot(&mut ledger()?)?, snapshot);
        assert!(budget.usage().work > 0);
    }
    Ok(())
}
