// SPDX-License-Identifier: AGPL-3.0-only
//! Certificate and context identity mutations cannot publish a membership prefix.

use super::super::decision_baseline::Example;
use super::{IntroductionFixture, frame_draft, membership, prepared_introduction};
use crate::testing::builders::admit;
use crate::testing::common::TestResult;
use crate::testing::readings::ledger;
use nrz_cas::decision::{DecisionIntroduction, EnclosureOrigin};
use nrz_cas::domain::{ContextDraft, HypothesisRef};
use nrz_cas::proof::{ApplicationDraft, Certificate, HypothesisKind, Primitive, Statement};
use nrz_cas::syntax::Draft;
use nrz_cas::{Handle, Reference};

fn evidence(fixture: &mut IntroductionFixture) -> TestResult<&mut DecisionIntroduction> {
    let Certificate::Decision(value) =
        &mut fixture.steps.last_mut().ok_or("introduction")?.certificate
    else {
        return Err("decision certificate".into());
    };
    Ok(value)
}

fn refuses(fixture: &mut IntroductionFixture) -> TestResult {
    let source = &mut fixture.partition.source;
    let before = source.cas.canonical_snapshot(&mut ledger()?)?;
    let fault = source
        .cas
        .check_proof(&source.context, &fixture.steps, &mut ledger()?)
        .err()
        .ok_or("invalid introduction accepted")?;
    assert!(matches!(fault.kind, nrz_cas::FaultKind::Invalid(_)));
    assert_eq!(source.cas.canonical_snapshot(&mut ledger()?)?, before);
    Ok(())
}

/// Source equalities, original replay targets, complete partitions and exact boxes are indispensable.
/// ´test:integration:decision-introduction-rejects-incomplete-evidence´
#[test]
fn decision_introduction_rejects_incomplete_evidence() -> TestResult {
    for mutation in 0..9 {
        let mut fixture = prepared_introduction(Example::Sign, false)?;
        let unrelated = fixture.partition.source.number(75, 1)?;
        match mutation {
            0 => evidence(&mut fixture)?.enclosures[0].target = unrelated,
            1 => evidence(&mut fixture)?.enclosures[0].origin = EnclosureOrigin::Unverified,
            2 => {
                evidence(&mut fixture)?.enclosures.clear();
            }
            3 => {
                fixture.steps.remove(0);
                fixture.steps[0].premises.clear();
            }
            4 => evidence(&mut fixture)?.partition.coordinates[0] = unrelated,
            5 => evidence(&mut fixture)?.cut.association = fixture.partition.arms[2].clone(),
            6 => {
                evidence(&mut fixture)?.enclosures[0].lower =
                    super::super::decision_partitions::rational(-76, 1)?;
            }
            7 => evidence(&mut fixture)?.enclosures[0].origin = EnclosureOrigin::ScalarKernel,
            _ => evidence(&mut fixture)?.enclosures[0].origin = EnclosureOrigin::MatrixKernel,
        }
        refuses(&mut fixture)?;
    }
    Ok(())
}

/// A widened family or claimed option does not inherit the narrower checked introduction.
/// ´test:integration:decision-introduction-rejects-widening´
#[test]
fn decision_introduction_rejects_widening() -> TestResult {
    let mut fixture = prepared_introduction(Example::Integer, true)?;
    evidence(&mut fixture)?.enclosures[0].input_family = fixture.partition.source.ambient.clone();
    refuses(&mut fixture)?;
    let mut fixture = super::prepared_introduction(Example::Sign, false)?;
    let Statement::Subset { superset, .. } =
        &mut fixture.steps.last_mut().ok_or("introduction")?.conclusion
    else {
        return Err("subset".into());
    };
    *superset = fixture.partition.source.ambient.clone();
    refuses(&mut fixture)
}

fn admitted() -> TestResult<(IntroductionFixture, Handle)> {
    let mut fixture = prepared_introduction(Example::Sign, false)?;
    let source = &mut fixture.partition.source;
    let proof = source
        .cas
        .check_proof(&source.context, &fixture.steps, &mut ledger()?)?;
    let draft = frame_draft(&fixture, &proof);
    let frame = admit(&mut fixture.partition.source.cas, draft, &mut ledger()?)?;
    Ok((fixture, frame))
}

/// Source proof, sibling frame and escaped slot cannot impersonate an introduced membership.
/// ´test:integration:decision-certified-frame-refusals´
#[test]
fn decision_certified_frame_refusals() -> TestResult {
    let (mut fixture, frame) = admitted()?;
    let source = &mut fixture.partition.source;
    let sibling = admit(
        &mut source.cas,
        Draft::Context(ContextDraft::Local {
            parent: Reference::Existing(source.context.clone()),
            region: Reference::Existing(fixture.option.clone()),
            hypotheses: Vec::new(),
        }),
        &mut ledger()?,
    )?;
    assert_ne!(frame, sibling);
    assert_eq!(
        HypothesisRef {
            context: sibling.clone(),
            slot: 0
        }
        .qualified(&source.cas)?
        .kind,
        HypothesisKind::RegionMembership
    );
    for (context, slot) in [
        (frame.clone(), 1),
        (sibling, 0),
        (source.context.clone(), 0),
    ] {
        let mut step = membership(&source.cas, &frame, &fixture.option)?;
        step.context = context.clone();
        if let Certificate::Hypothesis(reference) = &mut step.certificate {
            reference.slot = slot;
        }
        let before = source.cas.counts();
        assert!(
            source
                .cas
                .check_proof(&context, &[step], &mut ledger()?)
                .is_err()
        );
        assert_eq!(source.cas.counts(), before);
    }
    let proof = source
        .cas
        .check_proof(&source.context, &fixture.steps[..1], &mut ledger()?)?;
    let draft = Draft::Context(ContextDraft::Certified {
        parent: Reference::Existing(source.context.clone()),
        region: Reference::Existing(fixture.option),
        introduction: Reference::Existing(proof.proof().clone()),
    });
    let before = source.cas.counts();
    assert!(source.cas.admit(&[draft], &mut ledger()?).is_err());
    assert_eq!(source.cas.counts(), before);
    Ok(())
}

/// A membership certificate cannot be repurposed as a cover for elimination.
/// ´test:integration:decision-introduction-is-not-a-cover´
#[test]
fn decision_introduction_is_not_a_cover() -> TestResult {
    let mut fixture = prepared_introduction(Example::Sign, false)?;
    fixture.steps.last_mut().ok_or("introduction")?.conclusion = Statement::Cover {
        domain: fixture.partition.source.ambient.clone(),
        members: vec![fixture.option.clone()],
    };
    refuses(&mut fixture)
}

/// The checked frame and source assumptions survive a fresh portable proof lifecycle and citations.
/// ´test:integration:decision-certified-citation-closure´
#[test]
fn decision_certified_citation_closure() -> TestResult {
    let (mut fixture, frame) = admitted()?;
    let source = &mut fixture.partition.source;
    let step = membership(&source.cas, &frame, &fixture.option)?;
    let proof = source.cas.check_proof(&frame, &[step], &mut ledger()?)?;
    assert!(proof.is_discharged());
    assert!(!proof.is_unconditional());
    assert_eq!(proof.hypotheses().len(), 2);
    let citations = source.cas.proof_citations(proof.proof(), &mut ledger()?)?;
    assert_eq!(citations.len(), 3);
    let introduction = &citations[1];
    assert_eq!(
        introduction
            .auxiliary_schema(&source.cas)?
            .ok_or("schema")?
            .name,
        "decision.enclosure-membership"
    );
    assert_eq!(citations[2].premises[0].proof, introduction.proof);
    assert_eq!(
        citations[2].premises[0].kind,
        HypothesisKind::CertifiedRegionMembership
    );
    assert_eq!(
        HypothesisRef {
            context: frame,
            slot: 0
        }
        .qualified(&source.cas)?
        .kind,
        HypothesisKind::CertifiedRegionMembership
    );
    let bytes = source.cas.export_witness(&proof, &mut ledger()?)?;
    let mut fresh = nrz_cas::Cas::new(ledger()?.limits().clone())?;
    let replayed = fresh.inspect(&bytes, &mut ledger()?)?;
    assert_eq!(fresh.export_witness(&replayed, &mut ledger()?)?, bytes);
    assert_eq!(
        fresh
            .proof_citations(replayed.proof(), &mut ledger()?)?
            .len(),
        3
    );
    Ok(())
}

/// Equal conclusions with different source proof roots remain different certified frames.
/// ´test:integration:decision-certified-proof-identity´
#[test]
fn decision_certified_proof_identity() -> TestResult {
    let (mut fixture, first_frame) = admitted()?;
    let source = &mut fixture.partition.source;
    let family = source.cas.admissible_region(&source.context)?;
    let mut introduction = fixture.steps[1].clone();
    introduction.premises = vec![2];
    let reflexive = ApplicationDraft {
        context: source.context.clone(),
        rule: Primitive::RegionConstruct,
        conclusion: Statement::Subset {
            subset: family.clone(),
            superset: family,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    };
    let transported = ApplicationDraft {
        context: source.context.clone(),
        rule: Primitive::ContextWeaken,
        conclusion: fixture.steps[0].conclusion.clone(),
        premises: vec![0, 1],
        substitutions: Vec::new(),
        certificate: Certificate::None,
    };
    let proof = source.cas.check_proof(
        &source.context,
        &[
            fixture.steps[0].clone(),
            reflexive,
            transported,
            introduction,
        ],
        &mut ledger()?,
    )?;
    let draft = frame_draft(&fixture, &proof);
    let second_frame = admit(
        &mut fixture.partition.source.cas,
        draft.clone(),
        &mut ledger()?,
    )?;
    assert_ne!(second_frame, first_frame);
    let source = &mut fixture.partition.source;
    assert_eq!(
        source.cas.admissible_region(&first_frame)?,
        source.cas.admissible_region(&second_frame)?
    );
    let before = source.cas.counts();
    assert_eq!(admit(&mut source.cas, draft, &mut ledger()?)?, second_frame);
    assert_eq!(source.cas.counts(), before);
    Ok(())
}

/// The exact auxiliary register and its guide entries describe executable checked instances.
/// ´test:integration:decision-introduction-register´
#[test]
fn decision_introduction_register() -> TestResult {
    let schemas = &nrz_cas::decision::DECISION_SCHEMAS;
    assert_eq!(schemas[0].name, "decision.enclosure-membership");
    assert_eq!(schemas[0].version, 1);
    for document in [
        include_str!("../../../README.md"),
        include_str!("../../../../../templates/crates/cas/README.md.j2"),
    ] {
        for schema in schemas {
            let paragraph = format!(
                "Formal schema `{}`, version {}. {} Required premises: {} Standing: {}",
                schema.name,
                schema.version,
                schema.statement,
                schema.premises,
                schema.standing.description()
            );
            assert_eq!(document.matches(&paragraph).count(), 1);
            assert_eq!(
                document
                    .lines()
                    .filter(|line| line.starts_with("**")
                        && line.ends_with(&format!("· `{}`", schema.label)))
                    .count(),
                1
            );
        }
    }
    let (mut fixture, frame) = admitted()?;
    let source = &mut fixture.partition.source;
    let step = membership(&source.cas, &frame, &fixture.option)?;
    let witness = source.cas.check_proof(&frame, &[step], &mut ledger()?)?;
    let rows = source
        .cas
        .proof_citations(witness.proof(), &mut ledger()?)?;
    assert_eq!(rows[1].auxiliary_schema(&source.cas)?, Some(&schemas[0]));
    Ok(())
}

pub(super) fn positive_fixture() -> TestResult<IntroductionFixture> {
    let mut fixture = super::prepared_introduction(Example::Sign, false)?;
    let target = evidence(&mut fixture)?.enclosures[0].target.clone();
    let target = fixture
        .partition
        .source
        .term(nrz_cas::syntax::Operator::Neg, &[target])?;
    let domain = fixture.partition.source.ambient.clone();
    let predicates = super::source_context(
        &mut fixture.partition,
        &[(target.clone(), (75, 1))],
        domain.clone(),
    )?;
    let context = fixture.partition.source.context.clone();
    let coordinate = fixture.partition.question.coordinates[0].clone();
    let option = fixture
        .partition
        .source
        .guard(nrz_cas::syntax::PredicateKind::Positive, &[coordinate])?;
    fixture.option.clone_from(&option);
    fixture.steps[0].context.clone_from(&context);
    fixture.steps[0].conclusion = Statement::PredicateFact {
        region: domain.clone(),
        predicate: predicates[0].clone(),
    };
    fixture.steps[0].certificate = Certificate::Hypothesis(HypothesisRef {
        context: context.clone(),
        slot: 0,
    });
    fixture.steps[1].context.clone_from(&context);
    fixture.steps[1].conclusion = Statement::Subset {
        subset: domain,
        superset: option,
    };
    let association = fixture.partition.arms[2].clone();
    let proof = evidence(&mut fixture)?;
    proof.cut.association = association;
    proof.cut.bounds[0].lower = super::super::decision_partitions::rational(299, 4)?;
    proof.cut.bounds[0].upper = super::super::decision_partitions::rational(301, 4)?;
    proof.enclosures[0].context = context;
    proof.enclosures[0].target.clone_from(&target);
    proof.enclosures[0].origin = EnclosureOrigin::ExactRational {
        expression: target,
        value: super::super::decision_partitions::rational(75, 1)?,
    };
    proof.enclosures[0]
        .lower
        .clone_from(&proof.cut.bounds[0].lower);
    proof.enclosures[0]
        .upper
        .clone_from(&proof.cut.bounds[0].upper);
    Ok(fixture)
}

/// The positive twin uses a different exact original target and retains its source equality.
/// ´test:integration:decision-introduction-positive-twin´
#[test]
fn decision_introduction_positive_twin() -> TestResult {
    let mut fixture = positive_fixture()?;
    let source = &mut fixture.partition.source;
    let proof = source
        .cas
        .check_proof(&source.context, &fixture.steps, &mut ledger()?)?;
    assert!(proof.is_discharged());
    assert_eq!(
        proof.hypotheses(),
        &[HypothesisRef {
            context: source.context.clone(),
            slot: 0
        }]
    );
    let draft = frame_draft(&fixture, &proof);
    let source = &mut fixture.partition.source;
    let frame = admit(&mut source.cas, draft, &mut ledger()?)?;
    assert_eq!(
        HypothesisRef {
            context: frame,
            slot: 0
        }
        .qualified(&source.cas)?
        .kind,
        HypothesisKind::CertifiedRegionMembership
    );
    Ok(())
}

/// Merely retaining a certified sign frame does not implicitly discharge an original reciprocal.
/// ´test:integration:decision-certified-sign-needs-consequence´
#[test]
fn decision_certified_sign_needs_consequence() -> TestResult {
    let (mut fixture, frame) = admitted()?;
    let source = &mut fixture.partition.source;
    let coordinate = fixture.partition.question.coordinates[0].clone();
    let one = source.number(1, 1)?;
    let reciprocal = source.term(nrz_cas::syntax::Operator::Div, &[one, coordinate])?;
    let domain = source.cas.admissible_region(&frame)?;
    let step = crate::testing::witness::equality(
        &frame,
        &domain,
        Primitive::Reflexivity,
        &reciprocal,
        &reciprocal,
    );
    let proof = source.cas.check_proof(&frame, &[step], &mut ledger()?)?;
    assert!(!proof.is_discharged());
    assert_eq!(proof.judgment().residuals.len(), 1);
    assert_eq!(
        source
            .cas
            .predicate_hypothesis_kind(&proof.judgment().residuals[0].predicate)?,
        HypothesisKind::NonZero
    );
    Ok(())
}

/// Parent and option identities cannot be changed while retaining an otherwise valid introduction.
/// ´test:integration:decision-certified-frame-contract´
#[test]
fn decision_certified_frame_contract() -> TestResult {
    let (mut fixture, frame) = admitted()?;
    let source = &mut fixture.partition.source;
    let intro = source
        .cas
        .check_proof(&source.context, &fixture.steps, &mut ledger()?)?;
    for (parent, region) in [
        (frame, fixture.option),
        (source.context.clone(), source.ambient.clone()),
    ] {
        let draft = Draft::Context(ContextDraft::Certified {
            parent: Reference::Existing(parent),
            region: Reference::Existing(region),
            introduction: Reference::Existing(intro.proof().clone()),
        });
        let before = source.cas.counts();
        let fault = source
            .cas
            .admit(&[draft], &mut ledger()?)
            .err()
            .ok_or("changed frame accepted")?;
        assert_eq!(
            fault.kind,
            nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::EndpointMismatch)
        );
        assert_eq!(source.cas.counts(), before);
    }
    Ok(())
}
