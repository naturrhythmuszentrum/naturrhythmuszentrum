// SPDX-License-Identifier: AGPL-3.0-only
//! Full native error carriers survive typed refusal, late failure and original-closure repair.
use super::super::{
    drive::Event,
    native::{Failure, Native},
    node::{Contract, State},
    owner::Owner,
};
use super::{
    fixture::{self, TestResult},
    pending,
};
use crate::loan::{
    graph::walk::Error,
    lc_shape::tests::output::{Setup, setup},
};
use crate::{ExecutionAccount, Heap, LoanError};
use nrz_bindings::{Limits, readings::algebra::enclosure::lc_root::Certificate};
use nrz_scalar::Scalar;

/// An immutable malformed replay remains terminal and returns the identical original declaration.
pub(super) fn typed<P: Scalar + 'static>() -> TestResult {
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        radian,
    } = setup::<P>(Limits::default(), [2, 3, 2])?;
    let certificate = crate::loan::lc_shape::tests::output::graph_certificate(
        &inventory,
        &registry,
        &radian,
        &mut account,
    )?;
    let mut bytes = certificate.to_bytes(&mut account.ledger)?;
    bytes[..8].copy_from_slice(b"NRZENC01");
    let wrong = Certificate::from_bytes(&bytes, &mut account.ledger)?;
    let (mut owner, _) = fixture::admit_selected(
        &inventory,
        &axes,
        &registry,
        &radian,
        &mut heap,
        &mut account,
        ([8; 12], &(0..12).collect::<Vec<_>>(), Some(&wrong)),
    )?;
    let Contract::Omega { specialization, .. } = owner.backing.nodes[10].contract else {
        return Err("omega contract".into());
    };
    issue(&mut owner, &mut heap, &mut account)?;
    let omega = owner.handle(10, &mut account)?;
    for _ in 0..128 {
        let event = owner.drive_one(&mut heap, &mut account, &[])?;
        if let Event::PreparationRefused {
            node,
            declaration,
            error,
            probe,
            evidence,
            control,
        } = event
        {
            assert_eq!(node, omega);
            assert_eq!(declaration.specialization_key(), specialization);
            assert!(std::ptr::eq(declaration.inventory(), &raw const inventory));
            assert!(matches!(
                error,
                crate::LcShapeError::Declaration(nrz_bindings::AdmissionError::Algebra(_))
            ));
            control?;
            owner.acknowledge_probe(probe?, &mut account)?;
            assert_eq!(owner.backing.nodes[10].state, State::Refused);
            assert!(owner.backing.nodes[10].native.is_none());
            owner.acknowledge_return(omega, evidence?, &heap, &mut account)?;
            assert!(matches!(
                owner.retry_original(omega, &heap, &mut account),
                Err(Error::InvalidPhase)
            ));
            drop(declaration);
            return Ok(());
        }
        settle(&mut owner, event, &heap, &mut account)?;
    }
    Err("bounded typed preparation".into())
}

/// A late defensive corruption produces Failed after a prefix; no graph retry restores evaluation.
pub(super) fn execution<P: Scalar + 'static>() -> TestResult {
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        radian,
    } = setup::<P>(Limits::default(), [2, 3, 2])?;
    let (mut owner, _) = fixture::admit(
        &inventory,
        &axes,
        &registry,
        &radian,
        &mut heap,
        &mut account,
    )?;
    issue(&mut owner, &mut heap, &mut account)?;
    pending::prepared(&mut owner, &mut heap, &mut account)?;
    let event = pending::next_native(&mut owner, &mut heap, &mut account)?;
    settle(&mut owner, event, &heap, &mut account)?;
    let Some(Native::OmegaPending(native)) = &mut owner.backing.nodes[10].native else {
        return Err("pending owner".into());
    };
    let ticket = native.ticket();
    crate::loan::lc_origin::pending::execution::checks::late_outcome(native);
    crate::loan::fault::reset_reads();
    let Event::Refused {
        node,
        error: Failure::Execution(crate::LcProductError::Kernel(crate::KernelError::Contract)),
        probe: Some(probe),
        control,
    } = pending::next_native(&mut owner, &mut heap, &mut account)?
    else {
        return Err("cancel-only failure".into());
    };
    control?;
    // Eleven full cells precede the malformed final record, each with five three-channel sources.
    assert_eq!(crate::loan::fault::reads(), 11 * 5 * 3);
    owner.acknowledge_probe(probe, &mut account)?;
    assert_eq!(owner.backing.nodes[10].state, State::Repair);
    assert!(matches!(
        owner.backing.nodes[10].native,
        Some(Native::OmegaFailed(_))
    ));
    assert!(matches!(
        owner.retry_original(node, &heap, &mut account),
        Err(Error::InvalidPhase)
    ));
    assert!(heap.acquire_pin::<P>(ticket, &mut account).is_err());
    cancelled(&mut owner, &mut heap, &mut account)?;
    assert!(matches!(
        heap.take_producer_outcome(ticket, &mut account)?,
        crate::ProducerOutcome::AlreadyDelivered
    ));
    Ok(())
}

/// Restore the exact temporarily unavailable occupation; publication resumes without reevaluation.
pub(super) fn repair<P: Scalar + 'static>(cancel: bool) -> TestResult {
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        radian,
    } = setup::<P>(Limits::default(), [2, 3, 2])?;
    let (mut owner, _) = fixture::admit(
        &inventory,
        &axes,
        &registry,
        &radian,
        &mut heap,
        &mut account,
    )?;
    issue(&mut owner, &mut heap, &mut account)?;
    pending::prepared(&mut owner, &mut heap, &mut account)?;
    for _ in 0..2 {
        let event = pending::next_native(&mut owner, &mut heap, &mut account)?;
        settle(&mut owner, event, &heap, &mut account)?;
    }
    let Some(Native::OmegaEvaluated(candidate)) = &owner.backing.nodes[10].native else {
        return Err("evaluated owner".into());
    };
    let ticket = candidate.ticket();
    let saved = heap.loans.groups[ticket.identity.root.slot]
        .take()
        .ok_or("original occupation")?;
    crate::loan::fault::reset_reads();
    let refused = pending::next_native(&mut owner, &mut heap, &mut account);
    heap.loans.groups[ticket.identity.root.slot] = Some(saved);
    let Event::Refused {
        node,
        error: Failure::Native(LoanError::StaleOccupation),
        probe: Some(probe),
        control,
    } = refused?
    else {
        return Err("stale candidate".into());
    };
    control?;
    owner.acknowledge_probe(probe, &mut account)?;
    let Some(Native::OmegaEvaluated(candidate)) = &owner.backing.nodes[10].native else {
        return Err("same evaluated candidate".into());
    };
    assert_eq!(candidate.ticket(), ticket);
    assert_eq!(owner.backing.nodes[10].state, State::Repair);
    if cancel {
        cancelled(&mut owner, &mut heap, &mut account)?;
    } else {
        owner.retry_original(node, &heap, &mut account)?;
        let event = pending::next_native(&mut owner, &mut heap, &mut account)?;
        settle(&mut owner, event, &heap, &mut account)?;
        assert!(matches!(
            owner.backing.nodes[10].native,
            Some(Native::OmegaPublished(_))
        ));
    }
    assert_eq!(crate::loan::fault::reads(), 0);
    Ok(())
}

/// Every input enters its genuine native loan and authenticated handback.
fn issue<P: Scalar + 'static>(
    owner: &mut Owner<'_, P, 5>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult {
    for position in 0..5 {
        fixture::publish_input(owner, position, heap, account)?;
    }
    Ok(())
}

/// Only committed probes and original producer outcomes are acknowledged here.
fn settle<P: Scalar + 'static>(
    owner: &mut Owner<'_, P, 5>,
    event: Event<'_, P, 5>,
    heap: &Heap,
    account: &mut ExecutionAccount,
) -> TestResult {
    match event {
        Event::Advanced { probe, control, .. } => {
            control?;
            owner.acknowledge_probe(probe, account)?;
        }
        Event::Terminal {
            node,
            evidence,
            control,
        } => {
            assert!(node.slot < 10);
            control?;
            owner.acknowledge_outcome(node, evidence, heap, account)?;
        }
        Event::Control => {}
        other => return Err(format!("native phase progression: {other:?}").into()),
    }
    Ok(())
}

/// Explicit cancellation consumes only the original held continuation and its own delivery.
pub(super) fn cancelled<P: Scalar + 'static>(
    owner: &mut Owner<'_, P, 5>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult {
    let omega = owner.handle(10, account)?;
    let Event::Terminal {
        node,
        evidence,
        control,
    } = owner.cancel_node(omega, heap, account)?
    else {
        return Err("explicit cancellation".into());
    };
    control?;
    owner.acknowledge_outcome(node, evidence, heap, account)?;
    assert!(matches!(
        owner.backing.nodes[10].native,
        Some(Native::Cancelled)
    ));
    Ok(())
}
