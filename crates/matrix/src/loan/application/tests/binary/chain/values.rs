// SPDX-License-Identifier: AGPL-3.0-only
//! Completed producers feed every consumer through the public three-role route.

use super::super as binary;
use super::super::super as common;
use super::{TestResult, apply, candidate, published};
use crate::reciprocal_testing::oracle::Rational;
use crate::{BinaryElementwise, CellOutcome, Elementwise, HostChannel};
use nrz_bindings::Limits;

/// All fifteen edges check nearest centers, immediate errors and whole boxes for every component.
/// Empty, sparse and full selections retain their independent membership and load controls.
/// ´test:unit:chain-fifteen-producer-consumer-edges´
#[test]
fn chain_fifteen_producer_consumer_edges() -> TestResult {
    for cells in [0, 1, 63, 64, 65] {
        for components in [1, 3] {
            for selection in 0..3 {
                for producer in 0..5 {
                    for consumer in [
                        Elementwise::Square,
                        Elementwise::SquareRoot,
                        Elementwise::Reciprocal,
                    ] {
                        edge(cells, components, selection, producer, consumer)?;
                    }
                }
            }
        }
    }
    quotient_reciprocal_quarter()?;
    Ok(())
}

/// The design's fixed `C = k = 1` full-demand control has centers 8 and 2 and zero totals.
/// Its final interval must contain `1 / (8 / 2) = 1/4`, independently of producer readings.
fn quotient_reciprocal_quarter() -> TestResult {
    let binary::Fixture {
        mut heap,
        mut account,
        request,
    } = binary::custom(
        1,
        1,
        [2; 2],
        2,
        false,
        BinaryElementwise::Quotient,
        Limits::default(),
        |operand, _, _| (if operand == 0 { 8.0 } else { 2.0 }, 0.0),
    )?;
    let owner = binary::ready(heap.request_pointwise_quotient(request, &mut account))?;
    let owner = candidate(owner, &mut account)?;
    let (tile, _) = published(heap.publish_application(owner, &mut account))?;
    let tile = apply(
        &mut heap,
        tile.into_pin(),
        Elementwise::Reciprocal,
        2,
        &mut account,
    )?;
    let [center, total, _] = super::oracle::readings(&tile, &heap, &mut account, 0, 0)?;
    assert!(Rational::new(1, 4)?.covered(center, total)?);
    Ok(())
}

/// One public edge independently checks selected load and invocation counts.
fn edge(
    cells: usize,
    components: usize,
    selection: usize,
    producer: usize,
    consumer: Elementwise,
) -> TestResult {
    let function = if producer == 4 {
        BinaryElementwise::Quotient
    } else {
        BinaryElementwise::Product
    };
    let offsets = (0..cells)
        .map(|cell| Ok(f64::from(u32::try_from(cell)?) / 8.0))
        .collect::<TestResult<Vec<_>>>()?;
    let binary::Fixture {
        mut heap,
        mut account,
        request,
    } = binary::custom(
        cells,
        components,
        [1; 2],
        2,
        false,
        function,
        Limits::default(),
        |operand, cell, component| {
            (
                if operand == 0 {
                    8.0 + offsets[cell] + [0.0, 0.03125, 0.0625][component]
                } else {
                    2.0
                },
                0.0,
            )
        },
    )?;
    let owner = binary::ready(binary::admit(&mut heap, request, function, &mut account))?;
    let owner = candidate(owner, &mut account)?;
    let (mut tile, _) = published(heap.publish_application(owner, &mut account))?;
    if producer < 3 {
        let unary = [
            Elementwise::Square,
            Elementwise::SquareRoot,
            Elementwise::Reciprocal,
        ][producer];
        tile = apply(&mut heap, tile.into_pin(), unary, 2, &mut account)?;
    }
    let input = super::oracle::snapshot(&tile, &heap, &mut account, cells, components)?;
    let request = super::request(
        &mut heap,
        tile.into_pin(),
        consumer,
        selection,
        &mut account,
    )?;
    let owner = common::ready(super::admit(&mut heap, request, consumer, &mut account))?;
    crate::loan::fault::reset_reads();
    crate::loan::fault::reset_scalars();
    let owner = candidate(owner, &mut account)?;
    let count = (0..cells)
        .filter(|cell| common::requested(*cell, selection))
        .count();
    assert_eq!(crate::loan::fault::reads(), 2 * components * count);
    assert_eq!(crate::loan::fault::scalars(), components * count);
    let (tile, _) = published(heap.publish_application(owner, &mut account))?;
    let reader = tile.reader(&heap, &mut account)?;
    verify(&reader, &mut account, &input, selection, consumer)?;
    Ok(())
}

/// Every selected triple is checked against the same cell and component of its immediate producer.
pub(super) fn verify(
    reader: &crate::TileReader<'_, f64>,
    account: &mut crate::ExecutionAccount,
    input: &[Vec<[f64; 3]>],
    selection: usize,
    function: Elementwise,
) -> TestResult {
    for (cell, components) in input.iter().enumerate() {
        let selected = common::requested(cell, selection);
        assert_eq!(reader.contains(cell, account)?, selected);
        assert_eq!(
            matches!(
                reader.application_outcome(cell, account)?,
                CellOutcome::Completed
            ),
            selected
        );
        if selected {
            for (component, source) in components.iter().enumerate() {
                let mut output = [0.0; 3];
                for (value, channel) in output.iter_mut().zip(HostChannel::ALL) {
                    *value = reader.read_component(channel, cell, component, account)?;
                }
                super::oracle::check(*source, output, function)?;
            }
        }
    }
    Ok(())
}
