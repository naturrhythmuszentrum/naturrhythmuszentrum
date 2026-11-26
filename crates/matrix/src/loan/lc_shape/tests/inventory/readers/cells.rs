// SPDX-License-Identifier: AGPL-3.0-only
//! Complete observations use independently mapped roles and a nonempty mask intersection.

mod boundaries;
mod product;
mod refusals;
use super::{Members, TestResult, fixture, setup};
use crate::{
    ExecutionAccount, Heap, LcInputCell, LcInputInventory, LcInputShape, LcShapeAxes,
    LcShapeRequest, LcShapeTile,
};
use nrz_scalar::Scalar;

/// Exact independently chosen companions, in units of 1/32; repeated L differs in every row.
const VALUES: [[[u8; 3]; 2]; 5] = [
    [[18, 2, 1], [22, 2, 1]],
    [[12, 2, 1], [20, 2, 1]],
    [[2, 1, 1], [4, 1, 1]],
    [[32, 0, 0], [32, 0, 0]],
    [[20, 1, 1], [21, 1, 0]],
];
fn source<P: Scalar + 'static>(
    schema: &LcInputShape<P>,
    axes: &LcShapeAxes,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    shift: usize,
    values: &[[[P; 3]; 2]; 5],
) -> TestResult<crate::PublishedTile<P>> {
    let mut loan = fixture::granted(heap.request_loan(schema.source(axes, account)?, account))?;
    for (cell, values) in values[schema.input_index()].into_iter().enumerate() {
        for (channel, value) in crate::HostChannel::ALL.into_iter().zip(values) {
            if cell != 1 || channel != crate::HostChannel::Error || shift == 1 {
                loan.write_component(channel, cell, 0, value, account)?;
            }
        }
    }
    match heap.handback(loan, account) {
        crate::HandbackOutcome::Published(tile, _) => Ok(tile),
        other => Err(format!("{other:?}").into()),
    }
}

/// Every sparse role contains 0,3,11; each also has distinct holes.
const SELECTED: [[usize; 5]; 5] = [
    [0, 1, 3, 5, 11],
    [0, 2, 3, 6, 11],
    [0, 3, 4, 7, 11],
    [0, 3, 6, 9, 11],
    [0, 3, 5, 8, 11],
];
fn publish<'a, P: Scalar + 'static>(
    inventory: &'a LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    cells: &[Vec<usize>; 5],
    shift: usize,
) -> TestResult<Members<'a, P>> {
    let values = VALUES.map(|rows| rows.map(|row| row.map(|n| P::from_u8(n) / P::from_u8(32))));
    publish_values(inventory, axes, heap, account, cells, shift, &values)
}
pub(in crate::loan::lc_shape::tests) fn publish_values<'a, P: Scalar + 'static>(
    inventory: &'a LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    cells: &[Vec<usize>; 5],
    shift: usize,
    values: &[[[P; 3]; 2]; 5],
) -> TestResult<Members<'a, P>> {
    let mut tiles = Vec::new();
    let mut layouts = Vec::new();
    for (index, cells) in cells.iter().enumerate() {
        let schema = inventory.input(index).ok_or("original role")?;
        let (tile, layout) = member(schema, axes, heap, account, cells, shift, values)?;
        tiles.push(tile);
        layouts.push(layout);
    }
    Ok(Members { tiles, layouts })
}
fn member<'a, P: Scalar + 'static>(
    schema: &'a LcInputShape<P>,
    axes: &LcShapeAxes,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    cells: &[usize],
    shift: usize,
    values: &[[[P; 3]; 2]; 5],
) -> TestResult<(LcShapeTile<'a, P>, crate::CellLayoutVersion)> {
    let input = source(schema, axes, heap, account, shift, values)?;
    mapped(schema, axes, heap, account, cells, input)
}
/// Map an independently published or repaired original source through the same transport.
fn mapped<'a, P: Scalar + 'static>(
    schema: &'a LcInputShape<P>,
    axes: &LcShapeAxes,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    cells: &[usize],
    input: crate::PublishedTile<P>,
) -> TestResult<(LcShapeTile<'a, P>, crate::CellLayoutVersion)> {
    let index = schema.input_index();
    let (length, offset, stride) = [
        (32, 24, -2),
        (40, 1, 3),
        (16, 2, 1),
        (16, 13, -1),
        (24, 0, 2),
    ][index];
    let layout = heap.create_cell_layout(
        axes.combined.axis(),
        crate::Layout::strided(crate::Extent::new(12, 1)?, length, offset, stride, 1)?,
        &mut account.ledger,
    )?;
    let demand = heap.create_mask(layout, cells, &mut account.ledger)?;
    let map = heap.cell_entry_map(
        crate::Extent::new(2, 1)?,
        demand,
        Some((schema.projection(axes, account)?, crate::CellFactor::Outer)),
        &mut account.ledger,
    )?;
    let owner = crate::conversion_testing::owner()?;
    let quantity = schema.question().relation().inputs()[index].descriptor();
    let input_shape = crate::conversion_testing::shape(&owner, quantity, 2)?;
    let output_shape = crate::conversion_testing::shape(&owner, quantity, 12)?;
    let operation = crate::OperationDraft::map_cell_copy(input_shape, output_shape, map)?
        .check(crate::ExecutionLimits::default())?;
    let request = crate::PointwiseRequest {
        operation,
        input: input.into_pin(),
        destination: schema.destination(axes, account)?,
        demand,
        limits: crate::ExecutionLimits::default(),
    };
    let mut application = match schema.request(heap, axes, request, account) {
        LcShapeRequest::Ready(owner) => owner,
        other @ LcShapeRequest::Refused(..) => return Err(format!("{other:?}").into()),
    };
    application.evaluate(account)?;
    let tile = application.publish(heap, account)?;
    drop(application);
    heap.retire_mask(demand, &mut account.ledger)?;
    Ok((tile, layout))
}

/// Five independent strided layouts preserve exact original companions and the literal
/// joint mask [0,3,11], including source holes, fresh occupations and outside pins.
/// ´test:unit:loan-lc-input-cells-original-images´
#[test]
fn loan_lc_input_cells_original_images() -> TestResult {
    images::<f32>()?;
    images::<f64>()
}
fn images<P: Scalar + 'static>() -> TestResult {
    let (mut heap, mut account, inventory, axes) = setup::<P>()?;
    for mode in 0..3 {
        for shift in [0, 1] {
            let masks = std::array::from_fn(|index| match mode {
                0 => vec![],
                1 => SELECTED[index].to_vec(),
                _ => (0..12).collect(),
            });
            let mut members = publish(&inventory, &axes, &mut heap, &mut account, &masks, shift)?;
            let mut outside = Vec::new();
            for tile in &members.tiles {
                outside.push(tile.retain(&mut heap, &mut account)?);
            }
            members.tiles = outside;
            assert!(matches!(
                heap.residence(&mut account, 100_000)?,
                crate::Residence::Pending { .. }
            ));
            let references = members.references();
            let readers = inventory.readers(&heap, &references, &mut account)?;
            crate::loan::fault::reset_reads();
            let injection = crate::loan::fault::Injection::after(0);
            let cells = readers.into_cells(&mut account)?;
            assert_eq!(crate::loan::fault::reads(), 0);
            observe(&cells, &inventory, &axes, &mut account, mode, shift)?;
            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
            drop(injection);
            drop(references);
            members.finish(&mut heap, &mut account)?;
        }
    }
    Ok(())
}
fn oracle<P: Scalar>(cell: &LcInputCell<'_, P, 5>, instance: usize) -> TestResult {
    use crate::loan::application::tests::binary::oracle::Rational;
    for (reading, values) in cell.readings().iter().zip(VALUES) {
        assert_eq!(
            reading.numerical_error_scope(),
            crate::NumericalErrorScope::HostInput
        );
        for (value, numerator) in reading.values().into_iter().zip(values[instance]) {
            assert_eq!(
                Rational::binary(value.to_f64())?
                    .compare(&Rational::fraction(i64::from(numerator), 32)?),
                std::cmp::Ordering::Equal
            );
        }
    }
    Ok(())
}

fn observe<P: Scalar + 'static>(
    cells: &crate::LcInputCells<'_, P, 5>,
    inventory: &LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    account: &mut ExecutionAccount,
    mode: usize,
    shift: usize,
) -> TestResult {
    for (cell, instance) in [0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1].into_iter().enumerate() {
        crate::loan::fault::reset_reads();
        let full = mode == 2;
        let sparse = mode == 1;
        let intersection = [0, 3, 11].contains(&cell);
        let demanded = full || (sparse && intersection);
        let source_present = instance == 0 || shift == 1;
        let complete = demanded && source_present;
        let result = cells.read_cell(cell, account);
        if complete {
            let result = result?;
            assert!(std::ptr::eq(
                result.inventory(),
                std::ptr::from_ref(inventory)
            ));
            assert_eq!(result.axes(), *axes);
            assert_eq!(result.cell(), cell);
            oracle(&result, instance)?;
            assert_eq!(crate::loan::fault::reads(), 15);
        } else {
            assert!(matches!(result, Err(crate::LoanError::CoverageMismatch)));
            assert_eq!(crate::loan::fault::reads(), 0);
        }
    }
    Ok(())
}

/// Current original companions supply the two exact native containment revisions.
/// ´test:unit:loan-lc-native-containment-original-inputs´
#[test]
fn loan_lc_native_containment_original_inputs() -> TestResult {
    containment32()?;
    containment64()
}

fn containment32() -> TestResult {
    use nrz_bindings::{LcArithmeticMode, LcFace, LcStratum};
    let (mut heap, mut account, inventory, axes) = setup::<f32>()?;
    let members = publish(
        &inventory,
        &axes,
        &mut heap,
        &mut account,
        &SELECTED.map(Vec::from),
        1,
    )?;
    let references = members.references();
    let readers = inventory
        .readers(&heap, &references, &mut account)?
        .into_cells(&mut account)?;
    let cell = readers.read_cell(0, &mut account)?;
    let reading = &cell.readings()[0];
    let lower = 0.5_f32;
    let upper = 0.625_f32;
    let stratum = LcStratum {
        lower: Some(LcFace {
            exact: nrz_bindings::Rational::from_ratio(1, 2, &mut account.ledger)?,
            inward: lower,
            open: false,
        }),
        upper: Some(LcFace {
            exact: nrz_bindings::Rational::from_ratio(5, 8, &mut account.ledger)?,
            inward: upper,
            open: false,
        }),
    };
    assert!(reading.check_containment32(
        [lower, upper],
        &stratum,
        LcArithmeticMode::REQUIRED,
        &mut account.ledger
    )?);
    let strict = LcStratum {
        lower: Some(LcFace {
            open: true,
            ..stratum.lower.ok_or("lower")?
        }),
        upper: None,
    };
    assert!(!reading.check_containment32(
        [lower, upper],
        &strict,
        LcArithmeticMode::REQUIRED,
        &mut account.ledger
    )?);
    Ok(())
}

fn containment64() -> TestResult {
    use nrz_bindings::{LcArithmeticMode, LcFace, LcStratum};
    let (mut heap, mut account, inventory, axes) = setup::<f64>()?;
    let members = publish(
        &inventory,
        &axes,
        &mut heap,
        &mut account,
        &SELECTED.map(Vec::from),
        1,
    )?;
    let references = members.references();
    let readers = inventory
        .readers(&heap, &references, &mut account)?
        .into_cells(&mut account)?;
    let cell = readers.read_cell(0, &mut account)?;
    let reading = &cell.readings()[0];
    let lower = 0.5_f64;
    let upper = 0.625_f64;
    let stratum = LcStratum {
        lower: Some(LcFace {
            exact: nrz_bindings::Rational::from_ratio(1, 2, &mut account.ledger)?,
            inward: lower,
            open: false,
        }),
        upper: Some(LcFace {
            exact: nrz_bindings::Rational::from_ratio(5, 8, &mut account.ledger)?,
            inward: upper,
            open: false,
        }),
    };
    assert!(reading.check_containment64(
        [lower, upper],
        &stratum,
        LcArithmeticMode::REQUIRED,
        &mut account.ledger
    )?);
    let strict = LcStratum {
        lower: None,
        upper: Some(LcFace {
            open: true,
            ..stratum.upper.ok_or("upper")?
        }),
    };
    assert!(!reading.check_containment64(
        [lower, upper],
        &strict,
        LcArithmeticMode::REQUIRED,
        &mut account.ledger
    )?);
    Ok(())
}
