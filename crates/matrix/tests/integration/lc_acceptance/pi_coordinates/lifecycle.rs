// SPDX-License-Identifier: AGPL-3.0-only
//! Hoisted pi coordinate owners preserve live source invalidation and allocation identity.

use super::{TestResult, exact_policy, fixture, ledger, pi};
use nrz_bindings::{
    Budget, ExactConstant, Scale,
    readings::algebra::{Location, syntax::Constant},
};
use nrz_matrix::{ConversionError, CoordinateError, Heap, PiCoordinateContract, ReadingBinding};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Every original state, radius and ordered covariance parameter remains a live dependency.
/// ´test:integration:pi-live-coordinate-invalidates-every-source´
#[test]
fn pi_live_coordinate_invalidates_every_source() -> TestResult {
    changes::<f32>()?;
    changes::<f64>()
}

/// A source-free coefficient owner is shared while every consuming selection captures its own versions.
fn changes<P: Scalar + 'static>() -> TestResult {
    let plan = fixture::plan()?;
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut ledger()?)?;
    let contract = PiCoordinateContract::new(
        &plan.graph,
        &plan.contract,
        &circle,
        &exact_policy(),
        &context,
        &mut ledger()?,
    )?;
    for change in 0..7 {
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
        let selected =
            heap.select_restored_reading(&restored, &inputs[1], &context, &mut ledger()?)?;
        replace(&mut heap, &inputs, change)?;
        let mut budget = prior()?;
        let before = budget.usage();
        assert!(
            heap.restore_certified_reading(
                &normalized,
                &contract,
                &fixture::execution(),
                &context,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.usage(), before);
        assert!(
            heap.normalize_certified_reading(
                &inputs[0],
                &contract,
                &fixture::execution(),
                &context,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.usage(), before);
        assert!(heap.coordinate_origin(&selected).is_err());
        assert!(heap.value_by_symbol(selected.symbol(), &selected).is_err());
        assert!(
            heap.select_restored_reading(&restored, &inputs[1], &context, &mut budget)
                .is_err()
        );
        assert_eq!(budget.usage(), before);
        assert!(contract.constant().same_region(&circle));
    }
    Ok(())
}

/// Equal-bit writes still advance each selected data or companion version.
fn replace<P: Scalar + 'static>(
    heap: &mut Heap,
    inputs: &[ReadingBinding<P>],
    change: usize,
) -> TestResult {
    if change < 2 {
        let target = &inputs[if change == 0 { 0 } else { 3 }];
        let values = heap.reading_entries(target)?;
        heap.replace_reading_entries(target, &values, &mut ledger()?)?;
    } else {
        let index = change - 2;
        let values = heap.reading_companion_entries(&inputs[0], index)?;
        heap.replace_reading_companion_entries(&inputs[0], index, &values, &mut ledger()?)?;
    }
    Ok(())
}

/// Distinct immutable graph and constant authorities never alias a selected coordinate certificate.
/// ´test:integration:pi-live-coordinate-refuses-substitution´
#[test]
fn pi_live_coordinate_refuses_substitution() -> TestResult {
    substitutions::<f32>()?;
    substitutions::<f64>()
}

/// A new constant region has its own certificate; a clone is the same region without new coefficients.
fn substitutions<P: Scalar + 'static>() -> TestResult {
    let plan = fixture::plan()?;
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut ledger()?)?;
    let other = pi::<P>(&context, &mut ledger()?)?;
    assert!(!circle.same_region(&other));
    let contract = PiCoordinateContract::new(
        &plan.graph,
        &plan.contract,
        &circle,
        &exact_policy(),
        &context,
        &mut ledger()?,
    )?;
    let separate = PiCoordinateContract::new(
        &plan.graph,
        &plan.contract,
        &other,
        &exact_policy(),
        &context,
        &mut ledger()?,
    )?;
    assert!(!contract.same_region(&separate));
    assert!(contract.same_region(&contract.clone()));
    let (heap, source) = fixture::imported::<P>(&plan)?;
    let normalized = heap.normalize_certified_reading(
        &source,
        &contract,
        &fixture::execution(),
        &context,
        &mut ledger()?,
    )?;
    let replacement = heap.normalize_certified_reading(
        &source,
        &separate,
        &fixture::execution(),
        &context,
        &mut ledger()?,
    )?;
    assert!(!std::ptr::eq(
        normalized.coefficients().as_ptr(),
        replacement.coefficients().as_ptr()
    ));
    let mut budget = prior()?;
    let before = budget.usage();
    assert!(matches!(
        heap.restore_certified_reading(
            &normalized,
            &separate,
            &fixture::execution(),
            &context,
            &mut budget
        ),
        Err(CoordinateError::Identity)
    ));
    assert_eq!(budget.usage(), before);
    Ok(())
}

/// A certificate refuses a foreign graph and never accepts a restoration as normalized input.
/// ´test:integration:pi-live-coordinate-refuses-graph-and-direction´
#[test]
fn pi_live_coordinate_refuses_graph_and_direction() -> TestResult {
    graph_and_direction::<f32>()?;
    graph_and_direction::<f64>()
}

/// Exact graph authority is independent of equal numerical values and equal declaration text.
fn graph_and_direction<P: Scalar + 'static>() -> TestResult {
    let plan = fixture::plan()?;
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut ledger()?)?;
    let contract = PiCoordinateContract::new(
        &plan.graph,
        &plan.contract,
        &circle,
        &exact_policy(),
        &context,
        &mut ledger()?,
    )?;
    let (heap, source) = fixture::imported::<P>(&plan)?;
    let normalized = heap.normalize_certified_reading(
        &source,
        &contract,
        &fixture::execution(),
        &context,
        &mut ledger()?,
    )?;
    let mut budget = prior()?;
    let before = budget.usage();
    let restored = heap.restore_certified_reading(
        &normalized,
        &contract,
        &fixture::execution(),
        &context,
        &mut ledger()?,
    )?;
    assert!(matches!(
        heap.restore_certified_reading(
            &restored,
            &contract,
            &fixture::execution(),
            &context,
            &mut budget
        ),
        Err(CoordinateError::Identity)
    ));
    assert_eq!(budget.usage(), before);
    let foreign = fixture::plan()?;
    let (foreign_heap, foreign_source) = fixture::imported::<P>(&foreign)?;
    assert!(matches!(
        foreign_heap.normalize_certified_reading(
            &foreign_source,
            &contract,
            &fixture::execution(),
            &context,
            &mut budget
        ),
        Err(CoordinateError::Identity)
    ));
    assert_eq!(budget.usage(), before);
    Ok(())
}

/// Unsupported exact graphs retain the named numerical refusal without consuming caller Work.
/// ´test:integration:pi-live-coordinate-unsupported-constants-are-atomic´
#[test]
fn pi_live_coordinate_unsupported_constants_are_atomic() -> TestResult {
    let mut setup = ledger()?;
    let exact = ExactConstant::named(Constant::Pi, &mut setup)?;
    for value in [
        ExactConstant::named(Constant::E, &mut setup)?,
        exact.mul(&exact, &mut setup)?,
    ] {
        let plan = fixture::with_scale(Scale::Expression(value))?;
        unsupported::<f32>(&plan)?;
        unsupported::<f64>(&plan)?;
    }
    Ok(())
}

/// The positive sum is refused by exact reading-map proof before numerical admission is available.
/// ´test:integration:pi-live-coordinate-sum-refuses-exact-map´
#[test]
fn pi_live_coordinate_sum_refuses_exact_map() -> TestResult {
    use nrz_bindings::readings::{
        ReadingError,
        algebra::{FaultKind, InvalidCode},
    };
    let mut setup = ledger()?;
    let exact = ExactConstant::named(Constant::Pi, &mut setup)?;
    let sum = exact.add(&ExactConstant::one(), &mut setup)?;
    let numerical_caller = prior()?;
    let before = numerical_caller.usage();
    let result = fixture::with_scale(Scale::Expression(sum));
    let error = result.err().ok_or("sum map unexpectedly admitted")?;
    let Some(ReadingError::Algebra(fault)) = error.downcast_ref::<ReadingError>() else {
        return Err(format!("expected exact reading refusal, got {error:?}").into());
    };
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::ReadingNonsingularity)
    );
    assert_eq!(numerical_caller.usage(), before);
    Ok(())
}

/// Graph authentication cannot grant numerical authority to an unsupported coefficient grammar.
fn unsupported<P: Scalar + 'static>(plan: &super::state_fixture::Plan) -> TestResult {
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut ledger()?)?;
    let mut budget = prior()?;
    let before = budget.usage();
    assert!(matches!(
        PiCoordinateContract::new(
            &plan.graph,
            &plan.contract,
            &circle,
            &exact_policy(),
            &context,
            &mut budget
        ),
        Err(CoordinateError::Constant(ConversionError::Map(
            nrz_bindings::MapError::Units(nrz_bindings::UnitError::UnsupportedExactConstantExport)
        )))
    ));
    assert_eq!(budget.usage(), before);
    Ok(())
}

/// Earlier unrelated Work remains intact on every rejected attempt.
fn prior() -> TestResult<Budget> {
    let mut budget = ledger()?;
    budget.work(17, Location::new("prior live coordinate caller"))?;
    Ok(budget)
}
