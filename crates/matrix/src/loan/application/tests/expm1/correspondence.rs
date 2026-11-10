// SPDX-License-Identifier: AGPL-3.0-only
//! Original correspondence and metadata refusals precede all managed allocation and selected loads.

use super::{Fixture, TestResult, fixture};
use crate::loan::fault::Injection;
use crate::{
    ApplicationError, Expm1ApplicationRequest, LoanError, PreparedExpm1Operation, RegionContract,
};
use std::rc::Rc;

/// Recheck the explicit original request with independently supplied region witnesses.
fn check(
    request: &crate::PointwiseExpm1Request,
    input: RegionContract,
    output: RegionContract,
    account: &mut crate::ExecutionAccount,
) -> Result<PreparedExpm1Operation, ApplicationError> {
    let original = request.law.authority()?.request();
    PreparedExpm1Operation::check(
        &request.law,
        &original.original.target,
        &original.original.context,
        &original.original.coordinates[0].coordinate,
        request.law.domain()?,
        original.bits,
        original.environment,
        input,
        output,
        request.operation.source.clone(),
        request.operation.destination.clone(),
        request.operation.axis,
        account,
    )
}
/// Original constructor loops visit four coordinate-conversion arrays and both complete entry lists.
#[test]
fn prepared_constructor_work_neighbours() -> TestResult {
    for (cells, components) in [(0, 1), (1, 1), (3, 2), (65, 3)] {
        let required = 1 + 2 + 4 * (cells + components) + 2 * cells * components + 4;
        for short in [0, 1] {
            let Fixture {
                mut account,
                request,
                ..
            } = fixture(cells, components, [-2.0, 0.0], 2, false)?;
            account.ledger.work(
                account.limits().work - account.usage().work - u64::try_from(required - short)?,
                crate::loan::AT,
            )?;
            let before = account.usage().work;
            let injection = Injection::after(0);
            let result = check(
                &request,
                request.operation.operation.inputs()[0].clone(),
                request.operation.operation.outputs()[0].clone(),
                &mut account,
            );
            assert_eq!(result.is_ok(), short == 0);
            if short == 0 {
                assert_eq!(account.usage().work - before, u64::try_from(required)?);
            } else {
                assert!(matches!(
                    result,
                    Err(ApplicationError::Loan(LoanError::Budget(_)))
                ));
                assert_eq!(account.usage().work - before, u64::try_from(required - 4)?);
            }
            assert_eq!(Injection::attempts(), 0);
            drop(injection);
        }
    }
    Ok(())
}
/// Equal shape and descriptor do not authorize new source/destination identities or axes.
#[test]
fn prepared_source_destination_and_axis_substitution_refuse_before_allocation() -> TestResult {
    for substitution in 0..3 {
        let Fixture {
            mut heap,
            mut account,
            mut request,
        } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
        if substitution == 0 {
            request.operation.source = nrz_bindings::BindingSymbols::declare(
                &nrz_bindings::Descriptor::ratio(),
                [1, 1],
                [false; 2],
                &mut account.ledger,
            )?;
        } else {
            let symbols = nrz_bindings::BindingSymbols::declare(
                &nrz_bindings::Descriptor::ratio(),
                [1, 1],
                [true; 2],
                &mut account.ledger,
            )?;
            let axis = if substitution == 2 {
                heap.create_cell_axis(1, &mut account.ledger)?
            } else {
                request.destination.axis
            };
            request.destination = crate::HostTileDeclaration::check(
                symbols,
                axis,
                crate::Coverage::RepresentedInputs,
                &mut account,
            )?;
        }
        let pin = request.input.ticket()?;
        let binding = crate::loan::retained_expm1::Binding::of(&request.law)?;
        let injection = Injection::after(0);
        crate::loan::fault::reset_reads();
        let returned = match heap.request_pointwise_expm1(request, &mut account) {
            Expm1ApplicationRequest::ApplicationRefused(ApplicationError::Loan(error), request) => {
                assert!(matches!(
                    error,
                    LoanError::WrongLoan | LoanError::LayoutMismatch
                ));
                request
            }
            other => return Err(format!("{other:?}").into()),
        };
        assert_eq!(returned.input.ticket()?, pin);
        assert_eq!(
            crate::loan::retained_expm1::Binding::of(&returned.law)?,
            binding
        );
        assert_eq!(Injection::attempts(), 0);
        assert_eq!(crate::loan::fault::reads(), 0);
        drop(injection);
    }
    Ok(())
}
/// Both full shapes are scanned even when an asserted zero is outside empty demand.
#[test]
fn prepared_structural_zero_refusals_preserve_original_scope() -> TestResult {
    for output in [false, true] {
        for scope in [
            crate::StructureScope::CenterOnly,
            crate::StructureScope::WholeFamily,
        ] {
            let Fixture {
                mut heap,
                mut account,
                mut request,
            } = fixture(2, 2, [-2.0, 0.0], 0, false)?;
            let original = request.operation.operation.inputs()[0].shape();
            let mut entries =
                vec![crate::EntryDraft::Declared(nrz_bindings::Descriptor::ratio()); 4];
            entries[3] = crate::EntryDraft::Zero(nrz_bindings::Descriptor::ratio());
            let shape = crate::ShapeDraft::new(
                original.domain().clone(),
                original.codomain().clone(),
                vec![crate::BlockDraft::new(0..2, 0..2, entries)],
                scope,
            )
            .check(
                Rc::clone(&original.declarations),
                crate::ShapeLimits::default(),
            )?;
            let changed = RegionContract::check(shape)?;
            let input = if output {
                request.operation.operation.inputs()[0].clone()
            } else {
                changed.clone()
            };
            let result = if output {
                changed
            } else {
                request.operation.operation.outputs()[0].clone()
            };
            request.operation = check(&request, input, result, &mut account)?;
            let before = account.usage().work;
            let injection = Injection::after(0);
            let error = match heap.request_pointwise_expm1(request, &mut account) {
                Expm1ApplicationRequest::ApplicationRefused(error, _) => error,
                other @ Expm1ApplicationRequest::ApplicationReady(_) => {
                    return Err(format!("{other:?}").into());
                }
            };
            assert!(
                matches!(error, ApplicationError::UnsupportedStructure { output: actual, entry: 3, scope: actual_scope } if actual == output && actual_scope == scope)
            );
            assert_eq!(
                account.usage().work - before,
                3 + 16 + 4 + u64::from(output) * 4
            );
            assert_eq!(Injection::attempts(), 0);
            drop(injection);
        }
    }
    Ok(())
}
/// Swapped component axes, derived operations and a nonidentity entry conversion are unsupported.
#[test]
fn prepared_component_maps_and_derived_shapes_refuse() -> TestResult {
    let Fixture {
        mut account,
        request,
        ..
    } = fixture(2, 2, [-2.0, 0.0], 2, false)?;
    for substitution in 0..4 {
        let input = request.operation.operation.inputs()[0].clone();
        let mut output = request.operation.operation.outputs()[0].clone();
        match substitution {
            0 => output.shape.domain = crate::kernel_testing::axis(2)?,
            1 => output.shape.recipe = crate::ShapeRecipe::Selection,
            2 => {
                output.shape.entries[0].conversion = Some(nrz_bindings::Conversion {
                    scale: nrz_bindings::readings::algebra::Rational::zero().into(),
                    shift: nrz_bindings::readings::algebra::Rational::zero().into(),
                });
            }
            3 => output.shape.entries[0].original = Some("original-expression".into()),
            _ => return Err("substitution".into()),
        }
        let injection = Injection::after(0);
        assert!(check(&request, input, output, &mut account).is_err());
        assert_eq!(Injection::attempts(), 0);
        drop(injection);
    }
    Ok(())
}
/// The same prepared tag cannot enter Square through a privately substituted checked operation.
#[test]
fn prepared_tag_refused_by_square_application() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
    let square = crate::PointwiseRequest {
        operation: request.operation.operation,
        input: request.input,
        destination: request.destination,
        demand: request.demand,
        limits: request.limits,
    };
    let injection = Injection::after(0);
    assert!(matches!(
        heap.request_pointwise_square(square, &mut account),
        crate::ApplicationRequest::ApplicationRefused(
            ApplicationError::Loan(LoanError::UnsupportedContract),
            _
        )
    ));
    assert_eq!(Injection::attempts(), 0);
    drop(injection);
    Ok(())
}

/// A separately named dimensionless kind is not the coherent ratio law.
#[test]
fn prepared_named_dimensionless_descriptor_is_unsupported() -> TestResult {
    let Fixture {
        mut account,
        request,
        ..
    } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
    let named = nrz_bindings::Descriptor::new(
        nrz_bindings::Kind::Named(
            "dimensionless-signal".into(),
            Box::new(nrz_bindings::Dimension::integers([0; 7])?),
        ),
        nrz_bindings::Measure::coherent(),
        nrz_bindings::Role::Difference,
    )?;
    assert_eq!(
        named.dimension(),
        nrz_bindings::Descriptor::ratio().dimension()
    );
    for output in [false, true] {
        let mut input = request.operation.operation.inputs()[0].clone();
        let mut result = request.operation.operation.outputs()[0].clone();
        if output {
            result.quantity = named.clone();
        } else {
            input.quantity = named.clone();
        }
        let injection = Injection::after(0);
        assert!(matches!(
            check(&request, input, result, &mut account),
            Err(ApplicationError::Loan(LoanError::UnsupportedContract))
        ));
        assert_eq!(Injection::attempts(), 0);
        drop(injection);
    }
    Ok(())
}
