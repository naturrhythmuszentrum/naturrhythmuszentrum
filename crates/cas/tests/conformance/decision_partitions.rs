// SPDX-License-Identifier: AGPL-3.0-only
//! Complete finite decision partitions, authenticated reuse and exact refusal twins.

use super::decision_baseline::{Example, Fixture};
use crate::testing::builders::admit;
use crate::testing::common::TestResult;
use crate::testing::readings::ledger;
use nrz_cas::Location;
use nrz_cas::decision::{CaseCut, CasePartition, CoordinateInterval, IntegerCellRange};
use nrz_cas::domain::{CaseArmDraft, CasesDraft, RegionExpr};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::syntax::{Draft, Operator, PredicateKind};
use nrz_cas::{FaultKind, Handle, InvalidCode, Rational, Reference};

pub(super) struct PartitionFixture {
    pub(super) source: Fixture,
    pub(super) question: CasePartition,
    pub(super) arms: Vec<Handle>,
}

pub(super) fn rational(n: i64, d: u64) -> TestResult<Rational> {
    Ok(Rational::from_ratio(n, d, &mut ledger()?)?)
}

pub(super) fn case(source: &mut Fixture, domain: &Handle, arms: &[Handle]) -> TestResult<Handle> {
    admit(
        &mut source.cas,
        Draft::Cases(CasesDraft {
            header: Reference::Existing(source.header.clone()),
            scope: Vec::new(),
            domain: Reference::Existing(domain.clone()),
            arms: arms.iter().cloned().map(Reference::Existing).collect(),
        }),
        &mut ledger()?,
    )
}

fn association(source: &mut Fixture, region: Handle, value: i64) -> TestResult<Handle> {
    let value = source.number(value, 1)?;
    admit(
        &mut source.cas,
        Draft::CaseArm(CaseArmDraft {
            header: Reference::Existing(source.header.clone()),
            scope: Vec::new(),
            placements: Vec::new(),
            region: Reference::Existing(region),
            value: Reference::Existing(value),
        }),
        &mut ledger()?,
    )
}

fn signs(source: &mut Fixture, coordinate: &Handle) -> TestResult<[Handle; 3]> {
    let minus = source.term(Operator::Neg, std::slice::from_ref(coordinate))?;
    let zero = source.number(0, 1)?;
    Ok([
        source.guard(PredicateKind::Positive, &[minus])?,
        source.guard(PredicateKind::Equal, &[coordinate.clone(), zero])?,
        source.guard(PredicateKind::Positive, std::slice::from_ref(coordinate))?,
    ])
}

fn phase(source: &mut Fixture, coordinates: &[Handle]) -> TestResult<Vec<Handle>> {
    let horizontal = signs(source, &coordinates[0])?;
    let vertical = signs(source, &coordinates[1])?;
    Ok(vec![
        horizontal[2].clone(),
        source.meet(vec![horizontal[1].clone(), vertical[2].clone()])?,
        source.meet(vec![horizontal[1].clone(), vertical[0].clone()])?,
        source.meet(vec![horizontal[0].clone(), vertical[2].clone()])?,
        source.meet(vec![horizontal[0].clone(), vertical[0].clone()])?,
        source.meet(vec![horizontal[0].clone(), vertical[1].clone()])?,
        source.meet(vec![horizontal[1].clone(), vertical[1].clone()])?,
    ])
}

pub(super) fn prepared(example: Example) -> TestResult<PartitionFixture> {
    let mut source = Fixture::new(example)?;
    let keys = if matches!(example, Example::Phase) {
        vec!["a", "b"]
    } else {
        vec!["x"]
    };
    let coordinates = keys
        .into_iter()
        .map(|key| source.term(Operator::Free(key.into()), &[]))
        .collect::<TestResult<Vec<_>>>()?;
    let (domain, regions) = match example {
        Example::Integer => {
            let mut regions = Vec::new();
            for index in 0..5 {
                regions.push(source.interval(&coordinates[0], (index, 1), (index + 1, 1), true)?);
            }
            (
                source.interval(&coordinates[0], (0, 1), (5, 1), true)?,
                regions,
            )
        }
        Example::Sign => (
            source.ambient.clone(),
            signs(&mut source, &coordinates[0])?.into(),
        ),
        Example::Phase => (source.ambient.clone(), phase(&mut source, &coordinates)?),
    };
    let mut arms = Vec::new();
    for (index, region) in regions.into_iter().enumerate() {
        arms.push(association(&mut source, region, i64::try_from(index)?)?);
    }
    let function = case(&mut source, &domain, &arms)?;
    let integer = if matches!(example, Example::Integer) {
        Some(IntegerCellRange {
            coordinate: coordinates[0].clone(),
            lower: Some(rational(0, 1)?),
            upper: Some(rational(5, 1)?),
        })
    } else {
        None
    };
    let question = CasePartition {
        function,
        coordinates,
        integer,
    };
    Ok(PartitionFixture {
        source,
        question,
        arms,
    })
}

pub(super) fn cut(fixture: &PartitionFixture, example: Example) -> TestResult<CaseCut> {
    let endpoints = match example {
        Example::Integer => vec![((203, 64), (205, 64))],
        Example::Sign => vec![((-301, 4), (-299, 4))],
        Example::Phase => vec![((-7, 4), (-5, 4)), ((1, 4), (1, 2))],
    };
    let mut bounds = Vec::new();
    for (coordinate, (lower, upper)) in fixture.question.coordinates.iter().zip(endpoints) {
        bounds.push(CoordinateInterval {
            coordinate: coordinate.clone(),
            lower: rational(lower.0, lower.1)?,
            upper: rational(upper.0, upper.1)?,
        });
    }
    let selected = if matches!(example, Example::Sign) {
        0
    } else {
        3
    };
    Ok(CaseCut {
        function: fixture.question.function.clone(),
        association: fixture.arms[selected].clone(),
        bounds,
        integer: fixture.question.integer.clone(),
    })
}

fn accepts(fixture: &PartitionFixture) -> TestResult {
    let before = fixture.source.cas.counts();
    let checked = fixture
        .source
        .cas
        .check_case_partition(&fixture.question, &mut ledger()?)?;
    assert_eq!(checked.contract(), &fixture.question);
    assert_eq!(fixture.source.cas.counts(), before);
    Ok(())
}

fn refuses(fixture: &PartitionFixture, expected: InvalidCode) -> TestResult {
    let before = fixture.source.cas.counts();
    let fault = fixture
        .source
        .cas
        .check_case_partition(&fixture.question, &mut ledger()?)
        .err()
        .ok_or("false partition accepted")?;
    assert_eq!(fault.kind, FaultKind::Invalid(expected));
    assert_eq!(fixture.source.cas.counts(), before);
    Ok(())
}

/// Integer cells, the complete sign trichotomy and all seven phase charts partition their domains.
/// ´test:integration:decision-partition-complete-examples´
#[test]
fn decision_partition_complete_examples() -> TestResult {
    for example in [Example::Integer, Example::Sign, Example::Phase] {
        let fixture = prepared(example)?;
        accepts(&fixture)?;
        let checked = fixture
            .source
            .cas
            .check_case_partition(&fixture.question, &mut ledger()?)?;
        fixture.source.cas.check_partition_cut(
            &checked,
            &cut(&fixture, example)?,
            &mut ledger()?,
        )?;
    }
    Ok(())
}

/// Missing an integer cell, zero singleton or any owned phase axis refuses exact cover.
/// ´test:integration:decision-partition-missing-members´
#[test]
fn decision_partition_missing_members() -> TestResult {
    for example in [Example::Integer, Example::Sign, Example::Phase] {
        let mut fixture = prepared(example)?;
        let domain = if matches!(example, Example::Integer) {
            fixture
                .source
                .interval(&fixture.question.coordinates[0], (0, 1), (5, 1), true)?
        } else {
            fixture.source.ambient.clone()
        };
        for omitted in 0..fixture.arms.len() {
            let arms = fixture
                .arms
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != omitted)
                .map(|(_, arm)| arm.clone())
                .collect::<Vec<_>>();
            fixture.question.function = case(&mut fixture.source, &domain, &arms)?;
            refuses(&fixture, InvalidCode::CoverageHole)?;
        }
    }
    Ok(())
}

/// Overlap refuses even for equal values; an option outside a narrowed domain also refuses.
/// ´test:integration:decision-partition-overlap-and-domain´
#[test]
fn decision_partition_overlap_and_domain() -> TestResult {
    let mut fixture = prepared(Example::Sign)?;
    let x = fixture.question.coordinates[0].clone();
    let nonnegative = fixture
        .source
        .guard(PredicateKind::NonNegative, std::slice::from_ref(&x))?;
    let overlapping = association(&mut fixture.source, nonnegative, 1)?;
    let mut arms = fixture.arms.clone();
    arms.push(overlapping);
    let ambient = fixture.source.ambient.clone();
    fixture.question.function = case(&mut fixture.source, &ambient, &arms)?;
    refuses(&fixture, InvalidCode::DecisionPartitionOverlap)?;
    let positive = fixture.source.guard(PredicateKind::Positive, &[x])?;
    fixture.question.function = case(&mut fixture.source, &positive, &fixture.arms)?;
    refuses(&fixture, InvalidCode::DecisionPartitionOutsideDomain)?;
    let empty = fixture.source.region(RegionExpr::Empty)?;
    let absent = association(&mut fixture.source, empty.clone(), 0)?;
    fixture.question.function = case(&mut fixture.source, &empty, &[absent])?;
    refuses(&fixture, InvalidCode::EmptyDomain)
}

/// Unbounded, shifted, fractional or mislabelled integer families never inherit cell authority.
/// ´test:integration:decision-partition-integer-contract´
#[test]
fn decision_partition_integer_contract() -> TestResult {
    let mut fixture = prepared(Example::Integer)?;
    let original = fixture.question.clone();
    fixture
        .question
        .integer
        .as_mut()
        .ok_or("missing range")?
        .upper = None;
    refuses(&fixture, InvalidCode::DecisionIntegerRange)?;
    fixture.question.clone_from(&original);
    fixture
        .question
        .integer
        .as_mut()
        .ok_or("missing range")?
        .lower = Some(rational(1, 2)?);
    refuses(&fixture, InvalidCode::DecisionIntegerRange)?;
    fixture.question.clone_from(&original);
    fixture
        .question
        .integer
        .as_mut()
        .ok_or("missing range")?
        .upper = Some(rational(6, 1)?);
    refuses(&fixture, InvalidCode::DecisionIntegerRange)?;
    fixture.question.clone_from(&original);
    let x = fixture.question.coordinates[0].clone();
    let region = fixture.source.interval(&x, (0, 1), (1, 1), true)?;
    fixture.arms[0] = association(&mut fixture.source, region, 1)?;
    let domain = fixture.source.interval(&x, (0, 1), (5, 1), true)?;
    fixture.question.function = case(&mut fixture.source, &domain, &fixture.arms)?;
    refuses(&fixture, InvalidCode::DecisionIntegerRange)
}

/// Sealed reuse rejects a different case revision, range, coordinate inventory and store.
/// ´test:integration:decision-partition-qualified-reuse´
#[test]
fn decision_partition_qualified_reuse() -> TestResult {
    let mut fixture = prepared(Example::Integer)?;
    let checked = fixture
        .source
        .cas
        .check_case_partition(&fixture.question, &mut ledger()?)?;
    let original = cut(&fixture, Example::Integer)?;
    let mut changed = original.clone();
    changed.integer.as_mut().ok_or("missing range")?.upper = Some(rational(6, 1)?);
    assert!(
        fixture
            .source
            .cas
            .check_partition_cut(&checked, &changed, &mut ledger()?)
            .is_err()
    );
    changed.clone_from(&original);
    changed.bounds.clear();
    assert!(
        fixture
            .source
            .cas
            .check_partition_cut(&checked, &changed, &mut ledger()?)
            .is_err()
    );
    changed.clone_from(&original);
    let domain = fixture.source.ambient.clone();
    changed.function = case(&mut fixture.source, &domain, &fixture.arms)?;
    assert!(
        fixture
            .source
            .cas
            .check_partition_cut(&checked, &changed, &mut ledger()?)
            .is_err()
    );
    let other = prepared(Example::Integer)?;
    let fault = other
        .source
        .cas
        .check_partition_cut(&checked, &original, &mut ledger()?)
        .err()
        .ok_or("foreign partition accepted")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::ForeignHandle));
    Ok(())
}

/// Undefined original operands, named origins and oblique affine cuts remain unsupported.
/// ´test:integration:decision-partition-total-predicates´
#[test]
fn decision_partition_total_predicates() -> TestResult {
    let mut fixture = prepared(Example::Phase)?;
    let a = fixture.question.coordinates[0].clone();
    let b = fixture.question.coordinates[1].clone();
    let oblique = fixture.source.term(Operator::Add, &[a.clone(), b])?;
    let zero = fixture.source.number(0, 1)?;
    let undefined = fixture.source.term(Operator::Div, &[a.clone(), zero])?;
    let named = fixture
        .source
        .guard(PredicateKind::Named("partition-provenance".into()), &[a])?;
    let positive = fixture.source.guard(PredicateKind::Positive, &[oblique])?;
    let invalid = fixture
        .source
        .guard(PredicateKind::Positive, &[undefined])?;
    let ambient = fixture.source.ambient.clone();
    for region in [named, positive, invalid] {
        let arm = association(&mut fixture.source, region, 0)?;
        fixture.question.function = case(&mut fixture.source, &ambient, &[arm])?;
        refuses(&fixture, InvalidCode::UnsupportedCertificate)?;
    }
    Ok(())
}

/// Rational shifts, negative slopes and canceled coordinates retain exact singleton ownership.
/// ´test:integration:decision-partition-affine-boundaries´
#[test]
fn decision_partition_affine_boundaries() -> TestResult {
    let mut fixture = prepared(Example::Phase)?;
    let a = fixture.question.coordinates[0].clone();
    let b = fixture.question.coordinates[1].clone();
    let canceled = fixture.source.term(Operator::Sub, &[b.clone(), b])?;
    let sum = fixture.source.term(Operator::Add, &[a, canceled])?;
    let offset = fixture.source.number(1, 2)?;
    let shifted = fixture.source.term(Operator::Sub, &[sum, offset])?;
    let factor = fixture.source.number(-3, 2)?;
    let affine = fixture.source.term(Operator::Div, &[shifted, factor])?;
    let regions = signs(&mut fixture.source, &affine)?;
    let mut arms = Vec::new();
    for region in regions {
        arms.push(association(&mut fixture.source, region, 1)?);
    }
    let ambient = fixture.source.ambient.clone();
    fixture.question.function = case(&mut fixture.source, &ambient, &arms)?;
    accepts(&fixture)?;
    fixture.question.function = case(
        &mut fixture.source,
        &ambient,
        &[arms[0].clone(), arms[2].clone()],
    )?;
    refuses(&fixture, InvalidCode::CoverageHole)
}

/// Native union and set difference preserve complete total-predicate partition semantics.
/// ´test:integration:decision-partition-boolean-regions´
#[test]
fn decision_partition_boolean_regions() -> TestResult {
    let mut fixture = prepared(Example::Sign)?;
    let x = fixture.question.coordinates[0].clone();
    let regions = signs(&mut fixture.source, &x)?;
    let nonzero = fixture.source.region(RegionExpr::Union(vec![
        Reference::Existing(regions[0].clone()),
        Reference::Existing(regions[2].clone()),
    ]))?;
    let ambient = fixture.source.ambient.clone();
    let complement = fixture.source.region(RegionExpr::Difference {
        base: Reference::Existing(ambient.clone()),
        excluded: Reference::Existing(nonzero.clone()),
    })?;
    let arms = [
        association(&mut fixture.source, nonzero, 1)?,
        association(&mut fixture.source, complement, 0)?,
    ];
    fixture.question.function = case(&mut fixture.source, &ambient, &arms)?;
    accepts(&fixture)
}

/// Missing, duplicate and foreign axes refuse; coordinate order remains an explicit contract.
/// ´test:integration:decision-partition-coordinate-identities´
#[test]
fn decision_partition_coordinate_identities() -> TestResult {
    let mut fixture = prepared(Example::Phase)?;
    let original = fixture.question.clone();
    fixture.question.coordinates.pop();
    refuses(&fixture, InvalidCode::DecisionBox)?;
    fixture.question.clone_from(&original);
    fixture
        .question
        .coordinates
        .push(original.coordinates[0].clone());
    refuses(&fixture, InvalidCode::DecisionBox)?;
    fixture.question.clone_from(&original);
    let other = prepared(Example::Phase)?;
    fixture.question.coordinates[0] = other.question.coordinates[0].clone();
    refuses(&fixture, InvalidCode::ForeignHandle)?;
    fixture.question.clone_from(&original);
    fixture.question.coordinates.reverse();
    accepts(&fixture)
}

/// Partition and cut checks introduce no source fact and cannot discharge the original divisor.
/// ´test:integration:decision-partition-no-membership-authority´
#[test]
fn decision_partition_no_membership_authority() -> TestResult {
    let mut fixture = prepared(Example::Sign)?;
    let before = fixture.source.cas.counts();
    let checked = fixture
        .source
        .cas
        .check_case_partition(&fixture.question, &mut ledger()?)?;
    fixture.source.cas.check_partition_cut(
        &checked,
        &cut(&fixture, Example::Sign)?,
        &mut ledger()?,
    )?;
    assert_eq!(fixture.source.cas.counts(), before);
    let x = fixture.question.coordinates[0].clone();
    let quotient = fixture.source.term(Operator::Div, &[x.clone(), x])?;
    let one = fixture.source.number(1, 1)?;
    let step = crate::testing::witness::equality(
        &fixture.source.context,
        &fixture.source.ambient,
        nrz_cas::proof::Primitive::Cancel,
        &quotient,
        &one,
    );
    let witness =
        fixture
            .source
            .cas
            .check_proof(&fixture.source.context, &[step], &mut ledger()?)?;
    assert!(!witness.is_discharged());
    assert_eq!(
        witness.hypotheses(),
        [] as [nrz_cas::domain::HypothesisRef; 0]
    );
    Ok(())
}

/// Literal measurements distinguish full admission from authenticated reuse plus a new cut.
const PARTITION_COSTS: [(Example, u64, u64, u64, u64, u64, u64); 3] = [
    (Example::Integer, 165_692, 77_288, 524, 13, 23_132, 58_076),
    (Example::Sign, 17_140, 52_332, 192, 3, 5_388, 47_920),
    (Example::Phase, 48_944, 67_112, 256, 9, 10_420, 55_248),
];

fn allowance(kind: BudgetKind, needed: u64) -> Limits {
    let mut limits = Limits::default();
    match kind {
        BudgetKind::Work => limits.work = needed,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = needed,
        BudgetKind::OutputBytes => limits.output_bytes = needed,
        BudgetKind::RegionCells => limits.region_cells = needed,
        _ => {}
    }
    limits
}

fn admission_boundary(
    fixture: &PartitionFixture,
    kind: BudgetKind,
    needed: u64,
    prior: u64,
) -> TestResult<bool> {
    let limits = allowance(kind, needed);
    let mut budget = Budget::new(limits.clone())?;
    budget.work(prior, Location::new("prior partition work"))?;
    let before = fixture.source.cas.counts();
    let result = fixture
        .source
        .cas
        .check_case_partition(&fixture.question, &mut budget);
    assert_eq!(fixture.source.cas.counts(), before);
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

/// Admission has exact Work, staging, output and Cartesian-cell N/N−1 boundaries.
/// ´test:integration:decision-partition-resource-boundaries´
#[test]
fn decision_partition_resource_boundaries() -> TestResult {
    for (example, work, staged, output, cells, _, _) in PARTITION_COSTS {
        let fixture = prepared(example)?;
        let mut budget = ledger()?;
        let checked = fixture
            .source
            .cas
            .check_case_partition(&fixture.question, &mut budget)?;
        assert_eq!(budget.usage().work, work);
        assert_eq!(budget.usage().region_cells, cells);
        assert_eq!(checked.retained_bytes(), output);
        for (kind, needed) in [
            (BudgetKind::Work, work),
            (BudgetKind::StagedPayloadBytes, staged),
            (BudgetKind::OutputBytes, output),
            (BudgetKind::RegionCells, cells),
        ] {
            assert!(admission_boundary(&fixture, kind, needed, 0)?);
            assert!(!admission_boundary(&fixture, kind, needed - 1, 0)?);
        }
        assert!(!admission_boundary(&fixture, BudgetKind::Work, work, 1)?);
        let limits = allowance(BudgetKind::StagedPayloadBytes, staged);
        let mut budget = Budget::new(limits.clone())?;
        let before = fixture.source.cas.counts();
        let result =
            budget.with_retained_payload(1, Location::new("retained partition input"), |budget| {
                fixture
                    .source
                    .cas
                    .check_case_partition(&fixture.question, budget)
            });
        assert!(matches!(result, Err(fault) if matches!(fault.kind,
            FaultKind::Exhausted { budget: BudgetKind::StagedPayloadBytes, .. })));
        assert_eq!(budget.limits(), &limits);
        assert_eq!(fixture.source.cas.counts(), before);
    }
    Ok(())
}

fn reuse_boundary(
    fixture: &PartitionFixture,
    example: Example,
    kind: BudgetKind,
    needed: u64,
) -> TestResult<bool> {
    let checked = fixture
        .source
        .cas
        .check_case_partition(&fixture.question, &mut ledger()?)?;
    let question = cut(fixture, example)?;
    let limits = allowance(kind, needed);
    let mut budget = Budget::new(limits.clone())?;
    let before = fixture.source.cas.counts();
    let result = fixture
        .source
        .cas
        .check_partition_cut(&checked, &question, &mut budget);
    assert_eq!(fixture.source.cas.counts(), before);
    assert_eq!(budget.limits(), &limits);
    match result {
        Ok(()) => Ok(true),
        Err(fault) => {
            assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind));
            Ok(false)
        }
    }
}

/// A reused partition charges only contract matching and the independently checked current cut.
/// ´test:integration:decision-partition-reuse-boundaries´
#[test]
fn decision_partition_reuse_boundaries() -> TestResult {
    for (example, _, _, _, _, work, staged) in PARTITION_COSTS {
        let fixture = prepared(example)?;
        for (kind, needed) in [
            (BudgetKind::Work, work),
            (BudgetKind::StagedPayloadBytes, staged),
        ] {
            assert!(reuse_boundary(&fixture, example, kind, needed)?);
            assert!(!reuse_boundary(&fixture, example, kind, needed - 1)?);
        }
        let checked = fixture
            .source
            .cas
            .check_case_partition(&fixture.question, &mut ledger()?)?;
        let question = cut(&fixture, example)?;
        let mut budget = Budget::new(Limits {
            output_bytes: 0,
            payload_bytes: 0,
            work: 2 * work,
            ..Limits::default()
        })?;
        let before = fixture.source.cas.counts();
        fixture
            .source
            .cas
            .check_partition_cut(&checked, &question, &mut budget)?;
        assert_eq!(budget.usage().work, work);
        fixture
            .source
            .cas
            .check_partition_cut(&checked, &question, &mut budget)?;
        assert_eq!(budget.usage().work, 2 * work);
        let usage = budget.usage();
        let result = fixture
            .source
            .cas
            .check_partition_cut(&checked, &question, &mut budget);
        assert!(matches!(result, Err(fault) if matches!(fault.kind,
            FaultKind::Exhausted { budget: BudgetKind::Work, .. })));
        assert_eq!(budget.usage(), usage);
        assert_eq!(fixture.source.cas.counts(), before);
    }
    Ok(())
}

/// Refused partitions and widened boxes preserve the checked revision and consume a shared ledger.
/// ´test:integration:decision-partition-atomic-lifecycle´
#[test]
fn decision_partition_atomic_lifecycle() -> TestResult {
    for (example, work, _, _, _, reuse, _) in PARTITION_COSTS {
        let fixture = prepared(example)?;
        let mut budget = Budget::new(Limits {
            payload_bytes: 0,
            work: work + reuse,
            ..Limits::default()
        })?;
        let before = fixture.source.cas.counts();
        let checked = fixture
            .source
            .cas
            .check_case_partition(&fixture.question, &mut budget)?;
        let question = cut(&fixture, example)?;
        fixture
            .source
            .cas
            .check_partition_cut(&checked, &question, &mut budget)?;
        assert_eq!(budget.usage().work, work + reuse);
        assert_eq!(fixture.source.cas.counts(), before);
        let mut changed = question.clone();
        changed.bounds[0].lower = rational(-100, 1)?;
        changed.bounds[0].upper = rational(100, 1)?;
        let mut failed = ledger()?;
        assert!(
            fixture
                .source
                .cas
                .check_partition_cut(&checked, &changed, &mut failed)
                .is_err()
        );
        assert!(failed.usage().work > 0);
        fixture
            .source
            .cas
            .check_partition_cut(&checked, &question, &mut failed)?;
        assert_eq!(fixture.source.cas.counts(), before);
        let mut short = Budget::new(Limits {
            work: work + reuse - 1,
            ..Limits::default()
        })?;
        let checked = fixture
            .source
            .cas
            .check_case_partition(&fixture.question, &mut short)?;
        assert!(
            fixture
                .source
                .cas
                .check_partition_cut(&checked, &question, &mut short)
                .is_err()
        );
        assert_eq!(fixture.source.cas.counts(), before);
    }
    Ok(())
}
