// SPDX-License-Identifier: AGPL-3.0-only

//! Errors from the documentation pipeline.

use thiserror::Error;

/// A path-specific failure in documentation generation.
#[derive(Debug, Error)]
pub enum Error {
    /// A filesystem operation failed.
    #[error("I/O at {path}: {source}")]
    Io {
        /// Root-relative file or directory.
        path: String,
        /// Original operating-system error.
        #[source]
        source: std::io::Error,
    },
    /// A TOML document is not syntactically valid.
    #[error("TOML syntax in {path}: {detail}")]
    TomlSyntax {
        /// Root-relative document.
        path: String,
        /// Parser diagnostic.
        detail: String,
    },
    /// A JSON document is not syntactically valid.
    #[error("JSON syntax in {path}: {detail}")]
    JsonSyntax {
        /// Root-relative document.
        path: String,
        /// Parser diagnostic.
        detail: String,
    },
    /// An envelope is missing or has an invalid field.
    #[error("envelope in {path}: {detail}")]
    Envelope {
        /// Root-relative document.
        path: String,
        /// Failed envelope rule.
        detail: String,
    },
    /// A typed payload is invalid.
    #[error("payload in {path}: {detail}")]
    Payload {
        /// Root-relative document.
        path: String,
        /// Failed payload rule.
        detail: String,
    },
    /// Workspace membership is invalid.
    #[error("membership in {path}: {detail}")]
    Membership {
        /// Root-relative manifest or member.
        path: String,
        /// Failed membership rule.
        detail: String,
    },
    /// The document manifest is invalid.
    #[error("manifest in {path}: {detail}")]
    Manifest {
        /// Root-relative manifest or output.
        path: String,
        /// Failed manifest rule.
        detail: String,
    },
    /// A template header is invalid.
    #[error("template header in {path}: {detail}")]
    TemplateHeader {
        /// Root-relative template.
        path: String,
        /// Failed header rule.
        detail: String,
    },
    /// A dataset required by a document is absent or incompatible.
    #[error("dataset {dataset} for {document}: {detail}")]
    DatasetCompatibility {
        /// Root-relative document output.
        document: String,
        /// Dataset name or path.
        dataset: String,
        /// Failed compatibility rule.
        detail: String,
    },
    /// A template could not be loaded or evaluated.
    #[error("template {path}: {detail}")]
    Template {
        /// Root-relative template.
        path: String,
        /// Template diagnostic.
        detail: String,
    },
}

/// Attach a root-relative path to an I/O error.
pub(crate) fn io(path: &str, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}
