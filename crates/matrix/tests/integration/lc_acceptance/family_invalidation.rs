// SPDX-License-Identifier: AGPL-3.0-only
//! Each consumer call revalidates all original sources and both complete published outputs.

use super::{
    family_refusals::refuse, family_support as family, reciprocal_statistics as shared,
    root_support as inputs,
};
use crate::reading_testing::TestResult;
use crate::spectral_testing::Fixture;
use nrz_bindings::{Budget, CompanionKind, readings::CompanionReadingKind};
use nrz_matrix::{Heap, ReadingBinding, ReciprocalDerivativeContract};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Equal-bit changes to every original channel, operator value and root channel stale the family.
/// ´test:integration:lc-operator-root-family-rechecks-every-original-and-output-version´
#[test]
fn lc_operator_root_family_rechecks_every_original_and_output_version() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_output_store_limits(budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    changes::<f32>(&fixture, &contract, &mut budget)?;
    changes::<f64>(&fixture, &contract, &mut budget)
}

fn changes<P: Scalar + 'static>(
    fixture: &Fixture,
    contract: &ReciprocalDerivativeContract,
    budget: &mut Budget,
) -> TestResult {
    for change in 0..10 {
        let mut case = inputs::nominal::<P>(fixture, contract, budget)?;
        let outputs = family::publish(fixture, &mut case, budget)?;
        let context = ArithmeticContext::new()?;
        case.heap
            .lc_operator_root_covariance(outputs.moments, &outputs.roots, &context, budget)?;
        if change < 6 {
            let slot = change / 3;
            replace(&mut case.heap, &case.original[slot], change % 3, budget)?;
        } else if change < 9 {
            replace(
                &mut case.heap,
                &outputs.roots.readings().bindings()[0],
                change - 6,
                budget,
            )?;
        } else {
            let value = case.heap.value(outputs.operator.value())?;
            let version = value.stamp().version();
            let centers = (0..4)
                .map(|entry| value.region().get(entry % 2, entry / 2))
                .collect::<Result<Vec<_>, _>>()?;
            case.heap
                .replace_represented(outputs.operator.value(), version, &centers)?;
            assert!(
                outputs
                    .roots
                    .root_covariance(&case.heap, &context)?
                    .is_some()
            );
        }
        refuse(&case.heap, outputs.moments, &outputs.roots, budget)?;
        assert!(contract.exact().native().witness().is_discharged());
    }
    Ok(())
}

/// The original selected parameter target determines the radius channel, independently of its offset.
fn replace<P: Scalar + 'static>(
    heap: &mut Heap,
    reading: &ReadingBinding<P>,
    channel: usize,
    budget: &mut Budget,
) -> TestResult {
    if channel == 0 {
        let values = heap.reading_entries(reading)?;
        heap.replace_reading_entries(reading, &values, budget)?;
    } else {
        let kind = if channel == 1 {
            CompanionKind::DeterministicBound
        } else {
            CompanionKind::NumericalError
        };
        let index = reading
            .companions()
            .iter()
            .position(|companion| {
                companion.companion().kind() == CompanionReadingKind::Radius(kind)
                    && companion
                        .targets()
                        .iter()
                        .any(|target| target.reading() == reading.reading())
            })
            .ok_or("original companion")?;
        let values = heap.reading_companion_entries(reading, index)?;
        heap.replace_reading_companion_entries(reading, index, &values, budget)?;
    }
    Ok(())
}
