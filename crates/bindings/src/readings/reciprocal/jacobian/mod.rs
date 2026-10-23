// SPDX-License-Identifier: AGPL-3.0-only
//! Shared source derivatives retain the checked original operator and physical references.

mod cache;
mod key;

pub use cache::ReciprocalJacobianCache;
pub use key::ReciprocalJacobianKey;

use super::{PositiveReciprocalMap, ReadingError, support};
use nrz_cas::{
    Cas,
    jacobians::SourceJacobian,
    limits::{Budget, BudgetKind, BudgetUsage},
};
use std::rc::Rc;

/// Complete cumulative snapshots around native source-Jacobian construction and replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceJacobianUsage {
    /// Caller ledger before deriving this exact key.
    pub before: BudgetUsage,
    /// Caller ledger after constructing, checking and replaying the whole derivative.
    pub after: BudgetUsage,
}

/// One shared exact derivative, independent of numerical values and storage versions.
#[derive(Clone, Debug)]
pub struct ReciprocalJacobian(Rc<Evidence>);

/// Whole-family representation and derivative evidence remain inseparable.
#[derive(Debug)]
struct Evidence {
    /// Original physical operation and ordered source/fixed declaration.
    key: ReciprocalJacobianKey,
    /// Native complete derivative of the original homogeneous operator matrix.
    native: SourceJacobian,
    /// Complete caller-ledger snapshots, not per-request peaks.
    usage: SourceJacobianUsage,
    /// Conservative retained native proof, key and physical-definition payload.
    bytes: u64,
}

impl ReciprocalJacobian {
    /// Original typed operator with every reference, positive input and zero coefficient.
    #[must_use]
    pub fn definition(&self) -> &PositiveReciprocalMap {
        self.0.key.definition()
    }

    /// Exact declarations and original relation used for cache comparison.
    #[must_use]
    pub fn key(&self) -> &ReciprocalJacobianKey {
        &self.0.key
    }

    /// Full homogeneous matrix derivative and original-to-explicit representation proof.
    ///
    /// Rows follow the original homogeneous matrix in logical column order. Consumers
    /// select operator coefficients only after retaining this complete evidence.
    #[must_use]
    pub fn native(&self) -> &SourceJacobian {
        &self.0.native
    }

    /// Complete cumulative usage, including all work preceding this derivative.
    #[must_use]
    pub fn usage(&self) -> SourceJacobianUsage {
        self.0.usage
    }

    /// Retained allowance to reserve while lowering or evaluating this derivative.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }

    /// Derive from the sealed original key under exactly the caller's ledger.
    fn derive(
        cas: &mut Cas,
        key: &ReciprocalJacobianKey,
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        let before = budget.usage();
        let native = budget.with_retained_payload(key.bytes, support::AT, |budget| {
            cas.source_jacobian(&key.native, budget)
        })?;
        let mut bytes = key.bytes.checked_add(1024).ok_or(ReadingError::Shape)?;
        for witness in std::iter::once(native.witness()).chain(native.representation()) {
            let size =
                u64::try_from(witness.exported_byte_len()).map_err(|_| ReadingError::Shape)?;
            bytes = bytes
                .checked_add(size.checked_mul(8).ok_or(ReadingError::Shape)?)
                .ok_or(ReadingError::Shape)?;
        }
        budget.check(BudgetKind::StagedPayloadBytes, bytes, support::AT)?;
        budget.check(BudgetKind::OutputBytes, bytes, support::AT)?;
        Ok(Self(Rc::new(Evidence {
            key: key.clone(),
            native,
            usage: SourceJacobianUsage {
                before,
                after: budget.usage(),
            },
            bytes,
        })))
    }
}
