// SPDX-License-Identifier: AGPL-3.0-only
//! Admission finishes every allocation and cleanup prepayment before installing Out.

mod plan;
use super::{HostCore, OwnedLoan};
use crate::loan::{
    ExecutionAccount, HostChannel, HostTileDeclaration, LoanError,
    payload::{LoanChannel, Numerical},
    preparation::Preparation,
    registry::{
        AllocationKind, BundleOwner, Debit, Declaration, Group, Identity, Liveness,
        ManagedDescriptor, PinRecord, ProducerOutcome, State, Token,
    },
    vector,
};
use crate::{
    CellLayoutVersion, Heap, HeapError, Layout, MaskVersion,
    heap::{
        descriptors::DescriptorRecord,
        slots::{PreparedSlots, SlotPlan},
    },
    mask::storage::{LayoutRecord, MaskRecord, Native},
    storage::Format,
};
use nrz_scalar::Scalar;
use plan::Plan;
use std::{marker::PhantomData, rc::Rc};

/// Complete private owners and records precede the single no-fail installation.
struct Prepared {
    /// Sole boxed core is allocated before any occupation becomes observable.
    core: Box<HostCore>,
    /// Complete descriptor occupations.
    descriptors: PreparedSlots<DescriptorRecord>,
    /// Privately reserved native layout occupation.
    layouts: PreparedSlots<Native<LayoutRecord>>,
    /// Privately reserved native mask occupation.
    masks: PreparedSlots<Native<MaskRecord>>,
    /// Exact reservation index records, without another numerical owner.
    debits: Vec<Debit>,
    /// Prechecked vacant or appended debit positions.
    debit_slots: [usize; 15],
    /// Initial weak-pin row is occupied while the loan is out.
    pin_slot: usize,
    /// Mutation revision with the pending terminal headroom preserved.
    revision: u64,
}
/// Track replacement-buffer acquisition before transferring that allocation to the heap.
pub(in crate::loan) fn grow_tracked<T>(
    values: &mut Vec<T>,
    capacity: usize,
    guard: &mut Preparation,
) -> Result<(), LoanError> {
    if capacity <= values.capacity() {
        return Ok(());
    }
    let mut replacement = vector(capacity)?;
    guard.acquired(true)?;
    replacement.append(values);
    *values = replacement;
    guard.retain_table();
    Ok(())
}
/// Retained growth and simultaneous old-buffer residence for an exact replacement.
pub(in crate::loan) fn growth<T>(
    values: &Vec<T>,
    requested: usize,
) -> Result<(usize, usize), LoanError> {
    let old = values
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(LoanError::ByteOverflow)?;
    let new = requested
        .max(values.capacity())
        .checked_mul(size_of::<T>())
        .ok_or(LoanError::ByteOverflow)?;
    Ok((new - old, if new > old { old } else { 0 }))
}
/// Reserve one exact allocation identity and its non-owning accounting record.
fn token(
    identity: Identity,
    kind: AllocationKind,
    elements: usize,
    bytes: usize,
    debits: &mut Vec<Debit>,
    slots: &[usize; 15],
) -> Result<Token, LoanError> {
    let key = crate::heap::identity::identity()?;
    let slot = slots[debits.len()];
    debits.push(Debit {
        kind,
        owner: identity,
        key,
        elements,
        bytes,
    });
    Ok(Token {
        slot,
        key,
        receipt: identity.receipt,
    })
}
/// Complete independently checked admission on the original account.
pub(super) fn request<P: Scalar + 'static>(
    heap: &mut Heap,
    declaration: HostTileDeclaration<P>,
    account: &mut ExecutionAccount,
) -> Result<OwnedLoan<P>, LoanError> {
    account.authenticate(declaration.account)?;
    if declaration.axis.scope != heap.scope {
        return Err(LoanError::ForeignOwner);
    }
    account
        .ledger
        .with_work_phase(crate::loan::FAMILY, "loan admission", |budget| {
            let plan = Plan::new(heap, &declaration, budget)?;
            let prepared = prepare(heap, declaration, plan)?;
            Ok(install(heap, prepared))
        })
}
/// Allocate every private owner after the complete simultaneous preflight and cleanup prepayment.
fn prepare<P: Scalar + 'static>(
    heap: &mut Heap,
    declaration: HostTileDeclaration<P>,
    mut plan: Plan,
) -> Result<Prepared, LoanError> {
    let receipt_identity = plan.cleanup.identity;
    let receipt = std::mem::replace(
        &mut plan.cleanup,
        crate::loan::registry::CleanupReceipt::empty(receipt_identity),
    );
    let mut guard = Preparation::new(receipt);
    grow_tracked(&mut heap.loans.groups, plan.group_capacity, &mut guard)?;
    grow_tracked(&mut heap.loans.debits, plan.debit_capacity, &mut guard)?;
    grow_tracked(&mut heap.loans.pins, plan.pin_capacity, &mut guard)?;
    let descriptors = heap.descriptors.slots.change_tracked(
        plan.channels,
        heap.limits.descriptors,
        &mut guard,
    )?;
    let layouts = heap
        .masks
        .layouts
        .change_tracked(1, heap.limits.descriptors, &mut guard)?;
    let masks = heap
        .masks
        .masks
        .change_tracked(1, heap.limits.descriptors, &mut guard)?;
    let identity = identity(
        heap.scope,
        &declaration,
        &plan,
        &descriptors,
        &layouts,
        &masks,
    )?;
    let mut debits = vector(plan.debit_count)?;
    guard.acquired(true)?;
    let channels = channels::<P>(&plan, identity, &mut debits, &mut guard)?;
    let headers = header_tokens(identity, &plan, &mut debits)?;
    let Shells {
        layout,
        bundle,
        pin,
        witness,
    } = shells(&declaration, identity, &mut guard)?;
    let mut records = vector(plan.channels)?;
    guard.acquired(true)?;
    for (channel, present) in HostChannel::ALL.into_iter().zip(plan.present) {
        if present {
            records.push(DescriptorRecord::Managed(ManagedDescriptor {
                identity,
                channel,
                filled: false,
            }));
        }
    }
    let mut layout_records = vector(1)?;
    guard.acquired(true)?;
    layout_records.push(Native::Reserved(identity.root));
    let mut mask_records = vector(1)?;
    guard.acquired(true)?;
    mask_records.push(Native::Reserved(identity.root));
    let descriptors = descriptors.bind(records)?;
    let layouts = layouts.bind(layout_records)?;
    let masks = masks.bind(mask_records)?;
    let declaration = Declaration {
        symbols: declaration.symbols,
        axis: declaration.axis,
        extent: declaration.extent,
        coverage: declaration.coverage,
    };
    #[cfg(test)]
    crate::loan::fault::before()?;
    let mut core = Box::new(HostCore {
        identity,
        declaration,
        channels,
        layout,
        bundle,
        pin,
        headers,
        cleanup: crate::loan::registry::CleanupReceipt::empty(receipt_identity),
        witness,
    });
    guard.acquired(false)?;
    core.cleanup = guard.finish()?;
    Ok(Prepared {
        core,
        descriptors,
        layouts,
        masks,
        debits,
        debit_slots: plan.debit_slots,
        pin_slot: plan.pin_slot,
        revision: plan.revision,
    })
}
/// Fixed identity incidence binds every reserved member before a liveness witness is constructed.
fn identity<P: Scalar + 'static>(
    scope: u64,
    declaration: &HostTileDeclaration<P>,
    plan: &Plan,
    descriptors: &SlotPlan,
    layouts: &SlotPlan,
    masks: &SlotPlan,
) -> Result<Identity, LoanError> {
    let root = *descriptors.keys().first().ok_or(HeapError::Capacity)?;
    let layout = CellLayoutVersion {
        scope,
        key: *layouts.keys().first().ok_or(HeapError::Capacity)?,
    };
    let mask = MaskVersion {
        scope,
        key: *masks.keys().first().ok_or(HeapError::Capacity)?,
        layout,
    };
    let mut keys = [None; 3];
    let mut positions = descriptors.keys().iter();
    for (key, present) in keys.iter_mut().zip(plan.present) {
        if present {
            *key = positions.next().copied();
        }
    }
    Ok(Identity {
        scope,
        account: declaration.account,
        root,
        channels: keys,
        layout,
        mask,
        request: crate::heap::identity::identity()?,
        nonce: crate::heap::identity::identity()?,
        declaration: declaration.identity,
        receipt: plan.cleanup.identity,
        format: Format::of::<P>(),
    })
}
/// Numerical and flag allocations remain individually owned throughout preparation.
fn channels<P: Scalar + 'static>(
    plan: &Plan,
    identity: Identity,
    debits: &mut Vec<Debit>,
    guard: &mut Preparation,
) -> Result<[Option<LoanChannel>; 3], LoanError> {
    let mut owners: [Option<LoanChannel>; 3] = [None, None, None];
    for (owner, present) in owners.iter_mut().zip(plan.present) {
        if present {
            let matrix = Numerical::zeroed::<P>(plan.length)?;
            if plan.length != 0 {
                guard.acquired(false)?;
            }
            let mut components = vector(plan.component_words)?;
            if plan.component_words != 0 {
                guard.acquired(false)?;
            }
            components.resize(plan.component_words, 0);
            let mut complete = vector(plan.cell_words)?;
            if plan.cell_words != 0 {
                guard.acquired(false)?;
            }
            complete.resize(plan.cell_words, 0);
            *owner = Some(LoanChannel {
                matrix,
                components,
                complete,
                tokens: [
                    token(
                        identity,
                        AllocationKind::Numerical,
                        plan.length,
                        plan.rounded,
                        debits,
                        &plan.debit_slots,
                    )?,
                    token(
                        identity,
                        AllocationKind::Tracking,
                        0,
                        plan.component_words * 8,
                        debits,
                        &plan.debit_slots,
                    )?,
                    token(
                        identity,
                        AllocationKind::Tracking,
                        0,
                        plan.cell_words * 8,
                        debits,
                        &plan.debit_slots,
                    )?,
                ],
            });
        }
    }
    Ok(owners)
}
/// No allocation, checked arithmetic, Work debit or caller callback follows this final move.
fn install<P: Scalar>(heap: &mut Heap, prepared: Prepared) -> OwnedLoan<P> {
    let identity = prepared.core.identity;
    heap.descriptors.slots.publish(prepared.descriptors);
    heap.masks.layouts.publish(prepared.layouts);
    heap.masks.masks.publish(prepared.masks);
    while heap.loans.groups.len() < heap.descriptors.slots.scan_work() {
        heap.loans.groups.push(None);
    }
    heap.loans.groups[identity.root.slot] = Some(Group {
        identity,
        state: State::Reserved,
        outcome: ProducerOutcome::Pending,
        delivered: false,
        pin: prepared.pin_slot,
        debits: crate::loan::registry::DebitInventory::Host([None; 15]),
    });
    for (index, debit) in prepared.debits.into_iter().enumerate() {
        let slot = prepared.debit_slots[index];
        if let Some(group) = heap.loans.groups[identity.root.slot].as_mut() {
            group.debits.entries_mut()[index] = Some((slot, debit.key));
        }
        heap.loans.install(slot, debit);
    }
    if let Some(group) = heap.loans.groups[identity.root.slot].as_mut() {
        group.state = State::Out(Rc::downgrade(&prepared.core.witness));
    }
    let pin_token = &prepared.core.headers[4];
    heap.loans.install_pin(
        prepared.pin_slot,
        crate::loan::registry::PinIndex {
            identity: pin_token.key,
            witness: None,
            debit: (pin_token.slot, pin_token.key),
        },
    );
    heap.loans.pending += 1;
    heap.loans.revision = prepared.revision;
    OwnedLoan {
        core: prepared.core,
        marker: PhantomData,
    }
}

/// All shared shells are constructed privately before any weak pointer escapes.
struct Shells {
    /// Shared immutable native layout.
    layout: Rc<LayoutRecord>,
    /// Empty complete-bundle destination.
    bundle: Rc<BundleOwner>,
    /// Empty initial reader destination.
    pin: Rc<PinRecord>,
    /// Final-field expiration witness.
    witness: Rc<Liveness>,
}
/// Record each acquired shell immediately, including the prefix before a later refusal.
/// Logical refusals and reserved capacities are typed outcomes. Fixed `Box`/`Rc` shells
/// use the workspace's stable allocation boundary; a real allocator abort is out of scope.
fn shells<P: Scalar>(
    declaration: &HostTileDeclaration<P>,
    identity: Identity,
    guard: &mut Preparation,
) -> Result<Shells, LoanError> {
    #[cfg(test)]
    crate::loan::fault::before()?;
    let layout = Rc::new(LayoutRecord {
        axis: declaration.axis,
        components: Layout::contiguous(declaration.extent)?,
        gather: None,
    });
    guard.acquired(false)?;
    #[cfg(test)]
    crate::loan::fault::before()?;
    let bundle = Rc::new(BundleOwner { payload: None });
    guard.acquired(false)?;
    #[cfg(test)]
    crate::loan::fault::before()?;
    let pin = Rc::new(PinRecord { payload: None });
    guard.acquired(false)?;
    #[cfg(test)]
    crate::loan::fault::before()?;
    let witness = Rc::new(Liveness {
        identity,
        receipt: identity.receipt,
    });
    guard.acquired(false)?;
    Ok(Shells {
        layout,
        bundle,
        pin,
        witness,
    })
}

/// Each private shell has one exact allocation debit before its owner can escape.
fn header_tokens(
    identity: Identity,
    plan: &Plan,
    debits: &mut Vec<Debit>,
) -> Result<[Token; 5], LoanError> {
    Ok([
        token(
            identity,
            AllocationKind::Owner,
            0,
            plan.headers[0],
            debits,
            &plan.debit_slots,
        )?,
        token(
            identity,
            AllocationKind::Owner,
            0,
            plan.headers[1],
            debits,
            &plan.debit_slots,
        )?,
        token(
            identity,
            AllocationKind::Owner,
            0,
            plan.headers[2],
            debits,
            &plan.debit_slots,
        )?,
        token(
            identity,
            AllocationKind::Owner,
            0,
            plan.headers[3],
            debits,
            &plan.debit_slots,
        )?,
        token(
            identity,
            AllocationKind::Pin,
            0,
            plan.headers[4],
            debits,
            &plan.debit_slots,
        )?,
    ])
}
