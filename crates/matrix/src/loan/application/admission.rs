// SPDX-License-Identifier: AGPL-3.0-only
//! The complete closed application inventory precedes every private allocation.

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
    debit_slots: [usize; 23],
    /// Initial result-pin index reservation.
    pin_slot: usize,
    /// Admission revision with terminal headroom.
    revision: u64,
}
/// Existing host routes retain their exact admission policy and charged prefix.
pub(super) fn request<P: Scalar + 'static>(
    heap: &mut Heap,
    request: PointwiseRequest<P>,
    function: crate::Elementwise,
    account: &mut ExecutionAccount,
) -> ApplicationRequest<P> {
    request_with_origin(heap, request, function, super::chain::Input::Host, account)
}
/// Retain the complete request until every fallible preparation has succeeded.
pub(super) fn request_with_origin<P: Scalar + 'static>(
    heap: &mut Heap,
    request: PointwiseRequest<P>,
    function: crate::Elementwise,
    origin: super::chain::Input,
    account: &mut ExecutionAccount,
) -> ApplicationRequest<P> {
    request_kind(
        heap,
        request,
        crate::KernelKind::Elementwise(function),
        origin,
        account,
    )
}
/// Closed mapped movement shares preparation without changing the original unary tariffs.
pub(super) fn request_kind<P: Scalar + 'static>(
    heap: &mut Heap,
    request: PointwiseRequest<P>,
    expected: crate::KernelKind,
    origin: super::chain::Input,
    account: &mut ExecutionAccount,
) -> ApplicationRequest<P> {
    let checked = account
        .ledger
        .with_work_phase(super::FAMILY, "pointwise admission", |budget| {
            let mut borrowed = super::prepared::Request::from(&request);
            borrowed.expected = expected;
            super::contract::check(heap, &borrowed, account.identity, origin, budget)?;
            let plan = plan::Plan::new(heap, &borrowed, budget)?;
            preparation::prepare(heap, &borrowed, plan).map_err(ApplicationError::from)
        });
    match checked {
        Ok(prepared) => {
            ApplicationRequest::ApplicationReady(install(heap, prepared.finish(request)))
        }
        Err(error) => ApplicationRequest::ApplicationRefused(error, request),
    }
}
/// The original funded law use moves only after the common private preparation succeeds.
pub(super) fn request_expm1(
    heap: &mut Heap,
    request: super::PointwiseExpm1Request,
    account: &mut ExecutionAccount,
) -> super::Expm1ApplicationRequest {
    let checked =
        account
            .ledger
            .with_work_phase(super::FAMILY, "prepared application admission", |budget| {
                super::prepared::authenticate(&request, heap, account.identity, budget)?;
                let borrowed = super::prepared::Request::from(&request);
                super::contract::check(
                    heap,
                    &borrowed,
                    account.identity,
                    super::chain::Input::Host,
                    budget,
                )?;
                let plan = plan::Plan::new(heap, &borrowed, budget)?;
                crate::loan::retained_expm1::Attachment::prepare_validation(
                    &request.law,
                    request.operation.binding,
                    heap,
                    budget,
                )?;
                preparation::prepare(heap, &borrowed, plan).map_err(ApplicationError::from)
            });
    match checked {
        Ok(mut prepared) => {
            prepared.core.input = Some(request.input);
            prepared.core.host.retained_law = Some(
                crate::loan::retained_expm1::Attachment::prepared(request.law),
            );
            super::Expm1ApplicationRequest::ApplicationReady(install(heap, prepared))
        }
        Err(error) => super::Expm1ApplicationRequest::ApplicationRefused(error, request),
    }
}
/// Both pins move only after the same complete private preparation as unary members.
pub(super) fn request_binary(
    heap: &mut Heap,
    request: super::PointwiseBinaryRequest,
    expected: crate::BinaryElementwise,
    account: &mut ExecutionAccount,
) -> super::BinaryApplicationRequest {
    let checked =
        account
            .ledger
            .with_work_phase(super::FAMILY, "binary application admission", |budget| {
                let mut borrowed = super::prepared::Request::from(&request);
                borrowed.expected = crate::KernelKind::BinaryElementwise(expected);
                super::contract::check(
                    heap,
                    &borrowed,
                    account.identity,
                    super::chain::Input::Host,
                    budget,
                )?;
                let plan = plan::Plan::new(heap, &borrowed, budget)?;
                preparation::prepare(heap, &borrowed, plan).map_err(ApplicationError::from)
            });
    match checked {
        Ok(mut prepared) => {
            let super::binary::Auxiliary::Binary(side) =
                &mut prepared.core.evidence.endpoint_tokens
            else {
                return super::BinaryApplicationRequest::ApplicationRefused(
                    LoanError::WrongLoan.into(),
                    request,
                );
            };
            side.input = Some(request.right);
            prepared.core.input = Some(request.left);
            super::BinaryApplicationRequest::ApplicationReady(install(heap, prepared))
        }
        Err(error) => super::BinaryApplicationRequest::ApplicationRefused(error, request),
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
    let mut inventory = [None; 23];
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
        debits: if prepared.core.evidence.prepared.is_some() {
            DebitInventory::PreparedApplication(inventory)
        } else if prepared.core.evidence.endpoint_tokens.binary().is_some() {
            DebitInventory::BinaryApplication(std::array::from_fn(|index| inventory[index]))
        } else {
            DebitInventory::Application(std::array::from_fn(|index| inventory[index]))
        },
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

/// Isolate the identical complete internal plan from independently funded fixture setup.
#[cfg(test)]
pub(super) fn check_plan(
    heap: &Heap,
    request: &super::PointwiseExpm1Request,
    budget: &mut nrz_bindings::Budget,
) -> Result<(), LoanError> {
    plan::Plan::new(heap, &super::prepared::Request::from(request), budget).map(|_| ())
}
