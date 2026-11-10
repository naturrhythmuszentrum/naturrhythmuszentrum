// SPDX-License-Identifier: AGPL-3.0-only
//! Sealed completion gates the common managed publication, then releases the source edge.

use super::{
    ApplicationCancellation, ApplicationCore, ApplicationPublication, CellOutcome, Origin,
    OwnedApplication,
};
use crate::loan::{
    ExecutionAccount, LoanError, charge,
    preparation::Preparation,
    registry::{
        Bundle, PinIndex, PinPayload, ProducerOutcome, PublicationReceipt, QualifiedMaskVersion,
        State,
    },
};
use crate::{
    Heap,
    heap::resources::{ResourcePlan, ResourcePolicy},
    mask::storage::MaskRecord,
};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Empty mask shell and separately prepaid destruction, prepared before ownership moves.
struct Prepared {
    /// Empty backing is replaced only during the no-fail commit.
    mask: Rc<MaskRecord>,
    /// Exact new byte total in the existing filled-owner debit.
    filled_bytes: usize,
    /// The producer's pre-reserved terminal successor.
    revision: u64,
    /// Reserved initial result-pin index.
    pin_slot: usize,
    /// This attempt's acquired shell cleanup.
    cleanup: crate::loan::registry::CleanupReceipt,
}
/// Ready owners and authentication failures keep the exact owner and phase.
pub(super) fn publish<P: Scalar + 'static>(
    heap: &mut Heap,
    mut owner: OwnedApplication<P>,
    account: &mut ExecutionAccount,
) -> ApplicationPublication<P> {
    let authentication = account.ledger.with_work_phase(
        super::FAMILY,
        "application return authentication",
        |budget| {
            charge(
                budget,
                25 + 7 * usize::from(owner.core.evidence.prepared.is_some())
                    + crate::loan::retained_expm1::Attachment::WORK
                        * usize::from(owner.core.host.retained_law.is_some()),
            )?;
            authenticate(heap, &owner.core, account.identity)
        },
    );
    if let Err(error) = authentication {
        return ApplicationPublication::ApplicationRejected(error.into(), owner);
    }
    if !owner.core.candidate {
        return ApplicationPublication::ApplicationRejected(
            LoanError::UnsupportedContract.into(),
            owner,
        );
    }
    let prepared =
        account
            .ledger
            .with_work_phase(super::FAMILY, "application publication", |budget| {
                prepare(heap, &mut owner.core, budget)
            });
    match prepared {
        Ok(prepared) => commit(heap, owner, prepared),
        Err(error) => ApplicationPublication::ApplicationRepair(error.into(), owner),
    }
}
/// Authenticate the shared core and all three additional exact allocation tokens.
fn authenticate<P: Scalar>(
    heap: &Heap,
    core: &ApplicationCore<P>,
    account: u64,
) -> Result<(), LoanError> {
    if account != core.host.identity.account {
        return Err(LoanError::ForeignAccount);
    }
    heap.authenticate_identity(core.host.identity)?;
    if let Some(law) = &core.host.retained_law {
        law.validate_current(heap)?;
    }
    super::endpoints::validate_core(core)?;
    if core.host.channels.iter().any(Option::is_none)
        || core.host.identity.channels.iter().any(Option::is_none)
    {
        return Err(LoanError::WrongLoan);
    }
    let group = heap.loans.group(core.host.identity)?;
    let State::Out(witness) = &group.state else {
        return Err(LoanError::DuplicateReturn);
    };
    let inventory_matches = matches!(
        (core.evidence.prepared.is_some(), &group.debits),
        (false, crate::loan::registry::DebitInventory::Application(_))
            | (
                true,
                crate::loan::registry::DebitInventory::PreparedApplication(_)
            )
    );
    if !inventory_matches
        || !witness.ptr_eq(&Rc::downgrade(&core.host.witness))
        || core.host.witness.identity != core.host.identity
        || core.host.witness.receipt != core.host.cleanup.identity
    {
        return Err(LoanError::WrongLoan);
    }
    for token in core
        .host
        .headers
        .iter()
        .chain(
            core.host
                .channels
                .iter()
                .flatten()
                .flat_map(|channel| channel.tokens.iter()),
        )
        .chain(
            core.endpoints
                .iter()
                .flatten()
                .flat_map(|channel| channel.tokens.iter()),
        )
        .chain([&core.filled_token, &core.extent_token, &core.evidence.token])
    {
        if !heap.loans.authentic_token(token, core.host.identity) {
            return Err(LoanError::WrongLoan);
        }
    }
    Ok(())
}
/// Only the shell is new: prospective words and their exact debit already belong to the core.
fn prepare<P: Scalar + 'static>(
    heap: &Heap,
    core: &mut ApplicationCore<P>,
    budget: &mut nrz_bindings::Budget,
) -> Result<Prepared, LoanError> {
    let identity = core.host.identity;
    let revision = heap.loans.terminal_revision()?;
    shell_preflight(heap, core, budget)?;
    charge(budget, 5)?;
    let mut guard = Preparation::new(crate::loan::registry::CleanupReceipt {
        identity: identity.receipt,
        allocations: 1,
        records: 1,
        edges: 3,
    });
    heap.current_bundle(core.input.as_ref().ok_or(LoanError::WrongLoan)?, budget)?;
    charge(budget, 1)?;
    if core
        .input
        .as_ref()
        .ok_or(LoanError::WrongLoan)?
        .ticket()?
        .identity
        != core.evidence.source_identity
    {
        return Err(LoanError::WrongLoan);
    }
    core.evidence.validate(&core.host.declaration, identity)?;
    derive(core, budget)?;
    crate::loan::lifecycle::publication::validate_shells(heap, &core.host, budget)?;
    charge(budget, 9)?;
    let pin_slot = heap.loans.group(identity)?.pin;
    let reserved = heap
        .loans
        .pins
        .get(pin_slot)
        .and_then(Option::as_ref)
        .ok_or(LoanError::WrongLoan)?;
    if reserved.identity != core.host.headers[4].key || reserved.witness.is_some() {
        return Err(LoanError::WrongLoan);
    }
    let debit = heap
        .loans
        .debits
        .get(core.filled_token.slot)
        .and_then(Option::as_ref)
        .ok_or(LoanError::WrongLoan)?;
    let filled_bytes = debit
        .bytes
        .checked_add(crate::loan::rc_bytes::<MaskRecord>())
        .ok_or(LoanError::ByteOverflow)?;
    #[cfg(test)]
    crate::loan::fault::before()?;
    let mask = Rc::new(MaskRecord {
        layout: identity.layout,
        words: Vec::new(),
    });
    guard.acquired(false)?;
    Ok(Prepared {
        mask,
        filled_bytes,
        revision,
        pin_slot,
        cleanup: guard.finish()?,
    })
}
/// Publication and isolated shell admission derive the byte policy from the same checked core.
pub(super) fn shell_preflight<P: Scalar>(
    heap: &Heap,
    core: &ApplicationCore<P>,
    budget: &mut nrz_bindings::Budget,
) -> Result<(), LoanError> {
    charge(budget, heap.resource_work()?)?;
    if core.evidence.prepared.is_some() {
        budget.check(
            nrz_bindings::readings::algebra::limits::BudgetKind::StagedPayloadBytes,
            u64::try_from(crate::loan::rc_bytes::<MaskRecord>())
                .map_err(|_| LoanError::ByteOverflow)?,
            crate::loan::AT,
        )?;
    }
    let mut permit = heap.preflight_resources(
        ResourcePlan {
            retained: crate::loan::rc_bytes::<MaskRecord>(),
            ..ResourcePlan::default()
        },
        ResourcePolicy::from_budget(budget),
    )?;
    permit.account = Some(core.host.identity.account);
    permit.storage(heap.storage.epoch())?;
    if permit.scope != heap.scope {
        return Err(LoanError::ForeignOwner);
    }
    Ok(())
}
/// The initialized intersection can authorize only cells with independently sealed completion.
fn derive<P: Scalar + 'static>(
    core: &mut ApplicationCore<P>,
    budget: &mut nrz_bindings::Budget,
) -> Result<(), LoanError> {
    crate::loan::lifecycle::complete(&mut core.host, budget)?;
    super::endpoints::complete(core, budget)?;
    for (index, word) in core.filled.iter().enumerate() {
        let mut intersection = u64::MAX;
        for channel in core
            .host
            .channels
            .iter()
            .flatten()
            .chain(core.endpoints.iter().flatten())
        {
            charge(budget, 1)?;
            intersection &= channel.complete[index];
        }
        charge(budget, 1)?;
        if *word & !intersection != 0 {
            return Err(LoanError::CoverageMismatch);
        }
    }
    charge(budget, 1)?;
    let cells = core.host.declaration.extent.rows();
    if cells % 64 != 0
        && core
            .filled
            .last()
            .is_some_and(|word| *word >> (cells % 64) != 0)
    {
        return Err(LoanError::CoverageMismatch);
    }
    for (cell, outcome) in core.evidence.outcomes.iter().enumerate() {
        charge(budget, 1)?;
        let completed = matches!(outcome, CellOutcome::Completed);
        if completed != crate::loan::lifecycle::publication::filled_cell(&core.filled, cell)
            || matches!(outcome, CellOutcome::Pending)
        {
            return Err(LoanError::CoverageMismatch);
        }
        if completed {
            if core.evidence.prepared.is_some() {
                super::endpoints::validate_cell(core, cell, budget)?;
            } else {
                crate::loan::lifecycle::publication::validate_cell::<P>(&core.host, cell, budget)?;
            }
        }
        if let CellOutcome::PreparedRefused { component, .. } = outcome
            && (*component >= core.host.declaration.extent.columns()
                || core.evidence.prepared.is_none())
        {
            return Err(LoanError::WrongLoan);
        }
        if let CellOutcome::CellRefused { component, reason } = outcome {
            if core.evidence.prepared.is_some()
                || *component >= core.host.declaration.extent.columns()
            {
                return Err(LoanError::WrongLoan);
            }
            if let crate::KernelError::Domain {
                function, entry, ..
            } = reason
                && (*function != crate::Elementwise::Square || *entry != cell + cells * component)
            {
                return Err(LoanError::WrongLoan);
            }
        }
    }
    Ok(())
}
/// All unique-shell accesses are confirmed before moving any source or result ownership.
fn commit<P: Scalar + 'static>(
    heap: &mut Heap,
    mut owner: OwnedApplication<P>,
    mut prepared: Prepared,
) -> ApplicationPublication<P> {
    if Rc::get_mut(&mut owner.core.host.bundle).is_none()
        || Rc::get_mut(&mut owner.core.host.pin).is_none()
        || Rc::get_mut(&mut prepared.mask).is_none()
    {
        return ApplicationPublication::ApplicationRepair(LoanError::WrongLoan.into(), owner);
    }
    finish(heap, owner, prepared)
}
/// No-fail pointer moves publish the independent bundle before releasing the transferred source.
fn finish<P: Scalar + 'static>(
    heap: &mut Heap,
    owner: OwnedApplication<P>,
    mut prepared: Prepared,
) -> ApplicationPublication<P> {
    let ApplicationCore {
        endpoints,
        input,
        contract,
        extent_token,
        filled,
        filled_token,
        mut evidence,
        population: _,
        candidate: _,
        mut host,
    } = *owner.core;
    let identity = host.identity;
    let qualified = std::array::from_fn(|index| {
        Some(QualifiedMaskVersion {
            identity,
            channel: crate::loan::HostChannel::ALL[index],
        })
    });
    let receipt = receipt(identity);
    if let Some(mask) = Rc::get_mut(&mut prepared.mask) {
        mask.words = filled;
    }
    if evidence.prepared.is_some() {
        evidence.completed_mask = Some(std::ptr::NonNull::from(prepared.mask.as_ref()));
    }
    host.cleanup.allocations += prepared.cleanup.allocations;
    host.cleanup.records += prepared.cleanup.records;
    host.cleanup.edges += prepared.cleanup.edges;
    let endpoints = endpoints.map(|pair| super::endpoints::publish(pair, &mut heap.loans));
    let channels =
        crate::loan::lifecycle::publication::take_channels(&mut host.channels, &mut heap.loans);
    let [
        core_token,
        layout_token,
        liveness_token,
        bundle_token,
        pin_token,
    ] = host.headers;
    let pin_identity = pin_token.key;
    if let Some(bundle) = Rc::get_mut(&mut host.bundle) {
        bundle.payload = Some(Bundle {
            retained_law: host.retained_law,
            origin: Origin::application(evidence),
            endpoints,
            declaration: host.declaration,
            identity,
            channels,
            layout: Rc::clone(&host.layout),
            mask: Rc::clone(&prepared.mask),
            qualified,
            cleanup: host.cleanup,
            retained: [layout_token, bundle_token, filled_token],
        });
    }
    if let Some(pin) = Rc::get_mut(&mut host.pin) {
        pin.payload = Some(PinPayload {
            identity: pin_identity,
            account: identity.account,
            index: prepared.pin_slot,
            bundle: Rc::clone(&host.bundle),
            token: pin_token,
        });
    }
    if let Some(bundle) = host.bundle.payload.as_ref()
        && let Some(debit) = heap
            .loans
            .debits
            .get_mut(bundle.retained[2].slot)
            .and_then(Option::as_mut)
    {
        debit.bytes = prepared.filled_bytes;
    }
    crate::loan::lifecycle::publication::install_native(
        heap,
        identity,
        host.layout,
        prepared.mask,
        &qualified,
    );
    install_result(
        heap,
        identity,
        host.bundle,
        &host.pin,
        prepared.pin_slot,
        receipt,
    );
    // The terminal producer exists before either the transferred edge or witness expires.
    drop(input);
    drop(contract);
    drop(host.witness);
    heap.loans.release(extent_token.slot, extent_token.key);
    heap.loans.release(core_token.slot, core_token.key);
    heap.loans.release(liveness_token.slot, liveness_token.key);
    heap.loans.revision = prepared.revision;
    heap.loans.pending -= 1;
    ApplicationPublication::ApplicationPublished(
        crate::loan::lifecycle::publication::tile(host.pin),
        receipt,
    )
}
/// The fixed three-channel receipt has no ownership or numerical authority.
const fn receipt(identity: crate::loan::registry::Identity) -> PublicationReceipt {
    PublicationReceipt {
        ticket: crate::loan::LoanTicket { identity },
        present: [true; 3],
    }
}
/// Install the initial use edge and terminal producer from already complete private shells.
fn install_result(
    heap: &mut Heap,
    identity: crate::loan::registry::Identity,
    bundle: Rc<crate::loan::registry::BundleOwner>,
    pin: &Rc<crate::loan::registry::PinRecord>,
    pin_slot: usize,
    receipt: PublicationReceipt,
) {
    let Some(payload) = pin.payload.as_ref() else {
        return;
    };
    let pin_identity = payload.identity;
    let pin_debit = (payload.token.slot, payload.token.key);
    heap.loans.install_pin(
        pin_slot,
        PinIndex {
            identity: pin_identity,
            witness: Some(Rc::downgrade(pin)),
            debit: pin_debit,
        },
    );
    if let Some(group) = heap
        .loans
        .groups
        .get_mut(identity.root.slot)
        .and_then(Option::as_mut)
    {
        for debit in group.debits.entries_mut() {
            if *debit == Some(pin_debit) {
                *debit = None;
            }
        }
        group.state = State::Published(bundle);
        group.outcome = ProducerOutcome::Published(receipt);
        group.delivered = true;
    }
}
/// Cancellation authenticates the exact admitted inventory and spends only prepaid cleanup.
pub(super) fn cancel<P: Scalar + 'static>(
    heap: &mut Heap,
    owner: OwnedApplication<P>,
    account: &mut ExecutionAccount,
) -> ApplicationCancellation<P> {
    if let Err(error) = authenticate(heap, &owner.core, account.identity) {
        let rejected = account.ledger.with_work_phase(
            super::FAMILY,
            "rejected application cancellation",
            |budget| charge(budget, 1),
        );
        return ApplicationCancellation::ApplicationRejected(
            rejected.err().unwrap_or(error).into(),
            owner,
        );
    }
    let identity = owner.core.host.identity;
    let revision = match heap.loans.terminal_revision() {
        Ok(revision) => revision,
        Err(error) => return ApplicationCancellation::ApplicationRejected(error.into(), owner),
    };
    let receipt = crate::loan::CancellationReceipt {
        ticket: owner.ticket(),
        reason: crate::loan::AbsenceReason::Cancelled,
    };
    if let Some(group) = heap
        .loans
        .groups
        .get_mut(identity.root.slot)
        .and_then(Option::as_mut)
    {
        group.state = State::Retiring;
        group.outcome = ProducerOutcome::Absent(receipt);
        group.delivered = true;
    }
    let released =
        account
            .ledger
            .with_work_phase(super::FAMILY, "prepaid application cancellation", |_| {
                drop(owner);
                Ok::<(), std::convert::Infallible>(())
            });
    match released {
        Ok(()) => {}
        Err(never) => match never {},
    }
    heap.release_owner_debits(identity);
    heap.retire_native_reservations(identity);
    heap.loans.revision = revision;
    heap.loans.pending -= 1;
    ApplicationCancellation::ApplicationCancelled(receipt)
}
