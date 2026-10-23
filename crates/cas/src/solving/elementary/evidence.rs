// SPDX-License-Identifier: AGPL-3.0-only
//! Regional facts are compared after exact arithmetic, without substituting their hypotheses.

use super::evaluate::Evaluation;
use super::proof::{AT, invalid};
use super::value::Value;
use crate::budget::Budget;
use crate::domain_model::RegionExpr;
use crate::error::Fault;
use crate::graph::{LogicalId, Vertex};
use crate::model::{Operator, PredicateKind};
use crate::proof_model::{DomainContract, Statement};
use crate::solving::coefficients::forms::Form;
use crate::solving::coefficients::model::Value as Coefficient;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{reserve, vector};

/// Borrow condition syntax from either supported graph representation.
pub(crate) fn predicate(
    cas: &crate::graph::Cas,
    id: LogicalId,
) -> Result<(&PredicateKind, &[LogicalId]), Fault> {
    match cas.state.node(id)? {
        Vertex::Condition(value) => Ok((&value.kind, &value.arguments)),
        Vertex::Expression(value) => match &value.operator {
            Operator::Predicate(kind) => Ok((kind, &value.operands)),
            _ => Err(invalid()),
        },
        _ => Err(invalid()),
    }
}

/// Canonical constant embeddings preserve nonzero ground scalars without asserting quotient fields.
pub(crate) fn ground_unit(value: &Coefficient, budget: &mut Budget) -> Result<bool, Fault> {
    let mut current = value;
    loop {
        budget.work(1, AT)?;
        match current {
            Coefficient::Scalar(value) => return Ok(!value.zero(budget)?),
            Coefficient::Fraction(value) => {
                let (Some(numerator), Some(denominator)) = (
                    value.numerator.scalar(budget)?,
                    value.denominator.scalar(budget)?,
                ) else {
                    return Ok(false);
                };
                return Ok(!numerator.zero(budget)? && !denominator.zero(budget)?);
            }
            Coefficient::Tower(coefficients) => {
                let [constant] = coefficients.as_slice() else {
                    return Ok(false);
                };
                current = &constant.value;
            }
        }
    }
}

/// Formal parameter numerators and nonconstant quotient representatives remain undecided.
fn ground_nonzero(form: &Form, budget: &mut Budget) -> Result<bool, Fault> {
    let [term] = form.terms.as_slice() else {
        return Ok(false);
    };
    if !term.powers.is_empty() {
        return Ok(false);
    }
    ground_unit(&term.coefficient.value, budget)
}

impl Evaluation<'_> {
    /// Evidence lives in its own scope; tuple substitution must never rewrite a hypothesis.
    pub(crate) const fn unbound(&self) -> Evaluation<'_> {
        Evaluation {
            cas: self.cas,
            engine: crate::solving::coefficients::arithmetic::Engine {
                state: &self.cas.state,
                domain: self.engine.domain,
            },
            region: self.region,
            premises: self.premises,
            free: &[],
            bound: &[],
        }
    }

    /// Collect only explicit true guards and already checked predicate facts on the identical region.
    fn predicates(&self, budget: &mut Budget) -> Result<Vec<(LogicalId, bool)>, Fault> {
        let mut result = vector(self.premises.len(), budget)?;
        for premise in self.premises {
            budget.work(1, AT)?;
            if let Statement::PredicateFact { region, predicate } = &premise.judgment.statement
                && region == self.region
            {
                result.push((predicate.logical_id(), true));
            }
        }
        let mut pending = vector(1, budget)?;
        pending.push(self.region.logical_id());
        while let Some(id) = pending.pop() {
            budget.work(1, AT)?;
            match &self.cas.state.region(id)?.expression {
                RegionExpr::GuardTrue { base, predicate }
                | RegionExpr::GuardFalse { base, predicate } => {
                    let truth = matches!(
                        self.cas.state.region(id)?.expression,
                        RegionExpr::GuardTrue { .. }
                    );
                    reserve(&mut result, 1, budget)?;
                    result.push((*predicate, truth));
                    reserve(&mut pending, 1, budget)?;
                    pending.push(*base);
                }
                RegionExpr::Meet(members) => {
                    reserve(&mut pending, members.len(), budget)?;
                    pending.extend(members);
                }
                _ => {}
            }
        }
        Ok(result)
    }

    /// Exact nonvanishing requires ground arithmetic or a matching regional nonzero/sign fact.
    pub(crate) fn nonzero(&self, value: &Value, budget: &mut Budget) -> Result<(), Fault> {
        if ground_nonzero(&value.numerator, budget)? {
            return Ok(());
        }
        let original = self.unbound();
        for (id, truth) in self.predicates(budget)? {
            let (kind, arguments) = predicate(self.cas, id)?;
            let candidate = match (kind, arguments, truth) {
                (PredicateKind::NonZero | PredicateKind::Positive, [argument], true) => {
                    original.expression(*argument, false, budget)?
                }
                (PredicateKind::Equal, [left, right], false) => {
                    original.expression(*left, false, budget)?.sub(
                        &original.expression(*right, false, budget)?,
                        &self.engine,
                        budget,
                    )?
                }
                _ => continue,
            };
            if value.equals(&candidate, &self.engine, budget)?
                || value.equals(&candidate.neg(&self.engine, budget)?, &self.engine, budget)?
            {
                return Ok(());
            }
        }
        if let [term] = value.numerator.terms.as_slice()
            && term.powers.is_empty()
            && crate::solving::coefficients::transport::nonzero(
                &self.engine,
                &term.coefficient,
                self.region,
                self.premises,
                budget,
            )?
        {
            return Ok(());
        }
        Err(invalid())
    }

    /// Collect normalized zero relations from exact scoped equality evidence and coefficient guards.
    fn relations(&self, budget: &mut Budget) -> Result<Vec<Form>, Fault> {
        let mut result = vector(self.premises.len(), budget)?;
        let original = self.unbound();
        for premise in self.premises {
            budget.work(1, AT)?;
            let endpoints = match &premise.judgment.statement {
                Statement::Equality {
                    left,
                    right,
                    domain,
                    domain_contract: DomainContract::DefinedAndEqual,
                } if domain == self.region => Some((left, right)),
                _ => None,
            };
            if let Some((left, right)) = endpoints {
                let difference = original.expression(left.logical_id(), false, budget)?.sub(
                    &original.expression(right.logical_id(), false, budget)?,
                    &self.engine,
                    budget,
                )?;
                reserve(&mut result, 1, budget)?;
                result.push(difference.numerator);
            }
        }
        for (id, truth) in self.predicates(budget)? {
            if let (PredicateKind::Equal, [left, right], true) = {
                let (kind, args) = predicate(self.cas, id)?;
                (kind, args, truth)
            } {
                let difference = original.expression(*left, false, budget)?.sub(
                    &original.expression(*right, false, budget)?,
                    &self.engine,
                    budget,
                )?;
                reserve(&mut result, 1, budget)?;
                result.push(difference.numerator);
            }
        }
        Ok(result)
    }

    /// Finite ideal reduction proves only consequences of exact already defined equality relations.
    pub(crate) fn zero(&self, value: &Value, budget: &mut Budget) -> Result<(), Fault> {
        if value.numerator.terms.is_empty() {
            return Ok(());
        }
        let mut relations = self.relations(budget)?;
        let roots = self.root_relations(&value.numerator, budget)?;
        reserve(&mut relations, roots.len(), budget)?;
        relations.extend(roots);
        let mut remainder = value
            .numerator
            .copy(&self.engine, budget)
            .map_err(|e| fault(&e))?;
        while !remainder.terms.is_empty() {
            if self.parameter_zero(&remainder.terms[0].coefficient, budget)? {
                budget.work(crate::graph::size(remainder.terms.len(), AT)?, AT)?;
                remainder.terms.remove(0);
                continue;
            }
            let mut changed = false;
            for relation in &relations {
                if super::reduction::reduce(&mut remainder, relation, &self.engine, budget)?
                    .is_some()
                {
                    changed = true;
                    break;
                }
            }
            if !changed {
                return Err(invalid());
            }
        }
        Ok(())
    }

    /// Every original conjunct is evaluated; conjunction operands never become numerical samples.
    pub(crate) fn predicate(&self, root: LogicalId, budget: &mut Budget) -> Result<(), Fault> {
        let mut pending = vector(1, budget)?;
        pending.push(root);
        while let Some(id) = pending.pop() {
            budget.work(1, AT)?;
            let (kind, arguments) = predicate(self.cas, id)?;
            match (kind, arguments) {
                (PredicateKind::And, _) => {
                    reserve(&mut pending, arguments.len(), budget)?;
                    pending.extend(arguments);
                }
                (PredicateKind::Equal, [left, right]) => {
                    let difference = self.expression(*left, true, budget)?.sub(
                        &self.expression(*right, true, budget)?,
                        &self.engine,
                        budget,
                    )?;
                    self.zero(&difference, budget)?;
                }
                (PredicateKind::NonZero, [argument]) => {
                    self.nonzero(&self.expression(*argument, true, budget)?, budget)?;
                }
                (PredicateKind::Defined, _) => {
                    for argument in arguments {
                        self.expression(*argument, true, budget)?;
                    }
                }
                _ => return Err(invalid()),
            }
        }
        Ok(())
    }

    /// Region membership follows its exact constructor, after the same simultaneous substitution.
    pub(crate) fn domain(&self, root: LogicalId, budget: &mut Budget) -> Result<(), Fault> {
        let mut pending = vector(1, budget)?;
        pending.push(root);
        while let Some(id) = pending.pop() {
            budget.work(1, AT)?;
            match &self.cas.state.region(id)?.expression {
                RegionExpr::Ambient => {}
                RegionExpr::GuardTrue { base, predicate } => {
                    self.predicate(*predicate, budget)?;
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
}
