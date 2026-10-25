// SPDX-License-Identifier: AGPL-3.0-only
//! Certified conversion records retain constant lifetime and original data-version dependencies.

use super::{conversion, exact_policy, owner, pi};
use crate::conversion_testing::{TestResult, shape};
use nrz_bindings::{Budget, Limits, MapError, UnitError};
use nrz_matrix::{
    Binding, ConversionContract, ConversionError, ConversionMode, ExecutionLimits, Extent, Heap,
    HeapLimits, OperationDraft, OperationError, RecipeEvidence,
};
use nrz_scalar::{ArithmeticContext, LiteralLimits, Scalar};

/// Unbound pi coefficients refuse without publication, then explicit admission enables the same map.
/// ´test:integration:pi-conversion-requires-certified-constant´
#[test]
fn pi_conversion_requires_certified_constant() -> TestResult {
    unbound::<f32>()?;
    unbound::<f64>()
}

/// Failed ordinary lowering preserves graph usage and the original input version and value.
fn unbound<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let radian = declarations
        .registry()
        .measure("rad")
        .ok_or("radian")?
        .coherent();
    let degree = declarations.registry().measure("degree").ok_or("degree")?;
    let contract = ConversionContract::new(
        shape(&declarations, &radian, 1)?,
        shape(&declarations, degree, 1)?,
        declarations.conversion_map(&radian, degree)?,
        LiteralLimits::default(),
        ConversionMode::Deterministic,
    )?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &radian, &[P::ONE])?;
    let stamp = heap.value(source.value())?.stamp();
    let before = heap.provenance_usage();
    let result = crate::conversion_testing::execute(&mut heap, contract.clone(), source.value());
    let error = result
        .err()
        .ok_or("uncertified map unexpectedly succeeded")?;
    let failure = error
        .downcast_ref::<nrz_matrix::Failure<P>>()
        .ok_or_else(|| format!("expected typed evaluation failure, got {error:?}"))?;
    assert!(matches!(
        failure.cause(),
        OperationError::Conversion(ConversionError::Map(MapError::Units(
            UnitError::UnsupportedExactConstantExport
        )))
    ));
    assert_eq!(heap.provenance_usage(), before);
    assert_eq!(heap.value(source.value())?.stamp(), stamp);
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut Budget::new(Limits::default())?)?;
    contract.certify_pi(
        &circle,
        &exact_policy(),
        &context,
        &mut Budget::new(Limits::default())?,
    )?;
    crate::conversion_testing::execute(&mut heap, contract, source.value())?;
    Ok(())
}

/// Published evidence retains its original pi region after caller owners drop and data changes stale outputs.
/// ´test:integration:pi-conversion-record-lifetime-and-invalidation´
#[test]
fn pi_conversion_record_lifetime_and_invalidation() -> TestResult {
    lifetime::<f32>()?;
    lifetime::<f64>()
}

/// The source-free constant survives while the actual source dependency still controls validity.
fn lifetime<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let radian = declarations
        .registry()
        .measure("rad")
        .ok_or("radian")?
        .coherent();
    let degree = declarations.registry().measure("degree").ok_or("degree")?;
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut Budget::new(Limits::default())?)?;
    let original = circle.evidence();
    let coordinates = shape(&declarations, &radian, 1)?;
    let contract = conversion(
        &declarations,
        [&radian, degree],
        [&coordinates; 2],
        &circle,
        false,
    )?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &radian, &[P::ONE])?;
    let limits = ExecutionLimits::default();
    let operation = OperationDraft::convert(vec![contract]).check(limits)?;
    let bound = Binding::new(operation, vec![source.value()], limits).validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    drop(circle);
    let record = heap.operation_record(outcome.record())?;
    let Some(RecipeEvidence::PiConversion(claim)) = record.evidence().first() else {
        return Err("pi evidence".into());
    };
    assert!(claim.constant().same_region(&original));
    assert_eq!(claim.constant().constant_revision(), 1);
    let version = heap.value(source.value())?.stamp().version();
    heap.replace_represented(source.value(), version, &[P::from_u8(2)])?;
    assert!(heap.operation_record(outcome.record()).is_err());
    assert!(
        heap.value(outcome.output().first().ok_or("output")?.value())
            .is_err()
    );
    let historical = heap.historical_record(outcome.record())?;
    let Some(RecipeEvidence::PiConversion(claim)) = historical.evidence().first() else {
        return Err("historical pi evidence".into());
    };
    assert!(claim.constant().same_region(&original));
    Ok(())
}
