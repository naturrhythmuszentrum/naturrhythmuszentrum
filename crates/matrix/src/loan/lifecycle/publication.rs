// SPDX-License-Identifier: AGPL-3.0-only
//! All fallible staging precedes the final allocation-free publication move.

use super::{HandbackOutcome, OwnedLoan, complete};
use crate::loan::{
    ExecutionAccount, HostChannel, LoanError, charge,
    payload::{PublishedChannel, bit},
    rc_bytes,
    reader::{BundlePin, PublishedTile},
    registry::{
        Bundle, Debit, PinIndex, PinPayload, ProducerOutcome, PublicationReceipt,
        QualifiedMaskVersion, State, Token,
    },
    vector, words,
};
use crate::{
    Heap,
    heap::{
        descriptors::DescriptorRecord,
        resources::{ResourcePlan, ResourcePolicy, add},
    },
    mask::storage::{MaskRecord, Native},
};
use nrz_scalar::Scalar;
use std::{marker::PhantomData, rc::Rc};

/// Fully allocated publication staging; its drop destroys only this attempt's private prefix.
struct Prepared {
    /// Shared filled-word owner, not yet visible through native resolution.
    mask: Rc<MaskRecord>,
    /// Unique mask allocation reservation.
    debit: Debit,
    /// Prepared exact debit position.
    debit_slot: usize,
    /// Precomputed successful mutation revision.
    revision: u64,
    /// Exact initial weak-pin row already occupied by the pending producer.
    pin_slot: usize,
    /// Reserved initial-pin identity.
    pin: u64,
    /// Staging cleanup entitlement follows the new immutable mask owner.
    cleanup: crate::loan::registry::CleanupReceipt,
}
/// Recover the same core after every failed validation or staging attempt.
pub(super) fn handback<P: Scalar + 'static>(
    heap: &mut Heap,
    mut loan: OwnedLoan<P>,
    account: &mut ExecutionAccount,
) -> HandbackOutcome<P> {
    let prepared =
        account
            .ledger
            .with_work_phase(crate::loan::FAMILY, "handback preparation", |budget| {
                prepare(heap, &mut loan, budget)
            });
    let prepared = match prepared {
        Ok(prepared) => prepared,
        Err(error) => return HandbackOutcome::Repair(error, loan),
    };
    let identity = loan.core.identity;
    loan.core.cleanup.allocations += prepared.cleanup.allocations;
    loan.core.cleanup.records += prepared.cleanup.records;
    loan.core.cleanup.edges += prepared.cleanup.edges;
    let qualified = std::array::from_fn(|index| {
        identity.channels[index].map(|_| QualifiedMaskVersion {
            identity,
            channel: HostChannel::ALL[index],
        })
    });
    let receipt = PublicationReceipt {
        ticket: loan.ticket(),
        present: qualified.map(|attachment| attachment.is_some()),
    };
    // Exclusive heap access and private shells make these unique accesses stable through commit.
    let Some(bundle_shell) = Rc::get_mut(&mut loan.core.bundle) else {
        return HandbackOutcome::Repair(LoanError::WrongLoan, loan);
    };
    let Some(pin_shell) = Rc::get_mut(&mut loan.core.pin) else {
        return HandbackOutcome::Repair(LoanError::WrongLoan, loan);
    };
    let channels = take_channels(&mut loan.core.channels, &mut heap.loans);
    let [
        core_token,
        layout_token,
        liveness_token,
        bundle_token,
        pin_token,
    ] = loan.core.headers;
    let mask_token_key = prepared.debit.key;
    let mask_token = Token {
        slot: prepared.debit_slot,
        key: prepared.debit.key,
        receipt: identity.receipt,
    };
    bundle_shell.payload = Some(Bundle {
        retained_law: loan.core.retained_law,
        origin: crate::loan::application::Origin::host(),
        declaration: loan.core.declaration,
        identity,
        channels,
        layout: Rc::clone(&loan.core.layout),
        mask: Rc::clone(&prepared.mask),
        qualified,
        cleanup: loan.core.cleanup,
        retained: [layout_token, bundle_token, mask_token],
    });
    pin_shell.payload = Some(PinPayload {
        identity: prepared.pin,
        account: identity.account,
        index: prepared.pin_slot,
        bundle: Rc::clone(&loan.core.bundle),
        token: pin_token,
    });
    install_native(heap, identity, loan.core.layout, prepared.mask, &qualified);
    heap.loans.install(prepared.debit_slot, prepared.debit);
    let pin_debit = pin_shell
        .payload
        .as_ref()
        .map_or((0, 0), |pin| (pin.token.slot, pin.token.key));
    let witness = Rc::downgrade(&loan.core.pin);
    heap.loans.install_pin(
        prepared.pin_slot,
        PinIndex {
            identity: prepared.pin,
            witness: Some(witness),
            debit: pin_debit,
        },
    );
    publish_group(
        heap,
        identity,
        pin_debit,
        (prepared.debit_slot, mask_token_key),
        loan.core.bundle,
        receipt,
    );
    drop(loan.core.witness);
    heap.loans.release(core_token.slot, core_token.key);
    heap.loans.release(liveness_token.slot, liveness_token.key);
    heap.loans.revision = prepared.revision;
    heap.loans.pending -= 1;
    HandbackOutcome::Published(
        PublishedTile {
            pin: BundlePin {
                record: loan.core.pin,
                marker: PhantomData,
            },
        },
        receipt,
    )
}
/// Derive cell-local conjunctive coverage, then validate only selected initialized values.
fn prepare<P: Scalar + 'static>(
    heap: &mut Heap,
    loan: &mut OwnedLoan<P>,
    budget: &mut nrz_bindings::Budget,
) -> Result<Prepared, LoanError> {
    let identity = loan.core.identity;
    let revision = heap.loans.terminal_revision()?;
    let cells = loan.capacity();
    let word_count = words(cells);
    let bytes = add(
        rc_bytes::<MaskRecord>(),
        word_count.checked_mul(8).ok_or(LoanError::ByteOverflow)?,
    )?;
    charge(budget, heap.loans.debits.len())?;
    let debit_slot = heap.loans.vacant_debits(1)?[0];
    let debit_capacity = add(debit_slot, 1)?.max(heap.loans.debits.capacity());
    let (tables, old_tables) = super::admission::growth(&heap.loans.debits, debit_capacity)?;
    charge(budget, heap.resource_work()?)?;
    let permit = heap.preflight_resources(
        ResourcePlan {
            retained: add(bytes, tables)?,
            scratch: old_tables.checked_mul(2).ok_or(LoanError::ByteOverflow)?,
            ..ResourcePlan::default()
        },
        ResourcePolicy::from_budget(budget),
    )?;
    if permit.scope != heap.scope {
        return Err(LoanError::ForeignOwner);
    }
    // Each attempt prepays its own exact acquired prefix, including retained replacement capacity.
    let allocations = 1 + usize::from(word_count != 0) + usize::from(tables != 0);
    charge(budget, allocations + 4)?;
    let mut guard =
        crate::loan::preparation::Preparation::new(crate::loan::registry::CleanupReceipt {
            identity: identity.receipt,
            allocations,
            records: 1,
            edges: 3,
        });
    super::admission::grow_tracked(&mut heap.loans.debits, debit_capacity, &mut guard)?;
    let mut filled = vector(word_count)?;
    if word_count != 0 {
        guard.acquired(false)?;
    }
    charge(budget, word_count)?;
    filled.resize(word_count, 0);
    derive_filled(loan, &mut filled, budget)?;
    validate_payload::<P>(&loan.core, &filled, budget)?;
    let channels = loan.core.channels.iter().flatten().count();
    charge(budget, channels + 1)?;
    loan.core
        .declaration
        .symbols
        .check_import()
        .map_err(|_| LoanError::StaleDependency)?;
    if loan.core.layout.axis != loan.core.declaration.axis
        || loan.core.layout.components.extent() != loan.core.declaration.extent
    {
        return Err(LoanError::LayoutMismatch);
    }
    validate_shells(heap, &loan.core, budget)?;
    charge(budget, channels + 6)?;
    #[cfg(test)]
    crate::loan::fault::before()?;
    let mask = Rc::new(MaskRecord {
        layout: identity.layout,
        words: filled,
    });
    guard.acquired(false)?;
    let key = crate::heap::identity::identity()?;
    let pin = loan.core.headers[4].key;
    let pin_slot = heap.loans.group(identity)?.pin;
    let reserved_pin = heap
        .loans
        .pins
        .get(pin_slot)
        .and_then(Option::as_ref)
        .ok_or(LoanError::WrongLoan)?;
    if reserved_pin.identity != pin || reserved_pin.witness.is_some() {
        return Err(LoanError::WrongLoan);
    }
    Ok(Prepared {
        mask,
        debit_slot,
        debit: Debit {
            kind: crate::loan::registry::AllocationKind::Filled,
            owner: crate::loan::registry::DebitOwner::Bundle(identity),
            key,
            elements: 0,
            bytes,
        },
        revision,
        pin_slot,
        pin,
        cleanup: guard.finish()?,
    })
}

/// Move native payloads and destroy prepaid tracking allocations during the final commit.
pub(in crate::loan) fn take_channels(
    channels: &mut [Option<crate::loan::payload::LoanChannel>; 3],
    registry: &mut crate::loan::Registry,
) -> [Option<PublishedChannel>; 3] {
    std::mem::take(channels).map(|owner| {
        owner.map(|owner| {
            let [payload, components, complete] = owner.tokens;
            drop(owner.components);
            drop(owner.complete);
            registry.release(components.slot, components.key);
            registry.release(complete.slot, complete.key);
            PublishedChannel {
                matrix: owner.matrix,
                token: payload,
            }
        })
    })
}

/// Install only prepared native owners and descriptor attachments under the exclusive borrow.
pub(in crate::loan) fn install_native(
    heap: &mut Heap,
    identity: crate::loan::registry::Identity,
    layout: Rc<crate::mask::storage::LayoutRecord>,
    mask: Rc<MaskRecord>,
    qualified: &[Option<QualifiedMaskVersion>; 3],
) {
    if let Ok(entry) = heap.masks.layouts.get_mut(identity.layout.key) {
        *entry = Native::Published {
            owner: layout,
            managed: true,
        };
    }
    if let Ok(entry) = heap.masks.masks.get_mut(identity.mask.key) {
        *entry = Native::Published {
            owner: mask,
            managed: true,
        };
    }
    for key in identity.channels.into_iter().flatten() {
        if let Ok(DescriptorRecord::Managed(record)) = heap.descriptors.slots.get_mut(key) {
            record.filled = qualified[record.channel.index()].is_some();
        }
    }
}

/// Numerical validation never visits a hole, even when its safe initialized bytes are invalid.
pub(in crate::loan) fn validate_payload<P: Scalar + 'static>(
    core: &super::HostCore,
    filled: &[u64],
    budget: &mut nrz_bindings::Budget,
) -> Result<(), LoanError> {
    for cell in 0..core.declaration.extent.rows() {
        if filled_cell(filled, cell) {
            validate_cell::<P>(core, cell, budget)?;
        }
    }
    Ok(())
}
/// One actual filled-membership visit, shared by host and application publication.
pub(in crate::loan) fn filled_cell(filled: &[u64], cell: usize) -> bool {
    #[cfg(test)]
    crate::loan::fault::visited_publication_cell();
    bit(filled, cell)
}
/// Validate only the completed cell already selected by the caller's paid traversal.
pub(in crate::loan) fn validate_cell<P: Scalar + 'static>(
    core: &super::HostCore,
    cell: usize,
    budget: &mut nrz_bindings::Budget,
) -> Result<(), LoanError> {
    let cells = core.declaration.extent.rows();
    for component in 0..core.declaration.extent.columns() {
        let address = cell + cells * component;
        let mut total = None;
        for (channel, owner) in HostChannel::ALL.into_iter().zip(&core.channels) {
            if let Some(owner) = owner {
                charge(budget, 1)?;
                let value: P = owner.matrix.read(address)?;
                let valid = value.is_finite()
                    && (channel == HostChannel::Center || value >= P::ZERO)
                    && (channel != HostChannel::Error || total.is_some_and(|total| value <= total));
                if !valid {
                    return Err(LoanError::InvalidPayload {
                        channel,
                        cell,
                        component,
                    });
                }
                if channel == HostChannel::Total {
                    total = Some(value);
                }
            }
        }
    }
    Ok(())
}

/// Fixed producer metadata is replaced only after its complete owning bundle is ready.
fn publish_group(
    heap: &mut Heap,
    identity: crate::loan::registry::Identity,
    pin_debit: (usize, u64),
    mask_debit: (usize, u64),
    bundle: Rc<crate::loan::registry::BundleOwner>,
    receipt: PublicationReceipt,
) {
    if let Some(group) = heap
        .loans
        .groups
        .get_mut(identity.root.slot)
        .and_then(Option::as_mut)
    {
        for entry in group.debits.entries_mut() {
            if *entry == Some(pin_debit) {
                *entry = None;
            }
        }
        group.debits.entries_mut()[14] = Some(mask_debit);
        group.state = State::Published(bundle);
        group.outcome = ProducerOutcome::Published(receipt);
        group.delivered = true;
    }
}

/// Native reservations and private publication shells are stable under the exclusive heap borrow.
pub(in crate::loan) fn validate_shells(
    heap: &Heap,
    core: &super::HostCore,
    budget: &mut nrz_bindings::Budget,
) -> Result<(), LoanError> {
    // Two native reserved owners and the two private publication shells are checked again.
    charge(budget, 4)?;
    let identity = core.identity;
    for entry in [
        heap.masks.layouts.get(identity.layout.key)?.reserved_root(),
        heap.masks.masks.get(identity.mask.key)?.reserved_root(),
    ] {
        if entry != Some(identity.root) {
            return Err(LoanError::StaleDependency);
        }
    }
    if Rc::strong_count(&core.bundle) != 1
        || Rc::weak_count(&core.bundle) != 0
        || Rc::strong_count(&core.pin) != 1
        || Rc::weak_count(&core.pin) != 0
    {
        return Err(LoanError::WrongLoan);
    }
    Ok(())
}

/// Direct cell-local intersection reads every participating complete word and clears padding.
fn derive_filled<P: Scalar + 'static>(
    loan: &mut OwnedLoan<P>,
    filled: &mut [u64],
    budget: &mut nrz_bindings::Budget,
) -> Result<(), LoanError> {
    let cells = loan.capacity();
    complete(&mut loan.core, budget)?;
    for (index, word) in filled.iter_mut().enumerate() {
        *word = u64::MAX;
        for channel in loan.core.channels.iter().flatten() {
            charge(budget, 1)?;
            *word &= channel.complete[index];
        }
        charge(budget, 1)?;
    }
    charge(budget, 1)?;
    if cells % 64 != 0
        && let Some(last) = filled.last_mut()
    {
        *last &= (1_u64 << (cells % 64)) - 1;
    }
    Ok(())
}
