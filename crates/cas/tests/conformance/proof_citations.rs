// SPDX-License-Identifier: AGPL-3.0-only
//! Checked citation closure, assumption retention and exact extraction ceilings.

use crate::testing::budgets::proof_budget as budget;
use crate::testing::common::TestResult;
use crate::testing::proofs::Fixture;
use crate::testing::witness::equality;
use nrz_cas::limits::{Budget, BudgetKind};
use nrz_cas::proof::{
    HypothesisKind, PremiseCitation, Primitive, ProofCitation, QualifiedHypothesis, VerifiedWitness,
};
use nrz_cas::syntax::{Operator, PredicateKind};
use nrz_cas::{Cas, FaultKind, Location, LogicalId};

/// Three distinct checked applications share the original conditional cancellation proof.
fn composition() -> TestResult<(Fixture, VerifiedWitness)> {
    let mut fixture = Fixture::new()?;
    let subject = fixture.x.clone();
    fixture.assume("positive", PredicateKind::Positive, &subject)?;
    let quotient = fixture.term(Operator::Div, &[subject.clone(), subject])?;
    let cancel = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Cancel,
        &quotient,
        &fixture.one,
    );
    let mut reverse = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Symmetry,
        &fixture.one,
        &quotient,
    );
    reverse.premises = vec![0];
    let mut composed = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Transitivity,
        &quotient,
        &quotient,
    );
    composed.premises = vec![0, 1];
    let witness = fixture.cas.check_proof(
        &fixture.context,
        &[cancel, reverse, composed],
        &mut budget()?,
    )?;
    Ok((fixture, witness))
}

/// The actual checked rules, ordered shared edges and qualified assumptions survive composition and replay.
/// ´test:integration:proof-citations-retain-checked-composition´
#[test]
fn proof_citations_retain_checked_composition() -> TestResult {
    let (fixture, witness) = composition()?;
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    let citations = fixture
        .cas
        .proof_citations(witness.proof(), &mut budget()?)?;
    let names: Vec<_> = citations
        .iter()
        .map(|step| {
            let lemma = step.formal_schema().lemma;
            (lemma.name, lemma.version)
        })
        .collect();
    assert_eq!(names, [("eq.cancel", 1), ("eq.sym", 1), ("eq.trans", 1)]);
    assert_composed_conditions(&fixture, &witness, &citations)?;
    assert!(
        fixture
            .cas
            .proof_citations(&fixture.x, &mut budget()?)
            .is_err()
    );
    let mut fresh = Cas::new(budget()?.limits().clone())?;
    assert!(
        fresh
            .proof_citations(witness.proof(), &mut budget()?)
            .is_err()
    );
    let bytes = fixture.cas.export_witness(&witness, &mut budget()?)?;
    let replayed = fresh.inspect(&bytes, &mut budget()?)?;
    let imported = fresh.proof_citations(replayed.proof(), &mut budget()?)?;
    assert_eq!(
        imported.iter().map(|row| row.primitive).collect::<Vec<_>>(),
        citations
            .iter()
            .map(|row| row.primitive)
            .collect::<Vec<_>>()
    );
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// Check the shared edges and complete qualified assumption closure on every returned step.
fn assert_composed_conditions(
    fixture: &Fixture,
    witness: &VerifiedWitness,
    citations: &[ProofCitation],
) -> TestResult {
    for step in citations {
        let [used] = step.hypotheses.as_slice() else {
            return Err("lost assumption".into());
        };
        assert_eq!(used.context, fixture.context);
        assert_eq!(used.slot, 0);
        assert_eq!(used.kind, HypothesisKind::StrictSign);
        assert_eq!(used.kind.formal_schema().label, "assum:cas:hyp-strict-sign");
        assert_eq!(step.residuals, [] as [HypothesisKind; 0]);
        for premise in &step.premises {
            assert_eq!(premise.kind, HypothesisKind::Equality);
        }
    }
    let [first, second, last] = citations else {
        return Err("wrong step count".into());
    };
    assert_eq!(
        second.premises.first().ok_or("missing premise")?.proof,
        first.proof
    );
    let edges: Vec<_> = last.premises.iter().map(|item| &item.proof).collect();
    assert_eq!(edges, [&first.proof, &second.proof]);
    assert_eq!(&last.proof, witness.proof());
    Ok(())
}

/// Conservative simultaneous scratch, result and hypothesis element storage for the three-node DAG.
fn payload() -> TestResult<u64> {
    Ok(u64::try_from(
        7 * std::mem::size_of::<LogicalId>()
            + 3 * std::mem::size_of::<ProofCitation>()
            + 3 * std::mem::size_of::<PremiseCitation>()
            + 3 * std::mem::size_of::<QualifiedHypothesis>(),
    )?)
}

/// Independent ledger and payload bounds include prior Work and refuse atomically at one less.
/// ´test:integration:proof-citation-extraction-boundaries´
#[test]
fn proof_citation_extraction_boundaries() -> TestResult {
    let (fixture, witness) = composition()?;
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    let bytes = payload()?;
    let required = bytes + 61;
    let mut measured = budget()?;
    fixture
        .cas
        .proof_citations(witness.proof(), &mut measured)?;
    assert_eq!(measured.usage().work, required);
    for kind in [
        BudgetKind::Work,
        BudgetKind::StagedPayloadBytes,
        BudgetKind::OutputBytes,
        BudgetKind::ProofNodes,
    ] {
        let bound = match kind {
            BudgetKind::Work => required + 17,
            BudgetKind::ProofNodes => 3,
            _ => bytes,
        };
        for (limit, succeeds) in [(bound, true), (bound - 1, false)] {
            let mut limits = budget()?.limits().clone();
            match kind {
                BudgetKind::Work => limits.work = limit,
                BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = limit,
                BudgetKind::OutputBytes => limits.output_bytes = limit,
                BudgetKind::ProofNodes => limits.proof_nodes = limit,
                _ => return Err("unexpected citation budget".into()),
            }
            let mut ledger = Budget::new(limits)?;
            ledger.work(17, Location::new("prior citation work"))?;
            let result = fixture.cas.proof_citations(witness.proof(), &mut ledger);
            if succeeds {
                assert!(result.is_ok());
            } else {
                let fault = result.err().ok_or("short citation budget succeeded")?;
                assert!(
                    matches!(fault.kind, FaultKind::Exhausted { budget: exhausted, .. } if exhausted == kind)
                );
            }
            assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
        }
    }
    Ok(())
}

/// Open domain conditions are reported independently of qualified context assumptions.
/// ´test:integration:proof-citations-retain-open-residuals´
#[test]
fn proof_citations_retain_open_residuals() -> TestResult {
    let (mut fixture, _, step) = crate::testing::proofs::cancellation()?;
    let witness = fixture
        .cas
        .check_proof(&fixture.context, &[step], &mut budget()?)?;
    let citations = fixture
        .cas
        .proof_citations(witness.proof(), &mut budget()?)?;
    let [only] = citations.as_slice() else {
        return Err("wrong cancellation count".into());
    };
    assert_eq!(only.hypotheses, [] as [QualifiedHypothesis; 0]);
    assert_eq!(only.residuals, [HypothesisKind::NonZero]);
    assert!(!witness.is_discharged());
    Ok(())
}
