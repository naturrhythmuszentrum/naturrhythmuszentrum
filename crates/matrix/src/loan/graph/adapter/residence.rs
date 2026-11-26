// SPDX-License-Identifier: AGPL-3.0-only
//! Completion is relative to a named owner set and the serialized native heap snapshot.

use super::{
    native::Native,
    node::{Contract, State},
    owner::Owner,
};
use crate::loan::graph::{sink::Status, walk::Error};
use crate::{ExecutionAccount, Heap, LoanError, Residence};
use nrz_scalar::Scalar;

/// Borrowed names preserve the caller's order and aliases without allocating another owner list.
#[derive(Debug)]
pub struct OwnerSet<'a, 'i, P: Scalar, const N: usize> {
    /// Exact authenticated original owners supplied to the serialized completion operation.
    supplied: &'a [&'a Owner<'i, P, N>],
    /// Cardinality computed during the already charged first-occurrence traversal.
    distinct: usize,
}
impl<P: Scalar, const N: usize> OwnerSet<'_, '_, P, N> {
    /// Number of distinct original allocations in the supplied set.
    #[must_use]
    pub const fn distinct(&self) -> usize {
        self.distinct
    }
    /// Immutable owner names in supplied order, including repeated aliases.
    /// These locators grant no execution or reconstruction authority.
    pub fn identities(&self) -> impl ExactSizeIterator<Item = u64> + '_ {
        self.supplied
            .iter()
            .map(|owner| owner.backing.core.reservation.backing.identity)
    }
}
/// Numerical completion never claims to detect an omitted graph owner.
#[derive(Debug)]
pub enum Completion<'a, 'i, P: Scalar, const N: usize> {
    /// At least one named owner retains an application, control or delivery obligation.
    GraphPending(OwnerSet<'a, 'i, P, N>),
    /// Native scans, borrowers, outside uses or remaining numerical occupation block completion.
    NativePending(OwnerSet<'a, 'i, P, N>, Residence),
    /// The named owners settled and the original heap proved no numerical occupation.
    Complete {
        /// Exact authenticated supplied owner set, with its distinct cardinality.
        owners: OwnerSet<'a, 'i, P, N>,
        /// Empty tables and immutable history remain measured independently of payload.
        retained_native_bytes: usize,
    },
}
impl<'a, 'i, P: Scalar, const N: usize> Completion<'a, 'i, P, N> {
    /// Every completion status identifies the same explicit supplied owner set.
    #[must_use]
    pub const fn owners(&self) -> &OwnerSet<'a, 'i, P, N> {
        match self {
            Self::GraphPending(owners)
            | Self::NativePending(owners, _)
            | Self::Complete { owners, .. } => owners,
        }
    }
}

impl<'i, P: Scalar + 'static, const N: usize> Owner<'i, P, N> {
    /// The exclusive heap borrow serializes this charged snapshot against native returns and renewal.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn completion<'a>(
        owners: &'a [&'a Self],
        heap: &mut Heap,
        account: &mut ExecutionAccount,
        scan_slots: usize,
    ) -> Result<Completion<'a, 'i, P, N>, Error> {
        let result = Self::completion_inner(owners, heap, account, scan_slots);
        for owner in owners {
            owner.backing.gate.observe(&result);
        }
        result
    }

    /// Native soft pending status retains whether its actual original Work debit failed.
    fn completion_inner<'a>(
        owners: &'a [&'a Self],
        heap: &mut Heap,
        account: &mut ExecutionAccount,
        scan_slots: usize,
    ) -> Result<Completion<'a, 'i, P, N>, Error> {
        let mut settled = true;
        let mut distinct = 0_usize;
        for (index, owner) in owners.iter().enumerate() {
            account.authenticate(owner.backing.account)?;
            if heap.scope != owner.backing.heap {
                return Err(LoanError::ForeignOwner.into());
            }
            let mut repeated = false;
            for previous in &owners[..index] {
                crate::loan::charge(&mut account.ledger, 1)?; // A named owner allocation.
                if std::ptr::eq(*previous, *owner) {
                    repeated = true;
                    break;
                }
            }
            if !repeated {
                settled &= owner.settled(account)?;
                distinct = distinct.checked_add(1).ok_or(LoanError::CountOverflow)?;
            }
        }
        let named = OwnerSet {
            supplied: owners,
            distinct,
        };
        if !settled {
            return Ok(Completion::GraphPending(named));
        }
        let (residence, stopped) = heap.residence_status(account, scan_slots)?;
        if stopped {
            for owner in owners {
                owner
                    .backing
                    .gate
                    .observe(&Err::<(), _>(Error::WorkStopped));
            }
        }
        match residence {
            Residence::Empty { retained_bytes } => Ok(Completion::Complete {
                owners: named,
                retained_native_bytes: retained_bytes,
            }),
            pending @ Residence::Pending { .. } => Ok(Completion::NativePending(named, pending)),
        }
    }

    /// Capacity vacancies are not occupations; retained scope descriptors keep their saved progress.
    fn settled(&self, account: &mut ExecutionAccount) -> Result<bool, Error> {
        let core = &self.backing.core.reservation.backing;
        crate::loan::charge(&mut account.ledger, 1)?; // H serialized graph control header.
        let mut complete = core.control.flags == 0 && !self.backing.gate.stopped();
        for (slot, node) in self.backing.nodes.iter().enumerate() {
            crate::loan::charge(&mut account.ledger, 2)?; // N native record and original core occupation.
            complete &= core.nodes[slot].flags == 0;
            complete &= node.state != State::Borrowed && node.pin.is_none();
            complete &= matches!(node.native, None | Some(Native::Cancelled));
            complete &= !matches!(
                node.contract,
                Contract::Mapped { parts: Some(_), .. } | Contract::Input(Some(_))
            );
        }
        for scope in &core.scopes {
            crate::loan::charge(&mut account.ledger, 2)?; // M retained generation and saved traversal/notification cursors.
            if scope.generation != 0 {
                complete &= scope.cursors == [usize::MAX, core.proposed];
            }
        }
        complete &= self.backing.sink.status(account).map_err(Error::Delivery)? == Status::Complete;
        Ok(complete)
    }
}
