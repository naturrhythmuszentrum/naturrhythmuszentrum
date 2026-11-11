// SPDX-License-Identifier: AGPL-3.0-only
//! Original descriptor power and declaration identities precede represented numerical evaluation.

use super::{Fixture, TestResult, candidate, published, ready};
use crate::{
    ApplicationError, ApplicationRequest, Coverage, Elementwise, ExecutionAccount, ExecutionLimits,
    HandbackOutcome, Heap, HeapLimits, HostChannel, HostTileDeclaration, LoanError, LoanRequest,
    PointwiseRequest,
};
use nrz_bindings::{
    BindingSymbols, Budget, Context, Descriptor, Dimension, ExponentPolicy, Kind, Limits, Measure,
    Registry, Role,
};
use std::rc::Rc;

/// Physical dimensions are specified independently for a length and an inverse length.
fn descriptor(name: &str, power: i64) -> TestResult<Descriptor> {
    Ok(Descriptor::new(
        Kind::Named(
            name.into(),
            Box::new(Dimension::integers([power, 0, 0, 0, 0, 0, 0])?),
        ),
        Measure::coherent(),
        Role::Difference,
    )?)
}
/// Bindings owns the checked minus-one power; the result schema is declared independently.
#[expect(
    clippy::default_trait_access,
    reason = "the frozen bindings context infers its private exact environment type"
)]
fn physical_fixture() -> TestResult<Fixture<f64>> {
    let input_descriptor = descriptor("length", 1)?;
    let output_descriptor = descriptor("inverse-length", -1)?;
    let exponent = Dimension::integers([-1, 0, 0, 0, 0, 0, 0])?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_power(
        input_descriptor.kind().clone(),
        exponent.exponents()[0].clone(),
        output_descriptor.kind().clone(),
    )?;
    let declarations =
        Rc::new(Context::new(registry, Default::default(), Default::default())?.freeze()?);
    let input = crate::conversion_testing::shape(&declarations, &input_descriptor, 2)?;
    let output = crate::conversion_testing::shape(&declarations, &output_descriptor, 2)?;
    let operation = crate::OperationDraft::map_elementwise(input, output, Elementwise::Reciprocal)?
        .check(ExecutionLimits::default())?;
    assert_eq!(operation.outputs()[0].descriptor(), &output_descriptor);
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut symbolic = Budget::new(Limits::default())?;
    let axis = heap.create_cell_axis(2, &mut symbolic)?;
    let input = BindingSymbols::declare(&input_descriptor, [2, 1], [false; 2], &mut symbolic)?;
    let output = BindingSymbols::declare(&output_descriptor, [2, 1], [true; 2], &mut symbolic)?;
    let mut budget = Budget::new(Limits::default())?;
    budget.work(3, crate::loan::AT)?;
    let mut account = ExecutionAccount::new(budget)?;
    let input = HostTileDeclaration::check(input, axis, Coverage::RepresentedInputs, &mut account)?;
    let destination =
        HostTileDeclaration::check(output, axis, Coverage::RepresentedInputs, &mut account)?;
    let mut source = match heap.request_loan(input, &mut account) {
        LoanRequest::Granted(source) => source,
        other @ LoanRequest::Refused(_) => return Err(format!("{other:?}").into()),
    };
    for (cell, value) in [2.0, -2.0].into_iter().enumerate() {
        source.write_component(HostChannel::Center, cell, 0, value, &mut account)?;
    }
    let (tile, receipt) = match heap.handback(source, &mut account) {
        HandbackOutcome::Published(tile, receipt) => (tile, receipt),
        other => return Err(format!("{other:?}").into()),
    };
    let layout = receipt
        .filled(HostChannel::Center)
        .ok_or("filled")?
        .native()
        .layout();
    let demand = heap.create_mask(layout, &[0, 1], &mut account.ledger)?;
    Ok(Fixture {
        heap,
        account,
        request: PointwiseRequest {
            operation,
            input: tile.into_pin(),
            destination,
            demand,
            limits: ExecutionLimits::default(),
        },
    })
}
/// Exact reciprocal values do not erase the checked inverse physical descriptor.
#[test]
fn reciprocal_original_minus_one_descriptor_and_readings() -> TestResult {
    let mut setup = physical_fixture()?;
    let expected = descriptor("inverse-length", -1)?;
    let owner = candidate(
        ready(
            setup
                .heap
                .request_pointwise_reciprocal(setup.request, &mut setup.account),
        )?,
        &mut setup.account,
    )?;
    assert_eq!(
        owner.core.host.declaration.symbols.value().descriptor(),
        &expected
    );
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    for (cell, input) in [2.0, -2.0].into_iter().enumerate() {
        let values = [
            reader.read_component(HostChannel::Center, cell, 0, &mut setup.account)?,
            reader.read_component(HostChannel::Total, cell, 0, &mut setup.account)?,
            reader.read_component(HostChannel::Error, cell, 0, &mut setup.account)?,
        ];
        super::oracle::check(input, 0.0, values)?;
    }
    Ok(())
}
/// Individually valid same-shape declarations cannot replace either original descriptor.
#[test]
fn reciprocal_wrong_original_descriptors_refuse_before_allocation() -> TestResult {
    for output in [false, true] {
        let mut setup = physical_fixture()?;
        if output {
            setup.request.destination.symbols = BindingSymbols::declare(
                &Descriptor::ratio(),
                [2, 1],
                [true; 2],
                &mut Budget::new(Limits::default())?,
            )?;
        } else {
            let owner = crate::kernel_testing::owner()?;
            let input = crate::kernel_testing::shape(
                &owner,
                &crate::kernel_testing::axis(1)?,
                &crate::kernel_testing::axis(2)?,
            )?;
            setup.request.operation = crate::OperationDraft::map_elementwise(
                input.clone(),
                input,
                Elementwise::Reciprocal,
            )?
            .check(ExecutionLimits::default())?;
        }
        let pointer = Rc::as_ptr(&setup.request.input.record);
        let injection = crate::loan::fault::Injection::after(0);
        crate::loan::fault::reset_reads();
        crate::loan::fault::reset_scalars();
        match setup
            .heap
            .request_pointwise_reciprocal(setup.request, &mut setup.account)
        {
            ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::UnsupportedContract),
                request,
            ) => assert_eq!(Rc::as_ptr(&request.input.record), pointer),
            other => return Err(format!("{other:?}").into()),
        }
        assert_eq!(
            [
                crate::loan::fault::reads(),
                crate::loan::fault::scalars(),
                crate::loan::fault::Injection::attempts()
            ],
            [0; 3]
        );
        drop(injection);
    }
    Ok(())
}
/// Fresh destination symbols and host origin remain mandatory for the closed unary member.
#[test]
fn reciprocal_same_symbols_and_affine_or_copy_recipes_refuse() -> TestResult {
    for choice in 0..3 {
        let mut setup = super::fixture::<f64>(1, 1, 3, 2, false, Limits::default())?;
        match choice {
            0 => {
                let source = crate::loan::application::contract::source(
                    &crate::loan::application::prepared::Request::from(&setup.request),
                )?;
                setup.request.destination.symbols = source.declaration.symbols.clone();
            }
            1 => {
                let (prepared, recipe) = crate::reciprocal_testing::proved()?;
                setup.request.operation = crate::reciprocal_testing::draft(&prepared, &recipe)?
                    .check(ExecutionLimits::default())?;
            }
            2 => {
                setup.request.operation = crate::OperationDraft::copy(vec![
                    setup.request.operation.inputs()[0].shape().clone(),
                ])
                .check(ExecutionLimits::default())?;
            }
            _ => return Err("choice".into()),
        }
        let pointer = Rc::as_ptr(&setup.request.input.record);
        let injection = crate::loan::fault::Injection::after(0);
        match setup
            .heap
            .request_pointwise_reciprocal(setup.request, &mut setup.account)
        {
            ApplicationRequest::ApplicationRefused(ApplicationError::Loan(error), request) => {
                if choice == 0 {
                    assert!(matches!(error, LoanError::TargetMismatch));
                } else {
                    assert!(matches!(error, LoanError::UnsupportedContract));
                }
                assert_eq!(Rc::as_ptr(&request.input.record), pointer);
            }
            other => return Err(format!("{other:?}").into()),
        }
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        drop(injection);
    }
    Ok(())
}
/// Even the private shared adapter cannot admit reciprocal in binary32.
#[test]
fn reciprocal_private_binary32_profile_refuses_before_allocation() -> TestResult {
    let mut setup = super::fixture::<f32>(1, 1, 1, 2, false, Limits::default())?;
    let ticket = setup.request.input.ticket()?;
    let injection = crate::loan::fault::Injection::after(0);
    match crate::loan::application::admission::request(
        &mut setup.heap,
        setup.request,
        Elementwise::Reciprocal,
        &mut setup.account,
    ) {
        ApplicationRequest::ApplicationRefused(
            ApplicationError::Loan(LoanError::UnsupportedContract),
            request,
        ) => assert_eq!(request.input.ticket()?, ticket),
        other => return Err(format!("{other:?}").into()),
    }
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    Ok(())
}
