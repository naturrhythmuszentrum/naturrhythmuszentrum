// SPDX-License-Identifier: AGPL-3.0-only
//! Closed rational enclosure replay binds the original target and preserves every operand.

use super::decision_baseline::{Example, Fixture};
use super::decision_partitions::rational;
use crate::testing::builders::admit;
use crate::testing::common::TestResult;
use crate::testing::readings::ledger;
use nrz_cas::decision::{EnclosureOrigin, RationalEnclosure};
use nrz_cas::domain::ContextDraft;
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::syntax::{Constant, Draft, Operator};
use nrz_cas::{FaultKind, Handle, InvalidCode, Location, Reference};

fn question(
    source: &Fixture,
    target: Handle,
    value: (i64, u64),
    lower: (i64, u64),
    upper: (i64, u64),
) -> TestResult<RationalEnclosure> {
    Ok(RationalEnclosure {
        origin: EnclosureOrigin::ExactRational {
            expression: target.clone(),
            value: rational(value.0, value.1)?,
        },
        target,
        context: source.context.clone(),
        input_family: source.cas.admissible_region(&source.context)?,
        lower: rational(lower.0, lower.1)?,
        upper: rational(upper.0, upper.1)?,
    })
}

pub(super) fn driven(source: &mut Fixture) -> TestResult<Handle> {
    let omega = source.number(1_000, 1)?;
    let inductance = source.number(1, 40)?;
    let capacitance = source.number(1, 100_000)?;
    let one = source.number(1, 1)?;
    let left = source.term(Operator::Mul, &[omega.clone(), inductance])?;
    let denominator = source.term(Operator::Mul, &[omega, capacitance])?;
    let right = source.term(Operator::Div, &[one, denominator])?;
    source.term(Operator::Sub, &[left, right])
}

fn prepared(index: usize) -> TestResult<(Fixture, RationalEnclosure)> {
    let mut source = Fixture::new(Example::Phase)?;
    let (target, value, lower, upper) = match index {
        0 => (driven(&mut source)?, (-75, 1), (-301, 4), (-299, 4)),
        1 => (source.number(-3, 2)?, (-3, 2), (-7, 4), (-5, 4)),
        _ => (source.number(3, 8)?, (3, 8), (1, 4), (1, 2)),
    };
    let request = question(&source, target, value, lower, upper)?;
    Ok((source, request))
}

fn refuses(source: &Fixture, request: &RationalEnclosure, expected: InvalidCode) -> TestResult {
    let before = source.cas.counts();
    let fault = source
        .cas
        .check_rational_enclosure(request, &mut ledger()?)
        .err()
        .ok_or("invalid enclosure accepted")?;
    assert_eq!(fault.kind, FaultKind::Invalid(expected));
    assert_eq!(source.cas.counts(), before);
    Ok(())
}

/// The original driven rational expression and both nominal phase components replay exactly.
/// ´test:integration:decision-enclosure-closed-examples´
#[test]
fn decision_enclosure_closed_examples() -> TestResult {
    for index in 0..3 {
        let (source, request) = prepared(index)?;
        let before = source.cas.counts();
        let checked = source
            .cas
            .check_rational_enclosure(&request, &mut ledger()?)?;
        assert_eq!(checked.contract(), &request);
        assert_eq!(source.cas.counts(), before);
        let mut singleton = request;
        if let EnclosureOrigin::ExactRational { value, .. } = &singleton.origin {
            singleton.lower.clone_from(value);
            singleton.upper.clone_from(value);
        }
        source
            .cas
            .check_rational_enclosure(&singleton, &mut ledger()?)?;
    }
    Ok(())
}

/// A replay for another exact target and an incorrect candidate rational both refuse.
/// ´test:integration:decision-enclosure-original-target´
#[test]
fn decision_enclosure_original_target() -> TestResult {
    let (mut source, mut request) = prepared(0)?;
    let original = request.clone();
    request.target = source.number(75, 1)?;
    refuses(&source, &request, InvalidCode::EndpointMismatch)?;
    request.clone_from(&original);
    request.origin = EnclosureOrigin::ExactRational {
        expression: request.target.clone(),
        value: rational(-74, 1)?,
    };
    refuses(&source, &request, InvalidCode::EndpointMismatch)?;
    let foreign = prepared(0)?;
    request.origin = EnclosureOrigin::ExactRational {
        expression: foreign.1.target,
        value: rational(-75, 1)?,
    };
    refuses(&source, &request, InvalidCode::ForeignHandle)
}

/// Kernel and provenance-only claims have no exact replay authority.
/// ´test:integration:decision-enclosure-unsupported-origins´
#[test]
fn decision_enclosure_unsupported_origins() -> TestResult {
    let (source, mut request) = prepared(0)?;
    for origin in [
        EnclosureOrigin::ScalarKernel,
        EnclosureOrigin::MatrixKernel,
        EnclosureOrigin::Unverified,
    ] {
        request.origin = origin;
        refuses(&source, &request, InvalidCode::UnsupportedCertificate)?;
    }
    Ok(())
}

/// Reversed endpoints and a box excluding the exact target refuse atomically.
/// ´test:integration:decision-enclosure-false-boxes´
#[test]
fn decision_enclosure_false_boxes() -> TestResult {
    let (source, mut request) = prepared(0)?;
    request.lower = rational(1, 1)?;
    request.upper = rational(2, 1)?;
    refuses(&source, &request, InvalidCode::DecisionBox)?;
    std::mem::swap(&mut request.lower, &mut request.upper);
    refuses(&source, &request, InvalidCode::DecisionBox)
}

/// The exact local context and its admitted family survive; a widened family refuses.
/// ´test:integration:decision-enclosure-qualified-family´
#[test]
fn decision_enclosure_qualified_family() -> TestResult {
    let (mut source, mut request) = prepared(0)?;
    let coordinate = source.term(Operator::Free("a".into()), &[])?;
    let region = source.interval(&coordinate, (1, 1), (2, 1), false)?;
    let local = admit(
        &mut source.cas,
        Draft::Context(ContextDraft::Local {
            parent: Reference::Existing(source.context.clone()),
            region: Reference::Existing(region),
            hypotheses: Vec::new(),
        }),
        &mut ledger()?,
    )?;
    request.context = local;
    request.input_family = source.cas.admissible_region(&request.context)?;
    let checked = source
        .cas
        .check_rational_enclosure(&request, &mut ledger()?)?;
    assert_eq!(checked.contract().context, request.context);
    assert_eq!(checked.contract().input_family, request.input_family);
    request.input_family = source.ambient.clone();
    refuses(&source, &request, InvalidCode::EndpointMismatch)
}

/// Zero multiplication and quotient cancellation never erase an original undefined divisor.
/// ´test:integration:decision-enclosure-hidden-domain´
#[test]
fn decision_enclosure_hidden_domain() -> TestResult {
    let mut source = Fixture::new(Example::Sign)?;
    let zero = source.number(0, 1)?;
    let one = source.number(1, 1)?;
    let invalid = source.term(Operator::Div, &[one, zero.clone()])?;
    let product = source.term(Operator::Mul, &[zero, invalid.clone()])?;
    let quotient = source.term(Operator::Div, &[invalid.clone(), invalid])?;
    for target in [product, quotient] {
        let request = question(&source, target, (0, 1), (-1, 1), (1, 1))?;
        refuses(&source, &request, InvalidCode::RefutedDomain)?;
    }
    Ok(())
}

/// Named constants use exact cancellation only; the LC target 10/Pi has no rational replay.
/// ´test:integration:decision-enclosure-named-constant-boundary´
#[test]
fn decision_enclosure_named_constant_boundary() -> TestResult {
    let mut source = Fixture::new(Example::Integer)?;
    let pi = source.term(Operator::Constant(Constant::Pi), &[])?;
    let quotient = source.term(Operator::Div, &[pi.clone(), pi.clone()])?;
    let request = question(&source, quotient, (1, 1), (1, 1), (1, 1))?;
    source
        .cas
        .check_rational_enclosure(&request, &mut ledger()?)?;
    let ten = source.number(10, 1)?;
    let cycles = source.term(Operator::Div, &[ten, pi])?;
    let request = question(&source, cycles, (51, 16), (203, 64), (205, 64))?;
    refuses(&source, &request, InvalidCode::UnsupportedCertificate)
}

/// Free inputs remain unsupported even when multiplication would cancel them.
/// ´test:integration:decision-enclosure-free-input-boundary´
#[test]
fn decision_enclosure_free_input_boundary() -> TestResult {
    let mut source = Fixture::new(Example::Sign)?;
    let coordinate = source.term(Operator::Free("x".into()), &[])?;
    let zero = source.number(0, 1)?;
    let target = source.term(Operator::Mul, &[zero, coordinate])?;
    let request = question(&source, target, (0, 1), (0, 1), (0, 1))?;
    refuses(&source, &request, InvalidCode::UnsupportedCertificate)
}

/// A checked constant bound introduces no fact about an independent reciprocal coordinate.
/// ´test:integration:decision-enclosure-no-membership-authority´
#[test]
fn decision_enclosure_no_membership_authority() -> TestResult {
    let (mut source, request) = prepared(0)?;
    let before = source.cas.counts();
    source
        .cas
        .check_rational_enclosure(&request, &mut ledger()?)?;
    assert_eq!(source.cas.counts(), before);
    let coordinate = source.term(Operator::Free("a".into()), &[])?;
    let quotient = source.term(Operator::Div, &[coordinate.clone(), coordinate])?;
    let one = source.number(1, 1)?;
    let step = crate::testing::witness::equality(
        &source.context,
        &source.ambient,
        nrz_cas::proof::Primitive::Cancel,
        &quotient,
        &one,
    );
    let witness = source
        .cas
        .check_proof(&source.context, &[step], &mut ledger()?)?;
    assert!(!witness.is_discharged());
    assert_eq!(
        witness.hypotheses(),
        [] as [nrz_cas::domain::HypothesisRef; 0]
    );
    Ok(())
}

fn allowance(kind: BudgetKind, amount: u64) -> Limits {
    let mut limits = Limits::default();
    match kind {
        BudgetKind::Work => limits.work = amount,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = amount,
        BudgetKind::OutputBytes => limits.output_bytes = amount,
        BudgetKind::PayloadBytes => limits.payload_bytes = amount,
        _ => {}
    }
    limits
}

fn boundary(
    source: &Fixture,
    request: &RationalEnclosure,
    kind: BudgetKind,
    amount: u64,
    prior: u64,
) -> TestResult<bool> {
    let limits = allowance(kind, amount);
    let mut budget = Budget::new(limits.clone())?;
    budget.work(prior, Location::new("prior enclosure work"))?;
    let before = source.cas.counts();
    let result = source.cas.check_rational_enclosure(request, &mut budget);
    assert_eq!(source.cas.counts(), before);
    assert_eq!(budget.limits(), &limits);
    match result {
        Ok(_) => Ok(true),
        Err(fault) => {
            assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind));
            assert!(budget.usage().work >= prior);
            Ok(false)
        }
    }
}

/// Measured complete replay Work, staging, private graph payload and returned ownership.
const ENCLOSURE_COSTS: [(u64, u64, u64, u64); 3] = [
    (53_628, 29_187, 5_696, 800),
    (8_785, 22_856, 1_539, 792),
    (9_361, 22_856, 1_539, 792),
];

/// Replay has literal Work, staging, private graph and output N/N−1 boundaries.
/// ´test:integration:decision-enclosure-resource-boundaries´
#[test]
fn decision_enclosure_resource_boundaries() -> TestResult {
    for (index, (work, staged, graph, output)) in ENCLOSURE_COSTS.into_iter().enumerate() {
        let (source, request) = prepared(index)?;
        let mut budget = ledger()?;
        let checked = source.cas.check_rational_enclosure(&request, &mut budget)?;
        assert_eq!(budget.usage().work, work);
        assert_eq!(checked.retained_bytes(), output);
        for (kind, amount) in [
            (BudgetKind::Work, work),
            (BudgetKind::StagedPayloadBytes, staged),
            (BudgetKind::PayloadBytes, graph),
            (BudgetKind::OutputBytes, output),
        ] {
            assert!(boundary(&source, &request, kind, amount, 0)?);
            assert!(!boundary(&source, &request, kind, amount - 1, 0)?);
        }
        assert!(!boundary(&source, &request, BudgetKind::Work, work, 1)?);
        let limits = allowance(BudgetKind::StagedPayloadBytes, staged);
        let mut budget = Budget::new(limits.clone())?;
        let before = source.cas.counts();
        let result =
            budget.with_retained_payload(1, Location::new("retained enclosure input"), |budget| {
                source.cas.check_rational_enclosure(&request, budget)
            });
        assert!(matches!(result, Err(fault) if matches!(fault.kind,
            FaultKind::Exhausted { budget: BudgetKind::StagedPayloadBytes, .. })));
        assert_eq!(budget.limits(), &limits);
        assert_eq!(source.cas.counts(), before);
    }
    Ok(())
}

/// Repeated replay spends the same cumulative Work and exhausted replay publishes nothing.
/// ´test:integration:decision-enclosure-cumulative-ledger´
#[test]
fn decision_enclosure_cumulative_ledger() -> TestResult {
    for (index, (work, _, _, _)) in ENCLOSURE_COSTS.into_iter().enumerate() {
        let (source, request) = prepared(index)?;
        let limits = allowance(BudgetKind::Work, 2 * work);
        let mut budget = Budget::new(limits.clone())?;
        let before = source.cas.counts();
        source.cas.check_rational_enclosure(&request, &mut budget)?;
        source.cas.check_rational_enclosure(&request, &mut budget)?;
        assert_eq!(budget.usage().work, 2 * work);
        let result = source.cas.check_rational_enclosure(&request, &mut budget);
        assert!(matches!(result, Err(fault) if matches!(fault.kind,
            FaultKind::Exhausted { budget: BudgetKind::Work, .. })));
        assert_eq!(budget.usage().work, 2 * work);
        assert_eq!(budget.limits(), &limits);
        assert_eq!(source.cas.counts(), before);
    }
    Ok(())
}
