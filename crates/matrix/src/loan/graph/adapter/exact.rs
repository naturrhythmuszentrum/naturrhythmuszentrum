// SPDX-License-Identifier: AGPL-3.0-only
//! Allocation identities compose scoped allowances without reserving their payload twice.

use super::{
    native::Failure,
    node::Contract,
    node::Node,
    owner::{Borrowed, Owner},
};
use crate::loan::{
    charge, graph::walk::Error, lc_shape::output::bridge::retained_overlap::Observation,
    registry::AllocationKind,
};
use crate::{ExecutionAccount, Heap, LoanError};
use nrz_bindings::{AdmissionError, LcOmegaCorrespondence};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// A borrowed named graph exposes only its fixed original owner observations.
pub(super) trait Source: std::fmt::Debug {
    /// Four disjoint observation positions per declared node, including empty positions.
    fn slots(&self) -> Result<usize, LoanError>;
    /// No returned observation outlives its original native capability.
    fn observation(&self, position: usize) -> Result<Option<Observation<'_>>, Error>;
}

/// Different units remain separate; neither field is complete backend residence.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Report {
    /// Existing captured-token and correspondence logical allowances, deduplicated by identity.
    pub logical: u64,
    /// Actual replay buffers and disjoint outer Rc control blocks.
    pub observed: u64,
    /// Distinct covered allocations or allowance identities, never retaining-edge count.
    pub owners: usize,
}
impl Report {
    /// Admission can combine these justified disjoint scopes, without upgrading their coverage.
    pub(super) fn covered(self) -> Result<usize, LoanError> {
        usize::try_from(
            self.logical
                .checked_add(self.observed)
                .ok_or(LoanError::ByteOverflow)?,
        )
        .map_err(|_| LoanError::ByteOverflow)
    }
    /// The caller already proved that this identity has not appeared earlier.
    fn include(&mut self, value: &Observation<'_>) -> Result<(), Error> {
        self.logical = self
            .logical
            .checked_add(value.logical_allowance.unwrap_or(0))
            .ok_or(LoanError::ByteOverflow)?;
        self.observed = self
            .observed
            .checked_add(value.observed_residence.unwrap_or(0))
            .ok_or(LoanError::ByteOverflow)?;
        self.owners = self.owners.checked_add(1).ok_or(LoanError::CountOverflow)?;
        Ok(())
    }
}

/// Map the precise owning-layer refusal without replacing original Work exhaustion.
fn failure(error: AdmissionError) -> Error {
    Error::Adapter(Failure::Preparation(error.into()))
}

/// A native law has a captured child, correspondence child and disjoint outer control block.
fn law(value: &Rc<LcOmegaCorrespondence>, part: usize) -> Result<Option<Observation<'_>>, Error> {
    match part {
        0 => Observation::captured(value).map(Some).map_err(failure),
        1 => Ok(Some(Observation::correspondence(value))),
        2 => Ok(Some(Observation::native(value))),
        _ => Ok(None),
    }
}

/// Proposal observations are borrowed before any graph storage is acquired.
#[derive(Debug)]
pub(super) struct Nodes<'a, 'i, P: Scalar, const N: usize>(pub(super) &'a [Node<'i, P, N>]);
impl<P: Scalar, const N: usize> Source for Nodes<'_, '_, P, N> {
    fn slots(&self) -> Result<usize, LoanError> {
        self.0.len().checked_mul(4).ok_or(LoanError::CountOverflow)
    }
    fn observation(&self, position: usize) -> Result<Option<Observation<'_>>, Error> {
        node_observation(self.0, position)
    }
}
impl<P: Scalar, const N: usize> Source for Owner<'_, P, N> {
    fn slots(&self) -> Result<usize, LoanError> {
        self.backing
            .nodes
            .len()
            .checked_mul(4)
            .ok_or(LoanError::CountOverflow)
    }
    fn observation(&self, position: usize) -> Result<Option<Observation<'_>>, Error> {
        node_observation(&self.backing.nodes, position)
    }
}
/// Each reference is tied to the actual original phase/contract record.
fn node_observation<'a, P: Scalar, const N: usize>(
    nodes: &'a [Node<'_, P, N>],
    position: usize,
) -> Result<Option<Observation<'a>>, Error> {
    let Some(node) = nodes.get(position / 4) else {
        return Ok(None);
    };
    if position % 4 == 3 {
        return Ok(match node.contract {
            Contract::Omega {
                certificate: Some(value),
                ..
            } => Some(Observation::certificate(value)),
            _ => None,
        });
    }
    node.native
        .as_ref()
        .and_then(super::native::Native::retained_correspondence)
        .map_or(Ok(None), |value| law(value, position % 4))
}

/// An exclusive caller owns the snapshot; weak published edges are borrowed only during visits.
pub(super) struct Scope<'a> {
    /// Current owner, when it is already admitted.
    pub(super) current: Option<&'a dyn Source>,
    /// Explicit supplied owners retain their original borrowing lifetime.
    pub(super) prior: &'a [Borrowed<'a>],
    /// Includes published bundles retained by outside native readers after graph retirement.
    pub(super) heap: &'a Heap,
}
impl Scope<'_> {
    /// Fixed declared positions and fixed native debit slots bound the entire scan.
    fn slots(&self) -> Result<usize, LoanError> {
        let mut total = self
            .heap
            .loans
            .debits
            .len()
            .checked_mul(4)
            .ok_or(LoanError::CountOverflow)?;
        if let Some(current) = self.current {
            total = total
                .checked_add(current.slots()?)
                .ok_or(LoanError::CountOverflow)?;
        }
        for owner in self.prior {
            total = total
                .checked_add(owner.exact.slots()?)
                .ok_or(LoanError::CountOverflow)?;
        }
        Ok(total)
    }

    /// Every examined source header and native weak edge is funded before observation.
    fn visit(
        &self,
        mut position: usize,
        account: &mut nrz_bindings::Budget,
        inspect: &mut dyn FnMut(Observation<'_>, &mut nrz_bindings::Budget) -> Result<(), Error>,
    ) -> Result<(), Error> {
        for source in self
            .current
            .into_iter()
            .chain(self.prior.iter().map(|owner| owner.exact))
        {
            charge(account, 1)?; // A supplied original graph source.
            let slots = source.slots()?;
            if position < slots {
                charge(account, 1)?; // N fixed phase/contract record.
                return source
                    .observation(position)?
                    .map_or(Ok(()), |value| inspect(value, account));
            }
            position -= slots;
        }
        charge(account, 1)?; // I actual native debit position, including vacancies.
        let Some(Some(debit)) = self.heap.loans.debits.get(position / 4) else {
            return Ok(());
        };
        if let AllocationKind::LcBundle(weak) = &debit.kind {
            charge(account, 2)?; // P prepay this attempted temporary weak upgrade and its release.
            if let Some(bundle) = weak.upgrade()
                && let Some(value) = bundle.retained_correspondence()
                && let Some(value) = law(value, position % 4)?
            {
                inspect(value, account)?;
            }
        }
        Ok(())
    }

    /// Reporting observes covered units without changing owning-layer reservations.
    pub(super) fn inspect(&self, account: &mut ExecutionAccount) -> Result<Report, Error> {
        account
            .ledger
            .with_work_phase("matrix.graph", "exact owner inventory", |budget| {
                self.scan(budget, false).map(|(report, _)| report)
            })
    }

    /// Admission additionally asks each distinct owner for its existing persistent overlap.
    pub(super) fn reservation(
        &self,
        account: &mut ExecutionAccount,
    ) -> Result<(Report, u64), Error> {
        account
            .ledger
            .with_work_phase("matrix.graph", "exact owner reservation", |budget| {
                self.scan(budget, true)
            })
    }

    /// Stable first occurrence wins; meanings and carrier addresses never identify payloads.
    fn scan(
        &self,
        account: &mut nrz_bindings::Budget,
        reserve: bool,
    ) -> Result<(Report, u64), Error> {
        let _span = tracing::trace_span!("nrz.graph.exact_scope").entered();
        charge(account, 1 + self.prior.len())?; // H snapshot plus A supplied extents.
        let slots = self.slots()?;
        let mut report = Report::default();
        let mut overlap = 0_u64;
        for index in 0..slots {
            self.visit(index, account, &mut |value, account| {
                let mut repeated = false;
                for previous in 0..index {
                    self.visit(previous, account, &mut |other, account| {
                        charge(account, 1)?; // A allocation comparison.
                        repeated = value.same_allocation(&other, account).map_err(failure)?;
                        Ok(())
                    })?;
                    if repeated {
                        break;
                    }
                }
                if !repeated {
                    report.include(&value)?;
                    if reserve {
                        overlap = overlap
                            .checked_add(value.persistent_overlap(account).map_err(failure)?)
                            .ok_or(LoanError::ByteOverflow)?;
                    }
                }
                Ok(())
            })?;
        }
        Ok((report, overlap))
    }
}
