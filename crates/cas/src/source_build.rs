// SPDX-License-Identifier: AGPL-3.0-only
//! Transactional complete occurrence builders over typed syntax dependencies.

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Draft, Handle, LogicalId, OccurrenceDraft, Reference, size};

/// Construction accounting location.
const AT: Location = Location::new("typed occurrence construction");

/// Pending original uses are never deduplicated, even when their syntax is interned.
struct Builder<'a> {
    /// Store supplying immutable syntax during staging.
    cas: &'a Cas,
    /// Original source, absent for generated uses.
    source: Option<&'a Handle>,
    /// Checked origin of generated uses.
    provenance: Option<&'a Handle>,
    /// Child-before-parent occurrence drafts.
    drafts: Vec<Draft>,
    /// Completed child references.
    results: Vec<Reference>,
    /// Total expanded uses, including pending ones.
    count: u64,
}

/// Stage the whole unfolded tree, then publish all occurrences together.
pub(crate) fn materialize(
    cas: &mut Cas,
    syntax: &Handle,
    source: Option<&Handle>,
    provenance: Option<&Handle>,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let root = cas.checked(syntax)?;
    let mut builder = Builder {
        cas,
        source,
        provenance,
        drafts: Vec::new(),
        results: Vec::new(),
        count: 0,
    };
    let mut pending = vec![(root, 1_u64, false)];
    while let Some((id, depth, expanded)) = pending.pop() {
        budget.work(1, AT)?;
        let children = crate::descriptor::children(&cas.state, id, budget)?;
        if expanded {
            builder.finish(id, children.len(), budget)?;
        } else {
            builder.reserve(depth, children.len(), budget)?;
            let next = depth
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::Depth, AT))?;
            pending
                .try_reserve(children.len().saturating_add(1))
                .map_err(|_| Fault::allocation(AT))?;
            pending.push((id, depth, true));
            pending.extend(
                children
                    .into_iter()
                    .rev()
                    .map(|(_, child)| (child, next, false)),
            );
        }
    }
    let drafts = builder.drafts;
    cas.admit(&drafts, budget)?
        .last()
        .cloned()
        .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))
}

impl Builder<'_> {
    /// Reserve count, depth and conservative retained workspace before expansion.
    fn reserve(&mut self, depth: u64, children: usize, budget: &mut Budget) -> Result<(), Fault> {
        self.count = self
            .count
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::OriginalOccurrences, AT))?;
        let pending = self
            .count
            .checked_add(size(children, AT)?)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedVertices, AT))?;
        budget.check(BudgetKind::OriginalOccurrences, self.count, AT)?;
        budget.check(BudgetKind::StagedVertices, pending, AT)?;
        budget.check(BudgetKind::Depth, depth, AT)?;
        let bytes = pending
            .checked_mul(512)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        budget.work(512, AT)
    }

    /// Assemble a distinct occurrence after all of its typed children have completed.
    fn finish(&mut self, id: LogicalId, count: usize, budget: &mut Budget) -> Result<(), Fault> {
        let start = self
            .results
            .len()
            .checked_sub(count)
            .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))?;
        let index = u32::try_from(self.drafts.len())
            .map_err(|_| Fault::capacity(BudgetKind::StagedVertices, AT))?;
        self.drafts
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        self.drafts.push(Draft::Occurrence(OccurrenceDraft {
            expression: Reference::Existing(self.cas.handle(id)),
            source: self.source.cloned().map(Reference::Existing),
            span: None,
            children: self.results.split_off(start),
            provenance: self.provenance.cloned().map(Reference::Existing),
            binder_names: Vec::new(),
        }));
        self.results
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        self.results.push(Reference::Local(index));
        budget.work(1, AT)
    }
}
