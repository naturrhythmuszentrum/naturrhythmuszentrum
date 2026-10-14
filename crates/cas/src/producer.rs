// SPDX-License-Identifier: AGPL-3.0-only
//! Untrusted finite proof plans shared by mathematical producers.

use std::collections::{BTreeMap, BTreeSet};

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, LogicalId, size};
use crate::model::{Operator, Sort};
use crate::number::Rational;
use crate::proof_model::{ProofStep, Relation, VerifiedWitness};
use crate::rule::Primitive;

/// Producer bookkeeping location.
const AT: Location = Location::new("proof generation");

/// Ordered proposals, carrying no trusted mathematical status.
#[derive(Clone, Debug, Default)]
pub(crate) struct Plan {
    /// Child-before-parent untrusted steps.
    pub(crate) steps: Vec<ProofStep>,
}

impl Plan {
    /// Bound and append a proposal before any checker can accept it.
    pub(crate) fn push(&mut self, step: ProofStep, budget: &mut Budget) -> Result<u32, Fault> {
        let count = size(self.steps.len(), AT)?
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::ProofNodes, AT))?;
        budget.check(BudgetKind::ProofNodes, count, AT)?;
        let bytes = count
            .checked_mul(1024)
            .and_then(|n| n.checked_add(size(step.premises.len(), AT).ok()?.checked_mul(8)?))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        budget.work(bytes, AT)?;
        if let Some(index) = self.steps.iter().position(|previous| *previous == step) {
            return u32::try_from(index).map_err(|_| Fault::capacity(BudgetKind::ProofNodes, AT));
        }
        let index = u32::try_from(self.steps.len())
            .map_err(|_| Fault::capacity(BudgetKind::ProofNodes, AT))?;
        self.steps
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        self.steps.push(step);
        Ok(index)
    }

    /// Select exactly a final proof's dependency closure and remap all premise indices.
    pub(crate) fn selected(&self, root: u32, budget: &mut Budget) -> Result<Self, Fault> {
        let mut reached = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(index) = pending.pop() {
            budget.work(size(self.steps.len(), AT)?, AT)?;
            if !reached.insert(index) {
                continue;
            }
            let step = self.get(index)?;
            pending
                .try_reserve(step.premises.len())
                .map_err(|_| Fault::allocation(AT))?;
            for premise in &step.premises {
                if *premise >= index {
                    return Err(Fault::invalid(InvalidCode::Reference, AT));
                }
                pending.push(*premise);
            }
        }
        let mut result = Self::default();
        let mut positions = BTreeMap::new();
        for index in reached {
            let mut step = self.get(index)?.clone();
            for premise in &mut step.premises {
                *premise = *positions
                    .get(premise)
                    .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))?;
            }
            let position = result.push(step, budget)?;
            positions.insert(index, position);
        }
        Ok(result)
    }

    /// Read a checked finite local reference.
    pub(crate) fn get(&self, index: u32) -> Result<&ProofStep, Fault> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.steps.get(index))
            .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))
    }

    /// Independently reconstruct every proposal and seal only complete canonical evidence.
    pub(crate) fn check(
        &self,
        cas: &mut Cas,
        context: LogicalId,
        check_budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        let checked = (|| {
            let compiled = crate::proof::compile(cas, context, &self.steps, check_budget)?;
            let bytes = crate::wire::encode(cas, &compiled, check_budget)?;
            cas.finish_proof(&compiled, bytes, check_budget)
        })();
        checked.map_err(replay_fault)
    }

    /// A returned best result additionally survives fresh-store canonical byte replay.
    pub(crate) fn verify(
        &self,
        cas: &mut Cas,
        context: LogicalId,
        checker: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        let witness = self.check(cas, context, checker)?;
        cas.recheck_witness(&witness, checker)
            .map_err(replay_fault)?;
        Ok(witness)
    }
}

/// Make an untrusted inference with closed endpoints and ordered earlier premises.
pub(crate) fn step(
    cas: &Cas,
    primitive: Primitive,
    relation: Relation,
    left: LogicalId,
    right: LogicalId,
    premises: Vec<u32>,
) -> ProofStep {
    ProofStep {
        primitive,
        relation,
        left: cas.handle(left),
        right: cas.handle(right),
        premises,
        path: Vec::new(),
        substitutions: Vec::new(),
    }
}

/// Retain a closed scalar expression using the source term's exact context and sort.
pub(crate) fn term(
    cas: &mut Cas,
    model: LogicalId,
    operator: Operator,
    operands: &[LogicalId],
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let source = cas.state.expression(model)?;
    if !source.scope.is_empty() {
        return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
    }
    let context = source.context;
    let sort: Sort = source.sort.clone();
    cas.generated(context, operator, sort, operands, budget)
}

/// Construct an exact small integer in the same scalar context.
pub(crate) fn integer(
    cas: &mut Cas,
    model: LogicalId,
    value: i64,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let rational = Rational::from_ratio(value, 1, budget)?;
    term(cas, model, Operator::Rational(rational), &[], budget)
}

/// The service checker has an independent work ledger, reported under its own resource name.
pub(crate) fn replay_fault(mut fault: Fault) -> Fault {
    if let crate::error::FaultKind::Exhausted { budget: kind, .. } = &mut fault.kind
        && *kind == BudgetKind::Work
    {
        *kind = BudgetKind::ReplayWork;
    }
    fault
}
