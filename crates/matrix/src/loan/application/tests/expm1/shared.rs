// SPDX-License-Identifier: AGPL-3.0-only
//! Distinct result bundles retain one law with distinct funded uses and complete endpoint owners.
use super::{Fixture, TestResult, candidate, fixture, published, ready};
use crate::{
    CellOutcome, Coverage, HostTileDeclaration, PointwiseExpm1Request, PreparedExpm1Operation,
};

/// The second result has fresh symbols, its own source pin and an explicitly checked second law use.
pub(super) fn additional(setup: &mut Fixture) -> TestResult<PointwiseExpm1Request> {
    let original = &setup.request;
    let law = setup
        .heap
        .retain_retained_expm1(&original.law, &mut setup.account)?;
    let request = law.authority()?.request();
    let destination = nrz_bindings::BindingSymbols::declare(
        &nrz_bindings::Descriptor::ratio(),
        [1, 1],
        [true; 2],
        &mut setup.account.ledger,
    )?;
    let operation = PreparedExpm1Operation::check(
        &law,
        &request.original.target,
        &request.original.context,
        &request.original.coordinates[0].coordinate,
        law.domain()?,
        request.bits,
        request.environment,
        original.operation.operation.inputs()[0].clone(),
        original.operation.operation.outputs()[0].clone(),
        original.operation.source.clone(),
        destination.clone(),
        original.operation.axis,
        &mut setup.account,
    )?;
    Ok(PointwiseExpm1Request {
        operation,
        law,
        input: setup.heap.retain_pin(&original.input, &mut setup.account)?,
        destination: HostTileDeclaration::check(
            destination,
            original.destination.axis,
            Coverage::RepresentedInputs,
            &mut setup.account,
        )?,
        demand: original.demand,
        limits: original.limits,
    })
}
/// Dropping one result releases only its funded use; the other still authorizes stored endpoints.
#[test]
fn prepared_two_results_distinct_uses_and_single_final_destruction() -> TestResult {
    let mut setup = fixture(1, 1, [-2.0, 0.0], 2, false)?;
    let other = additional(&mut setup)?;
    let inventory = setup
        .heap
        .retained_expm1_inventory(&setup.request.law, &mut setup.account)?;
    assert_eq!(inventory.uses, 2);
    let first = crate::loan::retained_expm1::Binding::of(&setup.request.law)?;
    let second = crate::loan::retained_expm1::Binding::of(&other.law)?;
    assert_ne!(first, second);
    assert!(std::ptr::eq(
        setup.request.law.authority()?,
        other.law.authority()?
    ));
    let Fixture {
        mut heap,
        mut account,
        request,
    } = setup;
    let first = candidate(
        ready(heap.request_pointwise_expm1(request, &mut account))?,
        &mut account,
    )?;
    let (first, _) = published(heap.publish_application(first, &mut account))?;
    let second = candidate(
        ready(heap.request_pointwise_expm1(other, &mut account))?,
        &mut account,
    )?;
    let (second, _) = published(heap.publish_application(second, &mut account))?;
    let destroyed = super::laws::destroyed();
    drop(heap);
    drop(first);
    assert_eq!(super::laws::destroyed(), destroyed);
    let reader = second.pin().historical_reader(&mut account)?;
    assert!(matches!(
        reader.application_outcome(0, &mut account)?,
        CellOutcome::Completed
    ));
    assert!(
        reader
            .expm1_endpoints(0, 0, &mut account)?
            .into_iter()
            .all(f64::is_finite)
    );
    drop(second);
    assert_eq!(super::laws::destroyed(), destroyed + 1);
    Ok(())
}
/// The sole registry root keeps the complete immutable law and five-role result until reclamation.
#[test]
fn prepared_registry_root_only_and_endpoint_debit_reconciliation() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
    let demand = request.demand;
    let owner = ready(heap.request_pointwise_expm1(request, &mut account))?;
    heap.retire_mask(demand, &mut account.ledger)?;
    let owner = candidate(owner, &mut account)?;
    let (tile, receipt) = published(heap.publish_application(owner, &mut account))?;
    let bundle = tile
        .pin()
        .record
        .payload
        .as_ref()
        .and_then(|pin| pin.bundle.payload.as_ref())
        .ok_or("bundle")?;
    let tokens = bundle
        .endpoints
        .as_ref()
        .ok_or("endpoints")?
        .each_ref()
        .map(|channel| (channel.token.slot, channel.token.key));
    let destroyed = super::laws::destroyed();
    drop(tile);
    assert_eq!(super::laws::destroyed(), destroyed);
    let pin = heap.acquire_pin::<f64>(receipt.ticket(), &mut account)?;
    assert!(
        pin.historical_reader(&mut account)?
            .expm1_endpoints(0, 0, &mut account)
            .is_ok()
    );
    drop(pin);
    assert!(matches!(
        heap.reconcile_detached(&mut account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    assert!(matches!(
        heap.reclaim(&mut account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    assert!(matches!(
        heap.reconcile_detached(&mut account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    assert_eq!(super::laws::destroyed(), destroyed + 1);
    for (slot, key) in tokens {
        assert!(
            heap.loans.debits[slot]
                .as_ref()
                .is_none_or(|debit| debit.key != key)
        );
    }
    assert_eq!(heap.loans.elements()?, [0, 0]);
    assert!(heap.loans.debits.iter().all(Option::is_none));
    assert!(matches!(
        heap.residence(&mut account, 100_000)?,
        crate::Residence::Empty { .. }
    ));
    Ok(())
}
/// An all-refused application still publishes fixed outcomes and no readable numerical component.
#[test]
fn prepared_all_refused_output_exposes_no_reading() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture(65, 3, [-0.125, -0.0625], 2, false)?;
    let owner = candidate(
        ready(heap.request_pointwise_expm1(request, &mut account))?,
        &mut account,
    )?;
    crate::loan::fault::reset_scalars();
    let (tile, _) = published(heap.publish_application(owner, &mut account))?;
    assert_eq!(crate::loan::fault::scalars(), 0);
    let reader = tile.reader(&heap, &mut account)?;
    crate::loan::fault::reset_reads();
    for cell in 0..65 {
        assert!(!reader.contains(cell, &mut account)?);
        assert!(matches!(
            reader.application_outcome(cell, &mut account)?,
            CellOutcome::PreparedRefused {
                component: 0,
                reason: nrz_scalar::Expm1Failure::OutsideDomain
            }
        ));
        for component in 0..3 {
            assert!(
                reader
                    .expm1_endpoints(cell, component, &mut account)
                    .is_err()
            );
            for channel in crate::HostChannel::ALL {
                assert!(
                    reader
                        .read_component(channel, cell, component, &mut account)
                        .is_err()
                );
            }
        }
    }
    assert_eq!(crate::loan::fault::reads(), 0);
    Ok(())
}
