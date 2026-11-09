// SPDX-License-Identifier: AGPL-3.0-only
//! Sparse structural counterexamples and secondary dense comparisons retain original meanings.

use super::{TestResult, candidate, fixture, fixture_coverage, published, ready};
use crate::{ApplicationError, ApplicationRequest, HostChannel, StructureScope};
use nrz_bindings::Limits;
use nrz_scalar::Scalar;

/// The last structural entry lies outside a nonempty demand in every positive/refusal twin.
fn sparse_zero<P: Scalar + 'static>() -> TestResult {
    for output in [false, true] {
        for scope in [StructureScope::CenterOnly, StructureScope::WholeFamily] {
            for payload in 0..3 {
                let mut setup = fixture::<P>(3, 3, 3, 2, false, Limits::default())?;
                if payload > 0 {
                    super::refusal::replace_source(&mut setup, |_, _| {
                        (P::ZERO, if payload == 1 { P::ONE } else { P::ZERO })
                    })?;
                }
                let old_demand = setup.request.demand;
                setup.request.demand =
                    setup
                        .heap
                        .create_mask(old_demand.layout(), &[0], &mut setup.account.ledger)?;
                setup
                    .heap
                    .retire_mask(old_demand, &mut setup.account.ledger)?;
                let ordinary = setup.request.operation.clone();
                super::refusal::structural(&mut setup.request, output, 8, scope)?;
                let original = ordinary.inputs()[0].shape().declarations();
                let excluded = setup.request.operation.inputs()[0].shape().declarations();
                assert!(std::ptr::eq(original, excluded));
                let injection = crate::loan::fault::Injection::after(0);
                let mut request = match setup
                    .heap
                    .request_pointwise_square(setup.request, &mut setup.account)
                {
                    ApplicationRequest::ApplicationRefused(
                        ApplicationError::UnsupportedStructure {
                            output: actual,
                            entry: 8,
                            scope: actual_scope,
                        },
                        request,
                    ) => {
                        assert_eq!((actual, actual_scope), (output, scope));
                        request
                    }
                    other => return Err(format!("{other:?}").into()),
                };
                assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                drop(injection);
                request.operation = ordinary;
                let owner = candidate(
                    ready(
                        setup
                            .heap
                            .request_pointwise_square(request, &mut setup.account),
                    )?,
                    &mut setup.account,
                )?;
                let (tile, _) =
                    published(setup.heap.publish_application(owner, &mut setup.account))?;
                let reader = tile.reader(&setup.heap, &mut setup.account)?;
                assert!(reader.contains(0, &mut setup.account)?);
                assert!(!reader.contains(2, &mut setup.account)?);
                let expected = if payload == 0 { P::from_u8(4) } else { P::ZERO };
                assert_eq!(
                    reader.read_component(HostChannel::Center, 0, 2, &mut setup.account)?,
                    expected
                );
                if payload == 1 {
                    assert!(
                        reader.read_component(HostChannel::Total, 0, 2, &mut setup.account)?
                            >= P::ONE
                    );
                }
            }
        }
    }
    Ok(())
}
/// Equal-shape operations from one snapshot distinguish unsupported assertions from ordinary squares.
/// ´test:unit:pointwise-unselected-structural-zero-and-unconstrained-positive-twins´
#[test]
fn pointwise_unselected_structural_zero_and_unconstrained_positive_twins() -> TestResult {
    sparse_zero::<f32>()?;
    sparse_zero::<f64>()
}
/// Dense execution is a secondary regression; exact endpoint truth remains in the independent tests.
fn dense<P: Scalar + 'static>() -> TestResult {
    let mut setup = fixture_coverage::<P>(
        2,
        1,
        3,
        2,
        false,
        Limits::default(),
        crate::Coverage::DeclaredBounds,
    )?;
    let operation = setup.request.operation.clone();
    let owner = candidate(
        ready(
            setup
                .heap
                .request_pointwise_square(setup.request, &mut setup.account),
        )?,
        &mut setup.account,
    )?;
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    let extent = crate::Extent::new(2, 1)?;
    let centers = [P::from_u8(2), P::ONE / P::from_u8(4)];
    let totals = [P::ONE / P::from_u8(8), P::ONE / P::from_u8(2)];
    let source = setup.heap.import_declared_bounds(
        extent,
        &nrz_bindings::Descriptor::ratio(),
        &centers,
        &totals,
    )?;
    let bound = crate::Binding::new(
        operation,
        vec![source.value()],
        crate::ExecutionLimits::default(),
    )
    .validate(&mut setup.heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let output = crate::kernel_testing::output(&outcome)?;
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    assert_eq!(reader.coverage(), crate::Coverage::DeclaredBounds);
    for cell in 0..2 {
        let expected: [f64; 3] = crate::kernel_testing::entry(&setup.heap, output, cell)?.into();
        for (channel, expected) in HostChannel::ALL.into_iter().zip(expected) {
            assert_eq!(
                reader
                    .read_component(channel, cell, 0, &mut setup.account)?
                    .to_f64()
                    .to_bits(),
                expected.to_bits()
            );
        }
    }
    Ok(())
}
/// Both formats preserve declared-bound coverage and agree with the unchanged dense wrapper.
/// ´test:unit:pointwise-declared-bounds-and-dense-square-regression´
#[test]
fn pointwise_declared_bounds_and_dense_square_regression() -> TestResult {
    dense::<f32>()?;
    dense::<f64>()
}
