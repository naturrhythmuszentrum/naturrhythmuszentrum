// SPDX-License-Identifier: AGPL-3.0-only
//! Independent inverse identities check native projected root and reciprocal consumers.

use super::super::{Setup, TestResult, candidate, empty, publish};
use crate::kernel_oracle::Rational;
use crate::{
    ApplicationEvaluation, ApplicationRequest, Elementwise, HostChannel, PointwiseRequest,
};
use nrz_scalar::Scalar;

/// The source shape and native incidence remain unchanged when selecting a unary law.
pub(super) fn operation<P: Scalar>(
    request: &mut PointwiseRequest<P>,
    function: Elementwise,
) -> TestResult {
    let crate::binding::NativeRecipe::Kernel(law) = &request.operation.recipe else {
        return Err("kernel".into());
    };
    let crate::kernels::contract::Recipe::Transform(map) = &law.recipe else {
        return Err("map".into());
    };
    request.operation = crate::OperationDraft::map_projected_elementwise(
        request.operation.inputs()[0].shape().clone(),
        request.operation.outputs()[0].shape().clone(),
        map.clone(),
        function,
    )?
    .check(crate::ExecutionLimits::default())?;
    assert!(!request.operation.carries_statistics());
    Ok(())
}

/// Literal source intervals are checked by inverse identities, independently of interval images.
/// ´test:unit:loan-projected-unary-native-consumers´
#[test]
fn loan_projected_unary_native_consumers() -> TestResult {
    for function in [Elementwise::Reciprocal, Elementwise::SquareRoot] {
        for product in [false, true] {
            direct::<f32>(function, product)?;
            direct::<f64>(function, product)?;
        }
    }
    Ok(())
}

/// Gather holes, repeated product rows and outside readers retain their original association.
fn direct<P: Scalar + 'static>(function: Elementwise, product: bool) -> TestResult {
    let (setup, outside) = if product {
        let (setup, outside) = super::super::product_setup::<P>()?;
        (setup, Some(outside))
    } else {
        (super::super::setup::<P>()?, None)
    };
    let Setup {
        mut heap,
        mut account,
        mut request,
        demand,
    } = setup;
    operation(&mut request, function)?;
    let owner = match heap.request_projected_elementwise(request, &mut account) {
        ApplicationRequest::ApplicationReady(owner) => owner,
        other @ ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    crate::loan::fault::reset_reads();
    let owner = candidate(owner, &mut account)?;
    assert_eq!(crate::loan::fault::reads(), if product { 24 } else { 8 });
    let tile = publish(&mut heap, &mut account, owner)?;
    let reader = tile.reader(&heap, &mut account)?;
    check_values(&reader, &mut account, function, product)?;
    assert_eq!(
        reader.numerical_error_scope(),
        crate::NumericalErrorScope::LocalOperation
    );
    drop(tile);
    heap.retire_mask(demand, &mut account.ledger)?;
    super::super::retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    if let Some(outside) = outside {
        assert_eq!(
            outside.reader(&heap, &mut account)?.read_component(
                HostChannel::Error,
                4,
                1,
                &mut account
            )?,
            P::from_u8(2)
        );
        drop(outside);
    }
    empty(&mut heap, &mut account)
}

/// Independent original indices determine both component intervals and every hole.
fn check_values<P: Scalar + 'static>(
    reader: &crate::TileReader<'_, P>,
    account: &mut crate::ExecutionAccount,
    function: Elementwise,
    product: bool,
) -> TestResult {
    let original = if product {
        vec![
            Some(0),
            Some(0),
            Some(1),
            Some(1),
            None,
            None,
            None,
            None,
            Some(4),
            Some(4),
        ]
    } else {
        vec![Some(1), None, Some(4), None]
    };
    for (cell, original) in original.into_iter().enumerate() {
        assert_eq!(reader.contains(cell, account)?, original.is_some());
        if let Some(original) = original {
            for (component, base, radius) in [(0, 10.0, 3.0), (1, 20.0, 4.0)] {
                let center = reader
                    .read_component(HostChannel::Center, cell, component, account)?
                    .to_f64();
                let total = reader
                    .read_component(HostChannel::Total, cell, component, account)?
                    .to_f64();
                let error = reader
                    .read_component(HostChannel::Error, cell, component, account)?
                    .to_f64();
                let input = base + f64::from(u8::try_from(original)?);
                inverse(function, [center, total], [input - radius, input + radius])?;
                inverse(function, [center, error], [input; 2])?;
                assert!(error < radius / 1000.0);
            }
        }
    }
    Ok(())
}

/// Exact dyadic readings give rational endpoint checks without production enclosure arithmetic.
fn inverse(function: Elementwise, reading: [f64; 2], input: [f64; 2]) -> TestResult {
    let center = Rational::binary(reading[0])?;
    let radius = Rational::binary(reading[1])?;
    let lower = center.subtract(radius)?;
    let upper = center.add(radius)?;
    assert!(lower.numerator > 0);
    let input = [Rational::binary(input[0])?, Rational::binary(input[1])?];
    match function {
        Elementwise::Reciprocal => {
            let one = Rational::new(1, 1)?;
            assert!(lower.multiply(input[1])?.at_most(one)?);
            assert!(one.at_most(upper.multiply(input[0])?)?);
        }
        Elementwise::SquareRoot => {
            let lower = inward(lower, true)?;
            let upper = inward(upper, false)?;
            assert!(lower.multiply(lower)?.at_most(input[0])?);
            assert!(input[1].at_most(upper.multiply(upper)?)?);
        }
        Elementwise::Square => return Err("inverse-law fixture".into()),
    }
    Ok(())
}

/// Inward dyadics at 2^-60 keep squared numerators below i128 for these inputs below 28.
/// Moving inward strengthens containment and avoids squaring tiny error denominators.
fn inward(value: Rational, ceil: bool) -> TestResult<Rational> {
    let scale = 1_i128 << 60;
    let common = crate::kernel_oracle::gcd(scale, value.denominator);
    let numerator = value
        .numerator
        .checked_mul(scale / common)
        .ok_or("oracle overflow")?;
    let denominator = value.denominator / common;
    let floor = numerator.div_euclid(denominator);
    let increment = ceil && numerator.rem_euclid(denominator) != 0;
    Rational::new(floor + i128::from(increment), scale)
}

/// Two cells, two components, two loads, three stores, twelve invocation units,
/// four cell visits, three owner bindings, two word visits and entry give exactly 78.
/// ´test:unit:loan-projected-unary-literal-work-boundary´
#[test]
fn loan_projected_unary_literal_work_boundary() -> TestResult {
    for function in [Elementwise::Reciprocal, Elementwise::SquareRoot] {
        for available in [77, 78] {
            let Setup {
                mut heap,
                mut account,
                mut request,
                ..
            } = super::super::setup::<f64>()?;
            operation(&mut request, function)?;
            let owner = match heap.request_projected_elementwise(request, &mut account) {
                ApplicationRequest::ApplicationReady(owner) => owner,
                other @ ApplicationRequest::ApplicationRefused(..) => {
                    return Err(format!("{other:?}").into());
                }
            };
            let spend = account
                .limits()
                .work
                .checked_sub(account.usage().work)
                .and_then(|work| work.checked_sub(available))
                .ok_or("remaining Work")?;
            account.ledger.work(spend, crate::loan::AT)?;
            crate::loan::fault::reset_reads();
            let result = owner.evaluate(&mut account);
            if available == 78 {
                assert!(matches!(
                    result,
                    ApplicationEvaluation::ApplicationCandidate(_)
                ));
                assert_eq!(crate::loan::fault::reads(), 8);
            } else {
                assert!(matches!(
                    result,
                    ApplicationEvaluation::ApplicationNotStarted(..)
                ));
                assert_eq!(crate::loan::fault::reads(), 0);
            }
        }
    }
    Ok(())
}

/// Empty and complete demand use the same two-format mapped unary lifecycle.
/// ´test:unit:loan-projected-unary-full-and-empty´
#[test]
fn loan_projected_unary_full_and_empty() -> TestResult {
    for function in [Elementwise::Reciprocal, Elementwise::SquareRoot] {
        super::coverage_function::<f32>(function)?;
        super::coverage_function::<f64>(function)?;
    }
    Ok(())
}

/// Zero, a zero-crossing interval and an exact positive square have independent domain answers.
/// ´test:unit:loan-projected-unary-domain-refusals´
#[test]
fn loan_projected_unary_domain_refusals() -> TestResult {
    for function in [Elementwise::Reciprocal, Elementwise::SquareRoot] {
        for component in [0, 1] {
            domains::<f32>(function, component)?;
            domains::<f64>(function, component)?;
        }
    }
    Ok(())
}

/// Three literal two-component boxes share an identity compaction with one padding cell.
pub(super) fn domain_setup<P: Scalar + 'static>(
    centers: [[P; 2]; 3],
    totals: [[P; 2]; 3],
    coverage: crate::Coverage,
) -> TestResult<Setup<P>> {
    let (mut heap, mut account, input) =
        crate::loan::tests::fixture::<P>(3, 2, [true; 2], nrz_bindings::Limits::default())?;
    let input = crate::HostTileDeclaration::check(
        input.symbols.clone(),
        input.axis,
        coverage,
        &mut account,
    )?;
    let mut loan = crate::loan::tests::granted(heap.request_loan(input, &mut account))?;
    for (cell, (center, total)) in centers.into_iter().zip(totals).enumerate() {
        loan.write_cell(HostChannel::Center, cell, &center, &mut account)?;
        loan.write_cell(HostChannel::Total, cell, &total, &mut account)?;
        loan.write_cell(HostChannel::Error, cell, &[P::ZERO; 2], &mut account)?;
    }
    let (tile, source) = match heap.handback(loan, &mut account) {
        crate::HandbackOutcome::Published(tile, receipt) => (
            tile,
            receipt
                .filled(HostChannel::Center)
                .ok_or("center")?
                .native(),
        ),
        other => return Err(format!("{other:?}").into()),
    };
    let identity =
        heap.mask_correspondence(source.layout(), source.layout(), &mut account.ledger)?;
    let demand = heap.compact_mask(identity, source, 4, 3, &mut account.ledger)?;
    let axis = heap.mask(demand, demand.layout())?.axis();
    let symbols = nrz_bindings::BindingSymbols::declare(
        &nrz_bindings::Descriptor::ratio(),
        [4, 2],
        [true; 2],
        &mut account.ledger,
    )?;
    let destination = crate::HostTileDeclaration::check(symbols, axis, coverage, &mut account)?;
    let operation = super::super::operation_map(heap.cell_entry_map(
        crate::Extent::new(3, 2)?,
        demand,
        None,
        &mut account.ledger,
    )?)?;
    Ok(Setup {
        heap,
        account,
        demand,
        request: PointwiseRequest {
            operation,
            input: tile.into_pin(),
            destination,
            demand,
            limits: crate::ExecutionLimits::default(),
        },
    })
}

/// A refused component clears its entire cell while other selected cells publish normally.
fn domains<P: Scalar + 'static>(function: Elementwise, component: usize) -> TestResult {
    let mut centers = [[P::ZERO; 2], [P::from_u8(4); 2], [P::from_u8(4); 2]];
    let mut totals = [[P::ZERO; 2]; 3];
    centers[1][component] = P::ONE;
    totals[1][component] = P::from_u8(2);
    let Setup {
        mut heap,
        mut account,
        mut request,
        demand,
    } = domain_setup(centers, totals, crate::Coverage::RepresentedInputs)?;
    operation(&mut request, function)?;
    let owner = match heap.request_projected_elementwise(request, &mut account) {
        ApplicationRequest::ApplicationReady(owner) => owner,
        other @ ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    crate::loan::fault::reset_reads();
    let owner = candidate(owner, &mut account)?;
    assert_eq!(crate::loan::fault::reads(), 12);
    let expected = match function {
        Elementwise::Reciprocal => crate::DomainReason::ZeroInDivisor,
        Elementwise::SquareRoot => crate::DomainReason::NegativeRoot,
        Elementwise::Square => return Err("domain fixture".into()),
    };
    let crate::CellOutcome::CellRefused {
        component: actual_component,
        reason:
            crate::KernelError::Domain {
                function: actual,
                entry,
                reason,
            },
    } = &owner.core.evidence.outcomes[1]
    else {
        return Err("expected target component and domain entry".into());
    };
    assert_eq!(*actual_component, component);
    // The target has four rows, so component 1 is entry 5, not source entry 4.
    assert_eq!(*entry, if component == 0 { 1 } else { 5 });
    assert_eq!(*actual, function);
    assert_eq!(*reason, expected);
    let tile = publish(&mut heap, &mut account, owner)?;
    let reader = tile.reader(&heap, &mut account)?;
    assert_eq!(
        reader.contains(0, &mut account)?,
        function == Elementwise::SquareRoot
    );
    assert!(!reader.contains(1, &mut account)?);
    assert!(reader.contains(2, &mut account)?);
    assert!(!reader.contains(3, &mut account)?);
    assert_eq!(
        reader
            .read_component(HostChannel::Center, 2, 0, &mut account)?
            .to_f64(),
        if function == Elementwise::SquareRoot {
            2.0
        } else {
            0.25
        }
    );
    drop(tile);
    heap.retire_mask(demand, &mut account.ledger)?;
    super::super::retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    empty(&mut heap, &mut account)
}

/// Both native formats authenticate projected unary origins during a later Square continuation.
/// ´test:unit:loan-projected-unary-continuations´
#[test]
fn loan_projected_unary_continuations() -> TestResult {
    for function in [Elementwise::Reciprocal, Elementwise::SquareRoot] {
        continuation::<f32>(function)?;
        continuation::<f64>(function)?;
    }
    Ok(())
}

/// A whole original interval survives both operations, including its inherited uncertainty.
fn continuation<P: Scalar + 'static>(function: Elementwise) -> TestResult {
    let Setup {
        mut heap,
        mut account,
        mut request,
        demand,
    } = super::super::setup::<P>()?;
    operation(&mut request, function)?;
    let owner = match heap.request_projected_elementwise(request, &mut account) {
        ApplicationRequest::ApplicationReady(owner) => owner,
        other @ ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    let owner = candidate(owner, &mut account)?;
    let tile = publish(&mut heap, &mut account, owner)?;
    let request =
        super::super::arithmetic::square_request(&mut heap, &mut account, tile.into_pin())?;
    let selected = request.demand;
    let owner = match heap.request_chained_pointwise_square(request, &mut account) {
        ApplicationRequest::ApplicationReady(owner) => owner,
        other @ ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    let owner = candidate(owner, &mut account)?;
    let tile = publish(&mut heap, &mut account, owner)?;
    let reader = tile.reader(&heap, &mut account)?;
    for (cell, endpoints) in [(0, [8, 14]), (2, [11, 17])] {
        let center = reader
            .read_component(HostChannel::Center, cell, 0, &mut account)?
            .to_f64();
        let total = reader
            .read_component(HostChannel::Total, cell, 0, &mut account)?
            .to_f64();
        for endpoint in endpoints {
            let expected = match function {
                Elementwise::Reciprocal => Rational::new(1, endpoint * endpoint)?,
                Elementwise::SquareRoot => Rational::new(endpoint, 1)?,
                Elementwise::Square => return Err("continuation fixture".into()),
            };
            assert!(expected.covered(center, total)?);
        }
    }
    assert!(!reader.contains(1, &mut account)?);
    assert!(!reader.contains(3, &mut account)?);
    assert_eq!(
        reader.numerical_error_scope(),
        crate::NumericalErrorScope::LocalOperation
    );
    drop(tile);
    heap.retire_mask(selected, &mut account.ledger)?;
    heap.retire_mask(demand, &mut account.ledger)?;
    super::super::retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    empty(&mut heap, &mut account)
}
