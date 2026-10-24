// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed original LC declarations materialize once while grid consumers retain separate inputs.

use crate::reading_testing::{TestResult, ledger};
use crate::spectral_testing::Fixture;
use nrz_matrix::{
    Binding, CheckedOperation, ExecutionLimits, Heap, InvariantCache, ReadingBinding, ValueBinding,
};
use nrz_scalar::{ArithmeticContext, LiteralLimits, Scalar};

/// Two distinct RHS points read one original coefficient allocation and one complete guard key.
/// ´test:integration:lc-invariant-materializes-once-across-grid-consumers´
#[test]
fn lc_invariant_materializes_once_across_grid_consumers() -> TestResult {
    let fixture =
        Fixture::new(["5", "1/10", "1/2000"]).map_err(|e| format!("invariant fixture: {e:?}"))?;
    let operation = super::operator(&fixture).map_err(|e| format!("invariant operator: {e:?}"))?;
    let (mut heap, sources) = fixture
        .import([[0.25_f64, 0.0, 0.0], [0.0625, 0.0, 0.0]])
        .map_err(|e| format!("invariant import: {e:?}"))?;
    let selected = selections(&mut heap, &sources)?;
    let context = ArithmeticContext::new().map_err(|e| format!("invariant context: {e:?}"))?;
    let mut cache = InvariantCache::new(&heap, 4);
    let mut budget = ledger().map_err(|e| format!("invariant budget: {e:?}"))?;
    let first = cache
        .materialize(
            &mut heap,
            request(&operation, &selected),
            &context,
            &mut budget,
        )
        .map_err(|e| format!("first invariant: {e:?}"))?;
    let first_record = first.value().record();
    assert!(!first.reused());
    assert_eq!(
        first.value().definition().reuse_key().as_bytes(),
        fixture.law.operator().reuse_key().as_bytes()
    );
    consume(
        &mut heap,
        &operation,
        first.value(),
        &context,
        [1.0, 2.0],
        [-20, 6250],
    )?;
    let selected = selections(&mut heap, &sources)?;
    let second = cache
        .materialize(
            &mut heap,
            request(&operation, &selected),
            &context,
            &mut budget,
        )
        .map_err(|e| format!("second invariant: {e:?}"))?;
    assert!(second.reused());
    assert_eq!(second.value().record(), first_record);
    assert_eq!(cache.materializations(), 1);
    consume(
        &mut heap,
        &operation,
        second.value(),
        &context,
        [2.0, 3.0],
        [-30, 12_500],
    )?;
    assert_eq!(
        (first.work(), second.work(), budget.usage().work),
        (958, 5, 963)
    );
    assert_eq!(first.work() - second.work(), 953);
    assert_eq!(first.value().owned_bytes(), 848);
    assert_eq!(second.avoided_evaluation_work(), 956);
    Ok(())
}

/// Each later grid input remains a separate dependency of a separately certified solve.
fn consume(
    heap: &mut Heap,
    operation: &CheckedOperation,
    invariant: &nrz_matrix::InvariantValue<f64>,
    context: &ArithmeticContext,
    values: [f64; 2],
    expected: [i64; 2],
) -> TestResult {
    let solve = super::reuse::solve_from_operator(operation.outputs()[0].shape())
        .map_err(|e| format!("grid solve shape: {e:?}"))?;
    let coefficients = invariant
        .binding(heap, context)
        .map_err(|e| format!("grid coefficient binding: {e:?}"))?
        .clone();
    let rhs = heap
        .import_declared_bounds(
            nrz_matrix::Extent::new(2, 1)?,
            &nrz_bindings::Descriptor::ratio(),
            &values,
            &[0.0; 2],
        )
        .map_err(|e| format!("grid RHS import: {e:?}"))?;
    let rhs = heap
        .symbol_binding(rhs.value())
        .map_err(|e| format!("grid RHS binding: {e:?}"))?;
    let bound = Binding::from_symbols(solve, vec![coefficients, rhs], ExecutionLimits::default())
        .validate(heap)
        .map_err(|e| format!("grid solve binding: {e:?}"))?;
    let inputs = bound
        .normalized_inputs()
        .map_err(|e| format!("grid normalized inputs: {e:?}"))?;
    let candidate = bound
        .evaluate(inputs)
        .map_err(|e| format!("grid solve evaluation: {e:?}"))?;
    let outcome = bound
        .publish(candidate)
        .map_err(|e| format!("grid solve publication: {e:?}"))?;
    let output =
        crate::kernel_testing::output(&outcome).map_err(|e| format!("grid output: {e:?}"))?;
    for (index, target) in expected.into_iter().enumerate() {
        let (center, total, _) = crate::kernel_testing::entry(heap, output, index)
            .map_err(|e| format!("grid entry: {e:?}"))?;
        assert!(
            crate::reciprocal_testing::oracle::Rational::new(target, 1)?.covered(center, total)?
        );
    }
    Ok(())
}

/// Changing either source or any original radius version makes the cached reciprocal stale.
/// ´test:integration:lc-invariant-rechecks-every-source-and-companion-version´
#[test]
fn lc_invariant_rechecks_every_source_and_companion_version() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])
        .map_err(|e| format!("stale invariant fixture: {e:?}"))?;
    let operation =
        super::operator(&fixture).map_err(|e| format!("stale invariant operator: {e:?}"))?;
    for change in 0..6 {
        let (mut heap, sources) = fixture
            .import([
                [0.25_f64, 0.015_625, 0.000_976_562_5],
                [0.0625, 0.003_906_25, 0.000_244_140_625],
            ])
            .map_err(|e| format!("stale invariant import: {e:?}"))?;
        let selected = selections(&mut heap, &sources)?;
        let context =
            ArithmeticContext::new().map_err(|e| format!("stale invariant context: {e:?}"))?;
        let mut cache = InvariantCache::new(&heap, 4);
        let value = cache
            .materialize(
                &mut heap,
                request(&operation, &selected),
                &context,
                &mut ledger()?,
            )
            .map_err(|e| format!("stale invariant materialization: {e:?}"))?;
        mutate(&mut heap, &sources, change)?;
        assert!(value.value().binding(&heap, &context).is_err());
        assert!(
            cache
                .materialize(
                    &mut heap,
                    request(&operation, &selected),
                    &context,
                    &mut ledger()?
                )
                .is_err()
        );
        assert_eq!(cache.materializations(), 1);
    }
    Ok(())
}

/// Equal-bit replacements still advance the actual source or companion version.
fn mutate(heap: &mut Heap, sources: &[ReadingBinding<f64>; 2], change: usize) -> TestResult {
    if change < 2 {
        let values = heap
            .reading_entries(&sources[change])
            .map_err(|e| format!("invariant source values: {e:?}"))?;
        heap.replace_reading_entries(&sources[change], &values, &mut ledger()?)
            .map_err(|e| format!("invariant source replacement: {e:?}"))?;
    } else {
        let source = &sources[(change - 2) / 2];
        let kind = if change % 2 == 0 {
            nrz_bindings::CompanionKind::DeterministicBound
        } else {
            nrz_bindings::CompanionKind::NumericalError
        };
        let index = source
            .companions()
            .iter()
            .position(|companion| {
                companion.companion().kind()
                    == nrz_bindings::readings::CompanionReadingKind::Radius(kind)
                    && companion
                        .targets()
                        .iter()
                        .any(|target| target.reading() == source.reading())
            })
            .ok_or("invariant radius index")?;
        let values = heap
            .reading_companion_entries(source, index)
            .map_err(|e| format!("invariant parameter values: {e:?}"))?;
        heap.replace_reading_companion_entries(source, index, &values, &mut ledger()?)
            .map_err(|e| format!("invariant parameter replacement: {e:?}"))?;
    }
    Ok(())
}

/// Precision and bounded literal-import policy are distinct keys even for equal exact definitions.
/// ´test:integration:lc-invariant-separates-precision-and-import-model´
#[test]
fn lc_invariant_separates_precision_and_import_model() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])
        .map_err(|e| format!("model invariant fixture: {e:?}"))?;
    let operation =
        super::operator(&fixture).map_err(|e| format!("model invariant operator: {e:?}"))?;
    let (mut heap, sources) = fixture
        .import([[0.25_f64, 0.0, 0.0], [0.0625, 0.0, 0.0]])
        .map_err(|e| format!("model invariant import: {e:?}"))?;
    let selected = selections(&mut heap, &sources)?;
    let context =
        ArithmeticContext::new().map_err(|e| format!("model invariant context: {e:?}"))?;
    let mut cache = InvariantCache::new(&heap, 4);
    let first = cache
        .materialize(
            &mut heap,
            request(&operation, &selected),
            &context,
            &mut ledger()?,
        )
        .map_err(|e| format!("model initial materialization: {e:?}"))?;
    let policy = alternate(&fixture, &operation)?;
    let second = cache
        .materialize(
            &mut heap,
            request(&policy, &selected),
            &context,
            &mut ledger()?,
        )
        .map_err(|e| format!("model changed materialization: {e:?}"))?;
    assert!(!second.reused());
    assert_ne!(first.value().record(), second.value().record());
    let narrow = fixture
        .import_into(
            &mut heap,
            [[0.25_f32, 0.0, 0.0], [0.0625, 0.0, 0.0]],
            &mut ledger()?,
        )
        .map_err(|e| format!("narrow invariant import: {e:?}"))?;
    let narrow = selections(&mut heap, &narrow)?;
    let third = cache
        .materialize(
            &mut heap,
            request(&operation, &narrow),
            &context,
            &mut ledger()?,
        )
        .map_err(|e| format!("narrow invariant materialization: {e:?}"))?;
    assert!(!third.reused());
    assert_eq!(third.value().precision(), nrz_scalar::Precision::F32);
    assert_eq!(first.value().precision(), nrz_scalar::Precision::F64);
    assert_eq!(cache.materializations(), 3);
    Ok(())
}

/// The same exact operation is lowered under another explicit import policy without changing a default.
fn alternate(fixture: &Fixture, operation: &CheckedOperation) -> TestResult<CheckedOperation> {
    let inputs = operation
        .inputs()
        .iter()
        .map(|input| input.shape().clone())
        .collect();
    let output = operation.outputs()[0].shape().clone();
    let policy = LiteralLimits::new(512, 4096, 100_000);
    nrz_matrix::OperationDraft::positive_reciprocal_map(
        &fixture.graph,
        fixture.law.operator().clone(),
        inputs,
        output,
        policy,
        &mut ledger()?,
    )
    .map_err(|e| format!("alternate import model: {e:?}"))?
    .check(ExecutionLimits::default())
    .map_err(|e| format!("alternate model check: {e:?}").into())
}

/// An earlier positive value cannot supply the guard for a fresh zero-containing source box.
/// ´test:integration:lc-invariant-preserves-positive-guards-and-budget-boundaries´
#[test]
fn lc_invariant_preserves_positive_guards_and_budget_boundaries() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])
        .map_err(|e| format!("guard invariant fixture: {e:?}"))?;
    let operation =
        super::operator(&fixture).map_err(|e| format!("guard invariant operator: {e:?}"))?;
    let (mut heap, sources) = fixture
        .import([[0.25_f64, 0.0, 0.0], [0.0625, 0.0, 0.0]])
        .map_err(|e| format!("guard invariant import: {e:?}"))?;
    let selected = selections(&mut heap, &sources)?;
    let context =
        ArithmeticContext::new().map_err(|e| format!("guard invariant context: {e:?}"))?;
    let mut cache = InvariantCache::new(&heap, 1);
    for (limit, prior) in [(957, 0), (958, 1)] {
        assert!(
            cache
                .materialize(
                    &mut heap,
                    request(&operation, &selected),
                    &context,
                    &mut super::costs::policy(0, limit, prior)?
                )
                .is_err()
        );
        assert_eq!(cache.materializations(), 0);
    }
    cache
        .materialize(
            &mut heap,
            request(&operation, &selected),
            &context,
            &mut super::costs::policy(0, 958, 0)?,
        )
        .map_err(|e| format!("guard first materialization: {e:?}"))?;
    for (limit, prior, success) in [(5, 0, true), (4, 0, false), (5, 1, false)] {
        assert_eq!(
            cache
                .materialize(
                    &mut heap,
                    request(&operation, &selected),
                    &context,
                    &mut super::costs::policy(0, limit, prior)?
                )
                .is_ok(),
            success
        );
    }
    let invalid = fixture
        .import_into(
            &mut heap,
            [[0.25_f64, 0.0, 0.0], [0.0, 0.0, 0.0]],
            &mut ledger()?,
        )
        .map_err(|e| format!("invalid guard import: {e:?}"))?;
    let invalid = selections(&mut heap, &invalid)?;
    let mut separate = InvariantCache::new(&heap, 1);
    let failure = match separate.materialize(
        &mut heap,
        request(&operation, &invalid),
        &context,
        &mut ledger()?,
    ) {
        Err(nrz_matrix::InvariantError::Failure(failure)) => failure,
        Err(cause) => return Err(format!("guard lost execution evidence: {cause:?}").into()),
        Ok(_) => return Err("nonpositive invariant materialized".into()),
    };
    assert!(matches!(
        failure.cause(),
        nrz_matrix::OperationError::Kernel(_)
    ));
    assert_eq!(
        failure
            .record()
            .operation()
            .reciprocal_map()
            .ok_or("failed exact map")?
            .reuse_key()
            .as_bytes(),
        fixture.law.operator().reuse_key().as_bytes(),
    );
    assert_eq!(failure.record().historical_evidence().len(), 0);
    assert_eq!(separate.materializations(), 0);
    Ok(())
}

/// Cold materialization preflights its retained allocation and temporary dependency capture exactly.
/// ´test:integration:lc-invariant-payload-boundaries´
#[test]
fn lc_invariant_payload_boundaries() -> TestResult {
    let fixture =
        Fixture::new(["5", "1/10", "1/2000"]).map_err(|e| format!("payload fixture: {e:?}"))?;
    let operation = super::operator(&fixture).map_err(|e| format!("payload operator: {e:?}"))?;
    let (mut heap, sources) = fixture
        .import([[0.25_f64, 0.0, 0.0], [0.0625, 0.0, 0.0]])
        .map_err(|e| format!("payload import: {e:?}"))?;
    let selected = selections(&mut heap, &sources)?;
    let context = ArithmeticContext::new().map_err(|e| format!("payload context: {e:?}"))?;
    for (kind, needed) in [(1, 2160), (2, 848)] {
        for (limit, success) in [(needed, true), (needed - 1, false)] {
            let mut cache = InvariantCache::new(&heap, 1);
            assert_eq!(
                cache
                    .materialize(
                        &mut heap,
                        request(&operation, &selected),
                        &context,
                        &mut super::costs::policy(kind, limit, 0)?
                    )
                    .is_ok(),
                success
            );
            assert_eq!(cache.materializations(), usize::from(success));
        }
    }
    Ok(())
}

/// Foreign owners, a full cache and mutation or retirement of the cached result cannot grant reuse.
/// ´test:integration:lc-invariant-rejects-unavailable-materializations´
#[test]
fn lc_invariant_rejects_unavailable_materializations() -> TestResult {
    let fixture =
        Fixture::new(["5", "1/10", "1/2000"]).map_err(|e| format!("unavailable fixture: {e:?}"))?;
    let operation =
        super::operator(&fixture).map_err(|e| format!("unavailable operator: {e:?}"))?;
    for retire in [false, true] {
        let (mut heap, sources) = fixture
            .import([[0.25_f64, 0.0, 0.0], [0.0625, 0.0, 0.0]])
            .map_err(|e| format!("unavailable import: {e:?}"))?;
        let selected = selections(&mut heap, &sources)?;
        let context =
            ArithmeticContext::new().map_err(|e| format!("unavailable context: {e:?}"))?;
        let mut cache = InvariantCache::new(&heap, 1);
        let original = cache
            .materialize(
                &mut heap,
                request(&operation, &selected),
                &context,
                &mut ledger()?,
            )
            .map_err(|e| format!("unavailable materialization: {e:?}"))?;
        let bundle = original
            .value()
            .bundle(&heap, &context)
            .map_err(|e| format!("original bundle: {e:?}"))?;
        invalidate(&mut heap, bundle, retire)?;
        assert!(original.value().binding(&heap, &context).is_err());
        assert!(
            cache
                .materialize(
                    &mut heap,
                    request(&operation, &selected),
                    &context,
                    &mut ledger()?
                )
                .is_err()
        );
        assert_eq!(cache.materializations(), 1);
        foreign_owner(&fixture, &operation, &context, &mut cache)?;
    }
    Ok(())
}

/// Result mutation and retirement must invalidate the original publication before lookup can grant reuse.
fn invalidate(
    heap: &mut Heap,
    bundle: nrz_matrix::PublishedBundle<f64>,
    retire: bool,
) -> TestResult {
    if retire {
        heap.retire(bundle.value())
            .map_err(|e| format!("invariant retirement: {e:?}"))?;
    } else {
        let values = [0.0, -0.1, 0.000_16, 0.0];
        let version = heap
            .value(bundle.value())
            .map_err(|e| format!("cached version: {e:?}"))?
            .stamp()
            .version();
        heap.replace_represented(bundle.value(), version, &values)
            .map_err(|e| format!("invariant mutation: {e:?}"))?;
    }
    Ok(())
}

/// Numerical identity is owner-scoped even when the physical declarations and all float bits agree.
fn foreign_owner(
    fixture: &Fixture,
    operation: &CheckedOperation,
    context: &ArithmeticContext,
    cache: &mut InvariantCache,
) -> TestResult {
    let (mut heap, sources) = fixture
        .import([[0.25_f64, 0.0, 0.0], [0.0625, 0.0, 0.0]])
        .map_err(|e| format!("foreign invariant import: {e:?}"))?;
    let selected = selections(&mut heap, &sources)?;
    assert!(
        cache
            .materialize(
                &mut heap,
                request(operation, &selected),
                context,
                &mut ledger()?
            )
            .is_err()
    );
    let mut empty = InvariantCache::new(&heap, 0);
    assert!(
        empty
            .materialize(
                &mut heap,
                request(operation, &selected),
                context,
                &mut ledger()?
            )
            .is_err()
    );
    assert_eq!(empty.materializations(), 0);
    Ok(())
}

/// Original value and companion selections remain the fixed declarations of every request.
fn selections<P: Scalar + 'static>(
    heap: &mut Heap,
    sources: &[ReadingBinding<P>; 2],
) -> TestResult<Vec<ValueBinding<P>>> {
    let mut values = Vec::new();
    for source in sources {
        values.push(
            heap.select_reading(source, &mut ledger()?)
                .map_err(|e| format!("invariant source selection: {e:?}"))?,
        );
    }
    Ok(values)
}

/// Grid inputs are deliberately absent: this coefficient depends only on its fixed source versions.
fn request<P: Scalar + 'static>(
    operation: &CheckedOperation,
    sources: &[ValueBinding<P>],
) -> Binding<P> {
    Binding::from_symbols(
        operation.clone(),
        sources.to_vec(),
        ExecutionLimits::default(),
    )
}
