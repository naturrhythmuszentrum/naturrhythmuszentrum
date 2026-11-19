// SPDX-License-Identifier: AGPL-3.0-only
//! Joined local attempts describe native attachments without creating mathematical cache authority.

use super::{LcFrequencyTarget, LcInputProduct, LcProductError, LcRequestedState};
use crate::{ExecutionAccount, LoanError};
use nrz_scalar::Scalar;
use std::sync::atomic::AtomicU64;
use tracing::Level;

/// Process-local observation identities are distinct even if a subscriber recycles span identifiers.
static NEXT_ATTEMPT: AtomicU64 = AtomicU64::new(1);

/// Fixed observation metadata, independent of whether Work is charged per cell or per traversal.
pub(super) struct Attempt {
    /// Original logical tuple.
    pub(super) cell: usize,
    /// Fixed requested quantity vocabulary.
    pub(super) demand: &'static str,
    /// Public selected read or private origin traversal.
    pub(super) consumer: &'static str,
    /// Original continuation account identity.
    pub(super) account: u64,
    /// Cumulative Work before the attempt.
    pub(super) work: u64,
}

impl<P: Scalar + 'static, const N: usize> LcInputProduct<'_, P, N> {
    /// Observe one complete public attempt without changing its result or original ledger charges.
    pub(super) fn observe_attempt<R>(
        &self,
        cell: usize,
        demand: &'static str,
        account: &mut ExecutionAccount,
        run: impl FnOnce(&mut ExecutionAccount) -> Result<R, LcProductError>,
        state: impl FnOnce(&R) -> LcRequestedState<P>,
    ) -> Result<R, LcProductError> {
        let attempt = Attempt {
            cell,
            demand,
            consumer: "requested",
            account: account.identity,
            work: account.usage().work,
        };
        self.observe_prepaid(
            &attempt,
            || {
                let result = run(account);
                (result, account.usage().work)
            },
            state,
        )
    }

    /// The caller owns any Work phase; this boundary only observes its supplied attempt.
    pub(super) fn observe_prepaid<R>(
        &self,
        attempt: &Attempt,
        run: impl FnOnce() -> (Result<R, LcProductError>, u64),
        state: impl FnOnce(&R) -> LcRequestedState<P>,
    ) -> Result<R, LcProductError> {
        let Attempt {
            cell,
            demand,
            consumer,
            account,
            work,
        } = *attempt;
        let span = tracing::span!(target: "nrz::lc", Level::TRACE, "nrz.barrier",
            family = "lc", revision = 2_u64, owner = "nrz-matrix",
            question = "frequency", operation = "positive-domain", consumer, demand,
            scope = "local-represented-inputs", account,
            schema = self.cells.readers.inventory.input(0).map(crate::loan::lc_shape::LcInputShape::specialization_key),
            inductance = self.positions[0], capacitance = self.positions[1],
            axes = ?self.cells.readers.axes, cell, format = ?P::PRECISION,
            arithmetic_revision = self.context.revision(),
            attempt = tracing::field::Empty, work,
            outcome = tracing::field::Empty, reason = tracing::field::Empty);
        if span.is_disabled() {
            return run().0;
        }
        let Ok(attempt) = crate::heap::identity::allocate_identity(&NEXT_ATTEMPT) else {
            tracing::event!(name: "nrz.observer.loss", target: "nrz::lc", parent: &span,
                Level::TRACE, owner = "nrz-matrix", reason = "attempt-identity-exhausted");
            span.record("outcome", "incomplete");
            return run().0;
        };
        span.record("attempt", attempt);
        span.in_scope(|| {
            decision(attempt, "attempted", "none", "none", "none", work);
            self.claims(attempt);
            let (result, work) = run();
            let (outcome, target, reason, continuation) = match &result {
                Ok(result) => outcome(state(result)),
                Err(error) => ("refused", "none", refusal(error), "none"),
            };
            span.record("work", work);
            span.record("outcome", outcome);
            span.record("reason", reason);
            decision(attempt, outcome, target, reason, continuation, work);
            result
        })
    }

    /// Complete admitted metadata is available without inspecting any numerical plane or mask bit.
    fn claims(&self, attempt: u64) {
        for (position, reader) in self.cells.readers.readers.iter().enumerate() {
            if let Some(reader) = reader {
                tracing::event!(name: "nrz.claim", target: "nrz::lc", Level::TRACE,
                    family = "lc", attempt, claim = "admitted-input-attachment", position,
                    schema = self.cells.readers.inventory.input(position).map(crate::loan::lc_shape::LcInputShape::specialization_key),
                    account = reader.account, occupation = ?reader.bundle.identity,
                    companions = ?reader.bundle.qualified,
                    source = ?reader.bundle.origin.evidence.as_ref().map(|origin| origin.source_identity),
                    source_companions = ?reader.bundle.origin.evidence.as_ref().map(|origin| origin.attachments),
                    error_scope = ?reader.numerical_error_scope());
            }
        }
    }
}

/// Fixed event fields join attempted and terminal observations inside the same barrier span.
fn decision(
    attempt: u64,
    outcome: &'static str,
    target: &'static str,
    reason: &'static str,
    continuation: &'static str,
    work: u64,
) {
    tracing::event!(name: "nrz.decision", target: "nrz::lc", Level::TRACE,
        family = "lc", attempt, outcome, target, reason, continuation, work);
}

/// A local native outcome makes no full-feasibility or exact-equality assertion.
const fn outcome<P: Scalar>(
    state: LcRequestedState<P>,
) -> (&'static str, &'static str, &'static str, &'static str) {
    match state {
        LcRequestedState::Ready { .. } => ("completed", "none", "positive", "local-ready"),
        LcRequestedState::Undecided(value) => (
            "undecided",
            target(value),
            "no-strict-positive-separation",
            "retain-unresolved-inputs",
        ),
        LcRequestedState::Invalid(value) => (
            "invalid",
            target(value),
            "strict-negative-box",
            "invalid-domain",
        ),
    }
}

/// Targets are original positional roles or their local product/root, never numerical values.
const fn target(target: LcFrequencyTarget) -> &'static str {
    match target {
        LcFrequencyTarget::Inductance => "inductance",
        LcFrequencyTarget::Capacitance => "capacitance",
        LcFrequencyTarget::Product => "product",
        LcFrequencyTarget::Root => "root",
    }
}

/// Preserve the relevant refusal categories without formatting native readings or error payloads.
const fn refusal(error: &LcProductError) -> &'static str {
    match error {
        LcProductError::Loan(LoanError::ForeignAccount) => "foreign-account",
        LcProductError::Loan(LoanError::CoverageMismatch) => "coverage",
        LcProductError::Loan(LoanError::LayoutMismatch) => "capacity-or-layout",
        LcProductError::Loan(LoanError::Budget(_)) => "budget",
        LcProductError::Loan(LoanError::UnsupportedContract) => "unsupported-contract",
        LcProductError::Loan(LoanError::CountOverflow) => "count-overflow",
        LcProductError::Loan(_) => "loan",
        LcProductError::Kernel(_) => "arithmetic",
    }
}

/// Isolated allocation checks never mutate the production observation counter.
#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Attempts stop before wrap and repeated exhaustion leaves the counter unchanged.
    /// ´test:unit:loan-lc-sign-attempt-exhaustion´
    #[test]
    fn loan_lc_sign_attempt_exhaustion() {
        let counter = AtomicU64::new(u64::MAX - 1);
        assert_eq!(
            crate::heap::identity::allocate_identity(&counter),
            Ok(u64::MAX - 1)
        );
        for _ in 0..2 {
            assert_eq!(
                crate::heap::identity::allocate_identity(&counter),
                Err(crate::HeapError::CounterExhausted)
            );
            assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
        }
    }
}
