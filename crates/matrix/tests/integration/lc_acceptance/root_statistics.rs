// SPDX-License-Identifier: AGPL-3.0-only
//! Independent original-coordinate rational covariance oracles cover both signed root blocks.

use super::{reciprocal_statistics as shared, root_support as inputs};
use crate::reading_testing::TestResult;
use crate::reciprocal_testing::oracle::Rational;
use crate::spectral_testing::Fixture;
use nrz_bindings::Budget;
use nrz_matrix::{Binding, ExecutionLimits, MomentAssurance, ReciprocalDerivativeContract};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Both formats enclose all four exact original-box corner covariance targets and the nominal one.
/// ´test:integration:lc-root-covariance-encloses-original-corners´
#[test]
fn lc_root_covariance_encloses_original_corners() -> TestResult {
    let mut budget = shared::policy()?;
    for references in [["5", "1/10", "1/2000"], ["10", "1/5", "1/1000"]] {
        let mut fixture = Fixture::with_store_limits(references, budget.limits().clone())?;
        let contract = shared::derivative(&mut fixture, &mut budget)?;
        corners::<f32>(&fixture, &contract, &mut budget)?;
        corners::<f64>(&fixture, &contract, &mut budget)?;
    }
    Ok(())
}

/// Physical fractions are constructed independently of every emitted root or derivative endpoint.
fn corners<P: Scalar + 'static>(
    fixture: &Fixture,
    contract: &ReciprocalDerivativeContract,
    budget: &mut Budget,
) -> TestResult {
    for (inductance, capacitance) in [(15, 15), (15, 17), (17, 15), (17, 17), (16, 16)] {
        let point = [
            P::from_u8(inductance) / P::from_u8(64),
            P::from_u8(capacitance) / (P::from_u8(128) * P::from_u8(2)),
        ];
        let coefficients = [
            P::ONE / P::from_u8(128),
            P::ONE / (P::from_u8(128) * P::from_u8(4)),
        ];
        let case = inputs::prepare(
            fixture,
            contract,
            point.map(|center| [center, P::ZERO, P::ZERO]),
            coefficients,
            true,
            budget,
        )?;
        let plan = inputs::plan(fixture, &case, budget)?;
        let context = ArithmeticContext::new()?;
        let result = case.heap.evaluate_lc_spectrum(&plan, &context, budget)?;
        let values = result
            .root_covariance(&case.heap, &context)?
            .ok_or("root covariance")?;
        let original = [
            Rational::new(i64::from(inductance), 64)?,
            Rational::new(i64::from(capacitance), 256)?,
        ];
        covariance(&values, &oracle(&original)?)?;
        assert_eq!(
            result.moment_assurance(),
            Some(MomentAssurance::LinearizedMoments)
        );
        assert!(std::ptr::eq(
            plan.root_jacobian()
                .ok_or("root adapter")?
                .operator()
                .native(),
            contract.exact().native()
        ));
        assert_eq!(
            plan.statistical_identity().ok_or("model identity")?.0,
            case.heap.statistical(case.companion)?.model().id()
        );
    }
    Ok(())
}

/// For one common fluctuation, variance is `(cL/L+cC/C)^2/(4*L*C)`.
fn oracle(point: &[Rational; 2]) -> TestResult<Rational> {
    let left = Rational::new(1, 128)?.divide(&point[0])?;
    let right = Rational::new(1, 512)?.divide(&point[1])?;
    let sum = left.add(&right)?;
    let denominator = Rational::new(4, 1)?
        .multiply(&point[0])?
        .multiply(&point[1])?;
    sum.multiply(&sum)?.divide(&denominator)
}

/// Real blocks are exact zero; the two imaginary self blocks agree and the cross blocks negate.
fn covariance<P: Scalar>(
    values: &[[nrz_scalar::Enclosure<P>; 4]; 4],
    variance: &Rational,
) -> TestResult {
    for (row, entries) in values.iter().enumerate() {
        for (column, value) in entries.iter().enumerate() {
            let imaginary = row % 2 == 1 && column % 2 == 1;
            let expected = if imaginary {
                variance.multiply(&Rational::new(if row == column { 1 } else { -1 }, 1)?)?
            } else {
                Rational::new(0, 1)?
            };
            assert!(expected.enclosed(value.lower().get().to_f64(), value.upper().get().to_f64())?);
            if !imaginary {
                assert_eq!(value.lower().get().to_f64().to_bits(), 0.0_f64.to_bits());
                assert_eq!(value.upper().get().to_f64().to_bits(), 0.0_f64.to_bits());
            }
        }
    }
    Ok(())
}

/// Operator and published signed roots retain one exact derivative, model and original endpoint scope.
/// ´test:integration:lc-root-publication-shares-original-operator-jacobian´
#[test]
fn lc_root_publication_shares_original_operator_jacobian() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_output_store_limits(budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    published::<f32>(&fixture, &contract, &mut budget)?;
    published::<f64>(&fixture, &contract, &mut budget)
}

/// The statistical branch leaves every deterministic bit unchanged and remains live after compaction.
fn published<P: Scalar + 'static>(
    fixture: &Fixture,
    contract: &ReciprocalDerivativeContract,
    budget: &mut Budget,
) -> TestResult {
    let quarter = P::ONE / P::from_u8(4);
    let sixteenth = P::ONE / P::from_u8(16);
    let mut case = inputs::prepare(
        fixture,
        contract,
        [
            [quarter, P::ONE / P::from_u8(64), P::ZERO],
            [
                sixteenth,
                P::ONE / (P::from_u8(128) * P::from_u8(2)),
                P::ZERO,
            ],
        ],
        [
            P::ONE / P::from_u8(128),
            P::ONE / (P::from_u8(128) * P::from_u8(4)),
        ],
        true,
        budget,
    )?;
    let context = ArithmeticContext::new()?;
    let baseline = fixture.plan(case.original.clone(), budget)?;
    let expected = case
        .heap
        .evaluate_lc_spectrum(&baseline, &context, budget)?;
    let plan = inputs::plan(fixture, &case, budget)?;
    let model = operator(fixture, &mut case, budget)?;
    assert_eq!(plan.statistical_identity().ok_or("identity")?.0, model);
    let publication = fixture.publication(plan, budget)?;
    let roots = case
        .heap
        .publish_lc_spectrum(&publication, &context, budget)?;
    let rates = roots.evaluation().imaginary_rates(&case.heap, &context)?;
    let baseline = expected.imaginary_rates(&case.heap, &context)?;
    for (actual, expected) in rates.into_iter().zip(baseline) {
        assert_eq!(
            [actual.center, actual.total, actual.error].map(|value| value.to_f64().to_bits()),
            [expected.center, expected.total, expected.error].map(|value| value.to_f64().to_bits())
        );
    }
    covariance(
        &roots
            .root_covariance(&case.heap, &context)?
            .ok_or("published covariance")?,
        &Rational::new(1, 16)?,
    )?;
    assert_eq!(
        roots.evaluation().coverage(),
        [nrz_matrix::Coverage::DeclaredBounds; 2]
    );
    partials(&roots, &case.heap, &context)?;
    drop(case.original);
    drop(case.selected);
    drop(case.derivative);
    case.heap.compact()?;
    covariance(
        &roots
            .root_covariance(&case.heap, &context)?
            .ok_or("compacted covariance")?,
        &Rational::new(1, 16)?,
    )
}

/// The native operator publishes the exact shared proof and original model used by the roots.
fn operator<P: Scalar + 'static>(
    fixture: &Fixture,
    case: &mut inputs::Case<P>,
    budget: &mut Budget,
) -> TestResult<nrz_matrix::ModelId> {
    let bound = Binding::from_symbols(
        super::operator(fixture)?,
        case.selected.clone(),
        ExecutionLimits::default(),
    )
    .with_reciprocal_statistics(case.derivative.clone(), budget)?
    .validate_with_resource_budget(&mut case.heap, budget)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let output = crate::kernel_testing::output(&outcome)?;
    let moments = case
        .heap
        .moments(case.heap.moment_companion(output.value())?)?;
    assert!(std::ptr::eq(
        moments
            .allocation_inventory()
            .operation()
            .ok_or("operation")?
            .reciprocal_jacobian()
            .ok_or("operator Jacobian")?
            .native(),
        case.derivative.contract().exact().native()
    ));
    Ok(moments.model())
}

/// At the nominal point the positive rate partials are exactly -16 and -64.
fn partials<P: Scalar + 'static>(
    roots: &nrz_matrix::PublishedLcSpectrum<P>,
    heap: &nrz_matrix::Heap,
    context: &ArithmeticContext,
) -> TestResult {
    let values = roots
        .evaluation()
        .root_partials(heap, context)?
        .ok_or("root partials")?;
    for (row, expected) in values
        .into_iter()
        .zip([[0, 0], [-16, -64], [0, 0], [16, 64]])
    {
        for (value, numerator) in row.into_iter().zip(expected) {
            assert!(
                Rational::new(numerator, 1)?
                    .enclosed(value.lower().get().to_f64(), value.upper().get().to_f64())?
            );
        }
    }
    Ok(())
}
