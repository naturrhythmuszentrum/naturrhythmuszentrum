// SPDX-License-Identifier: AGPL-3.0-only
//! Projected arithmetic binds original companions once and never creates a copied input tile.

use super::{Setup, TestResult, candidate, empty, publish};
use crate::{ApplicationEvaluation, ApplicationRequest, HostChannel, PointwiseRequest};
use nrz_scalar::Scalar;

mod boundaries;
mod continuation;
mod observation;
mod unary;

/// Both formats directly consume gathered and repeated-product source cells.
/// ´test:unit:loan-projected-square-native-consumers´
#[test]
fn loan_projected_square_native_consumers() -> TestResult {
    direct::<f32>(false)?;
    direct::<f64>(false)?;
    direct::<f32>(true)?;
    direct::<f64>(true)
}

/// The original operation supplies frozen shapes; only its native map is reused.
fn square<P: Scalar>(request: &mut PointwiseRequest<P>) -> TestResult {
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
        crate::Elementwise::Square,
    )?
    .check(crate::ExecutionLimits::default())?;
    assert!(!request.operation.carries_statistics());
    Ok(())
}

/// Literal small-integer input intervals give exact centres, endpoint extrema and local errors.
fn direct<P: Scalar + 'static>(product: bool) -> TestResult {
    let (setup, outside) = if product {
        let (setup, outside) = super::product_setup::<P>()?;
        (setup, Some(outside))
    } else {
        (super::setup::<P>()?, None)
    };
    let Setup {
        mut heap,
        mut account,
        mut request,
        demand,
    } = setup;
    square(&mut request)?;
    let owner = match heap.request_projected_elementwise(request, &mut account) {
        ApplicationRequest::ApplicationReady(owner) => owner,
        other @ ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    super::super::super::fault::reset_reads();
    let owner = candidate(owner, &mut account)?;
    assert_eq!(
        super::super::super::fault::reads(),
        if product { 24 } else { 8 }
    );
    let tile = publish(&mut heap, &mut account, owner)?;
    let reader = tile.reader(&heap, &mut account)?;
    let expected = if product {
        vec![
            (0, 100.0, 49.0, 169.0),
            (1, 100.0, 49.0, 169.0),
            (2, 121.0, 64.0, 196.0),
            (8, 196.0, 121.0, 289.0),
        ]
    } else {
        vec![(0, 121.0, 64.0, 196.0), (2, 196.0, 121.0, 289.0)]
    };
    for (cell, center, lower, upper) in expected {
        let actual = reader
            .read_component(HostChannel::Center, cell, 0, &mut account)?
            .to_f64();
        let total = reader
            .read_component(HostChannel::Total, cell, 0, &mut account)?
            .to_f64();
        assert_eq!(actual, center);
        assert!(actual - total <= lower);
        assert!(actual + total >= upper);
        assert_eq!(
            reader
                .read_component(HostChannel::Error, cell, 0, &mut account)?
                .to_f64(),
            super::arithmetic::local_error::<P>(center)
        );
    }
    assert_eq!(
        reader.numerical_error_scope(),
        crate::NumericalErrorScope::LocalOperation
    );
    drop(tile);
    if let Some(outside) = &outside {
        assert!(matches!(
            heap.residence(&mut account, 100_000)?,
            crate::Residence::Pending { .. }
        ));
        assert_eq!(
            outside.reader(&heap, &mut account)?.read_component(
                HostChannel::Error,
                4,
                1,
                &mut account
            )?,
            P::from_u8(2)
        );
    }
    drop(outside);
    heap.retire_mask(demand, &mut account.ledger)?;
    super::retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    empty(&mut heap, &mut account)
}

/// Two cells times two components times (two loads + three stores + twelve invocation units),
/// plus four cell visits, three bindings, two word visits and one entry unit equals 78.
/// ´test:unit:loan-projected-square-literal-work-boundary´
#[test]
fn loan_projected_square_literal_work_boundary() -> TestResult {
    for available in [77, 78] {
        let Setup {
            mut heap,
            mut account,
            mut request,
            ..
        } = super::setup::<f64>()?;
        square(&mut request)?;
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
        account.ledger.work(spend, super::super::super::AT)?;
        super::super::super::fault::reset_reads();
        let result = owner.evaluate(&mut account);
        if available == 78 {
            assert!(matches!(
                result,
                ApplicationEvaluation::ApplicationCandidate(_)
            ));
            assert_eq!(super::super::super::fault::reads(), 8);
        } else {
            assert!(matches!(
                result,
                ApplicationEvaluation::ApplicationNotStarted(..)
            ));
            assert_eq!(super::super::super::fault::reads(), 0);
        }
    }
    Ok(())
}

/// Complete and empty gathered demand retain native mapping with no invented input coverage.
/// ´test:unit:loan-projected-square-full-and-empty´
#[test]
fn loan_projected_square_full_and_empty() -> TestResult {
    coverage::<f32>()?;
    coverage::<f64>()
}

/// Three original cells are filled; an empty demand performs no arithmetic or numerical loads.
fn coverage<P: Scalar + 'static>() -> TestResult {
    coverage_function::<P>(crate::Elementwise::Square)
}

/// Coverage depends on the selected original cells, independently of the unary law.
fn coverage_function<P: Scalar + 'static>(function: crate::Elementwise) -> TestResult {
    for populated in [false, true] {
        let Setup {
            mut heap,
            mut account,
            mut request,
            demand,
        } = super::setup_selection::<P>(&[0, 1, 4], 3)?;
        let selected = if populated {
            demand
        } else {
            heap.create_mask(demand.layout(), &[], &mut account.ledger)?
        };
        request.demand = selected;
        unary::operation(&mut request, function)?;
        let owner = match heap.request_projected_elementwise(request, &mut account) {
            ApplicationRequest::ApplicationReady(owner) => owner,
            other @ ApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        };
        super::super::super::fault::reset_reads();
        let owner = candidate(owner, &mut account)?;
        assert_eq!(
            super::super::super::fault::reads(),
            if populated { 12 } else { 0 }
        );
        let tile = publish(&mut heap, &mut account, owner)?;
        let reader = tile.reader(&heap, &mut account)?;
        for cell in 0..3 {
            assert_eq!(reader.contains(cell, &mut account)?, populated);
        }
        drop(tile);
        if !populated {
            heap.retire_mask(selected, &mut account.ledger)?;
        }
        heap.retire_mask(demand, &mut account.ledger)?;
        super::retire_copy_layout(&mut heap, &mut account, demand.layout())?;
        empty(&mut heap, &mut account)?;
    }
    Ok(())
}

/// A substituted target axis refuses before the first numerical observation or allocation.
/// ´test:unit:loan-projected-square-axis-refusal´
#[test]
fn loan_projected_square_axis_refusal() -> TestResult {
    let Setup {
        mut heap,
        mut account,
        mut request,
        demand,
    } = super::setup::<f64>()?;
    square(&mut request)?;
    let axis = request.destination.axis;
    request.destination.axis = heap.create_cell_axis(4, &mut account.ledger)?;
    super::super::super::fault::reset_reads();
    let injection = super::super::super::fault::Injection::after(0);
    let mut request = match heap.request_projected_elementwise(request, &mut account) {
        ApplicationRequest::ApplicationRefused(
            crate::ApplicationError::Loan(crate::LoanError::LayoutMismatch),
            request,
        ) => request,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(
        [
            super::super::super::fault::reads(),
            super::super::super::fault::Injection::attempts()
        ],
        [0; 2]
    );
    drop(injection);
    request.destination.axis = axis;
    let owner = match heap.request_projected_elementwise(request, &mut account) {
        ApplicationRequest::ApplicationReady(owner) => owner,
        other @ ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    let owner = candidate(owner, &mut account)?;
    let tile = publish(&mut heap, &mut account, owner)?;
    drop(tile);
    heap.retire_mask(demand, &mut account.ledger)?;
    super::retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    empty(&mut heap, &mut account)
}

/// Native padding has no dense numerical source and refuses dense scratch admission.
/// ´test:unit:loan-native-padding-dense-refusal´
#[test]
fn loan_native_padding_dense_refusal() -> TestResult {
    for padded in [false, true] {
        let Setup {
            mut heap,
            mut account,
            mut request,
            demand,
        } = if padded {
            super::setup::<f64>()?
        } else {
            super::setup_selection::<f64>(&[0, 1, 4], 3)?
        };
        for projected in [false, true] {
            if projected {
                square(&mut request)?;
            }
            let crate::binding::NativeRecipe::Kernel(law) = &request.operation.recipe else {
                return Err("kernel".into());
            };
            let result = law.physical_scratch::<f64, f64>(0);
            if padded {
                assert!(matches!(
                    result,
                    Err(crate::OperationError::Kernel(crate::KernelError::Contract))
                ));
            } else {
                assert!(result.is_ok());
            }
        }
        drop(request);
        heap.retire_mask(demand, &mut account.ledger)?;
        super::retire_copy_layout(&mut heap, &mut account, demand.layout())?;
        empty(&mut heap, &mut account)?;
    }
    Ok(())
}

/// Padding remains absent in every component and cannot escape through dense composition.
/// ´test:unit:loan-native-padding-map-refusal´
#[test]
fn loan_native_padding_map_refusal() -> TestResult {
    let Setup {
        mut heap,
        mut account,
        request,
        demand,
    } = super::setup::<f64>()?;
    assert!(!request.operation.carries_statistics());
    let map = heap.cell_entry_map(crate::Extent::new(5, 2)?, demand, None, &mut account.ledger)?;
    assert_eq!(map.source_index(0), Some(1));
    assert_eq!(map.source_index(2), Some(4));
    assert_eq!(map.source_index(3), None);
    assert_eq!(map.source_index(4), Some(6));
    assert_eq!(map.source_index(7), None);
    assert!(matches!(
        map.indices(),
        Err(crate::ShapeError::UnmappedEntries)
    ));
    let limits = crate::ShapeLimits::default();
    let identity = crate::EntryMap::reshape(map.target(), map.target(), limits)?;
    assert_eq!(
        map.compose(&identity, limits),
        Err(crate::ShapeError::UnmappedEntries)
    );
    let source_identity = crate::EntryMap::reshape(map.source(), map.source(), limits)?;
    assert_eq!(
        source_identity.compose(&map, limits),
        Err(crate::ShapeError::UnmappedEntries)
    );
    let output = request.operation.outputs()[0].shape();
    assert!(matches!(
        crate::OperationDraft::transform_entries(
            request.operation.inputs()[0].shape().clone(),
            output.domain().clone(),
            output.codomain().clone(),
            map,
            limits,
        ),
        Err(crate::OperationError::Shape(
            crate::ShapeError::UnmappedEntries
        ))
    ));
    drop(request);
    heap.retire_mask(demand, &mut account.ledger)?;
    super::retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    empty(&mut heap, &mut account)
}

/// Constructor refusals precede every companion read, even for a valid native selection.
/// ´test:unit:loan-native-copy-constructor-refusals´
#[test]
fn loan_native_copy_constructor_refusals() -> TestResult {
    let Setup {
        mut heap,
        mut account,
        request,
        demand,
    } = super::setup::<f64>()?;
    let owner = crate::kernel_testing::owner()?;
    let ratio = nrz_bindings::Descriptor::ratio();
    let input = copy_shape(&owner, &ratio, 5)?;
    let output = copy_shape(&owner, &ratio, 4)?;
    let map = heap.cell_entry_map(crate::Extent::new(5, 2)?, demand, None, &mut account.ledger)?;
    let explicit = crate::EntryMap::new(
        input.extent(),
        output.extent(),
        vec![1, 3, 4, 0, 6, 8, 9, 5],
        crate::ShapeLimits::default(),
    )?;
    let wrong_extent = copy_shape(&owner, &ratio, 3)?;
    let quantity = nrz_bindings::Descriptor::new(
        nrz_bindings::Kind::Named(
            "length".into(),
            Box::new(nrz_bindings::Dimension::integers([1, 0, 0, 0, 0, 0, 0])?),
        ),
        nrz_bindings::Measure::coherent(),
        nrz_bindings::Role::Difference,
    )?;
    let wrong_descriptor = copy_shape(&owner, &quantity, 4)?;
    let foreign = copy_shape(&crate::kernel_testing::owner()?, &ratio, 4)?;
    for (target, candidate, snapshot) in [
        (output, explicit, false),
        (wrong_extent, map.clone(), false),
        (wrong_descriptor, map.clone(), false),
        (foreign, map.clone(), true),
    ] {
        super::super::super::fault::reset_reads();
        let result = crate::OperationDraft::map_cell_copy(input.clone(), target, candidate);
        if snapshot {
            assert!(matches!(
                result,
                Err(crate::OperationError::Shape(
                    crate::ShapeError::SnapshotMismatch
                ))
            ));
        } else {
            assert!(matches!(
                result,
                Err(crate::OperationError::Kernel(crate::KernelError::Contract))
            ));
        }
        assert_eq!(super::super::super::fault::reads(), 0);
    }
    drop(map);
    drop(request);
    heap.retire_mask(demand, &mut account.ledger)?;
    super::retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    empty(&mut heap, &mut account)
}

/// Duplicate a homogeneous column so the fixture matches the native two-component layout.
fn copy_shape(
    owner: &std::rc::Rc<nrz_bindings::CheckedDeclarations>,
    quantity: &nrz_bindings::Descriptor,
    rows: usize,
) -> TestResult<crate::CheckedShape> {
    let input = crate::conversion_testing::shape(owner, quantity, rows)?;
    let limits = crate::ShapeLimits::default();
    let map = crate::EntryMap::new(
        input.extent(),
        crate::Extent::new(rows, 2)?,
        (0..rows).chain(0..rows).collect(),
        limits,
    )?;
    Ok(input.select(
        crate::kernel_testing::axis(2)?,
        input.codomain().clone(),
        map,
        limits,
    )?)
}
