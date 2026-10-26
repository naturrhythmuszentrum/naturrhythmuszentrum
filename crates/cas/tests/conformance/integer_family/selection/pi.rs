// SPDX-License-Identifier: AGPL-3.0-only
//! Original LC Integer projection retains the floor family and Machin certificate.

use super::{SelectionFixture, enclosure, fixture};
use crate::testing::common::TestResult;
use crate::testing::readings::ledger;
use nrz_cas::decision::EnclosureOrigin;
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::proof::VerifiedWitness;
use nrz_cas::{Cas, FaultKind, InvalidCode, Location, Rational};

fn dependencies(cas: &Cas, witness: &VerifiedWitness) -> TestResult {
    let mut family = 0;
    let mut pi = 0;
    for citation in cas.proof_citations(witness.proof(), &mut ledger()?)? {
        if let Some(schema) = citation.auxiliary_schema(cas)?
            && schema.name == "decision.integer-floor-family"
        {
            assert_eq!(schema.version, 2);
            family += 1;
        }
        if let Some(schema) = citation.constant_schema(cas, &mut ledger()?)? {
            assert_eq!(
                (schema.name, schema.version),
                ("constant.pi-machin-bounds", 1)
            );
            pi += 1;
        }
    }
    assert_eq!((family, pi), (1, 1));
    Ok(())
}

/// The original 10/Pi target selects exact Integer three under the checked nonnegative family.
/// ´test:integration:integer-family-pi-selection´
#[test]
fn integer_family_pi_selection() -> TestResult {
    for bounded in [false, true] {
        let mut source = fixture(bounded, "3", "10/Pi")?;
        let witness =
            source
                .frame
                .cas
                .check_proof(&source.context, &source.steps, &mut ledger()?)?;
        assert!(witness.is_discharged());
        assert!(!witness.is_unconditional());
        assert_eq!(witness.hypotheses().len(), 1);
        dependencies(&source.frame.cas, &witness)?;
        assert_eq!(
            source
                .frame
                .cas
                .materialize_integer_selection(&witness, 3, &mut ledger()?)?,
            3
        );
        let fault = source
            .frame
            .cas
            .materialize_integer_selection(&witness, 2, &mut ledger()?)
            .err()
            .ok_or("LC count exceeded capacity")?;
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::DecisionMaterializationCapacity)
        );
        let bytes = source.frame.cas.export_witness(&witness, &mut ledger()?)?;
        let mut receiver = Cas::new(Limits::default())?;
        let replayed = receiver.inspect(&bytes, &mut ledger()?)?;
        assert_eq!(replayed.hypotheses().len(), 1);
        dependencies(&receiver, &replayed)?;
        assert_eq!(
            receiver.materialize_integer_selection(&replayed, 3, &mut ledger()?)?,
            3
        );
        assert_eq!(receiver.export_witness(&replayed, &mut ledger()?)?, bytes);
    }
    Ok(())
}

/// Missing pi evidence and a valid enclosure crossing the floor boundary refuse atomically.
/// ´test:integration:integer-family-pi-refusals´
#[test]
fn integer_family_pi_refusals() -> TestResult {
    let mut source = fixture(false, "3", "10/Pi")?;
    enclosure(&mut source)?.origin = EnclosureOrigin::Unverified;
    refusal(&mut source, InvalidCode::UnsupportedCertificate)?;
    let mut source = fixture(false, "3", "10/Pi")?;
    enclosure(&mut source)?.lower = Rational::from_ratio(191, 64, &mut ledger()?)?;
    refusal(&mut source, InvalidCode::DecisionBoundaryIntersection)?;
    let mut source = fixture(false, "3", "10/Pi")?;
    let EnclosureOrigin::PiBound { certificate, .. } = &mut enclosure(&mut source)?.origin else {
        return Err("missing pi origin".into());
    };
    certificate.lower = Rational::from_ratio(3, 1, &mut ledger()?)?;
    certificate.upper = Rational::from_ratio(4, 1, &mut ledger()?)?;
    refusal(&mut source, InvalidCode::DecisionBox)?;
    refusal(
        &mut fixture(false, "4", "10/Pi")?,
        InvalidCode::DecisionBoundaryIntersection,
    )
}

/// Original LC projection and fresh replay retain the complete constant theorem.
/// ´test:integration:integer-family-pi-costs´
#[test]
fn integer_family_pi_costs() -> TestResult {
    let mut rows = Vec::new();
    for bounded in [false, true] {
        let mut source = fixture(bounded, "3", "10/Pi")?;
        let mut budget = ledger()?;
        let witness = source
            .frame
            .cas
            .check_proof(&source.context, &source.steps, &mut budget)?;
        let bytes = source.frame.cas.export_witness(&witness, &mut ledger()?)?;
        let mut receiver = Cas::new(Limits::default())?;
        let mut replay = ledger()?;
        receiver.inspect(&bytes, &mut replay)?;
        rows.push((budget.usage().work, replay.usage().work, bytes.len()));
    }
    assert_eq!(rows, [(166_952, 247_365, 1_562), (199_780, 299_788, 1_812)]);
    Ok(())
}

fn refusal(source: &mut SelectionFixture, expected: InvalidCode) -> TestResult {
    let before = source.frame.cas.counts();
    let snapshot = source.frame.cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = ledger()?;
    let fault = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut budget)
        .err()
        .ok_or("false LC selection accepted")?;
    assert_eq!(fault.kind, FaultKind::Invalid(expected));
    assert_eq!(source.frame.cas.counts(), before);
    assert_eq!(
        source.frame.cas.canonical_snapshot(&mut ledger()?)?,
        snapshot
    );
    assert!(budget.usage().work > 0);
    Ok(())
}

fn resource_attempt(
    bounded: bool,
    replay: bool,
    amount: u64,
    prior: u64,
    accepted: bool,
) -> TestResult {
    let mut source = fixture(bounded, "3", "10/Pi")?;
    let bytes = if replay {
        let witness =
            source
                .frame
                .cas
                .check_proof(&source.context, &source.steps, &mut ledger()?)?;
        Some(source.frame.cas.export_witness(&witness, &mut ledger()?)?)
    } else {
        None
    };
    let mut receiver = Cas::new(Limits::default())?;
    let cas = if replay {
        &mut receiver
    } else {
        &mut source.frame.cas
    };
    let before = cas.counts();
    let snapshot = cas.canonical_snapshot(&mut ledger()?)?;
    let limits = Limits {
        work: amount,
        ..Limits::default()
    };
    let mut budget = Budget::new(limits.clone())?;
    budget.work(prior, Location::new("prior original LC selection work"))?;
    let result = if let Some(bytes) = bytes {
        cas.inspect(&bytes, &mut budget)
    } else {
        cas.check_proof(&source.context, &source.steps, &mut budget)
    };
    if accepted {
        assert!(result?.is_discharged());
        assert_eq!(budget.usage().work, amount);
    } else {
        let fault = result.err().ok_or("LC selection exceeded Work")?;
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
    assert_eq!(budget.limits(), &limits);
    assert!(budget.usage().work > prior);
    Ok(())
}

/// Literal N/N−1 checks retain atomic graph state and prior caller Work for original LC proofs.
/// ´test:integration:integer-family-pi-resources´
#[test]
fn integer_family_pi_resources() -> TestResult {
    for (bounded, replay, work) in [
        (false, false, 166_952),
        (false, true, 247_365),
        (true, false, 199_780),
        (true, true, 299_788),
    ] {
        resource_attempt(bounded, replay, work, 0, true)?;
        resource_attempt(bounded, replay, work - 1, 0, false)?;
        resource_attempt(bounded, replay, work, 1, false)?;
        resource_attempt(bounded, replay, work + 1, 1, true)?;
    }
    Ok(())
}
