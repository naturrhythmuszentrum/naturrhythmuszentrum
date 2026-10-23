// SPDX-License-Identifier: AGPL-3.0-only
//! Finite component identities use exact regional premises and existing selection constraints.

use super::super::proof::{AT, invalid};
use super::super::value::Value;
use super::{Components, Parts};
use crate::budget::Budget;
use crate::domain_model::RegionExpr;
use crate::error::Fault;
use crate::graph::LogicalId;
use crate::model::PredicateKind;
use crate::proof_model::{DomainContract, Statement};
use crate::solving::SolveStatement;
use crate::solving::coefficients::forms::Form;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{reserve, vector};

impl Components<'_, '_> {
    /// Import each explicitly referenced exact regional fact once for the whole component interpretation.
    pub(crate) fn premises(&mut self, slots: &[u32], budget: &mut Budget) -> Result<(), Fault> {
        for slot in slots {
            budget.work(crate::graph::size(self.imported.len(), AT)?.max(1), AT)?;
            if self.imported.contains(slot) {
                continue;
            }
            let premise = super::super::proof::premise(self.evaluation.premises, *slot)?;
            reserve(&mut self.imported, 1, budget)?;
            self.imported.push(*slot);
            match &premise.judgment.statement {
                Statement::Solve(statement) => match statement.as_ref() {
                    SolveStatement::RealValues(fact) if &fact.region == self.evaluation.region => {
                        for handle in &fact.values {
                            let parts = self.expression(handle.logical_id(), &[], budget)?;
                            reserve(&mut self.equations, 1, budget)?;
                            self.equations.push(parts.imaginary.numerator);
                        }
                    }
                    SolveStatement::CoefficientEquality(fact)
                        if &fact.region == self.evaluation.region
                            && fact.left.logical_id().kind
                                == crate::graph::VertexKind::Expression
                            && fact.right.logical_id().kind
                                == crate::graph::VertexKind::Expression =>
                    {
                        self.endpoints(
                            fact.left.logical_id(),
                            fact.right.logical_id(),
                            &[],
                            budget,
                        )?;
                    }
                    _ => {}
                },
                Statement::Equality {
                    left,
                    right,
                    domain,
                    domain_contract: DomainContract::DefinedAndEqual,
                } if domain == self.evaluation.region => {
                    self.endpoints(left.logical_id(), right.logical_id(), &[], budget)?;
                }
                Statement::PredicateFact { predicate, region }
                    if region == self.evaluation.region =>
                {
                    self.record_predicate(predicate.logical_id(), &[], budget)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// True guards retain their exact region ancestry; unrelated strata contribute no facts.
    pub(crate) fn guards(&mut self, budget: &mut Budget) -> Result<(), Fault> {
        let mut pending = vector(1, budget)?;
        pending.push(self.evaluation.region.logical_id());
        while let Some(id) = pending.pop() {
            budget.work(1, AT)?;
            match &self.evaluation.cas.state.region(id)?.expression {
                RegionExpr::GuardTrue { base, predicate } => {
                    self.record_predicate(*predicate, &[], budget)?;
                    reserve(&mut pending, 1, budget)?;
                    pending.push(*base);
                }
                RegionExpr::GuardFalse { base, .. } => {
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
        Ok(())
    }

    /// An equality contributes both real and imaginary equations without any orientation guess.
    fn endpoints(
        &mut self,
        left: LogicalId,
        right: LogicalId,
        bound: &[Parts],
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let original = self.expression(left, bound, budget)?;
        let expected = self.expression(right, bound, budget)?;
        self.equation(
            &original.sum(&expected, true, self.evaluation, budget)?,
            budget,
        )
    }

    /// Clearing already checked denominators preserves zero equations.
    pub(crate) fn equation(&mut self, value: &Parts, budget: &mut Budget) -> Result<(), Fault> {
        reserve(&mut self.equations, 2, budget)?;
        let real = self.reduced(&value.real.numerator, budget)?;
        self.equations.push(self.monic(&real, budget)?);
        let imaginary = self.reduced(&value.imaginary.numerator, budget)?;
        self.equations.push(self.monic(&imaginary, budget)?);
        Ok(())
    }

    /// Normalize proved zero equations by a nonzero rational scalar to bound repeated reduction sizes.
    fn monic(&self, form: &Form, budget: &mut Budget) -> Result<Form, Fault> {
        let engine = &self.evaluation.engine;
        let Some(leading) = form.terms.first() else {
            return form.copy(engine, budget).map_err(|error| fault(&error));
        };
        let Some(coefficient) = super::selection::scalar(&leading.coefficient, budget)? else {
            return form.copy(engine, budget).map_err(|error| fault(&error));
        };
        let one = crate::number::Rational::from_ratio(1, 1, budget)?;
        let inverse = one.div(&coefficient, budget)?;
        let value = engine
            .rational(&inverse, budget)
            .map_err(|error| fault(&error))?;
        let scalar = Form::constant(value, engine, budget).map_err(|error| fault(&error))?;
        form.mul(&scalar, engine, budget)
            .map_err(|error| fault(&error))
    }

    /// Existence of this exact selection makes its full predicate available for these same slots.
    pub(crate) fn record_predicate(
        &mut self,
        root: LogicalId,
        bound: &[Parts],
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let mut pending = vector(1, budget)?;
        pending.push(root);
        while let Some(id) = pending.pop() {
            budget.work(1, AT)?;
            let (kind, arguments) = super::super::evidence::predicate(self.evaluation.cas, id)?;
            match (kind, arguments) {
                (PredicateKind::And, _) => {
                    reserve(&mut pending, arguments.len(), budget)?;
                    pending.extend(arguments);
                }
                (PredicateKind::Equal, [left, right]) => {
                    self.endpoints(*left, *right, bound, budget)?;
                }
                (PredicateKind::Positive | PredicateKind::NonZero, [argument]) => {
                    let parts = self.expression(*argument, bound, budget)?;
                    let nonzero = if *kind == PredicateKind::Positive {
                        reserve(&mut self.positive, 1, budget)?;
                        self.positive
                            .push(parts.real.copy(&self.evaluation.engine, budget)?);
                        parts.real
                    } else {
                        parts
                            .real
                            .mul(&parts.real, &self.evaluation.engine, budget)?
                            .add(
                                &parts.imaginary.mul(
                                    &parts.imaginary,
                                    &self.evaluation.engine,
                                    budget,
                                )?,
                                &self.evaluation.engine,
                                budget,
                            )?
                    };
                    reserve(&mut self.nonzero, 1, budget)?;
                    self.nonzero.push(nonzero);
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Pseudo-reduction multiplies only by proved ground units, so preserves zero and nonzero.
    fn reduced(&self, form: &Form, budget: &mut Budget) -> Result<Form, Fault> {
        let engine = &self.evaluation.engine;
        let mut remainder = form.copy(engine, budget).map_err(|error| fault(&error))?;
        let mut result = Form {
            terms: vector(0, budget)?,
        };
        while !remainder.terms.is_empty() {
            let mut changed = false;
            for equation in &self.equations {
                if let Some(scale) =
                    super::super::reduction::reduce(&mut remainder, equation, engine, budget)?
                {
                    if scale == super::super::reduction::Scale::Leading {
                        let coefficient = engine
                            .copy(
                                &equation.terms.first().ok_or_else(invalid)?.coefficient,
                                budget,
                            )
                            .map_err(|error| fault(&error))?;
                        let scalar = Form::constant(coefficient, engine, budget)
                            .map_err(|error| fault(&error))?;
                        result = result
                            .mul(&scalar, engine, budget)
                            .map_err(|error| fault(&error))?;
                    }
                    changed = true;
                    break;
                }
            }
            if !changed {
                budget.work(crate::graph::size(remainder.terms.len(), AT)?, AT)?;
                let leading = remainder.terms.remove(0);
                let mut terms = vector(1, budget)?;
                terms.push(leading);
                result = result
                    .add(&Form { terms }, engine, budget)
                    .map_err(|error| fault(&error))?;
            }
        }
        Ok(result)
    }

    /// Zero is a finite consequence of component equations, never of coefficient spelling.
    pub(crate) fn zero(&self, value: &Value, budget: &mut Budget) -> Result<(), Fault> {
        if self.reduced(&value.numerator, budget)?.terms.is_empty() {
            Ok(())
        } else {
            Err(invalid())
        }
    }

    /// Nonzero squares and exact regional nonzero facts are independent of conjugate orientation.
    pub(crate) fn nonzero(&self, value: &Value, budget: &mut Budget) -> Result<(), Fault> {
        #[cfg(test)]
        let _attribution = crate::testing::work_trace::Scope::phase("exclusion.component-nonzero");
        let engine = &self.evaluation.engine;
        for candidate in [
            value.copy(engine, budget)?,
            value.mul(value, engine, budget)?,
        ] {
            let reduced =
                Value::polynomial(self.reduced(&candidate.numerator, budget)?, engine, budget)?;
            if super::super::product::decides(self.evaluation.nonzero(&reduced, budget))? {
                return Ok(());
            }
            for fact in &self.nonzero {
                if super::super::product::decides(
                    self.zero(&candidate.sub(fact, engine, budget)?, budget),
                )? || super::super::product::decides(
                    self.zero(&candidate.add(fact, engine, budget)?, budget),
                )? {
                    return Ok(());
                }
            }
        }
        Err(invalid())
    }

    /// Constructive refinement verifies constraints before selected-root predicates may be assumed.
    pub(crate) fn verify_predicate(
        &mut self,
        root: LogicalId,
        bound: &[Parts],
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let mut pending = vector(1, budget)?;
        pending.push(root);
        while let Some(id) = pending.pop() {
            budget.work(1, AT)?;
            let (kind, arguments) = super::super::evidence::predicate(self.evaluation.cas, id)?;
            match (kind, arguments) {
                (PredicateKind::And, _) => {
                    reserve(&mut pending, arguments.len(), budget)?;
                    pending.extend(arguments);
                }
                (PredicateKind::Equal, [left, right]) => {
                    let original = self.expression(*left, bound, budget)?;
                    let expected = self.expression(*right, bound, budget)?;
                    let difference = original.sum(&expected, true, self.evaluation, budget)?;
                    self.zero(&difference.real, budget)?;
                    self.zero(&difference.imaginary, budget)?;
                }
                (PredicateKind::Positive | PredicateKind::NonNegative, [argument]) => {
                    let value = self.expression(*argument, bound, budget)?;
                    self.zero(&value.imaginary, budget)?;
                    self.sign(&value.real, *kind == PredicateKind::Positive, budget)?;
                }
                (PredicateKind::NonZero, [argument]) => {
                    let value = self.expression(*argument, bound, budget)?;
                    if !super::super::product::decides(self.nonzero(&value.real, budget))? {
                        self.nonzero(&value.imaginary, budget)?;
                    }
                }
                _ => return Err(invalid()),
            }
        }
        Ok(())
    }

    /// Sign claims use a rational value or an identical earlier positive regional fact.
    fn sign(&self, value: &Value, strict: bool, budget: &mut Budget) -> Result<(), Fault> {
        #[cfg(test)]
        let _attribution = crate::testing::work_trace::Scope::phase("exclusion.component-sign");
        if let Some(sign) = super::selection::sign(value, budget)?
            && (sign == std::cmp::Ordering::Greater
                || (!strict && sign == std::cmp::Ordering::Equal))
        {
            return Ok(());
        }
        for fact in &self.positive {
            if super::super::product::decides(
                self.zero(&value.sub(fact, &self.evaluation.engine, budget)?, budget),
            )? {
                return Ok(());
            }
        }
        Err(invalid())
    }
}

#[cfg(test)]
mod tests {
    use super::Components;
    use crate::budget::Budget;
    use crate::limits::Limits;
    use crate::solving::coefficients::{
        arithmetic::Engine,
        forms::{Form, Variable},
    };
    use crate::solving::elementary::evaluate::Evaluation;

    /// Field reduction preserves a previously retained term instead of applying a pseudo-scale.
    /// ´test:unit:component-field-reduction-scale´
    #[test]
    fn component_field_reduction_scale() -> Result<(), Box<dyn std::error::Error>> {
        for parameter in [false, true] {
            let (fixture, _) = if parameter {
                crate::testing::solve_blind::symbolic(&["p"])?
            } else {
                crate::testing::solve_blind::gaussian_polynomial(&[(0, 0), (1, 0)])?
            };
            let mut budget = Budget::new(Limits::blind_cubic())?;
            let evaluation = Evaluation {
                cas: &fixture.cas,
                engine: Engine {
                    state: &fixture.cas.state,
                    domain: &fixture.algebra,
                },
                region: &fixture.region,
                premises: &[],
                free: &[],
                bound: &[],
            };
            scale_case(&evaluation, &mut budget)?;
        }
        Ok(())
    }
    /// The independent expected remainder of x³+y² modulo 2y²-2 is x³+1.
    fn scale_case(
        evaluation: &Evaluation<'_>,
        budget: &mut Budget,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let engine = &evaluation.engine;
        let leading = Form::variable(Variable::Slot(0), engine, budget)?.pow(3, engine, budget)?;
        let square = Form::variable(Variable::Slot(1), engine, budget)?.pow(2, engine, budget)?;
        let one = Form::constant(engine.constant(true, budget)?, engine, budget)?;
        let two = one.add(&one, engine, budget)?;
        let relation = square
            .add(&one.neg(engine, budget)?, engine, budget)?
            .mul(&two, engine, budget)?;
        let original = leading.add(&square, engine, budget)?;
        let expected = leading.add(&one, engine, budget)?;
        let mut components = Components::new(evaluation, budget)?;
        components.equations.push(relation);
        let actual = components.reduced(&original, budget)?;
        assert_eq!(actual.terms.len(), expected.terms.len());
        for (actual, expected) in actual.terms.iter().zip(&expected.terms) {
            assert_eq!(actual.powers, expected.powers);
            assert!(engine.formal_equal(&actual.coefficient, &expected.coefficient, budget)?);
        }
        Ok(())
    }
}
