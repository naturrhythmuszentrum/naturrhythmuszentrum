// SPDX-License-Identifier: AGPL-3.0-only
//! Rollback generation is part of every retained local constant identity.

use super::*;
use crate::InvalidCode;
use crate::budget::{BudgetKind, Limits};
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
    let snapshot = case.cas.canonical_snapshot(&mut ledger()?)?;
    let mut stale = None;
    let refusal: Result<(), Fault> = case.cas.transaction(&mut ledger()?, |cas, budget| {
        let root = product(cas, &case.header, &case.pi, 2, budget)?;
        cas.prepare_constant(&case.context, &root, budget)?;
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
    let prepared = case
        .cas
        .prepare_constant(&case.context, &changed, &mut measured)?;
    assert_eq!(prepared.value(), &expected);
    assert_eq!(measured.usage().work, direct.usage().work);
    assert!(measured.usage().work > 1);
    assert_eq!(case.cas.retained_judgment_count(), 1);
    Ok(())
}

/// Even a saturated generation never permits reuse after serial recycling.
/// ´test:unit:constant-cache-saturated-generation´
/// ´claim:cas:constant-cache-saturated-generation´
#[test]
fn constant_cache_saturated_generation() -> TestResult {
    let mut case = fixture()?;
    case.cas.memo_epoch = u64::MAX;
    for _ in 0..2 {
        let mut budget = ledger()?;
        case.cas
            .prepare_constant(&case.context, &case.pi, &mut budget)?;
        assert_eq!(budget.usage().work, 10_888);
        assert_eq!(case.cas.retained_judgment_count(), 1);
    }
    Ok(())
}

/// A preparation is an indexed graph node reached by its original checked evidence edge.
/// ´test:unit:constant-graph-checked-evidence´
/// ´claim:cas:constant-graph-checked-evidence´
#[test]
fn constant_graph_checked_evidence() -> TestResult {
    let mut case = fixture()?;
    let before = case.cas.counts();
    let prepared = case
        .cas
        .prepare_constant(&case.context, &case.pi, &mut ledger()?)?;
    assert_eq!(prepared.formal_schema().name, "constant.closed-preparation");
    assert_eq!(prepared.formal_schema().version, 1);
    assert_eq!(
        prepared.premise_kinds(),
        &[
            crate::proof::HypothesisKind::DomainGuard,
            crate::proof::HypothesisKind::Equality
        ]
    );
    let node = case
        .cas
        .state
        .graph
        .node_weights()
        .find_map(|vertex| {
            if let crate::graph::Vertex::AnonymousLemma(node) = vertex {
                Some(node)
            } else {
                None
            }
        })
        .ok_or("retained anonymous lemma")?;
    assert_eq!(node.key.len(), 24);
    let after = case.cas.counts();
    assert_eq!(after.vertices, before.vertices + 1);
    assert_eq!(after.edges, before.edges + 3);
    assert_eq!(
        after.payload_bytes - before.payload_bytes,
        case.cas.retained_judgment_bytes()
    );
    let snapshot = case.cas.canonical_snapshot(&mut ledger()?)?;
    case.cas
        .prepare_constant(&case.context, &case.pi, &mut ledger()?)?;
    assert_eq!(case.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    Ok(())
}

/// Refused enclosing work removes its new graph evidence and keeps previous judgments intact.
/// ´test:unit:constant-graph-enclosing-refusal´
/// ´claim:cas:constant-graph-enclosing-refusal´
#[test]
fn constant_graph_enclosing_refusal() -> TestResult {
    let mut case = fixture()?;
    case.cas
        .prepare_constant(&case.context, &case.pi, &mut ledger()?)?;
    let snapshot = case.cas.canonical_snapshot(&mut ledger()?)?;
    let bytes = case.cas.retained_judgment_bytes();
    let mut budget = ledger()?;
    let mut spent = 0;
    let Err(fault) = case.cas.transaction(&mut budget, |cas, budget| {
        let original = product(cas, &case.header, &case.pi, 2, budget)?;
        cas.prepare_constant(&case.context, &original, budget)?;
        spent = budget.usage().work;
        Err::<(), _>(Fault::invalid(InvalidCode::Reference, AT))
    }) else {
        panic!("enclosing refusal must remove the new anonymous lemma and its edges");
    };
    assert_eq!(
        fault.kind,
        crate::FaultKind::Invalid(InvalidCode::Reference)
    );
    assert_eq!(budget.usage().work, spent);
    assert_eq!(case.cas.retained_judgment_count(), 1);
    assert_eq!(case.cas.retained_judgment_bytes(), bytes);
    assert_eq!(case.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    let mut refreshed = ledger()?;
    case.cas
        .prepare_constant(&case.context, &case.pi, &mut refreshed)?;
    assert_eq!(refreshed.usage().work, 10_888);
    assert_eq!(case.cas.retained_judgment_count(), 1);
    Ok(())
}

/// A separately owned copied store independently checks a retained original statement.
/// ´test:unit:constant-graph-owner-separation´
/// ´claim:cas:constant-graph-owner-separation´
#[test]
fn constant_graph_owner_separation() -> TestResult {
    let mut case = fixture()?;
    case.cas
        .prepare_constant(&case.context, &case.pi, &mut ledger()?)?;
    let mut other = case.cas.full_copy_for_test();
    let mut budget = ledger()?;
    other.prepare_constant(&case.context, &case.pi, &mut budget)?;
    assert_eq!(budget.usage().work, 10_888);
    assert_eq!(other.retained_judgment_count(), 1);
    Ok(())
}

/// Complete statement identity cannot replace a missing checked evidence edge.
/// ´test:unit:constant-graph-missing-evidence-refuses´
/// ´claim:cas:constant-graph-missing-evidence-refuses´
#[test]
fn constant_graph_missing_evidence_refuses() -> TestResult {
    use petgraph::visit::EdgeRef;
    let mut case = fixture()?;
    case.cas
        .prepare_constant(&case.context, &case.pi, &mut ledger()?)?;
    let edge = case
        .cas
        .state
        .graph
        .edge_references()
        .find(|edge| matches!(edge.weight(), crate::graph::Edge::CheckedEvidence))
        .map(|edge| edge.id())
        .ok_or("checked evidence edge")?;
    case.cas.state.graph.remove_edge(edge);
    let mut budget = ledger()?;
    let Err(fault) = case
        .cas
        .prepare_constant(&case.context, &case.pi, &mut budget)
    else {
        panic!("a canonical key without its checked evidence edge must refuse");
    };
    assert_eq!(
        fault.kind,
        crate::FaultKind::Invalid(InvalidCode::Reference)
    );
    assert_eq!(fault.location, AT);
    assert_eq!(budget.usage().work, 1);
    Ok(())
}
