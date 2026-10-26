// SPDX-License-Identifier: AGPL-3.0-only
//! Exact parameter bindings for native floor relations intersected with count domains.

use super::{Frame, admit, bound_guard, ledger};
use crate::testing::common::TestResult;
use nrz_cas::decision::{IntegerCellRange, IntegerFamily};
use nrz_cas::domain::{AssumptionDraft, ContextDraft, HypothesisRef, Layer, RegionExpr};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::proof::{ApplicationDraft, Certificate, Judgment, Primitive, Statement};
use nrz_cas::syntax::{AtomicSort, Draft, Operator, PredicateKind, Sort};
use nrz_cas::{Cas, FaultKind, Handle, InvalidCode, Location, Rational, Reference};

struct ParameterFixture {
    frame: Frame,
    context: Handle,
    steps: Vec<ApplicationDraft>,
}

fn integer(frame: &mut Frame, value: i64) -> TestResult<Handle> {
    let value = Rational::from_ratio(value, 1, &mut ledger()?)?;
    frame.term(
        Operator::Rational(value),
        Sort::Value(AtomicSort::Integer),
        &[],
    )
}

fn binding(frame: &mut Frame, parameter: &Handle, value: i64) -> TestResult<Handle> {
    let value = integer(frame, value)?;
    admit(
        &mut frame.cas,
        Draft::Condition {
            header: Reference::Existing(frame.header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::Equal,
            arguments: vec![
                Reference::Existing(parameter.clone()),
                Reference::Existing(value),
            ],
        },
        &mut ledger()?,
    )
}

fn fixture(lower: i64, upper: i64) -> TestResult<ParameterFixture> {
    with_expression(lower, upper, false)
}

fn with_expression(lower: i64, upper: i64, affine: bool) -> TestResult<ParameterFixture> {
    with_operator(lower, upper, affine.then_some(Operator::Add))
}

fn endpoint_expression(
    frame: &mut Frame,
    a: Handle,
    operator: Option<Operator>,
) -> TestResult<Handle> {
    let Some(operator) = operator else {
        return Ok(a);
    };
    let right = if operator == Operator::Mul {
        a.clone()
    } else {
        integer(frame, 1)?
    };
    frame.term(operator, Sort::Value(AtomicSort::Integer), &[a, right])
}

fn with_operator(
    lower: i64,
    upper: i64,
    operator: Option<Operator>,
) -> TestResult<ParameterFixture> {
    let mut frame = Frame::with_parameters(true)?;
    let (floor, [q, n]) = frame.relation(false)?;
    let a = frame.term(
        Operator::Free("a".into()),
        Sort::Value(AtomicSort::Integer),
        &[],
    )?;
    let b = frame.term(
        Operator::Free("b".into()),
        Sort::Value(AtomicSort::Integer),
        &[],
    )?;
    let a = endpoint_expression(&mut frame, a, operator)?;
    let range = frame.guard(
        PredicateKind::IntegerRange,
        &[n.clone(), a.clone(), b.clone()],
    )?;
    let relation = frame.region(RegionExpr::Meet(vec![
        Reference::Existing(floor),
        Reference::Existing(range),
    ]))?;
    let (function, _) = frame.cases(&relation, n.clone())?;
    let low = bound_guard(&mut frame, &q, &a, false, true)?;
    let high = bound_guard(&mut frame, &q, &b, true, true)?;
    let count = frame.guard(PredicateKind::NonNegative, std::slice::from_ref(&q))?;
    let domain = frame.region(RegionExpr::Meet(vec![
        Reference::Existing(low),
        Reference::Existing(high),
        Reference::Existing(count),
    ]))?;
    let bindings = [
        binding(&mut frame, &a, lower)?,
        binding(&mut frame, &b, upper)?,
    ];
    let family = IntegerFamily {
        function,
        coordinate: q.clone(),
        index: n,
        domain,
        lower: Some(a),
        upper: Some(b),
        count: Some(Box::new(IntegerCellRange {
            coordinate: q,
            lower: Some(Rational::zero()),
            upper: None,
        })),
    };
    finish(frame, family, &bindings)
}

fn finish(
    mut frame: Frame,
    family: IntegerFamily,
    bindings: &[Handle],
) -> TestResult<ParameterFixture> {
    let context = admit(
        &mut frame.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(frame.header.clone()),
            admissible: Reference::Existing(family.domain.clone()),
            assumptions: bindings
                .iter()
                .enumerate()
                .map(|(index, condition)| AssumptionDraft {
                    key: format!("parameter-{index}"),
                    condition: Reference::Existing(condition.clone()),
                })
                .collect(),
        }),
        &mut ledger()?,
    )?;
    let mut steps = Vec::new();
    for (index, predicate) in bindings.iter().enumerate() {
        steps.push(ApplicationDraft {
            rule: Primitive::ContextHypothesis,
            context: context.clone(),
            conclusion: Statement::PredicateFact {
                region: family.domain.clone(),
                predicate: predicate.clone(),
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::Hypothesis(HypothesisRef {
                context: context.clone(),
                slot: u32::try_from(index)?,
            }),
        });
    }
    steps.push(ApplicationDraft {
        rule: Primitive::RegionDecide,
        context: context.clone(),
        conclusion: Statement::IntegerFamily(Box::new(family)),
        premises: vec![0, 1],
        substitutions: Vec::new(),
        certificate: Certificate::None,
    });
    Ok(ParameterFixture {
        frame,
        context,
        steps,
    })
}

/// The unchanged unbound contract retains its exact unsupported-endpoint refusal and atomicity.
/// ´test:integration:integer-family-parameter-unbound´
#[test]
fn integer_family_parameter_unbound() -> TestResult {
    for amount in [2_034, 2_033] {
        let mut source = fixture(2, 5)?;
        let mut application = source.steps.pop().ok_or("missing family")?;
        application.premises.clear();
        let Statement::IntegerFamily(family) = &mut application.conclusion else {
            return Err("missing family statement".into());
        };
        family.count = None;
        let before = source.frame.cas.counts();
        let mut budget = Budget::new(Limits {
            work: amount,
            ..Limits::default()
        })?;
        let fault = source
            .frame
            .cas
            .check_proof(&source.context, &[application], &mut budget)
            .err()
            .ok_or("unbound parameter range admitted")?;
        if amount == 2_034 {
            assert_eq!(
                fault.kind,
                FaultKind::Invalid(InvalidCode::UnsupportedCertificate)
            );
            assert_eq!(budget.usage().work, 2_034);
        } else {
            assert!(matches!(
                fault.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::Work,
                    ..
                }
            ));
        }
        assert_eq!(source.frame.cas.counts(), before);
    }
    Ok(())
}

/// Exact bindings retain both qualified source conditions through independent fresh-store replay.
/// ´test:integration:integer-family-parameter-cover´
#[test]
fn integer_family_parameter_cover() -> TestResult {
    let mut measured = Vec::new();
    for lower in [2, -2] {
        let mut source = fixture(lower, 5)?;
        let before = source.frame.cas.counts();
        let mut budget = ledger()?;
        let witness = source
            .frame
            .cas
            .check_proof(&source.context, &source.steps, &mut budget)?;
        assert!(witness.is_discharged());
        assert!(!witness.is_unconditional());
        assert_eq!(witness.hypotheses().len(), 2);
        let bytes = source.frame.cas.export_witness(&witness, &mut ledger()?)?;
        let citations = source
            .frame
            .cas
            .proof_citations(witness.proof(), &mut ledger()?)?;
        let root = citations.last().ok_or("missing family citation")?;
        let schema = root
            .auxiliary_schema(&source.frame.cas)?
            .ok_or("missing schema")?;
        assert_eq!(
            (schema.name, schema.version),
            ("decision.integer-floor-family", 2)
        );
        assert_eq!(root.hypotheses.len(), 2);
        let mut receiver = Cas::new(Limits::default())?;
        let mut replay = ledger()?;
        let imported = receiver.inspect(&bytes, &mut replay)?;
        assert_eq!(imported.hypotheses().len(), 2);
        assert_eq!(receiver.export_witness(&imported, &mut ledger()?)?, bytes);
        measured.push((
            lower,
            budget.usage().work,
            source.frame.cas.counts().payload_bytes - before.payload_bytes,
            bytes.len(),
            replay.usage().work,
            receiver.counts().payload_bytes,
        ));
    }
    assert_eq!(
        measured,
        [
            (2, 203_005, 3_479, 1_896, 326_223, 22_810),
            (-2, 202_909, 3_479, 1_896, 326_127, 22_810),
        ]
    );
    Ok(())
}

fn resource_attempt(replay: bool, kind: BudgetKind, amount: u64, prior: u64) -> TestResult<bool> {
    let mut source = fixture(2, 5)?;
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
    budget.work(prior, Location::new("prior parameter family work"))?;
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

/// Literal proof and replay limits retain spent Work and refuse atomically at N minus one.
/// ´test:integration:integer-family-parameter-resource-boundaries´
#[test]
fn integer_family_parameter_resource_boundaries() -> TestResult {
    for (replay, work, staged, payload) in [
        (false, 203_005, 109_280, 22_810),
        (true, 326_223, 108_576, 22_810),
    ] {
        for (kind, amount) in [
            (BudgetKind::Work, work),
            (BudgetKind::StagedPayloadBytes, staged),
            (BudgetKind::PayloadBytes, payload),
        ] {
            assert!(resource_attempt(replay, kind, amount, 0)?);
            assert!(!resource_attempt(replay, kind, amount - 1, 0)?);
        }
        assert!(resource_attempt(replay, BudgetKind::OutputBytes, 0, 0)?);
        assert!(!resource_attempt(replay, BudgetKind::Work, work, 1)?);
        assert!(resource_attempt(replay, BudgetKind::Work, work + 1, 1)?);
        if replay {
            assert!(resource_attempt(true, BudgetKind::ReplayWork, work, 0)?);
            assert!(!resource_attempt(
                true,
                BudgetKind::ReplayWork,
                work - 1,
                0
            )?);
        }
    }
    Ok(())
}

fn family_mut(source: &mut ParameterFixture) -> TestResult<&mut IntegerFamily> {
    let application = source.steps.last_mut().ok_or("missing family")?;
    let Statement::IntegerFamily(family) = &mut application.conclusion else {
        return Err("missing family statement".into());
    };
    Ok(family)
}

fn refusal(source: &mut ParameterFixture) -> TestResult {
    let before = source.frame.cas.counts();
    let snapshot = source.frame.cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = ledger()?;
    let fault = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut budget)
        .err()
        .ok_or("false parameter family accepted")?;
    assert!(matches!(fault.kind, FaultKind::Invalid(_)));
    assert!(budget.usage().work > 0);
    assert_eq!(source.frame.cas.counts(), before);
    assert_eq!(
        source.frame.cas.canonical_snapshot(&mut ledger()?)?,
        snapshot
    );
    Ok(())
}

/// Empty declared ranges and empty intersections with the count domain cannot prove by vacuity.
/// ´test:integration:integer-family-parameter-empty´
#[test]
fn integer_family_parameter_empty() -> TestResult {
    for (lower, upper) in [(2, 2), (5, 2), (-4, -1), (-1, 0)] {
        refusal(&mut fixture(lower, upper)?)?;
    }
    Ok(())
}

/// Ordered bindings and exact count-domain identity cannot be omitted, swapped or widened.
/// ´test:integration:integer-family-parameter-refusals´
#[test]
fn integer_family_parameter_refusals() -> TestResult {
    let mut source = fixture(2, 5)?;
    let original = source.steps.clone();
    source.steps.last_mut().ok_or("missing family")?.premises = vec![1, 0];
    refusal(&mut source)?;
    source.steps.clone_from(&original);
    family_mut(&mut source)?.count = None;
    refusal(&mut source)?;
    source.steps.clone_from(&original);
    let count = family_mut(&mut source)?
        .count
        .as_mut()
        .ok_or("missing count domain")?;
    count.lower = Some(Rational::from_ratio(1, 2, &mut ledger()?)?);
    refusal(&mut source)?;
    source.steps.clone_from(&original);
    let count = family_mut(&mut source)?
        .count
        .as_mut()
        .ok_or("missing count domain")?;
    count.lower = Some(Rational::from_ratio(4, 1, &mut ledger()?)?);
    refusal(&mut source)?;
    source.steps.clone_from(&original);
    source.steps.remove(1);
    source.steps.last_mut().ok_or("missing family")?.premises = vec![0];
    refusal(&mut source)?;
    Ok(())
}

/// Integer-affine endpoint expressions retain their original binding proof and projected relation.
/// ´test:integration:integer-family-parameter-affine´
#[test]
fn integer_family_parameter_affine() -> TestResult {
    let mut source = with_expression(3, 5, true)?;
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut ledger()?)?;
    assert_eq!(witness.hypotheses().len(), 2);
    assert!(!witness.is_unconditional());
    let bytes = source.frame.cas.export_witness(&witness, &mut ledger()?)?;
    let mut receiver = Cas::new(Limits::default())?;
    let imported = receiver.inspect(&bytes, &mut ledger()?)?;
    assert_eq!(imported.hypotheses().len(), 2);
    assert_eq!(receiver.export_witness(&imported, &mut ledger()?)?, bytes);
    Ok(())
}

/// Independent replay expectations retain the count-domain constructor and both qualified bindings.
/// ´test:integration:integer-family-parameter-expected´
#[test]
fn integer_family_parameter_expected() -> TestResult {
    let mut source = fixture(2, 5)?;
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut ledger()?)?;
    let bytes = source.frame.cas.export_witness(&witness, &mut ledger()?)?;
    let mut receiver = fixture(2, 5)?;
    let mut expected = Judgment {
        context: receiver.context.clone(),
        layer: Layer::Logic,
        space: None,
        statement: receiver.steps.pop().ok_or("missing family")?.conclusion,
        residuals: Vec::new(),
    };
    let replayed = receiver
        .frame
        .cas
        .replay(&bytes, &expected, &mut ledger()?)?;
    assert_eq!(replayed.judgment(), &expected);
    assert_eq!(replayed.hypotheses().len(), 2);
    let Statement::IntegerFamily(family) = &mut expected.statement else {
        return Err("missing expected family".into());
    };
    family.count = None;
    let before = receiver.frame.cas.counts();
    let fault = receiver
        .frame
        .cas
        .replay(&bytes, &expected, &mut ledger()?)
        .err()
        .ok_or("omitted count domain accepted")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(receiver.frame.cas.counts(), before);
    Ok(())
}

/// Exact bindings do not erase an unsupported original nonlinear endpoint operation.
/// ´test:integration:integer-family-parameter-nonlinear´
#[test]
fn integer_family_parameter_nonlinear() -> TestResult {
    refusal(&mut with_operator(2, 5, Some(Operator::Mul))?)
}
