// SPDX-License-Identifier: AGPL-3.0-only
//! Typed requests, coverage validation and explicit bounded execution outcomes.

use std::collections::BTreeSet;

use crate::budget::{Budget, BudgetKind, BudgetUsage, Limits, LimitsOverride};
use crate::encoding::Encoder;
use crate::error::{Fault, FaultKind, InvalidCode, Location};
use crate::graph::{Cas, Draft, Handle, LogicalId, Reference, State, size};
use crate::model::{AtomicSort, Field, Operation, Operator, Sort};
use crate::task::Task;
use crate::traversal::TraversalReceipt;

/// One occurrence root paired with its own sealed complete traversal.
#[derive(Clone, Debug)]
pub struct CoveredInput {
    /// Original or generated occurrence root.
    pub root: Handle,
    /// Sealed receipt for exactly that root.
    pub receipt: TraversalReceipt,
}

/// A mathematical task supplied with complete occurrence-coverage receipts.
#[derive(Clone, Debug)]
pub struct Request {
    /// Requested operation and ordered variables.
    pub task: Task,
    /// Immutable full mathematical context.
    pub context: Handle,
    /// Ordered original/generated occurrence roots.
    pub inputs: Vec<CoveredInput>,
    /// Independently checked theorems supplied as explicit premises.
    pub premises: Vec<crate::proof_model::VerifiedWitness>,
    /// Explicit optional overrides for every named service limit.
    pub limits: LimitsOverride,
}

/// An explicit algorithm-domain exclusion, rather than a mathematical refutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedReason {
    /// The requested executable coefficient/calculus field is not admitted.
    Field,
    /// The expression is outside the univariate polynomial grammar.
    PolynomialFragment,
    /// A symbolic coefficient is outside the exact rational field.
    CoefficientField,
    /// The zero polynomial has undefined multiplicities.
    ZeroPolynomialMultiplicity,
}

/// A mathematical refusal returned before constructing an ill-sorted operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefusalReason {
    /// Complex roots have no admitted ordering.
    ComplexOrdering,
    /// Requested root choices are proved incompatible.
    IncompatibleSelection,
    /// A requested branch has been independently refuted.
    RefutedBranch,
    /// Zero completeness, simplicity or interval-map hypotheses fail.
    PullbackHypotheses,
    /// Required one-sided limits, polynomial coefficients or centre values disagree.
    AnalyticDisagreement,
    /// Distributions have no general point values.
    DistributionPointValue,
    /// The requested product has no admitted distribution meaning.
    DistributionProductUndefined,
    /// An action's exact test and distribution spaces differ.
    TestSpaceMismatch,
    /// A required definedness premise is false.
    RefutedDomain,
    /// The requested domain is empty.
    EmptyDomain,
    /// Applicable associations conflict.
    OverlapConflict,
    /// A finite cover has a witnessed hole.
    CoverageHole,
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
    /// The exact requested solution strength and representation have been proved.
    Solved {
        /// Retained original request.
        request: Handle,
        /// Independently replayed solution facts.
        answer: crate::solving::SolveAnswer,
        /// Monotone generation usage.
        usage: BudgetUsage,
    },
    /// Checked solution facts remain, but the requested target is not yet proved.
    SolvePartial {
        /// Retained original request.
        request: Handle,
        /// Only independently replayed partial facts.
        answer: crate::solving::SolveAnswer,
        /// Exact missing proof capability or obligation.
        reason: crate::solving::SolveReason,
        /// Monotone generation usage.
        usage: BudgetUsage,
    },
    /// A replayed statement proves the requested point derivative does not exist.
    Nonexistent {
        /// Retained exact request.
        request: Handle,
        /// Sealed mathematical evidence.
        result: crate::search::SearchResult,
        /// Generation usage.
        usage: BudgetUsage,
    },
    /// A derivative exists on some cells and has checked obstructions on others.
    Partial {
        /// Retained whole-domain request.
        request: Handle,
        /// Sealed derivative map.
        result: crate::search::SearchResult,
        /// Generation usage.
        usage: BudgetUsage,
    },
    /// Opaque region evidence cannot decide whether the requested whole domain is covered.
    UnknownTotality {
        /// Independently replayed partial facts.
        partial: Option<Box<crate::search::SearchResult>>,
        /// Retained request with its original coverage evidence.
        request: Handle,
        /// Exact unresolved region or definedness blocker.
        blocker: Fault,
        /// Monotone generation usage.
        usage: BudgetUsage,
    },
    /// A valid case operation lacks a supported mathematical premise.
    PendingPremise {
        /// Independently replayed partial facts.
        partial: Option<Box<crate::search::SearchResult>>,
        /// Retained request with its supplied evidence.
        request: Handle,
        /// Exact pending fragment or hypothesis.
        blocker: Fault,
        /// Monotone generation usage.
        usage: BudgetUsage,
    },
    /// A replayed mathematical result with no residual domain obligations.
    Proved {
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
        /// Missing solving proof, absent for other services.
        solve_reason: Option<crate::solving::SolveReason>,
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
    /// A witnessed domain contradiction prevents the requested theorem.
    Refused {
        /// Named mathematical operation or domain refusal.
        reason: RefusalReason,
        /// Retained request whose domain was refuted.
        request: Handle,
        /// Structured mathematical refusal.
        fault: Fault,
        /// Work consumed before refusal.
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
        /// Previously replayed solving facts, if any.
        solve_best: Option<Box<crate::solving::SolveAnswer>>,
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
        if let Task::Pullback { map_kind, .. } = &request.task {
            crate::pullback_wire::validate_intent(map_kind, budget)?;
        }
        let mut encoded = Encoder::new(budget, at);
        crate::task::encode(&request.task, &mut encoded)?;
        let operation_bytes = size(encoded.finish().len(), at)?;
        let input_count = size(request.inputs.len(), at)?;
        let staging = input_count
            .checked_mul(48)
            .and_then(|bytes| bytes.checked_add(operation_bytes))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?;
        budget.check(BudgetKind::StagedPayloadBytes, staging, at)?;
        budget.work(staging, at)?;
        for input in &request.inputs {
            self.checked(&input.root)?;
            if input.root != input.receipt.root {
                return Err(Fault::invalid(InvalidCode::Receipt, at));
            }
            if matches!(request.task, Task::ElaborateCases | Task::Solve(_))
                && !input.receipt.is_original()
            {
                return Err(Fault::invalid(InvalidCode::Receipt, at));
            }
            budget.check(BudgetKind::OriginalOccurrences, input.receipt.visited, at)?;
        }
        let mut premises = Vec::new();
        premises
            .try_reserve_exact(request.premises.len())
            .map_err(|_| Fault::allocation(at))?;
        for premise in &request.premises {
            self.checked(premise.proof())?;
            premises.push(Reference::Existing(premise.proof().clone()));
        }
        let field = &self.state.context(context)?.environment.field;
        let unsupported = match request.task.scalar() {
            Some(Operation::Differentiate(_) | Operation::Jacobian(_)) => *field != Field::Real,
            Some(Operation::SquareFree(_)) => !matches!(field, Field::Rational | Field::Real),
            _ => false,
        };
        let mut inputs = Vec::new();
        inputs
            .try_reserve_exact(request.inputs.len())
            .map_err(|_| Fault::allocation(at))?;
        inputs.extend(
            request
                .inputs
                .iter()
                .map(|input| Reference::Existing(input.root.clone())),
        );
        let handles = self.admit(
            &[Draft::Request {
                task: request.task.clone(),
                premises,
                context: Reference::Existing(request.context.clone()),
                inputs,
                limits: Box::new(limits),
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
            solve_best: None,
            fault,
            usage,
            best: None,
            replay_usage: BudgetUsage {
                work: 0,
                arithmetic_steps: 0,
                condition_steps: 0,
                case_arms: 0,
                region_nodes: 0,
                region_cells: 0,
                refinement_pairs: 0,
                local_contexts: 0,
                hypothesis_depth: 0,
                boundary_nodes: 0,
                jet_order: 0,
                jet_entries: 0,
                delta_order: 0,
                test_nodes: 0,
                pairing_steps: 0,
                pullback_zeros: 0,
            },
        },
    }
}

/// Validate a request's typed operands independently of execution availability.
pub(crate) fn validate(
    state: &State,
    task: &Task,
    context: LogicalId,
    inputs: &[LogicalId],
    limits: &Limits,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let at = Location::new("request validation");
    limits.validate()?;
    let context = state.context(context)?;
    let mut encoded = Encoder::new(budget, at);
    crate::task::encode(task, &mut encoded)?;
    let length = size(encoded.finish().len(), at)?;
    budget.work(length, at)?;
    validate_task(state, task, context, budget)?;
    for input in inputs {
        let (header, scope) =
            crate::descriptor::metadata(state, state.occurrence(*input)?.expression)?;
        if !scope.is_empty() || !crate::header::embeds(state, header, context.header_id, budget)? {
            return Err(Fault::invalid(InvalidCode::Scope, at));
        }
    }
    if let Task::Solve(spec) = task {
        return crate::solving::graph::request(state, spec, context, inputs, budget);
    }
    let Some(operation) = task.scalar() else {
        validate_native_inputs(state, task, inputs)?;
        return Ok(());
    };
    let operation = &operation;
    let expected = match operation {
        Operation::Equal => Some(2),
        Operation::Jacobian(_) => None,
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
            || !crate::header::embeds(state, expression.header, context.header_id, budget)?
        {
            return Err(Fault::invalid(InvalidCode::Scope, at));
        }
        let compatible = match operation {
            Operation::Equal => expression.sort == *first_sort,
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
    validate_variables(context, operation, inputs.len(), length, budget)?;
    Ok(())
}

/// Validate scalar variable identities independently of occurrence-root shape.
fn validate_variables(
    context: &crate::graph::Context,
    operation: &Operation,
    inputs: usize,
    length: u64,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let at = Location::new("request variable validation");
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
            .checked_mul(size(inputs, at)?)
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

/// Validate structural task references even when no producer is executable yet.
fn validate_task(
    state: &State,
    task: &Task,
    context: &crate::graph::Context,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let at = Location::new("task validation");
    for reference in task.references() {
        budget.work(1, at)?;
        let (header, scope) = crate::descriptor::metadata(state, reference.logical_id())?;
        if !scope.is_empty() || !crate::header::embeds(state, header, context.header_id, budget)? {
            return Err(Fault::invalid(InvalidCode::Context, at));
        }
    }
    if let Task::ApplyAt { point, .. } = task {
        state.expression(point.logical_id())?;
    }
    if let Task::Merge { proposed_value, .. } = task {
        state.expression(proposed_value.logical_id())?;
    }
    if let Task::Pair {
        action_mode: crate::task::ActionMode::NearSupport { neighborhood },
        ..
    } = task
    {
        state.region(neighborhood.logical_id())?;
    }
    match task {
        Task::Equal { domain, .. } | Task::Differentiate { domain, .. } => {
            state.region(domain.logical_id())?;
            if task.scalar().is_some()
                && domain.logical_id() != context.admissible
                && !matches!(task, Task::Differentiate { layer: crate::domain_model::Layer::Pointwise, .. }
                    if matches!(state.region(domain.logical_id())?.ambient, crate::domain_model::AmbientDomain::Real(_)))
            {
                return Err(Fault::invalid(InvalidCode::Context, at));
            }
        }
        Task::Coverage { domain } | Task::Select { region: domain } => {
            state.region(domain.logical_id())?;
        }
        Task::Eliminate { regions, .. } | Task::Merge { regions, .. } => {
            budget.check(BudgetKind::CaseArms, size(regions.len(), at)?, at)?;
            for region in regions {
                state.region(region.logical_id())?;
            }
        }
        Task::Limit(germ) => {
            state.boundary(germ.point.logical_id())?;
        }
        Task::Series(series) => {
            state.boundary(series.at.point.logical_id())?;
            budget.check(BudgetKind::JetOrder, u64::from(series.order), at)?;
        }
        Task::Boundary { point, .. } => {
            state.boundary(point.logical_id())?;
        }
        Task::Pair { space, .. }
        | Task::SmoothMultiply { space }
        | Task::EmbedRegular { space }
        | Task::PrimitiveUnique { space } => {
            state.space(space.logical_id())?;
        }
        Task::Pullback {
            source_space,
            target_space,
            map_kind,
        } => {
            state.space(source_space.logical_id())?;
            state.space(target_space.logical_id())?;
            crate::pullback_wire::validate_intent(map_kind, budget)?;
        }
        _ => {}
    }
    Ok(())
}

/// Native task arity and root kinds are checked before any executable producer runs.
fn validate_native_inputs(state: &State, task: &Task, inputs: &[LogicalId]) -> Result<(), Fault> {
    let at = Location::new("native request inputs");
    let expected = match task {
        Task::PrimitiveUnique { .. }
        | Task::Pullback { .. }
        | Task::Equal { .. }
        | Task::Pair { .. }
        | Task::SmoothMultiply { .. }
        | Task::Product { .. } => Some(2),
        Task::Eliminate { .. } => Some(0),
        Task::Limit(_)
        | Task::Series(_)
        | Task::Integrate(_)
        | Task::Sum(_)
        | Task::EmbedRegular { .. }
        | Task::ApplyAt { .. }
        | Task::ElaborateCases
        | Task::Select { .. }
        | Task::Merge { .. }
        | Task::Coverage { .. }
        | Task::Boundary { .. }
        | Task::Differentiate { .. } => Some(1),
        _ => None,
    };
    if expected.is_some_and(|count| inputs.len() != count) {
        return Err(Fault::invalid(InvalidCode::Sort, at));
    }
    for input in inputs {
        let root = state.occurrence(*input)?.expression;
        if matches!(task, Task::ElaborateCases) {
            state.source_form(root)?;
        } else {
            state.expression(root)?;
        }
    }
    Ok(())
}
