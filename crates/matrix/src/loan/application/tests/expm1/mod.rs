// SPDX-License-Identifier: AGPL-3.0-only
//! Prepared application controls inspect published values and actual selected accesses.

mod authority;
mod bytes;
mod correspondence;
mod gating;
mod lifecycle;
mod mask_origin;
mod metadata;
mod resources;
mod seals;
mod shared;
mod structure;
mod unsupported;
mod work;

use super::{TestResult, candidate, published};
use crate::loan::retained_expm1::tests as laws;
use crate::{
    CellOutcome, ExecutionAccount, ExecutionLimits, Expm1ApplicationRequest as ApplicationRequest,
    Heap, HostChannel, HostTileDeclaration, PointwiseExpm1Request, PreparedExpm1Operation,
};
use nrz_bindings::{BindingSymbols, Budget, Descriptor};

/// Real source, prepared authority and operation all share their original continuation account.
pub(in crate::loan::application) struct Fixture {
    pub(in crate::loan::application) heap: Heap,
    pub(in crate::loan::application) account: ExecutionAccount,
    pub(in crate::loan::application) request: PointwiseExpm1Request,
}
/// Independently declared source values, optional holes, and exact represented-domain admission.
pub(in crate::loan::application) fn fixture(
    cells: usize,
    components: usize,
    domain: [f64; 2],
    selection: usize,
    holes: bool,
) -> TestResult<Fixture> {
    fixture_limits(
        cells,
        components,
        domain,
        selection,
        holes,
        crate::HeapLimits::default(),
    )
}
/// Aggregate element ceilings are fixed before source or law setup.
fn fixture_limits(
    cells: usize,
    components: usize,
    domain: [f64; 2],
    selection: usize,
    holes: bool,
    limits: crate::HeapLimits,
) -> TestResult<Fixture> {
    let mut heap = Heap::new(limits)?;
    let mut account = ExecutionAccount::new(Budget::new(laws::limits())?)?;
    let mut symbolic = Budget::new(nrz_bindings::Limits::default())?;
    let shape = [u32::try_from(cells)?, u32::try_from(components)?];
    let source = BindingSymbols::declare(&Descriptor::ratio(), shape, [false; 2], &mut symbolic)?;
    let destination =
        BindingSymbols::declare(&Descriptor::ratio(), shape, [true; 2], &mut symbolic)?;
    let axis = heap.create_cell_axis(cells, &mut symbolic)?;
    let declaration = HostTileDeclaration::check(
        source.clone(),
        axis,
        crate::Coverage::RepresentedInputs,
        &mut account,
    )?;
    let destination_declaration = HostTileDeclaration::check(
        destination.clone(),
        axis,
        crate::Coverage::RepresentedInputs,
        &mut account,
    )?;
    let mut loan = match heap.request_loan(declaration, &mut account) {
        crate::LoanRequest::Granted(loan) => loan,
        other @ crate::LoanRequest::Refused(_) => return Err(format!("{other:?}").into()),
    };
    for cell in 0..cells {
        for component in 0..components {
            loan.write_component(
                HostChannel::Center,
                cell,
                component,
                value(cell, component),
                &mut account,
            )?;
        }
        if holes && cell % 7 == 3 {
            loan.write_component(HostChannel::Center, cell, 0, hole_value(cell), &mut account)?;
            loan.clear_cell(HostChannel::Center, cell, &mut account)?;
        }
    }
    let (tile, receipt) = match heap.handback(loan, &mut account) {
        crate::HandbackOutcome::Published(tile, receipt) => (tile, receipt),
        other => return Err(format!("{other:?}").into()),
    };
    let layout = receipt
        .filled(HostChannel::Center)
        .ok_or("source attachment")?
        .native()
        .layout();
    let selected: Vec<_> = (0..cells)
        .filter(|cell| super::requested(*cell, selection))
        .collect();
    let demand = heap.create_mask(layout, &selected, &mut account.ledger)?;
    let (cas, request) = laws::request()?;
    let law = heap
        .prepare_retained_expm1(
            cas,
            &request,
            domain,
            nrz_bindings::readings::algebra::enclosure::prepared_expm1::Policy::default(),
            &nrz_scalar::ArithmeticContext::new()?,
            &mut nrz_bindings::readings::algebra::enclosure::prepared_expm1::Counts::default(),
            &mut account,
        )
        .map_err(|(error, _)| error)?;
    let declarations = crate::kernel_testing::owner()?;
    let domain_axis = crate::kernel_testing::axis(components)?;
    let codomain = crate::kernel_testing::axis(cells)?;
    let shape = crate::kernel_testing::shape(&declarations, &domain_axis, &codomain)?;
    let regions = crate::OperationDraft::copy(vec![shape]).check(ExecutionLimits::default())?;
    let operation = PreparedExpm1Operation::check(
        &law,
        &request.original.target,
        &request.original.context,
        &request.original.coordinates[0].coordinate,
        domain,
        40,
        1,
        regions.inputs()[0].clone(),
        regions.outputs()[0].clone(),
        source,
        destination,
        axis,
        &mut account,
    )?;
    Ok(Fixture {
        heap,
        account,
        request: PointwiseExpm1Request {
            operation,
            law,
            input: tile.into_pin(),
            destination: destination_declaration,
            demand,
            limits: ExecutionLimits::default(),
        },
    })
}
/// Cleared cells retain distinct invalid families without authorizing a scalar load.
fn hole_value(cell: usize) -> f64 {
    [f64::NAN, -2.0, -f64::MIN_POSITIVE / 2.0][(cell / 7) % 3]
}
/// Values exercise late failure, outside-domain, cancellation and exact zero in stable order.
fn value(cell: usize, component: usize) -> f64 {
    match (cell % 6, component) {
        (0, _) => -0.5,
        (1, 0) | (4, 1) => -2.0,
        (2, 0) => -f64::MIN_POSITIVE / 2.0,
        (3, _) => -1.0,
        (1 | 2 | 4, _) => -0.25,
        (5, _) => 0.0,
        _ => -0.125,
    }
}
/// Extract only the common ready protocol, preserving typed errors in diagnostics.
fn ready(result: ApplicationRequest) -> TestResult<crate::OwnedApplication<f64>> {
    match result {
        ApplicationRequest::ApplicationReady(owner) => Ok(owner),
        other @ ApplicationRequest::ApplicationRefused(..) => Err(format!("{other:?}").into()),
    }
}
/// Independent logical sets at every mask-word boundary include placeholders and refused suffixes.
#[test]
fn prepared_application_selection_and_first_refusal_continues() -> TestResult {
    for cells in [0, 1, 63, 64, 65, 128] {
        for components in [1, 3] {
            for selection in 0..3 {
                let Fixture {
                    mut heap,
                    mut account,
                    request,
                } = fixture(cells, components, [-1.0, 0.0], selection, true)?;
                let owner = ready(heap.request_pointwise_expm1(request, &mut account))?;
                crate::loan::fault::reset_reads();
                crate::loan::fault::reset_scalars();
                crate::loan::fault::reset_stores();
                let owner = candidate(owner, &mut account)?;
                let available = (0..cells)
                    .filter(|cell| super::requested(*cell, selection) && cell % 7 != 3)
                    .count();
                assert_eq!(crate::loan::fault::reads(), available * components);
                assert_eq!(crate::loan::fault::scalars(), available * components);
                assert_eq!(crate::loan::fault::stores(), 5 * available * components);
                let (tile, _) = published(heap.publish_application(owner, &mut account))?;
                let reader = tile.reader(&heap, &mut account)?;
                for cell in 0..cells {
                    let outcome = reader.application_outcome(cell, &mut account)?;
                    let requested = super::requested(cell, selection);
                    let absent = cell % 7 == 3;
                    let refused =
                        cell % 6 == 1 || cell % 6 == 2 || (cell % 6 == 4 && components > 1);
                    let completed = requested && !absent && !refused;
                    assert_eq!(reader.contains(cell, &mut account)?, completed);
                    if !requested {
                        assert!(matches!(outcome, CellOutcome::NotRequested));
                    } else if absent {
                        assert!(matches!(outcome, CellOutcome::MissingInput));
                    } else if refused {
                        let CellOutcome::PreparedRefused { component, reason } = outcome else {
                            return Err("prepared refusal".into());
                        };
                        assert_eq!(*component, usize::from(cell % 6 == 4));
                        assert_eq!(
                            *reason,
                            if cell % 6 == 2 {
                                nrz_scalar::Expm1Failure::BelowNormalOperand
                            } else {
                                nrz_scalar::Expm1Failure::OutsideDomain
                            }
                        );
                    } else {
                        assert!(matches!(outcome, CellOutcome::Completed));
                    }
                    for component in 0..components {
                        if completed {
                            let [lower, upper] =
                                reader.expm1_endpoints(cell, component, &mut account)?;
                            let center = reader.read_component(
                                HostChannel::Center,
                                cell,
                                component,
                                &mut account,
                            )?;
                            let total = reader.read_component(
                                HostChannel::Total,
                                cell,
                                component,
                                &mut account,
                            )?;
                            assert!(
                                nrz_scalar::Enclosure::new(lower, upper)?
                                    .displacement_from(
                                        nrz_scalar::Finite::new(center)?,
                                        &nrz_scalar::ArithmeticContext::new()?
                                    )?
                                    .get()
                                    <= total
                            );
                        } else {
                            assert!(matches!(
                                reader.expm1_endpoints(cell, component, &mut account),
                                Err(crate::LoanError::CoverageMismatch)
                            ));
                            assert!(matches!(
                                reader.read_component(
                                    HostChannel::Center,
                                    cell,
                                    component,
                                    &mut account
                                ),
                                Err(crate::LoanError::CoverageMismatch)
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
