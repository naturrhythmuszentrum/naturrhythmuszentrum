// SPDX-License-Identifier: AGPL-3.0-only
//! One unsealed Real coefficient stratum, with its exact map and original source.

use super::construction::Frame;
use crate::budget::Budget;
use crate::graph::{Cas, Handle};
use crate::solving::RealSignConclusion;
use crate::solving::coefficients::model::{CoefficientFailure, Element};
use crate::solving::towers::dense::Dense;

/// A total ancestor guard, retained solely as a proposal recipe for local hypotheses.
pub(crate) struct Guard {
    /// Canonical total parameter polynomial in the original algebra.
    pub(crate) value: Element,
    /// Actual expression used by the region constructor.
    pub(crate) expression: Handle,
    /// The child alternative chosen by the untrusted branch traversal.
    pub(crate) sign: RealSignConclusion,
    /// Derived facts name an earlier same-context sign; ancestor guards use their region.
    pub(crate) proof: Option<u32>,
}

/// Every sign recipe carries its precise earlier application slot.
pub(crate) struct Signed {
    /// Actual Real scalar subject.
    pub(crate) subject: Handle,
    /// Proposed sign, independently checked during replay.
    pub(crate) sign: RealSignConclusion,
    /// Same-context premise slot returned by the shared construction scheduler.
    pub(crate) slot: u32,
}

/// Leaf construction uses the shared exact arithmetic and one request budget.
pub(crate) struct Leaf<'a> {
    /// Transaction candidate.
    pub(crate) cas: &'a mut Cas,
    /// Actual Real graph construction and common proof DAG.
    pub(crate) frame: &'a mut Frame,
    /// Complete original problem, including every exclusion.
    pub(crate) problem: &'a Handle,
    /// Earlier total guards on this branch path.
    pub(crate) guards: &'a [Guard],
    /// Same-region canonical coefficient map.
    pub(crate) map: Handle,
    /// Ordered-field consequences available to finite factor sign search.
    pub(crate) extra: Vec<Guard>,
    /// Earlier one-unit application fixes the arithmetic domain of exact identities.
    pub(crate) unit: u32,
    /// Diagnostic phases never supply mathematical premises.
    pub(crate) audit: &'a mut crate::solving::radicals::route::Audit,
    /// Shared producer ledger.
    pub(crate) budget: &'a mut Budget,
}

impl<'a> Leaf<'a> {
    /// No new arithmetic interpretation is introduced by changing coefficient stratum.
    pub(crate) fn new(
        cas: &'a mut Cas,
        frame: &'a mut Frame,
        problem: &'a Handle,
        original: &'a Dense,
        guards: &'a [Guard],
        audit: &'a mut crate::solving::radicals::route::Audit,
        budget: &'a mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        let images = super::render::images(cas, frame, &original.domain, budget)?;
        let map = frame.mapping(cas, &original.domain, &images, budget)?;
        let unit = frame.unit(cas, &original.domain, budget)?;
        Ok(Self {
            cas,
            frame,
            problem,
            guards,
            map,
            extra: Vec::new(),
            unit,
            audit,
            budget,
        })
    }
}

impl Leaf<'_> {
    /// Record the next operation and cumulative ledger without changing a proof obligation.
    pub(crate) fn mark(&mut self, phase: &'static str) -> Result<(), CoefficientFailure> {
        Ok(self.audit.mark(phase, &self.frame.region, 0, self.budget)?)
    }
}
