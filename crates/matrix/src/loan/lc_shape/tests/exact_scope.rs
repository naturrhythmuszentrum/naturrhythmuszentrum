// SPDX-License-Identifier: AGPL-3.0-only
//! Original interfaces and Rust record sizes supply expectations, never report calculations.

use super::fixture::{self, TestResult};
use crate::loan::lc_shape::exact_scope::{ExactScope, Exclusion, Measure};
use crate::{ExecutionAccount, Heap, HeapLimits, LcInputInventory, LcInputShape, Residence};
use nrz_bindings::{Budget, ExponentPolicy, LcForwardCover, LcRelation, Limits, Registry};
use nrz_scalar::Scalar;

/// Original identities survive numerical drain without becoming physical-plan coverage.
/// ´test:unit:lc-omega-graph-exact-scope-report´
#[test]
fn lc_omega_graph_exact_scope_report() -> TestResult {
    check::<f32>()?;
    check::<f64>()
}

/// Equal original values can belong to different allocations; cloned wrappers share payload.
fn check<P: Scalar + 'static>() -> TestResult {
    let mut budget = Budget::new(Limits::default())?;
    let question = fixture::question(&mut budget)?;
    let shared_question = question.clone();
    let independent = fixture::question(&mut budget)?;
    let shared = question.shares_allocation(&shared_question);
    let distinct = !question.shares_allocation(&independent);
    assert!(shared);
    assert!(distinct);
    assert_eq!(question.cycles(), independent.cycles());
    assert_eq!(
        question.retained_payload_bytes(),
        independent.retained_payload_bytes()
    );
    let mut account = ExecutionAccount::new(budget)?;
    let inventory = LcInputInventory::<P, 4>::declare(&question, [2, 3, 2], &mut account)?;
    let shared_inventory =
        LcInputInventory::<P, 4>::declare(&shared_question, [2, 3, 2], &mut account)?;
    let independent_inventory =
        LcInputInventory::<P, 4>::declare(&independent, [2, 3, 2], &mut account)?;
    let registry = Registry::new(ExponentPolicy::Rational);
    let other_registry = Registry::new(ExponentPolicy::Rational);
    let report = ExactScope::new(&inventory, &registry);
    let same = ExactScope::new(&inventory, &registry);
    let related = ExactScope::new(&shared_inventory, &registry);
    let unrelated = ExactScope::new(&independent_inventory, &other_registry);
    let before = account.usage();
    measures(&report, &inventory, &registry, &question)?;
    identities(&report, &same, &related, &unrelated)?;
    assert_eq!(account.usage(), before);
    // The schemas, not these caller wrappers, retain the authority observed below.
    drop(question);
    drop(shared_question);
    drop(independent);
    let mut heap = Heap::new(HeapLimits::default())?;
    let axes = fixture::axes(&heap, &mut account)?;
    let schema = inventory.input(0).ok_or("missing original L")?;
    let tile = super::source(schema, &axes, &mut heap, &mut account, 1)?;
    let physical_before = heap.resource_bytes()?;
    let elements_before = heap.loans.elements()?;
    let first = report.input(0).ok_or("missing report L")?;
    let original_allowances = (first.question.measure, first.relation.measure);
    let before = account.usage();
    measures(&report, &inventory, &registry, schema.question())?;
    assert_eq!(account.usage(), before);
    assert_eq!(heap.resource_bytes()?, physical_before);
    assert_eq!(heap.loans.elements()?, elements_before);
    let occupied = matches!(
        heap.residence(&mut account, 100_000)?,
        Residence::Pending { .. }
    );
    assert!(occupied);
    drop(tile);
    fixture::empty(&mut heap, &mut account)?;
    let after = report.input(0).ok_or("original vanished after drain")?;
    let same_question = first.question.same_owner(&after.question);
    let same_relation = first.relation.same_owner(&after.relation);
    assert!(same_question);
    assert!(same_relation);
    assert_eq!(
        (after.question.measure, after.relation.measure),
        original_allowances
    );
    let renewed = ExactScope::new(&inventory, &registry);
    let same_registry = renewed.registry().same_owner(&report.registry());
    assert!(same_registry);
    measures(&renewed, &inventory, &registry, schema.question())?;
    assert_eq!(heap.loans.elements()?, [0, 0]);
    Ok(())
}

/// Fixed Rust record sizes overlap; neither they nor logical allowances are backing capacity.
fn measures<P: Scalar>(
    report: &ExactScope<'_, P, 4>,
    inventory: &LcInputInventory<P, 4>,
    registry: &Registry,
    question: &LcForwardCover,
) -> TestResult {
    assert_eq!(
        ExactScope::<P, 4>::exclusions(),
        [
            Exclusion::SymbolicStores,
            Exclusion::Provenance,
            Exclusion::SchemaAndSymbolPools,
            Exclusion::LogicalBindings,
            Exclusion::RegistryMeasures,
            Exclusion::RegistryNames,
            Exclusion::RegistryRules,
            Exclusion::RegistryScaleRoots,
        ]
    );
    assert_eq!(ExactScope::<P, 4>::input_count(), 4);
    assert_eq!(report.input(4).map(|input| input.position), None);
    assert_eq!(report.input(usize::MAX).map(|input| input.position), None);
    assert_eq!(
        report.inventory().measure,
        Measure {
            logical_allowance_bytes: None,
            inline_record_size: size_of_val(inventory),
            backing_capacity_bytes: None,
        }
    );
    assert_eq!(
        report.registry().measure,
        Measure {
            logical_allowance_bytes: None,
            inline_record_size: size_of_val(registry),
            backing_capacity_bytes: None,
        }
    );
    for position in 0..4 {
        let input = report.input(position).ok_or("missing positional use")?;
        assert_eq!(input.position, position);
        // Both handle sizes are eight bytes on the supported target; byte values cannot
        // distinguish swapped handles. Check the distinct owning kinds as well as their
        // independent logical allowances below, without inventing unequal byte sizes.
        let different_handle_kinds = !input.question.same_owner(&input.relation);
        assert!(different_handle_kinds);
        assert_eq!(
            input.schema.measure,
            Measure {
                logical_allowance_bytes: None,
                inline_record_size: size_of::<LcInputShape<P>>(),
                backing_capacity_bytes: None,
            }
        );
        assert_eq!(
            input.question.measure,
            Measure {
                logical_allowance_bytes: Some(question.retained_payload_bytes()),
                inline_record_size: size_of::<LcForwardCover>(),
                backing_capacity_bytes: None,
            }
        );
        assert_eq!(
            input.relation.measure,
            Measure {
                logical_allowance_bytes: Some(question.relation().retained_payload_bytes()),
                inline_record_size: size_of::<LcRelation>(),
                backing_capacity_bytes: None,
            }
        );
    }
    Ok(())
}

/// Positional schema uses differ even where exact authority is shared; Registry stays borrowed.
fn identities<P: Scalar>(
    report: &ExactScope<'_, P, 4>,
    same: &ExactScope<'_, P, 4>,
    related: &ExactScope<'_, P, 4>,
    unrelated: &ExactScope<'_, P, 4>,
) -> TestResult {
    let same_inventory = report.inventory().same_owner(&same.inventory());
    let different_inventory = !report.inventory().same_owner(&related.inventory());
    let shared_registry = report.registry().same_owner(&related.registry());
    let distinct_registry = !report.registry().same_owner(&unrelated.registry());
    let different_kinds = !report.inventory().same_owner(&report.registry());
    assert!(same_inventory);
    assert!(different_inventory);
    assert!(shared_registry);
    assert!(distinct_registry);
    assert!(different_kinds);
    let first = report.input(0).ok_or("missing original")?;
    let clone = related.input(0).ok_or("missing shared original")?;
    let separate = unrelated.input(0).ok_or("missing independent original")?;
    let distinct_schema = !first.schema.same_owner(&clone.schema);
    let shared_question = first.question.same_owner(&clone.question);
    let shared_relation = first.relation.same_owner(&clone.relation);
    let distinct_question = !first.question.same_owner(&separate.question);
    let distinct_relation = !first.relation.same_owner(&separate.relation);
    assert!(distinct_schema);
    assert!(shared_question);
    assert!(shared_relation);
    assert!(distinct_question);
    assert!(distinct_relation);
    for position in 0..4 {
        let input = report
            .input(position)
            .ok_or("missing shared positional use")?;
        let shared_question = first.question.same_owner(&input.question);
        let shared_relation = first.relation.same_owner(&input.relation);
        assert!(shared_question);
        assert!(shared_relation);
        assert_eq!(first.schema.same_owner(&input.schema), position == 0);
    }
    Ok(())
}
