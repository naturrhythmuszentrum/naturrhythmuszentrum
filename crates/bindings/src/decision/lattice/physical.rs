// SPDX-License-Identifier: AGPL-3.0-only
//! Physical output arithmetic cites checked question constants and retains shared displacements.

use super::{DecisionError, LatticeAxis, LatticePreparationUsage};
use crate::{DecisionCoordinate, decision::storage};
use nrz_cas::{ExactConstant, Handle, LogicalId, Rational, limits::Budget};
use std::{collections::BTreeMap, rc::Rc};

/// Checked products and arithmetic judgments owned once by the selected physical tuple.
#[derive(Debug, Default)]
pub(super) struct PhysicalPreparations {
    /// Original physical source identity and exact Integer identity within this selected question.
    products: BTreeMap<(usize, LogicalId), Rc<ExactConstant>>,
    /// Every newly retained arithmetic judgment; output vectors cite these shared owners.
    owned: Vec<Rc<ExactConstant>>,
    /// Unique exact ownership and lookup counters.
    usage: LatticePreparationUsage,
}

/// Physical displacement and representative, sharing checked arithmetic evidence.
pub(super) struct Row {
    /// Exact coherent B*k contribution, a physical difference.
    pub(super) displacement: Rc<ExactConstant>,
    /// Original physical target minus that contribution, preserving its original role.
    pub(super) representative: Rc<ExactConstant>,
}

impl PhysicalPreparations {
    /// Unique newly retained judgments, excluding the question's checked sources.
    pub(super) const fn usage(&self) -> LatticePreparationUsage {
        self.usage
    }

    /// Construct one physical row using the checked question's exact constants.
    pub(super) fn row(
        &mut self,
        axis: &LatticeAxis,
        indices: &[Rational],
        identities: &[Handle],
        budget: &mut Budget,
    ) -> Result<Row, DecisionError> {
        let mut displacement: Option<Rc<ExactConstant>> = None;
        for ((source, index), identity) in axis.basis.iter().zip(indices).zip(identities) {
            let value = self.product(source, index, identity, budget)?;
            budget.work(1, storage::AT)?;
            if value.is_zero() {
                continue;
            }
            displacement = Some(if let Some(previous) = displacement {
                let sum =
                    budget.with_retained_payload(self.usage.bytes, storage::AT, |budget| {
                        previous.add(&value, budget)
                    })?;
                self.retain(sum, true, budget)?
            } else {
                value
            });
        }
        let displacement = if let Some(value) = displacement {
            value
        } else {
            self.retain(ExactConstant::zero(), false, budget)?
        };
        budget.work(1, storage::AT)?;
        let representative = if displacement.is_zero() {
            self.retain(axis.target.exact().clone(), false, budget)?
        } else {
            let value = budget.with_retained_payload(self.usage.bytes, storage::AT, |budget| {
                axis.target.exact().sub(&displacement, budget)
            })?;
            self.retain(value, true, budget)?
        };
        Ok(Row {
            displacement,
            representative,
        })
    }

    /// Reuse one exact source/index product; independent equal-valued source identities miss.
    fn product(
        &mut self,
        source: &DecisionCoordinate,
        index: &Rational,
        identity: &Handle,
        budget: &mut Budget,
    ) -> Result<Rc<ExactConstant>, DecisionError> {
        self.usage.requests = self.usage.requests.saturating_add(1);
        let key = (
            std::ptr::from_ref(source.exact()).addr(),
            identity.logical_id(),
        );
        if let Some(value) = self.products.get(&key) {
            budget.work(1, storage::AT)?;
            self.usage.hits = self.usage.hits.saturating_add(1);
            return Ok(Rc::clone(value));
        }
        budget.work(2, storage::AT)?;
        let value = if index.is_zero() {
            self.retain(ExactConstant::zero(), false, budget)?
        } else if source.exact().is_zero() || index == &Rational::one() {
            self.retain(source.exact().clone(), false, budget)?
        } else {
            let factor = ExactConstant::from(index.clone());
            let value = budget.with_retained_payload(self.usage.bytes, storage::AT, |budget| {
                source.exact().mul(&factor, budget)
            })?;
            self.retain(value, true, budget)?
        };
        self.products.insert(key, Rc::clone(&value));
        Ok(value)
    }

    /// Own a new arithmetic result or cite an already checked source graph without copying it.
    fn retain(
        &mut self,
        value: ExactConstant,
        fresh: bool,
        budget: &Budget,
    ) -> Result<Rc<ExactConstant>, DecisionError> {
        self.usage.bytes = self.usage.bytes.saturating_add(128);
        if fresh || value.as_rational().is_some() {
            self.usage.bytes = self
                .usage
                .bytes
                .saturating_add(storage::constant_bytes(&value));
        }
        if fresh && let Some((graph, _)) = value.graph() {
            self.usage.graphs = self.usage.graphs.saturating_add(1);
            let counts = graph.counts();
            self.usage.records = self.usage.records.saturating_add(
                u64::try_from(counts.vertices.saturating_add(counts.edges))
                    .map_err(|_| DecisionError::Capacity)?,
            );
        }
        storage::retained(self.usage.bytes, budget)?;
        self.owned
            .try_reserve(1)
            .map_err(|_| DecisionError::Capacity)?;
        let value = Rc::new(value);
        self.owned.push(Rc::clone(&value));
        Ok(value)
    }
}
