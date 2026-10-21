// SPDX-License-Identifier: AGPL-3.0-only
//! Independent sparse identity arithmetic over arbitrary admitted coefficient algebras.

use super::arithmetic::Engine;
use super::model::{CoefficientFailure, Element};
use super::storage::{AT, add, invalid, product, vector};
use crate::budget::{Budget, BudgetKind};
use crate::graph::{LogicalId, size};
use std::cmp::Ordering;

/// Formal slots and actual scalar atoms are distinct variable namespaces.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Variable {
    /// An ordered bound slot in a graph polynomial.
    Slot(u32),
    /// A scoped scalar atom, identified by its canonical graph node.
    Atom(LogicalId),
}

/// One sparse monomial and its exact coefficient.
#[derive(Debug)]
pub(crate) struct Term {
    /// Increasing variable keys with positive exponents.
    pub(crate) powers: Vec<(Variable, u32)>,
    /// Coefficient in the checker's declared domain.
    pub(crate) coefficient: Element,
}

/// A canonical decreasing graded-lex polynomial used only by the replay checker.
#[derive(Debug)]
pub(crate) struct Form {
    /// No repeated or zero terms.
    pub(crate) terms: Vec<Term>,
}

/// Compare formal exponent vectors, charging the complete key scan.
fn compare(left: &Term, right: &Term, budget: &mut Budget) -> Result<Ordering, CoefficientFailure> {
    budget.work(
        add(size(left.powers.len(), AT)?, size(right.powers.len(), AT)?)?,
        AT,
    )?;
    let degree = |term: &Term| {
        term.powers.iter().try_fold(0_u64, |total, (_, exponent)| {
            add(total, u64::from(*exponent))
        })
    };
    let order = degree(left)?.cmp(&degree(right)?);
    if order != Ordering::Equal {
        return Ok(order);
    }
    let mut first = left.powers.iter().peekable();
    let mut second = right.powers.iter().peekable();
    while let (Some(a), Some(b)) = (first.peek(), second.peek()) {
        match a.0.cmp(&b.0) {
            Ordering::Less => return Ok(Ordering::Greater),
            Ordering::Greater => return Ok(Ordering::Less),
            Ordering::Equal if a.1 != b.1 => return Ok(a.1.cmp(&b.1)),
            Ordering::Equal => {
                let _first = first.next();
                let _second = second.next();
            }
        }
    }
    Ok(first.len().cmp(&second.len()))
}

impl Term {
    /// Copy an exponent key and coefficient with explicit work and storage checks.
    fn copy(&self, engine: &Engine<'_>, budget: &mut Budget) -> Result<Self, CoefficientFailure> {
        Ok(Self {
            powers: super::storage::copied(&self.powers, budget)?,
            coefficient: engine.copy(&self.coefficient, budget)?,
        })
    }

    /// Checked sparse monomial multiplication in the fixed variable order.
    fn mul(
        &self,
        other: &Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        let mut powers = super::storage::copied(&self.powers, budget)?;
        super::storage::reserve(&mut powers, other.powers.len(), budget)?;
        for (variable, exponent) in &other.powers {
            budget.work(size(powers.len(), AT)?.max(1), AT)?;
            match powers.binary_search_by_key(variable, |entry| entry.0) {
                Ok(index) => {
                    let entry = powers.get_mut(index).ok_or_else(invalid)?;
                    entry.1 = entry.1.checked_add(*exponent).ok_or_else(|| {
                        crate::error::Fault::capacity(BudgetKind::PolynomialDegree, AT)
                    })?;
                }
                Err(index) => powers.insert(index, (*variable, *exponent)),
            }
        }
        let degree = powers.iter().try_fold(0_u64, |total, (_, exponent)| {
            add(total, u64::from(*exponent))
        })?;
        budget.check(BudgetKind::PolynomialDegree, degree, AT)?;
        budget.check(BudgetKind::PolynomialVariables, size(powers.len(), AT)?, AT)?;
        Ok(Self {
            powers,
            coefficient: engine.mul(&self.coefficient, &other.coefficient, budget)?,
        })
    }
}

impl Form {
    /// A constant, possibly zero, in the exact source algebra.
    pub(crate) fn constant(
        value: Element,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        let mut terms = vector(1, budget)?;
        if !engine.formal_zero(&value, budget)? {
            terms.push(Term {
                powers: Vec::new(),
                coefficient: value,
            });
        }
        Ok(Self { terms })
    }

    /// A formal variable with unit coefficient.
    pub(crate) fn variable(
        variable: Variable,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        let mut powers = vector(1, budget)?;
        powers.push((variable, 1));
        let mut terms = vector(1, budget)?;
        terms.push(Term {
            powers,
            coefficient: engine.constant(true, budget)?,
        });
        Ok(Self { terms })
    }

    /// Copy all terms with charge.
    pub(crate) fn copy(
        &self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        let mut terms = vector(self.terms.len(), budget)?;
        for term in &self.terms {
            terms.push(term.copy(engine, budget)?);
        }
        Ok(Self { terms })
    }

    /// Merge two sorted lists, reserving the uncancelled output before any coefficient work.
    pub(crate) fn add(
        &self,
        other: &Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        let count = add(size(self.terms.len(), AT)?, size(other.terms.len(), AT)?)?;
        budget.check(BudgetKind::PolynomialTerms, count, AT)?;
        let mut terms = vector(usize::try_from(count).map_err(|_| invalid())?, budget)?;
        let mut left = self.terms.iter().peekable();
        let mut right = other.terms.iter().peekable();
        while let (Some(a), Some(b)) = (left.peek(), right.peek()) {
            match compare(a, b, budget)? {
                Ordering::Greater => {
                    terms.push(a.copy(engine, budget)?);
                    let _left = left.next();
                }
                Ordering::Less => {
                    terms.push(b.copy(engine, budget)?);
                    let _right = right.next();
                }
                Ordering::Equal => {
                    let coefficient = engine.add(&a.coefficient, &b.coefficient, budget)?;
                    if !engine.formal_zero(&coefficient, budget)? {
                        terms.push(Term {
                            powers: super::storage::copied(&a.powers, budget)?,
                            coefficient,
                        });
                    }
                    let _left = left.next();
                    let _right = right.next();
                }
            }
        }
        for term in left.chain(right) {
            terms.push(term.copy(engine, budget)?);
        }
        Ok(Self { terms })
    }

    /// Negate coefficients without changing variable order.
    pub(crate) fn neg(
        &self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        let mut terms = vector(self.terms.len(), budget)?;
        for term in &self.terms {
            terms.push(Term {
                powers: super::storage::copied(&term.powers, budget)?,
                coefficient: engine.neg(&term.coefficient, budget)?,
            });
        }
        Ok(Self { terms })
    }

    /// At most mn term products and mn accumulator insertions, each using charged key comparisons.
    pub(crate) fn mul(
        &self,
        other: &Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        let pairs = product(size(self.terms.len(), AT)?, size(other.terms.len(), AT)?)?;
        if matches!(
            super::storage::domain(engine.state, engine.domain)?,
            crate::solving::CoefficientDomainData::Fraction { .. }
        ) {
            budget.work(pairs, AT)?;
            if self.parameter_unit(engine, budget)? {
                return other.unit_product(engine, budget);
            }
            if other.parameter_unit(engine, budget)? {
                return self.unit_product(engine, budget);
            }
            return self.parameter_product(other, engine, budget);
        }
        budget.check(BudgetKind::PolynomialTerms, pairs, AT)?;
        budget.work(pairs, AT)?;
        if self.parameter_unit(engine, budget)? {
            return other.unit_product(engine, budget);
        }
        if other.parameter_unit(engine, budget)? {
            return self.unit_product(engine, budget);
        }
        let mut result = Self { terms: Vec::new() };
        for left in &self.terms {
            for right in &other.terms {
                let term = left.mul(right, engine, budget)?;
                let mut terms = vector(1, budget)?;
                if !engine.formal_zero(&term.coefficient, budget)? {
                    terms.push(term);
                }
                result = result.add(&Self { terms }, engine, budget)?;
            }
        }
        Ok(result)
    }

    /// A reused unit product still checks the product's current term, variable and degree bounds.
    fn unit_product(
        &self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        budget.check(BudgetKind::PolynomialTerms, size(self.terms.len(), AT)?, AT)?;
        for term in &self.terms {
            budget.work(size(term.powers.len(), AT)?.max(1), AT)?;
            let degree = term.powers.iter().try_fold(0_u64, |total, (_, exponent)| {
                add(total, u64::from(*exponent))
            })?;
            budget.check(BudgetKind::PolynomialDegree, degree, AT)?;
            budget.check(
                BudgetKind::PolynomialVariables,
                size(term.powers.len(), AT)?,
                AT,
            )?;
        }
        self.copy(engine, budget)
    }

    /// Recognize only a literal parameter-field unit, optionally embedded in one exact quotient parent.
    fn parameter_unit(
        &self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<bool, CoefficientFailure> {
        let [term] = self.terms.as_slice() else {
            return Ok(false);
        };
        if !term.powers.is_empty() {
            return Ok(false);
        }
        let mut coefficient = &term.coefficient;
        let mut domain = engine.domain;
        if let crate::solving::CoefficientDomainData::Quotient { level } =
            super::storage::domain(engine.state, domain)?
        {
            let crate::solving::SolveDraft::ExtensionLevel(level) =
                crate::solving::graph::object(engine.state, level.logical_id())?
            else {
                return Err(invalid().into());
            };
            domain = &level.parent;
            let super::model::Value::Tower(entries) = &coefficient.value else {
                return Ok(false);
            };
            let [entry] = entries.as_slice() else {
                return Ok(false);
            };
            if coefficient.domain != *engine.domain {
                return Err(invalid().into());
            }
            coefficient = entry;
        }
        if !matches!(
            super::storage::domain(engine.state, domain)?,
            crate::solving::CoefficientDomainData::Fraction { .. }
        ) {
            return Ok(false);
        }
        budget.work(1, AT)?;
        if coefficient.domain != *domain {
            return Err(invalid().into());
        }
        let super::model::Value::Fraction(value) = &coefficient.value else {
            return Ok(false);
        };
        value
            .numerator
            .equals(&value.denominator, budget)
            .map_err(Into::into)
    }

    /// Insert an owned term without recopying every unrelated accumulated coefficient.
    fn accumulate(
        &mut self,
        term: Term,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<(), CoefficientFailure> {
        if engine.formal_zero(&term.coefficient, budget)? {
            return Ok(());
        }
        let mut first = 0;
        let mut last = self.terms.len();
        while first < last {
            let middle = first.midpoint(last);
            match compare(&self.terms[middle], &term, budget)? {
                Ordering::Greater => first = middle + 1,
                Ordering::Less => last = middle,
                Ordering::Equal => {
                    let coefficient =
                        engine.add(&self.terms[middle].coefficient, &term.coefficient, budget)?;
                    if engine.formal_zero(&coefficient, budget)? {
                        budget.work(size(self.terms.len() - middle, AT)?, AT)?;
                        self.terms.remove(middle);
                    } else {
                        self.terms[middle].coefficient = coefficient;
                    }
                    return Ok(());
                }
            }
        }
        budget.check(
            BudgetKind::PolynomialTerms,
            add(size(self.terms.len(), AT)?, 1)?,
            AT,
        )?;
        super::storage::reserve(&mut self.terms, 1, budget)?;
        budget.work(size(self.terms.len() - first, AT)?, AT)?;
        self.terms.insert(first, term);
        Ok(())
    }

    /// Merge owned parameter-field terms while preserving every unrelated accumulated coefficient.
    pub(crate) fn add_owned(
        &mut self,
        other: Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<(), CoefficientFailure> {
        if !matches!(
            super::storage::domain(engine.state, engine.domain)?,
            crate::solving::CoefficientDomainData::Fraction { .. }
        ) {
            return Err(invalid().into());
        }
        for term in other.terms {
            self.accumulate(term, engine, budget)?;
        }
        Ok(())
    }

    /// Stream prepaid term pairs into a bounded live accumulator, merging exact collisions.
    fn parameter_product(
        &self,
        other: &Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        let mut result = Self {
            terms: vector(0, budget)?,
        };
        for left in &self.terms {
            for right in &other.terms {
                result.accumulate(left.mul(right, engine, budget)?, engine, budget)?;
            }
        }
        Ok(result)
    }

    /// Nonnegative formal powers with bounded repeated squaring.
    pub(crate) fn pow(
        &self,
        exponent: u32,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        budget.check(BudgetKind::IntegerExponent, u64::from(exponent), AT)?;
        let mut result = Self::constant(engine.constant(true, budget)?, engine, budget)?;
        let mut factor = self.copy(engine, budget)?;
        let mut remaining = exponent;
        while remaining > 0 {
            if remaining & 1 == 1 {
                result = result.mul(&factor, engine, budget)?;
            }
            remaining >>= 1;
            if remaining > 0 {
                factor = factor.mul(&factor, engine, budget)?;
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::{Engine, Form, Term, Variable};
    use crate::{Rational, budget::Budget, limits::Limits};

    /// Owned insertion preserves graded order and cancels mixed terms independently of operand order.
    /// ´test:unit:parameter-form-product-accumulation´
    #[test]
    fn parameter_form_product_accumulation() -> Result<(), Box<dyn std::error::Error>> {
        let (fixture, _) = crate::testing::solve_blind::symbolic(&["p"])?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let engine = Engine {
            state: &fixture.cas.state,
            domain: &fixture.algebra,
        };
        let x = Form::variable(Variable::Slot(0), &engine, &mut budget)?;
        let y = Form::variable(Variable::Slot(1), &engine, &mut budget)?;
        let one = Form::constant(engine.constant(true, &mut budget)?, &engine, &mut budget)?;
        let left = x
            .add(&y, &engine, &mut budget)?
            .add(&one, &engine, &mut budget)?;
        let right = x
            .add(&y.neg(&engine, &mut budget)?, &engine, &mut budget)?
            .add(&one, &engine, &mut budget)?;
        let mut expected = Vec::new();
        for (powers, value) in [
            (vec![(Variable::Slot(0), 2)], 1),
            (vec![(Variable::Slot(1), 2)], -1),
            (vec![(Variable::Slot(0), 1)], 2),
            (vec![], 1),
        ] {
            expected.push(Term {
                powers,
                coefficient: engine
                    .rational(&Rational::from_ratio(value, 1, &mut budget)?, &mut budget)?,
            });
        }
        for (first, second) in [(&left, &right), (&right, &left)] {
            let actual = first.mul(second, &engine, &mut budget)?;
            assert_eq!(actual.terms.len(), expected.len());
            for (actual, expected) in actual.terms.iter().zip(&expected) {
                assert_eq!(actual.powers, expected.powers);
                assert!(engine.formal_equal(
                    &actual.coefficient,
                    &expected.coefficient,
                    &mut budget
                )?);
            }
        }
        Ok(())
    }
    /// Four prepaid pairs may cancel to two stored terms, while a third retained term is refused.
    /// ´test:unit:parameter-form-live-term-bound´
    #[test]
    fn parameter_form_live_term_bound() -> Result<(), Box<dyn std::error::Error>> {
        let (fixture, _) = crate::testing::solve_blind::symbolic(&["p"])?;
        let mut setup = Budget::new(Limits::blind_cubic())?;
        let engine = Engine {
            state: &fixture.cas.state,
            domain: &fixture.algebra,
        };
        let x = Form::variable(Variable::Slot(0), &engine, &mut setup)?;
        let one = Form::constant(engine.constant(true, &mut setup)?, &engine, &mut setup)?;
        let left = x.add(&one, &engine, &mut setup)?;
        let right = x.neg(&engine, &mut setup)?.add(&one, &engine, &mut setup)?;
        let limits = Limits {
            polynomial_terms: 2,
            ..Limits::blind_cubic()
        };
        let mut budget = Budget::new(limits)?;
        let result = left.mul(&right, &engine, &mut budget)?;
        assert_eq!(result.terms.len(), 2);
        for (actual, (powers, value)) in result
            .terms
            .iter()
            .zip([(vec![(Variable::Slot(0), 2)], -1), (vec![], 1)])
        {
            assert_eq!(actual.powers, powers);
            let expected =
                engine.rational(&Rational::from_ratio(value, 1, &mut setup)?, &mut setup)?;
            assert!(engine.formal_equal(&actual.coefficient, &expected, &mut setup)?);
        }
        let mut bounded = Budget::new(Limits {
            polynomial_terms: 2,
            ..Limits::blind_cubic()
        })?;
        let error = left
            .mul(&left, &engine, &mut bounded)
            .expect_err("third retained term");
        assert!(matches!(
            error,
            crate::solving::coefficients::model::CoefficientFailure::Fault(crate::Fault {
                kind: crate::FaultKind::Exhausted {
                    budget: crate::budget::BudgetKind::PolynomialTerms,
                    limit: 2,
                    needed: 3,
                    ..
                },
                ..
            })
        ));
        Ok(())
    }

    /// Build an actual immediate quotient over a parameter field for independent arithmetic checks.
    fn unit_fixture() -> Result<(crate::Cas, [crate::Handle; 2]), Box<dyn std::error::Error>> {
        let (mut fixture, _) = crate::testing::solve_blind::symbolic(&["p"])?;
        let mut setup = Budget::new(Limits::blind_cubic())?;
        let mut construction = crate::solving::radicals::construction::Construction {
            header: fixture.header.clone(),
            context: fixture.context.clone(),
            region: fixture.region.clone(),
            steps: Vec::new(),
        };
        let images = crate::solving::solve_search::coefficient_images(
            &mut fixture.cas,
            &construction,
            &fixture.algebra,
            &mut setup,
        )?;
        let map = construction.mapping(&mut fixture.cas, &fixture.algebra, &images, &mut setup)?;
        let engine = Engine {
            state: &fixture.cas.state,
            domain: &fixture.algebra,
        };
        let mut coefficients = Vec::new();
        for value in [-2, 0, 1] {
            coefficients
                .push(engine.rational(&Rational::from_ratio(value, 1, &mut setup)?, &mut setup)?);
        }
        let polynomial =
            crate::solving::towers::dense::Dense::new(&engine, coefficients, &mut setup)?;
        let extension =
            construction.adjoin(&mut fixture.cas, &polynomial, &map, None, &mut setup)?;
        Ok((fixture.cas, [fixture.algebra, extension.domain]))
    }

    /// Compare formal coefficients and exponents without relying on the product implementation.
    fn same_form(
        actual: &Form,
        expected: &Form,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(actual.terms.len(), expected.terms.len());
        for (actual, expected) in actual.terms.iter().zip(&expected.terms) {
            assert_eq!(actual.powers, expected.powers);
            assert!(engine.formal_equal(&actual.coefficient, &expected.coefficient, budget)?);
        }
        Ok(())
    }

    /// Literal one reuses its operand in a parameter algebra and its immediate monic quotient; two does not.
    /// ´test:unit:parameter-form-unit-reuse´
    #[test]
    fn parameter_form_unit_reuse() -> Result<(), Box<dyn std::error::Error>> {
        let (cas, domains) = unit_fixture()?;
        for domain in &domains {
            let mut budget = Budget::new(Limits::blind_cubic())?;
            let engine = Engine {
                state: &cas.state,
                domain,
            };
            let one = Form::constant(engine.constant(true, &mut budget)?, &engine, &mut budget)?;
            let two = Form::constant(
                engine.rational(&Rational::from_ratio(2, 1, &mut budget)?, &mut budget)?,
                &engine,
                &mut budget,
            )?;
            assert!(one.parameter_unit(&engine, &mut budget)?);
            assert!(!two.parameter_unit(&engine, &mut budget)?);
            let x = Form::variable(Variable::Slot(0), &engine, &mut budget)?;
            let operand = x.add(&two, &engine, &mut budget)?;
            for (left, right) in [(&one, &operand), (&operand, &one)] {
                let result = left.mul(right, &engine, &mut budget)?;
                same_form(&result, &operand, &engine, &mut budget)?;
            }
            for limits in [
                Limits {
                    polynomial_terms: 1,
                    ..Limits::blind_cubic()
                },
                Limits {
                    polynomial_degree: 0,
                    ..Limits::blind_cubic()
                },
            ] {
                let mut bounded = Budget::new(limits)?;
                assert!(one.mul(&operand, &engine, &mut bounded).is_err());
            }
            let doubled = two.mul(&operand, &engine, &mut budget)?;
            let expected = operand.add(&operand, &engine, &mut budget)?;
            same_form(&doubled, &expected, &engine, &mut budget)?;
        }
        Ok(())
    }
}
