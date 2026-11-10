// SPDX-License-Identifier: AGPL-3.0-only
//! Independent masked numerical, resource and ownership controls.

mod authority;
mod bytes;
pub(in crate::loan::application) mod expm1;
mod invariants;
mod laws;
mod lifecycle;
mod prefixes;
mod refusal;
mod resources;
mod retained;
mod review;
mod spare;
mod work;

use super::{
    ApplicationEvaluation, ApplicationPublication, ApplicationRequest, CellOutcome,
    OwnedApplication, PointwiseRequest,
};
use crate::{
    Coverage, ExecutionAccount, ExecutionLimits, HandbackOutcome, Heap, HeapLimits, HostChannel,
    HostTileDeclaration, LoanRequest,
};
use nrz_bindings::{BindingSymbols, Budget, Descriptor, Limits};
use nrz_scalar::Scalar;

/// Test errors retain original typed causes without production assertions.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
/// A complete original-account fixture includes its exact host source and ordinary demand.
struct Fixture<P: Scalar> {
    /// Existing shared numerical owner.
    heap: Heap,
    /// Original cumulative continuation account.
    account: ExecutionAccount,
    /// Original request, with its single source pin.
    request: PointwiseRequest<P>,
}
/// Independently declare meanings before constructing the host source.
fn fixture<P: Scalar + 'static>(
    cells: usize,
    components: usize,
    channels: usize,
    selection: usize,
    holes: bool,
    limits: Limits,
) -> TestResult<Fixture<P>> {
    fixture_coverage(
        cells,
        components,
        channels,
        selection,
        holes,
        limits,
        Coverage::RepresentedInputs,
    )
}
/// Both declared physical bounds and represented input families retain their original coverage.
fn fixture_coverage<P: Scalar + 'static>(
    cells: usize,
    components: usize,
    channels: usize,
    selection: usize,
    holes: bool,
    limits: Limits,
    coverage: Coverage,
) -> TestResult<Fixture<P>> {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut symbolic = Budget::new(Limits::default())?;
    let shape = [u32::try_from(cells)?, u32::try_from(components)?];
    let input = BindingSymbols::declare(
        &Descriptor::ratio(),
        shape,
        [channels > 1, channels > 2],
        &mut symbolic,
    )?;
    let output = BindingSymbols::declare(&Descriptor::ratio(), shape, [true, true], &mut symbolic)?;
    let axis = heap.create_cell_axis(cells, &mut symbolic)?;
    let mut budget = Budget::new(limits)?;
    budget.work(3, crate::loan::AT)?;
    let mut account = ExecutionAccount::new(budget)?;
    let input = HostTileDeclaration::<P>::check(input, axis, coverage, &mut account)?;
    let destination = HostTileDeclaration::<P>::check(output, axis, coverage, &mut account)?;
    let mut loan = match heap.request_loan(input, &mut account) {
        LoanRequest::Granted(loan) => loan,
        other @ LoanRequest::Refused(_) => return Err(format!("{other:?}").into()),
    };
    for cell in 0..cells {
        for component in 0..components {
            let center = if cell % 2 == 0 {
                P::ONE + P::ONE
            } else {
                P::ONE / (P::ONE + P::ONE + P::ONE + P::ONE)
            };
            let partial = holes && components > 1 && cell % 13 == 4 && component == components - 1;
            if !partial {
                loan.write_component(HostChannel::Center, cell, component, center, &mut account)?;
            }
            if channels > 1 && !(holes && cell % 5 == 1) {
                let total = if cell % 2 == 0 {
                    P::ONE / ((P::ONE + P::ONE) * (P::ONE + P::ONE) * (P::ONE + P::ONE))
                } else {
                    P::ONE / (P::ONE + P::ONE)
                };
                loan.write_component(HostChannel::Total, cell, component, total, &mut account)?;
            }
            if channels > 2 && !(holes && cell % 7 == 2) {
                loan.write_component(HostChannel::Error, cell, component, P::ZERO, &mut account)?;
            }
        }
        if holes && cell % 11 == 3 {
            loan.write_component(
                HostChannel::Center,
                cell,
                0,
                P::parse_nearest("NaN").ok_or("native NaN")?,
                &mut account,
            )?;
            loan.clear_cell(HostChannel::Center, cell, &mut account)?;
        }
    }
    let (tile, receipt) = match heap.handback(loan, &mut account) {
        HandbackOutcome::Published(tile, receipt) => (tile, receipt),
        other => return Err(format!("{other:?}").into()),
    };
    let layout = receipt
        .filled(HostChannel::Center)
        .ok_or("filled")?
        .native()
        .layout();
    let demand = (0..cells)
        .filter(|cell| requested(*cell, selection))
        .collect::<Vec<_>>();
    let demand = heap.create_mask(layout, &demand, &mut account.ledger)?;
    let declarations = crate::kernel_testing::owner()?;
    let domain = crate::kernel_testing::axis(components)?;
    let codomain = crate::kernel_testing::axis(cells)?;
    let input = crate::kernel_testing::shape(&declarations, &domain, &codomain)?;
    let output = crate::kernel_testing::shape(&declarations, &domain, &codomain)?;
    let operation =
        crate::OperationDraft::map_elementwise(input, output, crate::Elementwise::Square)?
            .check(ExecutionLimits::default())?;
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
/// Independent demand classes cover empty, sparse and full selections.
const fn requested(cell: usize, selection: usize) -> bool {
    selection == 2 || (selection == 1 && cell % 3 != 1)
}
/// Independent host logical sets never call the implementation's filled planner.
const fn available(cell: usize, components: usize, channels: usize, holes: bool) -> bool {
    !holes
        || (cell % 11 != 3
            && (components < 2 || cell % 13 != 4)
            && (channels < 2 || cell % 5 != 1)
            && (channels < 3 || cell % 7 != 2))
}
/// Preserve typed failure reports while extracting a successful ready owner.
fn ready<P: Scalar>(outcome: ApplicationRequest<P>) -> TestResult<OwnedApplication<P>> {
    match outcome {
        ApplicationRequest::ApplicationReady(owner) => Ok(owner),
        other @ ApplicationRequest::ApplicationRefused(..) => Err(format!("{other:?}").into()),
    }
}
/// Extract the single completed traversal without manufacturing evidence.
fn candidate<P: Scalar + 'static>(
    owner: OwnedApplication<P>,
    account: &mut ExecutionAccount,
) -> TestResult<OwnedApplication<P>> {
    match owner.evaluate(account) {
        ApplicationEvaluation::ApplicationCandidate(owner) => Ok(owner),
        other => Err(format!("{other:?}").into()),
    }
}
/// Extract only a fully published result.
fn published<P: Scalar>(
    outcome: ApplicationPublication<P>,
) -> TestResult<(crate::PublishedTile<P>, crate::PublicationReceipt)> {
    match outcome {
        ApplicationPublication::ApplicationPublished(tile, receipt) => Ok((tile, receipt)),
        other => Err(format!("{other:?}").into()),
    }
}
/// Independent rational endpoint truth covers crossing and noncrossing square boxes.
fn oracle<P: Scalar + 'static>(
    reader: &crate::TileReader<'_, P>,
    cell: usize,
    component: usize,
    bounded: bool,
    account: &mut ExecutionAccount,
) -> TestResult {
    use crate::kernel_oracle::Rational;
    let center = reader
        .read_component(HostChannel::Center, cell, component, account)?
        .to_f64();
    let total = reader
        .read_component(HostChannel::Total, cell, component, account)?
        .to_f64();
    let error = reader
        .read_component(HostChannel::Error, cell, component, account)?
        .to_f64();
    let nominal = if cell % 2 == 0 { (4, 1) } else { (1, 16) };
    assert!(Rational::new(nominal.0, nominal.1)?.covered(center, error)?);
    if bounded {
        let endpoints = if cell % 2 == 0 {
            [(225, 64), (289, 64)]
        } else {
            [(0, 1), (9, 16)]
        };
        for (numerator, denominator) in endpoints {
            assert!(Rational::new(numerator, denominator)?.covered(center, total)?);
        }
    } else {
        assert!(Rational::new(nominal.0, nominal.1)?.covered(center, total)?);
    }
    assert!(error <= total);
    Ok(())
}
/// Every selected scalar is observed at its actual accessor and shared arithmetic seam.
fn selections<P: Scalar + 'static>() -> TestResult {
    for cells in [0, 1, 63, 64, 65, 128] {
        for components in [1, 3] {
            for channels in 1..=3 {
                for selection in 0..3 {
                    let Fixture {
                        mut heap,
                        mut account,
                        request,
                    } = fixture::<P>(
                        cells,
                        components,
                        channels,
                        selection,
                        true,
                        Limits::default(),
                    )?;
                    let owner = ready(heap.request_pointwise_square(request, &mut account))?;
                    let expected = (0..cells)
                        .filter(|cell| {
                            requested(*cell, selection)
                                && available(*cell, components, channels, true)
                        })
                        .collect::<Vec<_>>();
                    crate::loan::fault::reset_reads();
                    crate::loan::fault::reset_scalars();
                    let owner = candidate(owner, &mut account)?;
                    assert_eq!(
                        crate::loan::fault::reads(),
                        (if channels == 1 { 1 } else { 2 }) * components * expected.len()
                    );
                    assert_eq!(crate::loan::fault::scalars(), components * expected.len());
                    let (tile, _) = published(heap.publish_application(owner, &mut account))?;
                    let reader = tile.reader(&heap, &mut account)?;
                    for cell in 0..cells {
                        assert_eq!(
                            reader.contains(cell, &mut account)?,
                            expected.contains(&cell)
                        );
                        let outcome = reader.application_outcome(cell, &mut account)?;
                        if expected.contains(&cell) {
                            assert!(matches!(outcome, CellOutcome::Completed));
                            for component in 0..components {
                                oracle(&reader, cell, component, channels > 1, &mut account)?;
                            }
                        } else if requested(cell, selection) {
                            assert!(matches!(outcome, CellOutcome::MissingInput));
                        } else {
                            assert!(matches!(outcome, CellOutcome::NotRequested));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
/// Both formats and word boundaries retain exact logical selection and rational enclosures.
/// ´test:unit:pointwise-independent-selection-and-square-oracle´
#[test]
fn pointwise_independent_selection_and_square_oracle() -> TestResult {
    selections::<f32>()?;
    selections::<f64>()
}
