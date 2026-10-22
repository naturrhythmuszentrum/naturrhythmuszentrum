// SPDX-License-Identifier: AGPL-3.0-only
//! Builder hooks keep every reusable check tied to its original semantic state.

use crate::{
    CheckedDeclarations, CheckedExpression, Context, Diagnostic, SnapshotError, SnapshotId,
};

impl Context {
    /// Capture identity only; this does not assert that declarations have been checked.
    #[must_use]
    pub fn snapshot_id(&self) -> SnapshotId {
        SnapshotId::new(self.semantic.clone(), self.revision)
    }

    /// Validate a prior check before reusing its physical interpretation.
    ///
    /// # Errors
    /// Refuses checks from another context or an earlier semantic revision.
    pub fn validate_checked(&self, checked: &CheckedExpression) -> Result<(), SnapshotError> {
        self.snapshot_id().validate(checked.snapshot_id())
    }

    /// Check all numerical declarations and consume the builder into an immutable owner.
    ///
    /// Unused bodies, hidden original operands, cycles and companion relationships
    /// receive the same checks as an explicit `check_all` call. Metadata is retained
    /// separately and never acquires numerical checking evidence.
    ///
    /// # Errors
    /// Preserves the first original diagnostic or CAS resource refusal from `check_all`.
    pub fn freeze(self) -> Result<CheckedDeclarations, Diagnostic> {
        let checks = self.check_all()?;
        Ok(CheckedDeclarations::new(self, checks))
    }
}

/// Reserve the next revision before committing any declaration or rule change.
pub(crate) fn next_revision(revision: u64) -> Result<u64, Diagnostic> {
    revision
        .checked_add(1)
        .ok_or_else(|| Diagnostic::declaration(SnapshotError::RevisionExhausted.into()))
}

#[cfg(test)]
mod tests {
    use super::Context;
    use crate::SnapshotError;
    use crate::{Declaration, ExponentPolicy, Failure, FunctionSignature, Registry};
    use nrz_cas::{limits::Limits, syntax::Environment};

    /// Exhaustion cannot mutate either declaration namespace or wrap its revision.
    /// ´test:unit:snapshot-revision-exhaustion-is-atomic´
    #[test]
    fn snapshot_revision_exhaustion_is_atomic() -> Result<(), Box<dyn std::error::Error>> {
        let mut context = Context::new(
            Registry::new(ExponentPolicy::Rational),
            Limits::default(),
            Environment::default(),
        )?;
        context.revision = u64::MAX;
        let captured = context.snapshot_id();
        let declaration = context
            .declare("note".into(), Declaration::Metadata("text".into()))
            .err()
            .ok_or("revision did not refuse")?;
        let function = context
            .declare_function(
                "project".into(),
                FunctionSignature {
                    parameters: Vec::new(),
                    result: crate::Descriptor::ratio(),
                },
            )
            .err()
            .ok_or("revision did not refuse")?;
        for error in [declaration, function] {
            assert_eq!(
                error.failure,
                Failure::Snapshot(SnapshotError::RevisionExhausted)
            );
        }
        assert!(context.declarations.is_empty());
        assert!(context.functions.is_empty());
        assert_eq!(context.snapshot_id(), captured);
        Ok(())
    }
}
