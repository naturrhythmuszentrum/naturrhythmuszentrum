// SPDX-License-Identifier: AGPL-3.0-only
//! Successful checks retain original operations independently of normalized coordinates.

use std::collections::BTreeMap;

use nrz_algebra::{Error, Expr, Limits, NormalForm, Symbol};

use crate::{Companion, Conversion, Declaration, Descriptor, Location, UncertaintyMode};

/// How a checked uncertainty expression acquires its declared value relationship.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompanionEvidence {
    /// A declared input relationship preserved by supported identity operations.
    Established(Companion),
    /// A kind-checked body whose uncertainty contributors belong to the value closures.
    Propagated {
        /// The declared target relationship, available to later uncertainty operations.
        companion: Companion,
        /// Contributing uncertainty symbols and their checked value relationships.
        contributors: BTreeMap<Symbol, Companion>,
    },
}

impl CompanionEvidence {
    /// The relationship carried by either established or propagated evidence.
    #[must_use]
    pub const fn companion(&self) -> &Companion {
        match self {
            Self::Established(companion) | Self::Propagated { companion, .. } => companion,
        }
    }

    /// Preserve the evidence source while changing a supported uncertainty mode or reference.
    pub(super) fn transform(&mut self, mode: UncertaintyMode, reference: Option<Symbol>) {
        let (Self::Established(companion) | Self::Propagated { companion, .. }) = self;
        companion.mode = mode;
        companion.reference = reference;
    }
}

/// The descriptor evidence associated with one original node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeEvidence {
    /// Original tree location or expanded declaration location.
    pub location: Location,
    /// Derived descriptor before conversion to coherent result coordinates.
    pub descriptor: Descriptor,
    /// Conversion from this node's declared result coordinates to coherent coordinates.
    pub conversion: Conversion,
    /// Function argument conversions from coherent coordinates to signature coordinates.
    pub arguments: Vec<Conversion>,
}

/// A validated original expression and an optional algebraic view of its coherent coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExpression {
    /// Original source operations, including those that a normal form would erase.
    pub(super) original: Expr,
    /// Root descriptor with its semantic identity and affine role.
    pub(super) descriptor: Descriptor,
    /// Every original node's postorder checking evidence.
    pub(super) nodes: Vec<NodeEvidence>,
    /// Coherent coordinate expression built only after each original operation passes.
    pub(super) coordinates: Expr,
    /// Owned declaration snapshots, including provenance and original derived expressions.
    pub(super) declarations: BTreeMap<Symbol, Declaration>,
    /// Optional conversion from coherent coordinates to the caller's expected measure.
    pub(super) output: Option<Conversion>,
    /// Established or propagated value and reference identity, when supplied by the expression.
    pub(super) companion: Option<CompanionEvidence>,
}

impl CheckedExpression {
    /// Value and reference names carried by established or explicitly propagated evidence.
    #[must_use]
    pub const fn companion(&self) -> Option<&Companion> {
        match &self.companion {
            Some(evidence) => Some(evidence.companion()),
            None => None,
        }
    }

    /// Distinguish preserved input identity from an explicitly declared propagation.
    #[must_use]
    pub const fn companion_evidence(&self) -> Option<&CompanionEvidence> {
        self.companion.as_ref()
    }

    /// Original operations; these remain authoritative for domains and physical validity.
    #[must_use]
    pub const fn original(&self) -> &Expr {
        &self.original
    }

    /// Root's physical descriptor, never discarded when coordinates normalize to zero.
    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }

    /// Postorder evidence for original nodes and expanded declarations.
    #[must_use]
    pub fn nodes(&self) -> &[NodeEvidence] {
        &self.nodes
    }

    /// The declarations used by this check, retained independently of later context changes.
    #[must_use]
    pub const fn declarations(&self) -> &BTreeMap<Symbol, Declaration> {
        &self.declarations
    }

    /// Conversion to an explicit requested output descriptor, if one was checked.
    #[must_use]
    pub const fn output_conversion(&self) -> Option<Conversion> {
        self.output
    }

    /// Compute a generic collection form of coherent coordinates after validation.
    ///
    /// The result carries no physical descriptor and is not an input to the checker.
    /// Original domain obligations and declared nonzero assumptions remain separate.
    ///
    /// # Errors
    /// Refuses algebraic overflow, literal undefinedness or resource exhaustion.
    pub fn normal_form(&self, limits: Limits) -> Result<NormalForm, Error> {
        NormalForm::new(&self.coordinates, limits)
    }
}
