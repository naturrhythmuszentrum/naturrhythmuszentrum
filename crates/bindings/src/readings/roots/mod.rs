// SPDX-License-Identifier: AGPL-3.0-only
//! Complete root coverage is replayed against the original covered request, never a reduced equation.

mod original;

pub use original::RootProblem;

use super::ReadingError;
use nrz_cas::{
    Cas, Handle, Location,
    domain::Layer,
    limits::{Budget, BudgetKind, BudgetUsage},
    proof::{Judgment, Statement, VerifiedWitness},
    solving::{SolveStatement, StatementCoverage},
};
use std::rc::Rc;

/// Location shared by original-request admission and coverage retention.
const AT: Location = Location::new("original root coverage");

/// Complete cumulative snapshots distinguish expected-claim construction from proof replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootReplayUsage {
    /// Caller usage before constructing the expected judgment.
    pub before: BudgetUsage,
    /// Caller usage immediately before independent replay.
    pub expected: BudgetUsage,
    /// Caller usage after successful replay, including every earlier caller operation.
    pub replayed: BudgetUsage,
}

/// Independently replayed complete root/factor evidence for one original problem and proposed set.
#[derive(Clone, Debug)]
pub struct RootCoverage(Rc<Evidence>);

/// Shared evidence has no numerical operating point or sampled premise.
#[derive(Debug)]
struct Evidence {
    /// Original occurrences, sealed source receipts and exact task.
    original: RootProblem,
    /// Proposed syntax becomes covered only through the matching checked theorem.
    set: Handle,
    /// Complete scoped root/factor theorem, including every original domain premise.
    witness: VerifiedWitness,
    /// Complete caller-ledger snapshots for this admission.
    usage: RootReplayUsage,
    /// Conservative native evidence and original-request retention.
    bytes: u64,
}

impl RootCoverage {
    /// Rebuild the complete expected claim from the original request and proposed set, then replay.
    ///
    /// The caller supplies an explicit cumulative budget; this method creates no ledger and
    /// changes no ceiling. The original request's search overrides do not select replay policy.
    /// Physical interpretation and numerical enclosure remain separate lowering obligations.
    /// # Errors
    /// Refuses changed problems, contexts, strata, divisor inventories, incomplete roots,
    /// conditional evidence, foreign handles, invalid proof bytes and exhausted resources.
    pub fn replay(
        cas: &mut Cas,
        original: &RootProblem,
        set: Handle,
        bytes: &[u8],
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        let before = budget.usage();
        let retained = original
            .retained_payload_bytes()
            .checked_add(
                u64::try_from(bytes.len())
                    .map_err(|_| ReadingError::Shape)?
                    .checked_mul(8)
                    .ok_or(ReadingError::Shape)?,
            )
            .and_then(|size| size.checked_add(1024))
            .ok_or(ReadingError::Shape)?;
        budget.check(BudgetKind::OutputBytes, retained, AT)?;
        let (witness, prepared) = budget.with_retained_payload(retained, AT, |budget| {
            let judgment = expected(cas, original, &set, budget)?;
            let prepared = budget.usage();
            let witness = cas.replay(bytes, &judgment, budget)?;
            Ok::<_, ReadingError>((witness, prepared))
        })?;
        if !witness.is_discharged() || !witness.hypotheses().is_empty() {
            return Err(ReadingError::Definition);
        }
        let usage = RootReplayUsage {
            before,
            expected: prepared,
            replayed: budget.usage(),
        };
        Ok(Self(Rc::new(Evidence {
            original: original.clone(),
            set,
            witness,
            usage,
            bytes: retained,
        })))
    }

    /// Exact covered source request, including its original occurrence receipts.
    #[must_use]
    pub fn original(&self) -> &RootProblem {
        &self.0.original
    }

    /// Proposed root set whose complete coverage was independently replayed.
    #[must_use]
    pub fn set(&self) -> &Handle {
        &self.0.set
    }

    /// Original checked root/factor judgment and complete portable proof closure.
    #[must_use]
    pub fn witness(&self) -> &VerifiedWitness {
        &self.0.witness
    }

    /// Full cumulative ledger snapshots, including charges preceding this replay.
    #[must_use]
    pub fn usage(&self) -> RootReplayUsage {
        self.0.usage
    }

    /// Retained native request and witness allowance for nested consumer operations.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }

    /// Export the original coverage proof under the same caller-owned policy.
    /// # Errors
    /// Refuses foreign evidence and exhausted cumulative Work or payload allowances.
    pub fn export(&self, cas: &Cas, budget: &mut Budget) -> Result<Vec<u8>, ReadingError> {
        Ok(budget.with_retained_payload(self.0.bytes, AT, |budget| {
            cas.export_witness(self.witness(), budget)
        })?)
    }
}

/// Neither an inspected proof statement nor a normalized polynomial chooses these endpoints.
fn expected(
    cas: &Cas,
    original: &RootProblem,
    set: &Handle,
    budget: &mut Budget,
) -> Result<Judgment, ReadingError> {
    budget.work(1, AT)?;
    Ok(Judgment {
        context: original.request().context.clone(),
        layer: Layer::Logic,
        space: None,
        residuals: Vec::new(),
        statement: Statement::Solve(Box::new(SolveStatement::Coverage(StatementCoverage {
            problem: original.problem().clone(),
            set: set.clone(),
            region: cas.admissible_region(&original.request().context)?,
        }))),
    })
}
