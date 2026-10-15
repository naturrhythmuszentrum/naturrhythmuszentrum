// SPDX-License-Identifier: AGPL-3.0-only
//! Atomic original case-form builders; complete walks mint receipts separately.

use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Draft, Handle};
use crate::source_model::{SourceFormDraft, SourceFormKind};

/// Original typed source syntax and its complete distinct-use occurrence root.
#[derive(Clone, Debug)]
pub struct OriginalForm {
    /// Shared immutable source descriptor.
    pub syntax: Handle,
    /// Original occurrence tree, with no traversal receipt yet.
    pub root: Handle,
}

/// Build an ordered original form with all selectors, values and the optional otherwise branch.
///
/// # Errors
/// Refuses malformed syntax, foreign source identities or exhausted resources atomically.
pub fn ordered_cases(
    cas: &mut Cas,
    mut draft: SourceFormDraft,
    source: &Handle,
    budget: &mut Budget,
) -> Result<OriginalForm, Fault> {
    draft.kind = SourceFormKind::OrderedCases;
    build(cas, draft, source, budget)
}

/// Build an original unordered case literal without erasing its written source order.
///
/// # Errors
/// Refuses otherwise clauses, malformed syntax or exhausted resources atomically.
pub fn case_literal(
    cas: &mut Cas,
    mut draft: SourceFormDraft,
    source: &Handle,
    budget: &mut Budget,
) -> Result<OriginalForm, Fault> {
    draft.kind = SourceFormKind::CaseLiteral;
    build(cas, draft, source, budget)
}

/// Source syntax and all distinct original uses publish together.
fn build(
    cas: &mut Cas,
    draft: SourceFormDraft,
    source: &Handle,
    budget: &mut Budget,
) -> Result<OriginalForm, Fault> {
    cas.transaction(budget, |candidate, budget| {
        let syntax = crate::case_syntax::admit(candidate, Draft::SourceForm(draft), budget)?;
        let root = candidate.materialize_original(&syntax, source, budget)?;
        Ok(OriginalForm { syntax, root })
    })
}
