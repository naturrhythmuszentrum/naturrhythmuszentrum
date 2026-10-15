// SPDX-License-Identifier: AGPL-3.0-only
//! Heap-owned statistical relations and versioned two-sided target inventories.

mod access;
mod publish;
mod target;

pub use access::{CompanionRef, StatisticalView};
pub use publish::CompanionSetDraft;
pub use target::TargetMap;

use crate::heap::identity::Key;
use std::{any::Any, rc::Rc};
use target::Target;

/// Heap-owned immutable statistical records; model payloads retain their sealed precision.
#[derive(Default)]
pub(crate) struct JointStore {
    /// Append-only relation inventory, bounded before publication.
    pub(crate) records: Vec<JointRecord>,
}
impl std::fmt::Debug for JointStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JointStore")
            .field("records", &self.records.len())
            .finish()
    }
}
/// Two-sided target relation and its own factor regions.
pub(crate) struct JointRecord {
    /// Exact target versions and logical selections.
    pub(crate) targets: [Target; 2],
    /// Ordered source coordinates selected on each side.
    pub(crate) selections: [Vec<usize>; 2],
    /// Immutable joint model, downcast only after checking sealed precision.
    pub(crate) model: Rc<dyn Any>,
    /// Independently stored represented factor and weight regions.
    pub(crate) regions: [Key; 2],
    /// Independently derived covariance entry meanings in column order.
    pub(crate) pairings: Vec<nrz_units::PairingWitness>,
}
