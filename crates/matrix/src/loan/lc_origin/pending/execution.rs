// SPDX-License-Identifier: AGPL-3.0-only
//! Consuming readiness and evaluation retain private owners across every refused phase.
#[cfg(test)]
pub(in crate::loan) mod checks;
mod outcome;
mod readiness;
mod surface;
use super::{LcApplicationPending, Storage};
use crate::loan::{
    charge,
    payload::{bit, set},
};
use crate::{ExecutionAccount, LcProductError, LoanError};
use nrz_scalar::Scalar;
use std::rc::Rc;
pub use surface::{LcApplicationEvaluated, LcApplicationEvaluationFailure, LcApplicationFailed};

/// Only this module can issue the complete-traversal entitlement after Work admission.
pub(in crate::loan) struct Traversal(());

/// The mutable output borrows are disjoint from the captured immutable source closure.
fn run<P: Scalar + 'static, const N: usize>(
    owner: &mut LcApplicationPending<'_, P, N>,
    account: &mut ExecutionAccount,
    started: &mut bool,
) -> Result<(), LcProductError> {
    account.authenticate(owner.identity.account)?;
    let native = owner.native.as_mut().ok_or(LoanError::WrongLoan)?;
    let Storage {
        identity,
        liveness,
        schedule,
        closure,
        demand_layout,
        demand_mask,
        demand,
        channels,
        outcomes,
        completed,
        ..
    } = &mut *native.storage;
    let source = readiness::Source {
        identity,
        liveness: &liveness.identity,
        schedule,
        closure,
        demand_layout,
        demand_mask,
        demand: *demand,
    };
    let ready = account
        .ledger
        .with_work_phase(crate::loan::FAMILY, "LC readiness", |budget| {
            readiness::prepare(owner.inventory, &source, budget)
        })?;
    account
        .ledger
        .with_work_phase(crate::loan::FAMILY, "LC evaluation", |budget| {
            let _span = tracing::trace_span!(
                "nrz.walk",
                family = "lc-origin",
                application = identity.request
            )
            .entered();
            charge(budget, 12)?;
            let words = ready.cells.div_ceil(64);
            if outcomes.len() != ready.cells
                || completed.words.len() != words
                || channels.iter().any(|channel| {
                    !channel.numerical.matches(identity.format, ready.cells)
                        || channel.components.len() != words
                        || channel.complete.len() != words
                })
            {
                return Err(LoanError::LayoutMismatch.into());
            }
            let completed = Rc::get_mut(completed).ok_or(LoanError::WrongLoan)?;
            let visits = N
                .checked_mul(7)
                .and_then(|n| n.checked_add(19))
                .and_then(|n| n.checked_mul(ready.demanded))
                .and_then(|n| n.checked_add(ready.cells))
                .and_then(|n| n.checked_add(1))
                .ok_or(LoanError::CountOverflow)?;
            charge(budget, visits)?;
            let permit = Traversal(());
            *started = true;
            for (cell, record) in outcomes.iter_mut().enumerate() {
                let requested = bit(&demand_mask.words, cell);
                if record.status != u8::from(requested) || record.target != 0 || record.reason != 0
                {
                    return Err(crate::KernelError::Contract.into());
                }
                if !requested {
                    continue;
                }
                let result = ready.product.omega_prepaid(
                    cell,
                    account.identity,
                    budget.usage().work,
                    &permit,
                );
                let (outcome, values) = outcome::convert(result, cell)?;
                if let Some(values) = values {
                    for (channel, value) in channels.iter_mut().zip(values) {
                        channel.numerical.write(cell, value)?;
                        set(&mut channel.components, cell, true);
                        set(&mut channel.complete, cell, true);
                    }
                    set(&mut completed.words, cell, true);
                }
                *record = outcome;
            }
            Ok(())
        })
}
