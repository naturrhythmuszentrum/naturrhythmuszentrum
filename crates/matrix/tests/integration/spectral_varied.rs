// SPDX-License-Identifier: AGPL-3.0-only
//! Independent changed-input and transport acceptance includes exact dyadic and enclosed rational inputs.

use crate::spectral_testing::{Fixture, TestResult, ledger};
use nrz_scalar::ArithmeticContext;

/// Both signs and both error channels cover independent exact changed-input rates and impedance.
/// ´test:integration:lc-spectral-varied-inputs-and-transports´
#[test]
fn lc_spectral_varied_inputs_and_transports() -> TestResult {
    let context = ArithmeticContext::new()?;
    for references in [
        ["5", "1/10", "1/2000"],
        ["10", "1/5", "1/1000"],
        ["2", "1/8", "1/4"],
    ] {
        let fixture = Fixture::with_outputs(references)?;
        for (inductance, capacitance, rate, impedance) in [
            (0.25_f64, 0.0625_f64, 8.0_f64, 2.0_f64),
            (0.0625, 0.25, 8.0, 0.5),
            (4.0, 0.25, 1.0, 4.0),
        ] {
            let (mut heap, inputs) =
                fixture.import([[inductance, 0.0, 0.0], [capacitance, 0.0, 0.0]])?;
            let plan = fixture.plan(inputs, &mut ledger()?)?;
            let evaluated = heap.evaluate_lc_spectrum(&plan, &context, &mut ledger()?)?;
            for (actual, expected) in evaluated
                .imaginary_rates(&heap, &context)?
                .into_iter()
                .zip([rate, -rate])
            {
                assert!((actual.center - expected).abs() <= actual.error);
                assert!(actual.error <= actual.total);
            }
            let actual = evaluated.characteristic_scale(&heap, &context)?;
            assert!((actual.center - impedance).abs() <= actual.error);
            assert_eq!(plan.law().multiplicities(), [1, 1]);
            assert!(plan.law().coverage().witness().is_unconditional());
            let publication = fixture.publication(plan, &mut ledger()?)?;
            let published = heap.publish_lc_spectrum(&publication, &context, &mut ledger()?)?;
            for (index, expected) in [(0, rate), (1, -rate)] {
                let root = published.reading(index, &heap, &context, &mut ledger()?)?;
                let imaginary = published.reading(index + 4, &heap, &context, &mut ledger()?)?[0];
                assert!((root[1].center - expected).abs() <= root[1].error);
                assert_eq!(root[1].center.to_bits(), imaginary.center.to_bits());
                assert_eq!(root[1].error.to_bits(), imaginary.error.to_bits());
                assert_eq!(root[0].center.to_bits(), 0.0_f64.to_bits());
            }
        }
    }
    Ok(())
}

/// The same checked signs and no-storage components execute in binary32 without a chosen tolerance.
/// ´test:integration:lc-spectral-published-binary32-enclosures´
#[test]
fn lc_spectral_published_binary32_enclosures() -> TestResult {
    let fixture = Fixture::with_outputs(["5", "1/10", "1/2000"])?;
    let context = ArithmeticContext::new()?;
    let (mut heap, inputs) = fixture.import([[0.25_f32, 0.0, 0.0], [0.0625, 0.0, 0.0]])?;
    let spectrum = fixture.plan(inputs, &mut ledger()?)?;
    let publication = fixture.publication(spectrum, &mut ledger()?)?;
    let published = heap.publish_lc_spectrum(&publication, &context, &mut ledger()?)?;
    for (index, expected) in [(0, 8.0_f32), (1, -8.0_f32)] {
        let values = published.reading(index, &heap, &context, &mut ledger()?)?;
        assert!((values[1].center - expected).abs() <= values[1].error);
        assert!(values[1].error <= values[1].total);
        assert_eq!(values[0].center.to_bits(), 0.0_f32.to_bits());
    }
    Ok(())
}

/// The independent physical reference pair is published through the same actual shared component bindings.
/// ´test:integration:lc-spectral-published-reference-pair´
#[test]
fn lc_spectral_published_reference_pair() -> TestResult {
    use nrz_scalar::{LiteralLimits, NumericLiteral};
    let fixture = Fixture::with_outputs(["5", "1/10", "1/2000"])?;
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
    let (mut heap, inputs) = fixture.import(values)?;
    let spectrum = fixture.plan(inputs, &mut ledger()?)?;
    let publication = fixture.publication(spectrum, &mut ledger()?)?;
    let published = heap.publish_lc_spectrum(&publication, &context, &mut ledger()?)?;
    for (root, expected) in [(0, 2000.0_f64), (1, -2000.0)] {
        let values = published.reading(root, &heap, &context, &mut ledger()?)?;
        let imaginary = published.reading(root + 4, &heap, &context, &mut ledger()?)?[0];
        assert!((values[1].center - expected).abs() <= values[1].error);
        assert!(values[1].error <= values[1].total);
        assert_eq!(values[1].center.to_bits(), imaginary.center.to_bits());
        assert_eq!(values[1].error.to_bits(), imaginary.error.to_bits());
        assert_eq!(values[1].total.to_bits(), imaginary.total.to_bits());
    }
    let normalized = published.evaluation().normalized(&heap, &context)?;
    assert!((normalized.center - 1.0).abs() <= normalized.error);
    let scale = published
        .evaluation()
        .characteristic_scale(&heap, &context)?;
    assert!((scale.center - 50.0).abs() <= scale.error);
    assert_eq!(publication.definition().law().multiplicities(), [1, 1]);
    Ok(())
}

/// Nonpositive and zero-containing physical families cannot publish any spectral result.
/// ´test:integration:lc-spectral-domain-refusals-are-atomic´
#[test]
fn lc_spectral_domain_refusals_are_atomic() -> TestResult {
    let fixture = Fixture::with_outputs(["5", "1/10", "1/2000"])?;
    let context = ArithmeticContext::new()?;
    for (center, radius) in [(0.0_f64, 0.0), (-1.0, 0.0), (1.0, 1.0), (1.0, 2.0)] {
        let (mut heap, inputs) = fixture.import([[center, radius, 0.0], [1.0, 0.0, 0.0]])?;
        let spectrum = fixture.plan(inputs, &mut ledger()?)?;
        let publication = fixture.publication(spectrum, &mut ledger()?)?;
        assert!(
            heap.publish_lc_spectrum(&publication, &context, &mut ledger()?)
                .is_err()
        );
    }
    Ok(())
}

/// Non-dyadic changed inputs propagate decoder displacement through two declared reference/time maps.
/// ´test:integration:lc-spectral-varied-rational-errors´
#[test]
fn lc_spectral_varied_rational_errors() -> TestResult {
    use nrz_scalar::{Finite, LiteralLimits, NumericLiteral};
    let context = ArithmeticContext::new()?;
    let rate = NumericLiteral::new("2100000", LiteralLimits::default())?
        .decode::<f64>(&context)?
        .enclosure()
        .sqrt(&context)?;
    let impedance = NumericLiteral::new("7000/3", LiteralLimits::default())?
        .decode::<f64>(&context)?
        .enclosure()
        .sqrt(&context)?;
    let values = rational_input_values(&context)?;
    for references in [["5", "1/10", "1/2000"], ["2", "1/8", "1/4"]] {
        let fixture = Fixture::with_outputs(references)?;
        let (mut heap, inputs) = fixture.import(values)?;
        let spectrum = fixture.plan(inputs, &mut ledger()?)?;
        let publication = fixture.publication(spectrum, &mut ledger()?)?;
        let published = heap.publish_lc_spectrum(&publication, &context, &mut ledger()?)?;
        for (index, expected) in [rate, rate.negated()].into_iter().enumerate() {
            let actual = published.reading(index, &heap, &context, &mut ledger()?)?[1];
            let required = expected.displacement_from(Finite::new(actual.center)?, &context)?;
            assert!(required.get() <= actual.error);
            assert!(actual.error > 0.0);
            assert!(actual.error <= actual.total);
        }
        let actual = published
            .evaluation()
            .characteristic_scale(&heap, &context)?;
        let required = impedance.displacement_from(Finite::new(actual.center)?, &context)?;
        assert!(required.get() <= actual.error);
        assert!(actual.error > 0.0);
        assert!(actual.error <= actual.total);
    }
    Ok(())
}

/// Decode the exact changed L/C values and preserve each outward displacement as its own error.
fn rational_input_values(context: &ArithmeticContext) -> TestResult<[[f64; 3]; 2]> {
    use nrz_scalar::{LiteralLimits, NumericLiteral};
    let mut values = [[0.0_f64; 3]; 2];
    for (index, text) in ["1/30", "1/70000"].into_iter().enumerate() {
        let decoded =
            NumericLiteral::new(text, LiteralLimits::default())?.decode::<f64>(context)?;
        let error = decoded
            .enclosure()
            .displacement_from(decoded.center(), context)?
            .get();
        assert!(error > 0.0);
        values[index] = [decoded.center().get(), error, error];
    }
    Ok(values)
}
