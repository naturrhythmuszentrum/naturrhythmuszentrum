// SPDX-License-Identifier: AGPL-3.0-only
//! Original source order and complete factor covariance remain independent of output variances.

use super::{family_support as family, reciprocal_statistics as shared, root_support as inputs};
use crate::reading_testing::TestResult;
use crate::reciprocal_testing::oracle::Rational;
use crate::spectral_testing::Fixture;
use nrz_bindings::Budget;
use nrz_matrix::{
    CompanionSetDraft, CovarianceBlock, JointLimits, JointModel, ReciprocalDerivativeContract,
    Source, SourceFactor,
};
use nrz_scalar::{ArithmeticContext, Enclosure, Scalar};

/// Reordered original columns and an independent second fluctuation both retain the complete family.
/// ´test:integration:lc-operator-root-family-preserves-source-order-and-full-rank-model´
#[test]
fn lc_operator_root_family_preserves_source_order_and_full_rank_model() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_output_store_limits(budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    for full_rank in [false, true] {
        verify::<f32>(&fixture, &contract, full_rank, &mut budget)?;
        verify::<f64>(&fixture, &contract, full_rank, &mut budget)?;
    }
    Ok(())
}

fn verify<P: Scalar + 'static>(
    fixture: &Fixture,
    contract: &ReciprocalDerivativeContract,
    full_rank: bool,
    budget: &mut Budget,
) -> TestResult {
    let mut case = prepare::<P>(fixture, contract, full_rank, budget)?;
    let outputs = family::publish(fixture, &mut case, budget)?;
    let moments = case.heap.moments(outputs.moments)?;
    let identity = outputs
        .roots
        .evaluation()
        .plan()
        .statistical_identity()
        .ok_or("root identity")?;
    assert_eq!(moments.source_indices(), [0, 1]);
    assert_eq!(
        identity.1,
        [moments.source_ids()[1], moments.source_ids()[0]]
    );
    let values = case.heap.lc_operator_root_covariance(
        outputs.moments,
        &outputs.roots,
        &ArithmeticContext::new()?,
        budget,
    )?;
    oracle(&values, full_rank)
}

/// Both native scalar selections reverse the model's source columns without introducing any copies.
pub(super) fn prepare<P: Scalar + 'static>(
    fixture: &Fixture,
    contract: &ReciprocalDerivativeContract,
    full_rank: bool,
    budget: &mut Budget,
) -> TestResult<inputs::Case<P>> {
    let values = [
        [P::ONE / P::from_u8(4), P::ZERO, P::ZERO],
        [P::ONE / P::from_u8(16), P::ZERO, P::ZERO],
    ];
    let (mut heap, original) = fixture.import_with_budget(values, budget)?;
    let selected = original
        .iter()
        .map(|input| heap.select_reading(input, budget))
        .collect::<Result<Vec<_>, _>>()?;
    let handles = super::selected_joint::capture_handles(&mut heap, &selected)?;
    let sources = [1, 0]
        .map(|index| Source::new(selected[index].symbol().descriptor(), 1))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let limits = JointLimits::default();
    let inductance = P::ONE / P::from_u8(128);
    let capacitance = inductance / P::from_u8(4);
    let (coefficients, weights) = if full_rank {
        (
            vec![
                capacitance,
                inductance,
                -capacitance / P::from_u8(2),
                inductance / P::from_u8(2),
            ],
            vec![P::ONE; 2],
        )
    } else {
        (vec![capacitance, inductance], vec![P::ONE])
    };
    let factor = SourceFactor::new(2, &coefficients, &weights, limits)?;
    let model = JointModel::from_factor(
        &sources,
        factor,
        &[
            CovarianceBlock::Known,
            CovarianceBlock::Known,
            CovarianceBlock::Known,
            CovarianceBlock::Known,
        ],
        limits,
    )?;
    let targets = super::selected_joint::capture_targets(&heap, handles)?;
    let companion = heap.publish_joint(
        CompanionSetDraft::new(model, targets, [vec![1], vec![0]]),
        limits,
    )?;
    let derivative = heap.evaluate_reciprocal_derivative(
        contract,
        &original,
        &ArithmeticContext::new()?,
        budget,
    )?;
    Ok(inputs::Case {
        heap,
        original,
        selected,
        derivative,
        companion,
    })
}

/// The second fluctuation changes operator variance while its two nominal root contributions cancel.
fn oracle<P: Scalar>(values: &[[Enclosure<P>; 8]; 8], full_rank: bool) -> TestResult {
    let first = [
        (0, 1),
        (1, 320),
        (-1, 200_000),
        (0, 1),
        (0, 1),
        (-1, 4),
        (0, 1),
        (1, 4),
    ];
    let second = [
        (0, 1),
        (1, 640),
        (1, 400_000),
        (0, 1),
        (0, 1),
        (0, 1),
        (0, 1),
        (0, 1),
    ];
    for (row, entries) in values.iter().enumerate() {
        for (column, entry) in entries.iter().enumerate() {
            let mut expected = Rational::new(first[row].0, first[row].1)?
                .multiply(&Rational::new(first[column].0, first[column].1)?)?;
            if full_rank {
                expected = expected.add(
                    &Rational::new(second[row].0, second[row].1)?
                        .multiply(&Rational::new(second[column].0, second[column].1)?)?,
                )?;
            }
            assert!(expected.enclosed(entry.lower().get().to_f64(), entry.upper().get().to_f64())?);
        }
    }
    Ok(())
}
