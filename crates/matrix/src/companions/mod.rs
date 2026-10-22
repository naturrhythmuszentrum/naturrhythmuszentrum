// SPDX-License-Identifier: AGPL-3.0-only
//! Heap-owned statistical relations and versioned two-sided target inventories.
//!
//! # Paired targets and statistical parameters
//!
//! `TargetMap` captures a current value occupation, version and exact logical order. `CompanionSetDraft` names both target maps and both source selections. `Heap::publish_joint` validates descriptors and mappings, then publishes two independently stored factor/weight regions and paired covariance descriptors atomically. `statistical` checks both target versions on every borrow; `joint_companions` provides the versioned coverage inventory. Compaction preserves the relation. Raw center copies require explicit rebinding to the retained source maps; equal bytes do not create independent sources. These centers are operating points, with no expectation claim. Supported affine, product and native nonlinear recipes propagate this statistical coverage; unimplemented recipes retain typed refusals. Attaching a model after operation history also refuses, preventing retroactive changes to uncaptured source assumptions. Replacement explicitly starts a new target version; old joint relations remain historical and refuse access.
//!
//! `JointModel::from_dense` retains that witness with the same global source-conflict and complete-table checks. Published dense companions own separate center and outward parameter-error regions. `CovarianceParameterView` exposes each entry through its paired product descriptor; backend storage metadata cannot substitute a ratio descriptor for that physical meaning. `ParameterErrorView` names the statistical companion, with bindings-derived `EvaluationDescriptor` entries for the full covariance product; it cannot be passed as a physical bound handle. These parameter enclosures do not bound random source excursions.
//!
//! A draft's optional `with_means` declares mathematical mean-parameter enclosures and constructs separate outward center-to-mean error regions before publication. Without it, `StatisticalView::means` is absent and the centers remain operating points. Repeated declarations for the same target must retain the same mean family and source map. Mean errors are numerical parameter errors, not physical-input radii. `cross_lineage` validates two current companion records, their shared model and all participating blocks, then exposes the exact source-selection rows needed by a later cross-covariance contraction. Repetitions retain dependence. These declaration APIs do not accept caller-supplied numerical Jacobians.

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
    pub(crate) regions: Vec<Key>,
    /// Optional declared mathematical mean families and their parameter errors.
    pub(crate) means: Option<Rc<dyn Any>>,
    /// Independently derived covariance entry meanings in column order.
    pub(crate) pairings: Vec<nrz_bindings::PairingWitness>,
    /// Full source-covariance parameter errors, independent of selected physical targets.
    pub(crate) evaluation: Vec<nrz_bindings::EvaluationDescriptor>,
}

mod parameter;
pub use parameter::{CovarianceParameterView, ParameterErrorView};

mod lineage;
pub use lineage::CrossLineage;

mod means;
pub use means::MeanParameters;
