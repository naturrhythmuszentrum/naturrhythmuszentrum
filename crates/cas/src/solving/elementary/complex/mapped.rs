// SPDX-License-Identifier: AGPL-3.0-only
//! Interpret fractions and reduced tower vectors through their exact scalar maps.

use super::super::proof::{AT, invalid};
use super::{Components, Parts};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{ExpressionAttachment, Handle, LogicalId};
use crate::model::Operator;
use crate::solving::coefficients::storage::{polynomial, value};
use crate::solving::{CoefficientMapData, CoefficientValueData, SolveDraft};

impl Components<'_, '_> {
    /// Preserve the old ground-map checks before opening a mapped literal.
    pub(crate) fn literal(
        &mut self,
        value: LogicalId,
        map: LogicalId,
        budget: &mut Budget,
    ) -> Result<Parts, Fault> {
        let SolveDraft::CoefficientMap(mapping) =
            crate::solving::graph::object(&self.evaluation.cas.state, map)?
        else {
            return Err(invalid());
        };
        self.mapped(&self.evaluation.cas.handle(value), mapping, budget)
    }

    /// Every recursive parent value retains its source, region and checked homomorphism.
    pub(crate) fn mapped(
        &mut self,
        handle: &Handle,
        map: &CoefficientMapData<Handle>,
        budget: &mut Budget,
    ) -> Result<Parts, Fault> {
        budget.work(1, AT)?;
        self.depth = self.depth.checked_add(1).ok_or_else(invalid)?;
        budget.check(BudgetKind::Depth, self.depth, AT)?;
        let stored = value(&self.evaluation.cas.state, handle)?;
        if stored.domain != map.source {
            return Err(invalid());
        }
        if map.region != *self.evaluation.region {
            crate::solving::family::mapping::base_map(self.evaluation, map, budget)?;
        }
        crate::solving::coefficients::proof::identity_map(
            self.evaluation.cas,
            map,
            self.evaluation.premises,
            budget,
        )?;
        let result = match &stored.value {
            CoefficientValueData::Rational { value } => Parts::rational(
                self.evaluation,
                value,
                &crate::number::Rational::zero(),
                budget,
            ),
            CoefficientValueData::Gaussian { real, imaginary } => {
                Parts::rational(self.evaluation, real, imaginary, budget)
            }
            CoefficientValueData::Fraction {
                numerator,
                denominator,
            } => {
                let dividend = self.polynomial(numerator, map, budget)?;
                let divisor = self.polynomial(denominator, map, budget)?;
                self.divide(&dividend, &divisor, budget)
            }
            CoefficientValueData::Tower { coefficients } => self.tower(coefficients, map, budget),
        };
        self.depth = self.depth.checked_sub(1).ok_or_else(invalid)?;
        result
    }

    /// Ground coefficients of a fraction polynomial use the canonical real/Gaussian embedding.
    fn ground(&self, handle: &Handle, budget: &mut Budget) -> Result<Parts, Fault> {
        match &value(&self.evaluation.cas.state, handle)?.value {
            CoefficientValueData::Rational { value } => Parts::rational(
                self.evaluation,
                value,
                &crate::number::Rational::zero(),
                budget,
            ),
            CoefficientValueData::Gaussian { real, imaginary } => {
                Parts::rational(self.evaluation, real, imaginary, budget)
            }
            _ => Err(invalid()),
        }
    }

    /// Fraction variables are evaluated in map order; their denominator is never discarded.
    fn polynomial(
        &mut self,
        handle: &Handle,
        map: &CoefficientMapData<Handle>,
        budget: &mut Budget,
    ) -> Result<Parts, Fault> {
        let stored = polynomial(&self.evaluation.cas.state, handle)?;
        if stored.variables.len() != map.images.len() {
            return Err(invalid());
        }
        let mut result = self.integer(0, budget)?;
        for term in &stored.terms {
            let mut product = self.ground(&term.coefficient, budget)?;
            for (exponent, image) in term.exponents.iter().zip(&map.images) {
                let factor = self.expression(image.logical_id(), &[], budget)?.power(
                    i64::from(*exponent),
                    self.evaluation,
                    budget,
                )?;
                product = product.mul(&factor, self.evaluation, budget)?;
            }
            result = result.sum(&product, false, self.evaluation, budget)?;
        }
        Ok(result)
    }

    /// A tower's parent map is the map of its actual selected generator relation.
    fn tower(
        &mut self,
        coefficients: &[Handle],
        map: &CoefficientMapData<Handle>,
        budget: &mut Budget,
    ) -> Result<Parts, Fault> {
        let [image] = map.images.as_slice() else {
            return Err(invalid());
        };
        let expression = self.evaluation.cas.state.expression(image.logical_id())?;
        let (Operator::RootValue { slot }, Some(ExpressionAttachment::SelectedRoot(selected))) =
            (&expression.operator, &expression.attachment)
        else {
            return Err(invalid());
        };
        let selection = super::super::selected::selection(
            self.evaluation.cas,
            &self.evaluation.cas.handle(*selected),
        )?;
        let relation = super::super::selected::relation(
            self.evaluation.cas,
            selection
                .relations
                .get(usize::try_from(*slot).map_err(|_| invalid())?)
                .ok_or_else(invalid)?,
        )?;
        let SolveDraft::CoefficientMap(parent) =
            crate::solving::graph::object(&self.evaluation.cas.state, relation.map.logical_id())?
        else {
            return Err(invalid());
        };
        let generator = self.expression(image.logical_id(), &[], budget)?;
        let mut result = self.integer(0, budget)?;
        for coefficient in coefficients.iter().rev() {
            result = result.mul(&generator, self.evaluation, budget)?.sum(
                &self.mapped(coefficient, parent, budget)?,
                false,
                self.evaluation,
                budget,
            )?;
        }
        Ok(result)
    }

    /// Integer components use the same metered scalar arithmetic as ground literals.
    pub(crate) fn integer(&self, integer: i64, budget: &mut Budget) -> Result<Parts, Fault> {
        Parts::rational(
            self.evaluation,
            &crate::number::Rational::from_ratio(integer, 1, budget)?,
            &crate::number::Rational::zero(),
            budget,
        )
    }
}
