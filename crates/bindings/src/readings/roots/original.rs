// SPDX-License-Identifier: AGPL-3.0-only
//! Original request admission delegates exact source and divisor validation to CAS.

use super::{AT, ReadingError};
use nrz_cas::{
    Cas, Handle, Reference,
    limits::{Budget, BudgetKind, BudgetUsage},
    request::{Request, Task},
    solving::{ExclusionPolicy, SolveGoal},
    syntax::Draft,
};
use std::rc::Rc;

/// A complete-root task with sealed original coverage and CAS-validated exact endpoints.
#[derive(Clone, Debug)]
pub struct RootProblem(Rc<Original>);

/// One retained original declaration, shared across requests and retries without reconstruction.
#[derive(Debug)]
struct Original {
    /// Complete caller request, including source receipts and explicit search policy.
    request: Request,
    /// Admitted request record ties the original occurrences to the solve problem.
    record: Handle,
    /// Exact solve problem, never replaced by its coefficient normalization.
    problem: Handle,
    /// Cumulative snapshots around original request validation.
    usage: [BudgetUsage; 2],
    /// Conservative retained metadata and supplied witness capacity.
    bytes: u64,
}

impl RootProblem {
    /// Check all original receipts and admit the exact complete-root request with CAS.
    ///
    /// CAS validates the original predicate and every ordered divisor occurrence, including
    /// operands beneath cancellation and zero. A receipt establishes traversal, not physical
    /// validity; callers must retain their independently checked physical lowering as well.
    /// # Errors
    /// Refuses noncomplete tasks, dropped exclusions, mismatched or generated receipts,
    /// changed original syntax, foreign handles and exhausted caller resources.
    pub fn check(
        cas: &mut Cas,
        request: Request,
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        let before = budget.usage();
        let Task::Solve(specification) = &request.task else {
            return Err(ReadingError::Definition);
        };
        if specification.goal != SolveGoal::Complete
            || specification.exclusions != ExclusionPolicy::PreserveOriginal
        {
            return Err(ReadingError::Definition);
        }
        let problem = specification.problem.clone();
        let bytes = retained(&request, budget)?;
        let record =
            budget.with_retained_payload(bytes, AT, |budget| admit(cas, &request, budget))?;
        Ok(Self(Rc::new(Original {
            request,
            record,
            problem,
            usage: [before, budget.usage()],
            bytes,
        })))
    }

    /// Original task, context, source roots, traversal receipts and explicit search policy.
    #[must_use]
    pub fn request(&self) -> &Request {
        &self.0.request
    }

    /// CAS-validated request record; admission alone supplies no root theorem.
    #[must_use]
    pub fn record(&self) -> &Handle {
        &self.0.record
    }

    /// Exact original solve problem, including the complete divisor occurrence inventory.
    #[must_use]
    pub fn problem(&self) -> &Handle {
        &self.0.problem
    }

    /// Caller usage before and after original-request admission, without resetting any counter.
    #[must_use]
    pub fn usage(&self) -> [BudgetUsage; 2] {
        self.0.usage
    }

    /// Native retained request allowance, independent of graph-owned syntax storage.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}

/// Bound request copies before allocating the CAS admission draft.
fn retained(request: &Request, budget: &mut Budget) -> Result<u64, ReadingError> {
    let count = request
        .inputs
        .len()
        .checked_add(request.premises.len())
        .ok_or(ReadingError::Shape)?;
    let mut bytes = u64::try_from(count)
        .map_err(|_| ReadingError::Shape)?
        .checked_mul(256)
        .and_then(|size| size.checked_add(2048))
        .ok_or(ReadingError::Shape)?;
    for input in &request.inputs {
        budget.work(1, AT)?;
        if input.receipt.root() != &input.root || !input.receipt.is_original() {
            return Err(ReadingError::Definition);
        }
        budget.check(BudgetKind::OriginalOccurrences, input.receipt.visited(), AT)?;
    }
    for premise in &request.premises {
        budget.work(1, AT)?;
        bytes = bytes
            .checked_add(
                u64::try_from(premise.exported_byte_len())
                    .map_err(|_| ReadingError::Shape)?
                    .checked_mul(8)
                    .ok_or(ReadingError::Shape)?,
            )
            .ok_or(ReadingError::Shape)?;
    }
    budget.check(BudgetKind::OutputBytes, bytes, AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    Ok(bytes)
}

/// The ordinary CAS request validator authenticates original syntax and exact denominator order.
fn admit(cas: &mut Cas, request: &Request, budget: &mut Budget) -> Result<Handle, ReadingError> {
    let draft = Draft::Request {
        task: request.task.clone(),
        context: Reference::Existing(request.context.clone()),
        inputs: request
            .inputs
            .iter()
            .map(|input| Reference::Existing(input.root.clone()))
            .collect(),
        premises: request
            .premises
            .iter()
            .map(|proof| Reference::Existing(proof.proof().clone()))
            .collect(),
        limits: Box::new(budget.limits().clone()),
    };
    cas.admit(&[draft], budget)?
        .pop()
        .ok_or(ReadingError::Definition)
}
