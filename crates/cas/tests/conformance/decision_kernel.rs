// SPDX-License-Identifier: AGPL-3.0-only
//! Scalar law certificates replay pure rational relations under the original target scope.

use super::decision_baseline::{Example, Fixture};
use super::decision_partitions::rational;
use crate::testing::common::TestResult;
use crate::testing::readings::ledger;
use nrz_cas::decision::{
    EnclosureOrigin, RationalEnclosure, ScalarKernelCertificate, ScalarKernelLaw,
    ScalarKernelRecord,
};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::syntax::{Branch, Operator};
use nrz_cas::{FaultKind, InvalidCode, Location};

/// Construct a known exact rational constant with explicitly covered endpoints.
fn constant(
    value: (i64, u64),
    lower: (i64, u64),
    upper: (i64, u64),
) -> TestResult<ScalarKernelRecord> {
    Ok(ScalarKernelRecord {
        law: ScalarKernelLaw::Rational(rational(value.0, value.1)?),
        lower: rational(lower.0, lower.1)?,
        upper: rational(upper.0, upper.1)?,
    })
}

/// Original constant, reciprocal and positive irrational root with dyadic certificate endpoints.
fn prepared(kind: u8) -> TestResult<(Fixture, RationalEnclosure)> {
    let mut source = Fixture::new(Example::Phase)?;
    let (target, records) = match kind {
        0 => (
            source.number(1, 10)?,
            vec![constant((1, 10), (3, 32), (7, 64))?],
        ),
        1 => {
            let one = source.number(1, 1)?;
            let three = source.number(3, 1)?;
            let target = source.term(Operator::Div, &[one, three])?;
            let records = vec![
                constant((3, 1), (3, 1), (3, 1))?,
                ScalarKernelRecord {
                    law: ScalarKernelLaw::Reciprocal { input: 0 },
                    lower: rational(21, 64)?,
                    upper: rational(11, 32)?,
                },
            ];
            (target, records)
        }
        _ => {
            let two = source.number(2, 1)?;
            let half = source.number(1, 2)?;
            let target = source.term(Operator::GeneralPower(Branch::StrictReal), &[two, half])?;
            let records = vec![
                constant((2, 1), (2, 1), (2, 1))?,
                ScalarKernelRecord {
                    law: ScalarKernelLaw::SquareRoot { input: 0 },
                    lower: rational(181, 128)?,
                    upper: rational(91, 64)?,
                },
            ];
            (target, records)
        }
    };
    let last = records.last().ok_or("missing result")?;
    let input_family = source.cas.admissible_region(&source.context)?;
    let request = RationalEnclosure {
        target: target.clone(),
        context: source.context.clone(),
        input_family: input_family.clone(),
        lower: last.lower.clone(),
        upper: last.upper.clone(),
        origin: EnclosureOrigin::ScalarKernel(Box::new(ScalarKernelCertificate {
            version: 1,
            expression: target,
            context: source.context.clone(),
            input_family,
            records,
        })),
    };
    Ok((source, request))
}

/// Read the untrusted payload for a single precisely targeted mutation.
fn certificate(request: &mut RationalEnclosure) -> TestResult<&mut ScalarKernelCertificate> {
    let EnclosureOrigin::ScalarKernel(value) = &mut request.origin else {
        return Err("missing scalar certificate".into());
    };
    Ok(value)
}

/// Pin the exact rejecting checker and preserve graph state and nonzero prior Work.
fn refuses(
    source: &Fixture,
    request: &RationalEnclosure,
    code: InvalidCode,
    checker: &'static str,
) -> TestResult {
    let before = source.cas.counts();
    let mut budget = ledger()?;
    budget.work(17, Location::new("prior scalar forgery"))?;
    let Err(fault) = source.cas.check_rational_enclosure(request, &mut budget) else {
        panic!("forged scalar certificate must refuse");
    };
    assert_eq!(fault.kind, FaultKind::Invalid(code));
    assert_eq!(fault.location, Location::new(checker));
    assert_eq!(source.cas.counts(), before);
    assert!(budget.usage().work >= 17);
    Ok(())
}

/// Every closed law's exact rational reductions preserve its original enclosure and graph.
/// ´test:integration:decision-kernel-closed-oracles´
/// ´claim:cas:decision-kernel-closed-oracles´
#[test]
fn decision_kernel_closed_oracles() -> TestResult {
    for kind in 0..3 {
        let (source, request) = prepared(kind)?;
        let before = source.cas.counts();
        let checked = source
            .cas
            .check_rational_enclosure(&request, &mut ledger()?)?;
        assert_eq!(checked.contract(), &request);
        assert_eq!(source.cas.counts(), before);
    }
    Ok(())
}

/// Inward roots, zero divisors and forward references fail at their exact endpoint checker.
/// ´test:integration:decision-kernel-endpoint-forgeries´
/// ´claim:cas:decision-kernel-endpoint-forgeries´
#[test]
fn decision_kernel_endpoint_forgeries() -> TestResult {
    let (source, mut request) = prepared(2)?;
    request.upper = request.lower.clone();
    certificate(&mut request)?.records[1].upper = request.upper.clone();
    refuses(
        &source,
        &request,
        InvalidCode::DecisionBox,
        "scalar kernel endpoint relations",
    )?;
    let (source, mut request) = prepared(1)?;
    certificate(&mut request)?.records[0] = constant((0, 1), (0, 1), (0, 1))?;
    refuses(
        &source,
        &request,
        InvalidCode::RefutedDomain,
        "scalar kernel endpoint relations",
    )?;
    let (source, mut request) = prepared(1)?;
    certificate(&mut request)?.records[1].law = ScalarKernelLaw::Reciprocal { input: 1 };
    refuses(
        &source,
        &request,
        InvalidCode::Reference,
        "scalar kernel endpoint relations",
    )
}

/// A nominal point cannot certify a variable on its full family or replace original scope.
/// ´test:integration:decision-kernel-nominal-coverage-refuses´
/// ´claim:cas:decision-kernel-nominal-coverage-refuses´
#[test]
fn decision_kernel_nominal_coverage_refuses() -> TestResult {
    let (mut source, mut request) = prepared(0)?;
    let coordinate = source.term(Operator::Free("a".into()), &[])?;
    let region = source.interval(&coordinate, (1, 1), (2, 1), false)?;
    let local = crate::testing::builders::admit(
        &mut source.cas,
        nrz_cas::syntax::Draft::Context(nrz_cas::domain::ContextDraft::Local {
            parent: nrz_cas::Reference::Existing(source.context.clone()),
            region: nrz_cas::Reference::Existing(region),
            hypotheses: Vec::new(),
        }),
        &mut ledger()?,
    )?;
    request.target = coordinate.clone();
    request.context = local.clone();
    request.input_family = source.cas.admissible_region(&local)?;
    request.lower = rational(3, 2)?;
    request.upper = request.lower.clone();
    let family = request.input_family.clone();
    let candidate = certificate(&mut request)?;
    candidate.expression = coordinate;
    candidate.context = local;
    candidate.input_family = family;
    candidate.records = vec![constant((3, 2), (3, 2), (3, 2))?];
    refuses(
        &source,
        &request,
        InvalidCode::UnsupportedCertificate,
        "scalar kernel coverage",
    )?;
    let (mut source, mut request) = prepared(0)?;
    let coordinate = source.term(Operator::Free("a".into()), &[])?;
    let narrow = source.interval(&coordinate, (1, 1), (2, 1), false)?;
    certificate(&mut request)?.input_family = narrow;
    refuses(
        &source,
        &request,
        InvalidCode::Scope,
        "scalar kernel coverage",
    )?;
    let (source, mut request) = prepared(0)?;
    certificate(&mut request)?.version = 3;
    refuses(
        &source,
        &request,
        InvalidCode::UnsupportedVersion,
        "scalar kernel coverage",
    )
}

/// A correct interval for another exact value cannot certify the original expression.
/// ´test:integration:decision-kernel-target-forgery´
/// ´claim:cas:decision-kernel-target-forgery´
#[test]
fn decision_kernel_target_forgery() -> TestResult {
    let (source, mut request) = prepared(0)?;
    certificate(&mut request)?.records[0] = constant((1, 8), (1, 8), (1, 8))?;
    request.lower = rational(1, 8)?;
    request.upper = rational(1, 8)?;
    refuses(
        &source,
        &request,
        InvalidCode::EndpointMismatch,
        "scalar kernel coverage",
    )
}

/// Complete checking receipt, including seventeen units of prior caller Work.
struct KernelCost {
    /// Complete successful Work ceiling.
    work: u64,
    /// Retained output ownership and final publication charge.
    bytes: u64,
    /// Complete ordered scalar law inventory.
    records: usize,
}

/// Constant, reciprocal and optimized positive-root receipts measured before pinning.
const KERNEL_COSTS: [KernelCost; 3] = [
    KernelCost {
        work: 10_196,
        bytes: 1320,
        records: 1,
    },
    KernelCost {
        work: 22_961,
        bytes: 1720,
        records: 2,
    },
    KernelCost {
        work: 69_071,
        bytes: 1720,
        records: 2,
    },
];

/// Complete checking has literal N/N−1 boundaries, unchanged limits and atomic exhausted output.
/// ´test:integration:decision-kernel-work-boundaries´
/// ´claim:cas:decision-kernel-work-boundaries´
#[test]
fn decision_kernel_work_boundaries() -> TestResult {
    for (kind, cost) in KERNEL_COSTS.iter().enumerate() {
        let (source, mut request) = prepared(kind.try_into()?)?;
        assert_eq!(certificate(&mut request)?.records.len(), cost.records);
        let before = source.cas.counts();
        let limits = Limits {
            work: cost.work,
            ..Limits::default()
        };
        let mut budget = Budget::new(limits.clone())?;
        budget.work(17, Location::new("prior scalar kernel"))?;
        let checked = source.cas.check_rational_enclosure(&request, &mut budget)?;
        assert_eq!(budget.usage().work, cost.work);
        assert_eq!(checked.retained_bytes(), cost.bytes);
        assert_eq!(checked.contract(), &request);
        assert_eq!(source.cas.counts(), before);
        assert_eq!(budget.limits(), &limits);
        let limits = Limits {
            work: cost.work - 1,
            ..Limits::default()
        };
        let mut short = Budget::new(limits.clone())?;
        short.work(17, Location::new("prior scalar kernel"))?;
        let Err(fault) = source.cas.check_rational_enclosure(&request, &mut short) else {
            panic!("scalar receipt must refuse below its complete Work boundary");
        };
        assert!(matches!(fault.kind, FaultKind::Exhausted {
            budget: BudgetKind::Work, limit, needed, ..
        } if limit == cost.work - 1 && needed == cost.work));
        assert_eq!(fault.location, Location::new("exact rational enclosure"));
        assert_eq!(short.usage().work, cost.work - cost.bytes);
        assert_eq!(short.limits(), &limits);
        assert_eq!(source.cas.counts(), before);
    }
    Ok(())
}

/// Sums and products cover the entire signed input boxes rather than just their exact centers.
/// ´test:integration:decision-kernel-full-box-relations´
/// ´claim:cas:decision-kernel-full-box-relations´
#[test]
fn decision_kernel_full_box_relations() -> TestResult {
    for product in [false, true] {
        let (mut source, mut request) = prepared(0)?;
        request.target = source.number(if product { -6 } else { 1 }, 1)?;
        request.lower = rational(if product { -12 } else { -1 }, 1)?;
        request.upper = rational(if product { -2 } else { 3 }, 1)?;
        let records = vec![
            constant((-2, 1), (-3, 1), (-1, 1))?,
            constant((3, 1), (2, 1), (4, 1))?,
            ScalarKernelRecord {
                law: if product {
                    ScalarKernelLaw::Product { left: 0, right: 1 }
                } else {
                    ScalarKernelLaw::Sum { left: 0, right: 1 }
                },
                lower: request.lower.clone(),
                upper: request.upper.clone(),
            },
        ];
        let target = request.target.clone();
        let candidate = certificate(&mut request)?;
        candidate.expression = target;
        candidate.records = records;
        source
            .cas
            .check_rational_enclosure(&request, &mut ledger()?)?;
        // The center's singleton discards the complete operand interval coverage.
        request.lower = rational(if product { -6 } else { 1 }, 1)?;
        request.upper = request.lower.clone();
        let lower = request.lower.clone();
        let upper = request.upper.clone();
        let candidate = certificate(&mut request)?;
        candidate.records[2].lower = lower;
        candidate.records[2].upper = upper;
        refuses(
            &source,
            &request,
            InvalidCode::DecisionBox,
            "scalar kernel endpoint relations",
        )?;
    }
    Ok(())
}

/// Original root preparation and squared bounds retain the measured independent cost components.
/// ´test:integration:decision-kernel-root-cost-components´
/// ´claim:cas:decision-kernel-root-cost-components´
#[test]
fn decision_kernel_root_cost_components() -> TestResult {
    let (source, mut request) = prepared(2)?;
    let mut original = ledger()?;
    nrz_cas::ExactConstant::from_expression(&source.cas, &request.target, &mut original)?;
    let radicand = nrz_cas::ExactConstant::from(rational(2, 1)?);
    let mut reconstructed = ledger()?;
    radicand
        .positive()
        .ok_or("positive radicand")?
        .sqrt(&mut reconstructed)?;
    let records = &certificate(&mut request)?.records;
    let mut relations = ledger()?;
    let lower = records[1].lower.mul(&records[1].lower, &mut relations)?;
    let upper = records[1].upper.mul(&records[1].upper, &mut relations)?;
    lower.compare(&records[0].lower, &mut relations)?;
    records[0].upper.compare(&upper, &mut relations)?;
    assert_eq!(
        [
            original.usage().work,
            reconstructed.usage().work,
            relations.usage().work
        ],
        [57_657, 56_527, 2176]
    );
    Ok(())
}

/// Root reuse checks sign and radicand identity, and intermediate roots still reconstruct fully.
/// ´test:integration:decision-kernel-root-reuse-boundary´
/// ´claim:cas:decision-kernel-root-reuse-boundary´
#[test]
fn decision_kernel_root_reuse_boundary() -> TestResult {
    let (mut source, mut request) = prepared(2)?;
    let negative = source.term(Operator::Neg, std::slice::from_ref(&request.target))?;
    request.target = negative.clone();
    certificate(&mut request)?.expression = negative;
    refuses(
        &source,
        &request,
        InvalidCode::EndpointMismatch,
        "scalar kernel coverage",
    )?;
    let (source, mut request) = prepared(2)?;
    request.lower = rational(221, 128)?;
    request.upper = rational(111, 64)?;
    let lower = request.lower.clone();
    let upper = request.upper.clone();
    let candidate = certificate(&mut request)?;
    candidate.records[0] = constant((3, 1), (3, 1), (3, 1))?;
    candidate.records[1].lower = lower;
    candidate.records[1].upper = upper;
    refuses(
        &source,
        &request,
        InvalidCode::EndpointMismatch,
        "scalar kernel coverage",
    )?;
    let (mut source, mut request) = prepared(2)?;
    request.target = source.number(2, 1)?;
    request.lower = rational(2, 1)?;
    request.upper = rational(2, 1)?;
    let target = request.target.clone();
    let candidate = certificate(&mut request)?;
    candidate.expression = target;
    candidate.records[0] = constant((4, 1), (4, 1), (4, 1))?;
    candidate.records[1].lower = rational(2, 1)?;
    candidate.records[1].upper = rational(2, 1)?;
    source
        .cas
        .check_rational_enclosure(&request, &mut ledger()?)?;
    let (mut source, mut request) = prepared(2)?;
    request.target = source.number(2, 1)?;
    request.lower = rational(1, 1)?;
    request.upper = rational(3, 1)?;
    let target = request.target.clone();
    let candidate = certificate(&mut request)?;
    candidate.expression = target;
    candidate.records.push(ScalarKernelRecord {
        law: ScalarKernelLaw::Product { left: 1, right: 1 },
        lower: rational(1, 1)?,
        upper: rational(3, 1)?,
    });
    source
        .cas
        .check_rational_enclosure(&request, &mut ledger()?)?;
    Ok(())
}

/// Version-two native input laws prove full sets, while negative radii and version-one claims refuse.
/// ´test:integration:decision-kernel-native-input-domains´
/// ´claim:cas:decision-kernel-native-input-domains´
#[test]
fn decision_kernel_native_input_domains() -> TestResult {
    let (source, mut request) = prepared(0)?;
    let candidate = certificate(&mut request)?;
    candidate.version = 2;
    candidate.records[0].law =
        ScalarKernelLaw::Input(Box::new(nrz_cas::decision::ScalarKernelInput {
            center: rational(1, 10)?,
            radius: rational(1, 256)?,
            value: nrz_cas::decision::ScalarKernelInputValue::Rational(rational(1, 10)?),
        }));
    source
        .cas
        .check_rational_enclosure(&request, &mut ledger()?)?;
    let mut legacy = request.clone();
    certificate(&mut legacy)?.version = 1;
    refuses(
        &source,
        &legacy,
        InvalidCode::UnsupportedVersion,
        "scalar kernel coverage",
    )?;
    let ScalarKernelLaw::Input(input) = &mut certificate(&mut request)?.records[0].law else {
        return Err("missing native input".into());
    };
    input.radius = rational(-1, 256)?;
    refuses(
        &source,
        &request,
        InvalidCode::RefutedDomain,
        "scalar kernel input coverage",
    )
}
