// SPDX-License-Identifier: AGPL-3.0-only
//! C-layout mirrors enumerate independent primitive fields, not production cost helpers.

use super::super::{
    Backing,
    records::{Edge, Endpoint, Frame, Node, Scope},
};
use std::mem::{align_of, offset_of, size_of};

/// Independent generic vector header: pointer, length, capacity.
#[repr(C)]
struct Vector([usize; 3]);
/// Independent node fields in semantic order.
#[repr(C)]
struct NodeMirror {
    generation: u64,
    attempts: u64,
    epochs: [u64; 4],
    links: [usize; 8],
    flags: u64,
    counts: [usize; 2],
}
/// Independent edge endpoints and incidence storage.
#[repr(C)]
struct EdgeMirror {
    endpoints: [(usize, u64); 2],
    links: [usize; 4],
    demand: usize,
    flags: u64,
}
/// Independent shared owner's fixed fields, cleanup record and seven vector headers.
#[repr(C)]
struct OwnerMirror {
    account: u64,
    identity: u64,
    drive: u64,
    proposed: usize,
    format: u8,
    control: [usize; 15],
    cleanup: [usize; 4],
    vectors: [Vector; 7],
}
/// Explicit shared-allocation counters precede the payload with its native alignment.
#[repr(C)]
struct Shared {
    strong: usize,
    weak: usize,
    payload: OwnerMirror,
}
/// Independent layouts pin this covered row only; no existing native layout changes.
pub(super) fn check() {
    assert_eq!(
        (size_of::<NodeMirror>(), align_of::<NodeMirror>()),
        (136, 8)
    );
    assert_eq!((size_of::<Node>(), align_of::<Node>()), (136, 8));
    assert_eq!(offset_of!(Node, links), offset_of!(NodeMirror, links));
    assert_eq!(offset_of!(Node, counts), 120);
    assert_eq!((size_of::<EdgeMirror>(), size_of::<Edge>()), (80, 80));
    assert_eq!(size_of::<Endpoint>(), 16);
    assert_eq!(offset_of!(Edge, demand), 64);
    assert_eq!(size_of::<Frame>(), 16);
    assert_eq!(size_of::<Scope>(), 32);
    assert_eq!((size_of::<OwnerMirror>(), size_of::<Backing>()), (360, 360));
    assert_eq!(offset_of!(Backing, cleanup), 160);
    assert_eq!(offset_of!(Backing, nodes), 192);
    assert_eq!(size_of::<Shared>(), 376);
}
