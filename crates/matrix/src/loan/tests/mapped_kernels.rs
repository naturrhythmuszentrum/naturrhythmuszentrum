// SPDX-License-Identifier: AGPL-3.0-only
//! Existing unary laws consume physical permutations through their prepaid selected reader.

use super::{TestResult, empty, fixture, granted};
use crate::{
    ApplicationEvaluation, ApplicationPublication, ApplicationRequest, Elementwise,
    ExecutionLimits, HandbackOutcome, HostChannel, HostTileDeclaration, PointwiseRequest,
};
use nrz_bindings::{BindingSymbols, Limits};
use nrz_scalar::Scalar;

/// Square in both formats and binary64 root/reciprocal preserve sparse original selections.
/// ´test:unit:loan-dense-layout-unary-kernels´
#[test]
fn loan_dense_layout_unary_kernels() -> TestResult {
    unary::<f32>(Elementwise::Square, crate::Heap::request_pointwise_square)?;
    unary::<f64>(Elementwise::Square, crate::Heap::request_pointwise_square)?;
    unary::<f64>(
        Elementwise::SquareRoot,
        crate::Heap::request_pointwise_square_root,
    )?;
    unary::<f64>(
        Elementwise::Reciprocal,
        crate::Heap::request_pointwise_reciprocal,
    )
}

/// Literal rational controls distinguish the two interleaved components and each source cell.
fn unary<P: Scalar + 'static>(
    function: Elementwise,
    admit: fn(
        &mut crate::Heap,
        PointwiseRequest<P>,
        &mut crate::ExecutionAccount,
    ) -> ApplicationRequest<P>,
) -> TestResult {
    let (mut heap, mut account, input) = fixture::<P>(3, 2, [true, true], Limits::default())?;
    let output = BindingSymbols::declare(
        input.symbols().value().descriptor(),
        [3, 2],
        [true, true],
        &mut account.ledger,
    )?;
    let destination =
        HostTileDeclaration::check(output, input.axis, input.coverage(), &mut account)?;
    let layout = crate::Layout::strided(crate::Extent::new(3, 2)?, 9, 6, -2, -1)?;
    let mut loan = granted(heap.request_loan_with_layout(input, layout, &mut account))?;
    for cell in [0, 2] {
        let value = if cell == 0 {
            P::from_u8(4)
        } else {
            P::from_u8(16)
        };
        loan.write_cell(HostChannel::Center, cell, &[value, P::ONE], &mut account)?;
        loan.write_cell(HostChannel::Total, cell, &[P::ZERO, P::ZERO], &mut account)?;
        loan.write_cell(HostChannel::Error, cell, &[P::ZERO, P::ZERO], &mut account)?;
    }
    let (tile, receipt) = match heap.handback(loan, &mut account) {
        HandbackOutcome::Published(tile, receipt) => (tile, receipt),
        other => return Err(format!("{other:?}").into()),
    };
    let demand = receipt
        .filled(HostChannel::Center)
        .ok_or("filled")?
        .native();
    let request = PointwiseRequest {
        operation: operation(function)?,
        input: tile.into_pin(),
        destination,
        demand,
        limits: ExecutionLimits::default(),
    };
    let owner = match admit(&mut heap, request, &mut account) {
        ApplicationRequest::ApplicationReady(owner) => owner,
        other @ ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    crate::loan::fault::reset_reads();
    let owner = match owner.evaluate(&mut account) {
        ApplicationEvaluation::ApplicationCandidate(owner) => owner,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(crate::loan::fault::reads(), 8);
    let tile = match heap.publish_application(owner, &mut account) {
        ApplicationPublication::ApplicationPublished(tile, _) => tile,
        other => return Err(format!("{other:?}").into()),
    };
    check_values(&tile, &heap, &mut account, function)?;
    drop(tile);
    empty(&mut heap, &mut account)
}

/// Original checked descriptors are prepared once for the entire logical tile.
fn operation(function: Elementwise) -> TestResult<crate::CheckedOperation> {
    let owner = crate::kernel_testing::owner()?;
    let domain = crate::kernel_testing::axis(2)?;
    let codomain = crate::kernel_testing::axis(3)?;
    let input = crate::kernel_testing::shape(&owner, &domain, &codomain)?;
    let output = crate::kernel_testing::shape(&owner, &domain, &codomain)?;
    Ok(
        crate::OperationDraft::map_elementwise(input, output, function)?
            .check(ExecutionLimits::default())?,
    )
}

/// Exact rational controls require no call to the numerical kernel under test.
fn check_values<P: Scalar + 'static>(
    tile: &crate::PublishedTile<P>,
    heap: &crate::Heap,
    account: &mut crate::ExecutionAccount,
    function: Elementwise,
) -> TestResult {
    let reader = tile.reader(heap, account)?;
    assert!(!reader.contains(1, account)?);
    for cell in [0, 2] {
        let expected = match function {
            Elementwise::Square => {
                if cell == 0 {
                    16.0
                } else {
                    256.0
                }
            }
            Elementwise::SquareRoot => {
                if cell == 0 {
                    2.0
                } else {
                    4.0
                }
            }
            Elementwise::Reciprocal => {
                if cell == 0 {
                    0.25
                } else {
                    0.0625
                }
            }
        };
        assert_eq!(
            reader
                .read_component(HostChannel::Center, cell, 0, account)?
                .to_f64(),
            expected
        );
        assert_eq!(
            reader.read_component(HostChannel::Center, cell, 1, account)?,
            P::ONE
        );
    }
    Ok(())
}
