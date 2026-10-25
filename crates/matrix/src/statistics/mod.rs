// SPDX-License-Identifier: AGPL-3.0-only
//! Affine and linearized moments with source-preserving, independently verified contractions.
//!
//! # Source-preserving affine propagation
//!
//! `OperationDraft::copy`, `add`, `subtract`, fixed `linear_map`, and exact entry transformations propagate complete joint covariance and cross blocks. Checked K source-selection rows establish the initial relation; subsequent operations compose sensitivities while preserving the original model and source identities. Repeated operands remain dependent: adding a value to itself multiplies its covariance by four. Distinct models, missing selected source rows, unknown participating cross blocks and exhausted limits refuse explicitly. Measure and precision conversions preserve this same source relation.
//!
//! `Heap::moment_companion` locates the propagated companion, and `Heap::moments` lends a `MomentView` with its explicit `MomentAssurance`, complete covariance, composed sensitivity, local Jacobian and separately declared mean parameters. An operating center alone does not create a mean claim. Each `ParameterView` exposes its own numerical evaluation error and parameter target; covariance error cannot be used as a deterministic physical bound. Both contraction stages use faer candidates with independent increasing-index interval verification, including source-parameter and derivative errors.
//!
//! `Heap::materialize_cross_covariance` requires both retained source lineages under one complete model. It publishes every cross entry with both exact target selections and refuses a raw copy without checked source rows. Moments and all numerical parameter regions are prepared before the existing atomic commit; a failed append publishes no value prefix. `Bound::statistical_work` exposes the conservative complete-phase allowance. Storage limits include all covariance, sensitivity, derivative and optional mean regions. Source replacement invalidates dependent moment access, and compaction preserves the source and parameter identities. Statistical dependency inspections are charged against the heap's layout-work limit and do not recursively enter provenance validation.

mod access;
mod affine;
mod allocation;
mod capture;
pub(crate) mod contraction;
mod cross;
mod declarations;
mod derivative;
mod error;
mod evaluate;
mod family;
pub(crate) mod inventory;
mod matrix;
pub(crate) use affine::Affine;
pub use inventory::{
    StatisticsAllocation, StatisticsAllocationChannel, StatisticsAllocationRole,
    StatisticsInventory,
};
pub(crate) use matrix::{Matrix, enclosed, singleton};
mod model;
mod nonlinear;
mod operating;
mod plan;
mod product;
mod publish;
pub(crate) mod reciprocal;
pub(crate) mod store;
mod structure;

pub use access::{MomentView, ParameterView};
pub use error::StatisticsError;
pub(crate) use plan::Plan;
pub(crate) use publish::{PreparedMoments, prepare};
pub use store::{MomentAssurance, MomentRef};
pub(crate) use store::{MomentPayload, Store};

pub(crate) use capture::refuse;
mod conversion;
mod precision;

mod structured;
pub(crate) use structured::lower_definition;
