// SPDX-License-Identifier: AGPL-3.0-only
//! Opt-in policy and the original layout, structure and account refusals precede allocation.

use super::super::super as common;
use super::{TestResult, admit, product, request as make_request};
use crate::{ApplicationError, ApplicationRequest, Elementwise, LoanError};
use nrz_bindings::{Budget, Limits};

/// Old host routes keep rejecting application origins; opt-in routes require an application origin.
#[test]
fn chain_origin_mode_is_opt_in_and_member_specific() -> TestResult {
    for function in [
        Elementwise::Square,
        Elementwise::SquareRoot,
        Elementwise::Reciprocal,
    ] {
        let (mut heap, mut account, tile, _) = product(1, 1)?;
        let request = make_request(&mut heap, tile.into_pin(), function, 2, &mut account)?;
        let result = match function {
            Elementwise::Square => heap.request_pointwise_square(request, &mut account),
            Elementwise::SquareRoot => heap.request_pointwise_square_root(request, &mut account),
            Elementwise::Reciprocal => heap.request_pointwise_reciprocal(request, &mut account),
        };
        assert!(matches!(
            result,
            ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::UnsupportedContract),
                _
            )
        ));
        let mut host = common::fixture::<f64>(1, 1, 1, 2, false, Limits::default())?;
        assert!(matches!(
            admit(&mut host.heap, host.request, function, &mut host.account),
            ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::UnsupportedContract),
                _
            )
        ));
        for other in [
            Elementwise::Square,
            Elementwise::SquareRoot,
            Elementwise::Reciprocal,
        ] {
            if other == function {
                continue;
            }
            let (mut heap, mut account, tile, _) = product(1, 1)?;
            let request = make_request(&mut heap, tile.into_pin(), other, 2, &mut account)?;
            assert!(matches!(
                admit(&mut heap, request, function, &mut account),
                ApplicationRequest::ApplicationRefused(
                    ApplicationError::Loan(LoanError::UnsupportedContract),
                    _
                )
            ));
        }
    }
    Ok(())
}

/// Sparse or empty demand cannot hide full-shape input or output structural zeros.
#[test]
fn chain_full_shape_structural_exclusion_outside_demand() -> TestResult {
    for selection in [0, 1] {
        for output in [false, true] {
            for scope in [
                crate::StructureScope::CenterOnly,
                crate::StructureScope::WholeFamily,
            ] {
                let (mut heap, mut account, tile, _) = product(3, 1)?;
                let mut request = make_request(
                    &mut heap,
                    tile.into_pin(),
                    Elementwise::Square,
                    selection,
                    &mut account,
                )?;
                common::refusal::structural(&mut request, output, 1, scope)?;
                let injection = crate::loan::fault::Injection::after(0);
                assert!(
                    matches!(admit(&mut heap, request, Elementwise::Square, &mut account),
                    ApplicationRequest::ApplicationRefused(ApplicationError::UnsupportedStructure { output: actual, entry: 1, scope: original }, _) if actual == output && original == scope)
                );
                assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                drop(injection);
            }
        }
    }
    Ok(())
}

/// Wrong demand layout, fresh axis, foreign continuation and stale generation retain typed refusals.
#[test]
fn chain_request_metadata_refusals_preserve_original_pin() -> TestResult {
    for choice in 0..5 {
        let (mut heap, mut account, tile, demand) = product(1, 1)?;
        let mut request = make_request(
            &mut heap,
            tile.into_pin(),
            Elementwise::SquareRoot,
            2,
            &mut account,
        )?;
        let pointer = std::rc::Rc::as_ptr(&request.input.record);
        let mut foreign = crate::ExecutionAccount::new(Budget::new(Limits::default())?)?;
        match choice {
            0 => request.demand = demand,
            1 => request.destination.axis = heap.create_cell_axis(1, &mut account.ledger)?,
            2 => request.destination.coverage = crate::Coverage::DeclaredBounds,
            3 => {}
            4 => {
                let parent = request
                    .input
                    .record
                    .payload
                    .as_ref()
                    .and_then(|pin| pin.bundle.payload.as_ref())
                    .and_then(|bundle| bundle.origin.evidence.as_ref())
                    .ok_or("evidence")?
                    .source_identity;
                heap.loans.groups[parent.root.slot]
                    .as_mut()
                    .ok_or("parent")?
                    .identity
                    .root
                    .generation += 1;
            }
            _ => return Err("choice".into()),
        }
        let injection = crate::loan::fault::Injection::after(0);
        let result = admit(
            &mut heap,
            request,
            Elementwise::SquareRoot,
            if choice == 3 {
                &mut foreign
            } else {
                &mut account
            },
        );
        let ApplicationRequest::ApplicationRefused(ApplicationError::Loan(error), request) = result
        else {
            return Err(format!("{result:?}").into());
        };
        assert!(
            match choice {
                0 | 1 => matches!(error, LoanError::LayoutMismatch),
                2 => matches!(error, LoanError::CoverageMismatch),
                3 => matches!(error, LoanError::ForeignAccount),
                4 => matches!(error, LoanError::StaleDependency),
                _ => false,
            },
            "{error:?}"
        );
        assert_eq!(std::rc::Rc::as_ptr(&request.input.record), pointer);
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        drop(injection);
    }
    Ok(())
}

/// A prepared result's retained evidence and endpoint roles cannot enter any native chain route.
#[test]
fn chain_prepared_result_refuses_before_output_allocation() -> TestResult {
    for function in [
        Elementwise::Square,
        Elementwise::SquareRoot,
        Elementwise::Reciprocal,
    ] {
        let mut setup = common::expm1::fixture(1, 1, [-2.0, 0.0], 2, false)?;
        let owner = match setup
            .heap
            .request_pointwise_expm1(setup.request, &mut setup.account)
        {
            crate::Expm1ApplicationRequest::ApplicationReady(owner) => owner,
            other @ crate::Expm1ApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        };
        let owner = super::candidate(owner, &mut setup.account)?;
        let (tile, _) =
            super::published(setup.heap.publish_application(owner, &mut setup.account))?;
        let request = make_request(
            &mut setup.heap,
            tile.into_pin(),
            function,
            2,
            &mut setup.account,
        )?;
        let injection = crate::loan::fault::Injection::after(0);
        assert!(matches!(
            admit(&mut setup.heap, request, function, &mut setup.account),
            ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::UnsupportedContract),
                _
            )
        ));
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        drop(injection);
    }
    Ok(())
}

/// Equal value, shape and descriptor do not replace demand's exact source-layout correspondence.
#[test]
fn chain_same_valued_same_shaped_operand_substitution_refuses() -> TestResult {
    let super::super::Fixture {
        mut heap,
        mut account,
        request: original,
    } = super::super::custom(
        1,
        1,
        [1; 2],
        2,
        false,
        crate::BinaryElementwise::Product,
        Limits::default(),
        |_, _, _| (1.0, 0.0),
    )?;
    let owner = super::super::ready(heap.request_pointwise_product(original, &mut account))?;
    let owner = super::candidate(owner, &mut account)?;
    let (first, _) = super::published(heap.publish_application(owner, &mut account))?;
    let extra = heap.retain_pin(first.pin(), &mut account)?;
    let second = super::apply(&mut heap, extra, Elementwise::Square, 2, &mut account)?;
    for tile in [&first, &second] {
        assert_eq!(
            tile.reader(&heap, &mut account)?
                .read_component(crate::HostChannel::Center, 0, 0, &mut account)?
                .to_bits(),
            1.0_f64.to_bits()
        );
    }
    let mut request = make_request(
        &mut heap,
        first.into_pin(),
        Elementwise::SquareRoot,
        2,
        &mut account,
    )?;
    let mut rightful = second.into_pin();
    std::mem::swap(&mut request.input, &mut rightful);
    let mut request = match admit(&mut heap, request, Elementwise::SquareRoot, &mut account) {
        ApplicationRequest::ApplicationRefused(
            ApplicationError::Loan(LoanError::LayoutMismatch),
            request,
        ) => request,
        other => return Err(format!("{other:?}").into()),
    };
    std::mem::swap(&mut request.input, &mut rightful);
    let owner = common::ready(admit(
        &mut heap,
        request,
        Elementwise::SquareRoot,
        &mut account,
    ))?;
    let owner = super::candidate(owner, &mut account)?;
    let _tile = super::published(heap.publish_application(owner, &mut account))?;
    Ok(())
}

/// Unrelated legacy storage growth cannot invalidate a pinned managed occupation's captured identity.
#[test]
fn chain_unrelated_legacy_relocation_keeps_current_lineage() -> TestResult {
    let (mut heap, mut account, tile, _) = product(1, 1)?;
    let request = make_request(
        &mut heap,
        tile.into_pin(),
        Elementwise::SquareRoot,
        2,
        &mut account,
    )?;
    let ticket = request.input.ticket()?;
    heap.import_represented(
        crate::Extent::new(128, 1)?,
        &nrz_bindings::Descriptor::ratio(),
        &[7.0; 128],
    )?;
    let owner = common::ready(admit(
        &mut heap,
        request,
        Elementwise::SquareRoot,
        &mut account,
    ))?;
    assert_eq!(owner.core.input.as_ref().ok_or("input")?.ticket()?, ticket);
    let owner = super::candidate(owner, &mut account)?;
    let _tile = super::published(heap.publish_application(owner, &mut account))?;
    Ok(())
}
