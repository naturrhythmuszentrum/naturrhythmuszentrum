// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed occupation observations distinguish payload capacity from tracking, sharing and table bytes.

use super::{ExecutionAccount, LoanError, LoanTicket, charge, registry::AllocationKind};
use crate::Heap;

/// A fixed snapshot owns no numerical edge and does not authorize selected access.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LoanInventory {
    /// Shared prepared-law owner identity of a published bundle, without another use edge.
    pub retained_law: Option<u64>,
    /// Addressable destination elements, including unwritten capacity and every present channel.
    pub elements: usize,
    /// Rounded numerical slab bytes, including backend row padding.
    pub numerical_bytes: usize,
    /// Component and complete-cell tracking capacity, zero after successful publication.
    pub initialization_bytes: usize,
    /// Shared filled-mask header and words, counted once for all qualified channel attachments.
    pub filled_bytes: usize,
    /// Detached core, layout, liveness and bundle headers still conservatively resident.
    pub owner_bytes: usize,
    /// All independent pin headers, including expired weak-index records awaiting reconciliation.
    pub pin_bytes: usize,
}
impl Heap {
    /// Inspect one exact occupation without acquiring a numerical pin or changing its lifecycle.
    /// # Errors
    /// Refuses foreign/stale identity, a different original account, byte overflow or exhausted Work.
    pub fn loan_inventory(
        &self,
        ticket: LoanTicket,
        account: &mut ExecutionAccount,
    ) -> Result<LoanInventory, LoanError> {
        if ticket.identity.scope != self.scope {
            return Err(LoanError::ForeignOwner);
        }
        account.authenticate(ticket.identity.account)?;
        account
            .ledger
            .with_work_phase(super::FAMILY, "occupation inventory", |budget| {
                charge(budget, 1)?;
                let group = self.loans.group(ticket.identity)?;
                let mut result = LoanInventory::default();
                if let super::registry::State::Published(owner) = &group.state
                    && let Some(law) = owner
                        .payload
                        .as_ref()
                        .and_then(|bundle| bundle.retained_law.as_ref())
                {
                    charge(budget, 1)?;
                    law.validate()?;
                    result.retained_law = Some(law.owner_identity());
                }
                for entry in &self.loans.debits {
                    charge(budget, 1)?;
                    if let Some(debit) = entry
                        && debit.owner == super::registry::DebitOwner::Bundle(ticket.identity)
                    {
                        result.elements = result
                            .elements
                            .checked_add(debit.elements)
                            .ok_or(LoanError::CountOverflow)?;
                        let total = match &debit.kind {
                            AllocationKind::Numerical => &mut result.numerical_bytes,
                            AllocationKind::Tracking => &mut result.initialization_bytes,
                            AllocationKind::Owner => &mut result.owner_bytes,
                            AllocationKind::Pin => &mut result.pin_bytes,
                            AllocationKind::Filled => &mut result.filled_bytes,
                            AllocationKind::PreparedOwner { .. }
                            | AllocationKind::PreparedUse(_) => return Err(LoanError::WrongLoan),
                        };
                        *total = total
                            .checked_add(debit.bytes)
                            .ok_or(LoanError::ByteOverflow)?;
                    }
                }
                Ok(result)
            })
    }
}
