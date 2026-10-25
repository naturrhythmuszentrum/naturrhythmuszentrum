// SPDX-License-Identifier: AGPL-3.0-only
//! Missing composition paths refuse instead of erasing original uncertainty or inventing source lineage.

use crate::reading_testing::{TestResult, ledger};
use crate::spectral_testing::Fixture;
use nrz_matrix::{Coverage, DependencyPolicy, Heap, HeapError, HeapLimits};
use nrz_scalar::ArithmeticContext;

/// Restored scalar projections carry the original snapshot and both complete radius channels.
/// ´test:integration:lc-energy-restored-state-selection´
#[test]
fn lc_energy_restored_state_selection() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let plan = super::state_fixture::plan(fixture.law.state().entries())?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let imported = super::state_fixture::import(&plan, &mut heap, DependencyPolicy::Entries)?;
    let context = ArithmeticContext::new()?;
    let normalized = heap.normalize_reading(
        &imported.bindings()[0],
        &plan.contract,
        &context,
        &mut ledger()?,
    )?;
    let restored = heap.restore_reading(&normalized, &context, &mut ledger()?)?;
    for (index, reading) in imported.bindings()[1..].iter().enumerate() {
        let component =
            heap.select_restored_reading(&restored, reading, &context, &mut ledger()?)?;
        assert!(component.symbols().bound().is_some());
        assert!(component.symbols().error().is_some());
        assert_eq!(
            component
                .symbol()
                .source()
                .and_then(nrz_bindings::QuantitySymbol::reading),
            Some(reading.reading())
        );
        let value = heap.value_by_symbol(component.symbol(), &component)?;
        assert_eq!(value.coverage(), Coverage::DeclaredBounds);
        assert_eq!(
            value.region().get(0, 0)?.to_bits(),
            restored.centers()[index].to_bits()
        );
        let total = heap.companion(value.bound().ok_or("restored total")?)?;
        let error = heap.companion(value.evaluation().ok_or("restored error")?)?;
        assert_eq!(
            total.region().get(0, 0)?.to_bits(),
            restored.totals()[index].to_bits()
        );
        assert_eq!(
            error.region().get(0, 0)?.to_bits(),
            restored.errors()[index].to_bits()
        );
        let origin = heap.coordinate_origin(&component)?;
        assert_eq!(
            origin.totals()[index].to_bits(),
            restored.totals()[index].to_bits()
        );
        assert_eq!(
            origin.errors()[index].to_bits(),
            restored.errors()[index].to_bits()
        );
        assert_eq!(origin.source().reading(), restored.source().reading());
    }
    Ok(())
}

/// Original selected L/C cannot acquire a native joint model through homogeneous schema lookup.
/// ´test:integration:lc-original-selected-sources-refuse-native-joint-attachment´
#[test]
fn lc_original_selected_sources_refuse_native_joint_attachment() -> TestResult {
    use nrz_matrix::{
        Binding, CompanionSetDraft, CovarianceBlock, ExecutionLimits, JointLimits, JointModel,
        Source, SourceFactor, TargetMap,
    };
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let operation = super::operator(&fixture)?;
    assert!(!operation.carries_statistics());
    let (mut heap, original) =
        fixture.import([[0.25_f64, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]])?;
    let selected = super::invariants::selections(&mut heap, &original)?;
    let sources = selected
        .iter()
        .map(|value| Source::new(value.symbol().descriptor(), 1))
        .collect::<Result<Vec<_>, _>>()?;
    let limits = JointLimits::default();
    let model = JointModel::from_factor(
        &sources,
        SourceFactor::new(2, &[0.007_812_5, 0.001_953_125], &[1.0], limits)?,
        &std::array::from_fn::<_, 4, _>(|_| CovarianceBlock::Known),
        limits,
    )?;
    let bound = Binding::from_symbols(operation, selected, ExecutionLimits::default())
        .validate(&mut heap)?;
    let handles = [
        bound.dependencies()[0].handle(),
        bound.dependencies()[1].handle(),
    ];
    drop(bound);
    let targets = [
        TargetMap::capture(&heap, handles[0])?,
        TargetMap::capture(&heap, handles[1])?,
    ];
    assert!(matches!(
        heap.publish_joint(
            CompanionSetDraft::new(model, targets, [vec![0], vec![1]]),
            limits
        ),
        Err(nrz_matrix::JointError::Heap(HeapError::MissingBinding))
    ));
    assert_eq!(heap.provenance_usage().records(), 0);
    for source in original {
        assert!(heap.reading_entries(&source).is_ok());
    }
    Ok(())
}
