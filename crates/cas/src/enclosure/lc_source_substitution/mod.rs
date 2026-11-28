// SPDX-License-Identifier: AGPL-3.0-only
//! Original-source substitution of the registered conditional LC root law.

mod check;
mod service;
mod wire;
pub use service::{Certificate, Receipt};
#[cfg(test)]
mod tests;

use crate::enclosure::lc_root;
use crate::{Handle, Location};

/// One scoped accounting location for the new source theorem.
const AT: Location = Location::new("LC source substitution");
/// Distinct recipe identity; the registered root wire remains unchanged.
const RECIPE: &str = "lc-source-substitution";

/// Acceleration scope; neither tag asserts an image converse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tag {
    /// Independent finite rational original supports, conditional on the source domain.
    Box,
    /// Correlated finite original sources and ordered admitted maps; outer coverage only.
    MappedCover,
}

/// Complete original inventory and positional simultaneous substitution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    /// The existing free-Real theorem, retained without changing its wire.
    pub root: lc_root::Request,
    /// Original context with its complete conditional source domain.
    pub context: Handle,
    /// Original ordered source declarations, including repeated positions.
    pub sources: Vec<Handle>,
    /// Every original Boolean constraint, including repeats and unused conditions.
    pub constraints: Vec<Handle>,
    /// One original source expression for each positional root input.
    pub substitution: Vec<Handle>,
    /// The original ordered product, strict root, reciprocal and reverse DAGs.
    pub targets: [Handle; 4],
    /// Checked source grammar.
    pub tag: Tag,
    /// Exact schema revision; only one is admitted.
    pub revision: u32,
}

impl Request {
    /// All original source references in their retained positional order.
    pub(crate) fn handles(&self) -> impl Iterator<Item = &Handle> {
        std::iter::once(&self.context)
            .chain(&self.sources)
            .chain(&self.constraints)
            .chain(&self.substitution)
            .chain(&self.targets)
    }
}

/// A conditional original-source coverage theorem, with no existence converse.
pub const SCHEMA: crate::proof::LemmaSchema = crate::proof::LemmaSchema {
    name: RECIPE,
    label: "thm:cas:lc-source-substitution",
    version: 1,
    standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
    statement: "For every assignment to the listed original source declarations in their recorded order (including repeated positions), under the exact root context, admissible region, fibres and retained constraints, simultaneous substitution of the registered positive-root law into the original target DAG preserves its conditional forward enclosure conclusion. Root input binders and endpoint binders retain their original order and remain distinct classes.",
    premises: "The exact root context and ordered root constraints, source-domain extras, declared source leaves, positional substitution, positive root premises and original native enclosure obligations; no image converse or source independence is inferred from equal numerical values. A bindings consumer checks completeness and order against its physical declaration inventory. The ordinary receipt is store-scoped; indexed construction additionally retains a checked graph judgment for the same conditional claim.",
};
