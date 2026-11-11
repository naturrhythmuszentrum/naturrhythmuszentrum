// SPDX-License-Identifier: AGPL-3.0-only
//! Exhaust each fixed admission metadata prefix before the complete preparation prepayment.

use super::{TestResult, fixture};
use crate::loan::fault;
use crate::{ApplicationError, ApplicationRequest, LoanError};
use nrz_bindings::{
    Limits,
    readings::algebra::{Exhaustion, FaultKind, limits::BudgetKind},
};
use std::rc::Rc;

/// C=3, k=h=1 has 16*h+49+2*L=71 metadata visits before the final 165-step prepayment.
/// Every cut through those visits and N-1 of the complete 236-step admission returns the same request.
#[test]
fn reciprocal_every_metadata_work_prefix_preserves_request_without_allocation() -> TestResult {
    for remaining in (0..=71).chain(std::iter::once(235)) {
        let mut setup = fixture::<f64>(3, 1, 1, 2, false, Limits::default())?;
        let ticket = setup.request.input.ticket()?;
        let pin = Rc::as_ptr(&setup.request.input.record);
        let inputs = Rc::as_ptr(&setup.request.operation.inputs);
        let outputs = Rc::as_ptr(&setup.request.operation.outputs);
        let destination = setup.request.destination.identity;
        let demand = setup.request.demand;
        let before_bytes = setup.heap.resource_bytes()?;
        assert!(setup.account.usage().work > 0);
        setup.account.ledger.work(
            setup.account.limits().work - setup.account.usage().work - remaining,
            crate::loan::AT,
        )?;
        let before = setup.account.usage().work;
        let injection = fault::Injection::after(0);
        fault::reset_reads();
        fault::reset_scalars();
        fault::reset_stores();
        // Request/current checks, six structural visits, axis/attachment checks, planning scans,
        // common inventory and the complete cleanup/initialization/preparation prepayment.
        let charges = [
            1, 1, 1, 1, 6, 2, 1, 3, 1, 1, 1, 1, 1, 1, 4, 4, 1, 9, 9, 1, 21, 165,
        ];
        let mut spent = 0;
        let needed = charges
            .into_iter()
            .find_map(|charge| {
                let next = spent + charge;
                if next > remaining {
                    Some(next)
                } else {
                    spent = next;
                    None
                }
            })
            .ok_or("missing refusal boundary")?;
        let request = match setup
            .heap
            .request_pointwise_reciprocal(setup.request, &mut setup.account)
        {
            ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::Budget(fault)),
                request,
            ) => {
                assert_eq!(
                    fault.kind,
                    FaultKind::Exhausted {
                        reason: Exhaustion::Limit,
                        budget: BudgetKind::Work,
                        limit: setup.account.limits().work,
                        needed: before + needed
                    }
                );
                assert_eq!(fault.location, crate::loan::AT);
                request
            }
            other => return Err(format!("prefix {remaining}: {other:?}").into()),
        };
        assert_eq!(request.input.ticket()?, ticket);
        assert_eq!(Rc::as_ptr(&request.input.record), pin);
        assert_eq!(Rc::as_ptr(&request.operation.inputs), inputs);
        assert_eq!(Rc::as_ptr(&request.operation.outputs), outputs);
        assert_eq!(request.destination.identity, destination);
        assert_eq!(request.demand, demand);
        assert_eq!(setup.account.usage().work - before, spent);
        assert_eq!(
            [
                fault::reads(),
                fault::scalars(),
                fault::stores(),
                fault::Injection::attempts()
            ],
            [0; 4]
        );
        assert_eq!(setup.heap.resource_bytes()?, before_bytes);
        assert_eq!(setup.heap.loans.pending, 0);
        drop(injection);
    }
    Ok(())
}
