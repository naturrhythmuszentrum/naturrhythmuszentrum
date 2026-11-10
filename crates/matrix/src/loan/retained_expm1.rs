// SPDX-License-Identifier: AGPL-3.0-only
//! Preparation provenance and charged lifetime edges retain one permanently encapsulated CAS.

mod admission;
mod inventory;
mod retention;

use super::{
    LoanError,
    registry::{CleanupReceipt, Token},
};
use crate::prepared_expm1::OwnedParts;
use nrz_bindings::readings::algebra::{Cas, enclosure::prepared_expm1::Prepared};
use nrz_scalar::{ArithmeticContext, Expm1Failure, Expm1Input};
use std::rc::Rc;

pub use inventory::RetainedExpm1Inventory;

/// Preparation or managed ownership admission refused; the constructor returns its producing CAS.
#[derive(Debug, thiserror::Error)]
pub enum RetainedExpm1Error {
    /// Exact preparation, represented-domain correspondence or native lowering refused.
    #[error("retained expm1 preparation: {0}")]
    Preparation(#[from] crate::PreparedExpm1Error),
    /// Managed accounting, identity or private allocation admission refused.
    #[error("retained expm1 ownership: {0}")]
    Ownership(#[from] LoanError),
}
/// Sealed runtime identity has no descriptor, mask, producer or statement-key authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::loan) struct LawIdentity {
    /// Owning heap scope.
    pub(in crate::loan) scope: u64,
    /// Original execution account.
    pub(in crate::loan) account: u64,
    /// Fresh nonwrapping owner identity.
    pub(in crate::loan) owner: u64,
}
/// Constructor provenance binds exact authority, native schedule and represented domain once.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Seal {
    /// Exact represented endpoint bits, including signed zero.
    domain: [u64; 2],
    /// Closed law, schedule, lowering and arithmetic revisions.
    revisions: [u32; 4],
}
/// The evidence/program fields are destroyed before the graph that authorizes them.
#[derive(Debug)]
struct FrozenLaw {
    /// The sole returned authority box and inline native coefficients.
    parts: OwnedParts,
    /// Admitted domain and revisions cannot be replaced separately.
    seal: Seal,
    /// Logical graph scope, recorded before permanent encapsulation.
    graph_bytes: u64,
    /// Exclusive encapsulation offers no graph accessor, extraction or mutation route.
    #[expect(
        dead_code,
        reason = "ownership keeps the producing graph alive; native reuse never accesses it"
    )]
    graph: Cas,
    /// Observe destruction after evidence, program and graph, without retaining them.
    #[cfg(test)]
    _destroyed: tests::Destroyed,
}
/// Private shared owner shell is filled before any strong edge or weak witness escapes.
#[derive(Debug)]
pub(in crate::loan) struct PreparedExpm1Owner {
    /// Actual immutable law; absent only during private shell preparation.
    payload: Option<FrozenLaw>,
    /// Runtime identity independent of statement content.
    identity: LawIdentity,
    /// Checked exact-evidence allowance, converted before private shell allocation.
    evidence: usize,
    /// Unique owner/evidence allowance debit.
    token: Token,
    /// Prepaid wrapper destruction; exact internals keep their symbolic scope.
    cleanup: CleanupReceipt,
}
/// Each use owns exactly one charged strong edge to the same immutable owner.
#[derive(Debug)]
pub(in crate::loan) struct PreparedExpm1UseRecord {
    /// Empty only while preparing the initial use alongside the owner shell.
    owner: Option<Rc<PreparedExpm1Owner>>,
    /// This use's immutable expected pairing keeps bundle attachments pointer-sized.
    binding: Binding,
    /// Unique use allocation and expiration record.
    token: Token,
    /// Prepaid header and edge release, independent of heap/account lifetime.
    cleanup: CleanupReceipt,
}
/// One non-cloneable, separately funded use of an immutable prepared `expm1` law.
///
/// Only `Heap::prepare_retained_expm1` constructs a law. Further uses require
/// `Heap::retain_retained_expm1` on the original account. Dropping a use releases
/// its edge without charging Work; reconciliation removes expired debit records.
#[derive(Debug)]
pub struct RetainedExpm1 {
    /// No shared pointer or weak upgrade escapes this module.
    record: Rc<PreparedExpm1UseRecord>,
}
impl RetainedExpm1 {
    /// Native evaluation borrows this use and performs no exact operation or allocation.
    /// # Errors
    /// Preserves the native domain, target, environment and below-normal causes.
    pub fn evaluate(
        &self,
        input: Expm1Input,
        context: &ArithmeticContext,
    ) -> Result<crate::Expm1Evaluation<'_>, Expm1Failure> {
        self.frozen()
            .map_err(|_| Expm1Failure::MalformedPlan)?
            .parts
            .evaluate(input, context)
    }
    /// Inspect the original request, exact domain, checked proof and target immutably.
    /// # Errors
    /// Refuses an incomplete private shell; public construction never exposes one.
    pub fn authority(&self) -> Result<&Prepared, LoanError> {
        Ok(&self.frozen()?.parts.authority)
    }
    /// Exact represented endpoints admitted by this owner.
    /// # Errors
    /// Refuses an incomplete private shell.
    pub fn domain(&self) -> Result<[f64; 2], LoanError> {
        Ok(self.frozen()?.seal.domain.map(f64::from_bits))
    }
    /// Borrow the immutable owner without acquiring an edge.
    fn owner(&self) -> Result<&PreparedExpm1Owner, LoanError> {
        self.record.owner.as_deref().ok_or(LoanError::WrongLoan)
    }
    /// Borrow only admitted, fully paired parts.
    fn frozen(&self) -> Result<&FrozenLaw, LoanError> {
        self.owner()?.payload.as_ref().ok_or(LoanError::WrongLoan)
    }
}
/// A private consumer captures this exact use, not a statement-key approximation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::loan) struct Binding {
    /// Exact heap/account/owner.
    identity: LawIdentity,
    /// Exact transferred use identity.
    use_identity: u64,
    /// Immutable represented-domain and revision pairing.
    seal: Seal,
}
/// Private lifetime attachment is deliberately independent of numerical completion evidence.
#[derive(Debug)]
pub(in crate::loan) struct Attachment {
    /// The moved already funded edge survives ready, candidate, repair and publication.
    law: RetainedExpm1,
}
impl Attachment {
    /// No current or historical path can substitute another law with an equal statement.
    pub(in crate::loan) fn validate(&self) -> Result<(), LoanError> {
        let actual = Binding::of(&self.law)?;
        if actual != self.law.record.binding {
            return Err(LoanError::WrongLoan);
        }
        Ok(())
    }
    /// Diagnostic shared owner identity grants no construction or reacquisition authority.
    pub(in crate::loan) fn owner_identity(&self) -> u64 {
        self.law.record.binding.identity.owner
    }
    /// Validate live debit witnesses before current attachment publication or access.
    pub(in crate::loan) fn validate_current(&self, heap: &crate::Heap) -> Result<(), LoanError> {
        retention::authenticate(heap, &self.law)
    }
    /// One owner validation and two exact debit/index inspections.
    pub(in crate::loan) const WORK: usize = 3;
    /// A checked consumer moves its funded use only after validating the captured pairing.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "private seam awaits the checked prepared application"
        )
    )]
    pub(in crate::loan) fn prepare(
        law: RetainedExpm1,
        binding: Binding,
        heap: &crate::Heap,
        budget: &mut nrz_bindings::Budget,
    ) -> Result<Self, (LoanError, RetainedExpm1)> {
        // The second validation entitlement funds cancellation without fresh Work.
        let result = super::charge(budget, 2 * Self::WORK).and_then(|()| {
            if Binding::of(&law)? != binding {
                return Err(LoanError::WrongLoan);
            }
            retention::authenticate(heap, &law)
        });
        match result {
            Ok(()) => Ok(Self { law }),
            Err(error) => Err((error, law)),
        }
    }
}
impl Binding {
    /// Capturing metadata creates no new retaining edge.
    pub(in crate::loan) fn of(law: &RetainedExpm1) -> Result<Self, LoanError> {
        Ok(Self {
            identity: law.owner()?.identity,
            use_identity: law.record.token.key,
            seal: law.frozen()?.seal,
        })
    }
}

/// Reuse the heap's nonwrapping identity allocator for owners, uses and receipts.
fn identity() -> Result<u64, LoanError> {
    #[cfg(test)]
    tests::before_identity()?;
    Ok(crate::heap::identity::identity()?)
}

#[cfg(test)]
pub(super) mod tests;
