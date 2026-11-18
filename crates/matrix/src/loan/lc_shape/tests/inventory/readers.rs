// SPDX-License-Identifier: AGPL-3.0-only
//! Original positional roles are read only through one complete authenticated inventory.

mod boundaries;
mod cells;
mod refusals;
use super::{TestResult, VALUES, fixture, setup, source};
use crate::loan::application::tests::binary::oracle::Rational;
use crate::{
    ExecutionAccount, Heap, HostChannel, LcInputInventory, LcInputReaders, LcInputShape,
    LcShapeAxes, LcShapeRequest, LcShapeTile,
};
use nrz_scalar::Scalar;

/// Independent tuple selections deliberately differ between original roles.
const SELECTED: [[usize; 5]; 5] = [
    [0, 3, 5, 8, 11],
    [0, 2, 5, 6, 10],
    [1, 3, 4, 7, 8],
    [0, 1, 4, 6, 9],
    [2, 3, 5, 9, 11],
];
pub(in crate::loan::lc_shape::tests) struct Members<'a, P: Scalar> {
    tiles: Vec<LcShapeTile<'a, P>>,
    layouts: Vec<crate::CellLayoutVersion>,
}
impl<'i, P: Scalar> Members<'i, P> {
    pub(in crate::loan::lc_shape::tests) fn detach(self) -> Vec<crate::CellLayoutVersion> {
        drop(self.tiles);
        self.layouts
    }
    pub(in crate::loan::lc_shape::tests) fn references(&self) -> Vec<&LcShapeTile<'i, P>> {
        self.tiles.iter().collect()
    }
    pub(in crate::loan::lc_shape::tests) fn finish(
        self,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> TestResult {
        drop(self.tiles);
        for layout in self.layouts {
            fixture::retire_layout(heap, account, layout)?;
        }
        fixture::empty(heap, account)
    }
}
/// All original schema members exist before the first numerical publication.
pub(in crate::loan::lc_shape::tests) fn publish<'a, P: Scalar + 'static>(
    inventory: &'a LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    mode: usize,
    shift: usize,
) -> TestResult<Members<'a, P>> {
    let mut tiles = Vec::new();
    let mut layouts = Vec::new();
    for (index, selected) in SELECTED.iter().enumerate() {
        let cells = match mode {
            0 => vec![],
            1 => selected.to_vec(),
            2 => (0..12).collect(),
            _ => return Err("coverage mode".into()),
        };
        let schema = inventory.input(index).ok_or("original member")?;
        let (tile, layout) = member(schema, axes, heap, account, &cells, shift)?;
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
) -> TestResult<(LcShapeTile<'a, P>, crate::CellLayoutVersion)> {
    let input = source(schema, axes, heap, account, shift)?;
    let candidate = fixture::request(schema, axes, input.into_pin(), cells, heap, account)?;
    let demand = candidate.demand;
    let mut application = match schema.request(heap, axes, candidate, account) {
        LcShapeRequest::Ready(owner) => owner,
        other @ LcShapeRequest::Refused(..) => return Err(format!("{other:?}").into()),
    };
    application.evaluate(account)?;
    let tile = application.publish(heap, account)?;
    drop(application);
    heap.retire_mask(demand, &mut account.ledger)?;
    Ok((tile, demand.layout()))
}

/// Exact original-support values and an independently enumerated tuple map survive a complete
/// current-reader set, sparse/empty/full demand, source holes and protected outside member pins.
/// ´test:unit:loan-lc-inventory-readers-original-images´
#[test]
fn loan_lc_inventory_readers_original_images() -> TestResult {
    images::<f32>()?;
    images::<f64>()
}
fn images<P: Scalar + 'static>() -> TestResult {
    let (mut heap, mut account, inventory, axes) = setup::<P>()?;
    for index in 0..5 {
        super::original_support(
            inventory.input(index).ok_or("original member")?,
            index,
            &mut account,
        )?;
    }
    for mode in 0..3 {
        for shift in [0, 1] {
            let mut members = publish(&inventory, &axes, &mut heap, &mut account, mode, shift)?;
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
            crate::loan::fault::reset_reads();
            let injection = crate::loan::fault::Injection::after(0);
            let readers = inventory.readers(&heap, &references, &mut account)?;
            assert_eq!(
                [
                    crate::loan::fault::reads(),
                    crate::loan::fault::Injection::attempts()
                ],
                [0; 2]
            );
            drop(injection);
            assert!(std::ptr::eq(readers.inventory(), &raw const inventory));
            assert_eq!(readers.axes(), axes);
            assert!(readers.reader(5).is_none());
            oracle(&readers, mode, shift, &mut account)?;
            drop(references);
            members.finish(&mut heap, &mut account)?;
        }
    }
    Ok(())
}
fn oracle<P: Scalar + 'static>(
    readers: &LcInputReaders<'_, P, 5>,
    mode: usize,
    shift: usize,
    account: &mut ExecutionAccount,
) -> TestResult {
    for index in 0..5 {
        let reader = readers.reader(index).ok_or("complete reader")?;
        for (cell, instance) in [0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1].into_iter().enumerate() {
            let full = mode == 2;
            let sparse = mode == 1;
            let demanded = SELECTED[index].contains(&cell);
            let selected = full || (sparse && demanded);
            let available = instance == 0 || shift == 1;
            let filled = selected && available;
            assert_eq!(reader.contains(cell, account)?, filled);
            if filled {
                for (channel, numerator) in
                    HostChannel::ALL.into_iter().zip(VALUES[index][instance])
                {
                    let value = reader.read_component(channel, cell, 0, account)?;
                    assert_eq!(
                        Rational::binary(value.to_f64())?
                            .compare(&Rational::fraction(i64::from(numerator), 32)?),
                        std::cmp::Ordering::Equal
                    );
                }
            } else {
                let before = crate::loan::fault::reads();
                assert!(matches!(
                    reader.read_component(HostChannel::Center, cell, 0, account),
                    Err(crate::LoanError::CoverageMismatch)
                ));
                assert_eq!(crate::loan::fault::reads(), before);
            }
        }
    }
    Ok(())
}
