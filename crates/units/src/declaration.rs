// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable input, observable, metadata and function declarations.

use std::collections::BTreeMap;

use nrz_cas::{Cas, Handle, Rational, limits::Limits, source::OriginalForm};
use std::cell::RefCell;

use crate::{Descriptor, Diagnostic, Failure, Registry, UncertaintyMode, UnitError};

/// Provenance and assurance are supplied claims, not certified by a units check.
///
/// TODO ´todo:code:identify-versioned-shared-source-declarations-separately´: identify versioned shared source declarations separately from descriptive provenance under rule:nrz:batch-source-covariance.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Evidence {
    /// Source identifier or explanatory provenance.
    pub source: String,
    /// Caller-supplied assurance description.
    pub assurance: String,
}

/// An observable's declared contract and its original expression.
///
/// TODO ´todo:code:attach-declared-output-shapes-and-indexed´: attach declared output shapes and indexed dependencies to future matrix observables under rule:nrz:shape-data-time-evidence.
///
/// TODO ´todo:code:retain-original-observable-dependencies-when-clients´: retain original observable dependencies when clients assign plan slots under def:nrz:plan-binding.
#[derive(Clone, Debug)]
pub struct Observable {
    /// Expected dimension, kind, role and output coordinate measure.
    pub descriptor: Descriptor,
    /// Original operations, never a normalized substitute for checking.
    pub expression: OriginalForm,
    /// Declared provenance and assurance, retained without numerical verification.
    pub evidence: Evidence,
}

/// Named value relationships for an independent uncertainty companion.
///
/// TODO ´todo:code:bind-uncertainty-targets-to-ordered-value´: bind uncertainty targets to ordered value entries and versions rather than scalar names alone under def:nrz:companion-shape.
///
/// TODO ´todo:code:represent-companion-relationships-across-distinct-binding´: represent companion relationships across distinct binding points without inferring independence under rule:nrz:batch-source-covariance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Companion {
    /// Absolute, relative, variance or covariance interpretation.
    ///
    /// TODO ´todo:code:distinguish-marginal-arrays-from-full-covariance´: distinguish marginal arrays from full covariance and joint coverage interpretations under def:nrz:companion-shape and rule:nrz:expanded-uncertainty-shapes.
    pub mode: UncertaintyMode,
    /// Primary value declaration.
    ///
    /// TODO ´todo:code:retain-the-primary-target-entry-map´: retain the primary target entry map through views and structural expansion under conv:nrz:shape-entry-order and rule:nrz:structured-companion-coordinates.
    pub value: String,
    /// Second value declaration, required exactly for covariance.
    ///
    /// TODO ´todo:code:retain-the-second-covariance-axis-and´: retain the second covariance axis and cross block target identities under def:nrz:covariance-shape.
    pub other: Option<String>,
    /// Compatible nonzero reference, required exactly for relative modes.
    ///
    /// TODO ´todo:code:identify-per-component-nonzero-references-and´: identify per component nonzero references and paired relative covariance scales under rule:nrz:correlation-and-relative-shapes.
    pub reference: Option<String>,
}

/// A symbol's data-time meaning.
///
/// TODO ´todo:code:extend-declared-quantities-with-ordered-shapes´: extend declared quantities with ordered shapes while keeping extents separate from physical counts under def:nrz:matrix-shape.
///
/// TODO ´todo:code:identify-immutable-declaration-versions-for-reusable´: identify immutable declaration versions for reusable plan validation under rule:nrz:plan-validation.
#[derive(Clone, Debug)]
pub enum Declaration {
    /// A numerical input with an explicit nonzero assumption if required.
    Input {
        /// Physical descriptor of source coordinates.
        descriptor: Descriptor,
        /// An assumption available to relative uncertainty checks.
        nonzero: bool,
        /// Declared provenance and assurance.
        evidence: Evidence,
    },
    /// A typed exact constant, including a physical zero or whole count import.
    Constant {
        /// Physical descriptor of the exact coordinate.
        descriptor: Descriptor,
        /// Exact coordinate before measure conversion.
        value: Rational,
        /// Declared provenance and assurance.
        evidence: Evidence,
    },
    /// An uncertainty input explicitly bound to its value and optional reference names.
    ///
    /// TODO ´todo:code:validate-imported-companion-extents-and-indexed´: validate imported companion extents and indexed value relationships before numerical access under def:nrz:companion-shape and rule:nrz:shape-data-time-evidence.
    UncertaintyInput {
        /// Independent physical descriptor of the uncertainty coordinates.
        descriptor: Descriptor,
        /// Declared value identities, checked during resolution.
        companion: Companion,
        /// Source and assurance claims, retained without numerical certification.
        evidence: Evidence,
    },
    /// A derived value whose original expression must satisfy its declared descriptor.
    Derived(Observable),
    /// An independently typed uncertainty expression with explicit value relationships.
    ///
    /// TODO ´todo:code:declare-output-covariance-maps-and-contributors´: declare output covariance maps and contributors without asserting a validated Jacobian under rule:nrz:shape-data-time-evidence.
    Uncertainty {
        /// Expected descriptor, expression and evidence of the uncertainty itself.
        observable: Observable,
        /// Values and optional reference described by this companion.
        companion: Companion,
    },
    /// Non-numerical metadata, forbidden as an algebraic operand.
    Metadata(String),
}

impl Declaration {
    /// The declaration's descriptor, if it is numerical.
    #[must_use]
    pub const fn descriptor(&self) -> Option<&Descriptor> {
        match self {
            Self::Input { descriptor, .. }
            | Self::Constant { descriptor, .. }
            | Self::UncertaintyInput { descriptor, .. } => Some(descriptor),
            Self::Derived(observable) | Self::Uncertainty { observable, .. } => {
                Some(&observable.descriptor)
            }
            Self::Metadata(_) => None,
        }
    }

    /// Whether the declaration supplies a nonzero condition without deriving one.
    #[must_use]
    pub fn is_nonzero(&self) -> bool {
        match self {
            Self::Input { nonzero, .. } => *nonzero,
            Self::Constant { value, .. } => *value != Rational::zero(),
            Self::Derived(_)
            | Self::Uncertainty { .. }
            | Self::UncertaintyInput { .. }
            | Self::Metadata(_) => false,
        }
    }
}

/// A fixed ordered function signature; measures specify its argument and return coordinates.
///
/// TODO ´todo:code:declare-shaped-arguments-results-and-dependency´: declare shaped arguments results and dependency maps for matrix valued applications under rule:nrz:shape-data-time-evidence.
///
/// TODO ´todo:code:freeze-declared-function-interfaces-across-all´: freeze declared function interfaces across all bindings of one plan under rule:nrz:batch-variation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionSignature {
    /// Expected ordered descriptors, including affine roles.
    pub parameters: Vec<Descriptor>,
    /// Declared result descriptor.
    pub result: Descriptor,
}

/// A reusable declaration environment; checks own snapshots of all evidence they use.
///
/// TODO ´todo:code:retain-shape-and-value-identity-snapshots´: retain shape and value identity snapshots independently of later declarations or batch reuse under dec:nrz:shape-descriptor-ownership.
#[derive(Debug)]
pub struct Context {
    /// The engine owns all original occurrences and generated coordinates.
    pub(super) cas: RefCell<Cas>,
    /// Immutable mathematical declarations shared by source parsing and proofs.
    pub(super) semantic: Handle,
    /// Explicit physical operation rules.
    pub(super) registry: Registry,
    /// Value and metadata declarations in deterministic name order.
    pub(super) declarations: BTreeMap<String, Declaration>,
    /// Client-defined fixed function signatures in a separate namespace.
    pub(super) functions: BTreeMap<String, FunctionSignature>,
    /// Complete physical semantic revision, advanced only by successful mutations.
    pub(super) revision: u64,
    /// Expression, expansion and work limits for each independent check.
    pub(super) limits: Limits,
}

impl Context {
    /// Add a declaration atomically; forward references are checked during resolution.
    ///
    /// TODO ´todo:code:validate-shape-declarations-and-companion-target´: validate shape declarations and companion target maps atomically at the context boundary under rule:nrz:shape-data-time-evidence.
    ///
    /// # Errors
    /// Refuses duplicate names, invalid kinds/counts or an exhausted semantic revision.
    pub fn declare(&mut self, name: String, declaration: Declaration) -> Result<(), Diagnostic> {
        if self.declarations.contains_key(&name) {
            return Err(Diagnostic::declaration(Failure::Duplicate(name)));
        }
        let mut registry = self.registry.clone();
        if let Declaration::Derived(observable) | Declaration::Uncertainty { observable, .. } =
            &declaration
        {
            self.cas()
                .and_then(|cas| {
                    cas.walk_original(&observable.expression.root, &mut crate::exact::budget()?)
                        .map(|_| ())
                        .map_err(|error| error.fault)
                })
                .map_err(|error| Diagnostic::declaration(error.into()))?;
        }
        if let Some(descriptor) = declaration.descriptor() {
            if descriptor.kind().is_relative()
                && !matches!(
                    declaration,
                    Declaration::Uncertainty { .. } | Declaration::UncertaintyInput { .. }
                )
            {
                return Err(Diagnostic::declaration(UnitError::RelativeReference.into()));
            }
            registry
                .admit(descriptor)
                .map_err(|error| Diagnostic::declaration(error.into()))?;
            if let Declaration::Constant { value, .. } = &declaration
                && descriptor.kind().count_domain().is_some()
            {
                descriptor
                    .check_count_literal(value)
                    .map_err(|error| Diagnostic::declaration(error.into()))?;
            }
        }
        let revision = crate::evidence::next_revision(self.revision)?;
        self.registry = registry;
        self.declarations.insert(name, declaration);
        self.revision = revision;
        Ok(())
    }

    /// Add an uninterpreted function's physical signature atomically.
    ///
    /// TODO ´todo:code:validate-function-contraction-and-output-axes´: validate function contraction and output axes alongside the declared scalar signatures under rule:nrz:shape-data-time-evidence.
    ///
    /// # Errors
    /// Refuses invalid descriptors, duplicate/builtin names or an exhausted semantic revision.
    pub fn declare_function(
        &mut self,
        name: String,
        signature: FunctionSignature,
    ) -> Result<(), Diagnostic> {
        if signature.result.kind().is_relative() {
            return Err(Diagnostic::declaration(UnitError::RelativeReference.into()));
        }
        if self.functions.contains_key(&name)
            || matches!(
                name.as_str(),
                "sin"
                    | "cos"
                    | "tan"
                    | "asin"
                    | "acos"
                    | "atan"
                    | "exp"
                    | "log"
                    | "sqrt"
                    | "count_ratio"
            )
        {
            return Err(Diagnostic::declaration(Failure::Duplicate(name)));
        }
        let mut registry = self.registry.clone();
        for descriptor in signature
            .parameters
            .iter()
            .chain(std::iter::once(&signature.result))
        {
            registry
                .admit(descriptor)
                .map_err(|error| Diagnostic::declaration(error.into()))?;
        }
        let revision = crate::evidence::next_revision(self.revision)?;
        self.registry = registry;
        self.functions.insert(name, signature);
        self.revision = revision;
        Ok(())
    }
}
