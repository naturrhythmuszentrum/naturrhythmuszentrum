// SPDX-License-Identifier: AGPL-3.0-only
//! The graph owns complete finite Machin judgments and their authenticated citation edges.

mod index;
pub(crate) use index::PiIndex;

use super::{AnonymousLemma, Cas, Edge, EdgeIndex, LogicalId, Vertex, VertexKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::pi_bounds::PiBoundCertificate;
use std::sync::Arc;

/// Preserve the existing complete-certificate citation refusal boundary.
const AT: Location = Location::new("checked Machin certificate");

/// Complete checked fields and their first original consumer move into one graph node.
#[derive(Clone, Debug)]
pub(crate) struct PiInventory {
    /// This allocation supplies both graph payload and borrowed canonical index bytes.
    pub(crate) certificate: Arc<PiBoundCertificate>,
    /// Original expression at which the independent finite checker established this citation.
    pub(crate) original: LogicalId,
    /// Unchanged complete-certificate payload plus the existing fixed metadata allowance.
    pub(crate) bytes: u64,
}

/// The registered Machin theorem has no assumptions about its original consumer's context.
#[derive(Clone, Debug)]
pub(crate) struct PiLemma {
    /// Checked finite expansion, exact sums and complete outward bounds.
    pub(super) inventory: PiInventory,
    /// Strong source identity authenticates this independently checked instance.
    owner: Arc<()>,
    /// Rollback namespace in which the finite checker last reconstructed this instance.
    epoch: u64,
    /// Original-consumer-to-judgment checked-evidence edge.
    citation: EdgeIndex<u32>,
}

impl Cas {
    /// Authenticate the complete graph statement and its actual incoming evidence edge.
    pub(crate) fn pi_inventory(&self, id: LogicalId) -> Result<&PiInventory, Fault> {
        Ok(&self.pi_lemma(id)?.inventory)
    }

    /// Return only graph-owned authority checked under the current source generation.
    pub(crate) fn pi_judgment(&self, id: LogicalId) -> Result<Option<u64>, Fault> {
        let node = self.pi_lemma(id)?;
        let owned = Arc::ptr_eq(&node.owner, &self.memo_owner);
        let current = node.epoch == self.memo_epoch;
        let reusable = self.memo_epoch != u64::MAX;
        Ok((owned && current && reusable).then_some(node.inventory.bytes - 256))
    }

    /// Index entries cannot authorize missing or wrongly typed checked evidence.
    fn pi_lemma(&self, id: LogicalId) -> Result<&PiLemma, Fault> {
        let Vertex::AnonymousLemma(node) = self.state.node(id)? else {
            return Err(invalid());
        };
        let AnonymousLemma::Pi(node) = node.as_ref() else {
            return Err(invalid());
        };
        let source = self.state.indices[&node.inventory.original];
        let target = self.state.indices[&id];
        let endpoints = self.state.graph.edge_endpoints(node.citation) == Some((source, target));
        let checked = matches!(
            self.state.graph.edge_weight(node.citation),
            Some(Edge::CheckedEvidence)
        );
        if !(endpoints && checked) {
            return Err(invalid());
        }
        Ok(node)
    }

    /// Refresh only after the full finite checker and enclosing receipt both succeeded.
    pub(super) fn refresh_pi(&mut self, id: LogicalId) {
        let index = self.state.indices[&id];
        if let Vertex::AnonymousLemma(node) = &mut self.state.graph[index]
            && let AnonymousLemma::Pi(node) = node.as_mut()
        {
            node.owner = Arc::clone(&self.memo_owner);
            node.epoch = self.memo_epoch;
        }
        self.state
            .pi_judgments
            .refresh(&self.memo_owner, self.memo_epoch);
    }
}

/// Move the complete preflighted payload into the authoritative graph without a fallible suffix.
pub(super) fn append(cas: &mut Cas, inventory: PiInventory) {
    let state = &mut cas.state;
    let serial = state
        .next
        .get(&VertexKind::AnonymousLemma)
        .copied()
        .unwrap_or(0);
    let id = LogicalId {
        kind: VertexKind::AnonymousLemma,
        serial,
    };
    let original = state.indices[&inventory.original];
    let certificate = Arc::clone(&inventory.certificate);
    let bytes = inventory.bytes;
    let citation = EdgeIndex::new(state.graph.edge_count() + 1);
    let index = state
        .graph
        .add_node(Vertex::AnonymousLemma(Box::new(AnonymousLemma::Pi(
            PiLemma {
                inventory,
                owner: Arc::clone(&cas.memo_owner),
                epoch: cas.memo_epoch,
                citation,
            },
        ))));
    state.graph.add_edge(index, original, Edge::Input(0));
    state.graph.add_edge(original, index, Edge::CheckedEvidence);
    state.indices.insert(id, index);
    state.pi_judgments.insert(certificate, id);
    state.pi_judgments.refresh(&cas.memo_owner, cas.memo_epoch);
    state.next.insert(VertexKind::AnonymousLemma, serial + 1);
    state.payload = state.payload.saturating_add(bytes);
    state.judgment_bytes = state.judgment_bytes.saturating_add(bytes);
    state.judgment_count += 1;
}

/// Wrong graph kinds and unauthenticated evidence have no retained authority.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Reference, AT)
}
