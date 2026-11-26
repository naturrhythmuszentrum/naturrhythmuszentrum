// SPDX-License-Identifier: AGPL-3.0-only
//! Scoped notification preserves real borrowed host occupations through refusal and repair.
use super::super::{
    input::{InputRequest, InputReturn},
    node::State,
    owner::Owner,
    residence::Completion,
    scope::Progress,
};
use super::{
    coverage,
    fixture::{self, TestResult},
};
use crate::loan::{
    graph::walk::{Error, Handle},
    lc_shape::tests::output::{Setup, setup},
};
use crate::{ExecutionAccount, Heap, HostChannel, LoanError, OwnedLoan};
use nrz_bindings::Limits;
use nrz_scalar::Scalar;

/// Foreign and stale graph handles, a malformed payload and a duplicate route preserve the loan.
pub(super) fn run<P: Scalar + 'static>(invalid: P, cancel: bool) -> TestResult {
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        radian,
    } = setup::<P>(Limits::default(), [2, 3, 2])?;
    let (mut owner, demands) = fixture::admit(
        &inventory,
        &axes,
        &registry,
        &radian,
        &mut heap,
        &mut account,
    )?;
    let handles = (0..12)
        .map(|slot| owner.handle(slot, &mut account))
        .collect::<Result<Vec<_>, _>>()?;
    let mut loan = request(&mut owner, handles[0], &mut heap, &mut account)?;
    let other = request(&mut owner, handles[1], &mut heap, &mut account)?;
    fixture::fill(&mut loan, 0, &mut account)?;
    loan.write_component(HostChannel::Center, 0, 0, invalid, &mut account)?;
    notify(&mut owner, &handles, &heap, &mut account)?;
    assert_eq!(owner.backing.nodes[0].state, State::Borrowed);
    assert_eq!(owner.backing.nodes[1].state, State::Borrowed);
    let ticket = loan.ticket();
    loan = invalid_handles(&mut owner, handles[0], loan, &mut heap, &mut account)?;
    let InputReturn::Rejected {
        mut loan,
        error:
            Error::Native(LoanError::InvalidPayload {
                channel: HostChannel::Center,
                ..
            }),
    } = owner.return_input(handles[0], loan, &mut heap, &mut account)
    else {
        return Err("genuine malformed handback".into());
    };
    assert_eq!(loan.ticket(), ticket);
    assert_eq!(owner.backing.nodes[0].state, State::Borrowed);
    assert!(matches!(
        Owner::completion(&[&owner], &mut heap, &mut account, 100_000)?,
        Completion::GraphPending(_)
    ));
    if cancel {
        cancel_input(&mut owner, handles[0], loan, &mut heap, &mut account)?;
    } else {
        loan.write_component(HostChannel::Center, 0, 0, P::ONE, &mut account)?;
        let InputReturn::Published { control } =
            owner.return_input(handles[0], loan, &mut heap, &mut account)
        else {
            return Err("repaired same loan".into());
        };
        control?;
    }
    // The first occupation has already settled; another genuine loan cannot replay its handback.
    let other_ticket = other.ticket();
    let InputReturn::Rejected {
        loan: other,
        error: Error::Native(LoanError::WrongLoan),
    } = owner.return_input(handles[0], other, &mut heap, &mut account)
    else {
        return Err("double handback route".into());
    };
    assert_eq!(other.ticket(), other_ticket);
    assert_eq!(owner.backing.nodes[1].loan, Some(other_ticket));
    cancel_input(&mut owner, handles[1], other, &mut heap, &mut account)?;
    coverage::drain(
        &mut owner,
        State::MissingInput,
        false,
        &mut heap,
        &mut account,
    )?;
    for demand in demands {
        heap.retire_mask(demand, &mut account.ledger)?;
        heap.residence(&mut account, 100_000)?;
        heap.retire_cell_layout(demand.layout(), &mut account.ledger)?;
    }
    for _ in 0..8 {
        if matches!(
            Owner::completion(&[&owner], &mut heap, &mut account, 100_000)?,
            Completion::Complete { .. }
        ) {
            return Ok(());
        }
    }
    Err("full repaired/cancelled scope remained occupied".into())
}

/// Authentication fails before native consumption and keeps the actual outstanding ticket.
fn invalid_handles<P: Scalar + 'static>(
    owner: &mut Owner<'_, P, 5>,
    handle: Handle,
    mut loan: OwnedLoan<P>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult<OwnedLoan<P>> {
    let ticket = loan.ticket();
    for foreign in [true, false] {
        let mut invalid = handle;
        if foreign {
            invalid.owner += 1;
        } else {
            invalid.generation += 1;
        }
        let InputReturn::Rejected {
            loan: returned,
            error,
        } = owner.return_input(invalid, loan, heap, account)
        else {
            return Err("invalid graph handback".into());
        };
        if foreign {
            assert!(matches!(error, Error::Native(LoanError::ForeignOwner)));
        } else {
            assert!(matches!(error, Error::Native(LoanError::StaleOccupation)));
        }
        assert_eq!(returned.ticket(), ticket);
        loan = returned;
    }
    Ok(loan)
}

/// Only the exact native cancellation receipt closes the graph's borrowed occupation.
fn cancel_input<P: Scalar + 'static>(
    owner: &mut Owner<'_, P, 5>,
    handle: Handle,
    loan: OwnedLoan<P>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult {
    let crate::CancelOutcome::Cancelled(receipt) = heap.cancel_loan(loan, account) else {
        return Err("host cancellation".into());
    };
    owner.input_cancelled(handle, receipt, heap, account)?;
    Ok(())
}

/// Issuance acknowledges its probe but keeps the native host capability outside the owner.
fn request<P: Scalar + 'static>(
    owner: &mut Owner<'_, P, 5>,
    handle: Handle,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult<OwnedLoan<P>> {
    let InputRequest::Granted {
        loan,
        evidence: Ok(evidence),
        control,
    } = owner.request_input(handle, heap, account, &[])?
    else {
        return Err("host issuance".into());
    };
    control?;
    owner.acknowledge_probe(evidence, account)?;
    Ok(loan)
}

/// Repeated notification never changes the genuine borrower state.
fn notify<P: Scalar + 'static>(
    owner: &mut Owner<'_, P, 5>,
    handles: &[Handle],
    heap: &Heap,
    account: &mut ExecutionAccount,
) -> TestResult {
    let scope = owner.begin_scope(&[handles[11]], handles, account)?;
    for _ in 0..128 {
        if owner.close_scope(scope, account)? == Progress::Published {
            break;
        }
    }
    for _ in 0..13 {
        owner.notify_one(scope, heap, account)?;
    }
    for _ in 0..3 {
        assert_eq!(owner.notify_one(scope, heap, account)?, Progress::Published);
    }
    Ok(())
}
