// SPDX-License-Identifier: AGPL-3.0-only
//! Parse only the exact polynomial fragment into independently evaluated sparse forms.

use super::arithmetic::Engine;
use super::forms::{Form, Variable};
use super::model::CoefficientFailure;
use super::storage::{AT, invalid, vector};
use crate::budget::{Budget, BudgetKind};
use crate::graph::{Cas, ExpressionAttachment, Handle, LogicalId};
use crate::model::Operator;
use crate::proof_model::{CheckedApplication, Statement};
use crate::solving::{SolveDraft, SolveStatement};

/// Complete interpretation shared by both identity endpoints and all relation multiples.
#[derive(Debug)]
pub(crate) struct Interpretation<'a> {
    /// Read-only graph store.
    pub(crate) cas: &'a Cas,
    /// Exact coefficient algebra.
    pub(crate) engine: Engine<'a>,
    /// Explicit scalar/domain map identity.
    pub(crate) map: &'a Handle,
    /// Exact regional scope of the theorem.
    pub(crate) region: &'a Handle,
    /// Already independently checked earlier applications.
    pub(crate) premises: &'a [CheckedApplication],
}

impl Interpretation<'_> {
    /// Borrow the exact interpretation map already checked for this application.
    fn coefficient_map(
        &self,
    ) -> Result<&crate::solving::CoefficientMapData<Handle>, crate::error::Fault> {
        let SolveDraft::CoefficientMap(map) =
            crate::solving::graph::object(&self.cas.state, self.cas.checked(self.map)?)?
        else {
            return Err(invalid());
        };
        Ok(map)
    }

    /// Each coefficient is interpreted in the same independently checked destination algebra.
    fn coefficient(
        &self,
        handle: &Handle,
        budget: &mut Budget,
    ) -> Result<super::model::Element, CoefficientFailure> {
        super::transport::coefficient(
            self.cas,
            self.coefficient_map()?,
            &self.engine,
            handle,
            budget,
        )
    }

    /// Graph polynomials retain their ordered bound slots, independent of declaration display names.
    fn polynomial(&self, handle: &Handle, budget: &mut Budget) -> Result<Form, CoefficientFailure> {
        super::proof::defined(self.cas, handle, self.region, self.premises, budget)?;
        let stored = super::storage::polynomial(&self.cas.state, handle)?;
        let map = self.coefficient_map()?;
        if stored.domain != *self.engine.domain && stored.domain != map.source {
            return Err(invalid().into());
        }
        let mut result = Form { terms: Vec::new() };
        for term in &stored.terms {
            let mut product = Form::constant(
                self.coefficient(&term.coefficient, budget)?,
                &self.engine,
                budget,
            )?;
            for (index, exponent) in term.exponents.iter().enumerate() {
                let slot = u32::try_from(index).map_err(|_| invalid())?;
                let variable = Form::variable(Variable::Slot(slot), &self.engine, budget)?;
                product = product.mul(
                    &variable.pow(*exponent, &self.engine, budget)?,
                    &self.engine,
                    budget,
                )?;
            }
            result = result.add(&product, &self.engine, budget)?;
        }
        Ok(result)
    }

    /// A root atom is defined only after its exact joint selection has an existence theorem.
    fn selected(
        &self,
        selection: LogicalId,
        budget: &mut Budget,
    ) -> Result<(), CoefficientFailure> {
        for premise in self.premises {
            budget.work(1, AT)?;
            if let Statement::Solve(statement) = &premise.judgment.statement
                && let SolveStatement::RootExistence(existence) = statement.as_ref()
                && existence.relation.logical_id() == selection
                && existence.region == *self.region
            {
                return Ok(());
            }
        }
        Err(invalid().into())
    }

    /// Leaves are canonical coefficients or scoped polynomial variables, with explicit root existence.
    fn atom(&self, id: LogicalId, budget: &mut Budget) -> Result<Form, CoefficientFailure> {
        let expression = self.cas.state.expression(id)?;
        match (&expression.operator, &expression.attachment) {
            (Operator::Rational(value), None) => {
                Form::constant(self.engine.rational(value, budget)?, &self.engine, budget)
            }
            (Operator::Free(_) | Operator::Bound { .. }, None) => {
                Form::variable(Variable::Atom(id), &self.engine, budget)
            }
            (Operator::RootValue { .. }, Some(ExpressionAttachment::SelectedRoot(selection))) => {
                self.selected(*selection, budget)?;
                Form::variable(Variable::Atom(id), &self.engine, budget)
            }
            (
                Operator::ExactCoefficient,
                Some(ExpressionAttachment::ExactCoefficient { value, map }),
            ) if *map == self.map.logical_id() => {
                let handle = self.cas.handle(*value);
                super::proof::defined(self.cas, &handle, self.region, self.premises, budget)?;
                Form::constant(self.coefficient(&handle, budget)?, &self.engine, budget)
            }
            _ => Err(invalid().into()),
        }
    }

    /// Execute one postfix arithmetic node without allowing unproved division or branch-sensitive powers.
    fn operation(
        &self,
        id: LogicalId,
        operands: &[Form],
        budget: &mut Budget,
    ) -> Result<Form, CoefficientFailure> {
        let expression = self.cas.state.expression(id)?;
        match (&expression.operator, operands) {
            (Operator::Neg, [value]) => value.neg(&self.engine, budget),
            (Operator::Add, [left, right]) => left.add(right, &self.engine, budget),
            (Operator::Sub, [left, right]) => {
                left.add(&right.neg(&self.engine, budget)?, &self.engine, budget)
            }
            (Operator::Mul, [left, right]) => left.mul(right, &self.engine, budget),
            (Operator::IntegerPower(exponent), [value]) if *exponent > 0 => value.pow(
                u32::try_from(*exponent).map_err(|_| invalid())?,
                &self.engine,
                budget,
            ),
            (_, []) => self.atom(id, budget),
            _ => Err(invalid().into()),
        }
    }

    /// Explicit postfix traversal keeps depth, storage and every visited expression bounded.
    fn expression(&self, handle: &Handle, budget: &mut Budget) -> Result<Form, CoefficientFailure> {
        let mut pending = vector(1, budget)?;
        pending.push((self.cas.checked(handle)?, false));
        let mut values: Vec<Form> = vector(1, budget)?;
        let region = self.cas.state.region(self.region.logical_id())?;
        while let Some((id, ready)) = pending.pop() {
            budget.work(1, AT)?;
            let expression = self.cas.state.expression(id)?;
            budget.check(BudgetKind::Depth, expression.depth, AT)?;
            if expression.header != region.header
                || expression.scope != region.scope
                || expression.space.is_some()
            {
                return Err(invalid().into());
            }
            if !ready && !expression.operands.is_empty() {
                let additional = expression.operands.len().checked_add(1).ok_or_else(|| {
                    crate::error::Fault::capacity(BudgetKind::StagedPayloadBytes, AT)
                })?;
                super::storage::reserve(&mut pending, additional, budget)?;
                pending.push((id, true));
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
                let result =
                    self.operation(id, values.get(start..).ok_or_else(invalid)?, budget)?;
                values.truncate(start);
                super::storage::reserve(&mut values, 1, budget)?;
                values.push(result);
            }
        }
        if values.len() != 1 {
            return Err(invalid().into());
        }
        values.pop().ok_or_else(|| invalid().into())
    }

    /// Interpret a coefficient, sparse polynomial or exactly parsed scalar polynomial expression.
    pub(crate) fn endpoint(
        &self,
        handle: &Handle,
        budget: &mut Budget,
    ) -> Result<Form, CoefficientFailure> {
        budget.work(1, AT)?;
        let id = self.cas.checked(handle)?;
        if id.kind == crate::graph::VertexKind::Expression {
            return self.expression(handle, budget);
        }
        match crate::solving::graph::object(&self.cas.state, id)? {
            SolveDraft::CoefficientValue(_) => {
                super::proof::defined(self.cas, handle, self.region, self.premises, budget)?;
                Form::constant(self.coefficient(handle, budget)?, &self.engine, budget)
            }
            SolveDraft::Polynomial(_) => self.polynomial(handle, budget),
            _ => Err(invalid().into()),
        }
    }
}
