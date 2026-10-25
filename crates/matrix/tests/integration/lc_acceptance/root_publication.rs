// SPDX-License-Identifier: AGPL-3.0-only
//! Published covariance retains output versions and cannot create deterministic source support.

use super::{reciprocal_statistics as shared, root_support as inputs};
use crate::reading_testing::TestResult;
use crate::spectral_testing::Fixture;
use nrz_matrix::{CompanionSetDraft, Coverage, Heap, HeapLimits, JointLimits, MomentAssurance};
use nrz_scalar::ArithmeticContext;

/// Statistical parameter bounds do not promote represented-only original readings to physical support.
/// ´test:integration:lc-root-statistics-preserve-represented-input-scope´
#[test]
fn lc_root_statistics_preserve_represented_input_scope() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_output_store_limits(budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let original = super::represented(&fixture, &mut heap)?;
    let selected = super::invariants::selections(&mut heap, &original)?;
    let handles = super::selected_joint::capture_handles(&mut heap, &selected)?;
    let model = super::selected_joint::joint_model(&selected, &[0.007_812_5, 0.001_953_125])?;
    let draft = CompanionSetDraft::new(
        model,
        super::selected_joint::capture_targets(&heap, handles)?,
        [vec![0], vec![1]],
    );
    let companion = heap.publish_joint(draft, JointLimits::default())?;
    let context = ArithmeticContext::new()?;
    let derivative =
        heap.evaluate_reciprocal_derivative(&contract, &original, &context, &mut budget)?;
    let plan = fixture
        .plan(original, &mut budget)?
        .with_reciprocal_statistics(&heap, derivative, companion, &mut budget)?;
    let publication = fixture.publication(plan, &mut budget)?;
    let roots = heap.publish_lc_spectrum(&publication, &context, &mut budget)?;
    assert_eq!(
        roots.evaluation().coverage(),
        [Coverage::RepresentedInputs; 2]
    );
    assert_eq!(
        roots.evaluation().moment_assurance(),
        Some(MomentAssurance::LinearizedMoments)
    );
    assert!(roots.root_covariance(&heap, &context)?.is_some());
    for reading in roots.readings().bindings() {
        assert_eq!(
            reading.phasor_input_coverage(),
            Some([Coverage::RepresentedInputs; 2])
        );
    }
    Ok(())
}

/// The shared root value and both root radius channels invalidate every published covariance block.
/// ´test:integration:lc-root-covariance-rechecks-published-parameters´
#[test]
fn lc_root_covariance_rechecks_published_parameters() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_output_store_limits(budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    for changed in 0..3 {
        let mut case = inputs::prepare(
            &fixture,
            &contract,
            [[0.25_f64, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]],
            [0.007_812_5, 0.001_953_125],
            true,
            &mut budget,
        )?;
        let plan = inputs::plan(&fixture, &case, &mut budget)?;
        let publication = fixture.publication(plan, &mut budget)?;
        let context = ArithmeticContext::new()?;
        let published = case
            .heap
            .publish_lc_spectrum(&publication, &context, &mut budget)?;
        let reading = &published.readings().bindings()[0];
        let before = case.heap.provenance_usage();
        let written = replace(&mut case.heap, reading, changed, &mut budget);
        if written.is_err() {
            assert_eq!(case.heap.provenance_usage(), before);
        }
        written.map_err(|error| format!("published parameter {changed} replacement: {error}"))?;
        let before = case.heap.provenance_usage();
        assert!(published.root_covariance(&case.heap, &context).is_err());
        assert_eq!(case.heap.provenance_usage(), before);
        let evaluated = published.evaluation().root_covariance(&case.heap, &context);
        if changed == 0 {
            // The shared graph's WholeBase policy also invalidates the original source readers.
            assert!(evaluated.is_err());
            assert_eq!(case.heap.provenance_usage(), before);
            for source in &case.original {
                assert!(case.heap.reading_entries(source).is_err());
                assert_eq!(case.heap.provenance_usage(), before);
            }
        } else {
            assert!(
                evaluated
                    .map_err(|error| format!("radius-only source evaluation {changed}: {error}"))?
                    .is_some()
            );
            for source in &case.original {
                assert_eq!(case.heap.reading_entries(source)?.len(), 1);
            }
        }
    }
    Ok(())
}

/// Context identifies the exact attempted mutation if a public write route refuses.
fn replace(
    heap: &mut Heap,
    reading: &nrz_matrix::ReadingBinding<f64>,
    changed: usize,
    budget: &mut nrz_bindings::Budget,
) -> TestResult {
    if changed == 0 {
        let values = heap
            .reading_entries(reading)
            .map_err(|error| format!("read value: {error}"))?;
        heap.replace_reading_entries(reading, &values, budget)?;
    } else {
        let index = changed - 1;
        let values = heap
            .reading_companion_entries(reading, index)
            .map_err(|error| format!("read radius {index}: {error}"))?;
        heap.replace_reading_companion_entries(reading, index, &values, budget)?;
    }
    Ok(())
}
