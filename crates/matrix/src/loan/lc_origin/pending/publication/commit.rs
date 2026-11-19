// SPDX-License-Identifier: AGPL-3.0-only
//! Every destination and unique shell was checked under the same exclusive heap borrow.
use super::super::super::{LcSeal, LcUsePayload};
use super::{super::LcApplicationPending, LcPublishedResult};
use crate::Heap;
use crate::loan::registry::{
    AllocationKind, LoanTicket, ProducerOutcome, PublicationReceipt, State,
};
use crate::mask::storage::Native;
use nrz_scalar::Scalar;
use std::{marker::PhantomData, rc::Rc};

/// Move the validated private owners into their already allocated destinations.
pub(super) fn publish<P: Scalar, const N: usize>(
    owner: &mut LcApplicationPending<'_, P, N>,
    heap: &mut Heap,
    seal: &LcSeal,
    revision: u64,
) -> LcPublishedResult<P> {
    let super::super::Native {
        mut bundle,
        mut initial_use,
        mut storage,
    } = owner.native.take().expect("validated native owner");
    let identity = storage.identity;
    // Five new owning/weak edges have separately prepaid destruction at publication.
    storage.cleanup.edges += 5;
    let use_token = storage
        .debits
        .initial_use
        .take()
        .expect("validated initial use token");
    let bundle_slot = storage
        .debits
        .bundle
        .as_ref()
        .expect("validated bundle token")
        .slot;
    let use_slot = use_token.slot;
    *heap
        .masks
        .layouts
        .get_mut(identity.layout.key)
        .expect("validated output layout") = Native::Published {
        owner: Rc::clone(&storage.layout),
        managed: true,
    };
    *heap
        .masks
        .masks
        .get_mut(identity.mask.key)
        .expect("validated filled mask") = Native::Published {
        owner: Rc::clone(&storage.completed),
        managed: true,
    };
    for key in identity.channels.into_iter().flatten() {
        let crate::heap::descriptors::DescriptorRecord::Managed(record) = heap
            .descriptors
            .slots
            .get_mut(key)
            .expect("validated descriptor")
        else {
            unreachable!("validated managed descriptor");
        };
        record.filled = true;
    }
    let shell = Rc::get_mut(&mut bundle).expect("validated unique bundle shell");
    shell.seal = Some(seal.clone());
    shell.storage = Some(storage);
    Rc::get_mut(&mut initial_use)
        .expect("validated unique use shell")
        .payload = Some(LcUsePayload {
        identity: use_token.key,
        account: identity.account,
        bundle: Rc::clone(&bundle),
        token: use_token,
    });
    heap.loans.debits[bundle_slot]
        .as_mut()
        .expect("validated bundle debit")
        .kind = AllocationKind::LcBundle(Rc::downgrade(&bundle));
    heap.loans.debits[use_slot]
        .as_mut()
        .expect("validated use debit")
        .kind = AllocationKind::LcUse {
        witness: Rc::downgrade(&initial_use),
        receipt: None,
    };
    let group = heap.loans.groups[identity.root.slot]
        .as_mut()
        .expect("validated output group");
    group.state = State::LcPublished(bundle);
    group.outcome = ProducerOutcome::Published(PublicationReceipt {
        ticket: LoanTicket { identity },
        present: [true; 3],
    });
    group.delivered = true;
    heap.loans.pending -= 1;
    heap.loans.revision = revision;
    LcPublishedResult {
        record: initial_use,
        marker: PhantomData,
    }
}
