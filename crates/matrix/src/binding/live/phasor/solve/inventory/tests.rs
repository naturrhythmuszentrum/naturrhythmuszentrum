// SPDX-License-Identifier: AGPL-3.0-only
//! Native owners refuse missing, substituted and replayed allocation authority.

use super::{Run, Stage, State};
use crate::phasor_solve_testing as fixture;
use crate::reading_testing::{TestResult, ledger};
use nrz_bindings::{Budget, Limits, readings::PhasorResidualNorm};
use nrz_scalar::{ArithmeticContext, Scalar};
use std::{cell::Cell, rc::Rc};

/// Both formats pin exact float capacity, common peak and aligned metadata after compact storage.
/// ´test:unit:phasor-inventory-costs-are-pinned´
#[test]
fn phasor_inventory_costs_are_pinned() -> TestResult {
    assert_eq!(
        [
            measurement::<f32>()?,
            measurement::<f64>()?,
            layout::<f32>(),
            layout::<f64>()
        ],
        [
            [49, 152, 608, 34, 256, 336, 496, 604],
            [49, 152, 1216, 34, 512, 672, 496, 600],
            [480, 496, 49, 392, 192, 208, 56, 24],
            [480, 496, 49, 392, 312, 328, 88, 24],
        ]
    );
    Ok(())
}
/// Actual aligned layouts explain the plan tables and precision-dependent final-owner padding.
const fn layout<P: Scalar>() -> [usize; 8] {
    [
        size_of::<super::Plan<P>>(),
        size_of::<super::RcLayout<super::Plan<P>>>(),
        size_of::<[Cell<State>; super::COUNT]>(),
        size_of::<[Cell<usize>; super::COUNT]>(),
        size_of::<super::super::Contents<P>>(),
        size_of::<super::RcLayout<super::super::Contents<P>>>(),
        size_of::<super::Owner<P, super::super::PhasorSolveCertificate<P>>>(),
        size_of::<super::PhasorInventory<P>>(),
    ]
}
/// Measure one complete native publication without counting any original exact definition twice.
fn measurement<P: Scalar + 'static>() -> TestResult<[usize; 8]> {
    let mut case = fixture::case(-P::from_u8(75), P::ZERO, false, &mut fixture::budget()?)?;
    let publication = case.publication(&mut ledger()?)?;
    let result =
        case.heap
            .publish_phasor_solve(&publication, &ArithmeticContext::new()?, &mut ledger()?)?;
    let inventory = result.evaluation().inventory();
    let mut floats = 0;
    let mut vectors = 0;
    let mut vector_bytes = 0;
    let mut bytes = 0;
    for allocation in inventory.allocations() {
        assert_eq!(
            allocation.owned_bytes(),
            allocation.float_entries()? * size_of::<P>()
        );
        floats += allocation.float_entries()?;
        bytes += allocation.owned_bytes();
        if allocation.stage()?.is_vector() {
            vectors += 1;
            vector_bytes += allocation.owned_bytes();
        }
    }
    Ok([
        inventory.allocations().count(),
        floats,
        bytes,
        vectors,
        vector_bytes,
        inventory.peak_payload_bytes(),
        inventory.planning_bytes(),
        inventory.metadata_bytes(),
    ])
}
/// A wrong concrete type or absent meaning refuses before the allocating closure can run.
/// ´test:unit:phasor-inventory-refuses-unplanned-allocation´
#[test]
fn phasor_inventory_refuses_unplanned_allocation() -> TestResult {
    let case = fixture::case(-75.0_f64, 0.0, false, &mut fixture::budget()?)?;
    let plan = case.solve(
        false,
        PhasorResidualNorm::ComponentEuclidean,
        &mut ledger()?,
    )?;
    let run = Run::new(&plan, false, &ledger()?)?;
    let called = Cell::new(false);
    let wrong = run.inline(Stage::InverseCandidate, || {
        called.set(true);
        Ok([0.0_f32; 4])
    });
    assert!(matches!(
        wrong,
        Err(crate::PhasorError::Inventory(
            crate::PrivateAllocationError::Representation
        ))
    ));
    let absent = Stage::InputCopy {
        operand: 2,
        copy: 0,
        channel: super::PhasorAllocationChannel::Center,
    };
    assert!(
        run.vector(absent, || {
            called.set(true);
            Ok(vec![0.0])
        })
        .is_err()
    );
    assert!(!called.get());
    let values = run.zeros(Stage::InverseDefects)?;
    drop(values);
    assert!(run.zeros(Stage::InverseDefects).is_err());
    assert!(run.zeros(Stage::PublishedCenters).is_err());
    Ok(())
}
/// Publication refuses a missing stage and a foreign exact same-shaped execution.
/// ´test:unit:phasor-inventory-requires-complete-original-owners´
#[test]
fn phasor_inventory_requires_complete_original_owners() -> TestResult {
    let case = fixture::case(-75.0_f64, 0.0, false, &mut fixture::budget()?)?;
    let plan = case.solve(
        false,
        PhasorResidualNorm::ComponentEuclidean,
        &mut ledger()?,
    )?;
    let evaluation =
        case.heap
            .evaluate_phasor_solve(&plan, &ArithmeticContext::new()?, &mut ledger()?)?;
    let run = Run(Rc::clone(&evaluation.inventory.plan));
    run.0.states[30].set(State::Planned);
    assert!(matches!(
        run.complete(&evaluation.contents),
        Err(crate::PhasorError::Inventory(
            crate::PrivateAllocationError::IncompleteCoverage
        ))
    ));
    run.0.states[30].set(State::Released);
    assert!(run.complete(&evaluation.contents).is_ok());
    let foreign = Run::new(&plan, false, &ledger()?)?;
    assert!(matches!(
        foreign.complete(&evaluation.contents),
        Err(crate::PhasorError::Inventory(
            crate::PrivateAllocationError::ForeignInventory
        ))
    ));
    Ok(())
}
/// Clones share final arrays and proof facts; historical receipts cannot keep numerical owners alive.
/// ´test:unit:phasor-inventory-clones-share-final-owners´
#[test]
fn phasor_inventory_clones_share_final_owners() -> TestResult {
    let case = fixture::case(-75.0_f64, 0.0, false, &mut fixture::budget()?)?;
    let plan = case.solve(
        false,
        PhasorResidualNorm::ComponentEuclidean,
        &mut ledger()?,
    )?;
    let evaluation =
        case.heap
            .evaluate_phasor_solve(&plan, &ArithmeticContext::new()?, &mut ledger()?)?;
    let historical = evaluation.inventory().clone();
    let weak = Rc::downgrade(&evaluation.contents);
    let shared = evaluation.clone();
    assert!(Rc::ptr_eq(&evaluation.contents, &shared.contents));
    let original = historical
        .allocations()
        .next()
        .ok_or("missing original allocation")?;
    let copied = shared
        .inventory()
        .allocations()
        .next()
        .ok_or("missing shared allocation")?;
    assert!(original.same_binding(&copied));
    drop(evaluation);
    assert!(weak.upgrade().is_some());
    drop(shared);
    assert!(weak.upgrade().is_none());
    assert_eq!(historical.allocations().count(), 46);
    Ok(())
}
/// The complete new metadata has independent exact staged and retained-byte boundaries.
/// ´test:unit:phasor-inventory-metadata-boundaries´
#[test]
fn phasor_inventory_metadata_boundaries() -> TestResult {
    metadata_boundaries::<f32>()?;
    metadata_boundaries::<f64>()
}
/// Both scalar formats admit exactly their observed aligned metadata and refuse one byte less.
fn metadata_boundaries<P: Scalar + 'static>() -> TestResult {
    let case = fixture::case(-P::from_u8(75), P::ZERO, false, &mut fixture::budget()?)?;
    let plan = case.solve(
        false,
        PhasorResidualNorm::ComponentEuclidean,
        &mut ledger()?,
    )?;
    let bytes = u64::try_from(super::record::metadata::<P>())?;
    for staged in [true, false] {
        for (limit, success) in [(bytes, true), (bytes - 1, false)] {
            let mut limits = Limits::default();
            if staged {
                limits.staged_payload_bytes = limit;
            } else {
                limits.output_bytes = limit;
            }
            assert_eq!(
                Run::new(&plan, true, &Budget::new(limits)?).is_ok(),
                success
            );
        }
    }
    Ok(())
}

/// Equal typed fields cannot substitute another stage's exact ticket in the same execution.
/// ´test:unit:phasor-inventory-refuses-substituted-ticket´
#[test]
fn phasor_inventory_refuses_substituted_ticket() -> TestResult {
    let case = fixture::case(-75.0_f64, 0.0, false, &mut fixture::budget()?)?;
    let plan = case.solve(
        false,
        PhasorResidualNorm::ComponentEuclidean,
        &mut ledger()?,
    )?;
    let mut evaluation =
        case.heap
            .evaluate_phasor_solve(&plan, &ArithmeticContext::new()?, &mut ledger()?)?;
    let run = Run(Rc::clone(&evaluation.inventory.plan));
    Rc::get_mut(&mut evaluation.contents)
        .ok_or("unique final owner")?
        .certificate
        .permit
        .slot = 40;
    let refused = run.complete(&evaluation.contents);
    Rc::get_mut(&mut evaluation.contents)
        .ok_or("unique final owner")?
        .certificate
        .permit
        .slot = 41;
    assert!(matches!(
        refused,
        Err(crate::PhasorError::Inventory(
            crate::PrivateAllocationError::ForeignInventory
        ))
    ));
    assert!(run.complete(&evaluation.contents).is_ok());
    Ok(())
}
