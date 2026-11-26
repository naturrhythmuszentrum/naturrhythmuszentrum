// SPDX-License-Identifier: AGPL-3.0-only
//! Refusal controls retain original capabilities and distinguish notification from host authority.

use super::super::{
    admission::Refusal,
    drive::Event,
    input::{InputRequest, InputReturn},
    node::{Node, State},
    owner::{Capacities, Owner},
    scope::Progress,
};
use super::fixture::{self, TestResult};
use crate::loan::graph::walk::Error;
use crate::loan::lc_shape::tests::output::{Setup, setup};
use crate::{ExecutionAccount, LoanError};
use nrz_bindings::{Budget, Limits};

/// Cardinality refusal occurs before moving or allocating any native declaration storage.
pub(super) fn admission() -> TestResult {
    let Setup {
        heap,
        mut account,
        inventory,
        axes,
        registry,
        ..
    } = setup::<f64>(Limits::default(), [2, 3, 2])?;
    let declaration = inventory
        .input(0)
        .ok_or("original")?
        .source(&axes, &mut account)?;
    let nodes = vec![Node::input(declaration)];
    let before = account.usage().work;
    let Err(Refusal { nodes, error }) = Owner::admit(
        nodes,
        &[],
        Capacities {
            nodes: 1,
            edges: 0,
            scopes: 0,
            drive: 1,
            records: 2,
            words: 11,
            witnesses: 0,
        },
        &inventory,
        &registry,
        &[],
        &heap,
        &mut account,
    ) else {
        return Err("cardinality refusal".into());
    };
    assert_eq!(nodes.len(), 1);
    assert!(matches!(
        error,
        Error::Native(LoanError::UnsupportedContract)
    ));
    assert_eq!(account.usage().work, before);
    Ok(())
}

/// An allocation refusal consumes one real attempt; an explicit epoch returns the original host loan.
pub(super) fn waiting() -> TestResult {
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        radian,
    } = setup::<f64>(Limits::default(), [2, 3, 2])?;
    let (mut owner, _) = fixture::admit(
        &inventory,
        &axes,
        &registry,
        &radian,
        &mut heap,
        &mut account,
    )?;
    let input = owner.handle(0, &mut account)?;
    let injection = crate::loan::fault::Injection::after(0);
    let refused = owner.request_input(input, &mut heap, &mut account, &[])?;
    drop(injection);
    let InputRequest::Refused {
        error,
        evidence,
        control,
    } = refused
    else {
        return Err("native allocation refusal".into());
    };
    assert!(matches!(error, LoanError::AllocationRefused));
    control?;
    let receipt = evidence.ok_or("probe receipt")?;
    let pull = owner.pull(receipt, &mut account)?;
    assert_eq!(pull.bytes(), 57);
    assert!(owner.acknowledge_probe(receipt, &mut account).is_err());
    drop(pull);
    let witness = owner.witness(receipt, &mut account)?;
    owner.acknowledge_probe(receipt, &mut account)?;
    assert_eq!(owner.backing.core.reservation.backing.nodes[0].attempts, 7);
    owner.begin_capacity_epoch(&mut account)?;
    let Event::Input {
        node,
        outcome:
            InputRequest::Granted {
                loan,
                evidence: Ok(evidence),
                control,
            },
    } = owner.drive_wait_one(&mut heap, &mut account, &[])?
    else {
        return Err("bounded native retry".into());
    };
    assert_eq!(node, input);
    control?;
    owner.acknowledge_probe(evidence, &mut account)?;
    assert_eq!(owner.backing.core.reservation.backing.nodes[0].attempts, 6);
    let ticket = loan.ticket();
    let mut foreign = ExecutionAccount::new(Budget::new(Limits::default())?)?;
    let InputReturn::Rejected { loan, error } =
        owner.return_input(input, loan, &mut heap, &mut foreign)
    else {
        return Err("foreign return accepted".into());
    };
    assert!(matches!(error, Error::Native(LoanError::ForeignAccount)));
    assert_eq!(loan.ticket(), ticket);
    let crate::CancelOutcome::Cancelled(receipt) = heap.cancel_loan(loan, &mut account) else {
        return Err("native cancellation".into());
    };
    owner.input_cancelled(input, receipt, &heap, &mut account)?;
    drop(owner);
    assert_eq!(
        witness.status(&mut account)?,
        crate::loan::graph::sink::Status::Abandoned
    );
    Ok(())
}

/// Notification closes only its owned unissued input and leaves a genuine issued loan mandatory.
pub(super) fn notification() -> TestResult {
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        radian,
    } = setup::<f64>(Limits::default(), [2, 3, 2])?;
    let (mut owner, _) = fixture::admit(
        &inventory,
        &axes,
        &registry,
        &radian,
        &mut heap,
        &mut account,
    )?;
    let first = owner.handle(0, &mut account)?;
    let second = owner.handle(1, &mut account)?;
    let borrowed = owner.borrowed();
    let bytes = owner.graph_bytes(&[], &mut account)?;
    assert_eq!(
        owner.graph_bytes(&[borrowed, borrowed], &mut account)?,
        bytes
    );
    let InputRequest::Granted {
        loan,
        evidence: Ok(evidence),
        control,
    } = owner.request_input(second, &mut heap, &mut account, &[])?
    else {
        return Err("issued expectation".into());
    };
    control?;
    owner.acknowledge_probe(evidence, &mut account)?;
    let scope = owner.begin_scope(&[first, second], &[first, second], &mut account)?;
    for _ in 0..32 {
        if owner.close_scope(scope, &mut account)? == Progress::Published {
            break;
        }
    }
    for _ in 0..13 {
        if owner.notify_one(scope, &heap, &mut account)? == Progress::Published {
            break;
        }
    }
    assert_eq!(owner.backing.nodes[0].state, State::Absent);
    assert_eq!(owner.backing.nodes[1].state, State::Borrowed);
    assert_eq!(owner.backing.nodes[2].state, State::Pending);
    assert_eq!(
        owner.notify_one(scope, &heap, &mut account)?,
        Progress::Published
    );
    assert!(owner.cancel_node(second, &mut heap, &mut account).is_err());
    let third = owner.handle(2, &mut account)?;
    assert!(owner.retry_original(third, &heap, &mut account).is_err());
    let Event::Terminal {
        node,
        evidence,
        control,
    } = owner.cancel_node(third, &mut heap, &mut account)?
    else {
        return Err("cancelled expectation".into());
    };
    assert_eq!(node, third);
    control?;
    owner.acknowledge_outcome(node, evidence, &heap, &mut account)?;
    let crate::CancelOutcome::Cancelled(receipt) = heap.cancel_loan(loan, &mut account) else {
        return Err("native cancellation".into());
    };
    owner.input_cancelled(second, receipt, &heap, &mut account)?;
    Ok(())
}

/// Original zero attempts reserve both terminal receipts before touching native request state.
pub(super) fn exhaustion() -> TestResult {
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        radian,
    } = setup::<f64>(Limits::default(), [2, 3, 2])?;
    let (mut owner, _) = fixture::admit_attempts(
        &inventory,
        &axes,
        &registry,
        &radian,
        &mut heap,
        &mut account,
        [0; 12],
    )?;
    let input = owner.handle(0, &mut account)?;
    let InputRequest::AttemptsExhausted {
        evidence,
        outcome,
        control,
    } = owner.request_input(input, &mut heap, &mut account, &[])?
    else {
        return Err("finite attempt exhaustion".into());
    };
    control?;
    assert_eq!(owner.pull(evidence, &mut account)?.bytes(), 33);
    assert_eq!(owner.pull(outcome, &mut account)?.bytes(), 41);
    owner.acknowledge_probe(evidence, &mut account)?;
    assert_eq!(owner.backing.nodes[0].state, State::AttemptsExhausted);
    Ok(())
}
