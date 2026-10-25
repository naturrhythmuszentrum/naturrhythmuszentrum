// SPDX-License-Identifier: AGPL-3.0-only
//! Certified signs discharge the original reciprocal guard while preserving source assumptions.

use super::super::decision_baseline::Example;
use super::{IntroductionFixture, frame_draft, membership, prepared_introduction};
use crate::testing::builders::admit;
use crate::testing::common::TestResult;
use crate::testing::readings::ledger;
use nrz_cas::domain::{ContextDraft, HypothesisRef};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::proof::{ApplicationDraft, Certificate, HypothesisKind, Primitive, Statement};
use nrz_cas::syntax::{Draft, Operator, PredicateKind};
use nrz_cas::{Cas, Fault, FaultKind, Handle, Reference};

struct SignFixture {
    source: IntroductionFixture,
    frame: Handle,
    steps: Vec<ApplicationDraft>,
}

fn prepared(positive: bool) -> TestResult<SignFixture> {
    let source = if positive {
        super::refusals::positive_fixture()?
    } else {
        prepared_introduction(Example::Sign, false)?
    };
    from_source(source)
}

fn from_source(mut fixture: IntroductionFixture) -> TestResult<SignFixture> {
    let source = &mut fixture.partition.source;
    let intro = source
        .cas
        .check_proof(&source.context, &fixture.steps, &mut ledger()?)?;
    let draft = frame_draft(&fixture, &intro);
    let source = &mut fixture.partition.source;
    let frame = admit(&mut source.cas, draft, &mut ledger()?)?;
    let coordinate = fixture.partition.question.coordinates[0].clone();
    let predicate = admit(
        &mut source.cas,
        Draft::Condition {
            header: Reference::Existing(source.header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::NonZero,
            arguments: vec![Reference::Existing(coordinate.clone())],
        },
        &mut ledger()?,
    )?;
    let domain = source.cas.admissible_region(&frame)?;
    let one = source.number(1, 1)?;
    let reciprocal = source.term(Operator::Div, &[one, coordinate])?;
    let original = crate::testing::witness::equality(
        &frame,
        &domain,
        Primitive::Reflexivity,
        &reciprocal,
        &reciprocal,
    );
    let consequence = ApplicationDraft {
        context: frame.clone(),
        rule: Primitive::RegionDecide,
        conclusion: Statement::PredicateFact {
            region: domain,
            predicate,
        },
        premises: vec![0],
        substitutions: Vec::new(),
        certificate: Certificate::DecisionNonZero(HypothesisRef {
            context: frame.clone(),
            slot: 0,
        }),
    };
    let discharge = ApplicationDraft {
        context: frame.clone(),
        rule: Primitive::ContextClose,
        conclusion: original.conclusion.clone(),
        premises: vec![2, 1],
        substitutions: Vec::new(),
        certificate: Certificate::DecisionDischarge,
    };
    let steps = vec![
        membership(&source.cas, &frame, &fixture.option)?,
        consequence,
        original,
        discharge,
    ];
    Ok(SignFixture {
        source: fixture,
        frame,
        steps,
    })
}

fn accepted(positive: bool) -> TestResult {
    let mut fixture = prepared(positive)?;
    let source = &mut fixture.source.partition.source;
    let original = source
        .cas
        .check_proof(&fixture.frame, &fixture.steps[2..3], &mut ledger()?)?;
    assert_eq!(original.judgment().residuals.len(), 1);
    assert_eq!(
        source
            .cas
            .predicate_hypothesis_kind(&original.judgment().residuals[0].predicate)?,
        HypothesisKind::NonZero
    );
    let witness = source
        .cas
        .check_proof(&fixture.frame, &fixture.steps, &mut ledger()?)?;
    assert!(witness.is_discharged());
    assert!(!witness.is_unconditional());
    assert_eq!(witness.judgment().statement, original.judgment().statement);
    assert_eq!(witness.hypotheses().len(), 2);
    let citations = source
        .cas
        .proof_citations(witness.proof(), &mut ledger()?)?;
    let schemas = citations
        .iter()
        .filter_map(|row| row.auxiliary_schema(&source.cas).transpose())
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        schemas.iter().map(|schema| schema.name).collect::<Vec<_>>(),
        [
            "decision.enclosure-membership",
            "decision.membership-nonzero",
            "decision.nonzero-discharge"
        ]
    );
    let final_step = citations.last().ok_or("discharge citation")?;
    let original_id = &final_step.premises[0].proof;
    let retained = citations
        .iter()
        .find(|row| &row.proof == original_id)
        .ok_or("original guard citation")?;
    assert_eq!(retained.residuals, [HypothesisKind::NonZero]);
    assert_eq!(final_step.residuals, [] as [HypothesisKind; 0]);
    let bytes = source.cas.export_witness(&witness, &mut ledger()?)?;
    let mut receiver = Cas::new(Limits::default())?;
    let replayed = receiver.inspect(&bytes, &mut ledger()?)?;
    assert!(replayed.is_discharged());
    assert_eq!(replayed.hypotheses().len(), 2);
    assert_eq!(receiver.export_witness(&replayed, &mut ledger()?)?, bytes);
    Ok(())
}

/// The driven negative source retains its original reciprocal and qualified source equality.
/// ´test:integration:decision-certified-sign-negative-reciprocal´
#[test]
fn decision_certified_sign_negative_reciprocal() -> TestResult {
    accepted(false)
}

/// The independently signed positive twin discharges the same exact nonzero obligation.
/// ´test:integration:decision-certified-sign-positive-reciprocal´
#[test]
fn decision_certified_sign_positive_reciprocal() -> TestResult {
    accepted(true)
}

fn refuses(fixture: &mut SignFixture) -> TestResult {
    let cas = &mut fixture.source.partition.source.cas;
    let before = cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = ledger()?;
    let fault = cas
        .check_proof(&fixture.frame, &fixture.steps, &mut budget)
        .err()
        .ok_or("invalid sign accepted")?;
    assert!(matches!(fault.kind, FaultKind::Invalid(_)));
    assert!(budget.usage().work > 0);
    assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
    Ok(())
}

/// Changed subjects, regions, slots and sibling frames cannot impersonate a certified sign.
/// ´test:integration:decision-certified-sign-refusals´
#[test]
fn decision_certified_sign_refusals() -> TestResult {
    for mutation in 0..5 {
        let mut fixture = prepared(false)?;
        let source = &mut fixture.source.partition.source;
        let sibling = admit(
            &mut source.cas,
            Draft::Context(ContextDraft::Local {
                parent: Reference::Existing(source.context.clone()),
                region: Reference::Existing(fixture.source.option.clone()),
                hypotheses: Vec::new(),
            }),
            &mut ledger()?,
        )?;
        match mutation {
            0 => {
                fixture.steps[1].certificate = Certificate::DecisionNonZero(HypothesisRef {
                    context: fixture.frame.clone(),
                    slot: 1,
                });
            }
            1 => {
                fixture.steps[1].certificate = Certificate::DecisionNonZero(HypothesisRef {
                    context: sibling,
                    slot: 0,
                });
            }
            2 => {
                fixture.steps[1].certificate = Certificate::DecisionNonZero(HypothesisRef {
                    context: source.context.clone(),
                    slot: 0,
                });
            }
            3 => {
                let Statement::PredicateFact { region, .. } = &mut fixture.steps[1].conclusion
                else {
                    return Err("fact".into());
                };
                region.clone_from(&source.ambient);
            }
            _ => {
                let unrelated = source.number(7, 1)?;
                let predicate = admit(
                    &mut source.cas,
                    Draft::Condition {
                        header: Reference::Existing(source.header.clone()),
                        scope: Vec::new(),
                        kind: PredicateKind::NonZero,
                        arguments: vec![Reference::Existing(unrelated)],
                    },
                    &mut ledger()?,
                )?;
                let Statement::PredicateFact {
                    predicate: target, ..
                } = &mut fixture.steps[1].conclusion
                else {
                    return Err("fact".into());
                };
                *target = predicate;
            }
        }
        refuses(&mut fixture)?;
    }
    Ok(())
}

/// Discharge preserves the original equality and requires its exact single open guard.
/// ´test:integration:decision-certified-sign-discharge-refusals´
#[test]
fn decision_certified_sign_discharge_refusals() -> TestResult {
    for mutation in 0..3 {
        let mut fixture = prepared(false)?;
        let source = &mut fixture.source.partition.source;
        let coordinate = fixture.source.partition.question.coordinates[0].clone();
        let domain = source.cas.admissible_region(&fixture.frame)?;
        let changed = crate::testing::witness::equality(
            &fixture.frame,
            &domain,
            Primitive::Reflexivity,
            &coordinate,
            &coordinate,
        );
        match mutation {
            0 => fixture.steps[3].conclusion = changed.conclusion,
            1 => fixture.steps[2] = changed,
            _ => fixture.steps[3].premises = vec![1, 2],
        }
        refuses(&mut fixture)?;
    }
    Ok(())
}

fn zero_fixture() -> TestResult<SignFixture> {
    let mut fixture = prepared_introduction(Example::Sign, false)?;
    let coordinate = fixture.partition.question.coordinates[0].clone();
    let zero = fixture.partition.source.number(0, 1)?;
    let domain = fixture.partition.source.ambient.clone();
    let predicates = super::source_context(
        &mut fixture.partition,
        &[(zero.clone(), (0, 1))],
        domain.clone(),
    )?;
    let context = fixture.partition.source.context.clone();
    let option = fixture
        .partition
        .source
        .guard(PredicateKind::Equal, &[coordinate, zero.clone()])?;
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
    let Certificate::Decision(evidence) = &mut fixture.steps[1].certificate else {
        return Err("decision".into());
    };
    evidence.cut.association = fixture.partition.arms[1].clone();
    evidence.cut.bounds[0].lower = super::rational(0, 1)?;
    evidence.cut.bounds[0].upper = super::rational(0, 1)?;
    evidence.enclosures[0].context = context;
    evidence.enclosures[0].target.clone_from(&zero);
    evidence.enclosures[0].lower = super::rational(0, 1)?;
    evidence.enclosures[0].upper = super::rational(0, 1)?;
    evidence.enclosures[0].origin = nrz_cas::decision::EnclosureOrigin::ExactRational {
        expression: zero,
        value: super::rational(0, 1)?,
    };
    from_source(fixture)
}

/// A certified singleton zero option never implies nonzero.
/// ´test:integration:decision-certified-sign-zero-refusal´
#[test]
fn decision_certified_sign_zero_refusal() -> TestResult {
    refuses(&mut zero_fixture()?)
}

fn limits(kind: BudgetKind, amount: u64) -> Limits {
    let mut result = Limits::default();
    match kind {
        BudgetKind::Work => result.work = amount,
        BudgetKind::StagedPayloadBytes => result.staged_payload_bytes = amount,
        BudgetKind::PayloadBytes => result.payload_bytes = amount,
        BudgetKind::OutputBytes => result.output_bytes = amount,
        _ => {}
    }
    result
}

fn attempt(positive: bool, owner: usize, kind: BudgetKind, amount: u64) -> TestResult<bool> {
    let mut fixture = prepared(positive)?;
    let source = &mut fixture.source.partition.source;
    let mut budget = Budget::new(limits(kind, amount))?;
    let before = source.cas.counts();
    let result = if owner < 2 {
        let steps = if owner == 0 {
            &fixture.steps[..2]
        } else {
            &fixture.steps[..]
        };
        source
            .cas
            .check_proof(&fixture.frame, steps, &mut budget)
            .map(|_| ())
    } else {
        let witness = source
            .cas
            .check_proof(&fixture.frame, &fixture.steps, &mut ledger()?)?;
        let bytes = source.cas.export_witness(&witness, &mut ledger()?)?;
        let mut receiver = Cas::new(Limits::default())?;
        let empty = receiver.counts();
        let result = receiver.inspect(&bytes, &mut budget).map(|_| ());
        if result.is_err() {
            assert_eq!(receiver.counts(), empty);
        }
        return exhausted(result, kind);
    };
    if result.is_err() {
        assert_eq!(source.cas.counts(), before);
    }
    exhausted(result, kind)
}

fn exhausted(result: Result<(), Fault>, kind: BudgetKind) -> TestResult<bool> {
    match result {
        Ok(()) => Ok(true),
        Err(fault) if matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind) => {
            Ok(false)
        }
        Err(fault) => Err(fault.into()),
    }
}

/// Exact consequence, original-guard discharge and replay resource boundaries.
/// ´test:integration:decision-certified-sign-boundaries´
#[test]
fn decision_certified_sign_boundaries() -> TestResult {
    for (positive, owners) in [
        (
            false,
            [
                [73_788, 50_224, 25_265, 0],
                [96_167, 54_960, 27_772, 0],
                [284_574, 54_384, 22_980, 800],
            ],
        ),
        (
            true,
            [
                [75_379, 51_504, 26_885, 0],
                [97_842, 56_240, 29_392, 0],
                [294_810, 55_664, 23_482, 800],
            ],
        ),
    ] {
        for (owner, amounts) in owners.into_iter().enumerate() {
            for (kind, amount) in [
                BudgetKind::Work,
                BudgetKind::StagedPayloadBytes,
                BudgetKind::PayloadBytes,
                BudgetKind::OutputBytes,
            ]
            .into_iter()
            .zip(amounts)
            {
                assert!(
                    attempt(positive, owner, kind, amount)?,
                    "{positive}:{owner}:{kind:?}"
                );
                if amount != 0 {
                    assert!(
                        !attempt(positive, owner, kind, amount - 1)?,
                        "{positive}:{owner}:{kind:?}"
                    );
                }
            }
        }
    }
    Ok(())
}

/// Auxiliary versions and qualified sign references are independently reconstructed on import.
/// ´test:integration:decision-certified-sign-wire-versions´
#[test]
fn decision_certified_sign_wire_versions() -> TestResult {
    let mut fixture = prepared(false)?;
    let cas = &mut fixture.source.partition.source.cas;
    let proof = cas.check_proof(&fixture.frame, &fixture.steps, &mut ledger()?)?;
    let bytes = cas.export_witness(&proof, &mut ledger()?)?;
    for tag in [15, 16] {
        super::wire::unknown_version(&bytes, tag)?;
    }
    Ok(())
}

/// An ordinary local sign assumption cannot invoke the certified consequence schema.
/// ´test:integration:decision-certified-sign-ordinary-refusal´
#[test]
fn decision_certified_sign_ordinary_refusal() -> TestResult {
    let mut fixture = prepared(false)?;
    let source = &mut fixture.source.partition.source;
    let ordinary = admit(
        &mut source.cas,
        Draft::Context(ContextDraft::Local {
            parent: Reference::Existing(source.context.clone()),
            region: Reference::Existing(fixture.source.option.clone()),
            hypotheses: Vec::new(),
        }),
        &mut ledger()?,
    )?;
    fixture.frame.clone_from(&ordinary);
    for step in &mut fixture.steps {
        step.context.clone_from(&ordinary);
    }
    fixture.steps[0] = membership(&source.cas, &ordinary, &fixture.source.option)?;
    fixture.steps[1].certificate = Certificate::DecisionNonZero(HypothesisRef {
        context: ordinary,
        slot: 0,
    });
    refuses(&mut fixture)
}
