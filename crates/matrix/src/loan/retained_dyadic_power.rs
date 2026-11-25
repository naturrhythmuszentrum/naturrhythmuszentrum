// SPDX-License-Identifier: AGPL-3.0-only
//! One permanently encapsulated CAS authority is shared only by separately funded uses.

mod admission;
mod inventory;
mod retention;

use super::{
    LoanError,
    registry::{CleanupReceipt, Token},
};
use crate::prepared_dyadic_power::OwnedParts;
use nrz_bindings::readings::algebra::Cas;
use nrz_scalar::{ArithmeticContext, DyadicPowerFailure};
use std::rc::Rc;

pub use inventory::RetainedDyadicPowerInventory;

/// Preparation and managed ownership refusals return the consumed producing CAS.
#[derive(Debug, thiserror::Error)]
pub enum RetainedDyadicPowerError {
    /// Original correspondence, exact enclosure or numerical admission refused.
    #[error("retained dyadic power preparation: {0}")]
    Preparation(#[from] crate::PreparedDyadicPowerError),
    /// Managed capacity, original-account or private allocation admission refused.
    #[error("retained dyadic power ownership: {0}")]
    Ownership(#[from] LoanError),
}

/// Runtime identity has no descriptor or statement-key construction authority.
type LawIdentity = super::retained_expm1::LawIdentity;

/// Private immutable native/domain/key specialization captured with the actual returned proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Seal {
    /// Exact represented endpoints, including signed-zero bits.
    domain: [u64; 2],
    /// Canonical dyadic numerator and root degree.
    exponent: (u64, u32),
    /// Argument and arithmetic revisions plus separate-steps tag.
    revisions: [u32; 3],
}

/// Evidence and native program end their lifetime before their producing CAS.
#[derive(Debug)]
struct FrozenLaw {
    /// Actual checked bound and immutable native argument.
    parts: OwnedParts,
    /// Private correspondence seal established before owner allocation.
    seal: Seal,
    /// Separate complete logical graph inventory observed before encapsulation.
    graph_bytes: u64,
    /// No public graph mutation or extraction route exists.
    #[expect(
        dead_code,
        reason = "retaining the producing CAS is the authority boundary"
    )]
    graph: Cas,
}

/// Shared owner shell and its unique retained evidence/key allowance.
#[derive(Debug)]
pub(in crate::loan) struct PreparedDyadicPowerOwner {
    /// Filled only after all fallible managed allocation has succeeded.
    payload: Option<FrozenLaw>,
    /// Original heap, account and nonwrapping owner identity.
    identity: LawIdentity,
    /// CAS-owned logical checked-enclosure allowance.
    evidence: usize,
    /// Actual separately inventoried retained proof and key buffer capacity.
    keys: usize,
    /// Unique owner debit and expiration witness identity.
    token: Token,
    /// Prepaid owner shell destruction entitlement.
    cleanup: CleanupReceipt,
}

/// Each use owns exactly one strong edge and one independently charged header.
#[derive(Debug)]
pub(in crate::loan) struct PreparedDyadicPowerUseRecord {
    /// Empty only during private initial-shell preparation.
    owner: Option<Rc<PreparedDyadicPowerOwner>>,
    /// Exact immutable use pairing and seal.
    binding: Binding,
    /// Unique use debit and expiration witness identity.
    token: Token,
    /// Prepaid header and edge release entitlement.
    cleanup: CleanupReceipt,
}

/// One noncloneable, separately funded use of a frozen original dyadic power.
#[derive(Debug)]
pub struct RetainedDyadicPower {
    /// No shared pointer or weak upgrade escapes the owner module.
    record: Rc<PreparedDyadicPowerUseRecord>,
}

impl RetainedDyadicPower {
    /// Evaluate only the admitted native point schedule through this funded borrow.
    /// # Errors
    /// Preserves the scalar environment, below-normal, domain and certification causes.
    pub fn evaluate(
        &self,
        x: f64,
        arithmetic: &ArithmeticContext,
    ) -> Result<crate::DyadicPowerEvaluation<'_>, DyadicPowerFailure> {
        self.frozen()
            .map_err(|_| DyadicPowerFailure::MalformedPlan)?
            .parts
            .evaluate(x, arithmetic)
    }

    /// Inspect the actual checked original while borrowing this funded use.
    /// # Errors
    /// Refuses an incomplete private shell that public construction cannot expose.
    pub fn authority(&self) -> Result<&nrz_bindings::DyadicPowerPreparation, LoanError> {
        Ok(&self.frozen()?.parts.authority)
    }

    /// Exact represented native domain admitted with the original authority.
    /// # Errors
    /// Refuses an incomplete private shell.
    pub fn domain(&self) -> Result<[f64; 2], LoanError> {
        Ok(self.frozen()?.seal.domain.map(f64::from_bits))
    }

    /// Borrow the shared owner without creating another retaining edge.
    fn owner(&self) -> Result<&PreparedDyadicPowerOwner, LoanError> {
        self.record.owner.as_deref().ok_or(LoanError::WrongLoan)
    }

    /// Borrow the fully paired evidence and native program.
    fn frozen(&self) -> Result<&FrozenLaw, LoanError> {
        self.owner()?.payload.as_ref().ok_or(LoanError::WrongLoan)
    }
}

/// One exact use identity and immutable native/proof specialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Binding {
    /// Original heap/account/owner identity.
    identity: LawIdentity,
    /// Unique funded use identity.
    use_identity: u64,
    /// Sealed numerical specialization.
    seal: Seal,
}

impl Binding {
    /// Capture paired metadata without acquiring or upgrading an edge.
    fn of(law: &RetainedDyadicPower) -> Result<Self, LoanError> {
        Ok(Self {
            identity: law.owner()?.identity,
            use_identity: law.record.token.key,
            seal: law.frozen()?.seal,
        })
    }
}

/// Reuse the heap's nonwrapping allocator for owner, use and receipt identities.
fn identity() -> Result<u64, LoanError> {
    Ok(crate::heap::identity::identity()?)
}

#[cfg(test)]
pub(super) mod tests;
