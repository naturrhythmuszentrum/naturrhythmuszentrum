// SPDX-License-Identifier: AGPL-3.0-only
//! Measure the closed checker boundary before any physical-family law is admitted.

use super::{Fixture, TestResult, certificate, constant, ledger, prepared, rational};
use nrz_cas::decision::{
    RationalEnclosure, ScalarKernelInput, ScalarKernelInputValue, ScalarKernelLaw,
    ScalarKernelRecord,
};
use nrz_cas::domain::ContextDraft;
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::syntax::{Draft, Operator};
use nrz_cas::{FaultKind, InvalidCode, Location, Reference};

/// Keep the original free input, its full interval and an optional affine reading in one scope.
fn family(mapped: bool) -> TestResult<(Fixture, RationalEnclosure)> {
    let (mut source, mut request) = prepared(0)?;
    let coordinate = source.term(Operator::Free("a".into()), &[])?;
    let region = source.interval(&coordinate, (1, 1), (2, 1), false)?;
    let context = crate::testing::builders::admit(
        &mut source.cas,
        Draft::Context(ContextDraft::Local {
            parent: Reference::Existing(source.context.clone()),
            region: Reference::Existing(region),
            hypotheses: Vec::new(),
        }),
        &mut ledger()?,
    )?;
    let numerator = if mapped {
        let two = source.number(2, 1)?;
        let one = source.number(1, 1)?;
        let scaled = source.term(Operator::Mul, &[two, coordinate])?;
        source.term(Operator::Add, &[scaled, one])?
    } else {
        coordinate
    };
    let denominator = source.number(1, 4)?;
    let target = source.term(Operator::Div, &[numerator, denominator])?;
    let (center, radius, lower, upper) = if mapped {
        ((4, 1), (1, 1), (3, 1), (5, 1))
    } else {
        ((3, 2), (1, 2), (1, 1), (2, 1))
    };
    let first = ScalarKernelRecord {
        law: ScalarKernelLaw::Input(Box::new(ScalarKernelInput {
            center: rational(center.0, center.1)?,
            radius: rational(radius.0, radius.1)?,
            value: ScalarKernelInputValue::Rational(rational(center.0, center.1)?),
        })),
        lower: rational(lower.0, lower.1)?,
        upper: rational(upper.0, upper.1)?,
    };
    let records = vec![
        first,
        constant((1, 4), (1, 4), (1, 4))?,
        ScalarKernelRecord {
            law: ScalarKernelLaw::Reciprocal { input: 1 },
            lower: rational(4, 1)?,
            upper: rational(4, 1)?,
        },
        ScalarKernelRecord {
            law: ScalarKernelLaw::Product { left: 0, right: 2 },
            lower: rational(4 * lower.0, 1)?,
            upper: rational(4 * upper.0, 1)?,
        },
    ];
    request.target = target.clone();
    request.context = context.clone();
    request.input_family = source.cas.admissible_region(&context)?;
    request.lower = records[3].lower.clone();
    request.upper = records[3].upper.clone();
    let input_family = request.input_family.clone();
    let candidate = certificate(&mut request)?;
    candidate.version = 2;
    candidate.expression = target;
    candidate.context = context;
    candidate.input_family = input_family;
    candidate.records = records;
    Ok((source, request))
}

/// A full numerical input box still lacks the retained mathematical source-family premise.
/// ´test:integration:decision-kernel-family-coverage-baseline´
/// ´claim:cas:decision-kernel-family-coverage-baseline´
#[test]
fn decision_kernel_family_coverage_baseline() -> TestResult {
    baseline(false, [1264, 1263])
}

/// An affine expression still lacks the source premise and authenticated reading definition.
/// ´test:integration:decision-kernel-mapped-coverage-baseline´
/// ´claim:cas:decision-kernel-mapped-coverage-baseline´
#[test]
fn decision_kernel_mapped_coverage_baseline() -> TestResult {
    baseline(true, [3474, 3473])
}

/// Measure a fresh store's refusal, preserving prior Work and every graph record.
fn baseline(mapped: bool, boundaries: [u64; 2]) -> TestResult {
    for work in boundaries {
        let (mut source, request) = family(mapped)?;
        let before = source.cas.counts();
        let mut budget = Budget::new(Limits {
            work,
            ..Limits::default()
        })?;
        let limits = budget.limits().clone();
        budget.work(17, Location::new("prior input coverage"))?;
        let Err(fault) = source.cas.check_rational_enclosure(&request, &mut budget) else {
            panic!("open original target must refuse closed nominal records");
        };
        if work == boundaries[0] {
            assert_eq!(
                fault.kind,
                FaultKind::Invalid(InvalidCode::UnsupportedCertificate)
            );
            assert_eq!(fault.location, Location::new("scalar kernel coverage"));
            assert_eq!(budget.usage().work, work);
        } else {
            let FaultKind::Exhausted {
                budget: kind,
                limit,
                needed,
                ..
            } = fault.kind
            else {
                panic!("one-below source replay must exhaust Work");
            };
            assert_eq!(kind, BudgetKind::Work);
            assert_eq!(limit, work);
            assert_eq!(needed, boundaries[0]);
            assert!(budget.usage().work >= 17);
        }
        assert_eq!(source.cas.counts(), before);
        assert_eq!(budget.limits(), &limits);
    }
    Ok(())
}
