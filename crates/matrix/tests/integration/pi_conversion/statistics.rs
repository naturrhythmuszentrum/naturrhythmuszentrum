// SPDX-License-Identifier: AGPL-3.0-only
//! Shared pi coefficients transform full covariance and retain original model and map evidence.

use super::{conversion, oracle, owner, pi};
use crate::{
    conversion_testing::{TestResult, shape},
    kernel_oracle::Rational,
};
use nrz_bindings::{Budget, Limits, PairingWitness, UncertaintyMode};
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, JointLimits, OperationDraft, ParameterView,
    RecipeEvidence,
};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Both covariance axes, means and cross blocks consume the retained certified pi factors.
/// ´test:integration:pi-conversion-transforms-full-covariance´
#[test]
fn pi_conversion_transforms_full_covariance() -> TestResult {
    paired::<f32>()?;
    paired::<f64>()
}

/// Two angle representations share one numerical constant and one original statistical family.
fn paired<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let radian = declarations
        .registry()
        .measure("rad")
        .ok_or("radian")?
        .coherent();
    let degree = declarations.registry().measure("degree").ok_or("degree")?;
    let turn = declarations.registry().measure("turn").ok_or("turn")?;
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut Budget::new(Limits::default())?)?;
    let coordinates = shape(&declarations, &radian, 2)?;
    let contracts = vec![
        conversion(
            &declarations,
            [&radian, degree],
            [&coordinates; 2],
            &circle,
            false,
        )?,
        conversion(
            &declarations,
            [&radian, turn],
            [&coordinates; 2],
            &circle,
            false,
        )?,
    ];
    let mut heap = Heap::new(HeapLimits::default())?;
    let (source, declared) = declared_source::<P>(&mut heap, &radian)?;
    let original = heap.statistical(declared)?;
    let model = original.model().id();
    let ids = original
        .model()
        .sources()
        .iter()
        .map(nrz_matrix::Source::id)
        .collect::<Vec<_>>();
    let limits = ExecutionLimits::default();
    let bound = Binding::new(
        OperationDraft::convert(contracts).check(limits)?,
        vec![source.value(); 2],
        limits,
    )
    .validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    for evidence in heap.operation_record(outcome.record())?.evidence() {
        let RecipeEvidence::PiConversion(claim) = evidence else {
            return Err("missing pi evidence".into());
        };
        assert!(claim.constant().same_region(&circle.evidence()));
        assert_eq!(claim.constant().constant_revision(), circle.pi().revision());
        assert_eq!(claim.constant().precision(), P::PRECISION);
    }
    let left = outcome.output().first().ok_or("degree output")?;
    let right = outcome.output().get(1).ok_or("turn output")?;
    let first = oracle::factor(180, 1, true, 48)?;
    let second = oracle::factor(1, 2, true, 56)?;
    for (output, factor) in [(left, first), (right, second)] {
        let view = heap.moments(heap.moment_companion(output.value())?)?;
        covariance(view.covariance(), factor, factor)?;
        means(view.means().ok_or("means")?, factor)?;
        assert_eq!(view.model(), model);
        assert_eq!(view.source_ids(), ids);
    }
    let cross =
        heap.materialize_cross_covariance(left.value(), right.value(), JointLimits::default())?;
    let view = heap.moments(cross)?;
    covariance(view.covariance(), first, second)?;
    assert_eq!(view.model(), model);
    assert_eq!(view.source_ids(), ids);
    assert_eq!(
        view.targets(),
        [
            heap.value(left.value())?.stamp(),
            heap.value(right.value())?.stamp()
        ]
    );
    let descriptor =
        PairingWitness::uncertainty(UncertaintyMode::Covariance, degree, Some(turn), None)?;
    assert_eq!(view.covariance().descriptor(0, 1)?, &descriptor);
    assert_eq!(
        view.covariance().error_descriptor(0, 1)?.target(),
        &descriptor
    );
    Ok(())
}

/// Every entry, including off-diagonal covariance, encloses both independent factor endpoints.
fn covariance<P: Scalar + 'static>(
    view: &ParameterView<'_, P>,
    left: [Rational; 2],
    right: [Rational; 2],
) -> TestResult {
    for (index, denominator) in [4, 8, 8, 2].into_iter().enumerate() {
        let original = Rational::new(1, denominator)?;
        let bounds = [
            left[0].multiply(right[0])?.multiply(original)?,
            left[1].multiply(right[1])?.multiply(original)?,
        ];
        oracle::covers(
            bounds,
            view.get(index % 2, index / 2)?.to_f64(),
            view.error(index % 2, index / 2)?.to_f64(),
        )?;
    }
    Ok(())
}

/// Mean errors include both the exact factor's enclosure and finite multiplication error.
fn means<P: Scalar + 'static>(view: &ParameterView<'_, P>, factor: [Rational; 2]) -> TestResult {
    for index in 0..2 {
        let original = Rational::new(i128::try_from(index + 1)?, 1)?;
        oracle::covers(
            [factor[0].multiply(original)?, factor[1].multiply(original)?],
            view.get(index, 0)?.to_f64(),
            view.error(index, 0)?.to_f64(),
        )?;
    }
    Ok(())
}

/// Import one angle vector and retain the returned original joint companion identity.
fn declared_source<P: Scalar + 'static>(
    heap: &mut Heap,
    descriptor: &nrz_bindings::Descriptor,
) -> TestResult<(nrz_matrix::PublishedBundle<P>, nrz_matrix::CompanionRef<P>)> {
    let source =
        heap.import_represented(Extent::new(2, 1)?, descriptor, &[P::ONE, P::from_u8(2)])?;
    let declared = crate::statistics_testing::attach(
        heap,
        source.value(),
        &[
            P::ONE / P::from_u8(4),
            P::ONE / P::from_u8(8),
            P::ONE / P::from_u8(8),
            P::ONE / P::from_u8(2),
        ],
        true,
    )?;
    Ok((source, declared))
}
