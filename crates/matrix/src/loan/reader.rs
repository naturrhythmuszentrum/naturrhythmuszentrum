// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit non-cloneable pins retain complete immutable numerical bundles.

use super::{
    ExecutionAccount, HostChannel, LoanError, charge,
    payload::bit,
    rc_bytes,
    registry::{Bundle, Debit, LoanTicket, PinIndex, PinPayload, PinRecord, State, Token},
};
use crate::{
    Heap,
    heap::resources::{ResourcePlan, ResourcePolicy},
};
use nrz_scalar::Scalar;
use std::{marker::PhantomData, rc::Rc};

/// One charged use of a complete immutable bundle; ordinary drop spends its prepaid release.
pub struct BundlePin<P: Scalar> {
    /// Public code cannot clone or upgrade this private record.
    pub(super) record: Rc<PinRecord>,
    /// Sealed format authenticated at acquisition.
    pub(super) marker: PhantomData<P>,
}
/// A successful publication owns the initially reserved pin, rather than merely a locator.
pub struct PublishedTile<P: Scalar> {
    /// Initial complete-bundle use edge.
    pub(super) pin: BundlePin<P>,
}
/// Borrowed selected access never exposes a dense rectangle or a mutable numerical view.
pub struct TileReader<'a, P: Scalar> {
    /// Complete immutable bundle borrowed from an authenticated pin.
    pub(super) bundle: &'a Bundle,
    /// Pin's original continuation account.
    pub(super) account: u64,
    /// Sealed scalar type.
    marker: PhantomData<P>,
}
impl<P: Scalar> std::fmt::Debug for BundlePin<P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BundlePin")
            .field(
                "identity",
                &self.record.payload.as_ref().map(|pin| pin.identity),
            )
            .finish_non_exhaustive()
    }
}
impl<P: Scalar> std::fmt::Debug for PublishedTile<P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PublishedTile")
            .field("pin", &self.pin)
            .finish()
    }
}
impl<P: Scalar> std::fmt::Debug for TileReader<'_, P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TileReader")
            .field("identity", &self.bundle.identity)
            .field("extent", &self.bundle.declaration.extent)
            .finish_non_exhaustive()
    }
}
impl<P: Scalar + 'static> BundlePin<P> {
    /// Exact historical selected access, independent of current semantic availability.
    /// # Errors
    /// Refuses incomplete private shells or a mismatched scalar format.
    pub fn historical_reader(
        &self,
        account: &mut ExecutionAccount,
    ) -> Result<TileReader<'_, P>, LoanError> {
        let pin = self.record.payload.as_ref().ok_or(LoanError::WrongLoan)?;
        account.authenticate(pin.account)?;
        account
            .ledger
            .with_work_phase(super::FAMILY, "historical reader", |budget| {
                let has_law = pin
                    .bundle
                    .payload
                    .as_ref()
                    .is_some_and(|bundle| bundle.retained_law.is_some());
                let prepared = pin.bundle.payload.as_ref().is_some_and(|bundle| {
                    bundle.endpoints.is_some()
                        || bundle
                            .origin
                            .evidence
                            .as_ref()
                            .is_some_and(|evidence| evidence.prepared.is_some())
                });
                let binary = pin
                    .bundle
                    .payload
                    .as_ref()
                    .and_then(|bundle| bundle.origin.evidence.as_ref())
                    .is_some_and(|evidence| evidence.endpoint_tokens.binary().is_some());
                charge(
                    budget,
                    3 + usize::from(has_law) + 3 * usize::from(prepared) + usize::from(binary),
                )?;
                let bundle = pin.bundle.payload.as_ref().ok_or(LoanError::WrongLoan)?;
                if let Some(law) = &bundle.retained_law {
                    law.validate()?;
                }
                self.checked_reader()
            })
    }
    /// Private layout checks borrow the complete owner after its charged law authentication.
    pub(super) fn checked_reader(&self) -> Result<TileReader<'_, P>, LoanError> {
        let pin = self.record.payload.as_ref().ok_or(LoanError::WrongLoan)?;
        let bundle = pin.bundle.payload.as_ref().ok_or(LoanError::WrongLoan)?;
        if bundle.origin.evidence.is_some() || bundle.endpoints.is_some() {
            bundle.origin.validate_current(bundle)?;
        }
        self.layout_reader()
    }
    /// Current occupation authentication already checked the immutable origin in its own visit.
    fn layout_reader(&self) -> Result<TileReader<'_, P>, LoanError> {
        let pin = self.record.payload.as_ref().ok_or(LoanError::WrongLoan)?;
        let bundle = pin.bundle.payload.as_ref().ok_or(LoanError::WrongLoan)?;
        let layout_matches = bundle.layout.axis == bundle.declaration.axis
            && bundle.layout.components.extent() == bundle.declaration.extent;
        if !layout_matches || bundle.mask.layout != bundle.identity.layout {
            return Err(LoanError::LayoutMismatch);
        }
        if bundle.identity.format != crate::storage::Format::of::<P>() {
            return Err(LoanError::LayoutMismatch);
        }
        Ok(TileReader {
            bundle,
            account: pin.account,
            marker: PhantomData,
        })
    }
    /// Diagnostic locator carries no additional numerical use edge.
    /// # Errors
    /// Refuses an incomplete internal publication shell.
    pub fn ticket(&self) -> Result<LoanTicket, LoanError> {
        Ok(LoanTicket {
            identity: self
                .record
                .payload
                .as_ref()
                .and_then(|pin| pin.bundle.payload.as_ref())
                .ok_or(LoanError::WrongLoan)?
                .identity,
        })
    }
}
impl<P: Scalar + 'static> PublishedTile<P> {
    /// Authenticate current exact occupations while borrowing both heap and initial pin.
    /// # Errors
    /// Refuses foreign, retired, replaced or otherwise stale current authority.
    pub fn reader<'a>(
        &'a self,
        heap: &'a Heap,
        account: &mut ExecutionAccount,
    ) -> Result<TileReader<'a, P>, LoanError> {
        let pin = self
            .pin
            .record
            .payload
            .as_ref()
            .ok_or(LoanError::WrongLoan)?;
        account.authenticate(pin.account)?;
        account
            .ledger
            .with_work_phase(super::FAMILY, "current reader", |budget| {
                heap.current_bundle(&self.pin, budget)?;
                charge(budget, 3)?;
                self.pin.layout_reader()
            })
    }
    /// Move the initial use edge into another owning facade without creating a new edge.
    #[must_use]
    pub fn into_pin(self) -> BundlePin<P> {
        self.pin
    }
    /// Borrow the initial pin for explicit charged duplication.
    #[must_use]
    pub const fn pin(&self) -> &BundlePin<P> {
        &self.pin
    }
}
impl<P: Scalar + 'static> TileReader<'_, P> {
    /// Reborrow the immutable bundle without adding a pin or extending its lifetime.
    pub(super) const fn reborrow(&self) -> TileReader<'_, P> {
        TileReader {
            bundle: self.bundle,
            account: self.account,
            marker: PhantomData,
        }
    }
    /// Declared capacity includes holes, which remain forbidden numerical reads.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.bundle.declaration.extent.rows()
    }
    /// Original explicit coverage assumption; successful storage does not upgrade it.
    #[must_use]
    pub const fn coverage(&self) -> crate::Coverage {
        self.bundle.declaration.coverage
    }
    /// Checked filled membership charges the authenticated original account.
    /// # Errors
    /// Refuses another account, out-of-capacity cells or exhausted Work.
    pub fn contains(&self, cell: usize, account: &mut ExecutionAccount) -> Result<bool, LoanError> {
        account.authenticate(self.account)?;
        if cell >= self.capacity() {
            return Err(LoanError::LayoutMismatch);
        }
        account
            .ledger
            .with_work_phase(super::FAMILY, "selected coverage", |budget| {
                charge(budget, 1)?;
                Ok(bit(&self.bundle.mask.words, cell))
            })
    }
    /// Load exactly one selected initialized component on the pin's original ledger.
    /// # Errors
    /// Refuses holes before touching numerical storage, absent channels, bounds and Work.
    pub fn read_component(
        &self,
        channel: HostChannel,
        cell: usize,
        component: usize,
        account: &mut ExecutionAccount,
    ) -> Result<P, LoanError> {
        account.authenticate(self.account)?;
        let owner = self.selected_owner(channel, cell, component)?;
        account
            .ledger
            .with_work_phase(super::FAMILY, "selected load", |budget| {
                charge(budget, 1)?;
                owner
                    .matrix
                    .read(self.bundle.layout.components.index(cell, component)?)
            })
    }
    /// Report the immutable publication boundary of this tile's numerical error channel.
    /// This metadata is available even when the historical input has retired.
    #[must_use]
    pub fn numerical_error_scope(&self) -> super::NumericalErrorScope {
        self.bundle
            .origin
            .evidence
            .as_ref()
            .map_or(super::NumericalErrorScope::HostInput, |evidence| {
                evidence.error_scope
            })
    }
    /// The prepaid application reader shares every address and attachment check.
    pub(super) fn read_prepaid(
        &self,
        channel: HostChannel,
        cell: usize,
        component: usize,
        _permit: &super::application::evaluation::Traversal,
    ) -> Result<P, LoanError> {
        self.selected_owner(channel, cell, component)?
            .matrix
            .read(self.bundle.layout.components.index(cell, component)?)
    }
    /// Refuse holes and invalid attachment authority before touching numerical storage.
    pub(super) fn selected_owner(
        &self,
        channel: HostChannel,
        cell: usize,
        component: usize,
    ) -> Result<&super::payload::PublishedChannel, LoanError> {
        if cell >= self.capacity() || component >= self.bundle.declaration.extent.columns() {
            return Err(LoanError::LayoutMismatch);
        }
        if !bit(&self.bundle.mask.words, cell) {
            return Err(LoanError::CoverageMismatch);
        }
        if self.bundle.origin.evidence.as_ref().is_some()
            && !matches!(
                self.bundle.origin.outcome(cell)?,
                super::application::CellOutcome::Completed
            )
        {
            return Err(LoanError::CoverageMismatch);
        }
        let qualified =
            self.bundle.qualified[channel.index()].ok_or(LoanError::CoverageMismatch)?;
        if qualified.identity != self.bundle.identity || qualified.channel != channel {
            return Err(LoanError::CoverageMismatch);
        }
        self.bundle.channels[channel.index()]
            .as_ref()
            .ok_or(LoanError::UnsupportedContract)
    }
    /// Inspect one immutable result outcome on this pin's original account.
    /// # Errors
    /// Refuses host origins, out-of-capacity cells, foreign accounts and exhausted Work.
    pub fn application_outcome(
        &self,
        cell: usize,
        account: &mut ExecutionAccount,
    ) -> Result<&super::application::CellOutcome, LoanError> {
        account.authenticate(self.account)?;
        account
            .ledger
            .with_work_phase(super::application::FAMILY, "result outcome", |budget| {
                charge(budget, 1)?;
                self.bundle.origin.outcome(cell)
            })
    }
}
impl Heap {
    /// Current access pairs the supplied pin with the exact live registry root.
    pub(super) fn current_bundle<P: Scalar + 'static>(
        &self,
        pin: &BundlePin<P>,
        budget: &mut nrz_bindings::Budget,
    ) -> Result<(), LoanError> {
        charge(budget, 1)?;
        let ticket = pin.ticket()?;
        if ticket.identity.scope != self.scope {
            return Err(LoanError::ForeignOwner);
        }
        charge(budget, 1)?;
        let group = self.loans.group(ticket.identity)?;
        let State::Published(owner) = &group.state else {
            return Err(LoanError::StaleDependency);
        };
        let payload = pin.record.payload.as_ref().ok_or(LoanError::WrongLoan)?;
        if !Rc::ptr_eq(owner, &payload.bundle) {
            return Err(LoanError::StaleDependency);
        }
        charge(budget, 1)?;
        let indexed = self
            .loans
            .pins
            .get(payload.index)
            .and_then(Option::as_ref)
            .is_some_and(|entry| {
                entry.identity == payload.identity
                    && entry
                        .witness
                        .as_ref()
                        .is_some_and(|witness| witness.ptr_eq(&Rc::downgrade(&pin.record)))
            });
        if !indexed || payload.identity != payload.token.key {
            return Err(LoanError::WrongLoan);
        }
        let bundle = owner.payload.as_ref().ok_or(LoanError::WrongLoan)?;
        self.current_owner(bundle, budget)
    }
    /// Bare tickets and retained pins share the same declaration, attachment and native checks.
    pub(in crate::loan) fn current_owner(
        &self,
        bundle: &Bundle,
        budget: &mut nrz_bindings::Budget,
    ) -> Result<(), LoanError> {
        let channels = bundle.channels.iter().flatten().count();
        charge(
            budget,
            channels
                + 2 * usize::from(bundle.endpoints.is_some())
                + 5
                + usize::from(bundle.origin.evidence.is_some())
                + usize::from(
                    bundle
                        .origin
                        .evidence
                        .as_ref()
                        .is_some_and(|evidence| evidence.endpoint_tokens.binary().is_some()),
                )
                + super::retained_expm1::Attachment::WORK
                    * usize::from(bundle.retained_law.is_some()),
        )?;
        self.validate_published_bundle(bundle)?;
        charge(
            budget,
            channels + 1 + 3 * usize::from(bundle.endpoints.is_some()),
        )?;
        bundle.origin.validate_current(bundle)?;
        if bundle.layout.axis != bundle.declaration.axis
            || bundle.layout.components.extent() != bundle.declaration.extent
        {
            return Err(LoanError::LayoutMismatch);
        }
        for key in bundle.identity.channels.into_iter().flatten() {
            charge(budget, 1)?;
            let crate::heap::descriptors::DescriptorRecord::Managed(record) =
                self.descriptors.slots.get(key)?
            else {
                return Err(LoanError::StaleDependency);
            };
            if record.identity != bundle.identity
                || record.filled != bundle.qualified[record.channel.index()].is_some()
            {
                return Err(LoanError::StaleDependency);
            }
        }
        charge(budget, 3)?;
        self.mask(bundle.identity.mask, bundle.identity.layout)?;
        Ok(())
    }
    /// Acquire a distinct charged use from a current bare locator.
    /// # Errors
    /// Refuses pending, foreign or stale occupations and exhausted original resources.
    pub fn acquire_pin<P: Scalar + 'static>(
        &mut self,
        ticket: LoanTicket,
        account: &mut ExecutionAccount,
    ) -> Result<BundlePin<P>, LoanError> {
        let bundle = account.ledger.with_work_phase(
            super::FAMILY,
            "ticket-to-pin authentication",
            |budget| {
                charge(budget, 2)?;
                if ticket.identity.scope != self.scope {
                    return Err(LoanError::ForeignOwner);
                }
                if ticket.identity.format != crate::storage::Format::of::<P>() {
                    return Err(LoanError::LayoutMismatch);
                }
                let group = self.loans.group(ticket.identity)?;
                let State::Published(bundle) = &group.state else {
                    return Err(crate::HeapError::OwnershipPending.into());
                };
                self.current_owner(bundle.payload.as_ref().ok_or(LoanError::WrongLoan)?, budget)?;
                Ok(Rc::clone(bundle))
            },
        )?;
        self.make_pin(bundle, account)
    }
    /// Duplicate a retained pin with a separately charged independent lifetime.
    /// # Errors
    /// Refuses a foreign owner, invalid shell or insufficient metadata/Work capacity.
    pub fn retain_pin<P: Scalar + 'static>(
        &mut self,
        pin: &BundlePin<P>,
        account: &mut ExecutionAccount,
    ) -> Result<BundlePin<P>, LoanError> {
        account
            .ledger
            .with_work_phase(super::FAMILY, "retained pin authentication", |budget| {
                self.current_bundle(pin, budget)
            })?;
        let payload = pin.record.payload.as_ref().ok_or(LoanError::WrongLoan)?;
        self.make_pin(Rc::clone(&payload.bundle), account)
    }
    /// End exactly one prepaid use edge and observe the zero-new-Work release phase.
    /// # Errors
    /// Returns the same pin for a foreign heap/account or an incomplete private shell.
    pub fn release_pin<P: Scalar + 'static>(
        &mut self,
        pin: BundlePin<P>,
        account: &mut ExecutionAccount,
    ) -> Result<(), (LoanError, BundlePin<P>)> {
        let checked = pin.ticket().and_then(|ticket| {
            if ticket.identity.scope != self.scope {
                return Err(LoanError::ForeignOwner);
            }
            let payload = pin.record.payload.as_ref().ok_or(LoanError::WrongLoan)?;
            account.authenticate(payload.account)
        });
        if let Err(error) = checked {
            return Err((error, pin));
        }
        let result = account
            .ledger
            .with_work_phase(super::FAMILY, "prepaid pin release", |_| {
                drop(pin);
                Ok::<(), std::convert::Infallible>(())
            });
        match result {
            Ok(()) => Ok(()),
            Err(never) => match never {},
        }
    }
    /// Reserve header and weak index before the new public edge can escape.
    fn make_pin<P: Scalar + 'static>(
        &mut self,
        bundle: Rc<super::registry::BundleOwner>,
        account: &mut ExecutionAccount,
    ) -> Result<BundlePin<P>, LoanError> {
        account
            .ledger
            .with_work_phase(super::FAMILY, "pin acquisition", |budget| {
                let identity = bundle
                    .payload
                    .as_ref()
                    .ok_or(LoanError::WrongLoan)?
                    .identity;
                let PinPlan {
                    revision,
                    pin_slot,
                    pins,
                    debit_slot,
                    debits,
                    allocations,
                } = pin_plan(self, budget)?;
                let bytes = rc_bytes::<PinRecord>();
                charge(budget, 7 + allocations)?;
                let mut guard =
                    super::preparation::Preparation::new(super::registry::CleanupReceipt {
                        identity: identity.receipt,
                        allocations,
                        records: 2,
                        edges: 0,
                    });
                super::lifecycle::admission::grow_tracked(&mut self.loans.pins, pins, &mut guard)?;
                super::lifecycle::admission::grow_tracked(
                    &mut self.loans.debits,
                    debits,
                    &mut guard,
                )?;
                let key = crate::heap::identity::identity()?;
                let pin = key;
                #[cfg(test)]
                super::fault::before()?;
                let record = Rc::new(PinRecord {
                    payload: Some(PinPayload {
                        identity: pin,
                        account: account.identity,
                        index: pin_slot,
                        bundle,
                        token: Token {
                            slot: debit_slot,
                            key,
                            receipt: identity.receipt,
                        },
                    }),
                });
                guard.acquired(false)?;
                let receipt = guard.finish()?;
                if receipt.identity != identity.receipt {
                    return Err(LoanError::WrongLoan);
                }
                self.loans.install(
                    debit_slot,
                    Debit {
                        kind: super::registry::AllocationKind::Pin,
                        owner: crate::loan::registry::DebitOwner::Bundle(identity),
                        key,
                        elements: 0,
                        bytes,
                    },
                );
                self.loans.install_pin(
                    pin_slot,
                    PinIndex {
                        identity: pin,
                        witness: Some(Rc::downgrade(&record)),
                        debit: (debit_slot, key),
                    },
                );
                self.loans.revision = revision;
                Ok(BundlePin {
                    record,
                    marker: PhantomData,
                })
            })
    }
}

/// Prepared weak/debit positions and exact replacement capacities for one independent pin.
struct PinPlan {
    /// Mutation revision preserving every pending producer's terminal successor.
    revision: u64,
    /// Exact weak-index row.
    pin_slot: usize,
    /// Complete prepared weak-index capacity.
    pins: usize,
    /// Exact unique header-debit row.
    debit_slot: usize,
    /// Complete prepared debit capacity.
    debits: usize,
    /// Newly allocated pin header and replacement buffers.
    allocations: usize,
}
/// Plan all table scans and simultaneous bytes before constructing any new pin allocation.
fn pin_plan(heap: &Heap, budget: &mut nrz_bindings::Budget) -> Result<PinPlan, LoanError> {
    let revision = heap.loans.next_revision()?;
    charge(budget, heap.loans.pins.len())?;
    let pin_slot = heap.loans.vacant_pin();
    let pins = pin_slot
        .checked_add(1)
        .ok_or(LoanError::CountOverflow)?
        .max(heap.loans.pins.capacity());
    charge(budget, heap.loans.debits.len())?;
    let debit_slot = heap.loans.vacant_debits(1)?[0];
    let debits = debit_slot
        .checked_add(1)
        .ok_or(LoanError::CountOverflow)?
        .max(heap.loans.debits.capacity());
    let (pin_growth, old_pins) = super::lifecycle::admission::growth(&heap.loans.pins, pins)?;
    let (debit_growth, old_debits) =
        super::lifecycle::admission::growth(&heap.loans.debits, debits)?;
    let tables = pin_growth
        .checked_add(debit_growth)
        .ok_or(LoanError::ByteOverflow)?;
    let old_tables = old_pins
        .checked_add(old_debits)
        .ok_or(LoanError::ByteOverflow)?;
    let bytes = rc_bytes::<PinRecord>();
    charge(budget, heap.resource_work()?)?;
    heap.preflight_resources(
        ResourcePlan {
            retained: bytes.checked_add(tables).ok_or(LoanError::ByteOverflow)?,
            scratch: old_tables.checked_mul(2).ok_or(LoanError::ByteOverflow)?,
            ..ResourcePlan::default()
        },
        ResourcePolicy::from_budget(budget),
    )?;
    // Authentication, creation, two prepared records, destruction and one pin drop.
    let allocations = 1 + usize::from(pin_growth != 0) + usize::from(debit_growth != 0);
    Ok(PinPlan {
        revision,
        pin_slot,
        pins,
        debit_slot,
        debits,
        allocations,
    })
}
