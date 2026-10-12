// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable input, observable, metadata and function declarations.

use std::collections::BTreeMap;

use nrz_algebra::{Expr, Limits, Rational, Symbol};

use crate::{Descriptor, Diagnostic, Failure, Registry, UncertaintyMode, UnitError};

/// Provenance and assurance are supplied claims, not certified by a units check.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Evidence {
    /// Source identifier or explanatory provenance.
    pub source: String,
    /// Caller-supplied assurance description.
    pub assurance: String,
}

/// An observable's declared contract and its original expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observable {
    /// Expected dimension, kind, role and output coordinate measure.
    pub descriptor: Descriptor,
    /// Original operations, never a normalized substitute for checking.
    pub expression: Expr,
    /// Declared provenance and assurance, retained without numerical verification.
    pub evidence: Evidence,
}

/// Named value relationships for an independent uncertainty companion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Companion {
    /// Absolute, relative, variance or covariance interpretation.
    pub mode: UncertaintyMode,
    /// Primary value declaration.
    pub value: Symbol,
    /// Second value declaration, required exactly for covariance.
    pub other: Option<Symbol>,
    /// Compatible nonzero reference, required exactly for relative modes.
    pub reference: Option<Symbol>,
}

/// A symbol's data-time meaning.
#[derive(Clone, Debug, Eq, PartialEq)]
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
            Self::Constant { value, .. } => *value != Rational::ZERO,
            Self::Derived(_)
            | Self::Uncertainty { .. }
            | Self::UncertaintyInput { .. }
            | Self::Metadata(_) => false,
        }
    }
}

/// A fixed ordered function signature; measures specify its argument and return coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionSignature {
    /// Expected ordered descriptors, including affine roles.
    pub parameters: Vec<Descriptor>,
    /// Declared result descriptor.
    pub result: Descriptor,
}

/// A reusable declaration environment; checks own snapshots of all evidence they use.
#[derive(Clone, Debug)]
pub struct Context {
    /// Explicit physical operation rules.
    pub(super) registry: Registry,
    /// Value and metadata declarations in deterministic name order.
    pub(super) declarations: BTreeMap<Symbol, Declaration>,
    /// Client-defined fixed function signatures in a separate namespace.
    pub(super) functions: BTreeMap<Symbol, FunctionSignature>,
    /// Expression, expansion and work limits for each independent check.
    pub(super) limits: Limits,
}

impl Context {
    /// Create an environment with explicit resource limits.
    #[must_use]
    pub const fn new(registry: Registry, limits: Limits) -> Self {
        Self {
            registry,
            declarations: BTreeMap::new(),
            functions: BTreeMap::new(),
            limits,
        }
    }

    /// Add a declaration atomically; forward references are checked during resolution.
    ///
    /// # Errors
    /// Refuses duplicate names, inconsistent kinds and invalid exact count imports.
    pub fn declare(&mut self, name: Symbol, declaration: Declaration) -> Result<(), Diagnostic> {
        if self.declarations.contains_key(&name) {
            return Err(Diagnostic::declaration(Failure::Duplicate(name)));
        }
        let mut registry = self.registry.clone();
        if let Declaration::Derived(observable) | Declaration::Uncertainty { observable, .. } =
            &declaration
        {
            observable
                .expression
                .validate_limits(self.limits)
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
                    .check_count_literal(*value)
                    .map_err(|error| Diagnostic::declaration(error.into()))?;
            }
        }
        self.registry = registry;
        self.declarations.insert(name, declaration);
        Ok(())
    }

    /// Add an uninterpreted function's physical signature atomically.
    ///
    /// # Errors
    /// Refuses inconsistent descriptors and duplicate or builtin function names.
    pub fn declare_function(
        &mut self,
        name: Symbol,
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
        self.registry = registry;
        self.functions.insert(name, signature);
        Ok(())
    }
}
