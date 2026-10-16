// SPDX-License-Identifier: AGPL-3.0-only
//! Heap-owned moments reference independent statistical parameter regions.

use super::{matrix::Matrix, model::Model};
use crate::{Dependency, heap::identity::Key};
use nrz_scalar::Scalar;
use nrz_units::EvaluationDescriptor;
use std::marker::PhantomData;

/// Statistical law, independent of deterministic physical-input coverage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MomentAssurance {
    /// Exact affine finite-second-moment identity conditional on the declared model.
    AffineMoments,
}
/// Opaque owner-scoped locator for an immutable moment companion.
#[derive(Clone, Copy, Debug)]
pub struct MomentRef<P: Scalar> {
    /// Heap identity.
    pub(crate) scope: u64,
    /// Immutable relation index.
    pub(crate) index: usize,
    /// Sealed parameter precision.
    pub(crate) marker: PhantomData<P>,
}
/// Append-only inventory bounded by descriptor and element limits.
#[derive(Debug, Default)]
pub(crate) struct Store {
    /// Complete published relations.
    pub(crate) records: Vec<Record>,
}
/// Mathematical source relation shared by covariance and sensitivity regions.
#[derive(Clone, Debug)]
pub(crate) struct Lineage {
    /// Original joint model.
    pub(crate) model: Model,
    /// Source indices in original order.
    pub(crate) columns: Vec<usize>,
    /// K companion indices establishing the original target rows.
    pub(crate) roots: Vec<usize>,
    /// Retained statistical law.
    pub(crate) assurance: MomentAssurance,
}
/// Parameter matrix and its own numerical error region.
#[derive(Debug)]
pub(crate) struct Parameter {
    /// Center then evaluation-error occupations.
    pub(crate) regions: [Key; 2],
    /// Paired physical meaning and evaluation role of every entry.
    pub(crate) descriptors: Vec<EvaluationDescriptor>,
}
/// Moments retain both exact target selections.
#[derive(Debug)]
pub(crate) struct Record {
    /// Equal for full covariance; distinct for cross blocks.
    pub(crate) targets: [Dependency; 2],
    /// Original identities and dependence.
    pub(crate) lineage: Lineage,
    /// Full covariance, never a diagonal substitute.
    pub(crate) covariance: Parameter,
    /// Composed source sensitivity, absent for cross blocks.
    pub(crate) sensitivity: Option<Parameter>,
    /// Local derivative and its numerical evaluation error.
    pub(crate) jacobian: Option<Parameter>,
    /// Declared means; absent for nominal operating points.
    pub(crate) means: Option<Parameter>,
}
/// Complete unpublished statistical parameters.
#[derive(Debug)]
pub(crate) struct MomentPayload<P: Scalar> {
    /// Original model and retained assurance.
    pub(crate) lineage: Lineage,
    /// Composed sensitivity to the common sources.
    pub(crate) sensitivity: Matrix<P>,
    /// Independently enclosed full covariance.
    pub(crate) covariance: Matrix<P>,
    /// Checked local derivative.
    pub(crate) jacobian: Matrix<P>,
    /// Exact affine mean family, when declared for every input.
    pub(crate) means: Option<Matrix<P>>,
}
