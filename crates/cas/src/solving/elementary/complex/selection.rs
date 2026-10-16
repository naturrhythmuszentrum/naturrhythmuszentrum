// SPDX-License-Identifier: AGPL-3.0-only
//! Selected-root equations and constraints are interpreted together, never inferred from tags.

use super::super::proof::{AT, invalid};
use super::{Atom, Components, Parts};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Handle, LogicalId};
use crate::model::Field;
use crate::number::Rational;
use crate::solving::coefficients::storage::{polynomial, reserve, value, vector};
use crate::solving::{CoefficientValueData, RootRelationData, SolveDraft};

impl Components<'_, '_> {
    /// Every slot is bound to the exact existing joint selection before any predicate is used.
    pub(crate) fn root(
        &mut self,
        selected: LogicalId,
        slot: u32,
        budget: &mut Budget,
    ) -> Result<Parts, Fault> {
        self.evaluation.existence(selected, budget)?;
        let selection = super::super::selected::selection(
            self.evaluation.cas,
            &self.evaluation.cas.handle(selected),
        )?;
        let relation = super::super::selected::relation(
            self.evaluation.cas,
            selection
                .relations
                .get(usize::try_from(slot).map_err(|_| invalid())?)
                .ok_or_else(invalid)?,
        )?;
        if relation.region != *self.evaluation.region {
            return Err(invalid());
        }
        budget.work(crate::graph::size(self.selections.len(), AT)?.max(1), AT)?;
        if !self.selections.contains(&selected) {
            reserve(&mut self.selections, 1, budget)?;
            self.selections.push(selected);
            self.selection(selected, selection, budget)?;
        }
        self.atom(
            Atom::Root(selected, slot),
            relation.ambient != Field::Complex,
            budget,
        )
    }

    /// Conjugation and product predicates share the same ordered component tuple.
    fn selection(
        &mut self,
        selected: LogicalId,
        selection: &crate::solving::RootSelectionData<Handle>,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        budget.solving(
            BudgetKind::SelectionSlots,
            crate::graph::size(selection.relations.len(), AT)?,
            AT,
        )?;
        let mut roots = vector(selection.relations.len(), budget)?;
        for (index, handle) in selection.relations.iter().enumerate() {
            let relation = super::super::selected::relation(self.evaluation.cas, handle)?;
            if relation.region != *self.evaluation.region {
                return Err(invalid());
            }
            let key = Atom::Root(selected, u32::try_from(index).map_err(|_| invalid())?);
            let mut root = self.atom(key, relation.ambient != Field::Complex, budget)?;
            if relation.ambient == Field::Complex {
                self.quadratic(key, relation, &mut root, budget)?;
            }
            roots.push(root);
        }
        self.record_predicate(selection.predicate.logical_id(), &roots, budget)?;
        for (root, handle) in roots.iter().zip(&selection.relations) {
            let relation = super::super::selected::relation(self.evaluation.cas, handle)?;
            let equation = self.root_equation(root, relation, budget)?;
            self.equation(&equation, budget)?;
        }
        Ok(())
    }

    /// Every mapped coefficient participates in the exact selected polynomial equation.
    pub(crate) fn root_equation(
        &mut self,
        root: &Parts,
        relation: &RootRelationData<Handle>,
        budget: &mut Budget,
    ) -> Result<Parts, Fault> {
        if relation.region != *self.evaluation.region {
            return Err(invalid());
        }
        let stored = polynomial(&self.evaluation.cas.state, &relation.polynomial)?;
        let SolveDraft::CoefficientMap(map) =
            crate::solving::graph::object(&self.evaluation.cas.state, relation.map.logical_id())?
        else {
            return Err(invalid());
        };
        if stored.variables.len() != 1 {
            return Err(invalid());
        }
        let mut result = self.integer(0, budget)?;
        for term in &stored.terms {
            let coefficient = self.mapped(&term.coefficient, map, budget)?;
            let power = root.power(
                i64::from(*term.exponents.first().ok_or_else(invalid)?),
                self.evaluation,
                budget,
            )?;
            result = result.sum(
                &coefficient.mul(&power, self.evaluation, budget)?,
                false,
                self.evaluation,
                budget,
            )?;
        }
        Ok(result)
    }

    /// A real quadratic with negative discriminant fixes its root's real part by completing the square.
    fn quadratic(
        &mut self,
        key: Atom,
        relation: &RootRelationData<Handle>,
        root: &mut Parts,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let Some((center, square)) = quadratic(self.evaluation.cas, relation, budget)? else {
            return Ok(());
        };
        root.real = Parts::rational(self.evaluation, &center, &Rational::zero(), budget)?.real;
        let expected = Parts::rational(self.evaluation, &square, &Rational::zero(), budget)?.real;
        let difference = root
            .imaginary
            .mul(&root.imaginary, &self.evaluation.engine, budget)?
            .sub(&expected, &self.evaluation.engine, budget)?;
        reserve(&mut self.equations, 1, budget)?;
        self.equations.push(difference.numerator);
        for (stored, parts) in &mut self.atoms {
            budget.work(1, AT)?;
            if *stored == key {
                *parts = root.copy(self.evaluation, budget)?;
            }
        }
        Ok(())
    }
}

/// Only canonical ground real constants qualify; no parameter sign is inferred.
fn rational(
    cas: &crate::graph::Cas,
    handle: &Handle,
    budget: &mut Budget,
) -> Result<Option<Rational>, Fault> {
    let mut current = handle;
    loop {
        budget.work(1, AT)?;
        match &value(&cas.state, current)?.value {
            CoefficientValueData::Rational { value } => {
                return Ok(Some(crate::solving::coefficients::scalar::rational_copy(
                    value, budget,
                )?));
            }
            CoefficientValueData::Gaussian { real, imaginary } if imaginary.is_zero() => {
                return Ok(Some(crate::solving::coefficients::scalar::rational_copy(
                    real, budget,
                )?));
            }
            CoefficientValueData::Tower { coefficients } if coefficients.is_empty() => {
                return Ok(Some(Rational::zero()));
            }
            CoefficientValueData::Tower { coefficients } if coefficients.len() == 1 => {
                current = coefficients.first().ok_or_else(invalid)?;
            }
            _ => return Ok(None),
        }
    }
}

/// Negative discriminant gives a strictly positive imaginary square, independently of root orientation.
fn quadratic(
    cas: &crate::graph::Cas,
    relation: &RootRelationData<Handle>,
    budget: &mut Budget,
) -> Result<Option<(Rational, Rational)>, Fault> {
    let stored = polynomial(&cas.state, &relation.polynomial)?;
    if stored
        .terms
        .first()
        .is_none_or(|term| term.exponents.as_slice() != [2])
    {
        return Ok(None);
    }
    let mut coefficients = [Rational::zero(), Rational::zero(), Rational::zero()];
    for term in &stored.terms {
        let Some(constant) = rational(cas, &term.coefficient, budget)? else {
            return Ok(None);
        };
        let index =
            usize::try_from(*term.exponents.first().ok_or_else(invalid)?).map_err(|_| invalid())?;
        *coefficients.get_mut(index).ok_or_else(invalid)? = constant;
    }
    let [constant, linear, leading] = &coefficients;
    let two = Rational::from_ratio(2, 1, budget)?;
    let four = Rational::from_ratio(4, 1, budget)?;
    let discriminant = linear
        .mul(linear, budget)?
        .sub(&four.mul(&leading.mul(constant, budget)?, budget)?, budget)?;
    if discriminant.compare(&Rational::zero(), budget)? != std::cmp::Ordering::Less {
        return Ok(None);
    }
    let center = linear
        .negated(budget)?
        .div(&two.mul(leading, budget)?, budget)?;
    let square = discriminant
        .negated(budget)?
        .div(&four.mul(&leading.mul(leading, budget)?, budget)?, budget)?;
    Ok(Some((center, square)))
}

/// Ground scalar components retain their exact sign through canonical parent inclusions.
pub(crate) fn scalar(
    value: &crate::solving::coefficients::model::Element,
    budget: &mut Budget,
) -> Result<Option<Rational>, Fault> {
    use crate::solving::coefficients::model::Value as Coefficient;
    use crate::solving::coefficients::scalar::Scalar;
    let mut current = value;
    loop {
        budget.work(1, AT)?;
        match &current.value {
            Coefficient::Scalar(Scalar::Rational(value)) => {
                return Ok(Some(crate::solving::coefficients::scalar::rational_copy(
                    value, budget,
                )?));
            }
            Coefficient::Scalar(Scalar::Gaussian(value)) => {
                return if value.imaginary(budget)?.is_zero() {
                    Ok(Some(value.real(budget)?))
                } else {
                    Ok(None)
                };
            }
            Coefficient::Tower(coefficients) if coefficients.is_empty() => {
                return Ok(Some(Rational::zero()));
            }
            Coefficient::Tower(coefficients) if coefficients.len() == 1 => {
                current = coefficients.first().ok_or_else(invalid)?;
            }
            Coefficient::Fraction(fraction) => {
                return match (
                    fraction.numerator.scalar(budget)?,
                    fraction.denominator.scalar(budget)?,
                ) {
                    (Some(Scalar::Rational(top)), Some(Scalar::Rational(bottom))) => {
                        Ok(Some(top.div(&bottom, budget)?))
                    }
                    _ => Ok(None),
                };
            }
            _ => return Ok(None),
        }
    }
}

/// A polynomial's sign is manifest only when it is a rational constant.
fn constant(
    form: &crate::solving::coefficients::forms::Form,
    budget: &mut Budget,
) -> Result<Option<Rational>, Fault> {
    match form.terms.as_slice() {
        [] => Ok(Some(Rational::zero())),
        [term] if term.powers.is_empty() => scalar(&term.coefficient, budget),
        _ => Ok(None),
    }
}

/// Rational quotient sign includes the actual denominator, without approximate evaluation.
pub(crate) fn sign(
    value: &super::super::value::Value,
    budget: &mut Budget,
) -> Result<Option<std::cmp::Ordering>, Fault> {
    let (Some(numerator), Some(denominator)) = (
        constant(&value.numerator, budget)?,
        constant(&value.denominator, budget)?,
    ) else {
        return Ok(None);
    };
    Ok(Some(
        numerator
            .div(&denominator, budget)?
            .compare(&Rational::zero(), budget)?,
    ))
}

impl Components<'_, '_> {
    /// Supplied witnesses prove every relation and constraint before existence is concluded.
    pub(crate) fn witnesses(
        &mut self,
        witnesses: &[Handle],
        selection: &crate::solving::RootSelectionData<Handle>,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        if witnesses.len() != selection.relations.len() {
            return Err(invalid());
        }
        let mut values = vector(witnesses.len(), budget)?;
        for (witness, handle) in witnesses.iter().zip(&selection.relations) {
            let value = self.expression(witness.logical_id(), &[], budget)?;
            let relation = super::super::selected::relation(self.evaluation.cas, handle)?;
            if self
                .evaluation
                .cas
                .state
                .expression(witness.logical_id())?
                .sort
                != crate::model::Sort::Value(crate::model::AtomicSort::Scalar(
                    relation.ambient.clone(),
                ))
            {
                return Err(invalid());
            }
            let equation = self.root_equation(&value, relation, budget)?;
            self.zero(&equation.real, budget)?;
            self.zero(&equation.imaginary, budget)?;
            values.push(value);
        }
        self.verify_predicate(selection.predicate.logical_id(), &values, budget)
    }
}
