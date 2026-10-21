// SPDX-License-Identifier: AGPL-3.0-only
//! Simultaneous scalar substitution with strict, separately checked division guards.

use super::proof::{AT, invalid};
use super::value::Value;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, ExpressionAttachment, Handle, LogicalId};
use crate::model::Operator;
use crate::proof_model::CheckedApplication;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::forms::{Form, Variable};
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{reserve, vector};

/// A read-only simultaneous interpretation, shared by equations, domains and inverse compositions.
pub(crate) struct Evaluation<'a> {
    /// Owning immutable graph.
    pub(crate) cas: &'a Cas,
    /// Exact formal coefficient algebra.
    pub(crate) engine: Engine<'a>,
    /// Actual coefficient region of the assertion.
    pub(crate) region: &'a Handle,
    /// Already replayed scoped facts.
    pub(crate) premises: &'a [CheckedApplication],
    /// Declaration replacements, applied once, never recursively.
    pub(crate) free: &'a [(&'a str, Value)],
    /// Optional innermost source frame replacements.
    pub(crate) bound: &'a [Value],
}

impl Evaluation<'_> {
    /// Independent declaration indices keep free parameters identical across lifted binder scopes.
    fn declaration(&self, id: LogicalId, key: &str, budget: &mut Budget) -> Result<Value, Fault> {
        for (name, value) in self.free {
            budget.work(1, AT)?;
            if *name == key {
                return value.copy(&self.engine, budget);
            }
        }
        if let Some(value) = self.parameter(id, budget)? {
            return Ok(value);
        }
        let expression = self.cas.state.expression(id)?;
        let header = self.cas.state.header(expression.header)?;
        for (index, declaration) in header.environment.declarations.iter().enumerate() {
            budget.work(1, AT)?;
            if declaration.key == key {
                return Value::variable(
                    Variable::Slot(u32::try_from(index).map_err(|_| invalid())?),
                    &self.engine,
                    budget,
                );
            }
        }
        Err(invalid())
    }

    /// Constants and root atoms never acquire a branch identity from their spelling.
    fn atom(&self, id: LogicalId, budget: &mut Budget) -> Result<Value, Fault> {
        let expression = self.cas.state.expression(id)?;
        match (&expression.operator, &expression.attachment) {
            (Operator::Rational(value), None) => {
                let coefficient = self.engine.rational(value, budget).map_err(|e| fault(&e))?;
                Value::polynomial(
                    Form::constant(coefficient, &self.engine, budget).map_err(|e| fault(&e))?,
                    &self.engine,
                    budget,
                )
            }
            (Operator::Free(key), None) => self.declaration(id, key, budget),
            (Operator::Bound { depth: 0, slot }, None) if !self.bound.is_empty() => self
                .bound
                .get(usize::try_from(*slot).map_err(|_| invalid())?)
                .ok_or_else(invalid)?
                .copy(&self.engine, budget),
            (Operator::Bound { .. }, None) => {
                Value::variable(Variable::Atom(id), &self.engine, budget)
            }
            (Operator::RootValue { .. }, Some(ExpressionAttachment::SelectedRoot(selection))) => {
                self.existence(*selection, budget)?;
                Value::variable(Variable::Atom(id), &self.engine, budget)
            }
            (
                Operator::ExactCoefficient,
                Some(ExpressionAttachment::ExactCoefficient { value, map }),
            ) => self.coefficient(*value, *map, budget),
            _ => Err(invalid()),
        }
    }

    /// Literal maps are checked before their exact coefficient value is interpreted.
    fn coefficient(
        &self,
        value: LogicalId,
        map: LogicalId,
        budget: &mut Budget,
    ) -> Result<Value, Fault> {
        let crate::solving::SolveDraft::CoefficientMap(mapping) =
            crate::solving::graph::object(&self.cas.state, map)?
        else {
            return Err(invalid());
        };
        let mut interpreted = self.mapped(&self.cas.handle(value), mapping, budget)?;
        reserve(&mut interpreted.maps, 1, budget)?;
        budget.work(crate::graph::size(interpreted.maps.len(), AT)?.max(1), AT)?;
        if !interpreted.maps.contains(&map) {
            interpreted.maps.push(map);
        }
        Ok(interpreted)
    }

    /// A selected root is usable only with existence of that exact joint selection.
    pub(crate) fn existence(&self, selection: LogicalId, budget: &mut Budget) -> Result<(), Fault> {
        for premise in self.premises {
            budget.work(1, AT)?;
            if let crate::proof_model::Statement::Solve(statement) = &premise.judgment.statement
                && let crate::solving::SolveStatement::RootExistence(value) = statement.as_ref()
                && (value.relation.logical_id(), &value.region) == (selection, self.region)
            {
                return Ok(());
            }
        }
        Err(invalid())
    }

    /// Strict operations check the divisor before any algebraic cancellation.
    fn operation(
        &self,
        id: LogicalId,
        operands: &[Value],
        checked: bool,
        budget: &mut Budget,
    ) -> Result<Value, Fault> {
        match (&self.cas.state.expression(id)?.operator, operands) {
            (Operator::Neg, [value]) => value.neg(&self.engine, budget),
            (Operator::Add, [left, right]) => left.add(right, &self.engine, budget),
            (Operator::Sub, [left, right]) => left.sub(right, &self.engine, budget),
            (Operator::Mul, [left, right]) => left.mul(right, &self.engine, budget),
            (Operator::Div, [left, right]) => {
                if checked {
                    self.nonzero(right, budget)?;
                }
                left.div(right, &self.engine, budget)
            }
            (Operator::IntegerPower(exponent), [value]) if *exponent > 0 => value.pow(
                u32::try_from(*exponent).map_err(|_| invalid())?,
                &self.engine,
                budget,
            ),
            (_, []) => self.atom(id, budget),
            _ => Err(invalid()),
        }
    }

    /// Explicit postfix traversal bounds storage and avoids recursive expression evaluation.
    pub(crate) fn expression(
        &self,
        id: LogicalId,
        checked: bool,
        budget: &mut Budget,
    ) -> Result<Value, Fault> {
        let mut pending = vector(1, budget)?;
        pending.push((id, false));
        let mut values = vector(1, budget)?;
        let region = self.cas.state.region(self.region.logical_id())?;
        while let Some((current, ready)) = pending.pop() {
            budget.work(1, AT)?;
            let expression = self.cas.state.expression(current)?;
            budget.check(BudgetKind::Depth, expression.depth, AT)?;
            if expression.header != region.header || expression.space.is_some() {
                return Err(invalid());
            }
            if !ready && !expression.operands.is_empty() {
                reserve(
                    &mut pending,
                    expression
                        .operands
                        .len()
                        .checked_add(1)
                        .ok_or_else(invalid)?,
                    budget,
                )?;
                pending.push((current, true));
                pending.extend(
                    expression
                        .operands
                        .iter()
                        .rev()
                        .map(|child| (*child, false)),
                );
            } else {
                let start = values
                    .len()
                    .checked_sub(expression.operands.len())
                    .ok_or_else(invalid)?;
                let result = self.operation(
                    current,
                    values.get(start..).ok_or_else(invalid)?,
                    checked,
                    budget,
                )?;
                values.truncate(start);
                reserve(&mut values, 1, budget)?;
                values.push(result);
            }
        }
        if values.len() != 1 {
            return Err(invalid());
        }
        values.pop().ok_or_else(invalid)
    }
}
