// SPDX-License-Identifier: AGPL-3.0-only
//! Complete original inventories preserve each independent positional transport capability.

mod boundaries;
pub(super) mod readers;
mod refusals;
use super::fixture::{self, TestResult};
use crate::loan::application::tests::binary::oracle::Rational;
use crate::{
    ExecutionAccount, Heap, HostChannel, LcInputInventory, LcInputShape, LcShapeAxes,
    LcShapeRequest,
};
use nrz_bindings::{Budget, LcQuantity, Limits};
use nrz_scalar::Scalar;

/// Literal original roles, including a second L constraint that is not a new physical input.
const ROLES: [LcQuantity; 5] = [
    LcQuantity::Inductance,
    LcQuantity::Capacitance,
    LcQuantity::InitialPhase,
    LcQuantity::Energy,
    LcQuantity::Inductance,
];
/// Exact source centers, total radii and errors in units of 1/32, independently chosen
/// inside the original supports L,C=[1/4,1], alpha=[0,1/4], E=1 and repeated L=[1/4,49/64].
const VALUES: [[[u8; 3]; 2]; 5] = [
    [[18, 2, 1], [22, 2, 1]],
    [[12, 2, 1], [20, 2, 1]],
    [[2, 1, 1], [4, 1, 1]],
    [[32, 0, 0], [32, 0, 0]],
    [[18, 2, 1], [22, 2, 1]],
];

/// Original role enumeration and exact dyadic images cover sparse/empty/full demands and reuse.
/// ´test:unit:loan-lc-input-inventory-original-images´
#[test]
fn loan_lc_input_inventory_original_images() -> TestResult {
    images::<f32>()?;
    images::<f64>()
}
fn images<P: Scalar + 'static>() -> TestResult {
    let (mut heap, mut account, inventory, axes) = setup::<P>()?;
    assert!(inventory.input(5).is_none());
    for (index, role) in ROLES.into_iter().enumerate() {
        let schema = inventory
            .input(index)
            .ok_or("missing original constraint")?;
        assert_eq!(schema.input_index(), index);
        assert_eq!(
            schema.question().relation().inputs()[index].quantity(),
            role
        );
        assert_eq!(schema.question().time().quantity(), LcQuantity::Time(0));
        original_support(schema, index, &mut account)?;
        for cells in [vec![], vec![0, 3, 5, 8, 11], (0..12).collect()] {
            for shift in [0, 1] {
                let input = source(schema, &axes, &mut heap, &mut account, shift)?;
                let candidate = fixture::request(
                    schema,
                    &axes,
                    input.into_pin(),
                    &cells,
                    &mut heap,
                    &mut account,
                )?;
                let demand = candidate.demand;
                let mut application =
                    match schema.request(&mut heap, &axes, candidate, &mut account) {
                        LcShapeRequest::Ready(owner) => owner,
                        other @ LcShapeRequest::Refused(..) => {
                            return Err(format!("{other:?}").into());
                        }
                    };
                application.evaluate(&mut account)?;
                let result = application.publish(&mut heap, &mut account)?;
                drop(application);
                let outside = result.retain(&mut heap, &mut account)?;
                drop(result);
                heap.retire_mask(demand, &mut account.ledger)?;
                assert!(matches!(
                    heap.residence(&mut account, 100_000)?,
                    crate::Residence::Pending { .. }
                ));
                assert_eq!(outside.schema().input_index(), index);
                assert_eq!(outside.axes(), axes);
                oracle(&outside, &cells, index, shift, &mut account)?;
                drop(outside);
                fixture::retire_layout(&mut heap, &mut account, demand.layout())?;
                fixture::empty(&mut heap, &mut account)?;
            }
        }
    }
    Ok(())
}
fn oracle<P: Scalar + 'static>(
    tile: &crate::LcShapeTile<'_, P>,
    cells: &[usize],
    index: usize,
    shift: usize,
    account: &mut ExecutionAccount,
) -> TestResult {
    let reader = tile.reader(account)?;
    // Original tuple enumeration: two instances, each with three samples and two branches.
    for (cell, instance) in [0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1].into_iter().enumerate() {
        let available = instance == 0 || shift == 1;
        let filled = cells.contains(&cell) && available;
        assert_eq!(reader.contains(cell, account)?, filled);
        if filled {
            for (channel, numerator) in HostChannel::ALL.into_iter().zip(VALUES[index][instance]) {
                let value = reader.read_component(channel, cell, 0, account)?;
                assert_eq!(
                    Rational::binary(value.to_f64())?
                        .compare(&Rational::fraction(i64::from(numerator), 32)?),
                    std::cmp::Ordering::Equal
                );
            }
        }
    }
    Ok(())
}
fn setup<P: Scalar + 'static>()
-> TestResult<(Heap, ExecutionAccount, LcInputInventory<P, 5>, LcShapeAxes)> {
    let mut budget = Budget::new(Limits::default())?;
    let question = fixture::inventory_question(true, true, &mut budget)?;
    let mut account = ExecutionAccount::new(budget)?;
    let inventory = LcInputInventory::declare(&question, [2, 3, 2], &mut account)?;
    let heap = Heap::new(crate::HeapLimits::default())?;
    let axes = fixture::axes(&heap, &mut account)?;
    Ok((heap, account, inventory, axes))
}
fn source<P: Scalar + 'static>(
    schema: &LcInputShape<P>,
    axes: &LcShapeAxes,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    shift: usize,
) -> TestResult<crate::PublishedTile<P>> {
    let mut loan = fixture::granted(heap.request_loan(schema.source(axes, account)?, account))?;
    for (cell, values) in VALUES[schema.input_index()].into_iter().enumerate() {
        for (channel, numerator) in HostChannel::ALL.into_iter().zip(values) {
            if cell != 1 || channel != HostChannel::Error || shift == 1 {
                loan.write_component(
                    channel,
                    cell,
                    0,
                    P::from_u8(numerator) / P::from_u8(32),
                    account,
                )?;
            }
        }
    }
    match heap.handback(loan, account) {
        crate::HandbackOutcome::Published(tile, _) => Ok(tile),
        other => Err(format!("{other:?}").into()),
    }
}

/// Read original exact supports independently of numerical transport and check the literal oracle.
fn original_support<P: Scalar>(
    schema: &LcInputShape<P>,
    index: usize,
    account: &mut ExecutionAccount,
) -> TestResult {
    let expected = match index {
        0 | 1 => [(1, 4), (1, 1)],
        2 => [(0, 1), (1, 4)],
        4 => [(1, 4), (49, 64)],
        3 => [(1, 1), (1, 1)],
        _ => return Err("unknown original role".into()),
    };
    let original = &schema.question().relation().inputs()[index];
    for (endpoint, ratio) in [original.interval().lower(), original.interval().upper()]
        .into_iter()
        .zip(expected)
    {
        assert_eq!(
            endpoint
                .ok_or("finite support")?
                .as_rational()
                .ok_or("rational support")?
                .small_ratio(&mut account.ledger)?,
            Some(ratio)
        );
    }
    let lower = Rational::fraction(expected[0].0, expected[0].1)?;
    let upper = Rational::fraction(expected[1].0, expected[1].1)?;
    for [center, total, error] in VALUES[index] {
        let center = Rational::fraction(i64::from(center), 32)?;
        let total = Rational::fraction(i64::from(total), 32)?;
        let error = Rational::fraction(i64::from(error), 32)?;
        assert_ne!(center.sub(&total).compare(&lower), std::cmp::Ordering::Less);
        assert_ne!(
            center.add(&total).compare(&upper),
            std::cmp::Ordering::Greater
        );
        assert_ne!(error.compare(&total), std::cmp::Ordering::Greater);
    }
    Ok(())
}
