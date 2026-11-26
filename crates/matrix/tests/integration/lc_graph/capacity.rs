// SPDX-License-Identifier: AGPL-3.0-only
//! Public simultaneous owners fill a caller's byte ceiling without changing any limit.
use super::fixture::Result;
use nrz_bindings::Registry;
use nrz_matrix::*;
use nrz_scalar::Scalar;

/// The fan-out caller selects a ceiling below the original default and never changes it.
pub(super) const LIMIT: u64 = 100_000;

/// Empty O376, adapter144, observer88 and K144 precede the selected witness words.
/// The caller supplies independently enumerated occupied graph bytes; native bytes come from
/// the pre-existing heap residence interface, never from the adapter being checked.
pub(super) fn fill<'i, P: Scalar + 'static>(
    owner: &LcGraphOwner<'i, P, 5>,
    graph: usize,
    inventory: &'i LcInputInventory<P, 5>,
    registry: &'i Registry,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result<LcGraphOwner<'i, P, 5>> {
    let native = native(heap, account)?;
    let limit = account.limits().payload_bytes;
    let available = usize::try_from(limit)?
        .checked_sub(native + graph + 752)
        .ok_or("capacity fixture already full")?;
    let ballast = LcGraphOwner::admit(
        Vec::new(),
        &[],
        LcGraphCapacities {
            nodes: 0,
            edges: 0,
            scopes: 0,
            drive: 1,
            records: 0,
            words: 0,
            witnesses: available / 8,
        },
        inventory,
        registry,
        &[owner.borrowed()],
        heap,
        account,
    )
    .map_err(|failure| failure.error)?;
    let observed =
        LcGraphSnapshot::inspect(&[owner.borrowed(), ballast.borrowed()], heap, account)?;
    let total =
        observed.graph_bytes + observed.observer_bytes + observed.delivery_history_bytes + native;
    assert_eq!(total, usize::try_from(limit)? - available % 8);
    Ok(ballast)
}

/// Existing native residence owns the byte measurement used to select an independent new account.
pub(super) fn native(heap: &mut Heap, account: &mut ExecutionAccount) -> Result<usize> {
    Ok(match heap.residence(account, 100_000)? {
        Residence::Empty { retained_bytes } => retained_bytes,
        Residence::Pending {
            bytes: Some(bytes), ..
        } => bytes,
        Residence::Pending { bytes: None, .. } => return Err("native inventory unfinished".into()),
    })
}
