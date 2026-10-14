// SPDX-License-Identifier: AGPL-3.0-only
//! Versioned, complete positive cost tables, pinned as immutable graph records.

use std::fmt;

use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, LogicalId, State, Vertex, size};
use crate::rule::{PRIMITIVES, Primitive};

/// Positive logical weights; costs do not assert mathematical truth or runtime.
#[derive(Clone, Eq, PartialEq)]
pub struct CostPolicy {
    /// Stable caller-chosen policy identity.
    pub key: String,
    /// Positive version; different contents require a different identity or version.
    pub version: u32,
    /// One positive weight for every primitive, in the published manifest order.
    pub weights: Vec<u64>,
}

impl fmt::Debug for CostPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CostPolicy")
            .field("key_bytes", &self.key.len())
            .field("version", &self.version)
            .field("weights", &self.weights.len())
            .finish_non_exhaustive()
    }
}

impl Default for CostPolicy {
    fn default() -> Self {
        Self {
            key: "logical".into(),
            version: 3,
            weights: PRIMITIVES
                .iter()
                .map(|primitive| primitive.specification().weight)
                .collect(),
        }
    }
}

/// Cost-table validation location.
const AT: Location = Location::new("cost policy");

impl CostPolicy {
    /// Check the complete fixed registry and its positive finite transition weights.
    pub(crate) fn validate(&self, budget: &mut Budget) -> Result<(), Fault> {
        budget.check(BudgetKind::NameBytes, size(self.key.len(), AT)?, AT)?;
        budget.work(
            size(self.key.len(), AT)?
                .checked_add(size(self.weights.len(), AT)?)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        if self.key.is_empty()
            || self.version == 0
            || self.weights.len() != PRIMITIVES.len()
            || self
                .weights
                .iter()
                .any(|weight| *weight == 0 || *weight > 1_000_000)
        {
            return Err(Fault::invalid(InvalidCode::Policy, AT));
        }
        if self.key == "logical"
            && (self.version < 3 || (self.version == 3 && *self != Self::default()))
        {
            return Err(Fault::invalid(InvalidCode::Policy, AT));
        }
        Ok(())
    }

    /// Read the weight from a previously validated table.
    pub(crate) fn weight(&self, primitive: Primitive) -> Result<u64, Fault> {
        PRIMITIVES
            .iter()
            .position(|candidate| *candidate == primitive)
            .and_then(|index| self.weights.get(index))
            .copied()
            .ok_or_else(|| Fault::invalid(InvalidCode::Policy, AT))
    }

    /// Stable bytes include the entire pinned rule manifest, not just its current length.
    pub(crate) fn encode(&self, out: &mut Encoder<'_>) -> Result<(), Fault> {
        out.name(&self.key)?;
        crate::wire::index(self.version, out)?;
        out.count(self.weights.len())?;
        for (primitive, weight) in PRIMITIVES.iter().zip(&self.weights) {
            let rule = primitive.specification();
            out.name(rule.key)?;
            crate::wire::index(rule.version, out)?;
            out.unsigned(*weight)?;
        }
        Ok(())
    }
}

impl Cas {
    /// Pin a table in a private request transaction; conflicting identity reuse is refused.
    pub(crate) fn pin_policy(
        &mut self,
        policy: &CostPolicy,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        policy.validate(budget)?;
        for id in self.state.indices.keys() {
            budget.work(
                size(policy.key.len(), AT)?
                    .checked_add(256)
                    .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
                AT,
            )?;
            if let Vertex::Policy(previous) = self.state.node(*id)?
                && previous.key == policy.key
                && previous.version == policy.version
            {
                if *previous == *policy {
                    return Ok(*id);
                }
                return Err(Fault::invalid(InvalidCode::Policy, AT));
            }
        }
        let mut state = std::mem::replace(&mut self.state, State::new());
        let result = self.insert(&mut state, Vertex::Policy(policy.clone()), &[], 0, budget);
        self.state = state;
        result
    }
}
