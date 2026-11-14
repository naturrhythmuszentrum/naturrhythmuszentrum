// SPDX-License-Identifier: AGPL-3.0-only
//! Two independently published hosts establish literal table geometry and logical availability.

use super::{BinaryElementwise, Fixture, Limits, TestResult};
use crate::{
    Coverage, ExecutionAccount, ExecutionLimits, HandbackOutcome, Heap, HeapLimits, HostChannel,
    HostTileDeclaration, LoanRequest, PointwiseBinaryOperation, PointwiseBinaryRequest,
    RegionContract,
};
use nrz_bindings::{BindingSymbols, Budget, Descriptor};

/// Default well-separated values permit either function in every available cell.
pub(super) fn fixture(
    cells: usize,
    components: usize,
    roles: [usize; 2],
    selection: usize,
    holes: bool,
    function: BinaryElementwise,
    limits: Limits,
) -> TestResult<Fixture> {
    custom(
        cells,
        components,
        roles,
        selection,
        holes,
        function,
        limits,
        |operand, cell, component| {
            let sign = if (cell + component + operand) % 2 == 0 {
                1.0
            } else {
                -1.0
            };
            (sign * if operand == 0 { 2.0 } else { 3.0 }, 0.25)
        },
    )
}
/// Shape and declaration constructors use the original account before the measured admission.
#[expect(
    clippy::too_many_arguments,
    reason = "the exact fixture controls geometry, limits and payload independently"
)]
pub(super) fn custom<P: nrz_scalar::Scalar + 'static>(
    cells: usize,
    components: usize,
    roles: [usize; 2],
    selection: usize,
    holes: bool,
    function: BinaryElementwise,
    limits: Limits,
    values: impl Fn(usize, usize, usize) -> (f64, f64),
) -> TestResult<Fixture<P>> {
    with_heap(
        cells,
        components,
        roles,
        selection,
        holes,
        function,
        limits,
        Heap::new(HeapLimits::default())?,
        values,
    )
}

/// The permanent heap ceiling is chosen before any host is created.
#[expect(
    clippy::too_many_arguments,
    reason = "fixture geometry and permanent resource policy are independent"
)]
pub(super) fn with_heap<P: nrz_scalar::Scalar + 'static>(
    cells: usize,
    components: usize,
    roles: [usize; 2],
    selection: usize,
    holes: bool,
    function: BinaryElementwise,
    limits: Limits,
    mut heap: Heap,
    values: impl Fn(usize, usize, usize) -> (f64, f64),
) -> TestResult<Fixture<P>> {
    let mut account = ExecutionAccount::new(Budget::new(limits)?)?;
    account.ledger.work(3, crate::loan::AT)?;
    let shape = [u32::try_from(cells)?, u32::try_from(components)?];
    let axis = heap.create_cell_axis(cells, &mut account.ledger)?;
    let left = BindingSymbols::declare(
        &Descriptor::ratio(),
        shape,
        [roles[0] > 1, roles[0] > 2],
        &mut account.ledger,
    )?;
    let right = BindingSymbols::declare(
        &Descriptor::ratio(),
        shape,
        [roles[1] > 1, roles[1] > 2],
        &mut account.ledger,
    )?;
    let output =
        BindingSymbols::declare(&Descriptor::ratio(), shape, [true; 2], &mut account.ledger)?;
    let destination = HostTileDeclaration::check(
        output.clone(),
        axis,
        Coverage::RepresentedInputs,
        &mut account,
    )?;
    let mut pins = Vec::new();
    for (operand, symbols) in [left, right].into_iter().enumerate() {
        let declaration =
            HostTileDeclaration::check(symbols, axis, Coverage::RepresentedInputs, &mut account)?;
        let mut loan = match heap.request_loan(declaration, &mut account) {
            LoanRequest::Granted(loan) => loan,
            other @ LoanRequest::Refused(_) => return Err(format!("{other:?}").into()),
        };
        for cell in 0..cells {
            for component in 0..components {
                let (center, radius) = values(operand, cell, component);
                for (role, value) in [center, radius, radius / 2.0]
                    .into_iter()
                    .enumerate()
                    .take(roles[operand])
                {
                    if !missing(cell, component, components, operand, role, holes) {
                        loan.write_component(
                            HostChannel::ALL[role],
                            cell,
                            component,
                            super::native::<P>(value),
                            &mut account,
                        )?;
                    }
                }
            }
        }
        let tile = match heap.handback(loan, &mut account) {
            HandbackOutcome::Published(tile, _) => tile,
            other => return Err(format!("{other:?}").into()),
        };
        pins.push(tile.into_pin());
    }
    let right = pins.pop().ok_or("right pin")?;
    let left = pins.pop().ok_or("left pin")?;
    let selected = (0..cells)
        .filter(|cell| super::requested(*cell, selection))
        .collect::<Vec<_>>();
    let demand = heap.create_mask(
        left.ticket()?.identity.layout,
        &selected,
        &mut account.ledger,
    )?;
    let declarations = crate::kernel_testing::owner()?;
    let domain = crate::kernel_testing::axis(components)?;
    let codomain = crate::kernel_testing::axis(cells)?;
    let region = RegionContract::check(crate::kernel_testing::shape(
        &declarations,
        &domain,
        &codomain,
    )?)?;
    let operation = PointwiseBinaryOperation::check(
        [region.clone(), region.clone()],
        region,
        [&left, &right],
        output,
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
/// The two sides have shifted independent holes; error-only holes still remove the whole cell.
const fn missing(
    cell: usize,
    component: usize,
    components: usize,
    operand: usize,
    role: usize,
    holes: bool,
) -> bool {
    let shifted = cell + operand;
    holes
        && match role {
            0 => {
                shifted % 11 == 3
                    || (components > 1 && shifted % 13 == 4 && component + 1 == components)
            }
            1 => shifted % 5 == 1,
            2 => shifted % 7 == 2,
            _ => false,
        }
}
/// Independent set formula uses no production mask, reader or planner.
pub(super) const fn available(
    cell: usize,
    components: usize,
    roles: [usize; 2],
    holes: bool,
) -> bool {
    let left = present(cell, components, roles[0], holes);
    let right = present(cell + 1, components, roles[1], holes);
    left && right
}
/// Each role is whole-cell closed even when only one component was omitted.
const fn present(cell: usize, components: usize, roles: usize, holes: bool) -> bool {
    !holes
        || (cell % 11 != 3
            && (components < 2 || cell % 13 != 4)
            && (roles < 2 || cell % 5 != 1)
            && (roles < 3 || cell % 7 != 2))
}
