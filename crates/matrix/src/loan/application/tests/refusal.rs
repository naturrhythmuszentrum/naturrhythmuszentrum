// SPDX-License-Identifier: AGPL-3.0-only
//! Whole-shape exclusion and native overflow retain original cell/component locations.

use super::{Fixture, TestResult, candidate, fixture, published, ready};
use crate::{
    ApplicationError, ApplicationRequest, CellOutcome, Elementwise, HostChannel, LoanError,
    StructureScope,
};
use nrz_bindings::{Budget, Descriptor, Limits};
use nrz_scalar::Scalar;

/// Replace the source through public host ownership, preserving the original shared cell axis.
pub(super) fn replace_source<P: Scalar + 'static>(
    fixture: &mut Fixture<P>,
    values: impl Fn(usize, usize) -> (P, P),
) -> TestResult {
    let source = crate::loan::application::contract::source(&fixture.request)?;
    let symbols = source.declaration.symbols.clone();
    let axis = source.declaration.axis;
    let cells = source.declaration.extent.rows();
    let components = source.declaration.extent.columns();
    let channels = source.channels.iter().flatten().count();
    let declaration = crate::HostTileDeclaration::<P>::check(
        symbols,
        axis,
        source.declaration.coverage,
        &mut fixture.account,
    )?;
    let mut loan = match fixture.heap.request_loan(declaration, &mut fixture.account) {
        crate::LoanRequest::Granted(loan) => loan,
        other @ crate::LoanRequest::Refused(_) => return Err(format!("{other:?}").into()),
    };
    for cell in 0..cells {
        for component in 0..components {
            let (center, total) = values(cell, component);
            loan.write_component(
                HostChannel::Center,
                cell,
                component,
                center,
                &mut fixture.account,
            )?;
            if channels > 1 {
                loan.write_component(
                    HostChannel::Total,
                    cell,
                    component,
                    total,
                    &mut fixture.account,
                )?;
            }
            if channels > 2 {
                loan.write_component(
                    HostChannel::Error,
                    cell,
                    component,
                    P::ZERO,
                    &mut fixture.account,
                )?;
            }
        }
    }
    let (tile, receipt) = match fixture.heap.handback(loan, &mut fixture.account) {
        crate::HandbackOutcome::Published(tile, receipt) => (tile, receipt),
        other => return Err(format!("{other:?}").into()),
    };
    let old = fixture.request.demand;
    let selected = (0..cells)
        .filter(|cell| {
            fixture
                .heap
                .mask(old, old.layout())
                .is_ok_and(|mask| mask.words()[cell / 64] & (1 << (cell % 64)) != 0)
        })
        .collect::<Vec<_>>();
    let layout = receipt
        .filled(HostChannel::Center)
        .ok_or("filled")?
        .native()
        .layout();
    fixture.request.demand =
        fixture
            .heap
            .create_mask(layout, &selected, &mut fixture.account.ledger)?;
    fixture.heap.retire_mask(old, &mut fixture.account.ledger)?;
    fixture.request.input = tile.into_pin();
    Ok(())
}
/// Construct independent input and output assertions through the public checked-shape route.
pub(super) fn structural<P: Scalar>(
    request: &mut crate::PointwiseRequest<P>,
    output: bool,
    entry: usize,
    scope: StructureScope,
) -> TestResult {
    let cells = request.destination.capacity();
    let components = request.destination.components();
    let original = request.operation.inputs()[0].shape();
    let owner = std::rc::Rc::clone(&original.declarations);
    let domain = original.domain().clone();
    let codomain = original.codomain().clone();
    let build = |zero: bool| -> TestResult<crate::CheckedShape> {
        let entries = (0..cells * components)
            .map(|index| {
                if zero && index == entry {
                    crate::EntryDraft::Zero(Descriptor::ratio())
                } else {
                    crate::EntryDraft::Declared(Descriptor::ratio())
                }
            })
            .collect();
        Ok(crate::ShapeDraft::new(
            domain.clone(),
            codomain.clone(),
            vec![crate::BlockDraft::new(0..cells, 0..components, entries)],
            scope,
        )
        .check(std::rc::Rc::clone(&owner), crate::ShapeLimits::default())?)
    };
    request.operation = crate::OperationDraft::map_elementwise(
        build(!output)?,
        build(output)?,
        Elementwise::Square,
    )?
    .check(crate::ExecutionLimits::default())?;
    Ok(())
}
/// Structural refusal is independent of numerical satisfaction and selected demand.
fn zeros<P: Scalar + 'static>() -> TestResult {
    for scope in [StructureScope::CenterOnly, StructureScope::WholeFamily] {
        for output in [false, true] {
            for (cells, components) in [(1, 1), (3, 3)] {
                for entry in [0, cells * components - 1] {
                    for selection in 0..3 {
                        for payload in 0..3 {
                            let mut setup = fixture::<P>(
                                cells,
                                components,
                                3,
                                selection,
                                false,
                                Limits::default(),
                            )?;
                            if payload > 0 {
                                replace_source(&mut setup, |_, _| {
                                    (P::ZERO, if payload == 1 { P::ONE } else { P::ZERO })
                                })?;
                            }
                            structural(&mut setup.request, output, entry, scope)?;
                            let ticket = setup.request.input.ticket()?;
                            let pointer = std::rc::Rc::as_ptr(&setup.request.input.record);
                            let before = setup.account.usage().work;
                            let injection = crate::loan::fault::Injection::after(0);
                            crate::loan::fault::reset_reads();
                            crate::loan::fault::reset_scalars();
                            let result = setup
                                .heap
                                .request_pointwise_square(setup.request, &mut setup.account);
                            let request = match result {
                                ApplicationRequest::ApplicationRefused(
                                    ApplicationError::UnsupportedStructure {
                                        output: actual,
                                        entry: index,
                                        scope: actual_scope,
                                    },
                                    request,
                                ) => {
                                    assert_eq!(
                                        (actual, index, actual_scope),
                                        (output, entry, scope)
                                    );
                                    request
                                }
                                other => return Err(format!("{other:?}").into()),
                            };
                            assert_eq!(request.input.ticket()?, ticket);
                            assert_eq!(std::rc::Rc::as_ptr(&request.input.record), pointer);
                            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                            assert_eq!(crate::loan::fault::reads(), 0);
                            assert_eq!(crate::loan::fault::scalars(), 0);
                            // Request authentication 1, current source 3h+12, then the actual metadata prefix.
                            assert_eq!(
                                setup.account.usage().work - before,
                                u64::try_from(
                                    22 + usize::from(output) * cells * components + entry + 1
                                )?
                            );
                            drop(injection);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
/// Full-shape assertions refuse before all allocations and numerical access in both formats.
/// ´test:unit:pointwise-structural-exclusion-both-sides-scopes-and-demands´
#[test]
fn pointwise_structural_exclusion_both_sides_scopes_and_demands() -> TestResult {
    zeros::<f32>()?;
    zeros::<f64>()
}
/// Exhaustion immediately before a metadata visit preserves the original request and pin.
/// ´test:unit:pointwise-structural-work-prefixes-preserve-request´
#[test]
fn pointwise_structural_work_prefixes_preserve_request() -> TestResult {
    for output in [false, true] {
        for entry in [0, 5] {
            let mut setup = fixture::<f64>(2, 3, 3, 0, false, Limits::default())?;
            structural(
                &mut setup.request,
                output,
                entry,
                StructureScope::WholeFamily,
            )?;
            let prefix = 22 + usize::from(output) * 6 + entry;
            let spend =
                setup.account.limits().work - setup.account.usage().work - u64::try_from(prefix)?;
            setup.account.ledger.work(spend, crate::loan::AT)?;
            let ticket = setup.request.input.ticket()?;
            let injection = crate::loan::fault::Injection::after(0);
            match setup
                .heap
                .request_pointwise_square(setup.request, &mut setup.account)
            {
                ApplicationRequest::ApplicationRefused(
                    ApplicationError::Loan(LoanError::Budget(_)),
                    request,
                ) => assert_eq!(request.input.ticket()?, ticket),
                other => return Err(format!("{other:?}").into()),
            }
            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
            drop(injection);
        }
    }
    Ok(())
}
/// An unbounded late component refuses only its own cell; every selected component still runs.
fn overflow<P: Scalar + 'static>(maximum: P, failures: &[usize]) -> TestResult {
    let mut setup = fixture::<P>(3, 3, 3, 2, false, Limits::default())?;
    replace_source(&mut setup, |cell, component| {
        if cell == 1 && failures.contains(&component) {
            (maximum, maximum)
        } else {
            (P::ONE + P::ONE, P::ZERO)
        }
    })?;
    let owner = ready(
        setup
            .heap
            .request_pointwise_square(setup.request, &mut setup.account),
    )?;
    crate::loan::fault::reset_reads();
    crate::loan::fault::reset_scalars();
    let before = setup.account.usage().work;
    let owner = candidate(owner, &mut setup.account)?;
    assert_eq!(setup.account.usage().work - before, 162);
    for channel in owner.core.host.channels.iter().flatten() {
        assert_eq!(channel.components, [511]);
    }
    assert_eq!(crate::loan::fault::reads(), 18);
    assert_eq!(crate::loan::fault::scalars(), 9);
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    assert!(matches!(
        reader.application_outcome(1, &mut setup.account)?,
        CellOutcome::CellRefused {
            component,
            reason: crate::KernelError::Domain {
                function: Elementwise::Square,
                entry,
                reason: crate::DomainReason::UnboundedImage
            }
        } if *component == failures[0] && *entry == 1 + 3 * failures[0]
    ));
    assert!(!reader.contains(1, &mut setup.account)?);
    for cell in [0, 2] {
        assert!(reader.contains(cell, &mut setup.account)?);
        assert_eq!(
            reader.read_component(HostChannel::Center, cell, 0, &mut setup.account)?,
            (P::ONE + P::ONE) * (P::ONE + P::ONE)
        );
    }
    Ok(())
}
/// Maximum-finite input-box overflow retains the dense wrapper's domain reason and original entry.
/// ´test:unit:pointwise-maximum-finite-overflow-and-later-valid-cell´
#[test]
fn pointwise_maximum_finite_overflow_and_later_valid_cell() -> TestResult {
    for failures in [&[2][..], &[0][..], &[1][..], &[0, 2][..], &[1, 2][..]] {
        overflow(f32::MAX, failures)?;
        overflow(f64::MAX, failures)?;
    }
    Ok(())
}
/// Unsupported closed laws return their exact request before every numerical allocation.
/// ´test:unit:pointwise-unsupported-recipe-refuses-before-allocation´
#[test]
fn pointwise_unsupported_recipe_refuses_before_allocation() -> TestResult {
    for function in [Elementwise::Reciprocal, Elementwise::SquareRoot] {
        let mut setup = fixture::<f64>(1, 1, 3, 2, false, Limits::default())?;
        let input = setup.request.operation.inputs()[0].shape().clone();
        let output = setup.request.operation.outputs()[0].shape().clone();
        setup.request.operation = crate::OperationDraft::map_elementwise(input, output, function)?
            .check(crate::ExecutionLimits::default())?;
        let ticket = setup.request.input.ticket()?;
        let injection = crate::loan::fault::Injection::after(0);
        match setup
            .heap
            .request_pointwise_square(setup.request, &mut setup.account)
        {
            ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::UnsupportedContract),
                request,
            ) => assert_eq!(request.input.ticket()?, ticket),
            other => return Err(format!("{other:?}").into()),
        }
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        drop(injection);
    }
    let mut foreign = crate::ExecutionAccount::new(Budget::new(Limits::default())?)?;
    let Fixture {
        mut heap,
        account: _,
        request,
    } = fixture::<f64>(1, 1, 3, 2, false, Limits::default())?;
    assert!(matches!(
        heap.request_pointwise_square(request, &mut foreign),
        ApplicationRequest::ApplicationRefused(
            ApplicationError::Loan(LoanError::ForeignAccount),
            _
        )
    ));
    Ok(())
}
