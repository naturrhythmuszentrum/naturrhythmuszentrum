// SPDX-License-Identifier: AGPL-3.0-only
//! Defining equations of existing joint selections and explicit scalar coefficient maps.

use super::evaluate::Evaluation;
use super::proof::{AT, invalid};
use super::value::Value;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{ExpressionAttachment, Handle, LogicalId};
use crate::model::Operator;
use crate::solving::coefficients::forms::{Form, Variable};
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{polynomial, reserve, vector};
use crate::solving::{
    CoefficientMapData, CoefficientValueData, RootRelationData, RootSelectionData, SolveDraft,
};

/// Borrow an ordered joint selection.
pub(crate) fn selection<'a>(
    cas: &'a crate::graph::Cas,
    handle: &Handle,
) -> Result<&'a RootSelectionData<Handle>, Fault> {
    match crate::solving::graph::object(&cas.state, cas.checked(handle)?)? {
        SolveDraft::RootSelection(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// Borrow a positive-degree root relation without inferring existence.
pub(crate) fn relation<'a>(
    cas: &'a crate::graph::Cas,
    handle: &Handle,
) -> Result<&'a RootRelationData<Handle>, Fault> {
    match crate::solving::graph::object(&cas.state, cas.checked(handle)?)? {
        SolveDraft::RootRelation(value) => Ok(value),
        _ => Err(invalid()),
    }
}

impl Evaluation<'_> {
    /// A ground scalar is copied into the actual formal arithmetic domain.
    fn ground(
        &self,
        stored: &CoefficientValueData<Handle>,
        budget: &mut Budget,
    ) -> Result<Value, Fault> {
        let rational = match stored {
            CoefficientValueData::Rational { value } => value,
            CoefficientValueData::Gaussian { real, imaginary } if imaginary.is_zero() => real,
            _ => return Err(invalid()),
        };
        let coefficient = self
            .engine
            .rational(rational, budget)
            .map_err(|e| fault(&e))?;
        Value::polynomial(
            Form::constant(coefficient, &self.engine, budget).map_err(|e| fault(&e))?,
            &self.engine,
            budget,
        )
    }

    /// Evaluate a canonical rational function through its explicit ordered scalar images.
    fn mapped_polynomial(
        &self,
        handle: &Handle,
        map: &CoefficientMapData<Handle>,
        budget: &mut Budget,
    ) -> Result<Value, Fault> {
        let stored = polynomial(&self.cas.state, handle)?;
        if stored.variables.len() != map.images.len() {
            return Err(invalid());
        }
        let mut result = Value::integer(0, &self.engine, budget)?;
        for term in &stored.terms {
            let coefficient =
                crate::solving::coefficients::storage::value(&self.cas.state, &term.coefficient)?;
            let mut product = self.ground(&coefficient.value, budget)?;
            for (exponent, image) in term.exponents.iter().zip(&map.images) {
                let factor = self.expression(image.logical_id(), true, budget)?.pow(
                    *exponent,
                    &self.engine,
                    budget,
                )?;
                product = product.mul(&factor, &self.engine, budget)?;
            }
            result = result.add(&product, &self.engine, budget)?;
        }
        Ok(result)
    }

    /// Interpret the selected scalar map, preserving every rational-function denominator.
    pub(crate) fn mapped(
        &self,
        handle: &Handle,
        map: &CoefficientMapData<Handle>,
        budget: &mut Budget,
    ) -> Result<Value, Fault> {
        let stored = crate::solving::coefficients::storage::value(&self.cas.state, handle)?;
        if stored.domain != map.source {
            return Err(invalid());
        }
        if map.region != *self.region {
            return Err(invalid());
        }
        crate::solving::coefficients::proof::identity_map(self.cas, map, self.premises, budget)?;
        match &stored.value {
            CoefficientValueData::Rational { .. } | CoefficientValueData::Gaussian { .. } => {
                self.ground(&stored.value, budget)
            }
            CoefficientValueData::Fraction {
                numerator,
                denominator,
            } => {
                let dividend = self.mapped_polynomial(numerator, map, budget)?;
                let divisor = self.mapped_polynomial(denominator, map, budget)?;
                self.nonzero(&divisor, budget)?;
                dividend.div(&divisor, &self.engine, budget)
            }
            CoefficientValueData::Tower { coefficients } => self.tower(coefficients, map, budget),
        }
    }

    /// A tower evaluation follows the exact parent map of its selected generator.
    fn tower(
        &self,
        coefficients: &[Handle],
        map: &CoefficientMapData<Handle>,
        budget: &mut Budget,
    ) -> Result<Value, Fault> {
        let [image] = map.images.as_slice() else {
            return Err(invalid());
        };
        let expression = self.cas.state.expression(image.logical_id())?;
        let (Operator::RootValue { slot }, Some(ExpressionAttachment::SelectedRoot(selected))) =
            (&expression.operator, &expression.attachment)
        else {
            return Err(invalid());
        };
        let selection = selection(self.cas, &self.cas.handle(*selected))?;
        let relation = relation(
            self.cas,
            selection
                .relations
                .get(usize::try_from(*slot).map_err(|_| invalid())?)
                .ok_or_else(invalid)?,
        )?;
        let SolveDraft::CoefficientMap(parent) =
            crate::solving::graph::object(&self.cas.state, relation.map.logical_id())?
        else {
            return Err(invalid());
        };
        let generator = self.expression(image.logical_id(), true, budget)?;
        let mut result = Value::integer(0, &self.engine, budget)?;
        for coefficient in coefficients.iter().rev() {
            result = result.mul(&generator, &self.engine, budget)?.add(
                &self.mapped(coefficient, parent, budget)?,
                &self.engine,
                budget,
            )?;
        }
        Ok(result)
    }

    /// Substitute a scalar into every term of the exact specialized root equation.
    pub(crate) fn root_equation(
        &self,
        root: &Value,
        relation: &RootRelationData<Handle>,
        budget: &mut Budget,
    ) -> Result<Value, Fault> {
        if relation.region != *self.region {
            return Err(invalid());
        }
        let stored = polynomial(&self.cas.state, &relation.polynomial)?;
        let SolveDraft::CoefficientMap(map) =
            crate::solving::graph::object(&self.cas.state, relation.map.logical_id())?
        else {
            return Err(invalid());
        };
        if stored.variables.len() != 1 {
            return Err(invalid());
        }
        let mut result = Value::integer(0, &self.engine, budget)?;
        for term in &stored.terms {
            let coefficient = self.mapped(&term.coefficient, map, budget)?;
            let power = root.pow(
                *term.exponents.first().ok_or_else(invalid)?,
                &self.engine,
                budget,
            )?;
            result = result.add(
                &coefficient.mul(&power, &self.engine, budget)?,
                &self.engine,
                budget,
            )?;
        }
        Ok(result)
    }

    /// Root atoms in an identity contribute only equations of their exact existing selection slots.
    pub(crate) fn root_relations(
        &self,
        form: &Form,
        budget: &mut Budget,
    ) -> Result<Vec<Form>, Fault> {
        let mut seen = vector(0, budget)?;
        let mut result = vector(0, budget)?;
        for term in &form.terms {
            for (variable, _) in &term.powers {
                let Variable::Atom(id) = variable else {
                    continue;
                };
                budget.work(crate::graph::size(seen.len(), AT)?.max(1), AT)?;
                if seen.contains(id) {
                    continue;
                }
                reserve(&mut seen, 1, budget)?;
                seen.push(*id);
                let value = self.cas.state.expression(*id)?;
                let (
                    Operator::RootValue { slot },
                    Some(ExpressionAttachment::SelectedRoot(selected)),
                ) = (&value.operator, &value.attachment)
                else {
                    continue;
                };
                let selected = selection(self.cas, &self.cas.handle(*selected))?;
                budget.solving(
                    BudgetKind::SelectionSlots,
                    crate::graph::size(selected.relations.len(), AT)?,
                    AT,
                )?;
                let handle = selected
                    .relations
                    .get(usize::try_from(*slot).map_err(|_| invalid())?)
                    .ok_or_else(invalid)?;
                let root = self.expression(*id, true, budget)?;
                let equation = self.root_equation(&root, relation(self.cas, handle)?, budget)?;
                reserve(&mut result, 1, budget)?;
                result.push(equation.numerator);
            }
        }
        Ok(result)
    }
}

/// Locate a slot occurrence by following only the supplied expression closure.
pub(crate) fn occurrence(
    cas: &crate::graph::Cas,
    roots: &[Handle],
    selected: &Handle,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let mut pending = vector(roots.len(), budget)?;
    pending.extend(roots.iter().map(Handle::logical_id));
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        let expression = cas.state.expression(id)?;
        if matches!((&expression.operator, &expression.attachment), (Operator::RootValue { slot: 0 }, Some(ExpressionAttachment::SelectedRoot(handle))) if *handle == selected.logical_id())
        {
            return Ok(id);
        }
        reserve(&mut pending, expression.operands.len(), budget)?;
        pending.extend(&expression.operands);
        if let Some(ExpressionAttachment::ExactCoefficient { map, .. }) = &expression.attachment
            && let SolveDraft::CoefficientMap(mapping) =
                crate::solving::graph::object(&cas.state, *map)?
        {
            reserve(&mut pending, mapping.images.len(), budget)?;
            pending.extend(mapping.images.iter().map(Handle::logical_id));
        }
    }
    Err(invalid())
}
