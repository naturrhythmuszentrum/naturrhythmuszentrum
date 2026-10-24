// SPDX-License-Identifier: AGPL-3.0-only
//! Fine driven parameter writes preserve their disjoint physical source readings.

use super::fixture;
use crate::reading_testing::{TestResult, ledger};
use nrz_bindings::readings::algebra::readings::EntryIndex;
use nrz_scalar::ArithmeticContext;

/// Published means inherit their actual operand closure and preserve disjoint parameter writes.
/// ´test:integration:driven-entries-parameter-isolation´
#[test]
fn driven_entries_parameter_isolation() -> TestResult {
    for policy in [
        nrz_matrix::DependencyPolicy::Entries,
        nrz_matrix::DependencyPolicy::WholeBase,
    ] {
        for changed in [15, 16] {
            parameter_isolation(policy, changed)?;
        }
    }
    Ok(())
}

fn parameter_isolation(policy: nrz_matrix::DependencyPolicy, changed: usize) -> TestResult {
    let mut case = fixture::lifecycle_case(policy, &mut super::budget()?)?;
    let means = case.means(&mut ledger()?)?;
    let executions = case.executions(&means, &mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    let statistics = case
        .heap
        .evaluate_phasor_statistics(&executions, &context, &mut ledger()?)?;
    for intersection in closures(&case, &means, changed)? {
        assert_eq!(intersection, [] as [EntryIndex; 0]);
    }
    let selected = case
        .heap
        .select_reading(&case.bindings[11], &mut ledger()?)?;
    let revision = case
        .heap
        .value_by_symbol(selected.symbol(), &selected)?
        .stamp();
    let roots = independent_spectrum(&mut case.heap)?;
    let published = mean_bits(&case.heap, &means)?;
    case.heap.compact()?;
    let original = case.heap.reading_entries(&case.bindings[changed])?;
    case.heap
        .replace_reading_entries(&case.bindings[changed], &original, &mut ledger()?)?;
    let fine = policy == nrz_matrix::DependencyPolicy::Entries;
    for (index, reading) in case.bindings.iter().enumerate() {
        let survives = fine && index != changed;
        assert_eq!(case.heap.reading_entries(reading).is_ok(), survives);
    }
    unavailable(&case.heap, &case.bindings[changed]);
    for mean in &means {
        assert_eq!(
            mean.evaluation().centers(&case.heap, &context).is_ok(),
            fine
        );
        for reading in mean.readings().bindings() {
            assert_eq!(case.heap.reading_entries(reading).is_ok(), fine);
            assert_eq!(
                reading.state(),
                if fine {
                    nrz_matrix::SymbolState::Published
                } else {
                    nrz_matrix::SymbolState::Unavailable
                }
            );
            if !fine {
                unavailable(&case.heap, reading);
            }
        }
    }
    if fine {
        assert_eq!(mean_bits(&case.heap, &means)?, published);
        assert_eq!(
            case.heap
                .value_by_symbol(selected.symbol(), &selected)?
                .stamp(),
            revision
        );
    }
    assert!(statistics.means(&case.heap, &context).is_err());
    assert!(statistics.moments(&case.heap, &context).is_err());
    assert!(statistics.parameter_errors(&case.heap, &context).is_err());
    spectrum_survives(&case.heap, &roots, &context)
}

fn mean_bits(
    heap: &nrz_matrix::Heap,
    means: &[nrz_matrix::PublishedPhasorSolve<f64>; 2],
) -> TestResult<Vec<Vec<u64>>> {
    means
        .iter()
        .flat_map(|mean| mean.readings().bindings())
        .map(|reading| {
            Ok(heap
                .reading_entries(reading)?
                .into_iter()
                .map(f64::to_bits)
                .collect())
        })
        .collect()
}

fn unavailable(heap: &nrz_matrix::Heap, reading: &nrz_matrix::ReadingBinding<f64>) {
    assert!(matches!(
        heap.reading_entries(reading),
        Err(nrz_matrix::LiveBindingError::Heap(
            nrz_matrix::HeapError::UnavailableSymbol
        ))
    ));
}

/// Whole expanded source or output writes retain the existing conservative exact-graph boundary.
/// ´test:integration:driven-expanded-writes-retain-coarse-invalidation´
#[test]
fn driven_expanded_writes_retain_coarse_invalidation() -> TestResult {
    for changed in [0, 5, 10, 17] {
        let mut case = fixture::entries_case()?;
        let means = case.means(&mut ledger()?)?;
        let executions = case.executions(&means, &mut ledger()?)?;
        let context = ArithmeticContext::new()?;
        let statistics =
            case.heap
                .evaluate_phasor_statistics(&executions, &context, &mut ledger()?)?;
        let roots = independent_spectrum(&mut case.heap)?;
        let reading = if changed == 17 {
            &means[0].readings().bindings()[0]
        } else {
            &case.bindings[changed]
        };
        let values = case.heap.reading_entries(reading)?;
        case.heap
            .replace_reading_entries(reading, &values, &mut ledger()?)?;
        for input in &case.bindings {
            unavailable(&case.heap, input);
        }
        for mean in &means {
            assert!(mean.evaluation().centers(&case.heap, &context).is_err());
            for output in mean.readings().bindings() {
                unavailable(&case.heap, output);
            }
        }
        assert!(statistics.moments(&case.heap, &context).is_err());
        spectrum_survives(&case.heap, &roots, &context)?;
    }
    Ok(())
}

/// Invalid values and unsupported partial expanded writes preserve all published readers and revisions.
/// ´test:integration:driven-entry-write-refusals-preserve-publications´
#[test]
fn driven_entry_write_refusals_preserve_publications() -> TestResult {
    let mut case = fixture::entries_case()?;
    let means = case.means(&mut ledger()?)?;
    let executions = case.executions(&means, &mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    let statistics = case
        .heap
        .evaluate_phasor_statistics(&executions, &context, &mut ledger()?)?;
    for (reading, values) in [
        (&case.bindings[11], vec![3.0]),
        (&means[0].readings().bindings()[3], vec![0.0]),
        (&case.bindings[15], vec![f64::NAN]),
        (&case.bindings[16], Vec::new()),
    ] {
        let before = fingerprint(&case, &means)?;
        assert!(
            case.heap
                .replace_reading_entries(reading, &values, &mut ledger()?)
                .is_err()
        );
        assert_eq!(fingerprint(&case, &means)?, before);
        for mean in &means {
            for output in mean.readings().bindings() {
                assert!(case.heap.reading_entries(output).is_ok());
            }
        }
        assert!(statistics.means(&case.heap, &context).is_ok());
        assert!(statistics.moments(&case.heap, &context).is_ok());
        assert!(statistics.parameter_errors(&case.heap, &context).is_ok());
    }
    Ok(())
}

struct Independent {
    roots: nrz_matrix::PublishedLcSpectrum<f64>,
    selected: nrz_matrix::ValueBinding<f64>,
    stamp: nrz_matrix::ValueStamp,
}

fn independent_spectrum(heap: &mut nrz_matrix::Heap) -> TestResult<Independent> {
    let fixture = crate::spectral_testing::Fixture::with_outputs(["5", "1/10", "1/2000"])?;
    let sources = fixture.import_into(
        heap,
        [[0.25_f64, 0.0, 0.0], [0.0625, 0.0, 0.0]],
        &mut ledger()?,
    )?;
    let plan = fixture.plan(sources, &mut ledger()?)?;
    let publication = fixture.publication(plan, &mut ledger()?)?;
    let roots =
        heap.publish_lc_spectrum(&publication, &ArithmeticContext::new()?, &mut ledger()?)?;
    let selected = heap.select_reading(&roots.readings().bindings()[4], &mut ledger()?)?;
    let stamp = heap.value_by_symbol(selected.symbol(), &selected)?.stamp();
    Ok(Independent {
        roots,
        selected,
        stamp,
    })
}

fn spectrum_survives(
    heap: &nrz_matrix::Heap,
    independent: &Independent,
    context: &ArithmeticContext,
) -> TestResult {
    assert_eq!(
        heap.value_by_symbol(independent.selected.symbol(), &independent.selected)?
            .stamp(),
        independent.stamp
    );
    for (index, sign) in [(0, 1), (1, -1)] {
        let value = independent
            .roots
            .reading(index, heap, context, &mut ledger()?)?[1];
        assert!(
            crate::reciprocal_testing::oracle::Rational::new(sign * 8, 1)?
                .covered(value.center, value.error)?
        );
    }
    Ok(())
}

/// Fresh output slots omit operand provenance; their empty closures cannot authorize publication isolation.
fn closures(
    case: &fixture::Case<f64>,
    means: &[nrz_matrix::PublishedPhasorSolve<f64>; 2],
    changed: usize,
) -> TestResult<Vec<Vec<EntryIndex>>> {
    let mut result = Vec::new();
    let written = case.bindings[changed].reading().family().root();
    for mean in means {
        for reading in mean.readings().bindings() {
            let mut resources = super::budget()?;
            let exact = case
                .graph
                .reading_dependencies(reading.reading().node(), &mut resources)?;
            result.push(case.graph.dependency_entry_intersection(
                &exact,
                written,
                &[EntryIndex { row: 0, column: 0 }],
                &mut resources,
            )?);
            let mut resources = super::budget()?;
            let exact = case.graph.witness_dependencies(
                reading.adapter().representation().witness(),
                &mut resources,
            )?;
            result.push(case.graph.dependency_entry_intersection(
                &exact,
                written,
                &[EntryIndex { row: 0, column: 0 }],
                &mut resources,
            )?);
        }
        for operand in mean.evaluation().plan().operands() {
            for input in std::iter::once(operand.whole()).chain(operand.copies()) {
                assert_ne!(input.reading(), case.bindings[15].reading());
                assert_ne!(input.reading(), case.bindings[16].reading());
            }
        }
    }
    Ok(result)
}

/// Every access revalidates the original captured entry revisions, without selecting fresh aliases.
/// Keep availability and every input/output value bit alongside those live revision checks.
pub(super) fn fingerprint(
    case: &fixture::Case<f64>,
    means: &[nrz_matrix::PublishedPhasorSolve<f64>; 2],
) -> TestResult<Vec<(nrz_matrix::SymbolState, Vec<u64>)>> {
    case.bindings
        .iter()
        .chain(means.iter().flat_map(|mean| mean.readings().bindings()))
        .map(|reading| {
            let values = case.heap.reading_entries(reading)?;
            Ok((
                reading.state(),
                values.into_iter().map(f64::to_bits).collect(),
            ))
        })
        .collect()
}

/// Every consumed impedance radius invalidates its own solve while the other mean survives.
/// ´test:integration:driven-operand-companion-writes-stale-dependent-means´
#[test]
fn driven_operand_companion_writes_stale_dependent_means() -> TestResult {
    for start in [0, 5] {
        for copy in [2, 3] {
            for channel in 0..2 {
                companion_write(start, copy, channel)?;
            }
        }
    }
    Ok(())
}

fn companion_write(start: usize, copy: usize, channel: usize) -> TestResult {
    let mut case = fixture::entries_case()?;
    let means = case.means(&mut ledger()?)?;
    let binding = &case.bindings[start + copy];
    let parameters = binding
        .companions()
        .iter()
        .enumerate()
        .filter(|(_, companion)| {
            companion
                .targets()
                .iter()
                .any(|target| target.reading() == binding.reading())
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    assert_eq!(parameters.len(), 2);
    let original = case
        .heap
        .reading_companion_entries(binding, parameters[channel])?;
    case.heap.replace_reading_companion_entries(
        binding,
        parameters[channel],
        &original,
        &mut ledger()?,
    )?;
    for (index, mean) in means.iter().enumerate() {
        let survives = index != start / 5;
        assert_eq!(
            mean.evaluation()
                .centers(&case.heap, &ArithmeticContext::new()?)
                .is_ok(),
            survives,
        );
        for reading in mean.readings().bindings() {
            assert_eq!(case.heap.reading_entries(reading).is_ok(), survives);
        }
    }
    Ok(())
}
