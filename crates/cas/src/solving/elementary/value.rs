// SPDX-License-Identifier: AGPL-3.0-only
//! Strict rational expressions over the independently metered coefficient algebra.

use crate::budget::Budget;
use crate::error::Fault;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::forms::{Form, Variable};
use crate::solving::coefficients::proof::fault;

/// Uncancelled denominator factors retain the defined domain of every input operation.
#[derive(Debug)]
pub(crate) struct Value {
    /// Exact scalar coefficient maps used before arithmetic cancellation.
    pub(crate) maps: Vec<crate::graph::LogicalId>,
    /// Polynomial numerator.
    pub(crate) numerator: Form,
    /// Polynomial denominator; no cancellation removes its obligations.
    pub(crate) denominator: Form,
}

impl Value {
    /// Preserve map identities even when an arithmetic term cancels to zero.
    fn combined_maps(
        &self,
        other: &Self,
        budget: &mut Budget,
    ) -> Result<Vec<crate::graph::LogicalId>, Fault> {
        let mut result = crate::solving::coefficients::storage::copied(&self.maps, budget)?;
        for map in &other.maps {
            budget.work(
                crate::graph::size(result.len(), super::proof::AT)?.max(1),
                super::proof::AT,
            )?;
            if !result.contains(map) {
                crate::solving::coefficients::storage::reserve(&mut result, 1, budget)?;
                result.push(*map);
            }
        }
        Ok(result)
    }

    /// Embed an exact polynomial as a total rational expression.
    pub(crate) fn polynomial(
        numerator: Form,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let denominator = Form::constant(
            engine.constant(true, budget).map_err(|e| fault(&e))?,
            engine,
            budget,
        )
        .map_err(|e| fault(&e))?;
        Ok(Self {
            maps: crate::solving::coefficients::storage::vector(0, budget)?,
            numerator,
            denominator,
        })
    }

    /// Construct a small integer through prepaid rational arithmetic.
    pub(crate) fn integer(
        value: i64,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let rational = crate::number::Rational::from_ratio(value, 1, budget)?;
        let coefficient = engine.rational(&rational, budget).map_err(|e| fault(&e))?;
        let polynomial = Form::constant(coefficient, engine, budget).map_err(|e| fault(&e))?;
        Self::polynomial(polynomial, engine, budget)
    }

    /// Construct a formal atom without assigning it any equation or nonzero property.
    pub(crate) fn variable(
        variable: Variable,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let polynomial = Form::variable(variable, engine, budget).map_err(|e| fault(&e))?;
        Self::polynomial(polynomial, engine, budget)
    }

    /// Copy both uncancelled components with metering.
    pub(crate) fn copy(&self, engine: &Engine<'_>, budget: &mut Budget) -> Result<Self, Fault> {
        Ok(Self {
            maps: crate::solving::coefficients::storage::copied(&self.maps, budget)?,
            numerator: self.numerator.copy(engine, budget).map_err(|e| fault(&e))?,
            denominator: self
                .denominator
                .copy(engine, budget)
                .map_err(|e| fault(&e))?,
        })
    }

    /// Negation preserves every original denominator.
    pub(crate) fn neg(&self, engine: &Engine<'_>, budget: &mut Budget) -> Result<Self, Fault> {
        Ok(Self {
            maps: crate::solving::coefficients::storage::copied(&self.maps, budget)?,
            numerator: self.numerator.neg(engine, budget).map_err(|e| fault(&e))?,
            denominator: self
                .denominator
                .copy(engine, budget)
                .map_err(|e| fault(&e))?,
        })
    }

    /// Cross multiplication preserves all strict-domain factors.
    pub(crate) fn add(
        &self,
        other: &Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        if matches!(
            crate::solving::coefficients::storage::domain(engine.state, engine.domain)?,
            crate::solving::CoefficientDomainData::Fraction { .. }
        ) && let Some(result) = self.shared_monomial_sum(other, engine, budget)?
        {
            return Ok(result);
        }
        let left = self
            .numerator
            .mul(&other.denominator, engine, budget)
            .map_err(|e| fault(&e))?;
        let right = other
            .numerator
            .mul(&self.denominator, engine, budget)
            .map_err(|e| fault(&e))?;
        Ok(Self {
            maps: self.combined_maps(other, budget)?,
            numerator: left.add(&right, engine, budget).map_err(|e| fault(&e))?,
            denominator: self
                .denominator
                .mul(&other.denominator, engine, budget)
                .map_err(|e| fault(&e))?,
        })
    }

    /// Share repeated monomial denominator factors without removing their zero locus or map provenance.
    fn shared_monomial_sum(
        &self,
        other: &Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Option<Self>, Fault> {
        use crate::solving::coefficients::{
            forms::Term,
            storage::{copied, vector},
        };
        let ([first], [second]) = (
            self.denominator.terms.as_slice(),
            other.denominator.terms.as_slice(),
        ) else {
            return Ok(None);
        };
        let mut left = copied(&first.powers, budget)?;
        let mut right = copied(&second.powers, budget)?;
        let mut common = false;
        for (variable, exponent) in &mut left {
            budget.work(
                crate::graph::size(right.len(), super::proof::AT)?.max(1),
                super::proof::AT,
            )?;
            if let Ok(index) = right.binary_search_by_key(variable, |entry| entry.0) {
                let shared = (*exponent).min(right[index].1);
                *exponent -= shared;
                right[index].1 -= shared;
                common |= shared > 0;
            }
        }
        if !common {
            return Ok(None);
        }
        budget.work(
            crate::solving::coefficients::storage::add(
                crate::graph::size(left.len(), super::proof::AT)?,
                crate::graph::size(right.len(), super::proof::AT)?,
            )?,
            super::proof::AT,
        )?;
        left.retain(|entry| entry.1 != 0);
        right.retain(|entry| entry.1 != 0);
        let mut left_terms = vector(1, budget)?;
        left_terms.push(Term {
            powers: left,
            coefficient: engine
                .copy(&first.coefficient, budget)
                .map_err(|e| fault(&e))?,
        });
        let mut right_terms = vector(1, budget)?;
        right_terms.push(Term {
            powers: right,
            coefficient: engine
                .copy(&second.coefficient, budget)
                .map_err(|e| fault(&e))?,
        });
        let first_reduced = Form { terms: left_terms };
        let second_reduced = Form { terms: right_terms };
        let left = self
            .numerator
            .mul(&second_reduced, engine, budget)
            .map_err(|e| fault(&e))?;
        let right = other
            .numerator
            .mul(&first_reduced, engine, budget)
            .map_err(|e| fault(&e))?;
        Ok(Some(Self {
            maps: self.combined_maps(other, budget)?,
            numerator: left.add(&right, engine, budget).map_err(|e| fault(&e))?,
            denominator: self
                .denominator
                .mul(&second_reduced, engine, budget)
                .map_err(|e| fault(&e))?,
        }))
    }

    /// Exact subtraction, with no semantic cancellation of holes.
    pub(crate) fn sub(
        &self,
        other: &Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        self.add(&other.neg(engine, budget)?, engine, budget)
    }

    /// Multiplication retains the product of the input denominators.
    pub(crate) fn mul(
        &self,
        other: &Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        Ok(Self {
            maps: self.combined_maps(other, budget)?,
            numerator: self
                .numerator
                .mul(&other.numerator, engine, budget)
                .map_err(|e| fault(&e))?,
            denominator: self
                .denominator
                .mul(&other.denominator, engine, budget)
                .map_err(|e| fault(&e))?,
        })
    }

    /// Division is used only after the caller has checked the divisor's nonvanishing.
    pub(crate) fn div(
        &self,
        other: &Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        Ok(Self {
            maps: self.combined_maps(other, budget)?,
            numerator: self
                .numerator
                .mul(&other.denominator, engine, budget)
                .map_err(|e| fault(&e))?,
            denominator: self
                .denominator
                .mul(&other.numerator, engine, budget)
                .map_err(|e| fault(&e))?,
        })
    }

    /// Positive powers retain all denominator factors, including those of zero numerators.
    pub(crate) fn pow(
        &self,
        exponent: u32,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        Ok(Self {
            maps: crate::solving::coefficients::storage::copied(&self.maps, budget)?,
            numerator: self
                .numerator
                .pow(exponent, engine, budget)
                .map_err(|e| fault(&e))?,
            denominator: self
                .denominator
                .pow(exponent, engine, budget)
                .map_err(|e| fault(&e))?,
        })
    }

    /// Equality of already defined rational expressions by exact cross multiplication.
    pub(crate) fn equals(
        &self,
        other: &Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<bool, Fault> {
        Ok(self.sub(other, engine, budget)?.numerator.terms.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::{Engine, Value, Variable};
    use crate::testing::common::TestResult;
    use crate::{
        budget::{Budget, BudgetKind},
        limits::Limits,
    };

    /// Sharing denominator powers keeps degree 41 where a cross-product would require 81.
    /// ´test:unit:parameter-shared-denominator-degree´
    #[test]
    fn parameter_shared_denominator_degree() -> TestResult {
        let (fixture, _) = crate::testing::solve_blind::symbolic(&["p"])?;
        let engine = Engine {
            state: &fixture.cas.state,
            domain: &fixture.algebra,
        };
        let mut setup = Budget::new(Limits::blind_cubic())?;
        let x = Value::variable(Variable::Slot(0), &engine, &mut setup)?;
        let one = Value::integer(1, &engine, &mut setup)?;
        let left = one.div(&x.pow(40, &engine, &mut setup)?, &engine, &mut setup)?;
        let right = one.div(&x.pow(41, &engine, &mut setup)?, &engine, &mut setup)?;
        let error = left
            .denominator
            .mul(
                &right.denominator,
                &engine,
                &mut Budget::new(Limits::blind_cubic())?,
            )
            .expect_err("unshared degree");
        assert!(matches!(
            error,
            crate::solving::coefficients::model::CoefficientFailure::Fault(crate::Fault {
                kind: crate::FaultKind::Exhausted {
                    budget: BudgetKind::PolynomialDegree,
                    limit: 64,
                    needed: 81,
                    ..
                },
                ..
            })
        ));
        let result = left.add(&right, &engine, &mut Budget::new(Limits::blind_cubic())?)?;
        assert_eq!(result.numerator.terms.len(), 2);
        assert_eq!(result.denominator.terms.len(), 1);
        assert_eq!(
            result.denominator.terms[0].powers,
            [(Variable::Slot(0), 41)]
        );
        let coefficient = engine.constant(true, &mut setup)?;
        for (term, powers) in result
            .numerator
            .terms
            .iter()
            .zip([vec![(Variable::Slot(0), 1)], vec![]])
        {
            assert_eq!(term.powers, powers);
            assert!(engine.formal_equal(&term.coefficient, &coefficient, &mut setup)?);
        }
        assert!(engine.formal_equal(
            &result.denominator.terms[0].coefficient,
            &coefficient,
            &mut setup
        )?);
        let mut bounded = Budget::new(Limits {
            polynomial_degree: 40,
            ..Limits::blind_cubic()
        })?;
        assert!(left.add(&right, &engine, &mut bounded).is_err());
        Ok(())
    }

    /// Exact cancellation of the sum keeps every denominator variable and both input map obligations.
    /// ´test:unit:parameter-shared-denominator-holes´
    #[test]
    fn parameter_shared_denominator_holes() -> TestResult {
        let (mut fixture, _) = crate::testing::solve_blind::symbolic(&["p", "q"])?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let construction = crate::solving::radicals::construction::Construction {
            header: fixture.header.clone(),
            context: fixture.context.clone(),
            region: fixture.region.clone(),
            steps: Vec::new(),
        };
        let images = crate::solving::solve_search::coefficient_images(
            &mut fixture.cas,
            &construction,
            &fixture.algebra,
            &mut budget,
        )?;
        let map = construction.mapping(&mut fixture.cas, &fixture.algebra, &images, &mut budget)?;
        let engine = Engine {
            state: &fixture.cas.state,
            domain: &fixture.algebra,
        };
        let (_, _, base) = engine.fraction_shape()?;
        // Own the base handle before admitting another graph map.
        let base = base.clone();
        let ground_map = construction.mapping(&mut fixture.cas, &base, &[], &mut budget)?;
        assert_ne!(map, ground_map);
        // Renew the immutable engine view after the graph mutation above.
        let engine = Engine {
            state: &fixture.cas.state,
            domain: &fixture.algebra,
        };
        let x = Value::variable(Variable::Slot(0), &engine, &mut budget)?;
        let y = Value::variable(Variable::Slot(1), &engine, &mut budget)?;
        let denominator = x.mul(&y, &engine, &mut budget)?;
        let mut left =
            Value::integer(1, &engine, &mut budget)?.div(&denominator, &engine, &mut budget)?;
        let mut right = left.neg(&engine, &mut budget)?;
        left.maps.push(map.logical_id());
        right.maps.push(ground_map.logical_id());
        let result = left.add(&right, &engine, &mut budget)?;
        assert_eq!(
            result
                .numerator
                .terms
                .iter()
                .map(|term| term.powers.as_slice())
                .collect::<Vec<_>>(),
            [] as [&[(Variable, u32)]; 0]
        );
        assert_eq!(result.maps, [map.logical_id(), ground_map.logical_id()]);
        assert_eq!(result.denominator.terms.len(), 1);
        assert_eq!(
            result.denominator.terms[0].powers,
            [(Variable::Slot(0), 1), (Variable::Slot(1), 1)]
        );
        let evaluation = crate::solving::elementary::evaluate::Evaluation {
            exclusions: &[],
            cas: &fixture.cas,
            engine,
            region: &fixture.region,
            premises: &[],
            free: &[],
            bound: &[],
        };
        let divisor = Value::polynomial(result.denominator, &engine, &mut budget)?;
        assert!(evaluation.nonzero(&divisor, &mut budget).is_err());
        let general = x.add(
            &Value::integer(1, &engine, &mut budget)?,
            &engine,
            &mut budget,
        )?;
        let other = Value::integer(1, &engine, &mut budget)?.div(&general, &engine, &mut budget)?;
        assert!(
            left.shared_monomial_sum(&other, &engine, &mut budget)?
                .is_none()
        );
        let expected = left
            .denominator
            .mul(&other.denominator, &engine, &mut budget)?;
        let actual = left.add(&other, &engine, &mut budget)?;
        let difference =
            actual
                .denominator
                .add(&expected.neg(&engine, &mut budget)?, &engine, &mut budget)?;
        assert_eq!(
            difference
                .terms
                .iter()
                .map(|term| term.powers.as_slice())
                .collect::<Vec<_>>(),
            [] as [&[(Variable, u32)]; 0]
        );
        Ok(())
    }
}
