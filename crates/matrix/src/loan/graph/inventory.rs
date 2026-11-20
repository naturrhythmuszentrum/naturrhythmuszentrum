// SPDX-License-Identifier: AGPL-3.0-only
//! Scoped allocation identity, not equal byte content, decides overlap.

use super::records::{Edge, Frame, Node, Scope};
use super::{Backing, Borrowed, ExecutionAccount, LoanError, charge};

/// Worksheet ownership coverage, never an invented zero for an unknown owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Row {
    /// Actual covered retained allocation capacity.
    Covered(usize),
    /// Existing native/bindings owners are outside this storage reservation's projection.
    OutsideScope,
    /// Backend or delivery/history allowance is not justified by this increment.
    Deferred,
}
/// An allocation-free scoped residence snapshot, not a graph completion result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Inventory {
    /// Header/nodes, incidence/index, traversal/scopes, bindings, native, backend, history.
    pub(crate) rows: [Row; 7],
    /// Distinct retained backing owners, including owners held only by inspectors.
    pub(crate) owners: usize,
    /// Enumerated references, kept distinct from deduplicated payload bytes.
    pub(crate) references: usize,
    /// Covered backing bytes only; not a complete graph or heap total.
    pub(crate) covered_bytes: usize,
}
impl Inventory {
    /// Inspect the supplied live owner set; duplicates never duplicate shared payload.
    pub(crate) fn inspect(
        owners: &[Borrowed<'_>],
        account: &mut ExecutionAccount,
    ) -> Result<Self, LoanError> {
        Self::authenticate(owners, account)?;
        account
            .ledger
            .with_work_phase("matrix.graph", "reservation inventory", |budget| {
                Self::measure(owners, budget)
            })
    }
    /// Existing account authentication remains uncharged and precedes every observation.
    pub(super) fn authenticate(
        owners: &[Borrowed<'_>],
        account: &ExecutionAccount,
    ) -> Result<(), LoanError> {
        for owner in owners {
            account.authenticate(owner.0.account)?;
        }
        Ok(())
    }
    /// Shared preflight body: its H entry belongs to the calling reservation or inspection.
    pub(super) fn measure(
        owners: &[Borrowed<'_>],
        budget: &mut nrz_bindings::Budget,
    ) -> Result<Self, LoanError> {
        charge(budget, 1)?;
        let mut totals = [0_usize; 3];
        let mut distinct = 0_usize;
        for (index, owner) in owners.iter().enumerate() {
            let mut repeated = false;
            for previous in &owners[..index] {
                // A: compare allocation identities, distinct from the uncharged account check.
                charge(budget, 1)?;
                if std::ptr::eq(previous.0, owner.0) {
                    repeated = true;
                    break;
                }
            }
            if !repeated {
                // H: enter inspection of this distinct backing header.
                charge(budget, 1)?;
                let rows = owner.0.rows()?;
                for (total, bytes) in totals.iter_mut().zip(rows) {
                    *total = add(*total, bytes)?;
                }
                distinct = add(distinct, 1)?;
            }
        }
        Ok(Self {
            rows: [
                Row::Covered(totals[0]),
                Row::Covered(totals[1]),
                Row::Covered(totals[2]),
                Row::OutsideScope,
                Row::OutsideScope,
                Row::Deferred,
                Row::Deferred,
            ],
            owners: distinct,
            references: owners.len(),
            covered_bytes: add(add(totals[0], totals[1])?, totals[2])?,
        })
    }
}
impl Backing {
    /// Actual physical vector capacities; fixed headers occur once in the shared owner.
    pub(super) fn rows(&self) -> Result<[usize; 3], LoanError> {
        let fixed = super::super::rc_bytes::<Self>();
        let nodes = add(fixed, bytes::<Node>(self.nodes.capacity())?)?;
        let incidence = add(
            bytes::<Edge>(self.edges.capacity())?,
            bytes::<u64>(self.ready.capacity())?,
        )?;
        let traversal = add(
            add(
                bytes::<Frame>(self.stack.capacity())?,
                bytes::<u64>(self.marks.capacity())?,
            )?,
            add(
                bytes::<Scope>(self.scopes.capacity())?,
                bytes::<u64>(self.membership.capacity())?,
            )?,
        )?;
        Ok([nodes, incidence, traversal])
    }
}
/// Checked addition keeps unknown or overflowing counts out of valid inventories.
pub(super) fn add(left: usize, right: usize) -> Result<usize, LoanError> {
    left.checked_add(right).ok_or(LoanError::ByteOverflow)
}
/// Include allocated vacancies, not merely initialized logical entries.
pub(super) fn bytes<T>(capacity: usize) -> Result<usize, LoanError> {
    capacity
        .checked_mul(size_of::<T>())
        .ok_or(LoanError::ByteOverflow)
}
