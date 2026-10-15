// SPDX-License-Identifier: AGPL-3.0-only
//! Complete bounded occurrence walks and fresh generated occurrence trees.

use std::collections::BTreeSet;
use std::fmt;

use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;

use crate::budget::{Budget, BudgetKind};
use crate::encoding::{self, Encoder};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Edge, Handle, LogicalId, Vertex, size, vertex_key, vertex_payload};
use crate::model::{AtomicSort, Scope, Span};

/// An original source identity and optional checked byte range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceLocation {
    /// Caller source identity.
    pub key: String,
    /// Byte range, absent when the original source has no precise span.
    pub span: Option<Span>,
}

/// A complete postorder visit to one distinct occurrence.
#[derive(Clone, Debug)]
pub struct Visit {
    /// The distinct original/generated occurrence.
    pub occurrence: Handle,
    /// Its shared typed syntax object; inspect it with `describe_syntax`.
    pub expression: Handle,
    /// Ordered root-relative operand positions.
    pub path: Vec<u32>,
    /// Source identity for an original occurrence.
    pub source: Option<SourceLocation>,
    /// Explicit bound frames at this syntax position.
    pub scope: Scope,
}

/// A coverage event; binder entry precedes the corresponding scoped child walk.
#[derive(Clone, Debug)]
pub enum WalkEvent {
    /// Enter the binding frame declared by this occurrence.
    EnterBinder {
        /// Binder-owning occurrence.
        occurrence: Handle,
        /// Root-relative path of that occurrence.
        path: Vec<u32>,
        /// Semantic frame sorts.
        sorts: Vec<AtomicSort>,
        /// Optional original display names.
        names: Vec<String>,
    },
    /// Visit one occurrence after all its children.
    Visit(Visit),
}

/// Sealed evidence of a complete occurrence walk, never a physical validation.
#[derive(Clone, Debug)]
pub struct TraversalReceipt {
    /// Certified immutable root, including runtime store identity.
    pub(crate) root: Handle,
    /// Number of distinct visited occurrences.
    pub(crate) visited: u64,
    /// Whether the walk certifies original rather than generated occurrences.
    pub(crate) original: bool,
}

impl TraversalReceipt {
    /// Read the number of occurrences covered by this sealed receipt.
    #[must_use]
    pub const fn visited(&self) -> u64 {
        self.visited
    }

    /// Read the certified root.
    #[must_use]
    pub const fn root(&self) -> &Handle {
        &self.root
    }

    /// Whether every visited occurrence retains an original source.
    #[must_use]
    pub const fn is_original(&self) -> bool {
        self.original
    }
}

/// All events and the receipt minted only after the complete walk succeeds.
#[derive(Clone, Debug)]
pub struct CompleteWalk {
    /// Stable binder-entry/postorder sequence.
    pub events: Vec<WalkEvent>,
    /// Coverage evidence; cannot be minted from a partial event sequence.
    pub receipt: TraversalReceipt,
}

/// Bounded partial diagnostics from a refused walk; this type contains no receipt.
#[derive(Clone, Debug)]
pub struct WalkFailure {
    /// The exact input or resource failure.
    pub fault: Fault,
    /// Events completed before refusal, useful only as partial diagnostics.
    pub events: Vec<WalkEvent>,
}

impl fmt::Display for WalkFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.fault, formatter)
    }
}

impl std::error::Error for WalkFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.fault)
    }
}

/// An explicit stack entry avoids recursive traversal and recursive dropping.
struct Frame {
    /// Occurrence to visit.
    id: LogicalId,
    /// Root-relative path.
    path: Vec<u32>,
    /// True when child visits have already been scheduled.
    expanded: bool,
}

/// Only this walk's live frames, output and visited dependency records count as workspace.
#[derive(Default)]
struct Workspace {
    /// Conservative bytes of scheduled frames, including the current frame.
    stack: u64,
    /// Retained encoded event payload plus fixed event overhead.
    output: u64,
    /// Records already charged through an occurrence or its interpretation dependencies.
    records: BTreeSet<NodeIndex<u32>>,
}

impl Workspace {
    /// Bound live stack, retained events, a scratch copy and the dependency frontier.
    fn check(&self, extra: u64, budget: &Budget) -> Result<(), Fault> {
        let at = Location::new("walk workspace");
        let bytes = self
            .output
            .checked_mul(2)
            .and_then(|n| n.checked_add(self.stack))
            .and_then(|n| {
                size(self.records.len(), at)
                    .ok()?
                    .checked_mul(64)?
                    .checked_add(n)
            })
            .and_then(|n| n.checked_add(extra))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, at)
    }

    /// Reserve frame storage and its copying work before allocating paths or stack entries.
    fn frames(&mut self, bytes: u64, budget: &mut Budget) -> Result<(), Fault> {
        self.check(bytes, budget)?;
        budget.work(bytes, Location::new("walk frames"))?;
        self.stack = self.stack.checked_add(bytes).ok_or_else(|| {
            Fault::capacity(BudgetKind::StagedPayloadBytes, Location::new("walk frames"))
        })?;
        Ok(())
    }

    /// Release a consumed frame while checking the workspace accounting invariant.
    fn release(&mut self, bytes: u64) -> Result<(), Fault> {
        self.stack = self
            .stack
            .checked_sub(bytes)
            .ok_or_else(|| Fault::invalid(InvalidCode::Reference, Location::new("walk frames")))?;
        Ok(())
    }

    /// Reserve emitted payload before copying it into the retained event sequence.
    fn event(&mut self, bytes: u64, budget: &mut Budget, at: Location) -> Result<(), Fault> {
        let next = self
            .output
            .checked_add(bytes)
            .ok_or_else(|| Fault::capacity(BudgetKind::OutputBytes, at))?;
        budget.check(BudgetKind::OutputBytes, next, at)?;
        self.check(
            bytes
                .checked_mul(2)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?,
            budget,
        )?;
        budget.work(bytes, at)?;
        self.output = next;
        Ok(())
    }

    /// Charge each reached immutable record once, following interpretation dependencies.
    fn reach(&mut self, cas: &Cas, root: LogicalId, budget: &mut Budget) -> Result<(), Fault> {
        let at = Location::new("walk dependencies");
        let index = cas
            .state
            .indices
            .get(&root)
            .copied()
            .ok_or_else(|| Fault::invalid(InvalidCode::Reference, at))?;
        self.check(32, budget)?;
        let mut pending = vec![index];
        while let Some(index) = pending.pop() {
            budget.work(1, at)?;
            if self.records.contains(&index) {
                continue;
            }
            self.check(frontier_bytes(pending.len(), 64)?, budget)?;
            self.records.insert(index);
            self.charge(cas, index, frontier_bytes(pending.len(), 0)?, budget)?;
            for edge in cas.state.graph.edges(index) {
                if matches!(edge.weight(), Edge::Child(_)) {
                    continue;
                }
                self.check(frontier_bytes(pending.len(), 32)?, budget)?;
                pending.try_reserve(1).map_err(|_| Fault::allocation(at))?;
                pending.push(edge.target());
            }
        }
        Ok(())
    }

    /// A whole-store coverage pays at least twice its logical payload, without a store scan.
    fn charge(
        &self,
        cas: &Cas,
        index: NodeIndex<u32>,
        frontier: u64,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let at = Location::new("walk record");
        let vertex = cas
            .state
            .graph
            .node_weight(index)
            .ok_or_else(|| Fault::invalid(InvalidCode::Reference, at))?;
        let (_, key) = vertex_key(vertex, &cas.state, budget)?;
        self.check(
            size(key.len(), at)?
                .checked_add(frontier)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?,
            budget,
        )?;
        let payload = vertex_payload(
            vertex,
            key.len(),
            cas.state.graph.edges(index).count(),
            budget,
        )?;
        let work = payload
            .checked_mul(2)
            .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?;
        budget.work(work, at)
    }
}

/// Conservative bytes of a pending dependency index vector plus one prospective allocation.
fn frontier_bytes(count: usize, extra: u64) -> Result<u64, Fault> {
    let at = Location::new("walk dependencies");
    size(count, at)?
        .checked_mul(32)
        .and_then(|n| n.checked_add(extra))
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))
}

/// Each frame owns a path; reserve fixed metadata and twice its encoded positions.
fn frame_bytes(depth: usize) -> Result<u64, Fault> {
    let at = Location::new("walk frames");
    size(depth, at)?
        .checked_mul(8)
        .and_then(|n| n.checked_add(128))
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))
}

impl Cas {
    /// Visit every original syntactic operand, including zero/canceled operands.
    ///
    /// # Errors
    /// Refuses foreign roots, open scopes, mixed provenance, or budget exhaustion.
    /// A partial walk never returns a traversal receipt.
    pub fn walk_original(
        &self,
        root: &Handle,
        budget: &mut Budget,
    ) -> Result<CompleteWalk, WalkFailure> {
        self.walk(root, true, budget)
    }

    /// Visit a freshly materialized generated tree without inventing source spans.
    ///
    /// # Errors
    /// Refuses foreign roots, original-source occurrences or exhausted budgets.
    pub fn walk_derived(
        &self,
        root: &Handle,
        budget: &mut Budget,
    ) -> Result<CompleteWalk, WalkFailure> {
        self.walk(root, false, budget)
    }

    /// Mint coverage only after the event producer has reached the complete root.
    fn walk(
        &self,
        root: &Handle,
        original: bool,
        budget: &mut Budget,
    ) -> Result<CompleteWalk, WalkFailure> {
        let mut events = Vec::new();
        match self.walk_events(root, original, budget, &mut events) {
            Ok(visited) => Ok(CompleteWalk {
                events,
                receipt: TraversalReceipt {
                    root: root.clone(),
                    visited,
                    original,
                },
            }),
            Err(fault) => Err(WalkFailure { fault, events }),
        }
    }

    /// Walk one finite occurrence tree on an explicit, metered stack.
    fn walk_events(
        &self,
        root: &Handle,
        original: bool,
        budget: &mut Budget,
        events: &mut Vec<WalkEvent>,
    ) -> Result<u64, Fault> {
        let at = Location::new("occurrence walk");
        let mut workspace = Workspace::default();
        let mut stack = self.walk_stack(root, &mut workspace, budget)?;
        let mut visited = 0_u64;
        while let Some(frame) = stack.pop() {
            budget.work(1, at)?;
            let bytes = frame_bytes(frame.path.len())?;
            let occurrence = self.state.occurrence(frame.id)?;
            if occurrence.source.is_some() != original {
                return Err(Fault::invalid(InvalidCode::Span, at));
            }
            budget.check(
                BudgetKind::Depth,
                size(frame.path.len(), at)?
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::Depth, at))?,
                at,
            )?;
            if frame.expanded {
                visited = visited
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::OriginalOccurrences, at))?;
                budget.check(BudgetKind::OriginalOccurrences, visited, at)?;
                let visit = self.make_visit(frame.id, frame.path, &mut workspace, budget)?;
                events.try_reserve(1).map_err(|_| Fault::allocation(at))?;
                events.push(WalkEvent::Visit(visit));
                workspace.release(bytes)?;
                continue;
            }
            workspace.reach(self, frame.id, budget)?;
            self.enter_binder(&frame, &mut workspace, budget, events)?;
            schedule(
                &mut stack,
                &frame,
                &occurrence.children,
                &mut workspace,
                budget,
            )?;
            workspace.release(bytes)?;
        }
        Ok(visited)
    }

    /// Validate the root scope and reserve the first frame before allocating the walk stack.
    fn walk_stack(
        &self,
        root: &Handle,
        workspace: &mut Workspace,
        budget: &mut Budget,
    ) -> Result<Vec<Frame>, Fault> {
        let at = Location::new("occurrence walk");
        let id = self.checked(root)?;
        let target = self.state.occurrence(id)?.expression;
        let (_, scope) = crate::descriptor::metadata(&self.state, target)?;
        if !scope.is_empty() {
            return Err(Fault::invalid(InvalidCode::Scope, at));
        }
        workspace.frames(frame_bytes(0)?, budget)?;
        Ok(vec![Frame {
            id,
            path: Vec::new(),
            expanded: false,
        }])
    }

    /// Emit a bounded binder-entry event before scheduling its children.
    fn enter_binder(
        &self,
        frame: &Frame,
        workspace: &mut Workspace,
        budget: &mut Budget,
        events: &mut Vec<WalkEvent>,
    ) -> Result<(), Fault> {
        let at = Location::new("occurrence walk");
        let occurrence = self.state.occurrence(frame.id)?;
        if let Some(binding) = crate::descriptor::binding(&self.state, occurrence.expression)? {
            let mut out = Encoder::new(budget, at);
            encoding::binding(Some(binding), &mut out)?;
            out.count(frame.path.len())?;
            for position in &frame.path {
                out.unsigned(u64::from(*position))?;
            }
            for name in &occurrence.binder_names {
                out.name(name)?;
            }
            let bytes = size(out.finish().len(), at)?
                .checked_add(128)
                .ok_or_else(|| Fault::capacity(BudgetKind::OutputBytes, at))?;
            workspace.event(bytes, budget, at)?;
            events.try_reserve(1).map_err(|_| Fault::allocation(at))?;
            events.push(WalkEvent::EnterBinder {
                occurrence: self.handle(frame.id),
                path: frame.path.clone(),
                sorts: binding.sorts.clone(),
                names: occurrence.binder_names.clone(),
            });
        }
        Ok(())
    }

    /// Materialize one bounded postorder visit without duplicating source text.
    fn make_visit(
        &self,
        id: LogicalId,
        path: Vec<u32>,
        workspace: &mut Workspace,
        budget: &mut Budget,
    ) -> Result<Visit, Fault> {
        let at = Location::new("occurrence visit");
        let occurrence = self.state.occurrence(id)?;
        let (_, scope) = crate::descriptor::metadata(&self.state, occurrence.expression)?;
        let mut out = Encoder::new(budget, at);
        encoding::scope(scope, &mut out)?;
        out.count(path.len())?;
        for position in &path {
            out.unsigned(u64::from(*position))?;
        }
        let source = if let Some(id) = occurrence.source {
            let Vertex::Source(source) = self.state.node(id)? else {
                return Err(Fault::invalid(InvalidCode::Reference, at));
            };
            out.name(&source.key)?;
            Some(source)
        } else {
            None
        };
        let bytes = size(out.finish().len(), at)?
            .checked_add(96)
            .ok_or_else(|| Fault::capacity(BudgetKind::OutputBytes, at))?;
        workspace.event(bytes, budget, at)?;
        Ok(Visit {
            occurrence: self.handle(id),
            expression: self.handle(occurrence.expression),
            path,
            source: source.map(|source| SourceLocation {
                key: source.key.clone(),
                span: occurrence.span,
            }),
            scope: scope.clone(),
        })
    }

    /// Expand a shared expression into distinct generated occurrences transactionally.
    ///
    /// # Errors
    /// Refuses foreign handles, invalid provenance or expression/occurrence/work limits.
    /// The resulting root must be walked separately before a client uses its receipt.
    pub fn materialize_derived(
        &mut self,
        expression: &Handle,
        provenance: &Handle,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        crate::source_build::materialize(self, expression, None, Some(provenance), budget)
    }

    /// Build every original typed use in written order, without inventing precise spans.
    ///
    /// # Errors
    /// Refuses invalid source/syntax identities and exhausted construction limits.
    /// The resulting root still requires a successful complete original walk.
    pub fn materialize_original(
        &mut self,
        syntax: &Handle,
        source: &Handle,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        crate::source_build::materialize(self, syntax, Some(source), None, budget)
    }
}

/// Prepay all new frames before cloning the parent's path into their owned paths.
fn schedule(
    stack: &mut Vec<Frame>,
    frame: &Frame,
    children: &[LogicalId],
    workspace: &mut Workspace,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let at = Location::new("walk frames");
    let depth = frame
        .path
        .len()
        .checked_add(1)
        .ok_or_else(|| Fault::capacity(BudgetKind::Depth, at))?;
    let bytes = frame_bytes(depth)?
        .checked_mul(size(children.len(), at)?)
        .and_then(|n| n.checked_add(frame_bytes(frame.path.len()).ok()?))
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?;
    workspace.frames(bytes, budget)?;
    let required = children
        .len()
        .checked_add(1)
        .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, at))?;
    stack
        .try_reserve(required)
        .map_err(|_| Fault::allocation(at))?;
    stack.push(Frame {
        id: frame.id,
        path: frame.path.clone(),
        expanded: true,
    });
    for (position, child) in children.iter().enumerate().rev() {
        let mut path = frame.path.clone();
        path.try_reserve(1).map_err(|_| Fault::allocation(at))?;
        path.push(u32::try_from(position).map_err(|_| Fault::capacity(BudgetKind::Edges, at))?);
        stack.push(Frame {
            id: *child,
            path,
            expanded: false,
        });
    }
    Ok(())
}
