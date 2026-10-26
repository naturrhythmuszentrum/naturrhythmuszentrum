// SPDX-License-Identifier: AGPL-3.0-only
//! Projected Integer selection keeps family, correspondence and enclosure authority separate.

use super::{Frame, admit, fixture as cover_fixture, ledger};
use crate::testing::common::TestResult;
use nrz_cas::decision::{EnclosureOrigin, IntegerFamily, IntegerSelection, RationalEnclosure};
use nrz_cas::domain::{AssumptionDraft, ContextDraft, HypothesisRef, Layer};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::proof::{ApplicationDraft, Certificate, Judgment, Primitive, Statement};
use nrz_cas::syntax::{AtomicSort, Draft, Operator, PredicateKind, Sort};
use nrz_cas::{Cas, FaultKind, Handle, InvalidCode, Location, Rational, Reference};

struct SelectionFixture {
    frame: Frame,
    context: Handle,
    steps: Vec<ApplicationDraft>,
}

fn exact(frame: &mut Frame, text: &str, integer: bool) -> TestResult<Handle> {
    let number = Rational::parse(text, &mut ledger()?)?;
    let sort = if integer {
        Sort::Value(AtomicSort::Integer)
    } else {
        super::super::real()
    };
    frame.term(Operator::Rational(number), sort, &[])
}

fn fixture(bounded: bool, selected: &str, target: &str) -> TestResult<SelectionFixture> {
    let source = cover_fixture(bounded)?;
    let Statement::IntegerFamily(family) = source.application.conclusion else {
        return Err("missing source family".into());
    };
    assemble(source.frame, *family, Vec::new(), selected, target)
}

fn assemble(
    mut frame: Frame,
    family: IntegerFamily,
    mut bindings: Vec<Handle>,
    selected: &str,
    target: &str,
) -> TestResult<SelectionFixture> {
    let value = exact(&mut frame, selected, true)?;
    let (target, number) = if target == "10/Pi" {
        let pi = frame.term(
            Operator::Constant(nrz_cas::syntax::Constant::Pi),
            super::super::real(),
            &[],
        )?;
        let ten = exact(&mut frame, "10", false)?;
        (
            frame.term(Operator::Div, super::super::real(), &[ten, pi])?,
            None,
        )
    } else {
        let number = Rational::parse(target, &mut ledger()?)?;
        (exact(&mut frame, target, false)?, Some(number))
    };
    let predicate = admit(
        &mut frame.cas,
        Draft::Condition {
            header: Reference::Existing(frame.header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::Equal,
            arguments: vec![
                Reference::Existing(family.coordinate.clone()),
                Reference::Existing(target.clone()),
            ],
        },
        &mut ledger()?,
    )?;
    bindings.push(predicate);
    let context = admit(
        &mut frame.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(frame.header.clone()),
            admissible: Reference::Existing(family.domain.clone()),
            assumptions: bindings
                .iter()
                .enumerate()
                .map(|(index, condition)| AssumptionDraft {
                    key: format!("selection-source-{index}"),
                    condition: Reference::Existing(condition.clone()),
                })
                .collect(),
        }),
        &mut ledger()?,
    )?;
    let steps = applications(&context, family, value, target, &bindings, number)?;
    Ok(SelectionFixture {
        frame,
        context,
        steps,
    })
}

fn hypothesis(
    context: &Handle,
    domain: &Handle,
    predicate: &Handle,
    slot: u32,
) -> ApplicationDraft {
    ApplicationDraft {
        rule: Primitive::ContextHypothesis,
        context: context.clone(),
        conclusion: Statement::PredicateFact {
            region: domain.clone(),
            predicate: predicate.clone(),
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Hypothesis(HypothesisRef {
            context: context.clone(),
            slot,
        }),
    }
}

fn applications(
    context: &Handle,
    family: IntegerFamily,
    value: Handle,
    target: Handle,
    bindings: &[Handle],
    number: Option<Rational>,
) -> TestResult<Vec<ApplicationDraft>> {
    let mut steps = Vec::new();
    for (index, predicate) in bindings.iter().enumerate() {
        steps.push(hypothesis(
            context,
            &family.domain,
            predicate,
            u32::try_from(index)?,
        ));
    }
    let cover = u32::try_from(steps.len())?;
    let correspondence = cover.checked_sub(1).ok_or("missing coordinate binding")?;
    steps.push(ApplicationDraft {
        rule: Primitive::RegionDecide,
        context: context.clone(),
        conclusion: Statement::IntegerFamily(Box::new(family.clone())),
        premises: (0..correspondence).collect(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    });
    let (lower, upper, origin) = if let Some(number) = number {
        (
            number.clone(),
            number.clone(),
            EnclosureOrigin::ExactRational {
                expression: target.clone(),
                value: number,
            },
        )
    } else {
        (
            Rational::from_ratio(203, 64, &mut ledger()?)?,
            Rational::from_ratio(205, 64, &mut ledger()?)?,
            EnclosureOrigin::PiBound {
                expression: target.clone(),
                certificate: Box::new(crate::conformance::pi_bounds::cycle()?),
            },
        )
    };
    let enclosure = RationalEnclosure {
        target,
        context: context.clone(),
        input_family: family.domain.clone(),
        lower,
        upper,
        origin,
    };
    steps.push(ApplicationDraft {
        rule: Primitive::CaseSelect,
        context: context.clone(),
        conclusion: Statement::IntegerSelection(Box::new(IntegerSelection { family, value })),
        premises: vec![cover, correspondence],
        substitutions: Vec::new(),
        certificate: Certificate::IntegerSelection(Box::new(enclosure)),
    });
    Ok(steps)
}

fn enclosure(source: &mut SelectionFixture) -> TestResult<&mut RationalEnclosure> {
    let Certificate::IntegerSelection(value) = &mut source
        .steps
        .last_mut()
        .ok_or("missing selection")?
        .certificate
    else {
        return Err("missing selection certificate".into());
    };
    Ok(value)
}

fn narrow(source: &mut SelectionFixture) -> TestResult {
    let certificate = enclosure(source)?;
    certificate.lower = Rational::from_ratio(203, 64, &mut ledger()?)?;
    certificate.upper = Rational::from_ratio(205, 64, &mut ledger()?)?;
    Ok(())
}

/// Exact N=3 projection retains its source condition through replay and separate capacity checking.
/// ´test:integration:integer-family-selection´
#[test]
fn integer_family_selection() -> TestResult {
    let mut measured = Vec::new();
    for bounded in [false, true] {
        let mut source = fixture(bounded, "3", "51/16")?;
        narrow(&mut source)?;
        let before = source.frame.cas.counts();
        let mut budget = ledger()?;
        let witness = source
            .frame
            .cas
            .check_proof(&source.context, &source.steps, &mut budget)?;
        assert!(witness.is_discharged());
        assert!(!witness.is_unconditional());
        assert_eq!(witness.hypotheses().len(), 1);
        let mut materialization = ledger()?;
        assert_eq!(
            source
                .frame
                .cas
                .materialize_integer_selection(&witness, 3, &mut materialization)?,
            3
        );
        let fault = source
            .frame
            .cas
            .materialize_integer_selection(&witness, 2, &mut ledger()?)
            .err()
            .ok_or("capacity overrun admitted")?;
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::DecisionMaterializationCapacity)
        );
        let bytes = source.frame.cas.export_witness(&witness, &mut ledger()?)?;
        let citations = source
            .frame
            .cas
            .proof_citations(witness.proof(), &mut ledger()?)?;
        let schema = citations
            .last()
            .ok_or("missing selection citation")?
            .auxiliary_schema(&source.frame.cas)?
            .ok_or("missing selection lemma")?;
        assert_eq!(
            (schema.name, schema.version),
            ("decision.integer-family-selection", 1)
        );
        let mut receiver = Cas::new(Limits::default())?;
        let mut replay = ledger()?;
        let imported = receiver.inspect(&bytes, &mut replay)?;
        assert_eq!(imported.hypotheses().len(), 1);
        assert_eq!(
            receiver.materialize_integer_selection(&imported, 3, &mut ledger()?)?,
            3
        );
        assert_eq!(receiver.export_witness(&imported, &mut ledger()?)?, bytes);
        measured.push((
            bounded,
            budget.usage().work,
            source.frame.cas.counts().payload_bytes - before.payload_bytes,
            bytes.len(),
            replay.usage().work,
            source.frame.cas.counts().payload_bytes,
            receiver.counts().payload_bytes,
            materialization.usage().work,
        ));
    }
    assert_eq!(
        measured,
        [
            (false, 108_477, 3_767, 1_416, 182_078, 16_693, 16_181, 37),
            (true, 141_257, 3_885, 1_666, 233_660, 19_710, 19_198, 37),
        ]
    );
    Ok(())
}

fn refusal(source: &mut SelectionFixture) -> TestResult {
    let before = source.frame.cas.counts();
    let snapshot = source.frame.cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = ledger()?;
    let fault = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut budget)
        .err()
        .ok_or("false Integer selection admitted")?;
    assert!(matches!(fault.kind, FaultKind::Invalid(_)));
    assert!(budget.usage().work > 0);
    assert_eq!(source.frame.cas.counts(), before);
    assert_eq!(
        source.frame.cas.canonical_snapshot(&mut ledger()?)?,
        snapshot
    );
    Ok(())
}

/// Missing cover, false boundary membership, changed result sorts and unsupported origins refuse atomically.
/// ´test:integration:integer-family-selection-refusals´
#[test]
fn integer_family_selection_refusals() -> TestResult {
    let mut source = fixture(true, "3", "51/16")?;
    let original = source.steps.clone();
    source.steps.last_mut().ok_or("missing selection")?.premises = vec![0, 0];
    refusal(&mut source)?;
    source.steps.clone_from(&original);
    enclosure(&mut source)?.upper = Rational::from_ratio(4, 1, &mut ledger()?)?;
    refusal(&mut source)?;
    source.steps.clone_from(&original);
    enclosure(&mut source)?.lower = Rational::from_ratio(2, 1, &mut ledger()?)?;
    refusal(&mut source)?;
    source.steps.clone_from(&original);
    enclosure(&mut source)?.origin = EnclosureOrigin::Unverified;
    refusal(&mut source)?;
    source.steps.clone_from(&original);
    let real = exact(&mut source.frame, "3", false)?;
    let Statement::IntegerSelection(selected) = &mut source
        .steps
        .last_mut()
        .ok_or("missing selection")?
        .conclusion
    else {
        return Err("missing selected statement".into());
    };
    selected.value = real;
    refusal(&mut source)?;
    refusal(&mut fixture(true, "5", "41/8")?)?;
    Ok(())
}

/// Parameter selection retains all three assumptions and refuses a cell outside the bound range.
/// ´test:integration:integer-family-selection-parameters´
#[test]
fn integer_family_selection_parameters() -> TestResult {
    let (frame, family, bindings) = super::parameters::selection_seed()?;
    let mut source = assemble(frame, family, bindings, "3", "51/16")?;
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut ledger()?)?;
    assert_eq!(witness.hypotheses().len(), 3);
    assert_eq!(
        source
            .frame
            .cas
            .materialize_integer_selection(&witness, 3, &mut ledger()?)?,
        3
    );
    let bytes = source.frame.cas.export_witness(&witness, &mut ledger()?)?;
    let mut receiver = Cas::new(Limits::default())?;
    let replayed = receiver.inspect(&bytes, &mut ledger()?)?;
    assert_eq!(replayed.hypotheses().len(), 3);
    let (frame, family, bindings) = super::parameters::selection_seed()?;
    refusal(&mut assemble(frame, family, bindings, "1", "9/8")?)?;
    Ok(())
}

/// Mathematical selection survives when its exact index exceeds machine or owner capacity.
/// ´test:integration:integer-family-selection-capacity´
#[test]
fn integer_family_selection_capacity() -> TestResult {
    for (selected, accepted) in [
        ("18446744073709551615", true),
        ("18446744073709551616", false),
    ] {
        let mut source = fixture(false, selected, selected)?;
        let witness =
            source
                .frame
                .cas
                .check_proof(&source.context, &source.steps, &mut ledger()?)?;
        let before = source.frame.cas.counts();
        let snapshot = source.frame.cas.canonical_snapshot(&mut ledger()?)?;
        let bytes = source.frame.cas.export_witness(&witness, &mut ledger()?)?;
        let result =
            source
                .frame
                .cas
                .materialize_integer_selection(&witness, u64::MAX, &mut ledger()?);
        if accepted {
            assert_eq!(result?, u64::MAX);
        } else {
            assert_eq!(
                result.err().ok_or("machine overflow admitted")?.kind,
                FaultKind::Invalid(InvalidCode::DecisionMaterializationCapacity)
            );
        }
        assert_eq!(source.frame.cas.counts(), before);
        assert_eq!(
            source.frame.cas.canonical_snapshot(&mut ledger()?)?,
            snapshot
        );
        assert_eq!(
            source.frame.cas.export_witness(&witness, &mut ledger()?)?,
            bytes
        );
    }
    Ok(())
}

/// A negative selected Integer remains exact while zero materializes at zero inclusive capacity.
/// ´test:integration:integer-family-selection-nonnegative-capacity´
#[test]
fn integer_family_selection_nonnegative_capacity() -> TestResult {
    for selected in ["-3", "0"] {
        let source = super::general_fixture(None, None)?;
        let Statement::IntegerFamily(family) = source.application.conclusion else {
            return Err("missing source family".into());
        };
        let mut source = assemble(source.frame, *family, Vec::new(), selected, selected)?;
        let witness =
            source
                .frame
                .cas
                .check_proof(&source.context, &source.steps, &mut ledger()?)?;
        let bytes = source.frame.cas.export_witness(&witness, &mut ledger()?)?;
        let before = source.frame.cas.counts();
        let snapshot = source.frame.cas.canonical_snapshot(&mut ledger()?)?;
        let result = source
            .frame
            .cas
            .materialize_integer_selection(&witness, 0, &mut ledger()?);
        if selected == "0" {
            assert_eq!(result?, 0);
        } else {
            assert_eq!(
                result.err().ok_or("negative count admitted")?.kind,
                FaultKind::Invalid(InvalidCode::DecisionMaterializationCapacity)
            );
        }
        assert_eq!(source.frame.cas.counts(), before);
        assert_eq!(
            source.frame.cas.canonical_snapshot(&mut ledger()?)?,
            snapshot
        );
        assert_eq!(
            source.frame.cas.export_witness(&witness, &mut ledger()?)?,
            bytes
        );
    }
    Ok(())
}

/// Receiving stores check independently built full selection claims and reject a changed result.
/// ´test:integration:integer-family-selection-expected´
#[test]
fn integer_family_selection_expected() -> TestResult {
    let mut source = fixture(true, "3", "51/16")?;
    narrow(&mut source)?;
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut ledger()?)?;
    let bytes = source.frame.cas.export_witness(&witness, &mut ledger()?)?;
    let mut receiver = fixture(true, "3", "51/16")?;
    let mut expected = Judgment {
        context: receiver.context.clone(),
        layer: Layer::Logic,
        space: None,
        statement: receiver.steps.pop().ok_or("missing selection")?.conclusion,
        residuals: Vec::new(),
    };
    let imported = receiver
        .frame
        .cas
        .replay(&bytes, &expected, &mut ledger()?)?;
    assert_eq!(imported.judgment(), &expected);
    assert_eq!(imported.hypotheses().len(), 1);
    assert_eq!(
        receiver
            .frame
            .cas
            .export_witness(&imported, &mut ledger()?)?,
        bytes
    );
    let Statement::IntegerSelection(selection) = &mut expected.statement else {
        return Err("missing expected selection".into());
    };
    selection.value = exact(&mut receiver.frame, "4", true)?;
    let before = receiver.frame.cas.counts();
    let snapshot = receiver.frame.cas.canonical_snapshot(&mut ledger()?)?;
    let fault = receiver
        .frame
        .cas
        .replay(&bytes, &expected, &mut ledger()?)
        .err()
        .ok_or("changed selected Integer accepted")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(receiver.frame.cas.counts(), before);
    assert_eq!(
        receiver.frame.cas.canonical_snapshot(&mut ledger()?)?,
        snapshot
    );
    Ok(())
}

/// Materialization uses its caller's cumulative Work without changing the selection or graph.
/// ´test:integration:integer-family-materialization-work´
#[test]
fn integer_family_materialization_work() -> TestResult {
    let mut source = fixture(false, "3", "51/16")?;
    narrow(&mut source)?;
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut ledger()?)?;
    let before = source.frame.cas.counts();
    let snapshot = source.frame.cas.canonical_snapshot(&mut ledger()?)?;
    let bytes = source.frame.cas.export_witness(&witness, &mut ledger()?)?;
    for (work, prior, accepted) in [(37, 0, true), (36, 0, false), (37, 1, false), (38, 1, true)] {
        let limits = Limits {
            work,
            ..Limits::default()
        };
        let mut budget = Budget::new(limits.clone())?;
        budget.work(prior, Location::new("prior Integer materialization work"))?;
        let result = source
            .frame
            .cas
            .materialize_integer_selection(&witness, 3, &mut budget);
        if accepted {
            assert_eq!(result?, 3);
            assert_eq!(budget.usage().work, work);
        } else {
            assert!(matches!(
                result.err().ok_or("materialization exceeded Work")?.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::Work,
                    ..
                }
            ));
            assert!(budget.usage().work > prior);
        }
        assert_eq!(budget.limits(), &limits);
        assert_eq!(source.frame.cas.counts(), before);
        assert_eq!(
            source.frame.cas.canonical_snapshot(&mut ledger()?)?,
            snapshot
        );
        assert_eq!(
            source.frame.cas.export_witness(&witness, &mut ledger()?)?,
            bytes
        );
    }
    Ok(())
}

fn resource_attempt(
    bounded: bool,
    replay: bool,
    kind: BudgetKind,
    amount: u64,
    prior: u64,
) -> TestResult<bool> {
    let mut source = fixture(bounded, "3", "51/16")?;
    narrow(&mut source)?;
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
    let limits = super::resource_limits(kind, amount)?;
    let mut budget = Budget::new(limits.clone())?;
    budget.work(prior, Location::new("prior Integer selection work"))?;
    let result = if let Some(bytes) = bytes {
        cas.inspect(&bytes, &mut budget)
    } else {
        cas.check_proof(&source.context, &source.steps, &mut budget)
    };
    assert_eq!(budget.limits(), &limits);
    assert!(budget.usage().work > prior);
    match result {
        Ok(_) => Ok(true),
        Err(fault) => {
            assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind));
            assert_eq!(cas.counts(), before);
            assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, snapshot);
            Ok(false)
        }
    }
}

/// Literal resource boundaries preserve atomic state and cumulative Work through proof and replay.
/// ´test:integration:integer-family-selection-resource-boundaries´
#[test]
fn integer_family_selection_resource_boundaries() -> TestResult {
    for (bounded, replay, work, staged, payload) in [
        (false, false, 108_477, 79_744, 16_693),
        (false, true, 182_078, 79_040, 16_181),
        (true, false, 141_257, 87_616, 19_710),
        (true, true, 233_660, 86_912, 19_198),
    ] {
        for (kind, amount) in [
            (BudgetKind::Work, work),
            (BudgetKind::StagedPayloadBytes, staged),
            (BudgetKind::PayloadBytes, payload),
        ] {
            assert!(resource_attempt(bounded, replay, kind, amount, 0)?);
            assert!(!resource_attempt(bounded, replay, kind, amount - 1, 0)?);
        }
        assert!(resource_attempt(
            bounded,
            replay,
            BudgetKind::OutputBytes,
            0,
            0
        )?);
        assert!(!resource_attempt(
            bounded,
            replay,
            BudgetKind::Work,
            work,
            1
        )?);
        assert!(resource_attempt(
            bounded,
            replay,
            BudgetKind::Work,
            work + 1,
            1
        )?);
        if replay {
            assert!(resource_attempt(
                bounded,
                true,
                BudgetKind::ReplayWork,
                work,
                0
            )?);
            assert!(!resource_attempt(
                bounded,
                true,
                BudgetKind::ReplayWork,
                work - 1,
                0
            )?);
        }
    }
    Ok(())
}

#[path = "selection/pi.rs"]
mod pi;
