// SPDX-License-Identifier: AGPL-3.0-only
//! Checked LC partials retain exact source evidence and invalidate affected numerical evaluations.

#[expect(
    clippy::redundant_pub_crate,
    reason = "derivative fixtures are shared within the public test target"
)]
mod fixture;

use crate::reading_testing::TestResult;
use crate::reciprocal_testing::oracle::Rational;
use fixture::{import, policy, prepare};
use nrz_bindings::{Budget, Limits};
use nrz_matrix::{Heap, HeapLimits, ReciprocalDerivativeError};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Both formats enclose independently computed LC coefficient derivatives at distinct operating points.
/// ´test:integration:reciprocal-derivative-encloses-original-coefficients´
#[test]
fn reciprocal_derivative_encloses_original_coefficients() -> TestResult {
    enclosures::<f32>()?;
    enclosures::<f64>()
}

/// Independent rational expectations cover every selected operator partial in both formats.
fn enclosures<P: Scalar + 'static>() -> TestResult {
    let mut budget = Budget::new(policy())?;
    let prepared = prepare(&[], &mut budget)?;
    let context = ArithmeticContext::new()?;
    for multiplier in [1, 2, 4] {
        let point = [
            P::from_u8(multiplier) / P::from_u8(32),
            P::from_u8(multiplier) / (P::from_u8(128) * P::from_u8(128) * P::from_u8(4)),
        ];
        let mut heap = Heap::new(HeapLimits::default())?;
        let imported = import(&mut heap, &prepared, &point, &mut budget)?;
        let before = budget.usage();
        let evaluation = heap.evaluate_reciprocal_derivative(
            &prepared.contract,
            imported.bindings(),
            &context,
            &mut budget,
        )?;
        assert_eq!(evaluation.usage().before, before);
        assert_eq!(evaluation.usage().after, budget.usage());
        let values = evaluation.values(&heap, &context, &mut budget)?;
        assert_eq!(values.len(), 8);
        for (index, input, reference) in [
            (1, 0, Rational::new(1, 40)?),
            (6, 1, Rational::new(-1, 100_000)?),
        ] {
            let original = Rational::binary(point[input].to_f64())?;
            let expected = reference.divide(&original.multiply(&original)?)?;
            assert!(expected.enclosed(
                values[index].lower().get().to_f64(),
                values[index].upper().get().to_f64()
            )?);
        }
        for index in [0, 2, 3, 4, 5, 7] {
            assert_eq!(values[index].lower().get().to_f64().to_bits(), 0);
            assert_eq!(values[index].upper().get().to_f64().to_bits(), 0);
        }
    }
    Ok(())
}

/// One selected C write invalidates every evaluated derivative while the original witness remains reusable.
/// ´test:integration:reciprocal-derivative-invalidates-shared-evaluations´
#[test]
fn reciprocal_derivative_invalidates_shared_evaluations() -> TestResult {
    let mut budget = Budget::new(policy())?;
    let prepared = prepare(&["C".into()], &mut budget)?;
    let context = ArithmeticContext::new()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let point = [0.03125_f64, 0.000_015_258_789_062_5];
    let imported = import(&mut heap, &prepared, &point, &mut budget)?;
    let first = heap.evaluate_reciprocal_derivative(
        &prepared.contract,
        imported.bindings(),
        &context,
        &mut budget,
    )?;
    let second = heap.evaluate_reciprocal_derivative(
        &prepared.contract,
        imported.bindings(),
        &context,
        &mut budget,
    )?;
    assert_eq!(first.values(&heap, &context, &mut budget)?.len(), 4);
    assert!(std::ptr::eq(
        first.contract().exact().native(),
        second.contract().exact().native()
    ));
    heap.replace_reading_entries(&imported.bindings()[1], &[point[1]], &mut budget)?;
    for evaluation in [&first, &second] {
        assert!(evaluation.values(&heap, &context, &mut budget).is_err());
        assert!(
            evaluation
                .operating_point(&heap, &context, &mut budget)
                .is_err()
        );
        assert!(
            evaluation
                .contract()
                .exact()
                .native()
                .witness()
                .is_discharged()
        );
    }
    let changed = [point[0] * 2.0, point[1] * 2.0];
    let imported = import(&mut heap, &prepared, &changed, &mut budget)?;
    let current = heap.evaluate_reciprocal_derivative(
        &prepared.contract,
        imported.bindings(),
        &context,
        &mut budget,
    )?;
    assert!(std::ptr::eq(
        first.contract().exact().native(),
        current.contract().exact().native()
    ));
    let captured = current.operating_point(&heap, &context, &mut budget)?;
    assert_eq!(
        captured
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        changed.map(f64::to_bits)
    );
    Ok(())
}

/// Original positivity and exact argument order are checked before any derivative result is exposed.
/// ´test:integration:reciprocal-derivative-refuses-invalid-operating-points´
#[test]
fn reciprocal_derivative_refuses_invalid_operating_points() -> TestResult {
    let mut budget = Budget::new(policy())?;
    let prepared = prepare(&[], &mut budget)?;
    let context = ArithmeticContext::new()?;
    for point in [[0.0_f64, 1.0], [1.0, -1.0], [1.0, 0.0]] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let imported = import(&mut heap, &prepared, &point, &mut budget)?;
        assert!(matches!(
            heap.evaluate_reciprocal_derivative(
                &prepared.contract,
                imported.bindings(),
                &context,
                &mut budget
            ),
            Err(ReciprocalDerivativeError::Arithmetic(_))
        ));
    }
    let mut heap = Heap::new(HeapLimits::default())?;
    let imported = import(&mut heap, &prepared, &[1.0_f64, 1.0], &mut budget)?;
    let mut reversed = imported.bindings().to_vec();
    reversed.reverse();
    assert!(matches!(
        heap.evaluate_reciprocal_derivative(&prepared.contract, &reversed, &context, &mut budget),
        Err(ReciprocalDerivativeError::Identity)
    ));
    Ok(())
}

/// Exact construction, derivative lowering, live import and evaluation share a single monotone allowance.
/// ´test:integration:reciprocal-derivative-cumulative-pipeline-boundary´
#[test]
fn reciprocal_derivative_cumulative_pipeline_boundary() -> TestResult {
    let mut measured = Budget::new(policy())?;
    let costs = pipeline(&mut measured)?;
    let required = measured.usage().work;
    assert_eq!(required, 8_791_324);
    for work in [required, required - 1] {
        let mut budget = Budget::new(Limits { work, ..policy() })?;
        let result = pipeline(&mut budget);
        assert_eq!(result.is_ok(), work == required, "{result:?}");
        if result.is_err() {
            let spent = budget.usage();
            assert!(pipeline(&mut budget).is_err());
            assert!(budget.usage().work >= spent.work);
        }
    }
    assert_eq!(costs, [8_314_366, 476_745, 213]);
    Ok(())
}

/// One complete production path keeps construction and numerical charges on the supplied ledger.
fn pipeline(budget: &mut Budget) -> TestResult<[u64; 3]> {
    let prepared = prepare(&[], budget)?;
    let exact = budget.usage().work;
    let mut heap = Heap::new(HeapLimits::default())?;
    let imported = import(
        &mut heap,
        &prepared,
        &[0.03125_f64, 0.000_015_258_789_062_5],
        budget,
    )?;
    let admitted = budget.usage().work;
    heap.evaluate_reciprocal_derivative(
        &prepared.contract,
        imported.bindings(),
        &ArithmeticContext::new()?,
        budget,
    )?;
    Ok([exact, admitted - exact, budget.usage().work - admitted])
}
