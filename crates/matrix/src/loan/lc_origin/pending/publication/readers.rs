// SPDX-License-Identifier: AGPL-3.0-only
//! Current and historical borrows preserve identical sealed represented-input assurance.
use super::super::Storage;
use super::{LcCellOutcome, LcOutcomeKind, LcOutcomeView, LcPublishedResult, validate};
use crate::loan::{charge, payload::bit};
use crate::{
    Coverage, ExecutionAccount, Heap, HostChannel, LoanError, MaskVersion, NumericalErrorScope,
};
use nrz_scalar::Scalar;
use std::{fmt, marker::PhantomData};

/// Numerical completion supplies an outer cover, not feasibility or physical determinacy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LcAssurance {
    /// Outer cover over represented input coordinates.
    pub coverage: Coverage,
    /// Only the final reciprocal operation's local numerical error.
    pub error_scope: NumericalErrorScope,
    /// Feasibility and determinacy both remain unasserted.
    pub certainty: super::super::super::LcCertainty,
}
/// An admitted borrow never converts to an ordinary reader or writable tile.
pub struct LcResultReader<'a, P: Scalar> {
    /// Immutable private storage retained by the borrowed specialized use.
    storage: &'a Storage,
    /// Original scalar representation, checked at admission.
    marker: PhantomData<P>,
}
impl<P: Scalar> fmt::Debug for LcResultReader<'_, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LcResultReader").finish_non_exhaustive()
    }
}
impl<P: Scalar + 'static> LcPublishedResult<P> {
    /// Borrow after checking the live output occupation and immutable seal.
    /// # Errors
    /// Refuses foreign, stale, malformed or insufficiently funded admission.
    pub fn current_reader(
        &self,
        heap: &Heap,
        account: &mut ExecutionAccount,
    ) -> Result<LcResultReader<'_, P>, LoanError> {
        self.reader(Some(heap), account)
    }
    /// Borrow retained history without requiring live sources or output occupations.
    /// # Errors
    /// Refuses foreign accounts, malformed seal or insufficient Work.
    pub fn historical_reader(
        &self,
        account: &mut ExecutionAccount,
    ) -> Result<LcResultReader<'_, P>, LoanError> {
        self.reader(None, account)
    }
    /// Admit a borrow without owning-edge acquisition or partition traversal.
    fn reader(
        &self,
        heap: Option<&Heap>,
        account: &mut ExecutionAccount,
    ) -> Result<LcResultReader<'_, P>, LoanError> {
        let payload = self.record.payload.as_ref().ok_or(LoanError::WrongLoan)?;
        account.authenticate(payload.account)?;
        if heap.is_some_and(|heap| heap.scope != self.ticket().identity.scope) {
            return Err(LoanError::ForeignOwner);
        }
        account
            .ledger
            .with_work_phase(crate::loan::FAMILY, "LC reader admission", |budget| {
                charge(budget, 1)?;
                let s = payload
                    .bundle
                    .storage
                    .as_ref()
                    .ok_or(LoanError::WrongLoan)?;
                let work = s
                    .closure
                    .rows
                    .len()
                    .checked_mul(28)
                    .and_then(|n| n.checked_add(14 + 6 * usize::from(heap.is_some())))
                    .ok_or(LoanError::CountOverflow)?;
                charge(budget, work)?;
                if payload.identity != payload.token.key
                    || payload.account != s.identity.account
                    || s.identity.format != crate::storage::Format::of::<P>()
                {
                    return Err(LoanError::WrongLoan);
                }
                validate::sealed(&payload.bundle)?;
                capture::<P>(s)?;
                if let Some(heap) = heap {
                    validate::current(heap, &payload.bundle)?;
                }
                Ok(LcResultReader {
                    storage: s,
                    marker: PhantomData,
                })
            })
    }
}
/// Captured schemas and tuple roles remain in their original positional order after inventory release.
pub(super) fn capture<P: Scalar + 'static>(s: &Storage) -> Result<(), LoanError> {
    for (index, row) in s.closure.rows.iter().enumerate() {
        let reader = crate::loan::reader::checked_record_reader::<P>(&row.pin)?;
        let mapped = reader.bundle;
        let source = mapped.origin.lc_source()?;
        if row.position != index
            || row.axes != s.closure.rows[0].axes
            || row.axes.combined.axis() != s.layout.axis
            || row.schema == 0
            || mapped.identity != row.mapped
            || source.identity != row.source
            || source.projection != row.projection
            || source.identity.layout != row.layout
            || source.declaration.axis != row.axes.instances
            || reader.account != s.identity.account
            || mapped.qualified != row.companions.map(Some)
        {
            return Err(LoanError::TargetMismatch);
        }
        for (a, b) in [
            (&row.symbols[0], &source.declaration.symbols),
            (&row.symbols[1], &mapped.declaration.symbols),
        ] {
            if a.value() != b.value() || a.bound() != b.bound() || a.error() != b.error() {
                return Err(LoanError::TargetMismatch);
            }
        }
    }
    Ok(())
}
impl<'a, P: Scalar + 'static> LcResultReader<'a, P> {
    /// Immutable demand identity, without any writable mask capability.
    #[must_use]
    pub const fn demand(&self) -> MaskVersion {
        self.storage.demand
    }
    /// Assurance is fixed by the immutable publication seal.
    #[must_use]
    pub const fn assurance(&self) -> LcAssurance {
        LcAssurance {
            coverage: Coverage::RepresentedInputs,
            error_scope: NumericalErrorScope::LocalOperation,
            certainty: super::super::super::LcCertainty::Undecided,
        }
    }
    /// Select one terminal predicate without allocating or scanning the partition.
    #[must_use]
    pub const fn view(&self, kind: LcOutcomeKind) -> LcOutcomeView<'_, 'a, P> {
        LcOutcomeView { reader: self, kind }
    }
    /// Inspect one sealed record; this is `H_view=1`, never a C-sized rescan.
    /// # Errors
    /// Refuses foreign accounts, bounds or exhausted inspection Work.
    pub fn outcome(
        &self,
        cell: usize,
        account: &mut ExecutionAccount,
    ) -> Result<LcCellOutcome, LoanError> {
        account.authenticate(self.storage.identity.account)?;
        account
            .ledger
            .with_work_phase(crate::loan::FAMILY, "LC outcome inspection", |budget| {
                charge(budget, 1)?;
                let record = *self
                    .storage
                    .outcomes
                    .get(cell)
                    .ok_or(LoanError::LayoutMismatch)?;
                super::outcomes::decode(record, cell)
            })
    }
    /// Load exactly one selected companion after all bounds, mask and channel checks.
    /// # Errors
    /// Refuses holes, foreign accounts, bounds and Work before loading a numerical component.
    pub fn read(
        &self,
        channel: HostChannel,
        cell: usize,
        account: &mut ExecutionAccount,
    ) -> Result<P, LoanError> {
        account.authenticate(self.storage.identity.account)?;
        account
            .ledger
            .with_work_phase(crate::loan::FAMILY, "LC selected component", |budget| {
                charge(budget, 6)?;
                let record = self
                    .storage
                    .outcomes
                    .get(cell)
                    .ok_or(LoanError::LayoutMismatch)?;
                if record.status != 2 {
                    return Err(LoanError::CoverageMismatch);
                }
                if !bit(&self.storage.completed.words, cell) {
                    return Err(LoanError::CoverageMismatch);
                }
                let component = &self.storage.channels[channel.index()];
                let address = self.storage.layout.components.index(cell, 0)?;
                component.numerical.read::<P>(address)
            })
    }
}
