// SPDX-License-Identifier: AGPL-3.0-only
//! Decision introductions retain exact source proofs, local identity and portable replay.

use super::decision_baseline::Example;
use super::decision_partitions::{PartitionFixture, cut, prepared, rational};
use crate::testing::builders::admit;
use crate::testing::common::TestResult;
use crate::testing::readings::ledger;
use nrz_cas::decision::{DecisionIntroduction, EnclosureOrigin, RationalEnclosure};
use nrz_cas::domain::{AssumptionDraft, ContextDraft, HypothesisRef};
use nrz_cas::proof::{ApplicationDraft, Certificate, Primitive, Statement, VerifiedWitness};
use nrz_cas::syntax::{Draft, Operator, PredicateKind};
use nrz_cas::{Cas, Handle, Reference};

#[path = "decision_introductions/refusals.rs"]
mod refusals;
#[path = "decision_introductions/resources.rs"]
mod resources;
#[path = "decision_introductions/selection.rs"]
mod selection;
#[path = "decision_introductions/sign.rs"]
mod sign;
#[path = "decision_introductions/wire.rs"]
mod wire;

struct IntroductionFixture {
    partition: PartitionFixture,
    steps: Vec<ApplicationDraft>,
    option: Handle,
}

fn replay_targets(
    fixture: &mut PartitionFixture,
    example: Example,
    surrogate: bool,
) -> TestResult<Vec<(Handle, (i64, u64))>> {
    let source = &mut fixture.source;
    Ok(match example {
        Example::Sign => vec![(super::decision_enclosures::driven(source)?, (-75, 1))],
        Example::Phase => vec![
            (source.number(-3, 2)?, (-3, 2)),
            (source.number(3, 8)?, (3, 8)),
        ],
        Example::Integer if surrogate => vec![(source.number(51, 16)?, (51, 16))],
        Example::Integer => {
            let pi = source.term(Operator::Constant(nrz_cas::syntax::Constant::Pi), &[])?;
            let ten = source.number(10, 1)?;
            vec![(source.term(Operator::Div, &[ten, pi])?, (51, 16))]
        }
    })
}

fn source_context(
    fixture: &mut PartitionFixture,
    targets: &[(Handle, (i64, u64))],
    domain: Handle,
) -> TestResult<Vec<Handle>> {
    let mut predicates = Vec::new();
    let mut assumptions = Vec::new();
    for (index, (coordinate, (target, _))) in
        fixture.question.coordinates.iter().zip(targets).enumerate()
    {
        let predicate = admit(
            &mut fixture.source.cas,
            Draft::Condition {
                header: Reference::Existing(fixture.source.header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::Equal,
                arguments: vec![
                    Reference::Existing(coordinate.clone()),
                    Reference::Existing(target.clone()),
                ],
            },
            &mut ledger()?,
        )?;
        assumptions.push(AssumptionDraft {
            key: format!("source-{index}"),
            condition: Reference::Existing(predicate.clone()),
        });
        predicates.push(predicate);
    }
    fixture.source.context = admit(
        &mut fixture.source.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(fixture.source.header.clone()),
            admissible: Reference::Existing(domain),
            assumptions,
        }),
        &mut ledger()?,
    )?;
    Ok(predicates)
}

fn prepared_introduction(example: Example, surrogate: bool) -> TestResult<IntroductionFixture> {
    let mut partition = prepared(example)?;
    let targets = replay_targets(&mut partition, example, surrogate)?;
    let (_, option) = partition.source.regions(example)?;
    let domain = if matches!(example, Example::Integer) {
        partition
            .source
            .interval(&partition.question.coordinates[0], (0, 1), (5, 1), true)?
    } else {
        partition.source.ambient.clone()
    };
    let predicates = source_context(&mut partition, &targets, domain.clone())?;
    let context = partition.source.context.clone();
    let cut = cut(&partition, example)?;
    let mut steps = Vec::new();
    let mut enclosures = Vec::new();
    for (index, ((target, value), predicate)) in targets.into_iter().zip(predicates).enumerate() {
        steps.push(ApplicationDraft {
            context: context.clone(),
            rule: Primitive::ContextHypothesis,
            conclusion: Statement::PredicateFact {
                region: domain.clone(),
                predicate,
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::Hypothesis(HypothesisRef {
                context: context.clone(),
                slot: u32::try_from(index)?,
            }),
        });
        enclosures.push(RationalEnclosure {
            origin: EnclosureOrigin::ExactRational {
                expression: target.clone(),
                value: rational(value.0, value.1)?,
            },
            target,
            context: context.clone(),
            input_family: domain.clone(),
            lower: cut.bounds[index].lower.clone(),
            upper: cut.bounds[index].upper.clone(),
        });
    }
    steps.push(ApplicationDraft {
        context,
        rule: Primitive::RegionDecide,
        conclusion: Statement::Subset {
            subset: domain,
            superset: option.clone(),
        },
        premises: (0..u32::try_from(steps.len())?).collect(),
        substitutions: Vec::new(),
        certificate: Certificate::Decision(Box::new(DecisionIntroduction {
            partition: partition.question.clone(),
            cut,
            enclosures,
        })),
    });
    Ok(IntroductionFixture {
        partition,
        steps,
        option,
    })
}

fn frame_draft(fixture: &IntroductionFixture, proof: &VerifiedWitness) -> Draft {
    Draft::Context(ContextDraft::Certified {
        parent: Reference::Existing(fixture.partition.source.context.clone()),
        region: Reference::Existing(fixture.option.clone()),
        introduction: Reference::Existing(proof.proof().clone()),
    })
}

fn membership(cas: &Cas, frame: &Handle, option: &Handle) -> TestResult<ApplicationDraft> {
    Ok(ApplicationDraft {
        context: frame.clone(),
        rule: Primitive::ContextHypothesis,
        conclusion: Statement::Subset {
            subset: cas.admissible_region(frame)?,
            superset: option.clone(),
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Hypothesis(HypothesisRef {
            context: frame.clone(),
            slot: 0,
        }),
    })
}

type CostRow = (&'static str, u64, u64, usize);

fn lifecycle(example: Example, expected: &[CostRow]) -> TestResult {
    let mut fixture = prepared_introduction(example, true)?;
    let source = &mut fixture.partition.source;
    let before = source.cas.counts();
    let mut budget = ledger()?;
    let intro = source
        .cas
        .check_proof(&source.context, &fixture.steps, &mut budget)?;
    let mut rows = vec![(
        "introduction",
        budget.usage().work,
        source.cas.counts().payload_bytes - before.payload_bytes,
        intro.exported_byte_len(),
    )];
    let draft = frame_draft(&fixture, &intro);
    let source = &mut fixture.partition.source;
    let before = source.cas.counts();
    let mut budget = ledger()?;
    let frame = admit(&mut source.cas, draft.clone(), &mut budget)?;
    rows.push((
        "frame",
        budget.usage().work,
        source.cas.counts().payload_bytes - before.payload_bytes,
        0,
    ));
    let before = source.cas.counts();
    let mut budget = ledger()?;
    let repeated = admit(&mut source.cas, draft, &mut budget)?;
    assert_eq!(repeated, frame);
    assert_eq!(source.cas.counts(), before);
    rows.push(("interned", budget.usage().work, 0, 0));
    membership_lifecycle(&mut fixture, &frame, &mut rows)?;
    assert_eq!(rows, expected);
    Ok(())
}

fn membership_lifecycle(
    fixture: &mut IntroductionFixture,
    frame: &Handle,
    rows: &mut Vec<CostRow>,
) -> TestResult {
    let source = &mut fixture.partition.source;
    let step = membership(&source.cas, frame, &fixture.option)?;
    let before = source.cas.counts();
    let mut budget = ledger()?;
    let member = source.cas.check_proof(frame, &[step], &mut budget)?;
    assert!(member.is_discharged());
    assert!(!member.is_unconditional());
    assert_eq!(member.hypotheses().len(), fixture.steps.len());
    rows.push((
        "membership",
        budget.usage().work,
        source.cas.counts().payload_bytes - before.payload_bytes,
        member.exported_byte_len(),
    ));
    let bytes = source.cas.export_witness(&member, &mut ledger()?)?;
    let mut receiver = Cas::new(ledger()?.limits().clone())?;
    let mut budget = ledger()?;
    let imported = receiver.inspect(&bytes, &mut budget)?;
    assert_eq!(imported.hypotheses().len(), member.hypotheses().len());
    assert_eq!(receiver.export_witness(&imported, &mut ledger()?)?, bytes);
    rows.push((
        "import",
        budget.usage().work,
        receiver.counts().payload_bytes,
        imported.exported_byte_len(),
    ));
    Ok(())
}

/// Exact lifecycle costs for the rational surrogate cycle cell.
/// ´test:integration:decision-introduction-lifecycle-integer´
#[test]
fn decision_introduction_lifecycle_integer() -> TestResult {
    lifecycle(
        Example::Integer,
        &[
            ("introduction", 322_565, 2_820, 2_775),
            ("frame", 1_648, 1_255, 0),
            ("interned", 1_646, 0, 0),
            ("membership", 113_794, 1_060, 2_927),
            ("import", 538_289, 33_902, 2_927),
        ],
    )
}

/// Exact lifecycle costs for the original driven sign target.
/// ´test:integration:decision-introduction-lifecycle-sign´
#[test]
fn decision_introduction_lifecycle_sign() -> TestResult {
    lifecycle(
        Example::Sign,
        &[
            ("introduction", 147_865, 2_640, 1_478),
            ("frame", 1_648, 1_255, 0),
            ("interned", 1_646, 0, 0),
            ("membership", 65_237, 1_060, 1_630),
            ("import", 237_975, 18_368, 1_630),
        ],
    )
}

/// Exact lifecycle costs for both nominal phase targets.
/// ´test:integration:decision-introduction-lifecycle-phase´
#[test]
fn decision_introduction_lifecycle_phase() -> TestResult {
    lifecycle(
        Example::Phase,
        &[
            ("introduction", 195_044, 4_141, 2_255),
            ("frame", 2_402, 1_426, 0),
            ("interned", 2_400, 0, 0),
            ("membership", 100_405, 1_111, 2_402),
            ("import", 359_124, 28_454, 2_402),
        ],
    )
}

/// The actual cycle expression has no rational replay origin and publishes nothing.
/// ´test:integration:decision-introduction-cycle-refusal´
#[test]
fn decision_introduction_cycle_refusal() -> TestResult {
    let mut fixture = prepared_introduction(Example::Integer, false)?;
    let source = &mut fixture.partition.source;
    let before = source.cas.counts();
    let mut budget = ledger()?;
    let fault = source
        .cas
        .check_proof(&source.context, &fixture.steps, &mut budget)
        .err()
        .ok_or("actual cycle target accepted")?;
    assert_eq!(
        fault.kind,
        nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::UnsupportedCertificate)
    );
    assert_eq!(source.cas.counts(), before);
    assert_eq!(budget.usage().work, 237_390);
    Ok(())
}
