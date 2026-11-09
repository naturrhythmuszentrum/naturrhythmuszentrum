// SPDX-License-Identifier: AGPL-3.0-only
//! The complete seventeen-entry plan precedes every private application allocation.

mod plan;
mod preparation;
use super::{
    ApplicationCore, ApplicationError, ApplicationRequest, OwnedApplication, PointwiseRequest,
};
use crate::loan::{
    ExecutionAccount, LoanError,
    registry::{BundleOwner, Debit, Liveness, PinRecord},
};
use crate::{
    Heap,
    heap::{descriptors::DescriptorRecord, slots::PreparedSlots},
    mask::storage::{LayoutRecord, MaskRecord, Native},
};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Private common owners and extra allocations remain separate from the original request.
struct Prepared<P: Scalar> {
    /// Complete detached output, installed only after the last fallible operation.
    core: Box<ApplicationCore<P>>,
    /// Prepared descriptor occupations.
    descriptors: PreparedSlots<DescriptorRecord>,
    /// Reserved native layout occupation.
    layouts: PreparedSlots<Native<LayoutRecord>>,
    /// Reserved native mask occupation.
    masks: PreparedSlots<Native<MaskRecord>>,
    /// Exact individual allocation records.
    debits: Vec<Debit>,
    /// Disjoint fixed debit destinations.
    debit_slots: [usize; 17],
    /// Initial result-pin index reservation.
    pin_slot: usize,
    /// Admission revision with terminal headroom.
    revision: u64,
}
/// Retain the complete request until every fallible preparation has succeeded.
pub(super) fn request<P: Scalar + 'static>(
    heap: &mut Heap,
    request: PointwiseRequest<P>,
    account: &mut ExecutionAccount,
) -> ApplicationRequest<P> {
    let checked = account
        .ledger
        .with_work_phase(super::FAMILY, "pointwise admission", |budget| {
            super::contract::check(heap, &request, account.identity, budget)?;
            let plan = plan::Plan::new(heap, &request, budget)?;
            preparation::prepare(heap, &request, plan).map_err(ApplicationError::from)
        });
    match checked {
        Ok(prepared) => {
            ApplicationRequest::ApplicationReady(install(heap, prepared.finish(request)))
        }
        Err(error) => ApplicationRequest::ApplicationRefused(error, request),
    }
}
/// Install only complete private records; no fallible work or allocation follows the input move.
fn install<P: Scalar>(heap: &mut Heap, prepared: Prepared<P>) -> OwnedApplication<P> {
    use crate::loan::registry::{DebitInventory, Group, PinIndex, ProducerOutcome, State};
    let identity = prepared.core.host.identity;
    heap.descriptors.slots.publish(prepared.descriptors);
    heap.masks.layouts.publish(prepared.layouts);
    heap.masks.masks.publish(prepared.masks);
    while heap.loans.groups.len() < heap.descriptors.slots.scan_work() {
        heap.loans.groups.push(None);
    }
    let mut inventory = [None; 17];
    for (index, debit) in prepared.debits.into_iter().enumerate() {
        let slot = prepared.debit_slots[index];
        inventory[index] = Some((slot, debit.key));
        heap.loans.install(slot, debit);
    }
    heap.loans.groups[identity.root.slot] = Some(Group {
        identity,
        state: State::Out(Rc::downgrade(&prepared.core.host.witness)),
        outcome: ProducerOutcome::Pending,
        delivered: false,
        pin: prepared.pin_slot,
        debits: DebitInventory::Application(inventory),
    });
    let pin = &prepared.core.host.headers[4];
    heap.loans.install_pin(
        prepared.pin_slot,
        PinIndex {
            identity: pin.key,
            witness: None,
            debit: (pin.slot, pin.key),
        },
    );
    heap.loans.pending += 1;
    heap.loans.revision = prepared.revision;
    OwnedApplication {
        core: prepared.core,
    }
}
