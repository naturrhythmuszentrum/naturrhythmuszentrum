// SPDX-License-Identifier: AGPL-3.0-only
//! Reviewed catalogue entries and server observations retain their provenance.

use serde::{Deserialize, Serialize};

/// A reviewed implementation source, checked before rendering its catalogue.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Source {
    /// Normal root-relative source path.
    pub(super) path: String,
    /// Deterministic byte fingerprint, without an authenticity claim.
    pub(super) fingerprint: String,
}

/// An authored numerical law tied to reviewed source and executable claims.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Kernel {
    /// Stable catalogue identity.
    pub(super) id: String,
    /// Owning adopted crate.
    pub(super) owner: String,
    /// Operation or policy family.
    pub(super) operation: String,
    /// Exact mathematical operation.
    pub(super) law: String,
    /// Required input domain.
    pub(super) domain: String,
    /// Enclosure or policy guarantee.
    pub(super) guarantee: String,
    /// Admitted numerical shape.
    pub(super) shape: String,
    /// Named failure boundary.
    pub(super) refusals: String,
    /// Reviewed implementation source.
    pub(super) source: String,
    /// Owner-qualified executable claims.
    pub(super) claims: Vec<String>,
}

/// One named refusal or stopping outcome with its declared interpretation.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Refusal {
    /// Owning adopted crate.
    pub(super) owner: String,
    /// Declaring public enum.
    pub(super) type_name: String,
    /// Exact source variant name.
    pub(super) variant: String,
    /// Failure, resource, successful stop or refusing stop.
    pub(super) category: String,
    /// Meaning established by the source declaration.
    pub(super) meaning: String,
    /// Reviewed declaring source.
    pub(super) source: String,
}

/// A server observation names the tree and command producing its value.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Measurement {
    /// Stable observation identity.
    pub(super) id: String,
    /// Observable being measured.
    pub(super) metric: String,
    /// Exact nonnegative decimal supplied by the observation.
    pub(super) value: String,
    /// Unit of that decimal value.
    pub(super) unit: String,
    /// UTC observation date.
    pub(super) date: String,
    /// Full source commit identifier.
    pub(super) commit: String,
    /// Exact observed command.
    pub(super) command: String,
    /// Server record and diagnostic patch identifying the tested tree.
    pub(super) source: String,
}

/// Strict input and dataset payload; the generator canonicalizes table order.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Records {
    /// Reviewed source versions used by catalogue rows.
    pub(super) source_versions: Vec<Source>,
    /// Numerical laws and policy guarantees.
    pub(super) kernels: Vec<Kernel>,
    /// Named errors and stopping outcomes.
    pub(super) refusals: Vec<Refusal>,
    /// Dated observations; generation never runs their commands.
    pub(super) measurements: Vec<Measurement>,
}
