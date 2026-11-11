// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit registry product and ordered quotient rules bind independently declared physical outputs.

use super::{BinaryElementwise, Fixture, Limits, TestResult, admit, candidate, published, ready};
use crate::{
    Coverage, ExecutionAccount, ExecutionLimits, HandbackOutcome, Heap, HeapLimits, HostChannel,
    HostTileDeclaration, LoanRequest, PointwiseBinaryOperation, PointwiseBinaryRequest,
    RegionContract,
};
use nrz_bindings::{
    BinaryOp, BindingSymbols, Budget, Context, Descriptor, Dimension, ExponentPolicy, Kind,
    Measure, Registry, Role,
};
use std::rc::Rc;

/// Independent named kinds retain their complete dimensions and coherent difference role.
fn descriptor(name: &str, exponents: [i64; 7]) -> TestResult<Descriptor> {
    Ok(Descriptor::new(
        Kind::Named(name.into(), Box::new(Dimension::integers(exponents)?)),
        Measure::coherent(),
        Role::Difference,
    )?)
}
/// Original coherent inductance/capacitance descriptors are derived through the frozen registry.
#[expect(
    clippy::default_trait_access,
    reason = "the frozen context infers its private exact environment"
)]
fn fixture(function: BinaryElementwise) -> TestResult<Fixture> {
    let left = descriptor("inductance", [2, 1, -2, -2, 0, 0, 0])?;
    let right = descriptor("capacitance", [-2, -1, 4, 2, 0, 0, 0])?;
    let output = match function {
        BinaryElementwise::Product => descriptor("time-squared", [0, 0, 2, 0, 0, 0, 0])?,
        BinaryElementwise::Quotient => descriptor("impedance-squared", [4, 2, -6, -4, 0, 0, 0])?,
    };
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_binary(
        match function {
            BinaryElementwise::Product => BinaryOp::Multiply,
            BinaryElementwise::Quotient => BinaryOp::Divide,
        },
        left.kind().clone(),
        right.kind().clone(),
        output.kind().clone(),
    )?;
    let declarations =
        Rc::new(Context::new(registry, Default::default(), Default::default())?.freeze()?);
    let regions = [
        RegionContract::check(crate::conversion_testing::shape(&declarations, &left, 1)?)?,
        RegionContract::check(crate::conversion_testing::shape(&declarations, &right, 1)?)?,
    ];
    let output_region =
        RegionContract::check(crate::conversion_testing::shape(&declarations, &output, 1)?)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = ExecutionAccount::new(Budget::new(Limits::default())?)?;
    let axis = heap.create_cell_axis(1, &mut account.ledger)?;
    let destination_symbols =
        BindingSymbols::declare(&output, [1; 2], [true; 2], &mut account.ledger)?;
    let destination = HostTileDeclaration::check(
        destination_symbols.clone(),
        axis,
        Coverage::RepresentedInputs,
        &mut account,
    )?;
    let mut pins = Vec::new();
    for (quantity, value) in [(left, 2.0), (right, 3.0)] {
        let symbols = BindingSymbols::declare(&quantity, [1; 2], [false; 2], &mut account.ledger)?;
        let declaration =
            HostTileDeclaration::check(symbols, axis, Coverage::RepresentedInputs, &mut account)?;
        let mut loan = match heap.request_loan(declaration, &mut account) {
            LoanRequest::Granted(loan) => loan,
            other @ LoanRequest::Refused(_) => return Err(format!("{other:?}").into()),
        };
        loan.write_component(HostChannel::Center, 0, 0, value, &mut account)?;
        pins.push(match heap.handback(loan, &mut account) {
            HandbackOutcome::Published(tile, _) => tile.into_pin(),
            other => return Err(format!("{other:?}").into()),
        });
    }
    let right = pins.pop().ok_or("right")?;
    let left = pins.pop().ok_or("left")?;
    let demand = heap.create_mask(left.ticket()?.identity.layout, &[0], &mut account.ledger)?;
    let operation = PointwiseBinaryOperation::check(
        regions,
        output_region,
        [&left, &right],
        destination_symbols,
        axis,
        function,
        ExecutionLimits::default(),
        &mut account,
    )?;
    Ok(Fixture {
        heap,
        account,
        request: PointwiseBinaryRequest {
            operation,
            left,
            right,
            destination,
            demand,
            limits: ExecutionLimits::default(),
        },
    })
}
/// The result's physical descriptor and exact readings survive both source retirements.
#[test]
fn binary_physical_product_and_ordered_quotient() -> TestResult {
    for function in [BinaryElementwise::Product, BinaryElementwise::Quotient] {
        let mut setup = fixture(function)?;
        let expected = setup
            .request
            .destination
            .symbols
            .value()
            .descriptor()
            .clone();
        let owner = candidate(
            ready(admit(
                &mut setup.heap,
                setup.request,
                function,
                &mut setup.account,
            ))?,
            &mut setup.account,
        )?;
        assert_eq!(
            owner.core.host.declaration.symbols.value().descriptor(),
            &expected
        );
        let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
        let reader = tile.reader(&setup.heap, &mut setup.account)?;
        super::oracle::check(
            [2.0, 3.0],
            [0.0; 2],
            function,
            super::values::readings(&reader, 0, 0, &mut setup.account)?,
        )?;
    }
    Ok(())
}
/// A complete descriptor or snapshot mismatch cannot be repaired by matching dimensions.
#[test]
fn binary_wrong_physical_descriptors_and_independent_snapshots_refuse() -> TestResult {
    for choice in 0..4 {
        let mut setup = fixture(BinaryElementwise::Product)?;
        let operation = &setup.request.operation.operation;
        let mut inputs = [operation.inputs()[0].clone(), operation.inputs()[1].clone()];
        let mut output = operation.outputs()[0].clone();
        let mut destination = setup.request.destination.symbols.clone();
        if choice < 3 {
            let independent = crate::kernel_testing::owner()?;
            let quantity = if choice == 2 {
                output.descriptor()
            } else {
                inputs[choice].descriptor()
            };
            let region = RegionContract::check(crate::conversion_testing::shape(
                &independent,
                quantity,
                1,
            )?)?;
            if choice == 2 {
                output = region;
            } else {
                inputs[choice] = region;
            }
        } else {
            destination = BindingSymbols::declare(
                &Descriptor::ratio(),
                [1; 2],
                [true; 2],
                &mut setup.account.ledger,
            )?;
        }
        let injection = crate::loan::fault::Injection::after(0);
        assert!(
            PointwiseBinaryOperation::check(
                inputs,
                output,
                [&setup.request.left, &setup.request.right],
                destination,
                setup.request.destination.axis,
                BinaryElementwise::Product,
                ExecutionLimits::default(),
                &mut setup.account
            )
            .is_err()
        );
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        drop(injection);
    }
    Ok(())
}
