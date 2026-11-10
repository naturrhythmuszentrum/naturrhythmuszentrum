// SPDX-License-Identifier: AGPL-3.0-only
//! Every storage role and certified reading participates in completion; placeholders confer no value.
use super::{Fixture, TestResult, candidate, fixture, published, ready};
use crate::loan::{fault, payload::set};
use crate::{ApplicationError, ApplicationPublication, LoanError};
use std::rc::Rc;

/// A missing component in any of the five roles prevents a complete cell from publishing.
#[test]
fn prepared_all_five_completion_roles_gate_publication_and_repair() -> TestResult {
    for role in 0..5 {
        let Fixture {
            mut heap,
            mut account,
            request,
        } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
        let mut owner = candidate(
            ready(heap.request_pointwise_expm1(request, &mut account))?,
            &mut account,
        )?;
        let channels = owner
            .core
            .host
            .channels
            .iter_mut()
            .flatten()
            .chain(owner.core.endpoints.iter_mut().flatten());
        set(
            &mut channels.into_iter().nth(role).ok_or("role")?.components,
            0,
            false,
        );
        fault::reset_reads();
        fault::reset_scalars();
        let mut owner = match heap.publish_application(owner, &mut account) {
            ApplicationPublication::ApplicationRepair(
                ApplicationError::Loan(LoanError::CoverageMismatch),
                owner,
            ) => owner,
            other => return Err(format!("{other:?}").into()),
        };
        assert_eq!(fault::reads(), 0);
        assert_eq!(fault::scalars(), 0);
        let channels = owner
            .core
            .host
            .channels
            .iter_mut()
            .flatten()
            .chain(owner.core.endpoints.iter_mut().flatten());
        set(
            &mut channels.into_iter().nth(role).ok_or("role")?.components,
            0,
            true,
        );
        let (tile, _) = published(heap.publish_application(owner, &mut account))?;
        assert_eq!(fault::reads(), 5);
        assert_eq!(fault::scalars(), 0);
        assert!(
            tile.reader(&heap, &mut account)?
                .expm1_endpoints(0, 0, &mut account)
                .is_ok()
        );
    }
    Ok(())
}
/// Ordering, finite values, nominal error and outward total coverage each independently refuse.
#[test]
fn prepared_published_reading_validation_loads_five_values_once() -> TestResult {
    for (role, value) in [
        (0, f64::NAN),
        (1, -1.0),
        (1, 0.0),
        (2, -1.0),
        (2, 1.0),
        (3, f64::NAN),
        (3, 1.0),
        (4, -2.0),
    ] {
        let Fixture {
            mut heap,
            mut account,
            request,
        } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
        let mut owner = candidate(
            ready(heap.request_pointwise_expm1(request, &mut account))?,
            &mut account,
        )?;
        let channels = owner
            .core
            .host
            .channels
            .iter_mut()
            .flatten()
            .chain(owner.core.endpoints.iter_mut().flatten());
        channels
            .into_iter()
            .nth(role)
            .ok_or("role")?
            .matrix
            .write(0, value)?;
        fault::reset_reads();
        fault::reset_scalars();
        assert!(matches!(
            heap.publish_application(owner, &mut account),
            ApplicationPublication::ApplicationRepair(
                ApplicationError::Loan(LoanError::InvalidPayload {
                    cell: 0,
                    component: 0,
                    ..
                }),
                _
            )
        ));
        assert_eq!(fault::reads(), 5);
        assert_eq!(fault::scalars(), 0);
    }
    Ok(())
}
/// The result's two endpoint backings release separately from its three ordinary readings.
#[test]
fn prepared_endpoint_backing_destruction_is_exact_after_heap_drop() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
    let owner = candidate(
        ready(heap.request_pointwise_expm1(request, &mut account))?,
        &mut account,
    )?;
    let (tile, _) = published(heap.publish_application(owner, &mut account))?;
    let mut pin = tile.into_pin();
    drop(heap);
    let before = fault::numerical_releases();
    let record = Rc::get_mut(&mut pin.record)
        .and_then(|record| record.payload.as_mut())
        .ok_or("unique pin")?;
    let bundle = Rc::get_mut(&mut record.bundle)
        .and_then(|owner| owner.payload.as_mut())
        .ok_or("unique bundle")?;
    // Private inspection splits ownership solely to observe actual destructors, then performs no read.
    let endpoints = bundle.endpoints.take().ok_or("endpoints")?;
    drop(pin);
    assert_eq!(
        fault::numerical_releases(),
        [before[0] + 3, before[1] + 3, before[2] + 3 * 64]
    );
    drop(endpoints);
    assert_eq!(
        fault::numerical_releases(),
        [before[0] + 5, before[1] + 5, before[2] + 5 * 64]
    );
    Ok(())
}
