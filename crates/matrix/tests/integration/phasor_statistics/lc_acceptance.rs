// SPDX-License-Identifier: AGPL-3.0-only
//! Distinct autonomous and driven experiments coexist without sharing an invented statistical model.

use super::fixture;
use crate::reading_testing::{TestResult, ledger};
use crate::reciprocal_testing::oracle::Rational;
use nrz_matrix::{Coverage, PhasorError};
use nrz_scalar::ArithmeticContext;

/// A refused resonance current leaves valid impedance, both driven currents and the spectrum readable.
/// ´test:integration:lc-acceptance-resonance-preserves-independent-results´
#[test]
fn lc_acceptance_resonance_preserves_independent_results() -> TestResult {
    let spectral = crate::spectral_testing::Fixture::with_outputs(["5", "1/10", "1/2000"])?;
    let mut case = fixture::case::<f64>()?;
    let sources = spectral.import_into(
        &mut case.heap,
        [[0.25_f64, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]],
        &mut ledger()?,
    )?;
    let plan = spectral.plan(sources.clone(), &mut ledger()?)?;
    let publication = spectral.publication(plan, &mut ledger()?)?;
    let means = case.means(&mut ledger()?)?;
    let executions = case.executions(&means, &mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    let statistics = case
        .heap
        .evaluate_phasor_statistics(&executions, &context, &mut ledger()?)?;
    let mut resonance = crate::phasor_solve_testing::case_in_heap::<f64>(
        0.0,
        0.0,
        false,
        case.heap,
        &mut ledger()?,
    )?;
    let refused = resonance.publication(&mut ledger()?)?;
    let spectrum = resonance
        .heap
        .publish_lc_spectrum(&publication, &context, &mut ledger()?)?;
    assert!(matches!(
        resonance
            .heap
            .publish_phasor_solve(&refused, &context, &mut ledger()?),
        Err(PhasorError::ZeroImpedance)
    ));
    for value in resonance.heap.reading_entries(&resonance.bindings[0])? {
        assert_eq!(value.to_bits(), 0.0_f64.to_bits());
    }
    let moments = statistics.moments(&resonance.heap, &context)?;
    let errors = statistics.parameter_errors(&resonance.heap, &context)?;
    for (index, numerator) in [1, -1, 1].into_iter().enumerate() {
        assert!(Rational::new(numerator, 22_500)?.covered(moments[index], errors[index])?);
    }
    let value = spectrum.reading(0, &resonance.heap, &context, &mut ledger()?)?[1];
    assert!(Rational::new(8, 1)?.covered(value.center, value.error)?);
    assert!(value.total > value.error);
    assert_eq!(
        spectrum.evaluation().coverage(),
        [Coverage::DeclaredBounds; 2]
    );
    resonance
        .heap
        .replace_reading_entries(&sources[1], &[0.0625], &mut ledger()?)?;
    assert!(
        spectrum
            .reading(0, &resonance.heap, &context, &mut ledger()?)
            .is_err()
    );
    assert!(statistics.moments(&resonance.heap, &context).is_ok());
    for mean in means {
        assert!(mean.evaluation().centers(&resonance.heap, &context).is_ok());
    }
    Ok(())
}

/// Common voltage and parameter revisions stale the statistical experiment without touching autonomous roots.
/// ´test:integration:lc-acceptance-source-and-parameter-coarse-invalidation´
#[test]
fn lc_acceptance_source_and_parameter_coarse_invalidation() -> TestResult {
    let spectral = crate::spectral_testing::Fixture::with_outputs(["5", "1/10", "1/2000"])?;
    for changed in [10, 15, 16] {
        let mut case = fixture::case::<f64>()?;
        let sources = spectral.import_into(
            &mut case.heap,
            [[0.25_f64, 0.0, 0.0], [0.0625, 0.0, 0.0]],
            &mut ledger()?,
        )?;
        let plan = spectral.plan(sources, &mut ledger()?)?;
        let publication = spectral.publication(plan, &mut ledger()?)?;
        let means = case.means(&mut ledger()?)?;
        let executions = case.executions(&means, &mut ledger()?)?;
        let context = ArithmeticContext::new()?;
        let statistics =
            case.heap
                .evaluate_phasor_statistics(&executions, &context, &mut ledger()?)?;
        let spectrum = case
            .heap
            .publish_lc_spectrum(&publication, &context, &mut ledger()?)?;
        let original = case.heap.reading_entries(&case.bindings[changed])?;
        case.heap
            .replace_reading_entries(&case.bindings[changed], &original, &mut ledger()?)?;
        assert!(statistics.means(&case.heap, &context).is_err());
        assert!(statistics.moments(&case.heap, &context).is_err());
        assert!(statistics.parameter_errors(&case.heap, &context).is_err());
        assert!(
            spectrum
                .reading(0, &case.heap, &context, &mut ledger()?)
                .is_ok()
        );
        // WholeBase covers the complete atomic input group, including both parameters.
        for mean in means {
            assert!(mean.evaluation().centers(&case.heap, &context).is_err());
        }
    }
    Ok(())
}

/// Complete current vectors retain real/imaginary order on both covariance axes and selected-copy identity.
/// ´test:integration:lc-acceptance-complete-covariance-targets´
#[test]
fn lc_acceptance_complete_covariance_targets() -> TestResult {
    let mut case = fixture::vector_case::<f64>()?;
    let means = case.means(&mut ledger()?)?;
    let executions = case.executions(&means, &mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    for (execution, sign) in executions.iter().zip([1, -1, 1]) {
        let value = case
            .heap
            .evaluate_companion(execution, &context, &mut ledger()?)?;
        let values = value.values(&case.heap, &context)?;
        let errors = value.errors(&case.heap, &context)?;
        assert_eq!(values.len(), 4);
        for (index, numerator) in [0, 0, 0, sign].into_iter().enumerate() {
            assert!(Rational::new(numerator, 22_500)?.covered(values[index], errors[index])?);
        }
        let law = execution
            .exact()
            .target()
            .phasor_law()
            .ok_or("affine covariance law")?;
        for (target, gain) in execution.targets().iter().zip(law.gains()) {
            assert_eq!(target.reading().entries().len(), 2);
            assert_eq!(gain.inputs(), 1);
            assert_eq!(gain.outputs(), 2);
        }
    }
    let wrong = nrz_matrix::CompanionExecution::new(
        std::rc::Rc::clone(&case.exact[1]),
        [case.bindings[15].clone(), case.bindings[16].clone()],
        vec![
            means[0].readings().bindings()[0].clone(),
            means[1].readings().bindings()[5].clone(),
        ],
        nrz_scalar::LiteralLimits::default(),
        &mut ledger()?,
    );
    assert!(matches!(
        wrong,
        Err(nrz_matrix::CompanionExecutionError::Identity)
    ));
    let value = case
        .heap
        .evaluate_companion(&executions[1], &context, &mut ledger()?)?;
    let changed = &means[1].readings().bindings()[3];
    let original = case.heap.reading_entries(changed)?;
    assert!(matches!(
        case.heap
            .replace_reading_entries(changed, &original, &mut ledger()?),
        Err(nrz_matrix::LiveBindingError::Heap(
            nrz_matrix::HeapError::MissingCoverage
        ))
    ));
    assert!(value.values(&case.heap, &context).is_ok());
    let whole = &means[1].readings().bindings()[0];
    let original = case.heap.reading_entries(whole)?;
    case.heap
        .replace_reading_entries(whole, &original, &mut ledger()?)?;
    assert!(value.values(&case.heap, &context).is_err());
    assert!(value.errors(&case.heap, &context).is_err());
    // Expanded block writes require coarse invalidation across the shared exact graph.
    assert!(means[0].evaluation().centers(&case.heap, &context).is_err());
    Ok(())
}
