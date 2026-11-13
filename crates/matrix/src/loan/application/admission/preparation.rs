// SPDX-License-Identifier: AGPL-3.0-only
//! Fallible private preparation leaves the exact input request outside its acquired prefix.

use super::super::{ApplicationCore, CellOutcome, Evidence, PointwiseRequest};
use super::{Prepared, plan::Plan};
use crate::loan::lifecycle::admission::grow_tracked;
use crate::loan::{
    HostChannel, HostTileDeclaration, LoanError,
    payload::{LoanChannel, Numerical},
    preparation::Preparation,
    registry::{
        AllocationKind, BundleOwner, Debit, Declaration, Identity, Liveness, ManagedDescriptor,
        PinRecord, Token,
    },
    vector,
};
use crate::{
    CellLayoutVersion, Heap, HeapError, Layout, MaskVersion,
    heap::{descriptors::DescriptorRecord, slots::SlotPlan},
    mask::storage::{LayoutRecord, Native},
    storage::Format,
};
use nrz_scalar::Scalar;
use std::rc::Rc;

impl<P: Scalar> Prepared<P> {
    /// Last ownership move follows all checks, allocations and receipt preparation.
    pub(super) fn finish(mut self, request: PointwiseRequest<P>) -> Self {
        self.core.input = Some(request.input);
        self
    }
}
/// Complete all owned output allocations while borrowing the original request.
pub(super) fn prepare<P: Scalar + 'static>(
    heap: &mut Heap,
    request: &super::super::prepared::Request<'_, P>,
    mut plan: Plan,
) -> Result<Prepared<P>, LoanError> {
    plan.permit.storage(heap.storage.epoch())?;
    if plan.permit.scope != heap.scope || plan.permit.account != Some(request.destination.account) {
        return Err(LoanError::ForeignOwner);
    }
    let receipt_identity = plan.cleanup.identity;
    let receipt = std::mem::replace(
        &mut plan.cleanup,
        crate::loan::registry::CleanupReceipt::empty(receipt_identity),
    );
    let mut guard = Preparation::new(receipt);
    grow_tracked(&mut heap.loans.groups, plan.group_capacity, &mut guard)?;
    grow_tracked(&mut heap.loans.debits, plan.debit_capacity, &mut guard)?;
    grow_tracked(&mut heap.loans.pins, plan.pin_capacity, &mut guard)?;
    let descriptors =
        heap.descriptors
            .slots
            .change_tracked(3, heap.limits.descriptors, &mut guard)?;
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
        request.destination,
        &plan,
        &descriptors,
        &layouts,
        &masks,
    )?;
    let mut debits = vector(plan.debit_count)?;
    guard.acquired(true)?;
    let reading_channels =
        channels::<P, 3>(&plan, plan.present, identity, &mut debits, &mut guard)?;
    let endpoints = endpoint_channels(
        &plan,
        request.member()?.is_prepared(),
        identity,
        &mut debits,
        &mut guard,
    )?;
    let headers = header_tokens(identity, &plan, &mut debits)?;
    let shells = shells(request.destination, identity, &mut guard)?;
    let endpoint_tokens = endpoints.as_ref().map(endpoint_identities);
    let extra = extras(
        heap,
        request,
        &plan,
        identity,
        &mut debits,
        &mut guard,
        endpoint_tokens,
    )?;
    let Records {
        descriptors,
        layouts,
        masks,
    } = records(identity, descriptors, layouts, masks, &mut guard)?;
    let declaration = declaration(request.destination);
    #[cfg(test)]
    crate::loan::fault::before()?;
    let mut core = Box::new(ApplicationCore {
        endpoints,
        input: None,
        contract: extra.contract,
        extent_token: extra.extent_token,
        filled: extra.filled,
        filled_token: extra.filled_token,
        evidence: extra.evidence,
        population: extra.population,
        candidate: false,
        host: crate::loan::lifecycle::HostCore {
            retained_law: None,
            identity,
            declaration,
            channels: reading_channels,
            layout: shells.layout,
            bundle: shells.bundle,
            pin: shells.pin,
            headers,
            cleanup: crate::loan::registry::CleanupReceipt::empty(receipt_identity),
            witness: shells.witness,
        },
    });
    guard.acquired(false)?;
    core.host.cleanup = guard.finish()?;
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
/// Fixed Lower/Upper identity order is captured before publication moves any token.
fn endpoint_identities(pair: &[LoanChannel; 2]) -> [(usize, u64, u64); 2] {
    pair.each_ref().map(|channel| {
        let token = &channel.tokens[0];
        (token.slot, token.key, token.receipt)
    })
}
/// Common prepared occupations retain their existing slot-table ownership.
struct Records {
    /// Three ordinary reading descriptors.
    descriptors: crate::heap::slots::PreparedSlots<DescriptorRecord>,
    /// One identity layout.
    layouts: crate::heap::slots::PreparedSlots<Native<LayoutRecord>>,
    /// One shared completed mask.
    masks: crate::heap::slots::PreparedSlots<Native<crate::mask::storage::MaskRecord>>,
}
/// Bind the three descriptor occupations and the two reserved native records without new authority.
fn records(
    identity: Identity,
    descriptors: SlotPlan,
    layouts: SlotPlan,
    masks: SlotPlan,
    guard: &mut Preparation,
) -> Result<Records, LoanError> {
    let mut records = vector(3)?;
    guard.acquired(true)?;
    for channel in HostChannel::ALL {
        records.push(DescriptorRecord::Managed(ManagedDescriptor {
            identity,
            channel,
            filled: false,
        }));
    }
    let mut layout_records = vector(1)?;
    guard.acquired(true)?;
    layout_records.push(Native::Reserved(identity.root));
    let mut mask_records = vector(1)?;
    guard.acquired(true)?;
    mask_records.push(Native::Reserved(identity.root));
    Ok(Records {
        descriptors: descriptors.bind(records)?,
        layouts: layouts.bind(layout_records)?,
        masks: masks.bind(mask_records)?,
    })
}
/// Endpoint payload and tracking owners use the same disjoint allocation preparation as readings.
fn endpoint_channels(
    plan: &Plan,
    prepared: bool,
    identity: Identity,
    debits: &mut Vec<Debit>,
    guard: &mut Preparation,
) -> Result<Option<[LoanChannel; 2]>, LoanError> {
    if !prepared {
        return Ok(None);
    }
    let [Some(lower), Some(upper)] = channels::<f64, 2>(plan, [true; 2], identity, debits, guard)?
    else {
        return Err(LoanError::WrongLoan);
    };
    Ok(Some([lower, upper]))
}
/// The application has precisely three additional separately owned allocations.
struct Extras {
    /// Captured available selection, later reduced to completed coverage.
    filled: Vec<u64>,
    /// Complete immutable metadata and outcome backing.
    evidence: Evidence,
    /// Checked one-cell numerical law.
    contract: super::super::member::CellContract,
    /// Unique prospective filled backing token.
    filled_token: Token,
    /// Unique per-cell extent owner token.
    extent_token: Token,
    /// Independently scanned population of captured A.
    population: usize,
}
/// Construct A and D classifications without retaining the ordinary demand owner.
fn extras<P: Scalar + 'static>(
    heap: &Heap,
    request: &super::super::prepared::Request<'_, P>,
    plan: &Plan,
    identity: Identity,
    debits: &mut Vec<Debit>,
    guard: &mut Preparation,
    endpoint_tokens: Option<[(usize, u64, u64); 2]>,
) -> Result<Extras, LoanError> {
    let source = super::super::contract::source(request)?;
    let member = request.member()?;
    let demand_layout = if member.is_projected() {
        request.demand.layout()
    } else {
        source.identity.layout
    };
    let demand = heap.mask(request.demand, demand_layout)?;
    let mut filled = vector(plan.cell_words)?;
    if plan.cell_words != 0 {
        guard.acquired(false)?;
    }
    filled.resize(plan.cell_words, 0);
    fill_availability(member, request, source, &demand, &mut filled)?;
    let population = filled.iter().try_fold(0_usize, |population, word| {
        population
            .checked_add(word.count_ones() as usize)
            .ok_or(LoanError::CountOverflow)
    })?;
    let outcomes = outcomes(
        request.destination.capacity(),
        &filled,
        demand.words(),
        guard,
    )?;
    let contract = cell_contract(request, guard)?;
    let filled_token = token(
        identity,
        AllocationKind::Filled,
        0,
        plan.extras[0],
        debits,
        &plan.debit_slots,
        &plan.identities,
    )?;
    let outcome_token = token(
        identity,
        AllocationKind::Owner,
        0,
        plan.extras[1],
        debits,
        &plan.debit_slots,
        &plan.identities,
    )?;
    let extent_token = token(
        identity,
        AllocationKind::Owner,
        0,
        plan.extras[2],
        debits,
        &plan.debit_slots,
        &plan.identities,
    )?;
    let auxiliary = auxiliary(request, plan, identity, debits, guard, endpoint_tokens)?;
    let evidence = Evidence {
        error_scope: request.member()?.error_scope(source.origin.error_scope()),
        prepared: request.prepared.cloned(),
        endpoint_tokens: auxiliary,
        completed_mask: None,
        operation: request.operation.clone(),
        excluded_inputs: Rc::clone(&request.operation.inputs),
        excluded_outputs: Rc::clone(&request.operation.outputs),
        law: Rc::clone(request.member()?.law()),
        source: Declaration {
            symbols: source.declaration.symbols.clone(),
            axis: source.declaration.axis,
            extent: source.declaration.extent,
            coverage: source.declaration.coverage,
        },
        source_identity: source.identity,
        attachments: source.qualified,
        demand: request.demand,
        destination: identity,
        revision: 0,
        outcomes,
        token: outcome_token,
    };
    Ok(Extras {
        filled,
        evidence,
        contract,
        filled_token,
        extent_token,
        population,
    })
}
/// The original word intersection and mapped cell incidence share the same prepaid owner.
fn fill_availability<P: Scalar>(
    member: super::super::member::Member<'_>,
    request: &super::super::prepared::Request<'_, P>,
    source: &crate::loan::registry::Bundle,
    demand: &crate::MaskView<'_>,
    filled: &mut [u64],
) -> Result<(), LoanError> {
    if member.is_projected() {
        super::super::movement::filled(member.map()?, source, demand, filled)?;
    } else {
        for (index, word) in filled.iter_mut().enumerate() {
            *word = demand.words()[index];
            for _ in source.channels.iter().flatten() {
                *word &= source.mask.words[index];
            }
            if let Some((_, pin)) = request.binary {
                let right = super::super::binary::bundle(pin)?;
                for _ in right.channels.iter().flatten() {
                    *word &= right.mask.words[index];
                }
            }
        }
    }
    Ok(())
}

/// Mutually exclusive prepared endpoints or the single inventoried binary side box.
fn auxiliary<P: Scalar>(
    request: &super::super::prepared::Request<'_, P>,
    plan: &Plan,
    identity: Identity,
    debits: &mut Vec<Debit>,
    guard: &mut Preparation,
    endpoints: Option<[(usize, u64, u64); 2]>,
) -> Result<super::super::binary::Auxiliary, LoanError> {
    use super::super::binary::{Auxiliary, Side};
    if request.member()?.is_projected() {
        return Ok(Auxiliary::Movement {
            layout: request.demand.layout(),
            axis: request.destination.axis,
        });
    }
    let Some((original, pin)) = request.binary else {
        return Ok(endpoints.map_or(Auxiliary::Absent, Auxiliary::Endpoints));
    };
    if endpoints.is_some() || request.prepared.is_some() {
        return Err(LoanError::WrongLoan);
    }
    let source = super::super::binary::bundle(pin)?;
    let token = token(
        identity,
        AllocationKind::Owner,
        0,
        plan.side_bytes,
        debits,
        &plan.debit_slots,
        &plan.identities,
    )?;
    #[cfg(test)]
    crate::loan::fault::before()?;
    let side = Box::new(Side {
        original: original.clone(),
        source: Declaration {
            symbols: source.declaration.symbols.clone(),
            axis: source.declaration.axis,
            extent: source.declaration.extent,
            coverage: source.declaration.coverage,
        },
        identity: source.identity,
        attachments: source.qualified,
        input: None,
        token,
    });
    guard.acquired(false)?;
    Ok(Auxiliary::Binary(side))
}

/// The one-cell extent and closed allowance use the same separately owned numerical contract.
fn cell_contract<P: Scalar>(
    request: &super::super::prepared::Request<'_, P>,
    guard: &mut Preparation,
) -> Result<super::super::member::CellContract, LoanError> {
    let extent = crate::Extent::new(1, request.destination.components())?;
    #[cfg(test)]
    crate::loan::fault::before()?;
    let inputs: Rc<[crate::Extent]> = if request.binary.is_some() {
        Rc::new([extent; 2])
    } else {
        Rc::new([extent])
    };
    guard.acquired(false)?;
    Ok(super::super::member::CellContract {
        inputs,
        output: extent,
        kind: request.member()?.law().kind(),
        work: request.member()?.invocation(extent.entries())?,
    })
}
/// The fixed outcome array preserves D and A independently of caller mask retirement.
fn outcomes(
    cells: usize,
    filled: &[u64],
    demand: &[u64],
    guard: &mut Preparation,
) -> Result<Vec<CellOutcome>, LoanError> {
    let mut outcomes = vector(cells)?;
    if cells != 0 {
        guard.acquired(false)?;
    }
    for cell in 0..cells {
        outcomes.push(if crate::loan::payload::bit(filled, cell) {
            CellOutcome::Pending
        } else if crate::loan::payload::bit(demand, cell) {
            CellOutcome::MissingInput
        } else {
            CellOutcome::NotRequested
        });
    }
    Ok(outcomes)
}
/// Shared symbolic declaration owners add no numerical allocation or retaining edge.
fn declaration<P: Scalar>(declaration: &HostTileDeclaration<P>) -> Declaration {
    Declaration {
        symbols: declaration.symbols.clone(),
        axis: declaration.axis,
        extent: declaration.extent,
        coverage: declaration.coverage,
    }
}
/// Reserve one exact allocation identity and its non-owning accounting record.
fn token(
    identity: Identity,
    kind: AllocationKind,
    elements: usize,
    bytes: usize,
    debits: &mut Vec<Debit>,
    slots: &[usize; 23],
    identities: &[u64; 26],
) -> Result<Token, LoanError> {
    let key = *identities
        .get(3 + debits.len())
        .ok_or(LoanError::WrongLoan)?;
    let slot = *slots.get(debits.len()).ok_or(LoanError::WrongLoan)?;
    debits.push(Debit {
        kind,
        owner: crate::loan::registry::DebitOwner::Bundle(identity),
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
        request: plan.identities[1],
        nonce: plan.identities[2],
        declaration: declaration.identity,
        receipt: plan.cleanup.identity,
        format: Format::of::<P>(),
    })
}
/// Numerical and flag allocations remain individually owned throughout preparation.
fn channels<P: Scalar + 'static, const N: usize>(
    plan: &Plan,
    present: [bool; N],
    identity: Identity,
    debits: &mut Vec<Debit>,
    guard: &mut Preparation,
) -> Result<[Option<LoanChannel>; N], LoanError> {
    let mut owners: [Option<LoanChannel>; N] = std::array::from_fn(|_| None);
    for (owner, present) in owners.iter_mut().zip(present) {
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
                        &plan.identities,
                    )?,
                    token(
                        identity,
                        AllocationKind::Tracking,
                        0,
                        plan.component_words * 8,
                        debits,
                        &plan.debit_slots,
                        &plan.identities,
                    )?,
                    token(
                        identity,
                        AllocationKind::Tracking,
                        0,
                        plan.cell_words * 8,
                        debits,
                        &plan.debit_slots,
                        &plan.identities,
                    )?,
                ],
            });
        }
    }
    Ok(owners)
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
            &plan.identities,
        )?,
        token(
            identity,
            AllocationKind::Owner,
            0,
            plan.headers[1],
            debits,
            &plan.debit_slots,
            &plan.identities,
        )?,
        token(
            identity,
            AllocationKind::Owner,
            0,
            plan.headers[2],
            debits,
            &plan.debit_slots,
            &plan.identities,
        )?,
        token(
            identity,
            AllocationKind::Owner,
            0,
            plan.headers[3],
            debits,
            &plan.debit_slots,
            &plan.identities,
        )?,
        token(
            identity,
            AllocationKind::Pin,
            0,
            plan.headers[4],
            debits,
            &plan.debit_slots,
            &plan.identities,
        )?,
    ])
}
