// SPDX-License-Identifier: AGPL-3.0-only
//! Complete original spectral law through live physical input bindings and restored numerical bounds.

use crate::spectral_testing::{Fixture, TestResult, ledger};
use nrz_scalar::{ArithmeticContext, LiteralLimits, NumericLiteral};

/// Independent reference rates and characteristic scale lie in both outward error channels.
/// ´test:integration:lc-spectral-reference-enclosures´
#[test]
fn lc_spectral_reference_enclosures() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let context = ArithmeticContext::new()?;
    let mut values = [[0.0_f64; 3]; 2];
    for (index, text) in ["1/40", "1/100000"].into_iter().enumerate() {
        let decoded =
            NumericLiteral::new(text, LiteralLimits::default())?.decode::<f64>(&context)?;
        let error = decoded
            .enclosure()
            .displacement_from(decoded.center(), &context)?
            .get();
        values[index] = [decoded.center().get(), error, error];
    }
    let mut storage = ledger()?;
    let (heap, inputs) = fixture.import_with_budget(values, &mut storage)?;
    let mut lowering = ledger()?;
    let plan = fixture.plan(inputs, &mut lowering)?;
    let mut execution = ledger()?;
    let evaluated = heap.evaluate_lc_spectrum(&plan, &context, &mut execution)?;
    for (value, expected) in evaluated
        .imaginary_rates(&heap, &context)?
        .into_iter()
        .zip([2000.0_f64, -2000.0])
    {
        assert!((value.center - expected).abs() <= value.error);
        assert!(value.error <= value.total);
    }
    let impedance = evaluated.characteristic_scale(&heap, &context)?;
    assert!((impedance.center - 50.0).abs() <= impedance.error);
    assert_eq!(
        (
            lowering.usage().work,
            execution.usage().work,
            plan.retained_payload_bytes(),
            fixture.input_construction_work,
            storage.usage().work
        ),
        (157_867, 358, 776, 185_892, 1481)
    );
    Ok(())
}

/// Zero-containing positive centers, negative boxes and strict zero boundaries all refuse.
/// ´test:integration:lc-spectral-whole-box-domain-refusals´
#[test]
fn lc_spectral_whole_box_domain_refusals() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let context = ArithmeticContext::new()?;
    for (point, radius, index) in [
        (0.0_f64, 0.0, 0),
        (-1.0, 0.0, 1),
        (1.0, 1.0, 0),
        (1.0, 2.0, 1),
    ] {
        let mut values = [[1.0, 0.0, 0.0]; 2];
        values[index] = [point, radius, 0.0];
        let (heap, inputs) = fixture.import(values)?;
        let plan = fixture.plan(inputs, &mut ledger()?)?;
        let result = heap.evaluate_lc_spectrum(&plan, &context, &mut ledger()?);
        if radius.to_bits() == 0.0_f64.to_bits() {
            assert!(
                matches!(result, Err(nrz_matrix::SpectralError::NonpositiveInput { input }) if input == index)
            );
        } else {
            assert!(
                matches!(result, Err(nrz_matrix::SpectralError::InputNotSeparated { input }) if input == index)
            );
        }
    }
    Ok(())
}

/// Replacing either original L/C value, even with identical bits, invalidates both conjugate rates.
/// ´test:integration:lc-spectral-original-source-invalidation´
#[test]
fn lc_spectral_original_source_invalidation() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let context = ArithmeticContext::new()?;
    for index in 0..2 {
        let (mut heap, inputs) = fixture.import([[1.0_f64, 0.0, 0.0]; 2])?;
        let plan = fixture.plan(inputs.clone(), &mut ledger()?)?;
        let first = heap.evaluate_lc_spectrum(&plan, &context, &mut ledger()?)?;
        let second = heap.evaluate_lc_spectrum(&plan, &context, &mut ledger()?)?;
        heap.replace_reading_entries(&inputs[index], &[1.0], &mut ledger()?)?;
        assert!(first.imaginary_rates(&heap, &context).is_err());
        assert!(second.characteristic_scale(&heap, &context).is_err());
        assert!(
            heap.evaluate_lc_spectrum(&plan, &context, &mut ledger()?)
                .is_err()
        );
        assert!(plan.law().coverage().witness().is_unconditional());
    }
    Ok(())
}

/// Equal schemas cannot reorder formal inputs or transfer an evaluation to another heap.
/// ´test:integration:lc-spectral-refuses-reordered-and-foreign-inputs´
#[test]
fn lc_spectral_refuses_reordered_and_foreign_inputs() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let context = ArithmeticContext::new()?;
    let (heap, inputs) = fixture.import([[1.0_f64, 0.0, 0.0]; 2])?;
    assert!(
        fixture
            .plan([inputs[1].clone(), inputs[0].clone()], &mut ledger()?)
            .is_err()
    );
    let plan = fixture.plan(inputs, &mut ledger()?)?;
    let evaluated = heap.evaluate_lc_spectrum(&plan, &context, &mut ledger()?)?;
    let other = nrz_matrix::Heap::new(nrz_matrix::HeapLimits::default())?;
    assert!(evaluated.imaginary_rates(&other, &context).is_err());
    Ok(())
}
