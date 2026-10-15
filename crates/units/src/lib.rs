// SPDX-License-Identifier: AGPL-3.0-only
#![doc = include_str!("../README.md")]
#![expect(
    clippy::multiple_crate_versions,
    reason = "nrz-cas uses petgraph 0.8.3 with hashbrown 0.15 while indexmap uses hashbrown 0.17"
)]

mod checked;
mod checker;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod constants;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod coordinates;
mod declaration;
mod descriptor;
mod diagnostic;
mod dimension;
mod engine;
mod error;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod evidence;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod exact;
mod kind;
mod measure;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod original;
mod pairing;
mod registry;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod sensitivity;

pub use checked::{CheckedExpression, CompanionEvidence, NodeEvidence};
pub use constants::{CheckedMap, ConstantKind, DescriptorConstant, MapError, MapIdentity, MapKind};
pub use declaration::{Companion, Context, Declaration, Evidence, FunctionSignature, Observable};
pub use descriptor::{Descriptor, Role, uncertainty};
pub use diagnostic::{Diagnostic, Expectation, Failure, Location};
pub use dimension::{BaseDimension, Dimension, ExponentPolicy};
pub use error::UnitError;
pub use evidence::{CheckedDeclarations, SnapshotError, SnapshotId};
pub use kind::{CountDomain, Kind, UncertaintyMode};
pub use measure::{Conversion, Measure, Scale};
pub use pairing::{
    EvaluationDescriptor, EvaluationKind, NamedReference, PairingError, PairingKind, PairingSide,
    PairingWitness,
};
pub use registry::{BinaryOp, Registry};
pub use sensitivity::{
    CheckedSensitivity, JacobianEntry, JacobianEvidence, SensitivityEntry, SensitivityError,
    SensitivityKernel, SensitivityObligation, SensitivityRecipe, SensitivityWork, SolveSymbols,
};

#[cfg(test)]
mod tests;
