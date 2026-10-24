// SPDX-License-Identifier: AGPL-3.0-only
//! Original LC bindings cover native arrays before their single-use permits reach allocation.

mod capture;
mod owner;
mod record;
mod run;
mod stage;

use super::{PhasorError, PhasorSolve};
use nrz_scalar::Scalar;
pub(super) use owner::Owner;
pub use record::{PhasorAllocation, PhasorInventory};
pub(super) use run::Run;
use stage::PhasorAllocationStage as Stage;
pub use stage::{PhasorAllocationChannel, PhasorAllocationStage};
use std::{cell::Cell, rc::Rc};

/// Closed schedule includes the optional publication suffix.
const COUNT: usize = 49;
/// Complete interpretation and observed lifetime of one allocation request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    /// The exact binding is prepared but no allocator has been called.
    Planned,
    /// A typed permit was issued before its numerical allocation.
    Issued,
    /// One typed owner has consumed its permit.
    Live,
    /// The complete initialized owner has been dropped.
    Released,
    /// The original source lacks this optional companion, or publication was not requested.
    Absent,
}
/// One shared original definition and finite lifecycle table, without copied declarations.
#[derive(Debug)]
struct Plan<P: Scalar> {
    /// Original equation, source versions, copy order, fixed inputs and metric definitions.
    solve: PhasorSolve<P>,
    /// Unique native execution independent of numerical storage addresses.
    execution: crate::ExecutionId,
    /// Full original allocation schedule, including optional scalar companion copies.
    states: [Cell<State>; COUNT],
    /// Actual vector capacities or inline owner sizes; each observation is written once.
    bytes: [Cell<usize>; COUNT],
    /// Common live numerical payload, never a sum of overlapping scoped peaks.
    live: Cell<usize>,
    /// Maximum common live payload during this native execution.
    peak: Cell<usize>,
    /// This execution includes the complete three-region publication suffix.
    publication: bool,
}
/// Authority is not cloneable and never lives apart from its actual numerical owner.
#[derive(Debug)]
struct Permit<P: Scalar> {
    /// Original allocating binding and the matching finite lifecycle table.
    plan: Rc<Plan<P>>,
    /// Exact closed schedule position.
    slot: usize,
}
/// Account for alignment of the actual reference-counted allocation.
#[repr(C)]
struct RcLayout<T> {
    /// Strong reference count.
    strong: usize,
    /// Weak reference count.
    weak: usize,
    /// Actual aligned allocation payload.
    value: T,
}

#[cfg(test)]
mod tests;
