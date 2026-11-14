// SPDX-License-Identifier: AGPL-3.0-only
//! A distinct one-cell right operand is a shared uncertain scalar, not copied independent inputs.

use super::{
    BinaryElementwise, Fixture, Limits, TestResult, admit, candidate, oracle, published, ready,
};
use crate::{
    Coverage, ExecutionLimits, HostChannel, HostTileDeclaration, PointwiseBinaryOperation,
    RegionContract,
};

/// Replace the ordinary right fixture with an independently declared scalar of configurable shape.
fn setup(
    cells: usize,
    selection: usize,
    right_shape: [usize; 2],
    missing: bool,
    function: BinaryElementwise,
    scalar: [f64; 2],
) -> TestResult<Fixture> {
    let mut setup = super::fixture::custom(
        cells,
        2,
        [3; 2],
        selection,
        false,
        function,
        Limits::default(),
        |operand, cell, component| {
            let sign = if (cell + component + operand) % 2 == 0 {
                1.0
            } else {
                -1.0
            };
            let radius = 0.25
                + f64::from(u32::try_from(cell).expect("fixture cell")) / 1024.0
                + f64::from(u32::try_from(component).expect("fixture component")) / 256.0;
            (sign * if operand == 0 { 2.0 } else { 3.0 }, radius)
        },
    )?;
    let axis = setup
        .heap
        .create_cell_axis(right_shape[0], &mut setup.account.ledger)?;
    let symbols = nrz_bindings::BindingSymbols::declare(
        &nrz_bindings::Descriptor::ratio(),
        [
            u32::try_from(right_shape[0])?,
            u32::try_from(right_shape[1])?,
        ],
        [true; 2],
        &mut setup.account.ledger,
    )?;
    let declaration = HostTileDeclaration::check(
        symbols,
        axis,
        Coverage::RepresentedInputs,
        &mut setup.account,
    )?;
    let mut loan = match setup.heap.request_loan(declaration, &mut setup.account) {
        crate::LoanRequest::Granted(loan) => loan,
        crate::LoanRequest::Refused(error) => return Err(error.into()),
    };
    for cell in 0..right_shape[0] {
        for component in 0..right_shape[1] {
            for (channel, value) in [
                (HostChannel::Center, scalar[0]),
                (HostChannel::Total, scalar[1]),
                (HostChannel::Error, 0.0),
            ] {
                if channel != HostChannel::Error || !missing {
                    loan.write_component(channel, cell, component, value, &mut setup.account)?;
                }
            }
        }
    }
    let tile = match setup.heap.handback(loan, &mut setup.account) {
        crate::HandbackOutcome::Published(tile, _) => tile,
        other => return Err(format!("{other:?}").into()),
    };
    setup.request.right = tile.into_pin();
    Ok(setup)
}

/// Both original regions retain one frozen declaration snapshot and their own extents.
fn operation(
    setup: &mut Fixture,
    function: BinaryElementwise,
) -> TestResult<PointwiseBinaryOperation> {
    let original = &setup.request.operation.operation;
    let left = original.inputs()[0].clone();
    let output = original.outputs()[0].clone();
    let extent = super::super::super::binary::bundle(&setup.request.right)?
        .declaration
        .extent;
    let right = RegionContract::check(crate::kernel_testing::shape(
        &output.shape().declarations,
        &crate::kernel_testing::axis(extent.columns())?,
        &crate::kernel_testing::axis(extent.rows())?,
    )?)?;
    Ok(PointwiseBinaryOperation::check(
        [left, right],
        output,
        [&setup.request.left, &setup.request.right],
        setup.request.destination.symbols.clone(),
        setup.request.destination.axis,
        function,
        ExecutionLimits::default(),
        &mut setup.account,
    )?)
}

/// Sparse, empty and full demand across word boundaries repeat exact scalar-box extrema.
/// ´test:unit:loan-binary-right-tile-scalar´
#[test]
fn loan_binary_right_tile_scalar() -> TestResult {
    for function in [BinaryElementwise::Product, BinaryElementwise::Quotient] {
        for cells in [0, 1, 65] {
            for selection in 0..3 {
                for missing in [false, true] {
                    values(cells, selection, missing, function)?;
                }
            }
        }
    }
    Ok(())
}

/// The error-only hole on the scalar suppresses every output without observing either input.
fn values(
    cells: usize,
    selection: usize,
    missing: bool,
    function: BinaryElementwise,
) -> TestResult {
    let mut setup = setup(cells, selection, [1, 1], missing, function, [3.0, 0.5])?;
    setup.request.operation = operation(&mut setup, function)?;
    let demand = setup.request.demand;
    let outside = setup
        .heap
        .retain_pin(&setup.request.right, &mut setup.account)?;
    crate::loan::fault::reset_reads();
    let owner = ready(admit(
        &mut setup.heap,
        setup.request,
        function,
        &mut setup.account,
    ))?;
    assert_eq!(crate::loan::fault::reads(), 0);
    let owner = candidate(owner, &mut setup.account)?;
    let count = (0..cells)
        .filter(|cell| super::requested(*cell, selection) && !missing)
        .count();
    assert_eq!(crate::loan::fault::reads(), count * 2 * 4);
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    for cell in 0..cells {
        let available = super::requested(cell, selection) && !missing;
        assert_eq!(reader.contains(cell, &mut setup.account)?, available);
        if available {
            for component in 0..2 {
                let left = if (cell + component) % 2 == 0 {
                    2.0
                } else {
                    -2.0
                };
                let readings =
                    super::values::readings(&reader, cell, component, &mut setup.account)?;
                let total = 0.25
                    + f64::from(u32::try_from(cell)?) / 1024.0
                    + f64::from(u32::try_from(component)?) / 256.0;
                oracle::tight([left, 3.0], [total, 0.5], function, readings)?;
            }
        }
    }
    drop(tile);
    setup.heap.retire_mask(demand, &mut setup.account.ledger)?;
    assert!(matches!(
        setup.heap.residence(&mut setup.account, 100_000)?,
        crate::Residence::Pending { .. }
    ));
    let reader = outside.historical_reader(&mut setup.account)?;
    assert_eq!(reader.contains(0, &mut setup.account)?, !missing);
    if !missing {
        assert_eq!(
            reader.read_component(HostChannel::Center, 0, 0, &mut setup.account)?,
            3.0
        );
    }
    drop(outside);
    assert!(matches!(
        setup.heap.residence(&mut setup.account, 100_000)?,
        crate::Residence::Empty { .. }
    ));
    assert_eq!(setup.heap.loans.elements()?, [0; 2]);
    assert!(setup.heap.loans.debits.iter().all(Option::is_none));
    Ok(())
}

/// Arbitrary right rectangles are refused explicitly before any numerical read or allocation.
/// ´test:unit:loan-binary-unsupported-extent´
#[test]
fn loan_binary_unsupported_extent() -> TestResult {
    for shape in [[1, 2], [3, 1], [2, 2]] {
        let mut setup = setup(3, 2, shape, false, BinaryElementwise::Product, [3.0, 0.5])?;
        let tickets = [setup.request.left.ticket()?, setup.request.right.ticket()?];
        crate::loan::fault::reset_reads();
        let injection = crate::loan::fault::Injection::after(0);
        let error =
            operation(&mut setup, BinaryElementwise::Product).expect_err("unsupported rectangle");
        assert!(
            error
                .to_string()
                .contains("unsupported binary operand extent")
        );
        assert_eq!(crate::loan::fault::reads(), 0);
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        assert_eq!(
            [setup.request.left.ticket()?, setup.request.right.ticket()?],
            tickets
        );
        drop(injection);
    }
    Ok(())
}

/// Two source visits, two word visits, and two cells times (two components times
/// (21 invocation + four loads + three stores + two projections), plus two cell visits) give 128.
/// ´test:unit:loan-binary-right-scalar-work-boundary´
#[test]
fn loan_binary_right_scalar_work_boundary() -> TestResult {
    for available in [127, 128] {
        let function = BinaryElementwise::Product;
        let mut setup = setup(3, 1, [1, 1], false, function, [3.0, 0.5])?;
        setup.request.operation = operation(&mut setup, function)?;
        let owner = ready(admit(
            &mut setup.heap,
            setup.request,
            function,
            &mut setup.account,
        ))?;
        super::leave(&mut setup.account, available)?;
        let before = setup.account.usage().work;
        crate::loan::fault::reset_reads();
        let result = owner.evaluate(&mut setup.account);
        if available == 128 {
            assert!(matches!(
                result,
                crate::ApplicationEvaluation::ApplicationCandidate(_)
            ));
            assert_eq!(crate::loan::fault::reads(), 16);
            assert_eq!(setup.account.usage().work - before, 128);
        } else {
            assert!(matches!(
                result,
                crate::ApplicationEvaluation::ApplicationNotStarted(
                    crate::ApplicationError::Loan(crate::LoanError::Budget(_)),
                    _
                )
            ));
            assert_eq!(crate::loan::fault::reads(), 0);
        }
    }
    Ok(())
}

/// A zero-crossing shared divisor refuses each demanded cell at its target entry.
/// ´test:unit:loan-binary-right-scalar-domain´
#[test]
fn loan_binary_right_scalar_domain() -> TestResult {
    let function = BinaryElementwise::Quotient;
    let mut setup = setup(3, 1, [1, 1], false, function, [0.0, 1.0])?;
    setup.request.operation = operation(&mut setup, function)?;
    let owner = ready(admit(
        &mut setup.heap,
        setup.request,
        function,
        &mut setup.account,
    ))?;
    let owner = candidate(owner, &mut setup.account)?;
    for cell in [0, 2] {
        assert!(
            matches!(owner.core.evidence.outcomes[cell], crate::CellOutcome::CellRefused {
            component: 0,
            reason: crate::KernelError::BinaryDomain {
                function: BinaryElementwise::Quotient,
                entry,
                reason: crate::DomainReason::ZeroInDivisor,
            },
        } if entry == cell)
        );
    }
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    for cell in 0..3 {
        assert!(!reader.contains(cell, &mut setup.account)?);
    }
    Ok(())
}

/// Allocation refusal and destination substitution retain both pins for a corrected retry.
/// ´test:unit:loan-binary-right-scalar-request-refusal´
#[test]
fn loan_binary_right_scalar_request_refusal() -> TestResult {
    for allocation in [false, true] {
        let function = BinaryElementwise::Product;
        let mut setup = setup(3, 1, [1, 1], false, function, [3.0, 0.5])?;
        setup.request.operation = operation(&mut setup, function)?;
        let original_axis = setup.request.destination.axis;
        let tickets = [setup.request.left.ticket()?, setup.request.right.ticket()?];
        if !allocation {
            setup.request.destination.axis =
                setup.heap.create_cell_axis(3, &mut setup.account.ledger)?;
        }
        crate::loan::fault::reset_reads();
        let injection = crate::loan::fault::Injection::after(0);
        let result = admit(&mut setup.heap, setup.request, function, &mut setup.account);
        let mut request = match result {
            crate::BinaryApplicationRequest::ApplicationRefused(
                crate::ApplicationError::Loan(error),
                request,
            ) => {
                if allocation {
                    assert!(matches!(error, crate::LoanError::AllocationRefused));
                } else {
                    // The operation authenticates the captured destination axis identity.
                    assert!(matches!(error, crate::LoanError::LayoutMismatch));
                    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                }
                request
            }
            other => return Err(format!("{other:?}").into()),
        };
        assert_eq!(crate::loan::fault::reads(), 0);
        assert_eq!([request.left.ticket()?, request.right.ticket()?], tickets);
        drop(injection);
        request.destination.axis = original_axis;
        let owner = ready(admit(
            &mut setup.heap,
            request,
            function,
            &mut setup.account,
        ))?;
        let owner = candidate(owner, &mut setup.account)?;
        let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
        assert!(
            tile.reader(&setup.heap, &mut setup.account)?
                .contains(2, &mut setup.account)?
        );
    }
    Ok(())
}
