// SPDX-License-Identifier: AGPL-3.0-only
//! Prepaid record installation cannot fail after a native probe has entered.

use super::codec::Evidence;
use super::{Encoding, Error, ExecutionAccount, Handle, Sink, Status, charge};
use crate::LoanError;
use std::cell::{Ref, RefMut};

/// Borrowed immutable words carry no acknowledgement or completion authority.
#[derive(Debug)]
pub struct Pull<'a> {
    /// Borrow protects the actual evidence backing against replacement.
    words: Ref<'a, Vec<u64>>,
    /// Exact original record authority returned to the caller.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the borrowed receipt retains its authenticated original locator"
        )
    )]
    pub(crate) handle: Handle,
    /// Admitted start and actual byte length.
    range: (usize, usize),
}
impl Pull<'_> {
    /// Padded words are immutable; the byte length identifies the lossless prefix.
    #[must_use]
    pub fn words(&self) -> &[u64] {
        &self.words[self.range.0..self.range.0 + self.range.1.div_ceil(8)]
    }
    /// Only the final word may contain zero padding.
    #[must_use]
    pub const fn bytes(&self) -> usize {
        self.range.1
    }
}
/// A fixed prepared transaction exclusively reserves its destination before native work.
#[derive(Debug)]
pub(crate) struct Prepared<'a> {
    /// Sole graph-side owner stays borrowed until commit or interruption.
    sink: &'a Sink,
    /// Mutable evidence borrow prevents another delivery from stealing this destination.
    words: RefMut<'a, [u64]>,
    /// Original destination, with its precomputed next generation.
    handle: Handle,
    /// Exact closed encoding selected before entering the native transition.
    encoding: Encoding,
    /// Fixed-width native phase selected by the admitted adapter transition.
    phase: u64,
}
/// Checked metadata carries no borrow or obligation until its fixed transaction is acquired.
#[derive(Clone, Copy, Debug)]
struct Destination {
    /// Exact original slot and next nonwrapping generation.
    handle: Handle,
    /// Admitted closed scalar encoding.
    encoding: Encoding,
    /// Disjoint preallocated evidence range.
    offset: usize,
}
impl Destination {
    /// Existing T/M/I tariff, unchanged for both single and paired destinations.
    fn prepay(self, budget: &mut nrz_bindings::Budget) -> Result<(), Error> {
        let count = self.encoding.bytes().div_ceil(8);
        charge(
            budget,
            1 + self.encoding.inspection() + self.encoding.fields() + count + 5,
        )?;
        Ok(())
    }
}
impl Sink {
    /// Preflight and prepay before invoking any native operation producing this evidence.
    pub(crate) fn prepare(
        &self,
        slot: usize,
        encoding: Encoding,
        account: &mut ExecutionAccount,
    ) -> Result<Prepared<'_>, Error> {
        self.prepare_on(slot, encoding, account.identity, &mut account.ledger)
    }
    /// Nested graph phases borrow the identical original ledger without reconstructing an account.
    pub(crate) fn prepare_on(
        &self,
        slot: usize,
        encoding: Encoding,
        identity: u64,
        budget: &mut nrz_bindings::Budget,
    ) -> Result<Prepared<'_>, Error> {
        let destination = self.destination(slot, encoding, identity, budget)?;
        let words = self
            .backing
            .words
            .try_borrow_mut()
            .map_err(|_| Error::Borrowed)?;
        destination.prepay(budget)?;
        let begin = destination.offset;
        let end = begin + destination.encoding.bytes().div_ceil(8);
        Ok(self.materialize(
            destination,
            RefMut::map(words, |words| &mut words[begin..end]),
        ))
    }

    /// A probe and an intact-return destination are funded together before native entry.
    pub(in crate::loan::graph) fn prepare_pair(
        &self,
        first: (usize, Encoding),
        second: (usize, Encoding),
        account: &mut ExecutionAccount,
    ) -> Result<(Prepared<'_>, Prepared<'_>), Error> {
        let first = self.destination(first.0, first.1, account.identity, &mut account.ledger)?;
        let second = self.destination(second.0, second.1, account.identity, &mut account.ledger)?;
        let first_end = first.offset + first.encoding.bytes().div_ceil(8);
        let second_end = second.offset + second.encoding.bytes().div_ceil(8);
        let forward = first_end <= second.offset;
        let backward = second_end <= first.offset;
        if !forward && !backward {
            return Err(Error::Full);
        }
        let words = self
            .backing
            .words
            .try_borrow_mut()
            .map_err(|_| Error::Borrowed)?;
        first.prepay(&mut account.ledger)?;
        second.prepay(&mut account.ledger)?;
        let words = RefMut::map(words, Vec::as_mut_slice);
        let (first_words, second_words) = RefMut::map_split(words, |words| {
            if forward {
                let (left, right) = words.split_at_mut(second.offset);
                (
                    &mut left[first.offset..first_end],
                    &mut right[..second_end - second.offset],
                )
            } else {
                let (left, right) = words.split_at_mut(first.offset);
                (
                    &mut right[..first_end - first.offset],
                    &mut left[second.offset..second_end],
                )
            }
        });
        Ok((
            self.materialize(first, first_words),
            self.materialize(second, second_words),
        ))
    }

    /// The same fixed inspections precede either a single or paired transaction.
    fn destination(
        &self,
        slot: usize,
        encoding: Encoding,
        identity: u64,
        budget: &mut nrz_bindings::Budget,
    ) -> Result<Destination, Error> {
        if identity != self.backing.account {
            return Err(LoanError::ForeignAccount.into());
        }
        charge(budget, 1)?; // H entry.
        charge(budget, 1)?; // M fixed stop status, including a prepared update.
        if self.backing.status.get() == Status::Overlength {
            return Err(Error::Overlength);
        }
        let record = self
            .backing
            .slots
            .get(slot)
            .ok_or(LoanError::StaleOccupation)?;
        charge(budget, 6)?; // M state, witnesses, codec, bound, generation, offset.
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
        Ok(Destination {
            handle: Handle {
                owner: self.backing.identity,
                slot,
                generation,
            },
            encoding,
            offset,
        })
    }

    /// Construct a fixed transaction after its complete copy/update tariff was prepaid.
    fn materialize<'a>(
        &'a self,
        destination: Destination,
        words: RefMut<'a, [u64]>,
    ) -> Prepared<'a> {
        Prepared {
            sink: self,
            words,
            handle: destination.handle,
            encoding: destination.encoding,
            phase: self.backing.slots[destination.handle.slot].position.phase,
        }
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
    /// Selecting a native phase changes only its bounded scalar, never owner or node authority.
    pub(in crate::loan::graph) const fn at_native_state(mut self, phase: u64) -> Self {
        self.phase = phase;
        self
    }
    /// Commit moves no native owner; wrong/overlong evidence returns unchanged to its caller.
    pub(crate) fn commit(mut self, evidence: Evidence) -> Result<Handle, (Evidence, Error)> {
        let record = &self.sink.backing.slots[self.handle.slot];
        let count = self.encoding.bytes().div_ceil(8);
        if let Err(evidence) = evidence.write(
            self.encoding,
            super::codec::Position {
                owner: record.position.owner,
                node: record.position.node,
                generation: record.position.generation,
                phase: self.phase,
            },
            &mut self.words[..count],
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
    #[cfg(test)]
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
#[cfg(test)]
pub(crate) struct Delivered<C> {
    /// The sink never retains this native capability or copies its payload.
    pub(crate) capability: C,
    /// Only the bounded record's installation result belongs to the sink.
    pub(crate) receipt: Result<Handle, Error>,
}
impl Sink {
    /// An overlength or Work stop returns its native capability with no partial evidence.
    #[cfg(test)]
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
