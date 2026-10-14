// SPDX-License-Identifier: AGPL-3.0-only
//! Append-only heterogeneous graph storage and atomic batch admission.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use petgraph::graph::{DiGraph, NodeIndex};

use crate::budget::{Budget, BudgetKind, Limits};
use crate::encoding::{self, Encoder};
use crate::error::{Fault, InvalidCode, Location};
use crate::model::{Binding, Environment, Operation, Operator, PredicateKind, Scope, Sort, Span};

/// A disjoint namespace for logical identifiers.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum VertexKind {
    /// Immutable source bytes.
    Source,
    /// Mathematical context and hypothesis declarations.
    Context,
    /// Typed mathematical predicate.
    Condition,
    /// Shared mathematical syntax.
    Expression,
    /// Original or generated occurrence.
    Occurrence,
    /// Binder-frame semantics.
    Binder,
    /// A first-class operation request.
    Request,
    /// A checked identity application, admitted only by the proof module.
    Application,
    /// A checked acyclic proof record, admitted only by the proof module.
    Proof,
}

impl VertexKind {
    /// Stable encoding independent of Rust enum layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::Source => 0,
            Self::Context => 1,
            Self::Condition => 2,
            Self::Expression => 3,
            Self::Occurrence => 4,
            Self::Binder => 5,
            Self::Request => 6,
            Self::Application => 7,
            Self::Proof => 8,
        }
    }
}

/// A deterministic identifier within one vertex-kind namespace.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LogicalId {
    /// Vertex kind.
    pub kind: VertexKind,
    /// Consecutive admission serial, beginning with zero in each namespace.
    pub serial: u64,
}

/// Runtime store identity; its address is never encoded or ordered.
#[derive(Debug)]
struct StoreToken;

/// An opaque store-scoped handle; callers cannot manufacture its store token.
#[derive(Clone)]
pub struct Handle {
    /// Shared runtime ownership identity.
    token: Arc<StoreToken>,
    /// Mathematical admission identifier.
    id: LogicalId,
}

impl Handle {
    /// Read a portable logical identifier, without conferring store ownership.
    #[must_use]
    pub const fn logical_id(&self) -> LogicalId {
        self.id
    }
}

impl fmt::Debug for Handle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Handle")
            .field(&self.id)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Handle {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && Arc::ptr_eq(&self.token, &other.token)
    }
}

impl Eq for Handle {}

/// A checked existing handle or an earlier index in the current draft batch.
#[derive(Clone, Debug)]
pub enum Reference {
    /// A live handle in the receiving store.
    Existing(Handle),
    /// A zero-based draft index, strictly earlier than its user.
    Local(u32),
}

/// A context hypothesis whose identity and predicate are both retained.
#[derive(Clone, Debug)]
pub struct AssumptionDraft {
    /// Caller hypothesis key.
    pub key: String,
    /// Earlier condition record.
    pub condition: Reference,
}

/// Untrusted ordered expression syntax.
#[derive(Clone, Debug)]
pub struct ExpressionDraft {
    /// Operator and complete mathematical payload.
    pub operator: Operator,
    /// Claimed mathematical result sort, checked against operands.
    pub sort: Sort,
    /// Context providing the semantic header.
    pub context: Reference,
    /// Explicit outer-to-inner binder frames.
    pub scope: Scope,
    /// Optional new frame for the designated body operands.
    pub binding: Option<Binding>,
    /// Ordered child expressions, each already admitted or earlier in the batch.
    pub operands: Vec<Reference>,
}

/// Untrusted source/provenance occurrence with a distinct identity for every use.
#[derive(Clone, Debug)]
pub struct OccurrenceDraft {
    /// Mathematical syntax denoted by this occurrence.
    pub expression: Reference,
    /// Original source, absent for generated occurrences.
    pub source: Option<Reference>,
    /// Optional checked source span; never present on generated occurrences.
    pub span: Option<Span>,
    /// Ordered child occurrences, exactly matching the expression operands.
    pub children: Vec<Reference>,
    /// Original expression/occurrence supplying generated provenance.
    pub provenance: Option<Reference>,
    /// Source binder display names, independent of de Bruijn identity.
    pub binder_names: Vec<String>,
}

/// One flat draft node. Arbitrary identity/proof nodes cannot be supplied here.
#[derive(Clone, Debug)]
pub enum Draft {
    /// Retain immutable UTF-8 source bytes.
    Source {
        /// Caller source key.
        key: String,
        /// Exact source text.
        text: String,
    },
    /// Admit a canonical context under a complete semantic header.
    Context {
        /// Declared field, branches and free identities.
        environment: Environment,
        /// Explicit mathematical hypotheses.
        assumptions: Vec<AssumptionDraft>,
    },
    /// Admit a typed predicate; this supplies no proof of its truth.
    Condition {
        /// Predicate vocabulary tag.
        kind: PredicateKind,
        /// Semantic context.
        context: Reference,
        /// Explicit bound frames.
        scope: Scope,
        /// Ordered predicate expressions.
        arguments: Vec<Reference>,
    },
    /// Admit shared mathematical syntax.
    Expression(ExpressionDraft),
    /// Admit a distinct original or generated occurrence.
    Occurrence(OccurrenceDraft),
    /// Retain a typed task; this does not authorize execution without coverage receipts.
    Request {
        /// Requested mathematical operation.
        operation: Operation,
        /// Immutable mathematical context.
        context: Reference,
        /// Ordered occurrence roots.
        inputs: Vec<Reference>,
        /// Fully resolved request budgets.
        limits: Limits,
    },
}

/// An immutable source weight.
#[derive(Clone, Debug)]
pub(crate) struct Source {
    /// Caller source identity.
    pub(crate) key: String,
    /// Exact retained UTF-8 text.
    pub(crate) text: String,
}

/// Canonical mathematical context weight.
#[derive(Clone, Debug)]
pub(crate) struct Context {
    /// Sorted semantic declarations and policies.
    pub(crate) environment: Environment,
    /// Canonical header bytes, excluding assumptions.
    pub(crate) header: Vec<u8>,
    /// Hypothesis keys and condition IDs in canonical order.
    pub(crate) assumptions: Vec<(String, LogicalId)>,
    /// Flattened, sorted hypothesis predicates retaining their provenance keys.
    pub(crate) canonical_assumptions: Vec<u8>,
}

/// A predicate weight; truth is checked separately from syntax admission.
#[derive(Clone, Debug)]
pub(crate) struct Condition {
    /// Predicate tag.
    pub(crate) kind: PredicateKind,
    /// Context used for mathematical interpretation.
    pub(crate) context: LogicalId,
    /// Explicit binder environment.
    pub(crate) scope: Scope,
    /// Ordered argument expression IDs.
    pub(crate) arguments: Vec<LogicalId>,
}

/// Immutable expression weight and cached, checked shape statistics.
#[derive(Clone, Debug)]
pub(crate) struct Expression {
    /// Complete operator metadata.
    pub(crate) operator: Operator,
    /// Checked result sort.
    pub(crate) sort: Sort,
    /// First admitting context; sharing is by its semantic header alone.
    pub(crate) context: LogicalId,
    /// Explicit frames.
    pub(crate) scope: Scope,
    /// New bound frame, if any.
    pub(crate) binding: Option<Binding>,
    /// Ordered expression operand IDs.
    pub(crate) operands: Vec<LogicalId>,
    /// Exact operand depth.
    pub(crate) depth: u64,
    /// Exact unfolded syntax size, checked against its ceiling.
    pub(crate) unfolded: u64,
}

/// Immutable occurrence weight.
#[derive(Clone, Debug)]
pub(crate) struct Occurrence {
    /// Shared syntax ID.
    pub(crate) expression: LogicalId,
    /// Original source if present.
    pub(crate) source: Option<LogicalId>,
    /// Checked original byte range.
    pub(crate) span: Option<Span>,
    /// Ordered unique child occurrences.
    pub(crate) children: Vec<LogicalId>,
    /// Generated provenance when no original source exists.
    pub(crate) provenance: Option<LogicalId>,
    /// Preserved binder display names.
    pub(crate) binder_names: Vec<String>,
}

/// A first-class operation request; it contains no successful proof flag.
#[derive(Clone, Debug)]
pub(crate) struct RequestRecord {
    /// Requested mathematical relation/operation.
    pub(crate) operation: Operation,
    /// Full context, including hypotheses.
    pub(crate) context: LogicalId,
    /// Original or generated occurrence roots in caller order.
    pub(crate) inputs: Vec<LogicalId>,
    /// Resolved resource policy.
    pub(crate) limits: Limits,
}

/// One authoritative heterogeneous vertex weight.
#[derive(Clone, Debug)]
pub(crate) enum Vertex {
    /// Retained source.
    Source(Source),
    /// Mathematical context.
    Context(Context),
    /// Unproved condition syntax.
    Condition(Condition),
    /// Shared mathematical expression.
    Expression(Expression),
    /// Distinct original/generated use.
    Occurrence(Occurrence),
    /// Semantic binder frame.
    Binder(Binding),
    /// A task whose algorithm may not be admitted yet.
    Request(RequestRecord),
    /// A kernel-checked instantiated primitive.
    Application(crate::proof_model::CheckedStep),
    /// A sealed proof root pointing to its authoritative application.
    Proof(LogicalId),
    /// A strictly cheaper reusable route backed by an older complete proof.
    Shortcut {
        /// Existing checked proof, never a forward dependency.
        proof: LogicalId,
        /// Positive logical-v1 cost after strict improvement checking.
        cost: u64,
    },
}

/// Typed graph edge roles; only internal validated admission can create edges.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Edge {
    /// An ordered mathematical operand.
    Operand(u32),
    /// A source occurrence's mathematical meaning.
    Denotes,
    /// Original retained source.
    Source,
    /// Ordered source-occurrence operand.
    Child(u32),
    /// Context interpretation.
    InContext,
    /// Ordered hypothesis reference.
    Assumption(u32),
    /// Ordered predicate expression argument.
    PredicateArgument(u32),
    /// An expression's semantic binder frame.
    Binder,
    /// A binder's scoped body position.
    Body(u32),
    /// Generated expression or occurrence provenance.
    Provenance,
    /// Ordered operation input root.
    Input(u32),
    /// Checked conclusion input.
    ConclusionLeft,
    /// Checked conclusion output.
    ConclusionRight,
    /// Ordered prerequisite proof.
    Premise(u32),
    /// Required typed domain condition.
    Obligation(u32),
    /// The checked application underlying a proof.
    Justifies,
    /// An older proof expanded by a derived shortcut.
    DependsOn,
}

/// Append-only storage plus derived lookup maps; the graph owns every weight.
#[derive(Clone, Debug)]
pub(crate) struct State {
    /// Authoritative heterogeneous graph.
    pub(crate) graph: DiGraph<Vertex, Edge, u32>,
    /// Logical-to-physical lookup, never serialized.
    pub(crate) indices: BTreeMap<LogicalId, NodeIndex<u32>>,
    /// Exact canonical syntax/context/condition keys.
    pub(crate) intern: BTreeMap<Vec<u8>, LogicalId>,
    /// Next serial per kind, independent of graph indices.
    next: BTreeMap<VertexKind, u64>,
    /// Source key lookup used to reject conflicting source bytes.
    sources: BTreeMap<String, LogicalId>,
    /// Parent uniqueness for all occurrence records.
    parents: BTreeMap<LogicalId, LogicalId>,
    /// Measured retained source bytes.
    source_bytes: u64,
    /// Measured original occurrence count.
    originals: u64,
    /// Conservative logical payload bytes, including owned interning keys.
    payload: u64,
}

impl State {
    /// Start an empty graph without allocating the requested maximum capacities.
    pub(crate) fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            indices: BTreeMap::new(),
            intern: BTreeMap::new(),
            next: BTreeMap::new(),
            sources: BTreeMap::new(),
            parents: BTreeMap::new(),
            source_bytes: 0,
            originals: 0,
            payload: 0,
        }
    }

    /// Look up a logical ID without indexing or trusting its kind.
    pub(crate) fn node(&self, id: LogicalId) -> Result<&Vertex, Fault> {
        self.indices
            .get(&id)
            .and_then(|index| self.graph.node_weight(*index))
            .ok_or_else(|| Fault::invalid(InvalidCode::Reference, Location::new("node lookup")))
    }

    /// Read an expression of the correct vertex kind.
    pub(crate) fn expression(&self, id: LogicalId) -> Result<&Expression, Fault> {
        match self.node(id)? {
            Vertex::Expression(value) => Ok(value),
            _ => Err(Fault::invalid(
                InvalidCode::Reference,
                Location::new("expression lookup"),
            )),
        }
    }

    /// Read an immutable context of the correct kind.
    pub(crate) fn context(&self, id: LogicalId) -> Result<&Context, Fault> {
        match self.node(id)? {
            Vertex::Context(value) => Ok(value),
            _ => Err(Fault::invalid(
                InvalidCode::Reference,
                Location::new("context lookup"),
            )),
        }
    }

    /// Read an occurrence of the correct kind.
    pub(crate) fn occurrence(&self, id: LogicalId) -> Result<&Occurrence, Fault> {
        match self.node(id)? {
            Vertex::Occurrence(value) => Ok(value),
            _ => Err(Fault::invalid(
                InvalidCode::Reference,
                Location::new("occurrence lookup"),
            )),
        }
    }
}

/// Occupancy counters independent of petgraph's internal index layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphCounts {
    /// Published vertices.
    pub vertices: usize,
    /// Published edges.
    pub edges: usize,
    /// Retained original source bytes.
    pub source_bytes: u64,
    /// Original occurrence count.
    pub original_occurrences: u64,
    /// Accounted logical payload bytes.
    pub payload_bytes: u64,
}

/// The independently owned symbolic store; it has no engine-package dependency.
pub struct Cas {
    /// Runtime identity excluded from canonical outputs.
    token: Arc<StoreToken>,
    /// Permanent storage ceilings.
    pub(crate) limits: Limits,
    /// Last completely committed state.
    pub(crate) state: State,
}

impl fmt::Debug for Cas {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Cas")
            .field("counts", &self.counts())
            .finish_non_exhaustive()
    }
}

impl Cas {
    /// Create an empty store under explicit permanent ceilings.
    ///
    /// # Errors
    /// Refuses inconsistent limits before constructing store state.
    pub fn new(limits: Limits) -> Result<Self, Fault> {
        limits.validate()?;
        Ok(Self {
            token: Arc::new(StoreToken),
            limits,
            state: State::new(),
        })
    }

    /// Copy a private transaction candidate after charging its full retained storage.
    pub(crate) fn fork(&self, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("proof transaction");
        for (kind, needed) in [
            (
                BudgetKind::StagedVertices,
                size(self.state.graph.node_count(), at)?,
            ),
            (
                BudgetKind::StagedEdges,
                size(self.state.graph.edge_count(), at)?,
            ),
            (BudgetKind::StagedPayloadBytes, self.state.payload),
        ] {
            storage_check(&self.limits, budget, kind, needed, at)?;
        }
        budget.work(self.state.payload, at)?;
        Ok(Self {
            token: Arc::clone(&self.token),
            limits: self.limits.clone(),
            state: self.state.clone(),
        })
    }

    /// Read current occupancy without exposing a physical graph index.
    #[must_use]
    pub fn counts(&self) -> GraphCounts {
        GraphCounts {
            vertices: self.state.graph.node_count(),
            edges: self.state.graph.edge_count(),
            source_bytes: self.state.source_bytes,
            original_occurrences: self.state.originals,
            payload_bytes: self.state.payload,
        }
    }

    /// Validate a live store-scoped handle and its logical reference.
    pub(crate) fn checked(&self, handle: &Handle) -> Result<LogicalId, Fault> {
        if !Arc::ptr_eq(&self.token, &handle.token) {
            return Err(Fault::invalid(
                InvalidCode::ForeignHandle,
                Location::new("handle lookup"),
            ));
        }
        self.state.node(handle.id)?;
        Ok(handle.id)
    }

    /// Mint a handle only after its logical record exists in a committed store.
    pub(crate) fn handle(&self, id: LogicalId) -> Handle {
        Handle {
            token: Arc::clone(&self.token),
            id,
        }
    }

    /// Read ordered mathematical operands as checked handles.
    ///
    /// # Errors
    /// Refuses a foreign/wrong-kind handle or insufficient work/allocation.
    pub fn operands(&self, handle: &Handle, budget: &mut Budget) -> Result<Vec<Handle>, Fault> {
        let id = self.checked(handle)?;
        let expression = self.state.expression(id)?;
        let at = Location::new("operand read");
        let bytes = size(expression.operands.len(), at)?
            .checked_mul(16)
            .ok_or_else(|| Fault::capacity(BudgetKind::OutputBytes, at))?;
        budget.check(BudgetKind::OutputBytes, bytes, at)?;
        budget.work(size(expression.operands.len(), at)?, at)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(expression.operands.len())
            .map_err(|_| Fault::allocation(at))?;
        result.extend(
            expression
                .operands
                .iter()
                .map(|operand| self.handle(*operand)),
        );
        Ok(result)
    }

    /// Borrow an expression's operator and result sort without formatting a tree.
    ///
    /// # Errors
    /// Refuses foreign handles or handles of another vertex kind.
    pub fn expression(&self, handle: &Handle) -> Result<(&Operator, &Sort), Fault> {
        let expression = self.state.expression(self.checked(handle)?)?;
        Ok((&expression.operator, &expression.sort))
    }

    /// Borrow an immutable source key and its exact UTF-8 text.
    ///
    /// # Errors
    /// Refuses a foreign handle or a handle of another vertex kind.
    pub fn source(&self, handle: &Handle) -> Result<(&str, &str), Fault> {
        match self.state.node(self.checked(handle)?)? {
            Vertex::Source(source) => Ok((&source.key, &source.text)),
            _ => Err(Fault::invalid(
                InvalidCode::Reference,
                Location::new("source read"),
            )),
        }
    }

    /// Validate and atomically publish a child-before-parent batch.
    ///
    /// # Errors
    /// Refuses malformed drafts, foreign/forward references, or exhausted budgets;
    /// the published graph and next logical identifiers remain unchanged.
    pub fn admit(&mut self, drafts: &[Draft], budget: &mut Budget) -> Result<Vec<Handle>, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = self.admit_batch(drafts, budget);
        budget.restore_limits(previous);
        result
    }

    /// Apply a batch under the already intersected storage ceilings.
    fn admit_batch(&mut self, drafts: &[Draft], budget: &mut Budget) -> Result<Vec<Handle>, Fault> {
        let at = Location::new("transaction");
        let pending = crate::validation::draft_payload(drafts, budget)?;
        let peak = self
            .state
            .payload
            .checked_add(pending)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?;
        storage_check(
            &self.limits,
            budget,
            BudgetKind::StagedPayloadBytes,
            peak,
            at,
        )?;
        storage_check(
            &self.limits,
            budget,
            BudgetKind::StagedVertices,
            size(self.state.graph.node_count(), at)?,
            at,
        )?;
        storage_check(
            &self.limits,
            budget,
            BudgetKind::StagedEdges,
            size(self.state.graph.edge_count(), at)?,
            at,
        )?;
        budget.work(peak, at)?;
        let mut staged = self.state.clone();
        let mut resolved = Vec::new();
        resolved
            .try_reserve_exact(drafts.len())
            .map_err(|_| Fault::allocation(at))?;
        for draft in drafts {
            budget.work(1, at)?;
            let id = self.admit_one(&mut staged, draft, &resolved, pending, budget)?;
            resolved.push(id);
        }
        let mut handles = Vec::new();
        handles
            .try_reserve_exact(resolved.len())
            .map_err(|_| Fault::allocation(at))?;
        handles.extend(resolved.iter().map(|id| self.handle(*id)));
        self.state = staged;
        Ok(handles)
    }

    /// Resolve one existing or strictly earlier batch reference.
    fn resolve(&self, reference: &Reference, locals: &[LogicalId]) -> Result<LogicalId, Fault> {
        match reference {
            Reference::Existing(handle) => self.checked(handle),
            Reference::Local(index) => usize::try_from(*index)
                .ok()
                .and_then(|index| locals.get(index))
                .copied()
                .ok_or_else(|| {
                    Fault::invalid(InvalidCode::Reference, Location::new("draft reference"))
                }),
        }
    }

    /// Resolve an ordered finite reference list before looking up any weights.
    fn resolve_all(
        &self,
        references: &[Reference],
        locals: &[LogicalId],
        budget: &mut Budget,
    ) -> Result<Vec<LogicalId>, Fault> {
        let at = Location::new("draft references");
        budget.work(size(references.len(), at)?, at)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(references.len())
            .map_err(|_| Fault::allocation(at))?;
        for reference in references {
            result.push(self.resolve(reference, locals)?);
        }
        Ok(result)
    }

    /// Dispatch one already bounded draft into the private candidate state.
    fn admit_one(
        &self,
        state: &mut State,
        draft: &Draft,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        match draft {
            Draft::Source { key, text } => self.admit_source(state, key, text, pending, budget),
            Draft::Context {
                environment,
                assumptions,
            } => self.admit_context(state, environment, assumptions, locals, pending, budget),
            Draft::Condition {
                kind,
                context,
                scope,
                arguments,
            } => {
                let context = self.resolve(context, locals)?;
                let arguments = self.resolve_all(arguments, locals, budget)?;
                crate::validation::condition(state, kind, context, scope, &arguments, budget)?;
                let mut edges = vec![(context, Edge::InContext)];
                for (position, argument) in arguments.iter().enumerate() {
                    edges.push((*argument, Edge::PredicateArgument(position_u32(position)?)));
                }
                self.insert(
                    state,
                    Vertex::Condition(Condition {
                        kind: kind.clone(),
                        context,
                        scope: scope.clone(),
                        arguments,
                    }),
                    &edges,
                    pending,
                    budget,
                )
            }
            Draft::Expression(expression) => {
                self.admit_expression(state, expression, locals, pending, budget)
            }
            Draft::Occurrence(occurrence) => {
                self.admit_occurrence(state, occurrence, locals, pending, budget)
            }
            Draft::Request {
                operation,
                context,
                inputs,
                limits,
            } => {
                let context = self.resolve(context, locals)?;
                let inputs = self.resolve_all(inputs, locals, budget)?;
                crate::request::validate(state, operation, context, &inputs, limits, budget)?;
                let mut edges = vec![(context, Edge::InContext)];
                for (position, input) in inputs.iter().enumerate() {
                    edges.push((*input, Edge::Input(position_u32(position)?)));
                }
                self.insert(
                    state,
                    Vertex::Request(RequestRecord {
                        operation: operation.clone(),
                        context,
                        inputs,
                        limits: limits.clone(),
                    }),
                    &edges,
                    pending,
                    budget,
                )
            }
        }
    }

    /// Retain or reuse a source while rejecting a conflicting key.
    fn admit_source(
        &self,
        state: &mut State,
        key: &str,
        text: &str,
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let at = Location::new("source admission");
        budget.work(
            size(state.sources.len(), at)?
                .checked_add(1)
                .and_then(|n| n.checked_mul(size(key.len(), at).ok()?.max(1)))
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?,
            at,
        )?;
        if let Some(id) = state.sources.get(key) {
            budget.work(size(text.len(), at)?, at)?;
            if let Vertex::Source(source) = state.node(*id)?
                && source.text == text
            {
                return Ok(*id);
            }
            return Err(Fault::invalid(InvalidCode::SourceConflict, at));
        }
        let next = state
            .source_bytes
            .checked_add(size(text.len(), at)?)
            .ok_or_else(|| Fault::capacity(BudgetKind::SourceBytes, at))?;
        storage_check(&self.limits, budget, BudgetKind::SourceBytes, next, at)?;
        let id = self.insert(
            state,
            Vertex::Source(Source {
                key: key.to_owned(),
                text: text.to_owned(),
            }),
            &[],
            pending,
            budget,
        )?;
        state.sources.insert(key.to_owned(), id);
        state.source_bytes = next;
        Ok(id)
    }

    /// Canonicalize a context before interning it.
    fn admit_context(
        &self,
        state: &mut State,
        environment: &Environment,
        assumptions: &[AssumptionDraft],
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let at = Location::new("context admission");
        let environment = crate::validation::environment(environment, budget)?;
        let mut encoded = Encoder::new(budget, at);
        encoding::environment(&environment, &mut encoded)?;
        let header = encoded.finish();
        budget.check(BudgetKind::ConditionAtoms, size(assumptions.len(), at)?, at)?;
        let mut declared = BTreeMap::new();
        let comparisons = size(assumptions.len(), at)?
            .checked_mul(size(assumptions.len(), at)?)
            .and_then(|n| n.checked_mul(budget.limits().name_bytes))
            .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?;
        budget.work(comparisons, at)?;
        for assumption in assumptions {
            let condition = self.resolve(&assumption.condition, locals)?;
            crate::validation::assumption(state, condition, &header, budget)?;
            if let Some(old) = declared.insert(assumption.key.clone(), condition)
                && old != condition
            {
                return Err(Fault::invalid(InvalidCode::Context, at));
            }
        }
        let assumptions: Vec<_> = declared.into_iter().collect();
        let canonical_assumptions = crate::context::canonical(state, &assumptions, budget)?;
        let mut edges = Vec::new();
        edges
            .try_reserve_exact(assumptions.len())
            .map_err(|_| Fault::allocation(at))?;
        for (position, (_, condition)) in assumptions.iter().enumerate() {
            edges.push((*condition, Edge::Assumption(position_u32(position)?)));
        }
        self.insert(
            state,
            Vertex::Context(Context {
                environment,
                header,
                assumptions,
                canonical_assumptions,
            }),
            &edges,
            pending,
            budget,
        )
    }

    /// Validate mathematical syntax before admitting its binder and expression.
    fn admit_expression(
        &self,
        state: &mut State,
        draft: &ExpressionDraft,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let at = Location::new("expression admission");
        let context = self.resolve(&draft.context, locals)?;
        let operands = self.resolve_all(&draft.operands, locals, budget)?;
        let (depth, unfolded) =
            crate::validation::expression(state, draft, context, &operands, budget)?;
        let expression = Vertex::Expression(Expression {
            operator: draft.operator.clone(),
            sort: draft.sort.clone(),
            context,
            scope: draft.scope.clone(),
            binding: draft.binding.clone(),
            operands: operands.clone(),
            depth,
            unfolded,
        });
        let (_, key) = vertex_key(&expression, state, budget)?;
        budget.work(
            size(state.intern.len(), at)?
                .checked_add(1)
                .and_then(|n| n.checked_mul(size(key.len(), at).ok()?))
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?,
            at,
        )?;
        if let Some(id) = state.intern.get(&key) {
            return Ok(*id);
        }
        let mut edges = vec![(context, Edge::InContext)];
        for (position, operand) in operands.iter().enumerate() {
            edges.push((*operand, Edge::Operand(position_u32(position)?)));
        }
        if let Some(binding) = &draft.binding {
            let mut bodies = Vec::new();
            bodies
                .try_reserve_exact(binding.body_operands.len())
                .map_err(|_| Fault::allocation(at))?;
            for position in &binding.body_operands {
                let index = usize::try_from(*position)
                    .map_err(|_| Fault::invalid(InvalidCode::Scope, at))?;
                let body = operands
                    .get(index)
                    .copied()
                    .ok_or_else(|| Fault::invalid(InvalidCode::Scope, at))?;
                bodies.push((body, Edge::Body(*position)));
            }
            // Binder records intentionally remain distinct for each scoped expression.
            let binder = self.insert(
                state,
                Vertex::Binder(binding.clone()),
                &bodies,
                pending,
                budget,
            )?;
            edges.push((binder, Edge::Binder));
        }
        self.insert(state, expression, &edges, pending, budget)
    }

    /// Validate distinct occurrences and atomically reserve their parent links.
    fn admit_occurrence(
        &self,
        state: &mut State,
        draft: &OccurrenceDraft,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let at = Location::new("occurrence admission");
        let expression = self.resolve(&draft.expression, locals)?;
        let source = draft
            .source
            .as_ref()
            .map(|value| self.resolve(value, locals))
            .transpose()?;
        let provenance = draft
            .provenance
            .as_ref()
            .map(|value| self.resolve(value, locals))
            .transpose()?;
        let children = self.resolve_all(&draft.children, locals, budget)?;
        let occurrence = Occurrence {
            expression,
            source,
            span: draft.span,
            children,
            provenance,
            binder_names: draft.binder_names.clone(),
        };
        crate::validation::occurrence(state, &occurrence, budget)?;
        let mut unique = BTreeSet::new();
        let mut edges = vec![(expression, Edge::Denotes)];
        for (position, child) in occurrence.children.iter().enumerate() {
            budget.work(
                size(state.parents.len(), at)?
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?,
                at,
            )?;
            if state.parents.contains_key(child) || !unique.insert(*child) {
                return Err(Fault::invalid(InvalidCode::OccurrenceParent, at));
            }
            edges.push((*child, Edge::Child(position_u32(position)?)));
        }
        if let Some(source) = source {
            edges.push((source, Edge::Source));
        }
        if let Some(provenance) = provenance {
            edges.push((provenance, Edge::Provenance));
        }
        let originals = state
            .originals
            .checked_add(u64::from(source.is_some()))
            .ok_or_else(|| Fault::capacity(BudgetKind::OriginalOccurrences, at))?;
        storage_check(
            &self.limits,
            budget,
            BudgetKind::OriginalOccurrences,
            originals,
            at,
        )?;
        let id = self.insert(
            state,
            Vertex::Occurrence(occurrence),
            &edges,
            pending,
            budget,
        )?;
        for child in unique {
            state.parents.insert(child, id);
        }
        state.originals = originals;
        Ok(id)
    }

    /// Publish only into a private state after every size/index/endpoint check.
    pub(crate) fn insert(
        &self,
        state: &mut State,
        vertex: Vertex,
        edges: &[(LogicalId, Edge)],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let at = Location::new("vertex admission");
        let (kind, key) = vertex_key(&vertex, state, budget)?;
        let interned = !matches!(kind, VertexKind::Occurrence | VertexKind::Binder);
        let comparisons = size(state.intern.len(), at)?
            .checked_add(1)
            .and_then(|n| n.checked_mul(size(key.len(), at).ok()?))
            .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?;
        budget.work(comparisons, at)?;
        if interned && let Some(id) = state.intern.get(&key) {
            return Ok(*id);
        }
        let node_count = size(state.graph.node_count(), at)?
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, at))?;
        let edge_count = size(state.graph.edge_count(), at)?
            .checked_add(size(edges.len(), at)?)
            .ok_or_else(|| Fault::capacity(BudgetKind::Edges, at))?;
        if node_count >= u64::from(u32::MAX) || edge_count >= u64::from(u32::MAX) {
            return Err(Fault::capacity(BudgetKind::Vertices, at));
        }
        for (kind, count) in [
            (BudgetKind::Vertices, node_count),
            (BudgetKind::StagedVertices, node_count),
            (BudgetKind::Edges, edge_count),
            (BudgetKind::StagedEdges, edge_count),
        ] {
            storage_check(&self.limits, budget, kind, count, at)?;
        }
        let added = size(key.len(), at)?
            .checked_mul(3)
            .and_then(|n| n.checked_add(256))
            .and_then(|n| size(edges.len(), at).ok()?.checked_mul(32)?.checked_add(n))
            .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, at))?;
        let payload = state
            .payload
            .checked_add(added)
            .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, at))?;
        storage_check(&self.limits, budget, BudgetKind::PayloadBytes, payload, at)?;
        storage_check(
            &self.limits,
            budget,
            BudgetKind::StagedPayloadBytes,
            payload
                .checked_add(pending)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?,
            at,
        )?;
        let serial = state.next.get(&kind).copied().unwrap_or(0);
        let next = serial
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, at))?;
        let mut targets = Vec::new();
        targets
            .try_reserve_exact(edges.len())
            .map_err(|_| Fault::allocation(at))?;
        for (target, role) in edges {
            let index = state
                .indices
                .get(target)
                .copied()
                .ok_or_else(|| Fault::invalid(InvalidCode::Reference, at))?;
            targets.push((index, *role));
        }
        let id = LogicalId { kind, serial };
        let index = state.graph.add_node(vertex);
        for (target, role) in targets {
            let _edge = state.graph.add_edge(index, target, role);
        }
        state.indices.insert(id, index);
        state.next.insert(kind, next);
        if interned {
            state.intern.insert(key, id);
        }
        state.payload = payload;
        Ok(id)
    }
}

/// Convert a host collection length without truncation.
pub(crate) fn size(value: usize, at: Location) -> Result<u64, Fault> {
    u64::try_from(value).map_err(|_| Fault::capacity(BudgetKind::PayloadBytes, at))
}

/// Enforce both the fixed store ceiling and the active call's possibly lower ceiling.
fn storage_check(
    limits: &Limits,
    budget: &Budget,
    kind: BudgetKind,
    needed: u64,
    at: Location,
) -> Result<(), Fault> {
    budget.check(kind, needed, at)?;
    let limit = limits.get(kind);
    if needed > limit {
        return Err(Fault::exhausted(kind, limit, needed, at));
    }
    Ok(())
}

/// Append a logical ID to a structural key without exposing graph indices.
pub(crate) fn encode_id(id: LogicalId, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.byte(id.kind.tag())?;
    out.unsigned(id.serial)
}

/// Convert a checked operand position to its wire/edge width.
fn position_u32(position: usize) -> Result<u32, Fault> {
    u32::try_from(position)
        .map_err(|_| Fault::capacity(BudgetKind::Edges, Location::new("operand position")))
}

/// Reconstruct a complete canonical record key from graph-owned weights.
pub(crate) fn vertex_key(
    vertex: &Vertex,
    state: &State,
    budget: &mut Budget,
) -> Result<(VertexKind, Vec<u8>), Fault> {
    let kind = match vertex {
        Vertex::Source(_) => VertexKind::Source,
        Vertex::Context(_) => VertexKind::Context,
        Vertex::Condition(_) => VertexKind::Condition,
        Vertex::Expression(_) => VertexKind::Expression,
        Vertex::Occurrence(_) => VertexKind::Occurrence,
        Vertex::Binder(_) => VertexKind::Binder,
        Vertex::Request(_) => VertexKind::Request,
        Vertex::Application(_) | Vertex::Shortcut { .. } => VertexKind::Application,
        Vertex::Proof(_) => VertexKind::Proof,
    };
    let mut out = Encoder::new(budget, Location::new("structural key"));
    out.byte(kind.tag())?;
    match vertex {
        Vertex::Application(application) => {
            out.byte(0)?;
            crate::proof_storage::encode_application(application, &mut out)?;
        }
        Vertex::Proof(application) => encode_id(*application, &mut out)?,
        Vertex::Shortcut { proof, cost } => {
            out.byte(1)?;
            encode_id(*proof, &mut out)?;
            out.unsigned(*cost)?;
        }
        Vertex::Source(source) => {
            out.name(&source.key)?;
            out.count(source.text.len())?;
            out.raw(source.text.as_bytes())?;
        }
        Vertex::Context(context) => {
            out.raw(&context.header)?;
            out.count(context.assumptions.len())?;
            out.raw(&context.canonical_assumptions)?;
        }
        Vertex::Condition(condition) => {
            out.raw(&state.context(condition.context)?.header)?;
            encoding::predicate(&condition.kind, &mut out)?;
            encoding::scope(&condition.scope, &mut out)?;
            out.count(condition.arguments.len())?;
            for argument in &condition.arguments {
                encode_id(*argument, &mut out)?;
            }
        }
        Vertex::Expression(expression) => {
            out.raw(&state.context(expression.context)?.header)?;
            encoding::operator(&expression.operator, &mut out)?;
            encoding::sort(&expression.sort, &mut out)?;
            encoding::scope(&expression.scope, &mut out)?;
            encoding::binding(expression.binding.as_ref(), &mut out)?;
            out.count(expression.operands.len())?;
            for operand in &expression.operands {
                encode_id(*operand, &mut out)?;
            }
        }
        Vertex::Occurrence(occurrence) => {
            encode_id(occurrence.expression, &mut out)?;
            for optional in [occurrence.source, occurrence.provenance] {
                out.byte(u8::from(optional.is_some()))?;
                if let Some(id) = optional {
                    encode_id(id, &mut out)?;
                }
            }
            out.byte(u8::from(occurrence.span.is_some()))?;
            if let Some(span) = occurrence.span {
                out.unsigned(span.start)?;
                out.unsigned(span.end)?;
            }
            out.count(occurrence.children.len())?;
            for child in &occurrence.children {
                encode_id(*child, &mut out)?;
            }
            out.count(occurrence.binder_names.len())?;
            for name in &occurrence.binder_names {
                out.name(name)?;
            }
        }
        Vertex::Binder(binding) => encoding::binding(Some(binding), &mut out)?,
        Vertex::Request(request) => {
            encoding::operation(&request.operation, &mut out)?;
            encode_id(request.context, &mut out)?;
            out.count(request.inputs.len())?;
            for input in &request.inputs {
                encode_id(*input, &mut out)?;
            }
            for kind in crate::budget::ALL_KINDS {
                out.unsigned(request.limits.get(kind))?;
            }
        }
    }
    Ok((kind, out.finish()))
}
