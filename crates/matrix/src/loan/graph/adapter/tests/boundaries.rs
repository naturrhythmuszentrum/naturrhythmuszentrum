// SPDX-License-Identifier: AGPL-3.0-only
//! Independently enumerated one-input storage includes sixty-three vacant native/core slots.

use super::super::{
    admission::Refusal,
    gate::Gate,
    node::Node,
    owner::{Backing, Capacities, Owner},
};
use super::fixture::TestResult;
use crate::LoanError;
use crate::loan::graph::{
    sink,
    walk::{Error, Kind},
};
use crate::loan::lc_shape::tests::output::{Setup, setup};
use nrz_bindings::{
    Limits,
    readings::algebra::{FaultKind, limits::BudgetKind},
};
use nrz_scalar::Scalar;

/// The proposed input has one attempt; the vacant pool does not create extra sink records.
const CAPACITY: Capacities = Capacities {
    nodes: 64,
    edges: 0,
    scopes: 0,
    drive: 100,
    records: 3,
    words: 19,
    witnesses: 0,
};

/// Known O ABI: shared376 + nodes64*136 + ready8 + stack64*16 + marks64*8 = 10,624.
/// Known K ABI: shared144 + records3*88 + evidence19*8 + account registration24 = 584.
/// Adapter header: two vector headers48, six pointer/identity/size words48, two pointers16, cleanup32.
/// Its gate contains graph8, persistent overlap8, ceilings16, optional exact byte record32, three bools and alignment padding5.
pub(super) fn inventory<P: Scalar>() -> (usize, usize) {
    assert_eq!(size_of::<Backing<'_, P, 5>>(), 144);
    assert_eq!(size_of::<Gate>(), 72);
    assert_eq!(size_of::<Kind>(), 1);
    assert_eq!(size_of::<sink::admission::Node>(), 48);
    // Native variants have their actual owning-layer ABI; they occupy one inline slot, not extra buffers.
    let native = size_of::<Node<'_, P, 5>>();
    let retained = 10_624 + 584 + 144 + 16 + 72 + 64 * (native + 8);
    // Caller proposal buffer, private kind buffer, and private destination buffer coexist.
    (retained, native + 1 + 48)
}

/// The native baseline is observed through its pre-existing owning interface, not the adapter planner.
pub(super) fn run<P: Scalar + 'static>() -> TestResult {
    let setup = setup::<P>(Limits::default(), [2, 3, 2])?;
    let native = setup.heap.resource_bytes()?;
    drop(setup);
    let (owned, temporary) = inventory::<P>();
    let retained = u64::try_from(native + owned)?;
    let staged = u64::try_from(native + owned + temporary)?;
    for (dimension, bound) in [
        (BudgetKind::PayloadBytes, retained),
        (BudgetKind::StagedPayloadBytes, staged),
    ] {
        for short in [false, true] {
            boundary::<P>(dimension, bound, short, native)?;
        }
    }
    Ok(())
}

/// Every boundary starts a new immutable original ledger and completes setup under that ceiling.
fn boundary<P: Scalar + 'static>(
    dimension: BudgetKind,
    bound: u64,
    short: bool,
    native: usize,
) -> TestResult {
    let mut limits = Limits::default();
    if dimension == BudgetKind::PayloadBytes {
        limits.payload_bytes = bound - u64::from(short);
    } else {
        limits.staged_payload_bytes = bound - u64::from(short);
    }
    let Setup {
        heap,
        mut account,
        inventory,
        axes,
        registry,
        ..
    } = setup::<P>(limits, [2, 3, 2])?;
    let declaration = inventory
        .input(0)
        .ok_or("original source")?
        .source(&axes, &mut account)?;
    assert_eq!(heap.resource_bytes()?, native);
    crate::loan::charge(&mut account.ledger, 13)?;
    let before = account.usage().work;
    let proposal = vec![Node::input(declaration)];
    let original = proposal.as_ptr();
    let result = Owner::admit(
        proposal,
        &[1],
        CAPACITY,
        &inventory,
        &registry,
        &[],
        &heap,
        &mut account,
    );
    if short {
        let Err(Refusal {
            nodes,
            error: Error::Native(LoanError::Budget(error)),
        }) = result
        else {
            return Err("adapter byte boundary".into());
        };
        assert_eq!(nodes.as_ptr(), original);
        let FaultKind::Exhausted {
            budget,
            needed,
            limit,
            ..
        } = error.kind
        else {
            return Err("original exhausted byte dimension".into());
        };
        assert_eq!(budget, dimension);
        assert_eq!((needed, limit), (bound, bound - 1));
        assert_eq!(sink::live_bytes(&account)?, 0);
    } else {
        let owner = result.map_err(|Refusal { error, .. }| error)?;
        assert_eq!(owner.backing.nodes.capacity(), 64);
        assert_eq!(owner.backing.nodes.len(), 1);
        assert_eq!(owner.backing.boundaries.capacity(), 64);
        assert_eq!(owner.backing.core.reservation.backing.nodes.len(), 64);
        drop(owner);
    }
    assert!(account.usage().work > before);
    Ok(())
}
