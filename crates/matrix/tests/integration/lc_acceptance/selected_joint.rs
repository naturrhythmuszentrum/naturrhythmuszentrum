// SPDX-License-Identifier: AGPL-3.0-only
//! Native joint declarations retain selected physical quantities and their original dependencies.

use crate::reading_testing::{TestResult, ledger};
use crate::spectral_testing::Fixture;
use nrz_matrix::{
    Binding, CompanionSetDraft, CovarianceBlock, Coverage, ExecutionLimits, Handle, Heap,
    HeapError, JointError, JointLimits, JointModel, ReadingBinding, Source, SourceFactor,
    TargetMap, Value, ValueBinding,
};
use nrz_scalar::{ArithmeticContext, Enclosure, Scalar};

/// Original selected L/C retain their physical identities in a declared common-source model.
/// ´test:integration:lc-original-selected-sources-accept-native-joint-attachment´
#[test]
fn lc_original_selected_sources_accept_native_joint_attachment() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    attachment(
        &fixture,
        [[0.25_f32, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]],
        [0.007_812_5, 0.001_953_125],
    )?;
    attachment(
        &fixture,
        [[0.25_f64, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]],
        [0.007_812_5, 0.001_953_125],
    )
}

/// Both formats keep the same selected meanings, model identities and physical coverage.
fn attachment<P: Scalar + 'static>(
    fixture: &Fixture,
    values: [[P; 3]; 2],
    coefficients: [P; 2],
) -> TestResult {
    let operation = super::operator(fixture)?;
    assert!(!operation.carries_statistics());
    let (mut heap, original) = fixture.import(values)?;
    let selected = super::invariants::selections(&mut heap, &original)?;
    let handles = capture_handles(&mut heap, &selected)?;
    let targets = capture_targets(&heap, handles)?;
    let stamps = targets.each_ref().map(TargetMap::stamp);
    let model = joint_model(&selected, &coefficients)?;
    let identity = model.id();
    let source_ids = model.sources().iter().map(Source::id).collect::<Vec<_>>();
    let companion = heap.publish_joint(
        CompanionSetDraft::new(model, targets, [vec![0], vec![1]]),
        JointLimits::default(),
    )?;
    for (index, binding) in selected.iter().enumerate() {
        assert_eq!(binding.symbol().reading(), Some(original[index].reading()));
        assert_eq!(
            heap.symbol_binding(handles[index])?.symbol(),
            binding.symbol()
        );
        assert_eq!(
            heap.value(handles[index])?.coverage(),
            Coverage::DeclaredBounds
        );
    }
    drop(original);
    heap.compact()?;
    let view = heap.statistical(companion)?;
    assert_eq!(view.targets(), stamps);
    assert_eq!(view.source_maps(), &[vec![0], vec![1]]);
    assert_eq!(view.model().id(), identity);
    assert_eq!(
        view.model()
            .sources()
            .iter()
            .map(Source::id)
            .collect::<Vec<_>>(),
        source_ids
    );
    assert!(view.means().is_none());
    covariance_oracle(view.model())?;
    assert!(matches!(
        Binding::from_symbols(operation, selected, ExecutionLimits::default()).validate(&mut heap),
        Err(nrz_matrix::OperationError::Heap(HeapError::MissingCoverage))
    ));
    assert_eq!(heap.provenance_usage().records(), 0);
    Ok(())
}

/// Companion-only writes invalidate attached evidence even when the base value stamp is unchanged.
/// ´test:integration:lc-selected-joint-rechecks-original-dependencies´
#[test]
fn lc_selected_joint_rechecks_original_dependencies() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    for change in 0..6 {
        for publish_first in [false, true] {
            let (mut heap, original, selected) = inputs(&fixture)?;
            let handles = capture_handles(&mut heap, &selected)?;
            let targets = capture_targets(&heap, handles)?;
            let draft = CompanionSetDraft::new(
                joint_model(&selected, &[0.007_812_5, 0.001_953_125])?,
                targets.clone(),
                [vec![0], vec![1]],
            );
            let companion = if publish_first {
                Some(heap.publish_joint(draft.clone(), JointLimits::default())?)
            } else {
                None
            };
            super::invariants::mutate(&mut heap, &original, change)?;
            let affected = if change < 2 { change } else { (change - 2) / 2 };
            assert!(
                targets[affected]
                    .validate(&heap, handles[affected])
                    .is_err()
            );
            assert!(TargetMap::capture(&heap, handles[affected]).is_err());
            if change >= 2 {
                let disjoint = 1 - affected;
                targets[disjoint].validate(&heap, handles[disjoint])?;
                if !publish_first {
                    assert!(heap.reading_entries(&original[disjoint]).is_ok());
                }
            }
            if let Some(companion) = companion {
                assert!(heap.statistical(companion).is_err());
            } else {
                assert!(heap.publish_joint(draft, JointLimits::default()).is_err());
            }
            assert_eq!(heap.provenance_usage().records(), 0);
        }
    }
    Ok(())
}

/// Wrong source quantities, lengths, indices and foreign owners leave no published relation.
/// ´test:integration:lc-selected-joint-refusals-are-atomic´
#[test]
fn lc_selected_joint_refusals_are_atomic() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let (mut heap, original, selected) = inputs(&fixture)?;
    let handles = capture_handles(&mut heap, &selected)?;
    let targets = capture_targets(&heap, handles)?;
    let model = joint_model(&selected, &[0.007_812_5, 0.001_953_125])?;
    for selections in [[vec![1], vec![0]], [vec![], vec![1]], [vec![0], vec![2]]] {
        assert!(matches!(
            heap.publish_joint(
                CompanionSetDraft::new(model.clone(), targets.clone(), selections),
                JointLimits::default(),
            ),
            Err(JointError::WrongTarget)
        ));
        assert_eq!(heap.joint_companions(handles[0])?.len(), 0);
        for source in &original {
            assert!(heap.reading_entries(source).is_ok());
        }
    }
    assert!(matches!(
        targets[0].validate(&heap, handles[1]),
        Err(JointError::WrongTarget)
    ));
    let (mut foreign, _, _) = inputs(&fixture)?;
    let draft = CompanionSetDraft::new(model, targets, [vec![0], vec![1]]);
    assert!(matches!(
        foreign.publish_joint(draft.clone(), JointLimits::default()),
        Err(JointError::Heap(HeapError::ForeignHandle))
    ));
    assert!(matches!(
        heap.publish_joint(draft.clone(), JointLimits::new(2, 4, 4)),
        Err(JointError::ResourceLimit)
    ));
    assert_eq!(heap.joint_companions(handles[0])?.len(), 0);
    let companion = heap.publish_joint(draft, JointLimits::new(2, 4, 5))?;
    assert_eq!(
        heap.statistical(companion)?.source_maps(),
        &[vec![0], vec![1]]
    );
    Ok(())
}

/// Mean parameters use the selected symbol's local coordinates rather than its base storage offset.
/// ´test:integration:lc-selected-joint-means-use-local-coordinates´
#[test]
fn lc_selected_joint_means_use_local_coordinates() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let plan = super::state_fixture::plan(fixture.law.state().entries())?;
    let mut heap = Heap::new(nrz_matrix::HeapLimits::default())?;
    let original =
        super::state_fixture::import(&plan, &mut heap, nrz_matrix::DependencyPolicy::Entries)?;
    let selected = heap.select_reading(&original.bindings()[2], &mut ledger()?)?;
    let handles = capture_handles(&mut heap, std::slice::from_ref(&selected))?;
    let target = TargetMap::capture(&heap, handles[0])?;
    assert_eq!(target.layout().index(0, 0)?, 1);
    let limits = JointLimits::default();
    let source = Source::new(selected.symbol().descriptor(), 1)?;
    let model = JointModel::from_factor(
        &[source],
        SourceFactor::new(1, &[0.031_25], &[1.0], limits)?,
        &[CovarianceBlock::Known],
        limits,
    )?;
    let mean = vec![Enclosure::new(0.218_75, 0.234_375)?];
    let draft = CompanionSetDraft::new(model, [target.clone(), target], [vec![0], vec![0]])
        .with_means(
            &heap,
            [mean.clone(), mean],
            &ArithmeticContext::new()?,
            limits,
        )?;
    let companion = heap.publish_joint(draft, limits)?;
    let view = heap.statistical(companion)?;
    let means = view.means().ok_or("declared means")?;
    assert_eq!(
        means.descriptors()[0].dimension(),
        selected.symbol().descriptor().dimension()
    );
    assert!(view.mean_error_regions()[0].get(0, 0)? >= 0.031_25);
    assert_eq!(heap.symbol_binding(handles[0])?.symbol(), selected.symbol());
    Ok(())
}

/// Reuse checked exact declarations while each trial owns fresh numerical occupations.
type Inputs = (Heap, [ReadingBinding<f64>; 2], Vec<ValueBinding<f64>>);

fn inputs(fixture: &Fixture) -> TestResult<Inputs> {
    let (mut heap, original) = fixture.import([
        [0.25_f64, 0.015_625, 0.000_976_562_5],
        [0.0625, 0.003_906_25, 0.000_244_140_625],
    ])?;
    let selected = super::invariants::selections(&mut heap, &original)?;
    Ok((heap, original, selected))
}

/// A deterministic validated copy exposes public dependency handles without recording an operation.
pub(super) fn capture_handles<P: Scalar + 'static>(
    heap: &mut Heap,
    selected: &[ValueBinding<P>],
) -> TestResult<[Handle<Value, P>; 2]> {
    let owner = crate::conversion_testing::owner()?;
    let shapes = selected
        .iter()
        .map(|value| crate::conversion_testing::shape(&owner, value.symbol().descriptor(), 1))
        .collect::<TestResult<Vec<_>>>()?;
    let operation = nrz_matrix::OperationDraft::copy(shapes).check(ExecutionLimits::default())?;
    let bound = Binding::from_symbols(operation, selected.to_vec(), ExecutionLimits::default())
        .validate(heap)?;
    let first = bound.dependencies()[0].handle();
    let second = bound
        .dependencies()
        .get(1)
        .map_or(first, nrz_matrix::InputSnapshot::handle);
    Ok([first, second])
}

/// Each target keeps its exact alias key and ordered base coordinates.
pub(super) fn capture_targets<P: Scalar + 'static>(
    heap: &Heap,
    handles: [Handle<Value, P>; 2],
) -> TestResult<[TargetMap<P>; 2]> {
    Ok([
        TargetMap::capture(heap, handles[0])?,
        TargetMap::capture(heap, handles[1])?,
    ])
}

/// One rank-one factor declares the shared fluctuation without an independence inference.
pub(super) fn joint_model<P: Scalar + 'static>(
    selected: &[ValueBinding<P>],
    coefficients: &[P],
) -> TestResult<JointModel<P>> {
    let sources = selected
        .iter()
        .map(|value| Source::new(value.symbol().descriptor(), 1))
        .collect::<Result<Vec<_>, _>>()?;
    let limits = JointLimits::default();
    Ok(JointModel::from_factor(
        &sources,
        SourceFactor::new(2, coefficients, &[P::ONE], limits)?,
        &std::array::from_fn::<_, 4, _>(|_| CovarianceBlock::Known),
        limits,
    )?)
}

/// Exact independent rank-one entries include the complete ordered cross block.
fn covariance_oracle<P: Scalar>(model: &JointModel<P>) -> TestResult {
    let context = ArithmeticContext::new()?;
    for (row, column, denominator) in [
        (0, 0, 16_384),
        (0, 1, 65_536),
        (1, 0, 65_536),
        (1, 1, 262_144),
    ] {
        let enclosure = model.covariance(row, column, &context, JointLimits::default())?;
        assert!(
            crate::reciprocal_testing::oracle::Rational::new(1, denominator)?.enclosed(
                enclosure.lower().get().to_f64(),
                enclosure.upper().get().to_f64()
            )?
        );
    }
    Ok(())
}

/// Publication needs three additional float entries and refuses one entry below atomically.
/// ´test:integration:lc-selected-joint-storage-boundary´
#[test]
fn lc_selected_joint_storage_boundary() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    for (elements, success) in [(5, true), (4, false)] {
        let mut heap = Heap::new(nrz_matrix::HeapLimits::new(4096, elements, 4096))?;
        let original = super::represented(&fixture, &mut heap)?;
        let selected = super::invariants::selections(&mut heap, &original)?;
        let handles = capture_handles(&mut heap, &selected)?;
        let draft = CompanionSetDraft::new(
            joint_model(&selected, &[0.007_812_5, 0.001_953_125])?,
            capture_targets(&heap, handles)?,
            [vec![0], vec![1]],
        );
        let result = heap.publish_joint(draft, JointLimits::default());
        assert_eq!(
            result.is_ok(),
            success,
            "element capacity {elements}: {result:?}"
        );
        if let Ok(companion) = result {
            let view = heap.statistical(companion)?;
            let entries = view.factor_region().ok_or("factor")?.extent().entries()
                + view.weight_region().ok_or("weights")?.extent().entries();
            assert_eq!(entries, 3);
        } else {
            assert!(matches!(result, Err(JointError::Heap(HeapError::Capacity))));
        }
        assert_eq!(
            heap.joint_companions(handles[0])?.len(),
            usize::from(success)
        );
        for (source, handle) in original.iter().zip(handles) {
            assert_eq!(heap.reading_entries(source)?.len(), 1);
            assert_eq!(heap.value(handle)?.coverage(), Coverage::RepresentedInputs);
            assert!(heap.value(handle)?.bound().is_none());
        }
    }
    Ok(())
}
