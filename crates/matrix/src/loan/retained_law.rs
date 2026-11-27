// SPDX-License-Identifier: AGPL-3.0-only
//! A closed retained-law allocation moves with its containing application bundle.

use super::{LoanError, retained_dyadic_power, retained_expm1};
use crate::Heap;

/// The admitted laws whose already funded uses can be retained by an application.
#[repr(C)]
#[derive(Debug)]
enum RetainedLaw {
    /// The original expm1 use and its exact binding.
    Expm1(retained_expm1::Attachment),
    /// The original dyadic power use and its exact binding.
    DyadicPower(retained_dyadic_power::Attachment),
}

/// A private shell exists before the fallible preparation finishes its ownership transfer.
#[repr(C)]
#[derive(Debug)]
pub(in crate::loan) struct RetainedLawAttachment {
    /// Empty only before the original funded use is transferred.
    law: Option<RetainedLaw>,
}

impl RetainedLawAttachment {
    /// At shared charge sites, only private substitution leaves a shell unfilled; validation refuses it before inspection, and its three Work keeps the refusal prefix independent of fill state.
    const EMPTY_WORK: usize = 3;

    /// Construct the separately reserved empty shell.
    pub(in crate::loan) const fn empty() -> Self {
        Self { law: None }
    }

    /// Refuse a private shell that was filled before the no-fail transfer.
    pub(in crate::loan) const fn require_empty(&self) -> Result<(), LoanError> {
        if self.law.is_some() {
            return Err(LoanError::WrongLoan);
        }
        Ok(())
    }

    /// Charge each filled law's validation tariff or an unfilled shell's three Work; preparation fills the shell before transfer.
    pub(in crate::loan) const fn validation_work(&self) -> usize {
        match &self.law {
            Some(RetainedLaw::DyadicPower(_)) => retained_dyadic_power::Attachment::WORK,
            Some(RetainedLaw::Expm1(_)) => retained_expm1::Attachment::WORK,
            None => Self::EMPTY_WORK,
        }
    }

    /// Move the checked expm1 use into the preallocated shell.
    pub(in crate::loan) fn install_expm1(&mut self, law: retained_expm1::RetainedExpm1) {
        self.law = Some(RetainedLaw::Expm1(retained_expm1::Attachment::prepared(
            law,
        )));
    }

    /// Move the checked dyadic use into the preallocated shell.
    pub(in crate::loan) fn install_dyadic_power(
        &mut self,
        law: retained_dyadic_power::RetainedDyadicPower,
    ) {
        self.law = Some(RetainedLaw::DyadicPower(
            retained_dyadic_power::Attachment::prepared(law),
        ));
    }

    /// Wrap an already checked concrete attachment for private refusal fixtures.
    #[cfg(test)]
    pub(in crate::loan) const fn wrap_expm1(law: retained_expm1::Attachment) -> Self {
        Self {
            law: Some(RetainedLaw::Expm1(law)),
        }
    }

    /// Borrow the concrete expm1 law after rejecting an unfilled shell.
    pub(in crate::loan) const fn expm1(&self) -> Result<&retained_expm1::Attachment, LoanError> {
        match &self.law {
            Some(RetainedLaw::Expm1(law)) => Ok(law),
            Some(RetainedLaw::DyadicPower(_)) | None => Err(LoanError::WrongLoan),
        }
    }

    /// Borrow the concrete dyadic law after rejecting an empty or foreign shell.
    pub(in crate::loan) const fn dyadic_power(
        &self,
    ) -> Result<&retained_dyadic_power::Attachment, LoanError> {
        match &self.law {
            Some(RetainedLaw::DyadicPower(law)) => Ok(law),
            Some(RetainedLaw::Expm1(_)) | None => Err(LoanError::WrongLoan),
        }
    }

    /// Check the retained use without granting a new one.
    pub(in crate::loan) fn validate(&self) -> Result<(), LoanError> {
        match &self.law {
            Some(RetainedLaw::Expm1(law)) => law.validate(),
            Some(RetainedLaw::DyadicPower(law)) => law.validate(),
            None => Err(LoanError::WrongLoan),
        }
    }

    /// Check the live owner and debit witnesses of the retained use.
    pub(in crate::loan) fn validate_current(&self, heap: &Heap) -> Result<(), LoanError> {
        match &self.law {
            Some(RetainedLaw::Expm1(law)) => law.validate_current(heap),
            Some(RetainedLaw::DyadicPower(law)) => law.validate_current(heap),
            None => Err(LoanError::WrongLoan),
        }
    }

    /// Bind the retained use to the original expm1 operation.
    pub(in crate::loan) fn validate_binding(
        &self,
        binding: retained_expm1::Binding,
        bits: u32,
    ) -> Result<(), LoanError> {
        self.expm1()?.validate_binding(binding, bits)
    }

    /// Bind the retained use to the exact dyadic owner and use seal.
    pub(in crate::loan) fn validate_dyadic_binding(
        &self,
        binding: retained_dyadic_power::Binding,
    ) -> Result<(), LoanError> {
        self.dyadic_power()?.validate_binding(binding)
    }

    /// Expose only the diagnostic owner identity of a filled attachment.
    pub(in crate::loan) fn owner_identity(&self) -> Result<u64, LoanError> {
        match &self.law {
            Some(RetainedLaw::Expm1(law)) => Ok(law.owner_identity()),
            Some(RetainedLaw::DyadicPower(law)) => Ok(law.owner_identity()),
            None => Err(LoanError::WrongLoan),
        }
    }

    /// Charge the private test fixture's added box to its existing bundle debit and cleanup.
    #[cfg(test)]
    pub(in crate::loan) fn account_fixture(
        heap: &mut Heap,
        core: &mut super::lifecycle::HostCore,
    ) -> Result<(), LoanError> {
        let debit = heap
            .loans
            .debits
            .get_mut(core.headers[3].slot)
            .and_then(Option::as_mut)
            .ok_or(LoanError::WrongLoan)?;
        debit.bytes = crate::heap::resources::add(debit.bytes, size_of::<Self>())?;
        core.cleanup.allocations = core
            .cleanup
            .allocations
            .checked_add(1)
            .ok_or(LoanError::CountOverflow)?;
        Ok(())
    }
}
