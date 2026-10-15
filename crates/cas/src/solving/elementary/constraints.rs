// SPDX-License-Identifier: AGPL-3.0-only
//! Finite polynomial consequences on the original strict solution region.

use super::evaluate::Evaluation;
use super::proof::{AT, invalid};
use super::value::Value;
use crate::budget::Budget;
use crate::domain_model::RegionExpr;
use crate::error::Fault;
use crate::graph::{Handle, LogicalId};
use crate::model::PredicateKind;
use crate::solving::SolveProblemData;
use crate::solving::coefficients::forms::Form;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{reserve, vector};

/// A conjunction of equations and strict nonvanishing conditions; it carries no theorem authority.
pub(crate) struct Constraints {
    /// Polynomial equations on the source solution region.
    pub(crate) zeros: Vec<Form>,
    /// All original and arithmetic denominator exclusions.
    pub(crate) nonzeros: Vec<Value>,
}

impl Constraints {
    /// Empty conjunction describes the whole product.
    pub(crate) const fn new() -> Self {
        Self {
            zeros: Vec::new(),
            nonzeros: Vec::new(),
        }
    }

    /// Retain the uncancelled domain of an evaluated expression.
    pub(crate) fn defined(
        &mut self,
        value: &Value,
        evaluation: &Evaluation<'_>,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let denominator = value
            .denominator
            .copy(&evaluation.engine, budget)
            .map_err(|e| fault(&e))?;
        reserve(&mut self.nonzeros, 1, budget)?;
        self.nonzeros
            .push(Value::polynomial(denominator, &evaluation.engine, budget)?);
        Ok(())
    }

    /// Extract every conjunct, including each expression's strict denominator domain.
    fn predicate(
        &mut self,
        evaluation: &Evaluation<'_>,
        root: LogicalId,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let mut pending = vector(1, budget)?;
        pending.push(root);
        while let Some(id) = pending.pop() {
            budget.work(1, AT)?;
            let (kind, arguments) = super::evidence::predicate(evaluation.cas, id)?;
            match (kind, arguments) {
                (PredicateKind::And, _) => {
                    reserve(&mut pending, arguments.len(), budget)?;
                    pending.extend(arguments);
                }
                (PredicateKind::Equal, [left, right]) => {
                    let value = evaluation.expression(*left, false, budget)?.sub(
                        &evaluation.expression(*right, false, budget)?,
                        &evaluation.engine,
                        budget,
                    )?;
                    self.defined(&value, evaluation, budget)?;
                    reserve(&mut self.zeros, 1, budget)?;
                    self.zeros.push(value.numerator);
                }
                (PredicateKind::NonZero, [argument]) => {
                    let value = evaluation.expression(*argument, false, budget)?;
                    self.defined(&value, evaluation, budget)?;
                    reserve(&mut self.nonzeros, 1, budget)?;
                    self.nonzeros.push(value);
                }
                (PredicateKind::Defined, _) => {
                    for argument in arguments {
                        self.defined(
                            &evaluation.expression(*argument, false, budget)?,
                            evaluation,
                            budget,
                        )?;
                    }
                }
                _ => return Err(invalid()),
            }
        }
        Ok(())
    }

    /// Read the exact guarded product, without widening an excluded region.
    pub(crate) fn domain(
        &mut self,
        evaluation: &Evaluation<'_>,
        root: LogicalId,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let mut pending = vector(1, budget)?;
        pending.push(root);
        while let Some(id) = pending.pop() {
            budget.work(1, AT)?;
            match &evaluation.cas.state.region(id)?.expression {
                RegionExpr::Ambient => {}
                RegionExpr::GuardTrue { base, predicate } => {
                    self.predicate(evaluation, *predicate, budget)?;
                    reserve(&mut pending, 1, budget)?;
                    pending.push(*base);
                }
                RegionExpr::Meet(members) => {
                    reserve(&mut pending, members.len(), budget)?;
                    pending.extend(members);
                }
                _ => return Err(invalid()),
            }
        }
        Ok(())
    }

    /// Collect the whole original relation, including denominators even if its formula was cancelled.
    pub(crate) fn problem(
        evaluation: &Evaluation<'_>,
        problem: &SolveProblemData<Handle>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let mut result = Self::new();
        result.predicate(evaluation, problem.predicate.logical_id(), budget)?;
        result.domain(evaluation, problem.domain.logical_id(), budget)?;
        for denominator in &problem.denominators {
            let value = evaluation.expression(denominator.logical_id(), false, budget)?;
            result.defined(&value, evaluation, budget)?;
            reserve(&mut result.nonzeros, 1, budget)?;
            result.nonzeros.push(value);
        }
        Ok(result)
    }

    /// Prove a polynomial consequence using only descending exact pseudo-reduction.
    pub(crate) fn zero(
        &self,
        evaluation: &Evaluation<'_>,
        value: &Value,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let mut remainder = value
            .numerator
            .copy(&evaluation.engine, budget)
            .map_err(|e| fault(&e))?;
        while !remainder.terms.is_empty() {
            let mut changed = false;
            for relation in &self.zeros {
                if let Some(reduced) =
                    super::reduction::reduce(&remainder, relation, &evaluation.engine, budget)?
                {
                    remainder = reduced;
                    changed = true;
                    break;
                }
            }
            if !changed {
                return evaluation.zero(
                    &Value::polynomial(remainder, &evaluation.engine, budget)?,
                    budget,
                );
            }
        }
        Ok(())
    }

    /// Compare an exclusion with an original exclusion or an independent coefficient-region fact.
    fn nonzero(
        &self,
        evaluation: &Evaluation<'_>,
        value: &Value,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        for original in &self.nonzeros {
            if value.equals(original, &evaluation.engine, budget)?
                || value.equals(
                    &original.neg(&evaluation.engine, budget)?,
                    &evaluation.engine,
                    budget,
                )?
            {
                return Ok(());
            }
        }
        evaluation.nonzero(value, budget)
    }

    /// Both equations and domain exclusions must follow, so a numerator-only rewrite cannot pass.
    pub(crate) fn implies(
        &self,
        evaluation: &Evaluation<'_>,
        target: &Self,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        for equation in &target.zeros {
            let value = Value::polynomial(
                equation
                    .copy(&evaluation.engine, budget)
                    .map_err(|e| fault(&e))?,
                &evaluation.engine,
                budget,
            )?;
            self.zero(evaluation, &value, budget)?;
        }
        for exclusion in &target.nonzeros {
            self.nonzero(evaluation, exclusion, budget)?;
        }
        Ok(())
    }
}
