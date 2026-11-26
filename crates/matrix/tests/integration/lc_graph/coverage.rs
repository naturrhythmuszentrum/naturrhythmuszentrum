// SPDX-License-Identifier: AGPL-3.0-only
//! Producer-first refusal preserves zero-capacity and mixed cancellation causes.
use super::{
    drive,
    fixture::{self, Result, Setup},
};
use nrz_bindings::Limits;
use nrz_matrix::*;
use nrz_scalar::Scalar;

pub(super) fn run<P: Scalar + 'static>() -> Result {
    for zero in [false, true] {
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
            Limits::default(),
            [if zero { 0 } else { 2 }, 3, 2],
        )?;
        let mut attempts = [8; 12];
        if !zero {
            attempts[0] = 0;
        }
        let selected = (0..axes.combined.axis().capacity()).collect::<Vec<_>>();
        let (mut owner, _) = fixture::admit_attempts(
            &inventory,
            &axes,
            &registry,
            &radian,
            &mut heap,
            &mut account,
            ((&certificate, &selected, false), attempts),
        )?;
        for position in 0..5 {
            issue(&mut owner, position, zero, &mut heap, &mut account)?;
        }
        assert!(
            drive::drain(
                &mut owner,
                LcGraphState::DependencyRefused(5),
                false,
                &mut heap,
                &mut account
            )?
            .is_none()
        );
    }
    Ok(())
}
fn issue<P: Scalar + 'static>(
    owner: &mut LcGraphOwner<'_, P, 5>,
    position: usize,
    zero: bool,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result {
    let handle = owner.handle(position, account)?;
    if !zero && position == 1 {
        let LcGraphEvent::Terminal {
            evidence, control, ..
        } = owner.cancel_node(handle, heap, account)?
        else {
            return Err("explicit expectation cancellation".into());
        };
        control?;
        owner.acknowledge_outcome(handle, evidence, heap, account)?;
        return Ok(());
    }
    match owner.request_input(handle, heap, account, &[])? {
        LcGraphInputRequest::Granted {
            mut loan,
            evidence: Ok(evidence),
            control,
        } => {
            control?;
            owner.acknowledge_probe(evidence, account)?;
            fixture::fill(&mut loan, position, if zero { 0 } else { 2 }, account)?;
            let LcGraphInputReturn::Published { control } =
                owner.return_input(handle, loan, heap, account)
            else {
                return Err("partial native handback".into());
            };
            control?;
        }
        LcGraphInputRequest::AttemptsExhausted {
            evidence, control, ..
        } if !zero && position == 0 => {
            control?;
            owner.acknowledge_probe(evidence, account)?;
        }
        other => return Err(format!("mixed input: {other:?}").into()),
    }
    Ok(())
}
