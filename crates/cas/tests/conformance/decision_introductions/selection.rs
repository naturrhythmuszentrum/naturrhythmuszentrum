// SPDX-License-Identifier: AGPL-3.0-only
//! Native selection consumes the introduced local membership through its existing refinement.

use super::super::decision_baseline::Example;
use super::{frame_draft, membership, prepared_introduction};
use crate::testing::builders::admit;
use crate::testing::common::TestResult;
use crate::testing::readings::ledger;
use nrz_cas::limits::LimitsOverride;
use nrz_cas::proof::{HypothesisKind, Statement};
use nrz_cas::request::{CoveredInput, Outcome, Request, Task};

fn request(example: Example) -> TestResult<(super::IntroductionFixture, Request)> {
    from_fixture(prepared_introduction(example, true)?)
}

fn from_fixture(
    mut fixture: super::IntroductionFixture,
) -> TestResult<(super::IntroductionFixture, Request)> {
    let source = &mut fixture.partition.source;
    let proof = source
        .cas
        .check_proof(&source.context, &fixture.steps, &mut ledger()?)?;
    let draft = frame_draft(&fixture, &proof);
    let source = &mut fixture.partition.source;
    let frame = admit(&mut source.cas, draft, &mut ledger()?)?;
    let step = membership(&source.cas, &frame, &fixture.option)?;
    let member = source.cas.check_proof(&frame, &[step], &mut ledger()?)?;
    let function = &fixture.partition.question.function;
    let root = source
        .cas
        .materialize_derived(function, function, &mut ledger()?)?;
    let receipt = source.cas.walk_derived(&root, &mut ledger()?)?.receipt;
    let request = Request {
        task: Task::Select {
            region: source.cas.admissible_region(&frame)?,
        },
        context: frame,
        inputs: vec![CoveredInput { root, receipt }],
        premises: vec![member],
        limits: LimitsOverride::default(),
    };
    Ok((fixture, request))
}

/// Literal selection costs retain qualified source hypotheses on success.
/// ´test:integration:decision-certified-selection-costs´
#[test]
fn decision_certified_selection_costs() -> TestResult {
    let mut rows = Vec::new();
    for example in [Example::Integer, Example::Sign, Example::Phase] {
        let (mut fixture, request) = request(example)?;
        let source = &mut fixture.partition.source;
        let outcome = source.cas.run(&request);
        if let Outcome::Proved { result, .. } = &outcome {
            let Statement::Equality { domain, right, .. } = &result.witness.judgment().statement
            else {
                return Err("selected equality".into());
            };
            assert_eq!(*domain, source.cas.admissible_region(&request.context)?);
            if matches!(example, Example::Integer) {
                let (operator, sort) = source.cas.expression(right)?;
                assert_eq!(
                    *sort,
                    nrz_cas::syntax::Sort::Value(nrz_cas::syntax::AtomicSort::Integer)
                );
                assert_eq!(
                    *operator,
                    nrz_cas::syntax::Operator::Rational(
                        super::super::decision_partitions::rational(3, 1)?
                    )
                );
                assert_eq!(source.cas.scalar_ambient(right)?, None);
            }
            let kinds = result
                .witness
                .hypotheses()
                .iter()
                .map(|used| used.qualified(&source.cas).map(|view| view.kind))
                .collect::<Result<Vec<_>, _>>()?;
            assert!(kinds.contains(&HypothesisKind::CertifiedRegionMembership));
            assert!(kinds.contains(&HypothesisKind::Equality));
        }
        rows.push((format!("{example:?}"), outcome_row(outcome)));
    }
    assert_eq!(
        rows,
        [
            (
                "Integer".into(),
                ("proved".into(), 383_318, 1_353_635, 3_186)
            ),
            ("Sign".into(), ("proved".into(), 124_250, 594_593, 1_844)),
            ("Phase".into(), ("pending".into(), 127_532, 0, 0)),
        ]
    );
    Ok(())
}

fn outcome_row(outcome: Outcome) -> (String, u64, u64, usize) {
    match outcome {
        Outcome::Proved { result, usage, .. } => (
            "proved".into(),
            usage.work,
            result.replay_usage.work,
            result.witness.exported_byte_len(),
        ),
        Outcome::PendingPremise { blocker, usage, .. } => {
            assert_eq!(
                blocker.kind,
                nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::PendingPremise)
            );
            ("pending".into(), usage.work, 0, 0)
        }
        Outcome::Refused { fault, usage, .. } | Outcome::Invalid { fault, usage, .. } => {
            (format!("failure {fault:?}"), usage.work, 0, 0)
        }
        Outcome::Exhausted {
            fault,
            usage,
            replay_usage,
            ..
        } => (
            format!("exhausted {fault:?}"),
            usage.work,
            replay_usage.work,
            0,
        ),
        other => (
            format!("other {:?}", std::mem::discriminant(&other)),
            0,
            0,
            0,
        ),
    }
}

/// Certified selection cannot silently drop the supplied local membership premise.
/// ´test:integration:decision-certified-selection-needs-membership´
#[test]
fn decision_certified_selection_needs_membership() -> TestResult {
    let (mut fixture, mut supplied) = request(Example::Sign)?;
    supplied.premises.clear();
    assert!(matches!(
        fixture.partition.source.cas.run(&supplied),
        Outcome::PendingPremise { .. }
    ));
    Ok(())
}

fn partial_fixture(shift: Option<i64>) -> TestResult<super::IntroductionFixture> {
    let mut fixture = prepared_introduction(Example::Sign, false)?;
    let source = &mut fixture.partition.source;
    let one = source.number(1, 1)?;
    let denominator = if let Some(shift) = shift {
        let offset = source.number(shift, 1)?;
        source.term(
            nrz_cas::syntax::Operator::Add,
            &[fixture.partition.question.coordinates[0].clone(), offset],
        )?
    } else {
        source.number(0, 1)?
    };
    let value = source.term(nrz_cas::syntax::Operator::Div, &[one, denominator])?;
    let association = admit(
        &mut source.cas,
        nrz_cas::syntax::Draft::CaseArm(nrz_cas::domain::CaseArmDraft {
            header: nrz_cas::Reference::Existing(source.header.clone()),
            scope: Vec::new(),
            placements: Vec::new(),
            region: nrz_cas::Reference::Existing(fixture.option.clone()),
            value: nrz_cas::Reference::Existing(value),
        }),
        &mut ledger()?,
    )?;
    fixture.partition.arms[0].clone_from(&association);
    let domain = source.ambient.clone();
    let function =
        super::super::decision_partitions::case(source, &domain, &fixture.partition.arms)?;
    fixture.partition.question.function.clone_from(&function);
    let nrz_cas::proof::Certificate::Decision(evidence) = &mut fixture.steps[1].certificate else {
        return Err("decision certificate".into());
    };
    evidence.partition.function.clone_from(&function);
    evidence.cut.function = function;
    evidence.cut.association = association;
    Ok(fixture)
}

/// An introduced option cannot make its original everywhere-undefined branch value defined.
/// ´test:integration:decision-certified-selection-local-definedness´
#[test]
fn decision_certified_selection_local_definedness() -> TestResult {
    let (mut fixture, supplied) = from_fixture(partial_fixture(None)?)?;
    let outcome = fixture.partition.source.cas.run(&supplied);
    let Outcome::Refused { fault, .. } = outcome else {
        return Err(format!("undefined selection: {:?}", outcome_row(outcome)).into());
    };
    assert_eq!(
        fault.kind,
        nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::RefutedDomain)
    );
    Ok(())
}

/// An ordinary subset proof cannot hide the later exact certified membership in a supplied list.
/// ´test:integration:decision-certified-selection-premise-filter´
#[test]
fn decision_certified_selection_premise_filter() -> TestResult {
    let (mut fixture, mut supplied) = request(Example::Sign)?;
    let source = &mut fixture.partition.source;
    let mut step = membership(&source.cas, &supplied.context, &fixture.option)?;
    step.rule = nrz_cas::proof::Primitive::RegionConstruct;
    step.certificate = nrz_cas::proof::Certificate::None;
    let ordinary = source
        .cas
        .check_proof(&supplied.context, &[step], &mut ledger()?)?;
    assert_eq!(
        ordinary.hypotheses(),
        [] as [nrz_cas::domain::HypothesisRef; 0]
    );
    supplied.premises.insert(0, ordinary);
    let Outcome::Proved { result, .. } = source.cas.run(&supplied) else {
        return Err("certified membership was hidden".into());
    };
    assert!(
        result
            .witness
            .hypotheses()
            .contains(&nrz_cas::domain::HypothesisRef {
                context: supplied.context,
                slot: 0,
            })
    );
    Ok(())
}

/// A countercell outside the source equality cannot refute a conditional selection.
/// ´test:integration:decision-certified-selection-conditional-countercell´
#[test]
fn decision_certified_selection_conditional_countercell() -> TestResult {
    let (mut fixture, supplied) = from_fixture(partial_fixture(Some(1))?)?;
    let outcome = fixture.partition.source.cas.run(&supplied);
    assert!(
        matches!(outcome, Outcome::PendingPremise { .. }),
        "{:?}",
        outcome_row(outcome)
    );
    Ok(())
}

fn bounded_attempt(
    example: Example,
    kind: nrz_cas::limits::BudgetKind,
    amount: u64,
) -> TestResult<bool> {
    use nrz_cas::limits::BudgetKind;
    let (mut fixture, mut supplied) = request(example)?;
    match kind {
        BudgetKind::Work => supplied.limits.work = Some(amount),
        BudgetKind::ReplayWork => supplied.limits.replay_work = Some(amount),
        BudgetKind::StagedPayloadBytes => supplied.limits.staged_payload_bytes = Some(amount),
        BudgetKind::PayloadBytes => supplied.limits.payload_bytes = Some(amount),
        BudgetKind::OutputBytes => supplied.limits.output_bytes = Some(amount),
        _ => return Err("selection resource".into()),
    }
    match fixture.partition.source.cas.run(&supplied) {
        Outcome::Proved { .. } => Ok(true),
        Outcome::PendingPremise { blocker, .. } if matches!(example, Example::Phase) => {
            assert_eq!(blocker.location.operation, "exact region partition");
            Ok(true)
        }
        Outcome::Exhausted { fault, .. } => {
            let nrz_cas::FaultKind::Exhausted { budget, .. } = fault.kind else {
                return Err(fault.into());
            };
            assert!(
                budget == kind || (kind == BudgetKind::ReplayWork && budget == BudgetKind::Work),
                "{fault:?}"
            );
            Ok(false)
        }
        other => Err(format!("unexpected selection: {:?}", outcome_row(other)).into()),
    }
}

/// Literal ceilings distinguish mandatory proof work from optional response-envelope work.
/// ´test:integration:decision-certified-selection-boundaries´
#[test]
fn decision_certified_selection_boundaries() -> TestResult {
    use nrz_cas::limits::BudgetKind;
    for (example, amounts) in [
        (
            Example::Integer,
            [376_470, 1_353_635, 162_478, 92_422, 3_186],
        ),
        (Example::Sign, [118_744, 594_593, 159_172, 45_336, 1_844]),
        (Example::Phase, [127_532, 0, 90_427, 87_846, 0]),
    ] {
        for (kind, amount) in [
            BudgetKind::Work,
            BudgetKind::ReplayWork,
            BudgetKind::StagedPayloadBytes,
            BudgetKind::PayloadBytes,
            BudgetKind::OutputBytes,
        ]
        .into_iter()
        .zip(amounts)
        {
            assert!(
                bounded_attempt(example, kind, amount)?,
                "{example:?}:{kind:?}"
            );
            if amount != 0 {
                assert!(
                    !bounded_attempt(example, kind, amount - 1)?,
                    "{example:?}:{kind:?}"
                );
            }
        }
    }
    Ok(())
}

/// A certified point-membership proof is not the complete cover required by elimination.
/// ´test:integration:decision-certified-selection-no-elimination-cover´
#[test]
fn decision_certified_selection_no_elimination_cover() -> TestResult {
    let (mut fixture, mut supplied) = request(Example::Sign)?;
    let source = &mut fixture.partition.source;
    let domain = source.cas.admissible_region(&supplied.context)?;
    let value = source.number(0, 1)?;
    supplied.inputs.clear();
    supplied.task = Task::Eliminate {
        regions: vec![fixture.option],
        conclusion: Box::new(Statement::Equality {
            left: fixture.partition.question.function,
            right: value,
            domain,
            domain_contract: nrz_cas::proof::DomainContract::DefinedAndEqual,
        }),
    };
    let Outcome::Invalid { fault, .. } = source.cas.run(&supplied) else {
        return Err("membership accepted as cover".into());
    };
    assert_eq!(
        fault.kind,
        nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::Rule)
    );
    Ok(())
}

fn integer_expected(
    fixture: &mut super::IntroductionFixture,
    supplied: &Request,
) -> TestResult<nrz_cas::proof::Judgment> {
    let source = &mut fixture.partition.source;
    let three = crate::testing::builders::expression(
        &mut source.cas,
        &source.context,
        nrz_cas::syntax::Operator::Rational(super::super::decision_partitions::rational(3, 1)?),
        nrz_cas::syntax::Sort::Value(nrz_cas::syntax::AtomicSort::Integer),
        &[],
        &mut ledger()?,
    )?;
    Ok(nrz_cas::proof::Judgment {
        context: supplied.context.clone(),
        layer: nrz_cas::domain::Layer::Pointwise,
        space: None,
        residuals: Vec::new(),
        statement: Statement::Equality {
            left: fixture.partition.question.function.clone(),
            right: three,
            domain: source.cas.admissible_region(&supplied.context)?,
            domain_contract: nrz_cas::proof::DomainContract::DefinedAndEqual,
        },
    })
}

/// Fresh expected-judgment replay retains Integer 3 and rejects a Real replacement atomically.
/// ´test:integration:decision-integer-selection-replay´
#[test]
fn decision_integer_selection_replay() -> TestResult {
    let (mut original, supplied) = request(Example::Integer)?;
    let Outcome::Proved { result, .. } = original.partition.source.cas.run(&supplied) else {
        return Err("Integer selection failed".into());
    };
    let bytes = original
        .partition
        .source
        .cas
        .export_witness(&result.witness, &mut ledger()?)?;
    let (mut receiver, expected_request) = request(Example::Integer)?;
    let mut expected = integer_expected(&mut receiver, &expected_request)?;
    let source = &mut receiver.partition.source;
    let checked = source.cas.replay(&bytes, &expected, &mut ledger()?)?;
    assert_eq!(checked.judgment(), &expected);
    assert_eq!(source.cas.export_witness(&checked, &mut ledger()?)?, bytes);
    let replacement = source.number(3, 1)?;
    let Statement::Equality { right, .. } = &mut expected.statement else {
        return Err("expected Integer equality".into());
    };
    *right = replacement;
    let before = source.cas.counts();
    assert!(
        source
            .cas
            .replay(&bytes, &expected, &mut ledger()?)
            .is_err()
    );
    assert_eq!(source.cas.counts(), before);
    Ok(())
}
