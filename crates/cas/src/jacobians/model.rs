// SPDX-License-Identifier: AGPL-3.0-only
//! Sealed derivative bundles retain the original representation beside its checked expansion.

use super::SourceJacobianKey;
use crate::graph::Handle;
use crate::proof_model::VerifiedWitness;

/// Explicit coordinate declarations for one partial-derivative family.
///
/// Only `sources` contribute columns, in their supplied order. Other declared
/// scalar symbols are held constant. `fixed` additionally forbids requesting those
/// declaration keys as sources; both lists participate in the portable identity.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SourceCoordinates {
    /// Distinct declared Real scalar keys, in Jacobian column order.
    pub sources: Vec<String>,
    /// Distinct declared Real scalar keys forbidden as derivative directions.
    pub fixed: Vec<String>,
}

/// Exact Jacobian and the ordinary replayable evidence connecting it to its original.
///
/// Matrix outputs flatten in logical column order. The derivative witness uses
/// that explicit output column. For matrix syntax, `representation` proves the
/// original equal to its explicit scalar matrix before flattening. Retain both
/// witnesses: the representation carries original domains even beneath zeros.
#[derive(Clone, Debug)]
pub struct SourceJacobian {
    /// Original expression, complete context and ordered coordinate declaration.
    pub(super) key: SourceJacobianKey,
    /// Explicit Real matrix with one column per requested source.
    pub(super) matrix: Handle,
    /// Original matrix equal to its explicit coefficient representation.
    pub(super) representation: Option<VerifiedWitness>,
    /// Checked partials on the entire admitted family, not sampled values.
    pub(super) witness: VerifiedWitness,
}

impl SourceJacobian {
    /// Complete portable identity and authenticated local input.
    #[must_use]
    pub const fn key(&self) -> &SourceJacobianKey {
        &self.key
    }

    /// Exact matrix, with logical output entries as rows and sources as columns.
    #[must_use]
    pub const fn matrix(&self) -> &Handle {
        &self.matrix
    }

    /// Ordinary portable Jacobian theorem; every entry has a checked partial premise.
    #[must_use]
    pub const fn witness(&self) -> &VerifiedWitness {
        &self.witness
    }

    /// Original-to-explicit matrix equality, absent for an original scalar.
    #[must_use]
    pub const fn representation(&self) -> Option<&VerifiedWitness> {
        self.representation.as_ref()
    }
}
