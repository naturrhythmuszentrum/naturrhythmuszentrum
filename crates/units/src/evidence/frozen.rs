// SPDX-License-Identifier: AGPL-3.0-only
//! The frozen owner retains the actual CAS, original occurrences and checked records.

use std::cell::Ref;
use std::collections::BTreeMap;

use nrz_cas::{Cas, Fault};

use crate::{
    CheckedExpression, Context, Declaration, Diagnostic, Failure, FunctionSignature, Registry,
    SnapshotError, SnapshotId,
};

/// Immutable checked declarations owning all CAS records referenced by their evidence.
///
/// Cloned descriptors, expressions and IDs do not own the CAS store; retain this
/// snapshot while inspecting their original occurrences or graph witnesses.
/// No mutable context, registry, function or check access is exposed.
///
#[doc = include_str!("../../tests/conformance/evidence.inc")]
#[derive(Debug)]
pub struct CheckedDeclarations {
    /// The original builder is privately owned, including its CAS store and rule tables.
    context: Context,
    /// Identity of the complete successfully checked semantic state.
    id: SnapshotId,
    /// Immutable results for every numerical declaration in deterministic name order.
    checks: BTreeMap<String, CheckedExpression>,
}

impl CheckedDeclarations {
    /// Called only after the complete builder has passed `check_all`.
    pub(crate) fn new(context: Context, checks: BTreeMap<String, CheckedExpression>) -> Self {
        Self {
            id: context.snapshot_id(),
            context,
            checks,
        }
    }

    /// Exact identity retained by every check in this snapshot.
    #[must_use]
    pub const fn id(&self) -> &SnapshotId {
        &self.id
    }

    /// All declarations, including provenance, metadata and original derived bodies.
    #[must_use]
    pub const fn declarations(&self) -> &BTreeMap<String, Declaration> {
        &self.context.declarations
    }

    /// The original semantic operation rules and kind inventory.
    #[must_use]
    pub const fn registry(&self) -> &Registry {
        &self.context.registry
    }

    /// Immutable ordered function signatures used by the original checks.
    #[must_use]
    pub const fn functions(&self) -> &BTreeMap<String, FunctionSignature> {
        &self.context.functions
    }

    /// Complete numerical evidence inventory; metadata has no entry here.
    #[must_use]
    pub const fn checks(&self) -> &BTreeMap<String, CheckedExpression> {
        &self.checks
    }

    /// Resolve a numerical declaration's checked evidence by its declared key.
    ///
    /// # Errors
    /// Refuses metadata and unknown names without inventing a checked descriptor.
    pub fn checked(&self, name: &str) -> Result<&CheckedExpression, Diagnostic> {
        self.checks.get(name).ok_or_else(|| {
            let failure = if self.context.declarations.contains_key(name) {
                Failure::Metadata(name.into())
            } else {
                Failure::UnknownSymbol(name.into())
            };
            Diagnostic::declaration(failure)
        })
    }

    /// Check the semantic dependency of a separately retained expression.
    ///
    /// # Errors
    /// Refuses foreign or stale evidence even when its descriptor is equal.
    pub fn validate_checked(&self, checked: &CheckedExpression) -> Result<(), SnapshotError> {
        self.id.validate(checked.snapshot_id())
    }

    /// Inspect CAS-owned original occurrences and witnesses while this owner is alive.
    ///
    /// # Errors
    /// Refuses a conflicting store borrow as an ordinary CAS fault.
    pub fn cas(&self) -> Result<Ref<'_, Cas>, Fault> {
        self.context.cas()
    }
}
