// SPDX-License-Identifier: AGPL-3.0-only
//! Complete bounded occurrence walks and fresh generated occurrence trees.

use std::fmt;

use crate::budget::{Budget, BudgetKind};
use crate::encoding::{self, Encoder};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, LogicalId, Vertex, size};
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
        let id = self.checked(root)?;
        let target = self.state.occurrence(id)?.expression;
        let (_, scope) = crate::descriptor::metadata(&self.state, target)?;
        let workspace = self
            .counts()
            .payload_bytes
            .checked_mul(2)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?;
        budget.check(BudgetKind::StagedPayloadBytes, workspace, at)?;
        budget.work(workspace, at)?;
        if !scope.is_empty() {
            return Err(Fault::invalid(InvalidCode::Scope, at));
        }
        let mut stack = vec![Frame {
            id,
            path: Vec::new(),
            expanded: false,
        }];
        let mut visited = 0_u64;
        let mut output = 0_u64;
        while let Some(frame) = stack.pop() {
            budget.work(1, at)?;
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
                let visit = self.make_visit(frame.id, frame.path, &mut output, budget)?;
                events.try_reserve(1).map_err(|_| Fault::allocation(at))?;
                events.push(WalkEvent::Visit(visit));
                continue;
            }
            self.enter_binder(&frame, &mut output, budget, events)?;
            let required = occurrence
                .children
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
            for (position, child) in occurrence.children.iter().enumerate().rev() {
                budget.work(
                    size(frame.path.len(), at)?
                        .checked_add(1)
                        .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?,
                    at,
                )?;
                let mut path = frame.path.clone();
                path.try_reserve(1).map_err(|_| Fault::allocation(at))?;
                path.push(
                    u32::try_from(position).map_err(|_| Fault::capacity(BudgetKind::Edges, at))?,
                );
                stack.push(Frame {
                    id: *child,
                    path,
                    expanded: false,
                });
            }
        }
        Ok(visited)
    }

    /// Emit a bounded binder-entry event before scheduling its children.
    fn enter_binder(
        &self,
        frame: &Frame,
        output: &mut u64,
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
            reserve_output(output, bytes, budget, at)?;
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
        output: &mut u64,
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
        reserve_output(output, bytes, budget, at)?;
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

/// Reserve output bytes before cloning event payloads.
fn reserve_output(
    total: &mut u64,
    amount: u64,
    budget: &mut Budget,
    at: Location,
) -> Result<(), Fault> {
    let next = total
        .checked_add(amount)
        .ok_or_else(|| Fault::capacity(BudgetKind::OutputBytes, at))?;
    budget.check(BudgetKind::OutputBytes, next, at)?;
    budget.work(amount, at)?;
    *total = next;
    Ok(())
}
