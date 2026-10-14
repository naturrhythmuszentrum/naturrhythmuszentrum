// SPDX-License-Identifier: AGPL-3.0-only
//! Typed requests, coverage validation and explicit bounded execution outcomes.

use std::collections::BTreeSet;

use crate::budget::{Budget, BudgetKind, BudgetUsage, Limits, LimitsOverride};
use crate::encoding::{self, Encoder};
use crate::error::{Fault, FaultKind, InvalidCode, Location};
use crate::graph::{Cas, Draft, Handle, LogicalId, Reference, State, size};
use crate::model::{AtomicSort, Field, Operation, Operator, Sort};
use crate::traversal::TraversalReceipt;

/// A mathematical task supplied with complete occurrence-coverage receipts.
#[derive(Clone, Debug)]
pub struct Request {
    /// Requested operation and ordered variables.
    pub operation: Operation,
    /// Immutable full mathematical context.
    pub context: Handle,
    /// Ordered original/generated occurrence roots.
    pub inputs: Vec<Handle>,
    /// One sealed coverage receipt for each corresponding root.
    pub receipts: Vec<TraversalReceipt>,
    /// Explicit optional overrides for every named service limit.
    pub limits: LimitsOverride,
}

/// An explicit algorithm-domain exclusion, rather than a mathematical refutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedReason {
    /// The requested executable coefficient/calculus field is not admitted.
    Field,
}

/// No context admission or coverage receipt establishes a model of its hypotheses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextConsistency {
    /// Recognized contradictions were rejected; general consistency remains unproved.
    Unknown,
}

/// Checked results and honest deferred, invalid or exhausted outcomes.
#[derive(Clone, Debug)]
pub enum Outcome {
    /// A replayed mathematical result with no residual domain obligations.
    Verified {
        /// Retained graph-owned request.
        request: Handle,
        /// Checked witness and heuristic accounting.
        result: crate::search::SearchResult,
        /// Generation work, separate from the result's checker usage.
        usage: BudgetUsage,
    },
    /// A replayed result on an explicitly restricted domain.
    Conditional {
        /// Retained graph-owned request.
        request: Handle,
        /// Checked witness retaining every unresolved guard.
        result: crate::search::SearchResult,
        /// Generation work.
        usage: BudgetUsage,
    },
    /// A valid first-class request whose derivation algorithm remains pending.
    Unresolved {
        /// Graph-owned request record.
        request: Handle,
        /// Explicit absence of a general consistency proof.
        consistency: ContextConsistency,
        /// A precise pending-fragment or refuted-domain blocker when available.
        blocker: Option<Fault>,
        /// Actual consumed work.
        usage: BudgetUsage,
    },
    /// A valid represented request outside the admitted executable domain.
    Unsupported {
        /// Graph-owned request record.
        request: Handle,
        /// Precise fragment exclusion.
        reason: UnsupportedReason,
        /// Explicit absence of a general consistency proof.
        consistency: ContextConsistency,
        /// Actual consumed work.
        usage: BudgetUsage,
    },
    /// An invalid input, without publishing a request.
    Invalid {
        /// Structured failure and location.
        fault: Fault,
        /// Work consumed before refusal.
        usage: BudgetUsage,
    },
    /// A resource limit, optionally retaining an already replayed best result.
    Exhausted {
        /// Only a previously checked result may survive later exhaustion.
        best: Option<crate::search::SearchResult>,
        /// Independent checker work consumed before refusal.
        replay_usage: BudgetUsage,
        /// Exact limit/range failure.
        fault: Fault,
        /// Work consumed before refusal.
        usage: BudgetUsage,
    },
}

impl Cas {
    /// Execute a bounded mathematical request under the default positive-cost policy.
    pub fn run(&mut self, request: &Request) -> Outcome {
        self.run_with_policy(request, &crate::search::SearchPolicy::default())
    }

    /// Check receipts before admitting an operation request record.
    pub(crate) fn prepare_request(
        &mut self,
        request: &Request,
        limits: Limits,
        budget: &mut Budget,
    ) -> Result<(Handle, bool), Fault> {
        let at = Location::new("request preparation");
        let context = self.checked(&request.context)?;
        let mut encoded = Encoder::new(budget, at);
        encoding::operation(&request.operation, &mut encoded)?;
        let operation_bytes = size(encoded.finish().len(), at)?;
        let input_count = size(request.inputs.len(), at)?;
        let staging = input_count
            .checked_mul(48)
            .and_then(|bytes| bytes.checked_add(operation_bytes))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?;
        budget.check(BudgetKind::StagedPayloadBytes, staging, at)?;
        budget.work(staging, at)?;
        if request.inputs.len() != request.receipts.len() {
            return Err(Fault::invalid(InvalidCode::Receipt, at));
        }
        budget.work(size(request.inputs.len(), at)?, at)?;
        for (root, receipt) in request.inputs.iter().zip(&request.receipts) {
            self.checked(root)?;
            if *root != receipt.root {
                return Err(Fault::invalid(InvalidCode::Receipt, at));
            }
            budget.check(BudgetKind::OriginalOccurrences, receipt.visited, at)?;
        }
        let field = &self.state.context(context)?.environment.field;
        let unsupported = match &request.operation {
            Operation::Differentiate(_) | Operation::Jacobian(_) => *field != Field::Real,
            Operation::SquareFree(_) => !matches!(field, Field::Rational | Field::Real),
            _ => false,
        };
        let mut inputs = Vec::new();
        inputs
            .try_reserve_exact(request.inputs.len())
            .map_err(|_| Fault::allocation(at))?;
        inputs.extend(request.inputs.iter().cloned().map(Reference::Existing));
        let handles = self.admit(
            &[Draft::Request {
                operation: request.operation.clone(),
                context: Reference::Existing(request.context.clone()),
                inputs,
                limits,
            }],
            budget,
        )?;
        let record = handles
            .first()
            .cloned()
            .ok_or_else(|| Fault::invalid(InvalidCode::Reference, at))?;
        Ok((record, unsupported))
    }
}

/// Classify a typed failure without trusting a searcher's success flag.
pub(crate) const fn failure(fault: Fault, usage: BudgetUsage) -> Outcome {
    match fault.kind {
        FaultKind::Invalid(_) => Outcome::Invalid { fault, usage },
        FaultKind::Exhausted { .. } => Outcome::Exhausted {
            fault,
            usage,
            best: None,
            replay_usage: BudgetUsage {
                work: 0,
                arithmetic_steps: 0,
                condition_steps: 0,
            },
        },
    }
}

/// Validate a request's typed operands independently of execution availability.
pub(crate) fn validate(
    state: &State,
    operation: &Operation,
    context: LogicalId,
    inputs: &[LogicalId],
    limits: &Limits,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let at = Location::new("request validation");
    limits.validate()?;
    let context = state.context(context)?;
    let mut encoded = Encoder::new(budget, at);
    encoding::operation(operation, &mut encoded)?;
    let length = size(encoded.finish().len(), at)?;
    budget.work(length, at)?;
    let expected = match operation {
        Operation::Equal => Some(2),
        Operation::Jacobian(_) | Operation::Solve => None,
        _ => Some(1),
    };
    if inputs.is_empty() || expected.is_some_and(|expected| inputs.len() != expected) {
        return Err(Fault::invalid(InvalidCode::Sort, at));
    }
    let first = inputs
        .first()
        .copied()
        .ok_or_else(|| Fault::invalid(InvalidCode::Sort, at))?;
    let first_sort = &state.expression(state.occurrence(first)?.expression)?.sort;
    for input in inputs {
        let expression = state.expression(state.occurrence(*input)?.expression)?;
        budget.work(size(context.header.len(), at)?, at)?;
        if !expression.scope.is_empty()
            || state.context(expression.context)?.header != context.header
        {
            return Err(Fault::invalid(InvalidCode::Scope, at));
        }
        let compatible = match operation {
            Operation::Equal => expression.sort == *first_sort,
            Operation::Integrate => matches!(expression.operator, Operator::Integral { .. }),
            Operation::Sum => matches!(expression.operator, Operator::Sum),
            Operation::Series => matches!(expression.operator, Operator::Series { .. }),
            Operation::Limit => matches!(expression.operator, Operator::Limit(_)),
            Operation::Solve => matches!(expression.operator, Operator::Set { .. }),
            Operation::Spectral => matches!(expression.operator, Operator::Spectral(_)),
            Operation::Differentiate(_) | Operation::Jacobian(_) | Operation::SquareFree(_) => {
                matches!(expression.sort, Sort::Value(AtomicSort::Scalar(_)))
            }
            _ => true,
        };
        if !compatible {
            return Err(Fault::invalid(InvalidCode::Sort, at));
        }
    }
    let variables = match operation {
        Operation::Differentiate(variable) | Operation::SquareFree(variable) => {
            std::slice::from_ref(variable)
        }
        Operation::Jacobian(variables) => variables.as_slice(),
        _ => &[],
    };
    let count = size(variables.len(), at)?;
    budget.check(
        BudgetKind::MatrixEntries,
        count
            .checked_mul(size(inputs.len(), at)?)
            .ok_or_else(|| Fault::capacity(BudgetKind::MatrixEntries, at))?,
        at,
    )?;
    let mut seen = BTreeSet::new();
    for variable in variables {
        budget.work(
            size(context.header.len(), at)?
                .checked_add(length)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?,
            at,
        )?;
        if !seen.insert(variable)
            || !context.environment.declarations.iter().any(|declaration| {
                declaration.key == *variable
                    && matches!(declaration.sort, Sort::Value(AtomicSort::Scalar(_)),)
            })
        {
            return Err(Fault::invalid(InvalidCode::Scope, at));
        }
    }
    if matches!(operation, Operation::Jacobian(_)) && variables.is_empty() {
        return Err(Fault::invalid(InvalidCode::Scope, at));
    }
    Ok(())
}
