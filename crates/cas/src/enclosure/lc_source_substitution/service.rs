// SPDX-License-Identifier: AGPL-3.0-only
//! Checked conditional source substitution and independent receiving-store replay.

use super::{AT, Request, SCHEMA, check, wire};
use crate::budget::{Budget, BudgetKind};
use crate::enclosure::lc_root;
use crate::{Cas, Fault, InvalidCode};
use std::sync::Arc;

/// Untrusted portable proposal; possession grants no checked theorem.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Certificate(Vec<u8>);

impl Certificate {
    /// Copy bounded untrusted evidence before independent replay.
    /// # Errors
    /// Refuses caller resource exhaustion.
    pub fn from_bytes(bytes: &[u8], budget: &mut Budget) -> Result<Self, Fault> {
        budget.check(BudgetKind::WitnessBytes, bytes.len() as u64, AT)?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes.len() as u64, AT)?;
        budget.work(bytes.len() as u64, AT)?;
        let mut copy = Vec::new();
        copy.try_reserve_exact(bytes.len())
            .map_err(|_| Fault::allocation(AT))?;
        copy.extend_from_slice(bytes);
        Ok(Self(copy))
    }

    /// Export only untrusted portable bytes.
    /// # Errors
    /// Refuses caller resource exhaustion.
    pub fn to_bytes(&self, budget: &mut Budget) -> Result<Vec<u8>, Fault> {
        Self::from_bytes(&self.0, budget).map(|value| value.0)
    }
}

/// Store-scoped conditional theorem, retaining its original graph and root authority.
#[derive(Clone, Debug)]
pub struct Receipt {
    /// Complete original statement.
    pub(crate) request: Request,
    /// Live registered root theorem.
    pub(crate) root: lc_root::Receipt,
    /// Portable checked proposal, copied on export.
    pub(crate) wire: Vec<u8>,
    /// Exact receiving-store owner.
    pub(crate) owner: Arc<()>,
}

impl Receipt {
    /// Actual local retained record and transcript residence, excluding the shared graph.
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        let count = self.request.sources.capacity()
            + self.request.constraints.capacity()
            + self.request.substitution.capacity()
            + self.request.root.inputs.capacity()
            + self.request.root.constraints.capacity();
        size_of::<Self>() as u64
            + (count as u64).saturating_mul(size_of::<crate::Handle>() as u64)
            + self.wire.capacity() as u64
    }

    /// Recheck the complete live original inventory before exposing the claim.
    /// # Errors
    /// Refuses a foreign store, stale original or exhausted caller ledger.
    pub fn request<'a>(&'a self, cas: &Cas, budget: &mut Budget) -> Result<&'a Request, Fault> {
        if !Arc::ptr_eq(&self.owner, &cas.memo_owner) {
            return Err(Fault::invalid(InvalidCode::ForeignHandle, AT));
        }
        check::verify(cas, &self.request, &self.root, budget)?;
        Ok(&self.request)
    }

    /// This schema asserts conditional coverage, never image equality.
    #[must_use]
    pub const fn formal_schema(&self) -> &'static crate::proof::LemmaSchema {
        &SCHEMA
    }

    /// Export a copy of the complete portable proposal.
    /// # Errors
    /// Refuses foreign/stale authority or exhausted caller resources.
    pub fn certificate(&self, cas: &Cas, budget: &mut Budget) -> Result<Certificate, Fault> {
        self.request(cas, budget)?;
        Certificate::from_bytes(&self.wire, budget)
    }
}

impl Cas {
    /// Reuse or publish a graph-owned source substitution for one particular root proof.
    /// # Errors
    /// Refuses nested publication, foreign sessions, changed originals and exhausted resources.
    pub fn construct_lc_source_substitution_indexed(
        &mut self,
        session: &mut crate::exact_cache::ExactCacheSession,
        request: &Request,
        root: &lc_root::Receipt,
        budget: &mut Budget,
    ) -> Result<Receipt, Fault> {
        if self.state.is_journaled() {
            return Err(Fault::invalid(InvalidCode::Reference, AT));
        }
        session.prepare(self)?;
        {
            let scope =
                budget.with_work_phase("lc-substitution-index", "key-construction", |budget| {
                    wire::scope(self, request, budget)
                })?;
            let root_wire =
                budget.with_work_phase("lc-substitution-index", "key-construction", |budget| {
                    root.certificate(self, budget)?.to_bytes(budget)
                })?;
            let wire =
                budget.with_work_phase("lc-substitution-index", "key-construction", |budget| {
                    wire::encode(&scope, &root_wire, budget)
                })?;
            if let Some(judgment) =
                budget.with_work_phase("lc-substitution-index", "probe", |budget| {
                    session.lookup(
                        self,
                        crate::exact_cache::Kind::Substitution,
                        &scope,
                        Some(&wire),
                        Some(&root.judgment),
                        budget,
                    )
                })?
            {
                let retained = budget.with_work_phase(
                    "lc-substitution-index",
                    "authentication",
                    |budget| self.lc_source_substitution_receipt(&judgment, budget),
                )?;
                if &retained.request == request && retained.root.judgment == root.judgment {
                    return budget.with_work_phase("lc-substitution-index", "copy", |budget| {
                        budget.check(
                            BudgetKind::OutputBytes,
                            self.retained_judgment_bytes()
                                .saturating_add(retained.retained_bytes()),
                            AT,
                        )?;
                        retain(
                            self,
                            &retained.request,
                            &retained.root,
                            &retained.wire,
                            budget,
                        )
                    });
                }
            }
            let proposal = session.reserve(&scope, budget)?;
            let (receipt, judgment, proof) =
                budget.with_retained_payload(proposal.retained_bytes(), AT, |budget| {
                    check::verify(self, request, root, budget)?;
                    let receipt = retain(self, request, root, &wire, budget)?;
                    let (judgment, proof) =
                        budget.with_retained_payload(receipt.retained_bytes(), AT, |budget| {
                            self.transaction(budget, |cas, budget| {
                                cas.retain_lc_source_substitution(
                                    receipt.clone(),
                                    proposal
                                        .retained_bytes()
                                        .saturating_add(receipt.retained_bytes()),
                                    budget,
                                )
                            })
                        })?;
                    Ok((receipt, judgment, proof))
                })?;
            session.admit(
                crate::exact_cache::Kind::Substitution,
                proposal,
                proof,
                Some(root.judgment.clone()),
                judgment,
            );
            Ok(receipt)
        }
    }

    /// Compose a checked registered root with one original source grammar.
    /// # Errors
    /// Refuses unsupported source syntax, an invalid root or exhausted resources.
    pub fn construct_lc_source_substitution(
        &self,
        request: &Request,
        root: &lc_root::Receipt,
        budget: &mut Budget,
    ) -> Result<Receipt, Fault> {
        check::verify(self, request, root, budget)?;
        let scope = wire::scope(self, request, budget)?;
        let root_wire = root.certificate(self, budget)?.to_bytes(budget)?;
        let wire = wire::encode(&scope, &root_wire, budget)?;
        retain(self, request, root, &wire, budget)
    }

    /// Replay against a receiver's expected original claim and registered root law.
    /// # Errors
    /// Refuses changed originals, wrong tags, invalid proof bytes and exhausted resources.
    pub fn check_lc_source_substitution(
        &mut self,
        expected: &Request,
        certificate: &Certificate,
        budget: &mut Budget,
    ) -> Result<Receipt, Fault> {
        let scope = wire::scope(self, expected, budget)?;
        let root_wire = wire::decode(&certificate.0, &scope, budget)?;
        let root_certificate = lc_root::Certificate::from_bytes(root_wire, budget)?;
        // Original syntax is checked before the root replay publishes its judgment.
        check::verify_originals(self, expected, budget)?;
        self.transaction(budget, |candidate, budget| {
            let root = candidate.check_lc_root(&expected.root, &root_certificate, budget)?;
            check::verify(candidate, expected, &root, budget)?;
            retain(candidate, expected, &root, &certificate.0, budget)
        })
    }
}

/// Prepay positional copies and the complete retained output before allocation.
fn retain(
    cas: &Cas,
    request: &Request,
    root: &lc_root::Receipt,
    wire: &[u8],
    budget: &mut Budget,
) -> Result<Receipt, Fault> {
    let handles = request.handles().count() + request.root.handles().count();
    let arrays = request.sources.len()
        + request.constraints.len()
        + request.substitution.len()
        + request.root.inputs.len()
        + request.root.constraints.len();
    let bytes = (size_of::<Receipt>() as u64)
        .saturating_add((arrays as u64).saturating_mul(size_of::<crate::Handle>() as u64))
        .saturating_add(wire.len() as u64);
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.check(BudgetKind::OutputBytes, bytes, AT)?;
    budget.work((handles as u64).saturating_add(wire.len() as u64), AT)?;
    let mut copy = Vec::new();
    copy.try_reserve_exact(wire.len())
        .map_err(|_| Fault::allocation(AT))?;
    copy.extend_from_slice(wire);
    let result = Receipt {
        request: request.clone(),
        root: root.clone(),
        wire: copy,
        owner: Arc::clone(&cas.memo_owner),
    };
    budget.check(BudgetKind::OutputBytes, result.retained_bytes(), AT)?;
    Ok(result)
}
