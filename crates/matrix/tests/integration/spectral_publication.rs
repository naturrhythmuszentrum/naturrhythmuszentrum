// SPDX-License-Identifier: AGPL-3.0-only
//! Both conjugate roots and every component share one checked live coordinate.

use crate::spectral_testing::{Fixture, TestResult, ledger};
use nrz_bindings::readings::RepresentationKind;
use nrz_scalar::ArithmeticContext;

/// Both Complex roots, exact real zeros and signed imaginary readings are published together.
/// ´test:integration:lc-spectral-publishes-complete-conjugate-family´
#[test]
fn lc_spectral_publishes_complete_conjugate_family() -> TestResult {
    let fixture = Fixture::with_outputs(["5", "1/10", "1/2000"])?;
    let (mut heap, inputs) = fixture.import([[0.25_f64, 0.0, 0.0], [0.0625, 0.0, 0.0]])?;
    let spectrum = fixture.plan(inputs, &mut ledger()?)?;
    let mut planning = ledger()?;
    let publication = fixture.publication(spectrum, &mut planning)?;
    let context = ArithmeticContext::new()?;
    let mut execution = ledger()?;
    let published = heap.publish_lc_spectrum(&publication, &context, &mut execution)?;
    assert_eq!(published.readings().bindings().len(), 6);
    views(&published, &heap, &context)?;
    assert_eq!(
        (
            planning.usage().work,
            execution.usage().work,
            publication.retained_payload_bytes(),
            fixture.outputs.as_ref().ok_or("outputs")?.work,
            publication.definition().retained_payload_bytes(),
            fixture.physical_construction_work,
            fixture.outputs.as_ref().ok_or("outputs")?.definition_work
        ),
        (123, 5_166, 503_608, 1_534_564, 227_968, 1_704_823, 13_763)
    );
    Ok(())
}

/// Replacing any original input or the shared output invalidates every root, component and zero.
/// ´test:integration:lc-spectral-published-family-retains-versions´
#[test]
fn lc_spectral_published_family_retains_versions() -> TestResult {
    let fixture = Fixture::with_outputs(["5", "1/10", "1/2000"])?;
    let context = ArithmeticContext::new()?;
    for changed in 0..3 {
        let (mut heap, inputs) = fixture.import([[1.0_f64, 0.0, 0.0]; 2])?;
        let spectrum = fixture.plan(inputs.clone(), &mut ledger()?)?;
        let publication = fixture.publication(spectrum, &mut ledger()?)?;
        let published = heap.publish_lc_spectrum(&publication, &context, &mut ledger()?)?;
        assert!(
            heap.publish_lc_spectrum(&publication, &context, &mut ledger()?)
                .is_err()
        );
        heap.compact()?;
        let target = if changed < 2 {
            &inputs[changed]
        } else {
            &published.readings().bindings()[0]
        };
        let entries = heap.reading_entries(target)?;
        heap.replace_reading_entries(target, &entries, &mut ledger()?)?;
        for index in 0..6 {
            assert!(
                published
                    .reading(index, &heap, &context, &mut ledger()?)
                    .is_err()
            );
            assert!(
                heap.reading_entries(&published.readings().bindings()[index])
                    .is_err()
            );
        }
    }
    Ok(())
}

/// Logical values and their own channels execute the retained signs over one stored coordinate.
fn views(
    published: &nrz_matrix::PublishedLcSpectrum<f64>,
    heap: &nrz_matrix::Heap,
    context: &ArithmeticContext,
) -> TestResult {
    let roots = [
        published.reading(0, heap, context, &mut ledger()?)?,
        published.reading(1, heap, context, &mut ledger()?)?,
    ];
    for (root, rate) in roots.iter().zip([8.0_f64, -8.0]) {
        assert_eq!(root[0].center.to_bits(), 0.0_f64.to_bits());
        assert_eq!(root[0].error.to_bits(), 0.0_f64.to_bits());
        assert!((root[1].center - rate).abs() <= root[1].error);
        assert!(root[1].error <= root[1].total);
    }
    for index in [2, 3] {
        let binding = &published.readings().bindings()[index];
        assert_eq!(
            binding.adapter().representation().kind(),
            RepresentationKind::None
        );
        assert_eq!(heap.reading_entries(binding)?, [] as [f64; 0]);
        assert_eq!(
            published.reading(index, heap, context, &mut ledger()?)?[0]
                .center
                .to_bits(),
            0.0_f64.to_bits()
        );
        assert!(!binding.companions().iter().any(|companion| {
            companion
                .targets()
                .iter()
                .any(|target| target.reading() == binding.reading())
        }));
    }
    for (index, root) in [(4, 0), (5, 1)] {
        let value = published.reading(index, heap, context, &mut ledger()?)?[0];
        assert_eq!(value.center.to_bits(), roots[root][1].center.to_bits());
        assert_eq!(value.error.to_bits(), roots[root][1].error.to_bits());
        assert_eq!(value.total.to_bits(), roots[root][1].total.to_bits());
    }
    assert_eq!(
        published.shared_discrepancy(heap, context)?.to_bits(),
        0.0_f64.to_bits()
    );
    for binding in published.readings().bindings() {
        assert!(binding.lc_spectral_result().is_some());
    }
    Ok(())
}

/// Wrong conjugate signs, logical projections and incomplete output inventories cannot be published.
/// ´test:integration:lc-spectral-publication-refuses-substituted-views´
#[test]
fn lc_spectral_publication_refuses_substituted_views() -> TestResult {
    use nrz_bindings::readings::{LcSpectralResultDefinition, SpectralLawError};
    let fixture = Fixture::with_outputs(["5", "1/10", "1/2000"])?;
    let outputs = fixture.outputs.as_ref().ok_or("outputs")?;
    let definition = &outputs.definition;
    let mut changed = definition.outputs().clone();
    let (first, rest) = changed.split_at_mut(1);
    rest[0].clone_from(&first[0]);
    assert!(matches!(
        LcSpectralResultDefinition::new(
            &fixture.graph,
            fixture.law.clone(),
            changed,
            definition.components().clone(),
            &mut ledger()?
        ),
        Err(SpectralLawError::Sign)
    ));
    let mut relations = definition.components().clone();
    relations.swap(0, 1);
    assert!(
        LcSpectralResultDefinition::new(
            &fixture.graph,
            fixture.law.clone(),
            definition.outputs().clone(),
            relations,
            &mut ledger()?
        )
        .is_err()
    );
    Ok(())
}

/// Missing zero views, reordered roots and missing or duplicated companions refuse before storage.
/// ´test:integration:lc-spectral-publication-requires-complete-inventory´
#[test]
fn lc_spectral_publication_requires_complete_inventory() -> TestResult {
    use nrz_matrix::{LcSpectralPublication, ReadingCoverage};
    let fixture = Fixture::with_outputs(["5", "1/10", "1/2000"])?;
    let outputs = fixture.outputs.as_ref().ok_or("outputs")?;
    let definition = &outputs.definition;
    let (_heap, inputs) = fixture.import([[1.0_f64, 0.0, 0.0]; 2])?;
    let spectrum = fixture.plan(inputs, &mut ledger()?)?;
    let mut readings = outputs.coverage.readings().to_vec();
    readings.remove(2);
    let missing = ReadingCoverage::new(
        outputs.coverage.entries().to_vec(),
        readings,
        outputs.coverage.overlaps().to_vec(),
        &mut ledger()?,
    )?;
    assert!(
        LcSpectralPublication::new(
            spectrum.clone(),
            definition.clone(),
            missing,
            outputs.companions.clone(),
            &mut ledger()?
        )
        .is_err()
    );
    let mut readings = outputs.coverage.readings().to_vec();
    readings.swap(0, 1);
    let swapped = ReadingCoverage::new(
        outputs.coverage.entries().to_vec(),
        readings,
        outputs.coverage.overlaps().to_vec(),
        &mut ledger()?,
    )?;
    assert!(
        LcSpectralPublication::new(
            spectrum.clone(),
            definition.clone(),
            swapped,
            outputs.companions.clone(),
            &mut ledger()?
        )
        .is_err()
    );
    let mut companions = outputs.companions.clone();
    companions.pop();
    assert!(
        LcSpectralPublication::new(
            spectrum.clone(),
            definition.clone(),
            outputs.coverage.clone(),
            companions,
            &mut ledger()?
        )
        .is_err()
    );
    let mut companions = outputs.companions.clone();
    let (first, rest) = companions.split_at_mut(1);
    rest[0].clone_from(&first[0]);
    assert!(
        LcSpectralPublication::new(
            spectrum,
            definition.clone(),
            outputs.coverage.clone(),
            companions,
            &mut ledger()?
        )
        .is_err()
    );
    Ok(())
}
