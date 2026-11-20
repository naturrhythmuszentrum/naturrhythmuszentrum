// SPDX-License-Identifier: AGPL-3.0-only
//! Covered storage is privately acquired after same-ledger byte and cleanup admission.

use super::{Arguments, Backing, Borrowed, ExecutionAccount, Inventory, LoanError, Reservation};
use super::{inventory::add, plan::Plan};
use crate::loan::{charge, preparation::Preparation, registry::CleanupReceipt};
use nrz_bindings::readings::algebra::limits::BudgetKind;
use nrz_scalar::Scalar;
use std::rc::Rc;

impl Reservation {
    /// Production uses the original heap identity source, never a fresh identity stream.
    pub(crate) fn reserve<P: Scalar + 'static>(
        args: Arguments<'_>,
        prior: &[Borrowed<'_>],
        account: &mut ExecutionAccount,
    ) -> Result<Self, LoanError> {
        Self::prepare::<P>(args, prior, account, crate::heap::identity::identity)
    }
    /// The actual admission path accepts an isolated identity source in boundary controls.
    #[cfg(test)]
    pub(super) fn reserve_with<P: Scalar + 'static>(
        args: Arguments<'_>,
        prior: &[Borrowed<'_>],
        account: &mut ExecutionAccount,
        source: &std::sync::atomic::AtomicU64,
    ) -> Result<Self, LoanError> {
        Self::prepare::<P>(args, prior, account, || {
            crate::heap::identity::allocate_identity(source)
        })
    }
    /// Prior owners are authenticated and deduplicated; uncovered rows remain outside scope.
    fn prepare<P: Scalar + 'static>(
        args: Arguments<'_>,
        prior: &[Borrowed<'_>],
        account: &mut ExecutionAccount,
        identity: impl FnOnce() -> Result<u64, crate::HeapError>,
    ) -> Result<Self, LoanError> {
        Inventory::authenticate(prior, account)?;
        let plan =
            account
                .ledger
                .with_work_phase("matrix.graph", "owner reservation", |budget| {
                    let existing = Inventory::measure(prior, budget)?;
                    // N: inspect every proposed attempt record before checked summation.
                    charge(budget, args.attempts.len())?;
                    let plan = Plan::new(args)?;
                    let combined = u64::try_from(add(existing.covered_bytes, plan.retained)?)
                        .map_err(|_| LoanError::ByteOverflow)?;
                    budget.check(BudgetKind::PayloadBytes, combined, crate::loan::AT)?;
                    budget.check(BudgetKind::StagedPayloadBytes, combined, crate::loan::AT)?;
                    charge(budget, plan.work)?;
                    Ok::<_, LoanError>(plan)
                })?;
        let identity = identity()?;
        let mut preparation = Preparation::new(CleanupReceipt {
            identity,
            allocations: plan.allocations,
            records: 1,
            edges: 0,
        });
        let mut nodes: Vec<super::records::Node> = buffer(args.nodes, &mut preparation)?;
        for (node, attempt) in nodes.iter_mut().zip(args.attempts) {
            node.generation = 1;
            node.attempts = *attempt;
        }
        let backing = Backing {
            account: account.identity,
            identity,
            drive: args.drive,
            proposed: args.attempts.len(),
            format: crate::storage::Format::of::<P>(),
            control: super::Control::default(),
            cleanup: CleanupReceipt::empty(identity),
            nodes,
            edges: buffer(args.edges, &mut preparation)?,
            ready: buffer(plan.ready, &mut preparation)?,
            stack: buffer(args.nodes, &mut preparation)?,
            marks: buffer(args.nodes, &mut preparation)?,
            scopes: buffer(args.scopes, &mut preparation)?,
            membership: buffer(plan.membership, &mut preparation)?,
        };
        #[cfg(test)]
        crate::loan::fault::before()?;
        // Rc allocation follows the repository's prepaid shared-header construction convention.
        let mut backing = Rc::new(backing);
        preparation.acquired(false)?;
        let owner = Rc::get_mut(&mut backing).ok_or(LoanError::WrongLoan)?;
        owner.cleanup = preparation.finish()?;
        Ok(Self { backing })
    }
}
/// Exact capacity is checked by the common native allocator; unexpected excess is refused.
fn buffer<T: Default + Clone>(
    capacity: usize,
    preparation: &mut Preparation,
) -> Result<Vec<T>, LoanError> {
    let mut values = crate::loan::vector(capacity)?;
    if capacity != 0 {
        preparation.acquired(false)?;
    }
    values.resize(capacity, T::default());
    Ok(values)
}
