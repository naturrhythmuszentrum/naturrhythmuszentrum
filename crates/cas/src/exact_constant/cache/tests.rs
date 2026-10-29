// SPDX-License-Identifier: AGPL-3.0-only
//! Rollback generation is part of every retained local constant identity.

use super::*;
use crate::budget::Limits;
use crate::domain_model::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use crate::graph::{Draft, ExpressionDraft, Reference};
use crate::model::{AtomicSort, Constant, Environment, Field, Operator, Sort};
use crate::number::Rational;
use crate::testing::common::TestResult;

/// Ordinary finite ledger without any enlarged limit.
fn ledger() -> Result<Budget, Fault> {
    Budget::new(Limits::default())
}

/// Small original source frame with a named constant.
struct Fixture {
    /// Mutable source store under ordinary limits.
    cas: Cas,
    /// Original real semantic header.
    header: Handle,
    /// Exact hypothesis-free context.
    context: Handle,
    /// Named pi leaf.
    pi: Handle,
}

/// Construct a canonical graph frame independently of the cache.
fn fixture() -> TestResult<Fixture> {
    let drafts = [
        Draft::Header(HeaderDraft::Core(Environment::default())),
        Draft::Expression(ExpressionDraft {
            space: None,
            operator: Operator::Constant(Constant::Pi),
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            header: Reference::Local(0),
            scope: Vec::new(),
            binding: None,
            operands: Vec::new(),
        }),
        Draft::Region(RegionDraft {
            header: Reference::Local(0),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::Ambient,
        }),
        Draft::Context(ContextDraft::Root {
            header: Reference::Local(0),
            admissible: Reference::Local(2),
            assumptions: Vec::new(),
        }),
    ];
    let mut cas = Cas::new(Limits::default())?;
    let handles = cas.admit(&drafts, &mut ledger()?)?;
    let [header, pi, _region, context]: [Handle; 4] = handles.try_into().map_err(|_| "fixture")?;
    Ok(Fixture {
        cas,
        header,
        context,
        pi,
    })
}

/// Generate a fresh product whose logical identifiers can be reused after rollback.
fn product(
    cas: &mut Cas,
    header: &Handle,
    pi: &Handle,
    number: i64,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let header = cas.checked(header)?;
    let pi = cas.checked(pi)?;
    let sort = Sort::Value(AtomicSort::Scalar(Field::Real));
    let rational = Rational::from_ratio(number, 1, budget)?;
    let factor = cas.generated(
        header,
        Operator::Rational(rational),
        sort.clone(),
        &[],
        budget,
    )?;
    let root = cas.generated(header, Operator::Mul, sort, &[factor, pi], budget)?;
    Ok(cas.handle(root))
}

/// A rolled-back serial cannot return a judgment about a different original product.
/// ´test:unit:constant-cache-rollback-generation´
/// ´claim:cas:constant-cache-rollback-generation´
#[test]
fn constant_cache_rollback_generation() -> TestResult {
    let mut case = fixture()?;
    let mut cache = ExactConstantCache::new();
    let snapshot = case.cas.canonical_snapshot(&mut ledger()?)?;
    let mut stale = None;
    let refusal: Result<(), Fault> = case.cas.transaction(&mut ledger()?, |cas, budget| {
        let root = product(cas, &case.header, &case.pi, 2, budget)?;
        cache.prepare(cas, &case.context, &root, budget)?;
        stale = Some(root);
        budget.work(budget.limits().work, AT)
    });
    assert!(matches!(
        refusal,
        Err(Fault {
            kind: crate::FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            },
            ..
        })
    ));
    assert_eq!(case.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    let changed = product(&mut case.cas, &case.header, &case.pi, 3, &mut ledger()?)?;
    let stale = stale.ok_or("transient original")?;
    assert_eq!(stale.logical_id(), changed.logical_id());
    let mut direct = ledger()?;
    let expected = ExactConstant::from_expression(&case.cas, &changed, &mut direct)?;
    let mut measured = ledger()?;
    let prepared = cache.prepare(&case.cas, &case.context, &changed, &mut measured)?;
    assert_eq!(prepared.value(), &expected);
    assert_eq!(measured.usage().work, direct.usage().work);
    assert!(measured.usage().work > 1);
    assert_eq!(cache.len(), 1);
    Ok(())
}

/// Even a saturated generation never permits reuse after serial recycling.
/// ´test:unit:constant-cache-saturated-generation´
/// ´claim:cas:constant-cache-saturated-generation´
#[test]
fn constant_cache_saturated_generation() -> TestResult {
    let mut case = fixture()?;
    case.cas.memo_epoch = u64::MAX;
    let mut cache = ExactConstantCache::new();
    for _ in 0..2 {
        let mut budget = ledger()?;
        cache.prepare(&case.cas, &case.context, &case.pi, &mut budget)?;
        assert_eq!(budget.usage().work, 10_888);
        assert_eq!(cache.len(), 1);
    }
    Ok(())
}
