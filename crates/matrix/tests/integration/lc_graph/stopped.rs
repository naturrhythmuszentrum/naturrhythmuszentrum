// SPDX-License-Identifier: AGPL-3.0-only
//! Original-ledger Work exhaustion leaves delivery outstanding and cleanup prepaid.
use super::{
    drive,
    fixture::{self, Result, Setup},
};
use nrz_bindings::{
    AdmissionError, Limits,
    readings::algebra::{FaultKind, limits::BudgetKind},
};
use nrz_matrix::*;
use nrz_scalar::Scalar;

pub(super) fn run<P: Scalar + 'static>() -> Result {
    // Prior work leaves 300000 units before account creation; construction alone needs 336554.
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        radian,
        certificate,
        ..
    } = Setup::<P>::with_remaining(false, false, Limits::default(), [2, 3, 2], Some(300_000))?;
    let (mut owner, _) = fixture::admit(
        &inventory,
        &axes,
        &registry,
        &radian,
        &mut heap,
        &mut account,
        (&certificate, &(0..12).collect::<Vec<_>>(), true),
    )?;
    let extra = owner.handle(12, &mut account)?;
    let LcGraphInputRequest::Granted {
        loan,
        evidence: Ok(evidence),
        control,
    } = owner.request_input(extra, &mut heap, &mut account, &[])?
    else {
        return Err("host before stop".into());
    };
    control?;
    let witness = owner.witness(evidence, &mut account)?;
    owner.acknowledge_probe(evidence, &mut account)?;
    drive::issue(&mut owner, 2, &mut heap, &mut account)?;
    stopped(&mut owner, &mut heap, &mut account)?;
    assert_eq!(account.limits(), &Limits::default());
    assert!(matches!(
        owner.drive_one(&mut heap, &mut account, &[]),
        Err(LcGraphError::WorkStopped)
    ));
    assert!(matches!(
        owner.begin_capacity_epoch(&mut account),
        Err(LcGraphError::WorkStopped)
    ));
    let before = account.usage();
    drop(owner);
    assert!(matches!(
        heap.cancel_loan(loan, &mut account),
        CancelOutcome::Cancelled(_)
    ));
    assert_eq!(account.usage(), before);
    assert_eq!(
        witness.status(&mut account)?,
        LcGraphDeliveryStatus::Abandoned
    );
    let before = account.usage();
    drop(witness);
    assert_eq!(account.usage(), before);
    Ok(())
}
fn stopped<P: Scalar + 'static>(
    owner: &mut LcGraphOwner<'_, P, 5>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result {
    for _ in 0..128 {
        match owner.drive_one(heap, account, &[])? {
            LcGraphEvent::PreparationRefused {
                error: LcShapeError::Declaration(AdmissionError::Algebra(fault)),
                declaration,
                ..
            } => {
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::Work,
                        ..
                    }
                ));
                drop(declaration);
                return Ok(());
            }
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
            other => return Err(format!("expected original Work stop: {other:?}").into()),
        }
    }
    Err("finite exact preparation did not stop".into())
}
