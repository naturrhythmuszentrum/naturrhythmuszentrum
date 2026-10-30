// SPDX-License-Identifier: AGPL-3.0-only
//! Physical source boxes remain qualified assumptions throughout exact kernel replay.

mod mapped;

use super::{Fixture, TestResult, certificate, constant, ledger, prepared, rational};
use crate::testing::builders::admit;
use nrz_cas::decision::{
    RationalEnclosure, ScalarKernelFamily, ScalarKernelFamilyInput, ScalarKernelLaw,
    ScalarKernelRecord,
};
use nrz_cas::domain::{AssumptionDraft, ContextDraft, HypothesisRef};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::syntax::{Draft, Operator, PredicateKind};
use nrz_cas::{FaultKind, InvalidCode, Location, Reference};

/// One actual source and two exact nonnegative endpoint predicates precede the certificate.
fn family() -> TestResult<(Fixture, RationalEnclosure)> {
    let (mut source, mut request) = prepared(0)?;
    let coordinate = source.term(Operator::Free("a".into()), &[])?;
    let lower = source.number(1, 1)?;
    let upper = source.number(2, 1)?;
    let differences = [
        source.term(Operator::Sub, &[coordinate.clone(), lower])?,
        source.term(Operator::Sub, &[upper, coordinate.clone()])?,
    ];
    let mut assumptions = Vec::new();
    for (index, difference) in differences.into_iter().enumerate() {
        let predicate = admit(
            &mut source.cas,
            Draft::Condition {
                header: Reference::Existing(source.header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::NonNegative,
                arguments: vec![Reference::Existing(difference)],
            },
            &mut ledger()?,
        )?;
        assumptions.push(AssumptionDraft {
            key: format!("physical-source-bound-{index}"),
            condition: Reference::Existing(predicate),
        });
    }
    source.context = admit(
        &mut source.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(source.header.clone()),
            admissible: Reference::Existing(source.ambient.clone()),
            assumptions,
        }),
        &mut ledger()?,
    )?;
    let family = ScalarKernelFamily {
        source: coordinate.clone(),
        lower: rational(1, 1)?,
        upper: rational(2, 1)?,
        premises: [0, 1].map(|slot| HypothesisRef {
            context: source.context.clone(),
            slot,
        }),
    };
    let quarter = source.number(1, 4)?;
    let target = source.term(Operator::Div, &[coordinate, quarter])?;
    request.target = target.clone();
    request.context = source.context.clone();
    request.input_family = source.ambient.clone();
    request.lower = rational(4, 1)?;
    request.upper = rational(8, 1)?;
    let candidate = certificate(&mut request)?;
    candidate.version = 3;
    candidate.expression = target;
    candidate.context = source.context.clone();
    candidate.input_family = source.ambient.clone();
    candidate.records = vec![
        ScalarKernelRecord {
            law: ScalarKernelLaw::Family(Box::new(ScalarKernelFamilyInput {
                center: rational(3, 2)?,
                radius: rational(1, 2)?,
                family,
            })),
            lower: rational(1, 1)?,
            upper: rational(2, 1)?,
        },
        constant((1, 4), (1, 4), (1, 4))?,
        ScalarKernelRecord {
            law: ScalarKernelLaw::Reciprocal { input: 1 },
            lower: rational(4, 1)?,
            upper: rational(4, 1)?,
        },
        ScalarKernelRecord {
            law: ScalarKernelLaw::Product { left: 0, right: 2 },
            lower: rational(4, 1)?,
            upper: rational(8, 1)?,
        },
    ];
    Ok((source, request))
}

/// The checked family receipt covers every source value and exposes both retained assumptions.
/// ´test:integration:decision-kernel-family-receipt´
/// ´claim:cas:decision-kernel-family-receipt´
#[test]
fn decision_kernel_family_receipt() -> TestResult {
    let (mut source, request) = family()?;
    let before = source.cas.counts();
    let mut budget = ledger()?;
    budget.work(17, Location::new("prior family receipt"))?;
    let checked = source.cas.check_rational_enclosure(&request, &mut budget)?;
    assert_eq!(checked.contract(), &request);
    let premises = checked.input_premises().collect::<Vec<_>>();
    assert_eq!(premises.len(), 2);
    for (index, premise) in premises.iter().enumerate() {
        assert_eq!(premise.context, source.context);
        assert_eq!(premise.slot, u32::try_from(index)?);
    }
    assert_eq!(source.cas.counts(), before);
    assert_eq!(budget.usage().work, 22_007);
    Ok(())
}

/// A fresh receipt succeeds at its measured ceiling and refuses atomically one unit below it.
/// ´test:integration:decision-kernel-family-work-boundary´
/// ´claim:cas:decision-kernel-family-work-boundary´
#[test]
fn decision_kernel_family_work_boundary() -> TestResult {
    for work in [22_007, 22_006] {
        let (mut source, request) = family()?;
        let before = source.cas.counts();
        let mut budget = Budget::new(Limits {
            work,
            ..Limits::default()
        })?;
        let limits = budget.limits().clone();
        budget.work(17, Location::new("prior family boundary"))?;
        let result = source.cas.check_rational_enclosure(&request, &mut budget);
        if work == 22_007 {
            assert_eq!(result?.contract(), &request);
            assert_eq!(budget.usage().work, work);
        } else {
            let Err(fault) = result else {
                panic!("one-below family receipt must exhaust Work");
            };
            let FaultKind::Exhausted {
                budget: kind,
                limit,
                needed,
                ..
            } = fault.kind
            else {
                panic!("expected family receipt Work exhaustion");
            };
            assert_eq!(kind, BudgetKind::Work);
            assert_eq!(limit, work);
            assert_eq!(needed, 22_007);
            assert_eq!(fault.location, Location::new("exact rational enclosure"));
            assert!(budget.usage().work >= 17);
        }
        assert_eq!(source.cas.counts(), before);
        assert_eq!(budget.limits(), &limits);
    }
    Ok(())
}

/// A changed exact family endpoint cannot inherit the original qualified assumption.
/// ´test:integration:decision-kernel-family-box-forgery´
/// ´claim:cas:decision-kernel-family-box-forgery´
#[test]
fn decision_kernel_family_box_forgery() -> TestResult {
    forgery(
        0,
        InvalidCode::EndpointMismatch,
        "scalar kernel family premise",
        1460,
    )
}

/// A nominal input singleton cannot replace coverage of a non-singleton physical family.
/// ´test:integration:decision-kernel-family-nominal-forgery´
/// ´claim:cas:decision-kernel-family-nominal-forgery´
#[test]
fn decision_kernel_family_nominal_forgery() -> TestResult {
    forgery(
        1,
        InvalidCode::DecisionBox,
        "scalar kernel family coverage",
        3322,
    )
}

/// The exact source identity belongs to both endpoint predicates and cannot be renamed.
/// ´test:integration:decision-kernel-family-source-forgery´
/// ´claim:cas:decision-kernel-family-source-forgery´
#[test]
fn decision_kernel_family_source_forgery() -> TestResult {
    forgery(
        2,
        InvalidCode::EndpointMismatch,
        "scalar kernel family premise",
        1460,
    )
}

/// Correct family bounds cannot certify a different original variable with the same proposed box.
/// ´test:integration:decision-kernel-family-original-forgery´
/// ´claim:cas:decision-kernel-family-original-forgery´
#[test]
fn decision_kernel_family_original_forgery() -> TestResult {
    for work in [11_900, 11_899] {
        let (mut source, mut request) = family()?;
        let other = source.term(Operator::Free("b".into()), &[])?;
        let quarter = source.number(1, 4)?;
        let target = source.term(Operator::Div, &[other, quarter])?;
        request.target = target.clone();
        certificate(&mut request)?.expression = target;
        let before = source.cas.counts();
        let mut budget = Budget::new(Limits {
            work,
            ..Limits::default()
        })?;
        let limits = budget.limits().clone();
        budget.work(17, Location::new("prior original forgery"))?;
        let Err(fault) = source.cas.check_rational_enclosure(&request, &mut budget) else {
            panic!("a different original source must refuse");
        };
        if work == 11_900 {
            assert_eq!(
                fault.kind,
                FaultKind::Invalid(InvalidCode::EndpointMismatch)
            );
            assert_eq!(
                fault.location,
                Location::new("scalar kernel family original")
            );
            assert_eq!(budget.usage().work, work);
        } else {
            let FaultKind::Exhausted {
                budget: kind,
                limit,
                needed,
                ..
            } = fault.kind
            else {
                panic!("one-below original correspondence must exhaust Work");
            };
            assert_eq!(kind, BudgetKind::Work);
            assert_eq!(limit, work);
            assert_eq!(needed, 11_900);
            assert!(budget.usage().work >= 17);
        }
        assert_eq!(source.cas.counts(), before);
        assert_eq!(budget.limits(), &limits);
    }
    Ok(())
}

/// Fresh-store refusals preserve the complete graph and all previously spent caller Work.
fn forgery(kind: u8, code: InvalidCode, checker: &'static str, work: u64) -> TestResult {
    let (mut source, mut request) = family()?;
    let other = source.term(Operator::Free("b".into()), &[])?;
    let record = &mut certificate(&mut request)?.records[0];
    let ScalarKernelLaw::Family(input) = &mut record.law else {
        return Err("family input".into());
    };
    match kind {
        0 => input.family.lower = rational(3, 2)?,
        1 => {
            input.radius = rational(0, 1)?;
            record.lower = input.center.clone();
            record.upper = input.center.clone();
        }
        2 => input.family.source = other,
        _ => return Err("unknown forgery".into()),
    }
    let before = source.cas.counts();
    let mut budget = ledger()?;
    budget.work(17, Location::new("prior family forgery"))?;
    let Err(fault) = source.cas.check_rational_enclosure(&request, &mut budget) else {
        panic!("forged physical input must refuse");
    };
    assert_eq!(fault.kind, FaultKind::Invalid(code));
    assert_eq!(fault.location, Location::new(checker));
    assert_eq!(source.cas.counts(), before);
    assert_eq!(budget.usage().work, work);
    Ok(())
}
