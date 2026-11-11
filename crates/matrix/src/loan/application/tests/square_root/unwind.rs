// SPDX-License-Identifier: AGPL-3.0-only
//! Unwinding at the final publication allocation destroys the detached producer exactly once.

use super::{TestResult, candidate, fixture, ready};
use crate::loan::fault;
use crate::{AbsenceReason, ProducerOutcome, Residence, ScanOutcome};
use nrz_bindings::Limits;
use std::rc::Rc;

/// No scalar is repeated, the input edge ends first, and one authenticated abandonment is delivered.
#[test]
fn square_root_late_publication_shell_unwind() -> TestResult {
    let mut setup = fixture::<f64>(1, 1, 3, 2, false, Limits::default())?;
    let demand = setup.request.demand;
    let owner = ready(
        setup
            .heap
            .request_pointwise_square_root(setup.request, &mut setup.account),
    )?;
    setup.heap.retire_mask(demand, &mut setup.account.ledger)?;
    let owner = candidate(owner, &mut setup.account)?;
    let ticket = owner.ticket();
    let source = Rc::downgrade(&owner.core.input.as_ref().ok_or("source")?.record);
    let witness = Rc::downgrade(&owner.core.host.witness);
    fault::watch_source_release(ticket.identity.receipt, source.clone());
    let released = fault::numerical_releases();
    let before = setup.account.usage().work;
    fault::reset_scalars();
    let injection = fault::Injection::unwinding_after(0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _result = setup.heap.publish_application(owner, &mut setup.account);
    }));
    assert!(result.is_err());
    assert_eq!(fault::Injection::attempts(), 1);
    assert_eq!(fault::scalars(), 0);
    // Fixed 118 plus 3*L+7*q+1+C+3*k*z = 15 visits.
    assert_eq!(setup.account.usage().work - before, 133);
    assert_eq!(
        fault::numerical_releases(),
        [released[0] + 3, released[1] + 3, released[2] + 192]
    );
    assert_eq!(source.strong_count(), 0);
    assert_eq!(witness.strong_count(), 0);
    assert_eq!(fault::source_at_witness(), Some(0));
    drop(injection);
    assert!(matches!(
        setup
            .heap
            .reconcile_detached(&mut setup.account, 1000, None),
        ScanOutcome::Complete
    ));
    assert!(
        matches!(setup.heap.take_producer_outcome(ticket, &mut setup.account)?, ProducerOutcome::Absent(receipt) if receipt.reason() == AbsenceReason::Abandoned)
    );
    assert_eq!(
        setup
            .heap
            .take_producer_outcome(ticket, &mut setup.account)?,
        ProducerOutcome::AlreadyDelivered
    );
    assert!(matches!(
        setup.heap.residence(&mut setup.account, 1000)?,
        Residence::Empty {
            retained_bytes: 14_352
        }
    ));
    assert_eq!(setup.heap.loans.elements()?, [0; 2]);
    assert!(setup.heap.loans.debits.iter().all(Option::is_none));
    Ok(())
}
