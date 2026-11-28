// SPDX-License-Identifier: AGPL-3.0-only
//! One caller ledger and atomic graph publication for construction and independent replay.

use super::{
    AT, Candidate, Events, Request, SCHEMA, candidate_bytes, capacity, invalid, request_bytes,
};
use crate::budget::{Budget, BudgetKind};
use crate::{Cas, Fault, Handle, InvalidCode};
use std::sync::Arc;

/// Untrusted portable bytes; decoding or possession alone establishes no mathematical authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Certificate(Vec<u8>);

impl Certificate {
    /// Whether both certificates refer to the same allocated byte buffer.
    ///
    /// This identity-only observation survives wrapper moves. Equal independent
    /// copies are distinct; zero-capacity certificates have no buffer and return
    /// false, including against themselves. No address, bytes, capacity or proof
    /// authority is exported, and no allocation or reservation occurs.
    #[must_use]
    pub fn shares_allocation(&self, other: &Self) -> bool {
        self.0.capacity() != 0
            && other.0.capacity() != 0
            && std::ptr::eq(self.0.as_ptr(), other.0.as_ptr())
    }

    /// Actual retained input residence, available without copying untrusted bytes.
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        size_of::<Self>() as u64 + self.0.capacity() as u64
    }

    /// Copy a bounded untrusted transcript; semantic checking occurs only in `Cas::check_lc_root`.
    /// # Errors
    /// Refuses exhausted witness, staged-byte or Work limits before copying.
    pub fn from_bytes(bytes: &[u8], budget: &mut Budget) -> Result<Self, Fault> {
        budget.with_retained_payload(bytes.len() as u64, AT, |budget| {
            copy(bytes, budget).map(Self)
        })
    }

    /// Export the exact proposed bytes without granting them checked status.
    /// # Errors
    /// Refuses exhausted witness, staged-byte or Work limits before copying.
    pub fn to_bytes(&self, budget: &mut Budget) -> Result<Vec<u8>, Fault> {
        budget.with_retained_payload(
            self.0.capacity() as u64 + size_of::<Self>() as u64,
            AT,
            |budget| copy(&self.0, budget),
        )
    }
}

/// Sealed durable judgment identity; its original CAS must remain live to inspect its claim.
#[derive(Clone, Debug)]
pub struct Receipt {
    /// A graph handle authenticates store identity and rollback lifetime.
    pub(crate) judgment: Handle,
    /// Exact graph owner, distinct even for an internal copy sharing logical handle identities.
    owner: Arc<()>,
}

impl Receipt {
    /// Read the complete original request from the authenticated graph-owned claim.
    /// The mathematical result is the conditional implication in `SCHEMA`, not a ground bound.
    /// # Errors
    /// Refuses a foreign store, rolled-back identity or missing checked-evidence edge.
    pub fn request<'a>(&self, cas: &'a Cas, budget: &mut Budget) -> Result<&'a Request, Fault> {
        self.authenticate(cas)?;
        cas.lc_root_request(&self.judgment, budget)
    }

    /// A logical-token copy alone cannot import a source store's checked receipt.
    fn authenticate(&self, cas: &Cas) -> Result<(), Fault> {
        if Arc::ptr_eq(&self.owner, &cas.memo_owner) {
            Ok(())
        } else {
            Err(Fault::invalid(InvalidCode::ForeignHandle, AT))
        }
    }

    /// Closed mathematical schema, whose premises remain conditional in every receipt.
    #[must_use]
    pub const fn formal_schema(&self) -> &'static crate::proof::LemmaSchema {
        &SCHEMA
    }

    /// Export a portable transcript from the retained graph, independent of source allocation IDs.
    /// # Errors
    /// Refuses foreign or stale receipts and exhausted caller resources.
    pub fn certificate(&self, cas: &Cas, budget: &mut Budget) -> Result<Certificate, Fault> {
        self.authenticate(cas)?;
        cas.with_retained_graph(budget, |budget| {
            copy(cas.lc_root_wire(&self.judgment, budget)?, budget).map(Certificate)
        })
    }
}

impl Cas {
    /// Construct and independently check the conditional LC family before durable publication.
    /// # Errors
    /// Refuses invalid original syntax, unsupported arithmetic revisions and exhausted resources.
    /// An error publishes no judgment and preserves consumed caller Work.
    pub fn construct_lc_root(
        &mut self,
        request: &Request,
        budget: &mut Budget,
    ) -> Result<Receipt, Fault> {
        run(self, request, None, false, 0, budget).map(|result| result.0)
    }

    /// Independently replay portable evidence against this store's complete expected original claim.
    /// # Errors
    /// Refuses foreign or modified originals, invalid or wrong-category evidence and exhaustion.
    /// No producer is called and no partial checked receipt is published.
    pub fn check_lc_root(
        &mut self,
        request: &Request,
        certificate: &Certificate,
        budget: &mut Budget,
    ) -> Result<Receipt, Fault> {
        run(self, request, Some(certificate), false, 0, budget).map(|result| result.0)
    }

    /// Reuse a checked graph root in one caller-held rollback generation.
    /// # Errors
    /// Refuses nested publication, foreign sessions, invalid originals and exhausted resources.
    pub fn construct_lc_root_indexed(
        &mut self,
        session: &mut crate::exact_cache::ExactCacheSession,
        request: &Request,
        budget: &mut Budget,
    ) -> Result<Receipt, Fault> {
        if self.state.is_journaled() {
            return Err(Fault::invalid(InvalidCode::Reference, AT));
        }
        session.prepare(self)?;
        {
            let statement =
                budget.with_work_phase("lc-root-index", "key-construction", |budget| {
                    super::wire::scope(self, request, budget)
                })?;
            if let Some(judgment) = budget.with_work_phase("lc-root-index", "probe", |budget| {
                session.lookup(
                    self,
                    crate::exact_cache::Kind::Root,
                    &statement,
                    None,
                    None,
                    budget,
                )
            })? {
                let matches =
                    budget.with_work_phase("lc-root-index", "authentication", |budget| {
                        Ok::<_, Fault>(self.lc_root_request(&judgment, budget)? == request)
                    })?;
                if matches {
                    return budget.with_work_phase("lc-root-index", "copy", |budget| {
                        budget.check(
                            BudgetKind::OutputBytes,
                            self.retained_judgment_bytes()
                                .saturating_add(size_of::<Receipt>() as u64),
                            AT,
                        )?;
                        Ok(Receipt {
                            judgment,
                            owner: Arc::clone(&self.memo_owner),
                        })
                    });
                }
            }
            let proposal = session.reserve(&statement, budget)?;
            let (receipt, proof) =
                budget.with_retained_payload(proposal.retained_bytes(), AT, |budget| {
                    run(
                        self,
                        request,
                        None,
                        true,
                        proposal
                            .retained_bytes()
                            .saturating_add(size_of::<Receipt>() as u64),
                        budget,
                    )
                })?;
            session.admit(
                crate::exact_cache::Kind::Root,
                proposal,
                proof,
                None,
                receipt.judgment.clone(),
            );
            Ok(receipt)
        }
    }
}

/// Restrict permanent storage and reserve the shared original graph once for the entire attempt.
fn run(
    cas: &mut Cas,
    request: &Request,
    certificate: Option<&Certificate>,
    indexed: bool,
    additional_output: u64,
    budget: &mut Budget,
) -> Result<(Receipt, Arc<[u8]>), Fault> {
    let previous = budget.restrict_storage(&cas.limits);
    let original_graph = cas
        .counts()
        .payload_bytes
        .saturating_sub(cas.retained_judgment_bytes());
    let external = certificate.map_or(0, Certificate::retained_bytes);
    let result =
        budget.with_retained_payload(original_graph.saturating_add(external), AT, |budget| {
            cas.transaction(budget, |cas, budget| {
                capacity(request, budget)?;
                let old = cas.judgment_reservation(cas.retained_judgment_bytes(), budget, AT)?;
                let (candidate, wire) = budget.with_retained_payload(old, AT, |budget| {
                    prepare(cas, request, certificate, budget)
                })?;
                let live = request_bytes(request)
                    .saturating_add(wire.capacity() as u64)
                    .saturating_add(size_of::<Vec<u8>>() as u64)
                    .saturating_add(size_of::<Receipt>() as u64);
                budget.with_retained_payload(live, AT, |budget| {
                    budget.with_work_phase("lc-root", "publication", |budget| {
                        cas.retain_lc_root(candidate, &wire, indexed, additional_output, budget)
                            .map(|(judgment, proof)| {
                                (
                                    Receipt {
                                        judgment,
                                        owner: Arc::clone(&cas.memo_owner),
                                    },
                                    proof,
                                )
                            })
                    })
                })
            })
        });
    budget.restore_limits(previous);
    result
}

/// Produce or decode an untrusted candidate, then invoke the same independent checker.
fn prepare(
    cas: &Cas,
    request: &Request,
    certificate: Option<&Certificate>,
    budget: &mut Budget,
) -> Result<(Candidate, Vec<u8>), Fault> {
    // Reauthenticate before serializing a potentially foreign original inventory.
    budget.with_work_phase("lc-root", "originals", |budget| {
        super::claim::validate(cas, request, budget, &mut Events::default())
    })?;
    let scope = budget.with_retained_payload(request_bytes(request), AT, |budget| {
        budget.with_work_phase("lc-root", "scope", |budget| {
            super::wire::scope(cas, request, budget)
        })
    })?;
    budget.with_retained_payload(
        scope.capacity() as u64 + size_of::<Vec<u8>>() as u64,
        AT,
        |budget| {
            let candidate = budget.with_work_phase("lc-root", "candidate", |budget| {
                if let Some(certificate) = certificate {
                    budget.with_retained_payload(
                        request_bytes(request).saturating_add(super::bytes(request)),
                        AT,
                        |budget| super::wire::decode(request, &scope, &certificate.0, budget),
                    )
                } else {
                    super::claim::construct(cas, request, budget, &mut Events::default())
                }
            })?;
            let wire = budget.with_retained_payload(
                request_bytes(request).saturating_add(candidate_bytes(&candidate)),
                AT,
                |budget| {
                    budget.with_work_phase("lc-root", "encoding", |budget| {
                        super::wire::encode(&candidate, &scope, budget)
                    })
                },
            )?;
            if let Some(certificate) = certificate {
                budget.work(wire.len() as u64, AT)?;
                if wire != certificate.0 {
                    return Err(invalid());
                }
            }
            budget.with_retained_payload(
                wire.capacity() as u64 + size_of::<Vec<u8>>() as u64,
                AT,
                |budget| {
                    budget.with_work_phase("lc-root", "checking", |budget| {
                        super::check::verify(
                            cas,
                            request,
                            &candidate,
                            budget,
                            &mut Events::default(),
                        )
                        .map(|_| ())
                    })
                },
            )?;
            Ok((candidate, wire))
        },
    )
}

/// Check all copy bounds before reserving the caller-visible untrusted buffer.
fn copy(bytes: &[u8], budget: &mut Budget) -> Result<Vec<u8>, Fault> {
    let length = bytes.len() as u64;
    budget.check(BudgetKind::WitnessBytes, length, AT)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        length.saturating_add(size_of::<Certificate>() as u64),
        AT,
    )?;
    budget.work(length, AT)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(bytes.len())
        .map_err(|_| Fault::allocation(AT))?;
    result.extend_from_slice(bytes);
    Ok(result)
}

/// Owning-layer move witness, extending the existing registered replay control.
#[cfg(test)]
pub(super) fn certificate_allocation_control(value: Certificate) {
    let pointer = value.0.as_ptr();
    let moved = Box::new(value);
    assert_eq!(pointer, moved.0.as_ptr());
    let same_buffer = moved.shares_allocation(&moved);
    assert!(same_buffer);
    let copy = (*moved).clone();
    assert_eq!(*moved, copy);
    let distinct_copy = !moved.shares_allocation(&copy);
    assert!(distinct_copy);
    let empty = Certificate(Vec::new());
    let empty_copy = Certificate(Vec::new());
    let no_buffer = !empty.shares_allocation(&empty) && !empty.shares_allocation(&empty_copy);
    assert!(no_buffer);
}
