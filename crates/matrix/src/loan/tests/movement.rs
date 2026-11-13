// SPDX-License-Identifier: AGPL-3.0-only
//! Independent original-cell and nonzero companion oracles for materialized selections.

use super::{TestResult, empty, fixture, granted};
use crate::{
    ApplicationEvaluation, ApplicationPublication, ApplicationRequest, CellOutcome,
    ExecutionAccount, ExecutionLimits, HandbackOutcome, Heap, HostChannel, HostTileDeclaration,
    LoanError, MaskVersion, OwnedApplication, PointwiseRequest, PublishedTile,
};
use nrz_bindings::{BindingSymbols, Limits};
use nrz_scalar::Scalar;

mod arithmetic;
mod combined;
mod gathered_product;

/// One independently described source and branch, with an intentional original input hole.
struct Setup<P: Scalar> {
    /// Original owner registry.
    heap: Heap,
    /// Unchanged cumulative operation ledger.
    account: ExecutionAccount,
    /// Complete checked map and sole source use edge.
    request: PointwiseRequest<P>,
    /// Native branch can retire after admission captures its complete correspondence.
    demand: MaskVersion,
}

/// Original cells 1 and 4 are present; branch cell 3 is a hole and target padding is unrequested.
fn setup<P: Scalar + 'static>() -> TestResult<Setup<P>> {
    setup_selection(&[1, 3, 4], 4)
}

/// Explicit branch members and capacity independently determine complete and partial selections.
fn setup_selection<P: Scalar + 'static>(cells: &[usize], capacity: usize) -> TestResult<Setup<P>> {
    setup_layout(cells, capacity, None)
}

/// Physical source layout does not alter the independently named original branch cells.
fn setup_layout<P: Scalar + 'static>(
    cells: &[usize],
    capacity: usize,
    layout: Option<crate::Layout>,
) -> TestResult<Setup<P>> {
    let (mut heap, mut account, input) = fixture::<P>(5, 2, [true, true], Limits::default())?;
    let (tile, source) = publish_source_layout(&mut heap, &mut account, input, layout)?;
    let branch = heap.create_mask(source.layout(), cells, &mut account.ledger)?;
    let identity =
        heap.mask_correspondence(source.layout(), source.layout(), &mut account.ledger)?;
    let demand = heap.compact_mask(identity, branch, capacity, cells.len(), &mut account.ledger)?;
    heap.retire_mask(branch, &mut account.ledger)?;
    let axis = heap.mask(demand, demand.layout())?.axis();
    let symbols = BindingSymbols::declare(
        &nrz_bindings::Descriptor::ratio(),
        [u32::try_from(capacity)?, 2],
        [true, true],
        &mut account.ledger,
    )?;
    let destination = HostTileDeclaration::check(
        symbols,
        axis,
        crate::Coverage::RepresentedInputs,
        &mut account,
    )?;
    let operation = operation_map(heap.cell_entry_map(
        crate::Extent::new(5, 2)?,
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
            limits: ExecutionLimits::default(),
        },
    })
}

/// The three source channels carry distinct component values and nonzero numerical error.
fn publish_source<P: Scalar + 'static>(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    declaration: HostTileDeclaration<P>,
) -> TestResult<(PublishedTile<P>, MaskVersion)> {
    publish_source_layout(heap, account, declaration, None)
}

/// The same literal numerical tags are stored through independently selected physical strides.
fn publish_source_layout<P: Scalar + 'static>(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    declaration: HostTileDeclaration<P>,
    layout: Option<crate::Layout>,
) -> TestResult<(PublishedTile<P>, MaskVersion)> {
    let mut loan = granted(match layout {
        Some(layout) => heap.request_loan_with_layout(declaration, layout, account),
        None => heap.request_loan(declaration, account),
    })?;
    for cell in [0, 1, 4] {
        let tag = u8::try_from(cell)?;
        loan.write_cell(
            HostChannel::Center,
            cell,
            &[P::from_u8(10 + tag), P::from_u8(20 + tag)],
            account,
        )?;
        loan.write_cell(
            HostChannel::Total,
            cell,
            &[P::from_u8(3), P::from_u8(4)],
            account,
        )?;
        loan.write_cell(HostChannel::Error, cell, &[P::ONE, P::from_u8(2)], account)?;
    }
    match heap.handback(loan, account) {
        HandbackOutcome::Published(tile, receipt) => Ok((
            tile,
            receipt
                .filled(HostChannel::Center)
                .ok_or("center")?
                .native(),
        )),
        other => Err(format!("{other:?}").into()),
    }
}

/// Full immutable entry maps retain the original checked descriptor correspondence.
fn operation(rows: usize, indices: Vec<usize>) -> TestResult<crate::CheckedOperation> {
    operation_from(5, rows, indices)
}

/// Independently supplied source and target extents remain part of the checked full map.
fn operation_from(
    source_rows: usize,
    rows: usize,
    indices: Vec<usize>,
) -> TestResult<crate::CheckedOperation> {
    let map = crate::EntryMap::new(
        crate::Extent::new(source_rows, 2)?,
        crate::Extent::new(rows, 2)?,
        indices,
        crate::ShapeLimits::default(),
    )?;
    operation_map(map)
}

/// Native and explicit descriptors share the same checked shape construction.
fn operation_map(map: crate::EntryMap) -> TestResult<crate::CheckedOperation> {
    let owner = crate::kernel_testing::owner()?;
    let domain = crate::kernel_testing::axis(2)?;
    let codomain = crate::kernel_testing::axis(map.source().rows())?;
    let input = crate::kernel_testing::shape(&owner, &domain, &codomain)?;
    let output_axis = crate::kernel_testing::axis(map.target().rows())?;
    Ok(crate::OperationDraft::transform_entries(
        input,
        domain,
        output_axis,
        map,
        crate::ShapeLimits::default(),
    )?
    .check(ExecutionLimits::default())?)
}

/// Extract only a successful admission; a refusal remains diagnostic test evidence.
fn ready<P: Scalar + 'static>(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    request: PointwiseRequest<P>,
) -> TestResult<OwnedApplication<P>> {
    match heap.request_mapped_copy(request, account) {
        ApplicationRequest::ApplicationReady(owner) => Ok(owner),
        other @ ApplicationRequest::ApplicationRefused(..) => Err(format!("{other:?}").into()),
    }
}

/// Noncontiguous movement copies every original companion without resetting numerical error.
/// ´test:unit:loan-mapped-movement-original-companions´
#[test]
fn loan_mapped_movement_original_companions() -> TestResult {
    movement::<f32>()?;
    movement::<f64>()
}

/// Independent literal source rows specify the new-to-original mapping, including padding.
fn movement<P: Scalar + 'static>() -> TestResult {
    let Setup {
        mut heap,
        mut account,
        request,
        demand,
    } = setup::<P>()?;
    let owner = ready(&mut heap, &mut account, request)?;
    heap.retire_mask(demand, &mut account.ledger)?;
    assert!(matches!(
        heap.retire_cell_layout(demand.layout(), &mut account.ledger),
        Err(crate::MaskError::Heap(crate::HeapError::PinnedUses))
    ));
    super::super::fault::reset_reads();
    let owner = match owner.evaluate(&mut account) {
        ApplicationEvaluation::ApplicationCandidate(owner) => owner,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(super::super::fault::reads(), 12);
    let tile = match heap.publish_application(owner, &mut account) {
        ApplicationPublication::ApplicationPublished(tile, _) => tile,
        other => return Err(format!("{other:?}").into()),
    };
    check_values(&tile, &heap, &mut account)?;
    // Registry reconciliation can retire the original numerical source without losing its map.
    assert!(matches!(
        heap.residence(&mut account, 100_000)?,
        crate::Residence::Pending { .. }
    ));
    check_values(&tile, &heap, &mut account)?;
    drop(tile);
    retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    empty(&mut heap, &mut account)
}

/// Literal original tuples determine all six readings and both absent outcome categories.
fn check_values<P: Scalar + 'static>(
    tile: &PublishedTile<P>,
    heap: &Heap,
    account: &mut ExecutionAccount,
) -> TestResult {
    let reader = tile.reader(heap, account)?;
    assert_eq!(
        reader.numerical_error_scope(),
        crate::NumericalErrorScope::PreservedInput
    );
    for (cell, values) in [(0, [11, 21]), (2, [14, 24])] {
        for (channel, expected) in [
            (HostChannel::Center, values),
            (HostChannel::Total, [3, 4]),
            (HostChannel::Error, [1, 2]),
        ] {
            for (component, value) in expected.into_iter().enumerate() {
                assert_eq!(
                    reader.read_component(channel, cell, component, account)?,
                    P::from_u8(value)
                );
            }
        }
    }
    assert!(matches!(
        reader.application_outcome(1, account)?,
        CellOutcome::MissingInput
    ));
    assert!(matches!(
        reader.application_outcome(3, account)?,
        CellOutcome::NotRequested
    ));
    for cell in [1, 3] {
        assert!(matches!(
            reader.read_component(HostChannel::Center, cell, 0, account),
            Err(LoanError::CoverageMismatch)
        ));
    }
    Ok(())
}

/// A swapped component or changed gathered cell refuses before output allocation and loads.
/// ´test:unit:loan-mapped-movement-map-refusals´
#[test]
fn loan_mapped_movement_map_refusals() -> TestResult {
    for indices in [vec![6, 3, 4, 0, 1, 8, 9, 5], vec![4, 3, 1, 0, 9, 8, 6, 5]] {
        let Setup {
            mut heap,
            mut account,
            mut request,
            demand,
        } = setup::<f64>()?;
        request.operation = operation(4, indices)?;
        super::super::fault::reset_reads();
        let request = match heap.request_mapped_copy(request, &mut account) {
            ApplicationRequest::ApplicationRefused(
                super::super::application::ApplicationError::Loan(LoanError::LayoutMismatch),
                request,
            ) => request,
            other => return Err(format!("{other:?}").into()),
        };
        assert_eq!(super::super::fault::reads(), 0);
        assert_eq!(
            request
                .input
                .historical_reader(&mut account)?
                .read_component(HostChannel::Error, 4, 1, &mut account)?,
            2.0
        );
        drop(request);
        heap.retire_mask(demand, &mut account.ledger)?;
        retire_copy_layout(&mut heap, &mut account, demand.layout())?;
        empty(&mut heap, &mut account)?;
    }
    Ok(())
}

/// Two available cells, two components, three loads/stores and six map-law units give
/// 2*((3+3+6)*2+2)+2 words+3 tile bindings+1 admission unit = 58, without replacing the cumulative ledger.
/// ´test:unit:loan-mapped-movement-literal-traversal-boundary´
#[test]
fn loan_mapped_movement_literal_traversal_boundary() -> TestResult {
    for available in [57, 58] {
        let Setup {
            mut heap,
            mut account,
            request,
            ..
        } = setup::<f64>()?;
        let owner = ready(&mut heap, &mut account, request)?;
        let remaining = account
            .limits()
            .work
            .checked_sub(account.usage().work)
            .and_then(|work| work.checked_sub(available))
            .ok_or("remaining Work")?;
        account.ledger.work(remaining, super::super::AT)?;
        super::super::fault::reset_reads();
        let result = owner.evaluate(&mut account);
        if available == 58 {
            assert!(matches!(
                result,
                ApplicationEvaluation::ApplicationCandidate(_)
            ));
            assert_eq!(super::super::fault::reads(), 12);
        } else {
            assert!(matches!(
                result,
                ApplicationEvaluation::ApplicationNotStarted(_, _)
            ));
            assert_eq!(super::super::fault::reads(), 0);
        }
    }
    Ok(())
}

/// Repeated product cells preserve source dependence and an independently protected reader.
/// ´test:unit:loan-mapped-movement-product-and-outside-reader´
#[test]
fn loan_mapped_movement_product_and_outside_reader() -> TestResult {
    product::<f32>()?;
    product::<f64>()
}

/// Explicit repeated row indices provide a product oracle without using product projection code.
fn product<P: Scalar + 'static>() -> TestResult {
    let (
        Setup {
            mut heap,
            mut account,
            request,
            demand,
        },
        outside,
    ) = product_setup::<P>()?;
    let owner = ready(&mut heap, &mut account, request)?;
    super::super::fault::reset_reads();
    let owner = match owner.evaluate(&mut account) {
        ApplicationEvaluation::ApplicationCandidate(owner) => owner,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(super::super::fault::reads(), 36);
    let tile = match heap.publish_application(owner, &mut account) {
        ApplicationPublication::ApplicationPublished(tile, _) => tile,
        other => return Err(format!("{other:?}").into()),
    };
    check_product(&tile, &heap, &mut account)?;
    drop(tile);
    heap.retire_mask(demand, &mut account.ledger)?;
    retire_copy_layout(&mut heap, &mut account, demand.layout())?;
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
    drop(outside);
    empty(&mut heap, &mut account)
}

/// Independent original and sample axes scope the repeated cell map before output admission.
fn product_setup<P: Scalar + 'static>() -> TestResult<(Setup<P>, PublishedTile<P>)> {
    let (mut heap, mut account, input) = fixture::<P>(5, 2, [true, true], Limits::default())?;
    let (outside, mask) = publish_source(&mut heap, &mut account, input)?;
    let source_axis = heap.mask(mask, mask.layout())?.axis();
    let samples = heap.create_cell_axis(2, &mut account.ledger)?;
    let product = heap.create_cell_product(source_axis, samples, &mut account.ledger)?;
    let layout = heap.create_cell_layout(
        product.axis(),
        crate::Layout::contiguous(crate::Extent::new(10, 2)?)?,
        &mut account.ledger,
    )?;
    let demand = heap.expand_product_mask(
        product,
        crate::CellFactor::Outer,
        mask,
        layout,
        &mut account.ledger,
    )?;
    let symbols = BindingSymbols::declare(
        &nrz_bindings::Descriptor::ratio(),
        [10, 2],
        [true, true],
        &mut account.ledger,
    )?;
    let destination = HostTileDeclaration::check(
        symbols,
        product.axis(),
        crate::Coverage::RepresentedInputs,
        &mut account,
    )?;
    let request = PointwiseRequest {
        operation: operation_map(crate::EntryMap::product_cells(
            product,
            crate::CellFactor::Outer,
            2,
            crate::ShapeLimits::default(),
        )?)?,
        input: heap.retain_pin(outside.pin(), &mut account)?,
        destination,
        demand,
        limits: ExecutionLimits::default(),
    };
    Ok((
        Setup {
            heap,
            account,
            request,
            demand,
        },
        outside,
    ))
}

/// Every duplicate receives the same original centre and incoming numerical error.
fn check_product<P: Scalar + 'static>(
    tile: &PublishedTile<P>,
    heap: &Heap,
    account: &mut ExecutionAccount,
) -> TestResult {
    let reader = tile.reader(heap, account)?;
    for (cells, expected) in [([0, 1], 10), ([2, 3], 11), ([8, 9], 14)] {
        for cell in cells {
            assert_eq!(
                reader.read_component(HostChannel::Center, cell, 0, account)?,
                P::from_u8(expected)
            );
            assert_eq!(
                reader.read_component(HostChannel::Error, cell, 1, account)?,
                P::from_u8(2)
            );
        }
    }
    for cell in [4, 5, 6, 7] {
        assert!(!reader.contains(cell, account)?);
    }
    Ok(())
}

/// Failed shell allocation returns the same completed copy for repair without moving values again.
/// ´test:unit:loan-mapped-movement-publication-repair´
#[test]
fn loan_mapped_movement_publication_repair() -> TestResult {
    let Setup {
        mut heap,
        mut account,
        request,
        demand,
    } = setup::<f64>()?;
    let owner = ready(&mut heap, &mut account, request)?;
    let owner = match owner.evaluate(&mut account) {
        ApplicationEvaluation::ApplicationCandidate(owner) => owner,
        other => return Err(format!("{other:?}").into()),
    };
    let ticket = owner.ticket();
    super::super::fault::reset_stores();
    let injection = super::super::fault::Injection::after(0);
    let result = heap.publish_application(owner, &mut account);
    drop(injection);
    let owner = match result {
        ApplicationPublication::ApplicationRepair(_, owner) => owner,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(owner.ticket(), ticket);
    let tile = match heap.publish_application(owner, &mut account) {
        ApplicationPublication::ApplicationPublished(tile, _) => tile,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(super::super::fault::stores(), 0);
    check_values(&tile, &heap, &mut account)?;
    drop(tile);
    heap.retire_mask(demand, &mut account.ledger)?;
    retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    empty(&mut heap, &mut account)
}

/// Empty selected demand invokes no numerical loads or stores in either format.
/// ´test:unit:loan-mapped-movement-empty-demand´
#[test]
fn loan_mapped_movement_empty_demand() -> TestResult {
    empty_demand::<f32>()?;
    empty_demand::<f64>()
}

/// The full map is still authenticated even though every output is explicitly unrequested.
fn empty_demand<P: Scalar + 'static>() -> TestResult {
    let Setup {
        mut heap,
        mut account,
        mut request,
        demand,
    } = setup::<P>()?;
    let empty_mask = heap.create_mask(demand.layout(), &[], &mut account.ledger)?;
    request.demand = empty_mask;
    let owner = ready(&mut heap, &mut account, request)?;
    super::super::fault::reset_reads();
    super::super::fault::reset_stores();
    let owner = match owner.evaluate(&mut account) {
        ApplicationEvaluation::ApplicationCandidate(owner) => owner,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(super::super::fault::reads(), 0);
    assert_eq!(super::super::fault::stores(), 0);
    let tile = match heap.publish_application(owner, &mut account) {
        ApplicationPublication::ApplicationPublished(tile, _) => tile,
        other => return Err(format!("{other:?}").into()),
    };
    let reader = tile.reader(&heap, &mut account)?;
    for cell in 0..4 {
        assert!(matches!(
            reader.application_outcome(cell, &mut account)?,
            CellOutcome::NotRequested
        ));
        assert!(!reader.contains(cell, &mut account)?);
    }
    drop(tile);
    heap.retire_mask(empty_mask, &mut account.ledger)?;
    heap.retire_mask(demand, &mut account.ledger)?;
    retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    empty(&mut heap, &mut account)
}

/// Every compacted target is complete, including the vacuous zero-capacity target.
/// ´test:unit:loan-mapped-movement-full-and-zero-capacity´
#[test]
fn loan_mapped_movement_full_and_zero_capacity() -> TestResult {
    full::<f32>()?;
    full::<f64>()
}

/// Literal original rows 0, 1 and 4 define all available compacted values.
fn full<P: Scalar + 'static>() -> TestResult {
    for populated in [false, true] {
        let (cells, capacity) = if populated {
            (vec![0, 1, 4], 3)
        } else {
            (Vec::new(), 0)
        };
        let Setup {
            mut heap,
            mut account,
            request,
            demand,
        } = setup_selection::<P>(&cells, capacity)?;
        let owner = ready(&mut heap, &mut account, request)?;
        super::super::fault::reset_reads();
        let owner = match owner.evaluate(&mut account) {
            ApplicationEvaluation::ApplicationCandidate(owner) => owner,
            other => return Err(format!("{other:?}").into()),
        };
        assert_eq!(super::super::fault::reads(), if populated { 18 } else { 0 });
        let tile = match heap.publish_application(owner, &mut account) {
            ApplicationPublication::ApplicationPublished(tile, _) => tile,
            other => return Err(format!("{other:?}").into()),
        };
        let reader = tile.reader(&heap, &mut account)?;
        assert_eq!(reader.capacity(), capacity);
        if populated {
            for (cell, expected) in [10, 11, 14].into_iter().enumerate() {
                assert!(reader.contains(cell, &mut account)?);
                assert!(matches!(
                    reader.application_outcome(cell, &mut account)?,
                    CellOutcome::Completed
                ));
                assert_eq!(
                    reader.read_component(HostChannel::Center, cell, 0, &mut account)?,
                    P::from_u8(expected)
                );
                assert_eq!(
                    reader.read_component(HostChannel::Error, cell, 1, &mut account)?,
                    P::from_u8(2)
                );
            }
        }
        drop(tile);
        heap.retire_mask(demand, &mut account.ledger)?;
        retire_copy_layout(&mut heap, &mut account, demand.layout())?;
        empty(&mut heap, &mut account)?;
    }
    Ok(())
}

/// Changed captured layout, target axis or original operation refuses before any new observation.
/// ´test:unit:loan-mapped-movement-capture-repair´
#[test]
fn loan_mapped_movement_capture_repair() -> TestResult {
    for publication in [false, true] {
        for mutation in 0..4 {
            capture_repair(publication, mutation)?;
        }
    }
    Ok(())
}

/// Restoring the exact captured metadata makes the same owner usable without replenishing Work.
fn capture_repair(publication: bool, mutation: usize) -> TestResult {
    let Setup {
        mut heap,
        mut account,
        request,
        demand,
    } = setup::<f64>()?;
    let alternate = operation(4, vec![4, 3, 1, 0, 9, 8, 6, 5])?;
    let mut owner = ready(&mut heap, &mut account, request)?;
    if publication {
        owner = candidate(owner, &mut account)?;
    }
    let ticket = owner.ticket();
    let original_demand = owner.core.evidence.demand;
    let original_axis = owner.core.host.declaration.axis;
    let original_operation = owner.core.evidence.operation.clone();
    let original_error_scope = owner.core.evidence.error_scope;
    match mutation {
        0 => owner.core.evidence.demand.layout = owner.core.evidence.source_identity.layout,
        1 => owner.core.host.declaration.axis = owner.core.evidence.source.axis,
        2 => owner.core.evidence.operation = alternate,
        3 => owner.core.evidence.error_scope = crate::NumericalErrorScope::LocalOperation,
        _ => return Err("mutation".into()),
    }
    owner = refused_capture(owner, publication, &mut heap, &mut account)?;
    assert_eq!(owner.ticket(), ticket);
    owner.core.evidence.demand = original_demand;
    owner.core.host.declaration.axis = original_axis;
    owner.core.evidence.operation = original_operation;
    owner.core.evidence.error_scope = original_error_scope;
    if !publication {
        owner = candidate(owner, &mut account)?;
    }
    let tile = publish(&mut heap, &mut account, owner)?;
    check_values(&tile, &heap, &mut account)?;
    drop(tile);
    heap.retire_mask(demand, &mut account.ledger)?;
    retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    empty(&mut heap, &mut account)
}

/// Refusal preserves the candidate and performs no numerical or allocation action.
fn refused_capture(
    owner: OwnedApplication<f64>,
    publication: bool,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult<OwnedApplication<f64>> {
    super::super::fault::reset_reads();
    super::super::fault::reset_stores();
    let injection = super::super::fault::Injection::after(0);
    let owner = if publication {
        match heap.publish_application(owner, account) {
            ApplicationPublication::ApplicationRepair(_, owner) => owner,
            other => return Err(format!("{other:?}").into()),
        }
    } else {
        match owner.evaluate(account) {
            ApplicationEvaluation::ApplicationNotStarted(_, owner) => owner,
            other => return Err(format!("{other:?}").into()),
        }
    };
    assert_eq!(
        [
            super::super::fault::reads(),
            super::super::fault::stores(),
            super::super::fault::Injection::attempts()
        ],
        [0; 3]
    );
    drop(injection);
    Ok(owner)
}

/// Evaluate using the public state transition without manufacturing completion metadata.
fn candidate<P: Scalar + 'static>(
    owner: OwnedApplication<P>,
    account: &mut ExecutionAccount,
) -> TestResult<OwnedApplication<P>> {
    match owner.evaluate(account) {
        ApplicationEvaluation::ApplicationCandidate(owner) => Ok(owner),
        other => Err(format!("{other:?}").into()),
    }
}

/// Publish through the fallible managed handback and return the original completed owner.
fn publish<P: Scalar + 'static>(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    owner: OwnedApplication<P>,
) -> TestResult<PublishedTile<P>> {
    match heap.publish_application(owner, account) {
        ApplicationPublication::ApplicationPublished(tile, _) => Ok(tile),
        other => Err(format!("{other:?}").into()),
    }
}

/// Further movement authenticates current ancestors while historical output readings stay usable.
/// ´test:unit:loan-mapped-movement-current-lineage´
#[test]
fn loan_mapped_movement_current_lineage() -> TestResult {
    for retire_source in [false, true] {
        lineage::<f32>(retire_source)?;
        lineage::<f64>(retire_source)?;
    }
    Ok(())
}

/// A literal identity map over the compacted result preserves its original total and error twice.
fn lineage<P: Scalar + 'static>(retire_source: bool) -> TestResult {
    let Setup {
        mut heap,
        mut account,
        request,
        demand,
    } = setup::<P>()?;
    let owner = ready(&mut heap, &mut account, request)?;
    let owner = candidate(owner, &mut account)?;
    let tile = publish(&mut heap, &mut account, owner)?;
    let request = next_copy(&mut heap, &mut account, tile.into_pin())?;
    let next_demand = request.demand;
    if retire_source {
        assert!(matches!(
            heap.residence(&mut account, 100_000)?,
            crate::Residence::Pending { .. }
        ));
        super::super::fault::reset_reads();
        let injection = super::super::fault::Injection::after(0);
        let request = match heap.request_mapped_copy(request, &mut account) {
            ApplicationRequest::ApplicationRefused(
                crate::ApplicationError::Loan(LoanError::StaleDependency),
                request,
            ) => request,
            other => return Err(format!("{other:?}").into()),
        };
        assert_eq!(
            [
                super::super::fault::reads(),
                super::super::fault::Injection::attempts()
            ],
            [0; 2]
        );
        drop(injection);
        assert_eq!(
            request
                .input
                .historical_reader(&mut account)?
                .read_component(HostChannel::Error, 2, 1, &mut account)?,
            P::from_u8(2)
        );
        drop(request);
    } else {
        let owner = ready(&mut heap, &mut account, request)?;
        let owner = candidate(owner, &mut account)?;
        let tile = publish(&mut heap, &mut account, owner)?;
        check_values(&tile, &heap, &mut account)?;
        drop(tile);
    }
    heap.retire_mask(next_demand, &mut account.ledger)?;
    heap.retire_mask(demand, &mut account.ledger)?;
    retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    empty(&mut heap, &mut account)
}

/// The successor declares fresh targets on the exact original result axis and layout.
fn next_copy<P: Scalar + 'static>(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    input: crate::BundlePin<P>,
) -> TestResult<PointwiseRequest<P>> {
    let bundle = input
        .record
        .payload
        .as_ref()
        .and_then(|pin| pin.bundle.payload.as_ref())
        .ok_or("input")?;
    let axis = bundle.declaration.axis;
    let symbols = BindingSymbols::declare(
        &nrz_bindings::Descriptor::ratio(),
        [4, 2],
        [true, true],
        &mut account.ledger,
    )?;
    let destination =
        HostTileDeclaration::check(symbols, axis, crate::Coverage::RepresentedInputs, account)?;
    let demand = heap.create_mask(
        input.ticket()?.identity.layout,
        &[0, 1, 2],
        &mut account.ledger,
    )?;
    Ok(PointwiseRequest {
        operation: operation_map(heap.cell_entry_map(
            crate::Extent::new(4, 2)?,
            demand,
            None,
            &mut account.ledger,
        )?)?,
        input,
        destination,
        demand,
        limits: ExecutionLimits::default(),
    })
}

/// Interleaved and gapped source planes preserve all original mapped readings in both formats.
/// ´test:unit:loan-mapped-movement-independent-layouts´
#[test]
fn loan_mapped_movement_independent_layouts() -> TestResult {
    layouts::<f32>()?;
    layouts::<f64>()
}

/// Literal cell/component tags remain the oracle across two unrelated physical stride inventories.
fn layouts<P: Scalar + 'static>() -> TestResult {
    let extent = crate::Extent::new(5, 2)?;
    for layout in [
        crate::Layout::strided(extent, 11, 1, 2, 1)?,
        crate::Layout::strided(extent, 15, 1, 3, 1)?,
    ] {
        let Setup {
            mut heap,
            mut account,
            request,
            demand,
        } = setup_layout::<P>(&[1, 3, 4], 4, Some(layout))?;
        let owner = ready(&mut heap, &mut account, request)?;
        super::super::fault::reset_reads();
        let owner = candidate(owner, &mut account)?;
        assert_eq!(super::super::fault::reads(), 12);
        let tile = publish(&mut heap, &mut account, owner)?;
        check_values(&tile, &heap, &mut account)?;
        drop(tile);
        heap.retire_mask(demand, &mut account.ledger)?;
        retire_copy_layout(&mut heap, &mut account, demand.layout())?;
        empty(&mut heap, &mut account)?;
    }
    Ok(())
}

/// Explicit modulo maps cannot claim authenticated product correspondence in either format.
/// ´test:unit:loan-mapped-movement-product-map-refusal´
#[test]
fn loan_mapped_movement_product_map_refusal() -> TestResult {
    product_refusal::<f32>()?;
    product_refusal::<f64>()
}

/// The literal wrong map assigns product cell one to source one instead of source zero.
fn product_refusal<P: Scalar + 'static>() -> TestResult {
    let (
        Setup {
            mut heap,
            mut account,
            mut request,
            demand,
        },
        outside,
    ) = product_setup::<P>()?;
    request.operation = operation(
        10,
        vec![0, 1, 2, 3, 4, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 5, 6, 7, 8, 9],
    )?;
    super::super::fault::reset_reads();
    let injection = super::super::fault::Injection::after(0);
    let request = match heap.request_mapped_copy(request, &mut account) {
        ApplicationRequest::ApplicationRefused(
            crate::ApplicationError::Loan(LoanError::LayoutMismatch),
            request,
        ) => request,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(
        [
            super::super::fault::reads(),
            super::super::fault::Injection::attempts()
        ],
        [0; 2]
    );
    drop(injection);
    drop(request);
    drop(outside);
    heap.retire_mask(demand, &mut account.ledger)?;
    retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    empty(&mut heap, &mut account)
}

/// Release completed numerical captures before retiring their shared native correspondence.
fn retire_copy_layout(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    layout: crate::CellLayoutVersion,
) -> TestResult {
    heap.residence(account, 100_000)?;
    heap.retire_cell_layout(layout, &mut account.ledger)?;
    Ok(())
}
