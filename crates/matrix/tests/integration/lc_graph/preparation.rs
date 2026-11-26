// SPDX-License-Identifier: AGPL-3.0-only
//! Capacity preparation refusal is distinct from the malformed-certificate public control.
use super::fixture::{self, Result, Setup};
use nrz_bindings::Limits;
use nrz_matrix::*;
use nrz_scalar::Scalar;

pub(super) fn run<P: Scalar + 'static>() -> Result {
    let limit = boundary::<P>()?;
    assert!(limit < Limits::default().payload_bytes);
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        radian,
        certificate,
        ..
    } = Setup::<P>::with_limits(
        false,
        false,
        Limits {
            payload_bytes: limit,
            ..Limits::default()
        },
        [2, 3, 2],
    )?;
    let (mut owner, _) = fixture::admit_storage(
        &inventory,
        &axes,
        &registry,
        &radian,
        &mut heap,
        &mut account,
        (
            (&certificate, &(0..12).collect::<Vec<_>>(), false),
            [1, 1, 1, 1, 1, 3, 3, 3, 3, 3, 4, 1],
            Some([49, 332, 0]),
        ),
    )?;
    let _pins = issue(&mut owner, &mut heap, &mut account)?;
    mapped(&mut owner, &mut heap, &mut account)?;
    // O3280; adapter144+observer88; K144+records4312+words2656+registration24.
    let graph = graph::<P>();
    let ballast = super::capacity::fill(
        &owner,
        graph,
        &inventory,
        &registry,
        &mut heap,
        &mut account,
    )?;
    let omega = owner.handle(10, &mut account)?;
    for _ in 0..32 {
        match owner.drive_one(&mut heap, &mut account, &[ballast.borrowed()])? {
            LcGraphEvent::PreparationRefused {
                node,
                declaration,
                error,
                probe,
                evidence,
                control,
            } => {
                assert_eq!(node, omega);
                assert!(std::ptr::eq(declaration.inventory(), &raw const inventory));
                assert!(
                    matches!(
                        error,
                        LcShapeError::Loan(LoanError::Heap(HeapError::ResourceBytes { .. }))
                    ),
                    "capacity preparation: {error:?}"
                );
                control?;
                owner.acknowledge_probe(probe?, &mut account)?;
                owner.acknowledge_return(node, evidence?, &heap, &mut account)?;
                assert!(matches!(
                    owner.retry_original(node, &heap, &mut account),
                    Err(LcGraphError::InvalidPhase)
                ));
                return Ok(());
            }
            event => acknowledge(&mut owner, event, &heap, &mut account)?,
        }
    }
    Err("capacity preparation did not run".into())
}

/// Stop before omega preparation, after every original mapped result is publicly published.
fn mapped<P: Scalar + 'static>(
    owner: &mut LcGraphOwner<'_, P, 5>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result<usize> {
    let mut peak = super::capacity::native(heap, account)?;
    for _ in 0..128 {
        let mut published = true;
        for slot in 5..10 {
            let handle = owner.handle(slot, account)?;
            published &= owner.state(handle, heap, account)? == LcGraphState::Published;
        }
        if published {
            return Ok(peak);
        }
        let event = owner.drive_one(heap, account, &[])?;
        acknowledge(owner, event, heap, account)?;
        peak = peak.max(super::capacity::native(heap, account)?);
    }
    Err("mapped prerequisites unfinished".into())
}
fn acknowledge<'i, P: Scalar + 'static>(
    owner: &mut LcGraphOwner<'i, P, 5>,
    event: LcGraphEvent<'i, P, 5>,
    heap: &Heap,
    account: &mut ExecutionAccount,
) -> Result {
    match event {
        LcGraphEvent::Advanced { probe, control, .. } => {
            control?;
            owner.acknowledge_probe(probe, account)?;
        }
        LcGraphEvent::Terminal {
            node,
            evidence,
            control,
        } => {
            control?;
            owner.acknowledge_outcome(node, evidence, heap, account)?;
        }
        LcGraphEvent::Control => {}
        other => return Err(format!("preparation progression: {other:?}").into()),
    }
    Ok(())
}

/// The fixed graph is enumerated from O, adapter and K storage, independently of its planner.
const fn graph<P: Scalar>() -> usize {
    3280 + 232 + 12 * (size_of::<LcGraphNode<'_, P, 5>>() + 8) + 7136
}
/// A separate original native lifecycle measures the owning layer's baseline. The tested account
/// starts with the maximum observed native baseline plus the independent graph and empty-owner752.
/// Neither account's limits change, and no expected byte figure comes from adapter inspection.
fn boundary<P: Scalar + 'static>() -> Result<u64> {
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        radian,
        certificate,
        ..
    } = Setup::<P>::new(false)?;
    let (mut owner, _) = fixture::admit_storage(
        &inventory,
        &axes,
        &registry,
        &radian,
        &mut heap,
        &mut account,
        (
            (&certificate, &(0..12).collect::<Vec<_>>(), false),
            [1, 1, 1, 1, 1, 3, 3, 3, 3, 3, 4, 1],
            Some([49, 332, 0]),
        ),
    )?;
    let _pins = issue(&mut owner, &mut heap, &mut account)?;
    let native = mapped(&mut owner, &mut heap, &mut account)?;
    Ok(u64::try_from(native + graph::<P>() + 752)?)
}

/// Original outside pins protect source lineage while ordinary residence scans reconcile debits.
fn issue<P: Scalar + 'static>(
    owner: &mut LcGraphOwner<'_, P, 5>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result<Vec<BundlePin<P>>> {
    let mut pins = Vec::new();
    for slot in 0..5 {
        let handle = owner.handle(slot, account)?;
        let mut loan = fixture::request(owner, handle, heap, account)?;
        let ticket = loan.ticket();
        fixture::fill(&mut loan, slot, 2, account)?;
        let LcGraphInputReturn::Published { control } =
            owner.return_input(handle, loan, heap, account)
        else {
            return Err("source publication".into());
        };
        control?;
        pins.push(heap.acquire_pin(ticket, account)?);
    }
    Ok(pins)
}
