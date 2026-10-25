// SPDX-License-Identifier: AGPL-3.0-only
//! Restored native selections retain pi evidence beyond temporary coordinate owners.

use super::{TestResult, exact_policy, fixture, ledger, pi};
use nrz_matrix::{Coverage, PiCoordinateContract};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Both numerical formats keep their original shared region after all temporary handles drop.
/// ´test:integration:pi-live-coordinate-retains-selected-evidence´
#[test]
fn pi_live_coordinate_retains_selected_evidence() -> TestResult {
    retention::<f32>()?;
    retention::<f64>()
}

/// Native publication preserves original assurance, covariance target identities and precision.
fn retention<P: Scalar + 'static>() -> TestResult {
    let plan = fixture::plan()?;
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut ledger()?)?;
    let original = circle.evidence();
    let contract = PiCoordinateContract::new(
        &plan.graph,
        &plan.contract,
        &circle,
        &exact_policy(),
        &context,
        &mut ledger()?,
    )?;
    let (mut heap, inputs) = fixture::all::<P>(&plan)?;
    let normalized = heap.normalize_certified_reading(
        &inputs[0],
        &contract,
        &fixture::execution(),
        &context,
        &mut ledger()?,
    )?;
    let restored = heap.restore_certified_reading(
        &normalized,
        &contract,
        &fixture::execution(),
        &context,
        &mut ledger()?,
    )?;
    let selected = heap.select_restored_reading(&restored, &inputs[1], &context, &mut ledger()?)?;
    let assurance = restored.coverage();
    assert_ne!(assurance, Coverage::RepresentedInputs);
    drop((normalized, restored, contract, circle, inputs, plan));
    heap.compact()?;
    let origin = heap.coordinate_origin(&selected)?;
    super::check(origin, true)?;
    assert_eq!(origin.coverage(), assurance);
    assert!(
        origin
            .constant()
            .ok_or("retained pi")?
            .evidence()
            .same_region(&original)
    );
    assert_eq!(
        origin
            .constant()
            .ok_or("retained pi")?
            .arithmetic_revision(),
        context.revision()
    );
    assert_eq!(
        origin
            .constant()
            .ok_or("retained pi")?
            .evidence()
            .precision(),
        P::PRECISION
    );
    assert_eq!(
        heap.value_by_symbol(selected.symbol(), &selected)?
            .coverage(),
        assurance
    );
    Ok(())
}
