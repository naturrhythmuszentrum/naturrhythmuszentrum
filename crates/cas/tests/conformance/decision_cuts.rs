// SPDX-License-Identifier: AGPL-3.0-only
//! Independently specified rational cut acceptance and refusal fixtures.

use super::decision_baseline::{Example, Fixture};
use crate::testing::builders::{admit, environment, expression, expression_draft};
use crate::testing::common::{TestResult, real};
use crate::testing::readings::ledger;
use nrz_cas::decision::{CaseCut, CoordinateInterval, IntegerCellRange};
use nrz_cas::domain::{CaseArmDraft, CasesDraft, HeaderDraft};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::syntax::{AtomicSort, Constant, Draft, Field, Operator, PredicateKind, Sort};
use nrz_cas::{FaultKind, Handle, InvalidCode, Location, Rational, Reference};

fn rational(numerator: i64, denominator: u64) -> TestResult<Rational> {
    Ok(Rational::from_ratio(
        numerator,
        denominator,
        &mut ledger()?,
    )?)
}

fn interval(
    coordinate: Handle,
    lower: (i64, u64),
    upper: (i64, u64),
) -> TestResult<CoordinateInterval> {
    Ok(CoordinateInterval {
        coordinate,
        lower: rational(lower.0, lower.1)?,
        upper: rational(upper.0, upper.1)?,
    })
}

fn association(
    fixture: &mut Fixture,
    region: Handle,
    value: Handle,
) -> TestResult<(Handle, Handle)> {
    let sort = fixture.cas.expression(&value)?.1.clone();
    let arm = admit(
        &mut fixture.cas,
        Draft::CaseArm(CaseArmDraft {
            placements: Vec::new(),
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            region: Reference::Existing(region),
            value: Reference::Existing(value),
        }),
        &mut ledger()?,
    )?;
    let function = admit(
        &mut fixture.cas,
        Draft::Cases(CasesDraft {
            sort,
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            domain: Reference::Existing(fixture.ambient.clone()),
            arms: vec![Reference::Existing(arm.clone())],
        }),
        &mut ledger()?,
    )?;
    Ok((function, arm))
}

fn fixture(example: Example) -> TestResult<(Fixture, CaseCut)> {
    let mut source = Fixture::new(example)?;
    let (_, region) = source.regions(example)?;
    let value = expression(
        &mut source.cas,
        &source.context,
        Operator::Rational(rational(3, 1)?),
        if matches!(example, Example::Integer) {
            Sort::Value(AtomicSort::Integer)
        } else {
            real()
        },
        &[],
        &mut ledger()?,
    )?;
    let (function, selected) = association(&mut source, region, value)?;
    let bounds = match example {
        Example::Integer | Example::Sign => {
            let coordinate = source.term(Operator::Free("x".into()), &[])?;
            let (lower, upper) = if matches!(example, Example::Integer) {
                ((203, 64), (205, 64))
            } else {
                ((-301, 4), (-299, 4))
            };
            vec![interval(coordinate, lower, upper)?]
        }
        Example::Phase => {
            let a = source.term(Operator::Free("a".into()), &[])?;
            let b = source.term(Operator::Free("b".into()), &[])?;
            vec![interval(a, (-7, 4), (-5, 4))?, interval(b, (1, 4), (1, 2))?]
        }
    };
    let integer = if matches!(example, Example::Integer) {
        Some(IntegerCellRange {
            coordinate: bounds[0].coordinate.clone(),
            lower: Some(rational(0, 1)?),
            upper: None,
        })
    } else {
        None
    };
    Ok((
        source,
        CaseCut {
            function,
            association: selected,
            bounds,
            integer,
        },
    ))
}

fn accept(source: &Fixture, cut: &CaseCut) -> TestResult {
    let before = source.cas.counts();
    source.cas.check_case_cut(cut, &mut ledger()?)?;
    assert_eq!(source.cas.counts(), before);
    Ok(())
}

fn refuse(source: &Fixture, cut: &CaseCut, expected: InvalidCode) -> TestResult {
    let before = source.cas.counts();
    let fault = source
        .cas
        .check_case_cut(cut, &mut ledger()?)
        .err()
        .ok_or("false cut accepted")?;
    assert_eq!(fault.kind, FaultKind::Invalid(expected));
    assert_eq!(source.cas.counts(), before);
    Ok(())
}

/// Half-open cells retain their exact lower singleton, strict upper boundary and integer range.
/// ´test:integration:decision-cut-integer-cell´
#[test]
fn decision_cut_integer_cell() -> TestResult {
    let (mut source, mut cut) = fixture(Example::Integer)?;
    accept(&source, &cut)?;
    cut.bounds[0].lower = rational(191, 64)?;
    refuse(&source, &cut, InvalidCode::DecisionBoundaryIntersection)?;
    cut.bounds[0].lower = rational(3, 1)?;
    cut.bounds[0].upper = rational(3, 1)?;
    accept(&source, &cut)?;
    cut.bounds[0].upper = rational(4, 1)?;
    refuse(&source, &cut, InvalidCode::DecisionBoundaryIntersection)?;
    cut.bounds[0].upper = rational(3, 1)?;
    cut.integer.as_mut().ok_or("missing range")?.upper = Some(rational(3, 1)?);
    refuse(&source, &cut, InvalidCode::DecisionIntegerRange)?;
    cut.integer.as_mut().ok_or("missing range")?.upper = None;
    cut.integer.as_mut().ok_or("missing range")?.lower = Some(rational(4, 1)?);
    refuse(&source, &cut, InvalidCode::DecisionIntegerRange)?;
    cut.integer.as_mut().ok_or("missing range")?.lower = Some(rational(1, 2)?);
    refuse(&source, &cut, InvalidCode::DecisionIntegerRange)?;
    cut.integer.as_mut().ok_or("missing range")?.lower = Some(rational(0, 1)?);
    let (_, region) = source.regions(Example::Integer)?;
    let fraction = source.number(3, 2)?;
    (cut.function, cut.association) = association(&mut source, region.clone(), fraction)?;
    refuse(&source, &cut, InvalidCode::DecisionIntegerRange)?;
    let other_index = expression(
        &mut source.cas,
        &source.context,
        Operator::Rational(rational(1, 1)?),
        Sort::Value(AtomicSort::Integer),
        &[],
        &mut ledger()?,
    )?;
    (cut.function, cut.association) = association(&mut source, region, other_index)?;
    refuse(&source, &cut, InvalidCode::DecisionBoundaryIntersection)
}

fn select_region(source: &mut Fixture, cut: &mut CaseCut, region: Handle) -> TestResult {
    let value = source.number(1, 1)?;
    (cut.function, cut.association) = association(source, region, value)?;
    cut.integer = None;
    Ok(())
}

/// Negative and positive boxes prove only their cuts; zero requires an exact singleton.
/// ´test:integration:decision-cut-sign-regions´
#[test]
fn decision_cut_sign_regions() -> TestResult {
    let (mut source, mut cut) = fixture(Example::Sign)?;
    accept(&source, &cut)?;
    cut.bounds[0].lower = rational(-1, 4)?;
    cut.bounds[0].upper = rational(1, 4)?;
    refuse(&source, &cut, InvalidCode::DecisionBoundaryIntersection)?;
    let x = cut.bounds[0].coordinate.clone();
    let region = source.guard(PredicateKind::Positive, std::slice::from_ref(&x))?;
    select_region(&mut source, &mut cut, region)?;
    cut.bounds[0].lower = rational(-1, 1)?;
    cut.bounds[0].upper = rational(1, 1)?;
    refuse(&source, &cut, InvalidCode::DecisionBoundaryIntersection)?;
    cut.bounds[0].lower = rational(299, 4)?;
    cut.bounds[0].upper = rational(301, 4)?;
    accept(&source, &cut)?;
    let zero = source.number(0, 1)?;
    let region = source.guard(PredicateKind::Equal, &[x, zero])?;
    select_region(&mut source, &mut cut, region)?;
    cut.bounds[0].lower = rational(0, 1)?;
    cut.bounds[0].upper = rational(0, 1)?;
    accept(&source, &cut)?;
    cut.bounds[0].upper = rational(1, 4)?;
    refuse(&source, &cut, InvalidCode::DecisionBoundaryIntersection)
}

/// The full upper-left rectangle fits its chart; meeting the owned real axis refuses that chart.
/// ´test:integration:decision-cut-phase-chart´
#[test]
fn decision_cut_phase_chart() -> TestResult {
    let (mut source, mut cut) = fixture(Example::Phase)?;
    accept(&source, &cut)?;
    cut.bounds[1].lower = rational(-1, 8)?;
    cut.bounds[1].upper = rational(1, 8)?;
    refuse(&source, &cut, InvalidCode::DecisionBoundaryIntersection)?;
    cut.bounds[1].lower = rational(0, 1)?;
    refuse(&source, &cut, InvalidCode::DecisionBoundaryIntersection)?;
    cut.bounds[1].upper = rational(0, 1)?;
    refuse(&source, &cut, InvalidCode::DecisionOutsideRegion)?;
    let a = cut.bounds[0].coordinate.clone();
    let b = cut.bounds[1].coordinate.clone();
    let negative = source.term(Operator::Neg, &[a])?;
    let left = source.guard(PredicateKind::Positive, &[negative])?;
    let zero = source.number(0, 1)?;
    let axis = source.guard(PredicateKind::Equal, &[b, zero])?;
    let region = source.meet(vec![left, axis])?;
    select_region(&mut source, &mut cut, region)?;
    accept(&source, &cut)
}

/// Exact affine coefficients account for both coordinates and cancel repeated occurrences.
/// ´test:integration:decision-cut-affine-predicates´
#[test]
fn decision_cut_affine_predicates() -> TestResult {
    let (mut source, mut cut) = fixture(Example::Phase)?;
    let a = cut.bounds[0].coordinate.clone();
    let b = cut.bounds[1].coordinate.clone();
    let two = source.number(2, 1)?;
    let four = source.number(4, 1)?;
    let twice = source.term(Operator::Mul, &[two, a.clone()])?;
    let fourfold = source.term(Operator::Mul, &[four, b])?;
    let sum = source.term(Operator::Add, &[twice, fourfold])?;
    let negative = source.term(Operator::Neg, &[sum])?;
    let region = source.guard(PredicateKind::Positive, &[negative])?;
    select_region(&mut source, &mut cut, region)?;
    accept(&source, &cut)?;
    let difference = source.term(Operator::Sub, &[a.clone(), a])?;
    let zero = source.number(0, 1)?;
    let region = source.guard(PredicateKind::Equal, &[difference, zero])?;
    select_region(&mut source, &mut cut, region)?;
    accept(&source, &cut)
}

/// Names, transcendental cuts and nonlinear or undefined operands grant no cut authority.
/// ´test:integration:decision-cut-unsupported-certificates´
#[test]
fn decision_cut_unsupported_certificates() -> TestResult {
    let (mut source, mut cut) = fixture(Example::Sign)?;
    let x = cut.bounds[0].coordinate.clone();
    let region = source.guard(
        PredicateKind::Named("forged-enclosure-origin".into()),
        std::slice::from_ref(&x),
    )?;
    select_region(&mut source, &mut cut, region)?;
    refuse(&source, &cut, InvalidCode::UnsupportedCertificate)?;
    let pi = source.term(Operator::Constant(Constant::Pi), &[])?;
    let irrational = source.term(Operator::Sub, &[pi, x.clone()])?;
    let square = source.term(Operator::Mul, &[x.clone(), x.clone()])?;
    let zero = source.number(0, 1)?;
    let undefined = source.term(Operator::Div, &[x, zero])?;
    for value in [irrational, square, undefined] {
        let region = source.guard(PredicateKind::Positive, &[value])?;
        select_region(&mut source, &mut cut, region)?;
        refuse(&source, &cut, InvalidCode::UnsupportedCertificate)?;
    }
    Ok(())
}

/// An arm from a wider case, missing coordinates and malformed boxes cannot change the question.
/// ´test:integration:decision-cut-identities-and-boxes´
#[test]
fn decision_cut_identities_and_boxes() -> TestResult {
    let (mut source, cut) = fixture(Example::Phase)?;
    let mut changed = cut.clone();
    changed.bounds.pop();
    refuse(&source, &changed, InvalidCode::DecisionBox)?;
    changed.clone_from(&cut);
    changed.bounds.push(cut.bounds[0].clone());
    refuse(&source, &changed, InvalidCode::DecisionBox)?;
    changed.clone_from(&cut);
    changed.bounds[0].lower = rational(1, 1)?;
    refuse(&source, &changed, InvalidCode::DecisionBox)?;
    changed.clone_from(&cut);
    changed.bounds.clear();
    refuse(&source, &changed, InvalidCode::DecisionBox)?;
    let value = source.number(1, 1)?;
    let ambient = source.ambient.clone();
    let (_, wider) = association(&mut source, ambient, value)?;
    changed.clone_from(&cut);
    changed.association = wider;
    refuse(&source, &changed, InvalidCode::DecisionOptionMismatch)?;
    let (foreign, _) = fixture(Example::Phase)?;
    refuse(&foreign, &cut, InvalidCode::ForeignHandle)
}

/// A pure cut check publishes no payload or witness, even with zero output and retained allowances.
/// ´test:integration:decision-cut-read-only-lifecycle´
#[test]
fn decision_cut_read_only_lifecycle() -> TestResult {
    for example in [Example::Integer, Example::Sign, Example::Phase] {
        let (source, cut) = fixture(example)?;
        let before = source.cas.counts();
        let mut budget = Budget::new(Limits {
            output_bytes: 0,
            payload_bytes: 0,
            ..Limits::default()
        })?;
        source.cas.check_case_cut(&cut, &mut budget)?;
        assert_eq!(source.cas.counts(), before);
        assert!(budget.usage().work > 0);
    }
    Ok(())
}

/// Literal first measurements for each cut under unchanged component and arithmetic ceilings.
const CUT_COSTS: [(Example, u64, u64); 3] = [
    (Example::Integer, 11_670, 51_736),
    (Example::Sign, 4_721, 47_728),
    (Example::Phase, 9_356, 54_992),
];

fn cut_attempt(
    source: &Fixture,
    cut: &CaseCut,
    kind: BudgetKind,
    limit: u64,
    prior: u64,
) -> TestResult<bool> {
    let mut limits = Limits::default();
    match kind {
        BudgetKind::Work => limits.work = limit,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = limit,
        _ => return Err("unsupported cut resource".into()),
    }
    let mut budget = Budget::new(limits.clone())?;
    budget.work(prior, Location::new("prior cut work"))?;
    let before = source.cas.counts();
    let result = source.cas.check_case_cut(cut, &mut budget);
    assert_eq!(source.cas.counts(), before);
    assert_eq!(budget.limits(), &limits);
    match result {
        Ok(()) => Ok(true),
        Err(fault) => {
            assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind));
            Ok(false)
        }
    }
}

/// Exact Work and staged-byte limits admit each cut at N and refuse at N−1.
/// ´test:integration:decision-cut-resource-boundaries´
#[test]
fn decision_cut_resource_boundaries() -> TestResult {
    for (example, work, staged) in CUT_COSTS {
        let (source, cut) = fixture(example)?;
        let mut measured = ledger()?;
        source.cas.check_case_cut(&cut, &mut measured)?;
        assert_eq!(measured.usage().work, work);
        for (kind, needed) in [
            (BudgetKind::Work, work),
            (BudgetKind::StagedPayloadBytes, staged),
        ] {
            assert!(cut_attempt(&source, &cut, kind, needed, 0)?);
            assert!(!cut_attempt(&source, &cut, kind, needed - 1, 0)?);
        }
        assert!(!cut_attempt(&source, &cut, BudgetKind::Work, work, 1)?);
        let mut budget = Budget::new(Limits {
            staged_payload_bytes: staged,
            ..Limits::default()
        })?;
        let limits = budget.limits().clone();
        let before = source.cas.counts();
        let refused =
            budget.with_retained_payload(1, Location::new("retained cut input"), |budget| {
                source.cas.check_case_cut(&cut, budget)
            });
        assert!(matches!(refused, Err(fault) if matches!(fault.kind,
            FaultKind::Exhausted { budget: BudgetKind::StagedPayloadBytes, .. })));
        assert_eq!(budget.limits(), &limits);
        assert_eq!(source.cas.counts(), before);
    }
    Ok(())
}

/// Repeated checking charges the same Work and cannot restart an exhausted cumulative allowance.
/// ´test:integration:decision-cut-cumulative-ledger´
#[test]
fn decision_cut_cumulative_ledger() -> TestResult {
    for (example, work, _) in CUT_COSTS {
        let (source, cut) = fixture(example)?;
        let before = source.cas.counts();
        let mut budget = Budget::new(Limits {
            work: 2 * work,
            ..Limits::default()
        })?;
        source.cas.check_case_cut(&cut, &mut budget)?;
        assert_eq!(budget.usage().work, work);
        source.cas.check_case_cut(&cut, &mut budget)?;
        assert_eq!(budget.usage().work, 2 * work);
        let usage = budget.usage();
        let refused = source.cas.check_case_cut(&cut, &mut budget);
        assert!(matches!(refused, Err(fault) if matches!(fault.kind,
            FaultKind::Exhausted { budget: BudgetKind::Work, .. })));
        assert_eq!(budget.usage(), usage);
        assert_eq!(source.cas.counts(), before);
    }
    Ok(())
}

/// Integer options retain their declared sort and refuse even an integral Real replacement.
/// ´test:integration:decision-cut-integer-result´
#[test]
fn decision_cut_integer_result() -> TestResult {
    let (mut source, mut cut) = fixture(Example::Integer)?;
    assert_eq!(
        source.cas.expression(&cut.function)?.1,
        &Sort::Value(AtomicSort::Integer)
    );
    assert_eq!(source.cas.scalar_ambient(&cut.function)?, None);
    accept(&source, &cut)?;
    let (_, region) = source.regions(Example::Integer)?;
    let value = source.number(3, 1)?;
    (cut.function, cut.association) = association(&mut source, region, value)?;
    refuse(&source, &cut, InvalidCode::DecisionIntegerRange)
}

#[path = "integer_family.rs"]
mod integer_family;

/// A changed declaration header and a narrowed case domain cannot reuse the same box cut.
/// ´test:integration:decision-cut-scope-and-domain´
#[test]
fn decision_cut_scope_and_domain() -> TestResult {
    let (mut source, cut) = fixture(Example::Sign)?;
    let header = admit(
        &mut source.cas,
        Draft::Header(HeaderDraft::Core(environment(Field::Real, &["x", "extra"]))),
        &mut ledger()?,
    )?;
    let coordinate = admit(
        &mut source.cas,
        expression_draft(
            Operator::Free("x".into()),
            real(),
            Reference::Existing(header),
            Vec::new(),
            None,
            &[],
        ),
        &mut ledger()?,
    )?;
    let mut changed = cut.clone();
    changed.bounds[0].coordinate = coordinate;
    refuse(&source, &changed, InvalidCode::Scope)?;
    let x = cut.bounds[0].coordinate.clone();
    let positive = source.guard(PredicateKind::Positive, &[x])?;
    changed.clone_from(&cut);
    changed.function = admit(
        &mut source.cas,
        Draft::Cases(CasesDraft {
            sort: real(),
            header: Reference::Existing(source.header.clone()),
            scope: Vec::new(),
            domain: Reference::Existing(positive),
            arms: vec![Reference::Existing(cut.association)],
        }),
        &mut ledger()?,
    )?;
    refuse(&source, &changed, InvalidCode::DecisionOutsideRegion)
}
