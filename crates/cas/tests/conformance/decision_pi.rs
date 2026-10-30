// SPDX-License-Identifier: AGPL-3.0-only
//! Original pi expressions replay with retained checked constant certificates.

use super::decision_baseline::{Example, Fixture};
use super::decision_partitions::rational;
use super::pi_bounds::cycle;
use crate::testing::common::TestResult;
use crate::testing::readings::ledger;
use nrz_cas::decision::{EnclosureOrigin, RationalEnclosure};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::syntax::{Constant, Operator};
use nrz_cas::{FaultKind, Handle, InvalidCode, Location};

fn target(source: &mut Fixture, index: usize) -> TestResult<Handle> {
    let pi = source.term(Operator::Constant(Constant::Pi), &[])?;
    match index {
        0 => Ok(pi),
        1 => {
            let ten = source.number(10, 1)?;
            source.term(Operator::Div, &[ten, pi])
        }
        2 => {
            let two = source.number(2, 1)?;
            source.term(Operator::Div, &[pi, two])
        }
        3 => source.term(Operator::Neg, &[pi]),
        4 => source.term(Operator::Add, &[pi.clone(), pi]),
        5 => {
            let one = source.number(1, 1)?;
            source.term(Operator::Sub, &[pi, one])
        }
        6 => {
            let factor = source.number(-2, 1)?;
            source.term(Operator::Mul, &[factor, pi])
        }
        7 => source.term(Operator::IntegerPower(2), &[pi]),
        8 => source.term(Operator::IntegerPower(-2), &[pi]),
        _ => source.term(Operator::Div, &[pi.clone(), pi]),
    }
}

fn question(
    source: &Fixture,
    target: Handle,
    lower: (i64, u64),
    upper: (i64, u64),
) -> TestResult<RationalEnclosure> {
    Ok(RationalEnclosure {
        origin: EnclosureOrigin::PiBound {
            expression: target.clone(),
            certificate: Box::new(cycle()?),
        },
        target,
        context: source.context.clone(),
        input_family: source.cas.admissible_region(&source.context)?,
        lower: rational(lower.0, lower.1)?,
        upper: rational(upper.0, upper.1)?,
    })
}

fn prepared() -> TestResult<(Fixture, RationalEnclosure)> {
    let mut source = Fixture::new(Example::Integer)?;
    let target = target(&mut source, 1)?;
    let request = question(&source, target, (203, 64), (205, 64))?;
    Ok((source, request))
}

/// Rational multiples, reciprocals, powers and cancellation keep their original domains.
/// ´test:integration:decision-pi-closed-targets´
#[test]
fn decision_pi_closed_targets() -> TestResult {
    for (index, (lower, upper)) in [
        ((3, 1), (4, 1)),
        ((203, 64), (205, 64)),
        ((1, 1), (2, 1)),
        ((-4, 1), (-3, 1)),
        ((6, 1), (7, 1)),
        ((2, 1), (3, 1)),
        ((-7, 1), (-6, 1)),
        ((9, 1), (10, 1)),
        ((1, 10), (1, 9)),
        ((1, 1), (1, 1)),
    ]
    .into_iter()
    .enumerate()
    {
        let mut source = Fixture::new(Example::Integer)?;
        let target = target(&mut source, index)?;
        let request = question(&source, target, lower, upper)?;
        let before = source.cas.counts();
        let checked = source
            .cas
            .check_rational_enclosure(&request, &mut ledger()?)?;
        assert_eq!(checked.contract(), &request);
        assert_eq!(
            checked.constant_schema(),
            Some(&nrz_cas::proof::CONSTANT_SCHEMAS[0])
        );
        super::pi_bounds::graph_after_citation(&source.cas, before, true);
    }
    Ok(())
}

fn refusal(source: &mut Fixture, request: &RationalEnclosure, code: InvalidCode) -> TestResult {
    let before = source.cas.counts();
    let snapshot = source.cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = ledger()?;
    let fault = source
        .cas
        .check_rational_enclosure(request, &mut budget)
        .err()
        .ok_or("false original pi enclosure accepted")?;
    assert_eq!(fault.kind, FaultKind::Invalid(code));
    assert_eq!(source.cas.counts(), before);
    assert_eq!(source.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    assert!(budget.usage().work > 0);
    Ok(())
}

/// A missing, inward, nominal-only or insufficiently narrow constant bound refuses atomically.
/// ´test:integration:decision-pi-refusals´
#[test]
fn decision_pi_refusals() -> TestResult {
    let (mut source, original) = prepared()?;
    let mut request = original.clone();
    request.origin = EnclosureOrigin::ExactRational {
        expression: request.target.clone(),
        value: rational(51, 16)?,
    };
    refusal(&mut source, &request, InvalidCode::UnsupportedCertificate)?;
    request.clone_from(&original);
    let EnclosureOrigin::PiBound { certificate, .. } = &mut request.origin else {
        return Err("missing pi certificate".into());
    };
    certificate.lower = rational(3, 1)?;
    certificate.upper = rational(4, 1)?;
    refusal(&mut source, &request, InvalidCode::DecisionBox)?;
    request.clone_from(&original);
    request.lower = rational(51, 16)?;
    request.upper = rational(51, 16)?;
    refusal(&mut source, &request, InvalidCode::DecisionBox)?;
    request.clone_from(&original);
    let EnclosureOrigin::PiBound { certificate, .. } = &mut request.origin else {
        return Err("missing pi certificate".into());
    };
    certificate.upper = rational(3, 1)?;
    refusal(&mut source, &request, InvalidCode::DecisionBox)?;
    Ok(())
}

/// The new constant theorem neither approximates e nor hides an original undefined operand.
/// ´test:integration:decision-pi-original-domain´
#[test]
fn decision_pi_original_domain() -> TestResult {
    let mut source = Fixture::new(Example::Integer)?;
    let e = source.term(Operator::Constant(Constant::E), &[])?;
    let request = question(&source, e, (2, 1), (3, 1))?;
    refusal(&mut source, &request, InvalidCode::UnsupportedCertificate)?;
    let pi = source.term(Operator::Constant(Constant::Pi), &[])?;
    let zero = source.number(0, 1)?;
    let invalid = source.term(Operator::Div, &[pi, zero.clone()])?;
    let hidden = source.term(Operator::Mul, &[zero, invalid])?;
    let request = question(&source, hidden, (0, 1), (0, 1))?;
    refusal(&mut source, &request, InvalidCode::RefutedDomain)
}

/// The complete LC replay includes its independently checked pi certificate.
/// ´test:integration:decision-pi-costs´
#[test]
fn decision_pi_costs() -> TestResult {
    let (mut source, request) = prepared()?;
    let mut budget = ledger()?;
    let checked = source.cas.check_rational_enclosure(&request, &mut budget)?;
    assert_eq!(
        (budget.usage().work, checked.retained_bytes()),
        (57_786, 1_336)
    );
    Ok(())
}

/// Literal Work and returned-ownership boundaries preserve the store and cumulative ledger.
/// ´test:integration:decision-pi-resources´
/// ´claim:cas:decision-pi-resources´
#[test]
fn decision_pi_resources() -> TestResult {
    for (kind, amount, prior, accepted) in [
        (BudgetKind::Work, 57_786, 0, true),
        (BudgetKind::Work, 57_785, 0, false),
        (BudgetKind::Work, 57_786, 1, false),
        (BudgetKind::Work, 57_787, 1, true),
        (BudgetKind::OutputBytes, 1_336, 0, true),
        (BudgetKind::OutputBytes, 1_335, 0, false),
    ] {
        let (mut source, request) = prepared()?;
        let before = source.cas.counts();
        let snapshot = source.cas.canonical_snapshot(&mut ledger()?)?;
        let mut limits = Limits::default();
        if kind == BudgetKind::Work {
            limits.work = amount;
        } else {
            limits.output_bytes = amount;
        }
        let mut budget = Budget::new(limits.clone())?;
        budget.work(prior, Location::new("prior pi replay work"))?;
        let result = source.cas.check_rational_enclosure(&request, &mut budget);
        if accepted {
            assert_eq!(result?.contract(), &request);
        } else {
            let fault = result.err().ok_or("pi replay exceeded budget")?;
            assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind));
        }
        assert!(budget.usage().work > prior);
        assert_eq!(budget.limits(), &limits);
        super::pi_bounds::graph_after_citation(&source.cas, before, accepted);
        if !accepted {
            assert_eq!(source.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
        }
    }
    Ok(())
}
