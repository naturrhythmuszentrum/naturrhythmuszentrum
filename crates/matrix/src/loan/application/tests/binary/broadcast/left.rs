// SPDX-License-Identifier: AGPL-3.0-only
//! Left-scalar incidence anchors demand on the right output rectangle without commuting quotient.

use super::{
    BinaryElementwise, Fixture, TestResult, admit, candidate, operation, published, ready,
};
use crate::{ApplicationError, BinaryApplicationRequest, HostChannel, LoanError};

/// Swap the original host pins before correspondence construction, retaining table-qualified demand.
fn setup(
    cells: usize,
    selection: usize,
    missing: bool,
    function: BinaryElementwise,
) -> TestResult<Fixture> {
    let mut setup = super::setup(cells, selection, [1, 1], missing, function, [3.0, 0.5])?;
    std::mem::swap(&mut setup.request.left, &mut setup.request.right);
    setup.request.operation = operation(&mut setup, function)?;
    Ok(setup)
}

/// Both ordered laws enclose independent scalar/table endpoint images across sparse multiword demand.
/// ´test:unit:loan-binary-left-tile-scalar´
#[test]
fn loan_binary_left_tile_scalar() -> TestResult {
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

/// The shared scalar's error-only hole removes all targets while preserving outside readers.
fn values(
    cells: usize,
    selection: usize,
    missing: bool,
    function: BinaryElementwise,
) -> TestResult {
    let mut setup = setup(cells, selection, missing, function)?;
    let demand = setup.request.demand;
    let outside = setup
        .heap
        .retain_pin(&setup.request.left, &mut setup.account)?;
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
        .filter(|cell| super::super::requested(*cell, selection) && !missing)
        .count();
    assert_eq!(crate::loan::fault::reads(), count * 8);
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    for cell in 0..cells {
        let available = super::super::requested(cell, selection) && !missing;
        assert_eq!(reader.contains(cell, &mut setup.account)?, available);
        if available {
            for component in 0..2 {
                let right = if (cell + component) % 2 == 0 {
                    2.0
                } else {
                    -2.0
                };
                let readings =
                    super::super::values::readings(&reader, cell, component, &mut setup.account)?;
                let total = 0.25
                    + f64::from(u32::try_from(cell)?) / 1024.0
                    + f64::from(u32::try_from(component)?) / 256.0;
                super::oracle::tight([3.0, right], [0.5, total], function, readings)?;
            }
        }
    }
    drop(tile);
    setup.heap.retire_mask(demand, &mut setup.account.ledger)?;
    assert!(matches!(
        setup.heap.residence(&mut setup.account, 100_000)?,
        crate::Residence::Pending { .. }
    ));
    assert_eq!(
        outside
            .historical_reader(&mut setup.account)?
            .contains(0, &mut setup.account)?,
        !missing
    );
    drop(outside);
    assert!(matches!(
        setup.heap.residence(&mut setup.account, 100_000)?,
        crate::Residence::Empty { .. }
    ));
    assert_eq!(setup.heap.loans.elements()?, [0; 2]);
    Ok(())
}

/// A mask on the scalar layout cannot replace demand on the output-sized right operand.
/// ´test:unit:loan-binary-left-scalar-demand-refusal´
#[test]
fn loan_binary_left_scalar_demand_refusal() -> TestResult {
    let function = BinaryElementwise::Quotient;
    let mut setup = setup(3, 1, false, function)?;
    let original = setup.request.demand;
    let wrong = setup.heap.create_mask(
        setup.request.left.ticket()?.identity.layout,
        &[0],
        &mut setup.account.ledger,
    )?;
    setup.request.demand = wrong;
    let tickets = [setup.request.left.ticket()?, setup.request.right.ticket()?];
    crate::loan::fault::reset_reads();
    let injection = crate::loan::fault::Injection::after(0);
    let mut request = match admit(&mut setup.heap, setup.request, function, &mut setup.account) {
        BinaryApplicationRequest::ApplicationRefused(
            ApplicationError::Loan(LoanError::LayoutMismatch),
            request,
        ) => request,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(crate::loan::fault::reads(), 0);
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    assert_eq!([request.left.ticket()?, request.right.ticket()?], tickets);
    assert_eq!(request.demand, wrong);
    drop(injection);
    request.demand = original;
    let owner = ready(admit(
        &mut setup.heap,
        request,
        function,
        &mut setup.account,
    ))?;
    let owner = candidate(owner, &mut setup.account)?;
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    assert_eq!(
        tile.reader(&setup.heap, &mut setup.account)?
            .read_component(HostChannel::Center, 2, 0, &mut setup.account)?,
        1.5
    );
    Ok(())
}

/// A refused component names the output rectangle's entry even when Left has only one entry.
/// ´test:unit:loan-binary-left-scalar-domain-entry´
#[test]
fn loan_binary_left_scalar_domain_entry() -> TestResult {
    let function = BinaryElementwise::Quotient;
    let mut setup = setup(3, 1, false, function)?;
    let symbols = nrz_bindings::BindingSymbols::declare(
        &nrz_bindings::Descriptor::ratio(),
        [3, 2],
        [true; 2],
        &mut setup.account.ledger,
    )?;
    let declaration = crate::HostTileDeclaration::check(
        symbols,
        setup.request.destination.axis,
        crate::Coverage::RepresentedInputs,
        &mut setup.account,
    )?;
    let mut loan = match setup.heap.request_loan(declaration, &mut setup.account) {
        crate::LoanRequest::Granted(loan) => loan,
        crate::LoanRequest::Refused(error) => return Err(error.into()),
    };
    for cell in 0..3 {
        let centers = if cell == 2 { [2.0, 0.0] } else { [2.0; 2] };
        loan.write_cell(HostChannel::Center, cell, &centers, &mut setup.account)?;
        loan.write_cell(HostChannel::Total, cell, &[0.5; 2], &mut setup.account)?;
        loan.write_cell(HostChannel::Error, cell, &[0.0; 2], &mut setup.account)?;
    }
    let table = match setup.heap.handback(loan, &mut setup.account) {
        crate::HandbackOutcome::Published(tile, _) => tile,
        other => return Err(format!("{other:?}").into()),
    };
    setup.request.demand = setup.heap.create_mask(
        table.pin().ticket()?.identity.layout,
        &[0, 2],
        &mut setup.account.ledger,
    )?;
    setup.request.right = table.into_pin();
    setup.request.operation = operation(&mut setup, function)?;
    let owner = ready(admit(
        &mut setup.heap,
        setup.request,
        function,
        &mut setup.account,
    ))?;
    let owner = candidate(owner, &mut setup.account)?;
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    assert!(reader.contains(0, &mut setup.account)?);
    assert!(!reader.contains(2, &mut setup.account)?);
    assert!(matches!(
        reader.application_outcome(2, &mut setup.account)?,
        crate::CellOutcome::CellRefused {
            component: 1,
            reason: crate::KernelError::BinaryDomain {
                function: BinaryElementwise::Quotient,
                entry: 5,
                reason: crate::DomainReason::ZeroInDivisor,
            },
        }
    ));
    Ok(())
}

/// The same two-source, two-word and two-cell inventory gives 128 units in either orientation.
/// ´test:unit:loan-binary-left-scalar-work-boundary´
#[test]
fn loan_binary_left_scalar_work_boundary() -> TestResult {
    for available in [127, 128] {
        let function = BinaryElementwise::Quotient;
        let mut setup = setup(3, 1, false, function)?;
        let owner = ready(admit(
            &mut setup.heap,
            setup.request,
            function,
            &mut setup.account,
        ))?;
        super::super::leave(&mut setup.account, available)?;
        crate::loan::fault::reset_reads();
        let result = owner.evaluate(&mut setup.account);
        if available == 128 {
            assert!(matches!(
                result,
                crate::ApplicationEvaluation::ApplicationCandidate(_)
            ));
            assert_eq!(crate::loan::fault::reads(), 16);
        } else {
            assert!(matches!(
                result,
                crate::ApplicationEvaluation::ApplicationNotStarted(
                    ApplicationError::Loan(LoanError::Budget(_)),
                    _
                )
            ));
            assert_eq!(crate::loan::fault::reads(), 0);
        }
    }
    Ok(())
}
