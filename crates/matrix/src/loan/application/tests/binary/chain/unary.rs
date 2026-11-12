// SPDX-License-Identifier: AGPL-3.0-only
//! Host-origin unary results and deeper unary spines share the same opt-in composition route.

use super::super::super as common;
use super::{TestResult, admit, candidate, published, request};
use crate::Elementwise;

/// Each native host-origin unary producer completes with each consumer, with V=1+22+(2+12)=37.
#[test]
fn chain_host_unary_producer_edges_and_origin_work() -> TestResult {
    for producer in [
        Elementwise::Square,
        Elementwise::SquareRoot,
        Elementwise::Reciprocal,
    ] {
        for consumer in [
            Elementwise::Square,
            Elementwise::SquareRoot,
            Elementwise::Reciprocal,
        ] {
            let mut setup =
                common::fixture::<f64>(3, 2, 1, 2, false, nrz_bindings::Limits::default())?;
            common::refusal::replace_source(&mut setup, |cell, component| {
                ([2.0, 3.0, 5.0][cell] + [0.0, 0.25][component], 0.0)
            })?;
            setup.request.operation = crate::OperationDraft::map_elementwise(
                setup.request.operation.inputs()[0].shape().clone(),
                setup.request.operation.outputs()[0].shape().clone(),
                producer,
            )?
            .check(crate::ExecutionLimits::default())?;
            let result = match producer {
                Elementwise::Square => setup
                    .heap
                    .request_pointwise_square(setup.request, &mut setup.account),
                Elementwise::SquareRoot => setup
                    .heap
                    .request_pointwise_square_root(setup.request, &mut setup.account),
                Elementwise::Reciprocal => setup
                    .heap
                    .request_pointwise_reciprocal(setup.request, &mut setup.account),
            };
            let owner = candidate(common::ready(result)?, &mut setup.account)?;
            let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
            let before = setup.account.usage().work;
            crate::loan::application::chain::authenticate(
                &setup.heap,
                tile.pin(),
                crate::loan::application::chain::Input::Application,
                &mut setup.account.ledger,
            )?;
            assert_eq!(setup.account.usage().work - before, 37);
            let input = super::oracle::snapshot(&tile, &setup.heap, &mut setup.account, 3, 2)?;
            let request = request(
                &mut setup.heap,
                tile.into_pin(),
                consumer,
                2,
                &mut setup.account,
            )?;
            let owner = common::ready(admit(
                &mut setup.heap,
                request,
                consumer,
                &mut setup.account,
            ))?;
            let owner = candidate(owner, &mut setup.account)?;
            assert!(
                owner
                    .core
                    .evidence
                    .outcomes
                    .iter()
                    .all(|outcome| matches!(outcome, crate::CellOutcome::Completed))
            );
            let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
            let reader = tile.reader(&setup.heap, &mut setup.account)?;
            super::values::verify(&reader, &mut setup.account, &input, 2, consumer)?;
        }
    }
    Ok(())
}

/// The unary walk uses borrowed state at arbitrary depth, with every edge paying its own J+2.
#[test]
fn chain_deeper_spine_origin_work_and_retired_demands() -> TestResult {
    let (mut heap, mut account, mut tile, mut demands) = super::prefix(1, false)?;
    for depth in 0..8 {
        let before = account.usage().work;
        crate::loan::application::chain::authenticate(
            &heap,
            tile.pin(),
            crate::loan::application::chain::Input::Application,
            &mut account.ledger,
        )?;
        assert_eq!(account.usage().work - before, 52 + 21 * depth);
        let request = super::request(
            &mut heap,
            tile.into_pin(),
            Elementwise::SquareRoot,
            2,
            &mut account,
        )?;
        demands.push(request.demand);
        let owner = common::ready(admit(
            &mut heap,
            request,
            Elementwise::SquareRoot,
            &mut account,
        ))?;
        let owner = candidate(owner, &mut account)?;
        (tile, _) = published(heap.publish_application(owner, &mut account))?;
    }
    for demand in demands {
        heap.retire_mask(demand, &mut account.ledger)?;
    }
    let before = account.usage().work;
    crate::loan::application::chain::authenticate(
        &heap,
        tile.pin(),
        crate::loan::application::chain::Input::Application,
        &mut account.ledger,
    )?;
    assert_eq!(account.usage().work - before, 52 + 21 * 8);
    let tile = super::apply(
        &mut heap,
        tile.into_pin(),
        Elementwise::Reciprocal,
        2,
        &mut account,
    )?;
    assert!(
        tile.reader(&heap, &mut account)?
            .contains(0, &mut account)?
    );
    Ok(())
}
