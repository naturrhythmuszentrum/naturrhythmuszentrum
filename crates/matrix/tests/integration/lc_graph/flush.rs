// SPDX-License-Identifier: AGPL-3.0-only
//! F2 closes explicit scopes while genuine borrowed occupations preserve repair authority.
use super::{
    drive,
    fixture::{self, Result, Setup},
};
use nrz_matrix::*;
use nrz_scalar::Scalar;

pub(super) fn run<P: Scalar + 'static>(invalid: P) -> Result {
    for cancel in [false, true] {
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
        let (mut owner, masks) = fixture::admit(
            &inventory,
            &axes,
            &registry,
            &radian,
            &mut heap,
            &mut account,
            (&certificate, &(0..12).collect::<Vec<_>>(), false),
        )?;
        let handles = (0..12)
            .map(|index| owner.handle(index, &mut account))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut loan = fixture::request(&mut owner, handles[0], &mut heap, &mut account)?;
        let other = fixture::request(&mut owner, handles[1], &mut heap, &mut account)?;
        fixture::fill(&mut loan, 0, 2, &mut account)?;
        loan.write_component(HostChannel::Center, 0, 0, invalid, &mut account)?;
        notify(&mut owner, &handles, &heap, &mut account)?;
        assert_eq!(
            owner.state(handles[0], &heap, &mut account)?,
            LcGraphState::Borrowed
        );
        let ticket = loan.ticket();
        let LcGraphInputReturn::Rejected {
            mut loan,
            error:
                LcGraphError::Native(LoanError::InvalidPayload {
                    channel: HostChannel::Center,
                    ..
                }),
        } = owner.return_input(handles[0], loan, &mut heap, &mut account)
        else {
            return Err("malformed handback must preserve occupation".into());
        };
        assert_eq!(loan.ticket(), ticket);
        assert!(matches!(
            LcGraphOwner::completion(&[&owner], &mut heap, &mut account, 100_000)?,
            LcGraphCompletion::GraphPending(_)
        ));
        if cancel {
            cancel_input(&mut owner, handles[0], loan, &mut heap, &mut account)?;
        } else {
            loan.write_component(HostChannel::Center, 0, 0, P::ONE, &mut account)?;
            let LcGraphInputReturn::Published { control } =
                owner.return_input(handles[0], loan, &mut heap, &mut account)
            else {
                return Err("same native loan repaired".into());
            };
            control?;
        }
        let other = routing(&mut owner, handles[0], other, &mut heap, &mut account)?;
        cancel_input(&mut owner, handles[1], other, &mut heap, &mut account)?;
        assert!(
            drive::drain(
                &mut owner,
                LcGraphState::MissingInput,
                false,
                &mut heap,
                &mut account
            )?
            .is_none()
        );
        fixture::retire(masks, &mut heap, &mut account)?;
        drive::complete(&owner, &mut heap, &mut account)?;
    }
    overlap::<P>()
}

pub(super) fn notify<P: Scalar + 'static>(
    owner: &mut LcGraphOwner<'_, P, 5>,
    handles: &[LcGraphHandle],
    heap: &Heap,
    account: &mut ExecutionAccount,
) -> Result {
    let scope = owner.begin_scope(&[handles[11]], handles, account)?;
    let mut closed = false;
    for _ in 0..128 {
        if owner.close_scope(scope, account)? == LcGraphScopeProgress::Published {
            closed = true;
            break;
        }
    }
    assert!(closed);
    for _ in 0..13 {
        owner.notify_one(scope, heap, account)?;
    }
    for _ in 0..3 {
        assert_eq!(
            owner.notify_one(scope, heap, account)?,
            LcGraphScopeProgress::Published
        );
    }
    Ok(())
}

pub(super) fn cancel_input<P: Scalar + 'static>(
    owner: &mut LcGraphOwner<'_, P, 5>,
    handle: LcGraphHandle,
    loan: OwnedLoan<P>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result {
    let CancelOutcome::Cancelled(receipt) = heap.cancel_loan(loan, account) else {
        return Err("native cancellation".into());
    };
    owner.input_cancelled(handle, receipt, heap, account)?;
    Ok(())
}

/// Bounded progress retires the first occupation while another loan remains genuinely borrowed.
fn settle_prefix<P: Scalar + 'static>(
    owner: &mut LcGraphOwner<'_, P, 5>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result {
    for _ in 0..128 {
        match owner.drive_one(heap, account, &[])? {
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
            LcGraphEvent::Quiescent => return Ok(()),
            other => return Err(format!("partial scoped progress: {other:?}").into()),
        }
    }
    Err("partial progress did not quiesce".into())
}

/// The old outside reader and required delivery remain independent of newly admitted input.
fn overlap<P: Scalar + 'static>() -> Result {
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
    let (mut owner, masks) = fixture::admit(
        &inventory,
        &axes,
        &registry,
        &radian,
        &mut heap,
        &mut account,
        (&certificate, &(0..12).collect::<Vec<_>>(), false),
    )?;
    let handle = owner.handle(0, &mut account)?;
    drive::issue(&mut owner, 2, &mut heap, &mut account)?;
    let outside = drive::drain(
        &mut owner,
        LcGraphState::Published,
        false,
        &mut heap,
        &mut account,
    )?
    .ok_or("outside reader")?;
    super::renewal::run(
        &owner,
        handle,
        &inventory,
        &axes,
        &registry,
        &mut heap,
        &mut account,
    )?;
    outside.release(&mut account).map_err(|(_, error)| error)?;
    fixture::retire(masks, &mut heap, &mut account)?;
    drive::complete(&owner, &mut heap, &mut account)
}

/// Duplicate and subsequently retired routes both preserve the other genuine occupation.
fn routing<P: Scalar + 'static>(
    owner: &mut LcGraphOwner<'_, P, 5>,
    handle: LcGraphHandle,
    other: OwnedLoan<P>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result<OwnedLoan<P>> {
    let ticket = other.ticket();
    let LcGraphInputReturn::Rejected {
        loan: other,
        error: LcGraphError::Native(LoanError::WrongLoan),
    } = owner.return_input(handle, other, heap, account)
    else {
        return Err("duplicate route preserved rightful loan".into());
    };
    assert_eq!(other.ticket(), ticket);
    settle_prefix(owner, heap, account)?;
    let LcGraphInputReturn::Rejected {
        loan: other,
        error: LcGraphError::Native(LoanError::StaleOccupation),
    } = owner.return_input(handle, other, heap, account)
    else {
        return Err("retired handle preserves rightful loan".into());
    };
    assert_eq!(other.ticket(), ticket);
    Ok(other)
}
