// SPDX-License-Identifier: AGPL-3.0-only
//! Per-check derived results and occurrence-relative evidence replay.

use std::rc::Rc;

use nrz_cas::limits::BudgetKind;
use nrz_cas::{Fault, Location as CasLocation};

use super::{Declaration, Diagnostic, Expectation, Location, NodeEvidence, Source, State, Value};
use crate::original::unsupported;

/// One successful depth check in the original execution order.
#[derive(Clone)]
pub(super) struct DepthCheck {
    /// Exact source and declaration chain for this operation.
    location: Location,
    /// Combined original-path and declaration-expansion depth.
    depth: usize,
}

/// The diagnostic prefix and evidence boundaries before expanding a declaration.
pub(super) struct Checkpoint {
    /// Calling occurrence's original location.
    location: Location,
    /// Calling occurrence's expansion depth.
    depth: usize,
    /// First node contributed by the declaration body.
    nodes: usize,
    /// First body depth check, excluding the calling symbol's own checks.
    checks: usize,
}

/// A result is reusable only within its borrowed immutable declaration snapshot.
pub(super) struct Memo<'a> {
    /// Identity of the declaration borrowed from the check's immutable `Context`.
    pub(super) snapshot: &'a Declaration,
    /// Fully checked value and already staged coordinate reference.
    value: Value,
    /// First-use prefix to replace at later occurrences.
    base: Checkpoint,
    /// Body evidence, including expanded dependency evidence, in postorder.
    nodes: Vec<NodeEvidence>,
    /// Body depth checks in their original execution order.
    checks: Vec<DepthCheck>,
    /// Logical copying work for the retained result and evidence.
    work: u64,
}

impl Memo<'_> {
    /// Account for both the retained location and its new occurrence prefix during replay.
    fn replay_work(&self, path: &[usize], definitions: &[String]) -> Result<u64, Fault> {
        let mut prefix = add_work(0, path.len(), 0)?;
        for name in definitions {
            prefix = add_work(prefix, name.len(), 1)?;
        }
        let entries = self
            .nodes
            .len()
            .checked_add(self.checks.len())
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(unsupported)?;
        prefix
            .checked_mul(entries)
            .and_then(|n| self.work.checked_add(n))
            .ok_or_else(|| {
                Fault::capacity(
                    BudgetKind::Work,
                    CasLocation::new("bindings declaration memo"),
                )
            })
    }

    /// Replace only the caller prefix, retaining each body span and nested definition suffix.
    fn location(
        &self,
        old: &Location,
        path: &[usize],
        definitions: &[String],
    ) -> Result<Location, Fault> {
        let suffix = old
            .path
            .strip_prefix(self.base.location.path.as_slice())
            .ok_or_else(unsupported)?;
        let chain = old
            .definitions
            .strip_prefix(self.base.location.definitions.as_slice())
            .ok_or_else(unsupported)?;
        let mut location = Location {
            path: path.to_vec(),
            span: old.span,
            definitions: definitions.to_vec(),
        };
        location.path.extend_from_slice(suffix);
        location.definitions.extend_from_slice(chain);
        Ok(location)
    }
}

impl<'a> State<'a> {
    /// Borrow the immutable snapshot independently of mutable traversal state.
    pub(super) fn snapshot(
        &self,
        source: &Source,
        name: &str,
    ) -> Result<&'a Declaration, Diagnostic> {
        let context = self.context;
        context.declarations.get(name).ok_or_else(|| {
            self.error(
                source,
                crate::Failure::UnknownSymbol(name.to_owned()),
                Expectation::Rule("a declared numerical symbol"),
                Vec::new(),
            )
        })
    }

    /// Keep successful checks ordered so a deeper cache hit reports the same first refusal.
    pub(super) fn record_depth(
        &mut self,
        location: Location,
        depth: usize,
    ) -> Result<(), Diagnostic> {
        let at = CasLocation::new("bindings declaration depth");
        let needed = u64::try_from(depth).unwrap_or(u64::MAX);
        let limit = self.context.limits.depth.min(128);
        if needed > limit {
            return Err(depth_error(
                &location,
                Fault::exhausted(BudgetKind::Depth, limit, needed, at),
            ));
        }
        self.budget
            .work(1, at)
            .map_err(|error| depth_error(&location, error))?;
        self.depth_checks
            .try_reserve(1)
            .map_err(|_| depth_error(&location, Fault::allocation(at)))?;
        self.depth_checks.push(DepthCheck { location, depth });
        Ok(())
    }

    /// Capture prefixes before entering the declaration's own diagnostic scope.
    pub(super) fn checkpoint(&self, source: &Source, depth: usize) -> Checkpoint {
        Checkpoint {
            location: self.location(source),
            depth,
            nodes: self.nodes.len(),
            checks: self.depth_checks.len(),
        }
    }

    /// Publish a memo only after the entire declaration and its dependencies have succeeded.
    pub(super) fn remember(
        &mut self,
        source: &Source,
        name: &str,
        snapshot: &'a Declaration,
        value: &Value,
        base: Checkpoint,
    ) -> Result<(), Diagnostic> {
        let nodes = self
            .nodes
            .get(base.nodes..)
            .ok_or_else(unsupported)
            .map_err(|error| self.memo_error(source, error))?;
        let checks = self
            .depth_checks
            .get(base.checks..)
            .ok_or_else(unsupported)
            .map_err(|error| self.memo_error(source, error))?;
        let work =
            copy_work(nodes, checks, value).map_err(|error| self.memo_error(source, error))?;
        self.budget
            .work(work, CasLocation::new("bindings declaration memo"))
            .map_err(|error| self.memo_error(source, error))?;
        let memo = Memo {
            snapshot,
            value: value.clone(),
            base,
            nodes: copied(nodes).map_err(|error| self.memo_error(source, error))?,
            checks: copied(checks).map_err(|error| self.memo_error(source, error))?,
            work,
        };
        self.memo.insert(name.to_owned(), Rc::new(memo));
        Ok(())
    }

    /// Reuse coordinates and physical results while retaining every occurrence's own evidence.
    pub(super) fn reuse(
        &mut self,
        source: &Source,
        depth: usize,
        memo: &Memo<'a>,
    ) -> Result<Value, Diagnostic> {
        let work = memo
            .replay_work(&self.path, &self.definitions)
            .map_err(|error| self.memo_error(source, error))?;
        self.budget
            .work(work, CasLocation::new("bindings declaration memo"))
            .map_err(|error| self.memo_error(source, error))?;
        for check in &memo.checks {
            let location = memo
                .location(&check.location, &self.path, &self.definitions)
                .map_err(|error| self.memo_error(source, error))?;
            let level = check
                .depth
                .checked_sub(memo.base.depth)
                .and_then(|n| n.checked_add(depth))
                .ok_or_else(|| {
                    self.memo_error(
                        source,
                        Fault::capacity(
                            BudgetKind::Depth,
                            CasLocation::new("bindings declaration memo"),
                        ),
                    )
                })?;
            self.record_depth(location, level)?;
        }
        self.nodes.try_reserve(memo.nodes.len()).map_err(|_| {
            self.memo_error(
                source,
                Fault::allocation(CasLocation::new("bindings declaration memo")),
            )
        })?;
        for node in &memo.nodes {
            let location = memo
                .location(&node.location, &self.path, &self.definitions)
                .map_err(|error| self.memo_error(source, error))?;
            let mut node = node.clone();
            node.location = location;
            self.nodes.push(node);
        }
        Ok(memo.value.clone())
    }

    /// A failed memo allocation or copy belongs to the occurrence that requested it.
    fn memo_error(&self, source: &Source, error: Fault) -> Diagnostic {
        self.error(
            source,
            error.into(),
            Expectation::Rule("bounded declaration expansion"),
            Vec::new(),
        )
    }
}

/// Preserve the original named depth refusal independently of a `Source` view.
fn depth_error(location: &Location, error: Fault) -> Diagnostic {
    Diagnostic {
        location: Box::new(location.clone()),
        failure: error.into(),
        derived: Vec::new(),
        expected: Expectation::Rule("bounded original traversal and declaration expansion"),
    }
}

/// Copy bounded evidence after its work charge, using fallible vector reservation.
fn copied<T: Clone>(items: &[T]) -> Result<Vec<T>, Fault> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(items.len())
        .map_err(|_| Fault::allocation(CasLocation::new("bindings declaration memo")))?;
    result.extend_from_slice(items);
    Ok(result)
}

/// Charge each copied value, evidence record, path position and definition-name byte.
fn copy_work(nodes: &[NodeEvidence], checks: &[DepthCheck], value: &Value) -> Result<u64, Fault> {
    let mut work = 1_024_u64;
    for name in value
        .trace
        .dependencies
        .iter()
        .chain(value.trace.contributors.keys())
    {
        work = add_work(work, name.len(), 64)?;
    }
    for (location, overhead) in nodes
        .iter()
        .map(|node| (&node.location, 128))
        .chain(checks.iter().map(|check| (&check.location, 32)))
    {
        work = add_work(work, location.path.len(), overhead)?;
        for name in &location.definitions {
            work = add_work(work, name.len(), 1)?;
        }
    }
    Ok(work)
}

/// Checked logical byte/count arithmetic for the memo's actual copying work.
fn add_work(current: u64, count: usize, overhead: u64) -> Result<u64, Fault> {
    u64::try_from(count)
        .ok()
        .and_then(|n| n.checked_add(overhead))
        .and_then(|n| current.checked_add(n))
        .ok_or_else(|| {
            Fault::capacity(
                BudgetKind::Work,
                CasLocation::new("bindings declaration memo"),
            )
        })
}
