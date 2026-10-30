// SPDX-License-Identifier: AGPL-3.0-only
//! Formal standing distinguishes proved instances from assumptions throughout composition.

use crate::testing::budgets::proof_budget as budget;
use crate::testing::builders::admit;
use crate::testing::common::TestResult;
use crate::testing::proofs::Fixture;
use crate::testing::witness::equality;
use nrz_cas::domain::{AssumptionDraft, ContextDraft};
use nrz_cas::proof::{
    ANONYMOUS_LEMMA_SCHEMA, ApplicationDraft, EMPTY_SCHEMAS, HYPOTHESIS_SCHEMAS, HypothesisKind,
    LemmaProvenance, PRIMITIVE_SCHEMAS, Primitive, Standing, VerifiedWitness,
};
use nrz_cas::syntax::{Draft, Operator, PredicateKind};
use nrz_cas::{Cas, Handle, Reference};

/// Every estate label agrees with its explicit standing, including unavailable schemas and cache reuse.
/// ´test:integration:formal-register-standing-is-complete´
#[test]
fn formal_register_standing_is_complete() {
    let entries = PRIMITIVE_SCHEMAS
        .iter()
        .map(|entry| &entry.lemma)
        .chain(&EMPTY_SCHEMAS)
        .chain(&nrz_cas::proof::CONSTANT_SCHEMAS)
        .chain(&nrz_cas::proof::RETAINED_SCHEMAS)
        .chain(&nrz_cas::enclosure::ENCLOSURE_SCHEMAS)
        .chain(&nrz_cas::decision::DECISION_SCHEMAS)
        .chain(&HYPOTHESIS_SCHEMAS)
        .chain(std::iter::once(&ANONYMOUS_LEMMA_SCHEMA));
    for schema in entries {
        let expected = if schema.label.starts_with("thm:") {
            Standing::Lemma(LemmaProvenance::CasChecked)
        } else {
            assert!(schema.label.starts_with("assum:"));
            Standing::Hypothesis
        };
        assert_eq!(schema.standing, expected);
    }
}

/// Introduce two distinct root assumptions without changing their original predicate identities.
fn assume_pair(fixture: &mut Fixture) -> TestResult {
    let header = fixture.cas.context_header(&fixture.context)?;
    let mut assumptions = Vec::new();
    for (key, kind, subject) in [
        ("x-positive", PredicateKind::Positive, fixture.x.clone()),
        ("y-nonzero", PredicateKind::NonZero, fixture.y.clone()),
    ] {
        let condition = admit(
            &mut fixture.cas,
            Draft::Condition {
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                kind,
                arguments: vec![Reference::Existing(subject)],
            },
            &mut budget()?,
        )?;
        assumptions.push(AssumptionDraft {
            key: key.into(),
            condition: Reference::Existing(condition),
        });
    }
    fixture.context = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header),
            admissible: Reference::Existing(fixture.domain.clone()),
            assumptions,
        }),
        &mut budget()?,
    )?;
    Ok(())
}

/// Propose the two cancellations and their ordered equality composition.
fn steps(fixture: &mut Fixture) -> TestResult<Vec<ApplicationDraft>> {
    let left = fixture.term(Operator::Div, &[fixture.x.clone(), fixture.x.clone()])?;
    let right = fixture.term(Operator::Div, &[fixture.y.clone(), fixture.y.clone()])?;
    let first = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Cancel,
        &left,
        &fixture.one,
    );
    let second = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Cancel,
        &right,
        &fixture.one,
    );
    let mut reverse = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Symmetry,
        &fixture.one,
        &right,
    );
    reverse.premises = vec![1];
    let mut join = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Transitivity,
        &left,
        &right,
    );
    join.premises = vec![0, 2];
    Ok(vec![first, second, reverse, join])
}

/// Read labels from the checked conclusion's exact qualified closure, not from unused context slots.
fn labels(cas: &Cas, witness: &VerifiedWitness) -> TestResult<Vec<&'static str>> {
    let mut result = Vec::new();
    for reference in witness.hypotheses() {
        result.push(reference.qualified(cas)?.kind.formal_schema().label);
    }
    Ok(result)
}

/// Both hypothesis labels survive composition and replay; an altered context cannot erase them.
/// ´test:integration:hypothesis-standing-survives-composition´
#[test]
fn hypothesis_standing_survives_composition() -> TestResult {
    let mut fixture = Fixture::new()?;
    let unassumed = fixture.context.clone();
    assume_pair(&mut fixture)?;
    let proposals = steps(&mut fixture)?;
    let witness = fixture
        .cas
        .check_proof(&fixture.context, &proposals, &mut budget()?)?;
    let expected = ["assum:cas:hyp-strict-sign", "assum:cas:hyp-nonzero"];
    assert_eq!(labels(&fixture.cas, &witness)?, expected);
    assert!(witness.is_discharged());
    assert!(!witness.is_unconditional());
    let citations = fixture
        .cas
        .proof_citations(witness.proof(), &mut budget()?)?;
    let final_step = citations.last().ok_or("missing conclusion")?;
    assert_eq!(
        final_step
            .hypotheses
            .iter()
            .map(|used| used.kind.formal_schema().label)
            .collect::<Vec<_>>(),
        expected
    );
    let bytes = fixture.cas.export_witness(&witness, &mut budget()?)?;
    let mut receiver = Cas::new(budget()?.limits().clone())?;
    let replayed = receiver.inspect(&bytes, &mut budget()?)?;
    assert_eq!(labels(&receiver, &replayed)?, expected);
    assert!(!replayed.is_unconditional());
    let mut changed = witness.judgment().clone();
    changed.context.clone_from(&unassumed);
    assert!(
        fixture
            .cas
            .replay(&bytes, &changed, &mut budget()?)
            .is_err()
    );
    omitted_assumptions(&mut fixture, &unassumed, proposals)?;
    Ok(())
}

/// Rechecking without the assumed frame restores both nonzero residuals instead of proving an unconditional result.
fn omitted_assumptions(
    fixture: &mut Fixture,
    context: &Handle,
    mut proposals: Vec<ApplicationDraft>,
) -> TestResult {
    for proposal in &mut proposals {
        proposal.context.clone_from(context);
    }
    let conditional = fixture
        .cas
        .check_proof(context, &proposals, &mut budget()?)?;
    assert!(!conditional.is_unconditional());
    assert!(!conditional.is_discharged());
    assert_eq!(conditional.judgment().residuals.len(), 2);
    let citations = fixture
        .cas
        .proof_citations(conditional.proof(), &mut budget()?)?;
    assert_eq!(
        citations
            .last()
            .ok_or("missing conditional conclusion")?
            .residuals,
        [HypothesisKind::NonZero, HypothesisKind::NonZero]
    );
    Ok(())
}

/// A complete proof with neither used hypotheses nor open residuals reports that exact status.
/// ´test:integration:unconditional-witness-requires-no-assumptions´
#[test]
fn unconditional_witness_requires_no_assumptions() -> TestResult {
    let mut fixture = Fixture::new()?;
    let proposal = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Reflexivity,
        &fixture.x,
        &fixture.x,
    );
    let witness = fixture
        .cas
        .check_proof(&fixture.context, &[proposal], &mut budget()?)?;
    assert!(witness.is_unconditional());
    assert!(witness.is_discharged());
    assert_eq!(
        witness.context_consistency(),
        nrz_cas::request::ContextConsistency::Unknown
    );
    Ok(())
}

/// Every theorem or assumption head in either guide belongs to exactly one complete register entry.
/// ´test:integration:formal-register-heads-are-bijective´
#[test]
fn formal_register_heads_are_bijective() {
    let entries: Vec<_> = PRIMITIVE_SCHEMAS
        .iter()
        .map(|entry| &entry.lemma)
        .chain(&EMPTY_SCHEMAS)
        .chain(&nrz_cas::proof::CONSTANT_SCHEMAS)
        .chain(&nrz_cas::proof::RETAINED_SCHEMAS)
        .chain(&nrz_cas::enclosure::ENCLOSURE_SCHEMAS)
        .chain(&nrz_cas::decision::DECISION_SCHEMAS)
        .chain(&HYPOTHESIS_SCHEMAS)
        .chain(std::iter::once(&ANONYMOUS_LEMMA_SCHEMA))
        .collect();
    for document in [
        include_str!("../../README.md"),
        include_str!("../../../../templates/crates/cas/README.md.j2"),
    ] {
        let lines: Vec<_> = document.lines().collect();
        let heads: Vec<_> = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| {
                let theorem = line.starts_with("**Theorem (");
                let assumption = line.starts_with("**Assumption (");
                theorem || assumption
            })
            .collect();
        assert_eq!(heads.len(), entries.len());
        for schema in &entries {
            let matching: Vec<_> = heads
                .iter()
                .filter(|(_, line)| line.ends_with(&format!("· `{}`", schema.label)))
                .collect();
            assert_eq!(matching.len(), 1);
            for (index, _) in matching {
                let category = if HYPOTHESIS_SCHEMAS
                    .iter()
                    .any(|entry| entry.name == schema.name)
                {
                    "hypothesis"
                } else if PRIMITIVE_SCHEMAS
                    .iter()
                    .any(|entry| entry.lemma.name == schema.name)
                {
                    "primitive"
                } else {
                    "schema"
                };
                let expected = format!(
                    "Formal {category} `{}`, version {}. {} Required premises: {} Standing: {}",
                    schema.name,
                    schema.version,
                    schema.statement,
                    schema.premises,
                    schema.standing.description()
                );
                assert_eq!(lines.get(index + 2), Some(&expected.as_str()));
            }
        }
    }
}

/// Anonymous local constant evidence changes neither witness bytes nor independent replay.
/// ´test:integration:anonymous-constant-preserves-witness´
/// ´claim:cas:anonymous-constant-preserves-witness´
#[test]
fn anonymous_constant_preserves_witness() -> TestResult {
    let mut fixture = Fixture::new()?;
    let pi = fixture.term(Operator::Constant(nrz_cas::syntax::Constant::Pi), &[])?;
    let proposal = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Reflexivity,
        &pi,
        &pi,
    );
    let witness = fixture.cas.check_proof(
        &fixture.context,
        std::slice::from_ref(&proposal),
        &mut budget()?,
    )?;
    let mut before_work = budget()?;
    let before = fixture.cas.export_witness(&witness, &mut before_work)?;
    fixture
        .cas
        .prepare_constant(&fixture.context, &pi, &mut budget()?)?;
    let mut after_work = budget()?;
    assert_eq!(
        fixture.cas.export_witness(&witness, &mut after_work)?,
        before
    );
    assert_eq!(before_work.usage().work, after_work.usage().work);
    let mut receiver = Cas::new(budget()?.limits().clone())?;
    let replayed = receiver.inspect(&before, &mut budget()?)?;
    assert_eq!(receiver.export_witness(&replayed, &mut budget()?)?, before);
    assert_eq!(receiver.retained_judgment_count(), 0);
    Ok(())
}
