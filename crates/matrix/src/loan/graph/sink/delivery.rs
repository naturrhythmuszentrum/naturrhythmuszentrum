// SPDX-License-Identifier: AGPL-3.0-only
//! Prepaid record installation cannot fail after a native probe has entered.

use super::codec::Evidence;
use super::{Encoding, Error, ExecutionAccount, Handle, Sink, Status, charge};
use crate::LoanError;
use std::cell::{Ref, RefMut};

/// Borrowed immutable words carry no acknowledgement or completion authority.
#[derive(Debug)]
pub(crate) struct Pull<'a> {
    /// Borrow protects the actual evidence backing against replacement.
    words: Ref<'a, Vec<u64>>,
    /// Exact original record authority returned to the caller.
    pub(crate) handle: Handle,
    /// Admitted start and actual byte length.
    range: (usize, usize),
}
impl Pull<'_> {
    /// Padded words are immutable; the byte length identifies the lossless prefix.
    pub(crate) fn words(&self) -> &[u64] {
        &self.words[self.range.0..self.range.0 + self.range.1.div_ceil(8)]
    }
    /// Only the final word may contain zero padding.
    pub(crate) const fn bytes(&self) -> usize {
        self.range.1
    }
}
/// A fixed prepared transaction exclusively reserves its destination before native work.
#[derive(Debug)]
pub(crate) struct Prepared<'a> {
    /// Sole graph-side owner stays borrowed until commit or interruption.
    sink: &'a Sink,
    /// Mutable evidence borrow prevents another delivery from stealing this destination.
    words: RefMut<'a, Vec<u64>>,
    /// Original destination, with its precomputed next generation.
    handle: Handle,
    /// Exact closed encoding selected before entering the native transition.
    encoding: Encoding,
    /// Preflighted destination offset, inline in this prepared record.
    offset: usize,
}
impl Sink {
    /// Preflight and prepay before invoking any native operation producing this evidence.
    pub(crate) fn prepare(
        &self,
        slot: usize,
        encoding: Encoding,
        account: &mut ExecutionAccount,
    ) -> Result<Prepared<'_>, Error> {
        account.authenticate(self.backing.account)?;
        charge(&mut account.ledger, 1)?; // H entry.
        charge(&mut account.ledger, 1)?; // M fixed stop status, including a prepared update.
        if self.backing.status.get() == Status::Overlength {
            return Err(Error::Overlength);
        }
        let record = self
            .backing
            .slots
            .get(slot)
            .ok_or(LoanError::StaleOccupation)?;
        charge(&mut account.ledger, 6)?; // M state, witnesses, codec, bound, generation, offset.
        let (state, witnesses, admitted, bound, current, offset) = (
            record.state.get(),
            record.witnesses.get(),
            record.encoding,
            record.bound,
            record.generation.get(),
            record.offset,
        );
        if state == 2 || state == 4 || witnesses != 0 {
            self.backing.status.set(Status::Incomplete);
            return Err(Error::Full);
        }
        let accepts = admitted == encoding
            || (admitted == Encoding::Outcome && encoding == Encoding::Refusal);
        if !accepts || encoding == Encoding::Vacant {
            return Err(Error::Unbounded);
        }
        if encoding.bytes() > bound {
            self.backing.status.set(Status::Overlength); // charged fixed status update.
            return Err(Error::Overlength);
        }
        let generation = if state == 3 {
            let Some(next) = current.checked_add(1) else {
                record.state.set(4); // prepaid permanent slot retirement.
                self.backing.status.set(Status::Incomplete);
                return Err(Error::Full);
            };
            next
        } else {
            current
        };
        let words = self
            .backing
            .words
            .try_borrow_mut()
            .map_err(|_| Error::Borrowed)?;
        let count = encoding.bytes().div_ceil(8);
        // T fixed transaction; M source fields; I directly initialized destination words;
        // M state/generation/length/outstanding/status commit, prepaid before native entry.
        charge(
            &mut account.ledger,
            1 + encoding.inspection() + encoding.fields() + count + 5,
        )?;
        Ok(Prepared {
            sink: self,
            words,
            handle: Handle {
                owner: self.backing.identity,
                slot,
                generation,
            },
            encoding,
            offset,
        })
    }
    /// Pull authenticates an outstanding record and borrows its immutable encoded fields.
    pub(crate) fn pull(
        &self,
        handle: Handle,
        account: &mut ExecutionAccount,
    ) -> Result<Pull<'_>, Error> {
        self.authenticate(handle, account)?;
        let record = &self.backing.slots[handle.slot];
        charge(&mut account.ledger, 2)?; // M destination offset and encoded byte length.
        let range = (record.offset, record.length.get());
        let words = self
            .backing
            .words
            .try_borrow()
            .map_err(|_| Error::Borrowed)?;
        Ok(Pull {
            words,
            handle,
            range,
        })
    }
    /// No foreign/stale/double acknowledgement can consume the rightful obligation.
    pub(crate) fn acknowledge(
        &self,
        handle: Handle,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        self.authenticate(handle, account)?;
        // A live pull must end before acknowledgement, including a witness-side pull.
        let borrow = self
            .backing
            .words
            .try_borrow_mut()
            .map_err(|_| Error::Borrowed)?;
        // T release transaction + M state/outstanding/status commit.
        charge(&mut account.ledger, 4)?;
        let record = &self.backing.slots[handle.slot];
        record.state.set(3);
        let remaining = self.backing.outstanding.get() - 1;
        self.backing.outstanding.set(remaining);
        let status = self.backing.status.get();
        if remaining == 0 && status == Status::Incomplete {
            self.backing.status.set(Status::Complete);
        }
        drop(borrow);
        Ok(())
    }
    /// Foreign-account rejection is uncharged; owner authentication precedes indexing.
    pub(super) fn authenticate(
        &self,
        handle: Handle,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        account.authenticate(self.backing.account)?;
        charge(&mut account.ledger, 2)?; // H entry + A standalone owner check.
        if handle.owner != self.backing.identity {
            return Err(LoanError::ForeignOwner.into());
        }
        charge(&mut account.ledger, 2)?; // M generation and outstanding state.
        let record = self
            .backing
            .slots
            .get(handle.slot)
            .ok_or(LoanError::StaleOccupation)?;
        if record.generation.get() != handle.generation {
            return Err(LoanError::StaleOccupation.into());
        }
        if record.state.get() != 2 {
            return Err(LoanError::DuplicateReturn.into());
        }
        Ok(())
    }
}
impl Prepared<'_> {
    /// Commit moves no native owner; wrong/overlong evidence returns unchanged to its caller.
    pub(crate) fn commit(mut self, evidence: Evidence) -> Result<Handle, (Evidence, Error)> {
        let record = &self.sink.backing.slots[self.handle.slot];
        let count = self.encoding.bytes().div_ceil(8);
        if let Err(evidence) = evidence.write(
            self.encoding,
            record.position,
            &mut self.words[self.offset..self.offset + count],
        ) {
            return Err((evidence, Error::Unbounded));
        }
        let outstanding = self.sink.backing.outstanding.get();
        if record.state.get() != 1 {
            self.sink.backing.outstanding.set(outstanding + 1);
        }
        record.generation.set(self.handle.generation);
        record.length.set(self.encoding.bytes());
        record.state.set(2);
        if self.sink.backing.status.get() != Status::Overlength {
            self.sink.backing.status.set(Status::Incomplete);
        }
        Ok(self.handle)
    }
}

impl Prepared<'_> {
    /// Return the full native cause and capability while installing only its prepaid fixed tag.
    pub(crate) fn return_refusal<C>(
        self,
        capability: C,
        error: crate::LcShapeError,
    ) -> Returned<C> {
        use super::codec::Refusal;
        use crate::LcShapeError;
        use nrz_bindings::{AdmissionError, UnitError};
        let tag = match &error {
            LcShapeError::Declaration(AdmissionError::Units(
                UnitError::UnsupportedMeasureScale(_),
            )) => Refusal::UnsupportedMeasureScale,
            LcShapeError::Declaration(AdmissionError::Units(UnitError::ConflictingKind(_))) => {
                Refusal::ConflictingKind
            }
            LcShapeError::Declaration(
                AdmissionError::Algebra(fault)
                | AdmissionError::Units(UnitError::Arithmetic(fault)),
            ) if fault.series_evidence.is_some() => Refusal::SeriesEvidence,
            LcShapeError::Loan(_) => Refusal::Loan,
            LcShapeError::Declaration(_) => Refusal::Declaration,
        };
        let receipt = self
            .commit(Evidence::Refusal(tag))
            .map_err(|(_, stopped)| stopped);
        Returned {
            capability,
            error,
            receipt,
        }
    }
}

impl Sink {
    /// The fixed destination is funded before a probe can consume its native attempt.
    pub(crate) fn probe<C>(
        &self,
        slot: usize,
        mut capability: C,
        account: &mut ExecutionAccount,
        probe: impl FnOnce(&mut C, &mut ExecutionAccount) -> super::codec::Capacity,
    ) -> Result<(C, Handle), (C, Error)> {
        let prepared = match self.prepare(slot, Encoding::Probe, account) {
            Ok(prepared) => prepared,
            Err(error) => return Err((capability, error)),
        };
        let capacity = probe(&mut capability, account);
        match prepared.commit(Evidence::Probe(capacity)) {
            Ok(handle) => Ok((capability, handle)),
            Err((_, error)) => Err((capability, error)),
        }
    }
}

/// A native consuming return always preserves both original values, including on a sink stop.
#[derive(Debug)]
pub(crate) struct Returned<C> {
    /// Identical native continuation or declaration.
    pub(crate) capability: C,
    /// Full structured cause, with its original String or Arc allocation.
    pub(crate) error: crate::LcShapeError,
    /// Fixed-record installation result; this never owns the native payload.
    pub(crate) receipt: Result<Handle, Error>,
}

/// A stopped delivery returns the identical native value without allocating an error box.
#[derive(Debug)]
pub(crate) struct Delivered<C> {
    /// The sink never retains this native capability or copies its payload.
    pub(crate) capability: C,
    /// Only the bounded record's installation result belongs to the sink.
    pub(crate) receipt: Result<Handle, Error>,
}
impl Sink {
    /// An overlength or Work stop returns its native capability with no partial evidence.
    pub(crate) fn deliver<C>(
        &self,
        slot: usize,
        capability: C,
        evidence: Evidence,
        account: &mut ExecutionAccount,
    ) -> Delivered<C> {
        let receipt = self
            .prepare(slot, evidence.encoding(), account)
            .and_then(|prepared| prepared.commit(evidence).map_err(|(_, error)| error));
        Delivered {
            capability,
            receipt,
        }
    }
}
