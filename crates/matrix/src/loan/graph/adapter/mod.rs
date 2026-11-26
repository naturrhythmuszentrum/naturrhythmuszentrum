// SPDX-License-Identifier: AGPL-3.0-only
//! Native graph composition keeps sealed capabilities in phase-specific records.

mod admission;
mod drive;
mod exact;
pub(crate) mod gate;
mod input;
pub(in crate::loan::graph) mod native;
mod node;
mod owner;
mod refusal;
mod scope;

mod access;
mod notify;

mod residence;
mod snapshot;

mod retry;

mod cancel;

pub use admission::Refusal as LcGraphAdmissionFailure;
pub use drive::Event as LcGraphEvent;
pub use exact::Report as LcGraphExactChildren;
pub use input::{InputRequest as LcGraphInputRequest, InputReturn as LcGraphInputReturn};
pub use native::Failure as LcGraphNativeFailure;
pub use node::{Mapping as LcGraphMapping, Node as LcGraphNode, State as LcGraphState};
pub use owner::{
    Borrowed as LcGraphBorrowed, Capacities as LcGraphCapacities, Owner as LcGraphOwner,
};
pub use residence::{Completion as LcGraphCompletion, OwnerSet as LcGraphOwnerSet};
pub use scope::{Progress as LcGraphScopeProgress, Scope as LcGraphScope};
pub use snapshot::{Native as LcGraphNativeResidence, Snapshot as LcGraphSnapshot};

#[cfg(test)]
mod tests;
