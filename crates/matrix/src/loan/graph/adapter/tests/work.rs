// SPDX-License-Identifier: AGPL-3.0-only
//! A failed prepayment may leave usable Work; it still permanently stops ordinary graph work.

use super::super::{
    drive::Event,
    gate::Gate,
    input::{InputRequest, InputReturn},
};
use super::fixture::{self, TestResult};
use crate::loan::graph::walk::Error;
use crate::loan::lc_shape::tests::output::{Setup, setup};
use crate::{ExecutionAccount, LoanError};
use nrz_bindings::{Budget, Limits};

/// H entry and A scope need two units; one remaining unit cannot admit the handback boundary.
pub(super) fn handback() -> TestResult {
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
    let InputRequest::Granted {
        loan,
        evidence: Ok(evidence),
        control,
    } = owner.request_input(input, &mut heap, &mut account, &[])?
    else {
        return Err("native host loan".into());
    };
    control?;
    owner.acknowledge_probe(evidence, &mut account)?;
    let ticket = loan.ticket();
    let spend = account.limits().work - account.usage().work - 1;
    crate::loan::charge(&mut account.ledger, usize::try_from(spend)?)?;
    let before = account.usage();
    let InputReturn::Rejected {
        loan,
        error: Error::Native(LoanError::Budget(_)),
    } = owner.return_input(input, loan, &mut heap, &mut account)
    else {
        return Err("original Work failure".into());
    };
    assert_eq!(loan.ticket(), ticket);
    assert_eq!(account.usage(), before);
    assert_eq!(account.limits().work - account.usage().work, 1);
    assert!(matches!(
        owner.drive_one(&mut heap, &mut account, &[]),
        Err(Error::WorkStopped)
    ));
    assert!(matches!(
        owner.begin_capacity_epoch(&mut account),
        Err(Error::WorkStopped)
    ));
    assert_eq!(account.usage(), before);
    let crate::CancelOutcome::Cancelled(receipt) = heap.cancel_loan(loan, &mut account) else {
        return Err("prepaid native cancellation".into());
    };
    assert_eq!(receipt.ticket(), ticket);
    assert_eq!(account.usage(), before);
    assert!(matches!(
        owner.input_cancelled(input, receipt, &heap, &mut account),
        Err(Error::WorkStopped)
    ));
    drop(owner);
    assert_eq!(account.usage(), before);
    carried()
}

/// A successful outer event cannot hide an original Work refusal in its control payload.
fn carried() -> TestResult {
    let mut account = ExecutionAccount::new(Budget::new(Limits {
        work: 1,
        ..Limits::default()
    })?)?;
    let gate = Gate::new(&account);
    let error = crate::loan::charge(&mut account.ledger, 2).expect_err("two visits exceed one");
    let event = Event::<f64, 5>::Control;
    gate.event(&event);
    assert!(!gate.stopped());
    let input = InputRequest::<f64>::Refused {
        error,
        evidence: None,
        control: Ok(()),
    };
    gate.input(&input);
    assert!(gate.stopped());
    Ok(())
}

/// Settled-owner inspection uses H1 + N2 + sink H1/M1; the following native scan cannot enter at zero Work.
pub(super) fn residence() -> TestResult {
    use super::super::{
        admission::Refusal,
        node::Node,
        owner::{Capacities, Owner},
        residence::Completion,
    };
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        ..
    } = setup::<f64>(Limits::default(), [2, 3, 2])?;
    // A retained empty demand gives the native scan real metadata slots to inspect.
    let _demand =
        crate::loan::lc_shape::tests::output::demand(&mut heap, &axes, &[], &mut account)?;
    let declaration = inventory
        .input(0)
        .ok_or("original input")?
        .source(&axes, &mut account)?;
    let mut owner = Owner::admit(
        vec![Node::input(declaration)],
        &[1],
        Capacities {
            nodes: 1,
            edges: 0,
            scopes: 0,
            drive: 100,
            records: 3,
            words: 19,
            witnesses: 0,
        },
        &inventory,
        &registry,
        &[],
        &heap,
        &mut account,
    )
    .map_err(|Refusal { error, .. }| error)?;
    let input = owner.handle(0, &mut account)?;
    let Event::Terminal {
        evidence, control, ..
    } = owner.cancel_node(input, &mut heap, &mut account)?
    else {
        return Err("original cancellation".into());
    };
    control?;
    owner.acknowledge_outcome(input, evidence, &heap, &mut account)?;
    assert!(matches!(
        owner.drive_one(&mut heap, &mut account, &[])?,
        Event::Control
    ));
    assert!(matches!(
        owner.drive_one(&mut heap, &mut account, &[])?,
        Event::Quiescent
    ));
    let spend = account.limits().work - account.usage().work - 5;
    crate::loan::charge(&mut account.ledger, usize::try_from(spend)?)?;
    assert!(matches!(
        Owner::completion(&[&owner], &mut heap, &mut account, 100_000)?,
        Completion::NativePending(..)
    ));
    assert!(owner.backing.gate.stopped());
    assert!(matches!(
        owner.begin_capacity_epoch(&mut account),
        Err(Error::WorkStopped)
    ));
    Ok(())
}
