// SPDX-License-Identifier: AGPL-3.0-only
//! Metered descending pseudo-reduction by proved zero polynomials.

use super::proof::{AT, invalid};
use crate::budget::Budget;
use crate::error::Fault;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::forms::{Form, Term};
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::vector;

/// The exact common scalar applied to terms retained outside the active dividend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Scale {
    /// Field cancellation leaves the dividend unscaled.
    Unit,
    /// Pseudo-reduction multiplies the dividend by the divisor's leading coefficient.
    Leading,
}

/// Remove one leading multiple, using only a manifestly nonzero ground leading coefficient.
pub(crate) fn reduce(
    dividend: &mut Form,
    divisor: &Form,
    engine: &Engine<'_>,
    budget: &mut Budget,
) -> Result<Option<Scale>, Fault> {
    budget.work(1, AT)?;
    let Some(leading) = dividend.terms.first() else {
        return Ok(None);
    };
    let Some(factor) = divisor.terms.first() else {
        return Ok(None);
    };
    if !super::evidence::ground_unit(&factor.coefficient.value, budget)? {
        return Ok(None);
    }
    let mut powers = crate::solving::coefficients::storage::copied(&leading.powers, budget)?;
    for (variable, exponent) in &factor.powers {
        budget.work(crate::graph::size(powers.len(), AT)?, AT)?;
        let Ok(index) = powers.binary_search_by_key(variable, |entry| entry.0) else {
            return Ok(None);
        };
        let power = powers.get_mut(index).ok_or_else(invalid)?;
        let Some(remainder) = power.1.checked_sub(*exponent) else {
            return Ok(None);
        };
        power.1 = remainder;
    }
    powers.retain(|(_, exponent)| *exponent > 0);
    if let Some(reduced) = gaussian(dividend, divisor, &powers, engine, budget)? {
        *dividend = reduced;
        return Ok(Some(Scale::Unit));
    }
    if constant_fraction(dividend, divisor, &powers, engine, budget)? {
        return Ok(Some(Scale::Unit));
    }
    *dividend = pseudo(dividend, divisor, powers, engine, budget)?;
    Ok(Some(Scale::Leading))
}

/// Preserve the original pseudo-reduction arithmetic for every remaining coefficient path.
fn pseudo(
    dividend: &Form,
    divisor: &Form,
    powers: Vec<(crate::solving::coefficients::forms::Variable, u32)>,
    engine: &Engine<'_>,
    budget: &mut Budget,
) -> Result<Form, Fault> {
    let leading = dividend.terms.first().ok_or_else(invalid)?;
    let factor = divisor.terms.first().ok_or_else(invalid)?;
    let mut terms = vector(1, budget)?;
    terms.push(Term {
        powers,
        coefficient: engine
            .copy(&leading.coefficient, budget)
            .map_err(|e| fault(&e))?,
    });
    let multiple = Form { terms }
        .mul(divisor, engine, budget)
        .map_err(|e| fault(&e))?;
    let scalar = Form::constant(
        engine
            .copy(&factor.coefficient, budget)
            .map_err(|e| fault(&e))?,
        engine,
        budget,
    )
    .map_err(|e| fault(&e))?;
    let scaled = dividend
        .mul(&scalar, engine, budget)
        .map_err(|e| fault(&e))?;
    scaled
        .add(
            &multiple.neg(engine, budget).map_err(|e| fault(&e))?,
            engine,
            budget,
        )
        .map_err(|e| fault(&e))
}

/// Exact Gaussian field division avoids repeatedly scaling all unrelated coefficients.
fn gaussian(
    dividend: &Form,
    divisor: &Form,
    powers: &[(crate::solving::coefficients::forms::Variable, u32)],
    engine: &Engine<'_>,
    budget: &mut Budget,
) -> Result<Option<Form>, Fault> {
    use crate::solving::coefficients::{model::Value, scalar::Scalar};
    let leading = dividend.terms.first().ok_or_else(invalid)?;
    let factor = divisor.terms.first().ok_or_else(invalid)?;
    let (Value::Scalar(Scalar::Gaussian(left)), Value::Scalar(Scalar::Gaussian(right))) =
        (&leading.coefficient.value, &factor.coefficient.value)
    else {
        return Ok(None);
    };
    if right.is_zero(budget)? {
        return Ok(None);
    }
    let mut terms = vector(1, budget)?;
    terms.push(Term {
        powers: crate::solving::coefficients::storage::copied(powers, budget)?,
        coefficient: engine.element(Value::Scalar(Scalar::Gaussian(left.div(right, budget)?))),
    });
    let multiple = Form { terms }
        .mul(divisor, engine, budget)
        .map_err(|e| fault(&e))?;
    Ok(Some(
        dividend
            .add(
                &multiple.neg(engine, budget).map_err(|e| fault(&e))?,
                engine,
                budget,
            )
            .map_err(|e| fault(&e))?,
    ))
}

/// Inspect both polynomials before interpreting a fraction as a ground-field value.
fn ground_fraction(
    value: &crate::solving::coefficients::fraction::Fraction,
    budget: &mut Budget,
) -> Result<Option<crate::solving::coefficients::scalar::Scalar>, Fault> {
    let Some(numerator) = value.numerator.scalar(budget)? else {
        return Ok(None);
    };
    let Some(denominator) = value.denominator.scalar(budget)? else {
        return Ok(None);
    };
    if denominator.zero(budget)? {
        return Ok(None);
    }
    Ok(Some(numerator.div(&denominator, budget)?))
}

/// Ground constants retain their field cancellation inside the exact parameter algebra.
fn constant_fraction(
    dividend: &mut Form,
    divisor: &Form,
    powers: &[(crate::solving::coefficients::forms::Variable, u32)],
    engine: &Engine<'_>,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    use crate::solving::coefficients::{fraction::Fraction, model::Value, polynomial::Polynomial};
    let leading = dividend.terms.first().ok_or_else(invalid)?;
    let factor = divisor.terms.first().ok_or_else(invalid)?;
    let (Value::Fraction(left), Value::Fraction(right)) =
        (&leading.coefficient.value, &factor.coefficient.value)
    else {
        return Ok(false);
    };
    let Some(numerator) = ground_fraction(left, budget)? else {
        return Ok(false);
    };
    let Some(denominator) = ground_fraction(right, budget)? else {
        return Ok(false);
    };
    if denominator.zero(budget)? {
        return Ok(false);
    }
    let numerator = Polynomial::constant(
        left.numerator.variables,
        numerator.div(&denominator, budget)?,
        budget,
    )?;
    let unit = numerator.one(budget)?;
    let mut terms = vector(1, budget)?;
    terms.push(Term {
        powers: crate::solving::coefficients::storage::copied(powers, budget)?,
        coefficient: engine.element(Value::Fraction(Fraction {
            numerator,
            denominator: unit,
        })),
    });
    let multiple = Form { terms }
        .mul(divisor, engine, budget)
        .map_err(|e| fault(&e))?;
    dividend
        .add_owned(
            multiple.neg(engine, budget).map_err(|e| fault(&e))?,
            engine,
            budget,
        )
        .map_err(|e| fault(&e))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{Engine, Form, reduce};
    use crate::budget::{Budget, BudgetKind};
    use crate::number::Rational;
    use crate::solving::coefficients::forms::Variable;
    use crate::solving::coefficients::{
        fraction::Fraction,
        model::Value,
        polynomial::{Polynomial, Term},
        scalar::{Ground, Scalar},
    };

    /// Exact field cancellation retains a small remainder when pseudo-scaling exceeds 64 bits.
    /// ´test:unit:gaussian-field-reduction-bound´
    #[test]
    fn gaussian_field_reduction_bound() -> Result<(), Box<dyn std::error::Error>> {
        let mut cas = crate::Cas::new(crate::limits::Limits::blind_cubic())?;
        let mut budget = Budget::new(crate::limits::Limits::blind_cubic())?;
        let declaration =
            crate::syntax::Draft::Solve(crate::solving::SolveDraft::CoefficientDomain(
                crate::solving::CoefficientDomainData::Gaussian,
            ));
        let domain = cas
            .admit(std::slice::from_ref(&declaration), &mut budget)?
            .into_iter()
            .next()
            .ok_or("Gaussian domain")?;
        let engine = Engine {
            state: &cas.state,
            domain: &domain,
        };
        let small = Rational::from_ratio(1, 17_892_269_640_269_892_018, &mut budget)?;
        let coefficient = Rational::from_ratio(23, 2, &mut budget)?;
        let overflow = small
            .mul(&coefficient, &mut budget)
            .expect_err("pseudo-scaling exceeds component bound");
        assert!(matches!(
            overflow.kind,
            crate::FaultKind::Exhausted {
                budget: BudgetKind::ComponentBits,
                ..
            }
        ));
        let variable = Form::variable(Variable::Slot(0), &engine, &mut budget)?;
        let constant = Form::constant(engine.rational(&small, &mut budget)?, &engine, &mut budget)?;
        let dividend = variable.add(&constant, &engine, &mut budget)?;
        let scalar = Form::constant(
            engine.rational(&coefficient, &mut budget)?,
            &engine,
            &mut budget,
        )?;
        let mut divisor = variable.mul(&scalar, &engine, &mut budget)?;
        let mut remainder = dividend.copy(&engine, &mut budget)?;
        assert_eq!(
            reduce(&mut remainder, &divisor, &engine, &mut budget)?,
            Some(super::Scale::Unit)
        );
        let [actual] = remainder.terms.as_slice() else {
            return Err("field remainder extent".into());
        };
        assert_eq!(actual.powers, [] as [(Variable, u32); 0]);
        let [expected] = constant.terms.as_slice() else {
            return Err("field expected constant".into());
        };
        assert!(engine.formal_equal(&actual.coefficient, &expected.coefficient, &mut budget)?);
        assert_eq!(divisor.terms.len(), 1);
        divisor.terms.clear();
        assert_ne!(divisor.terms.len(), 1);
        assert!(reduce(&mut remainder, &divisor, &engine, &mut budget)?.is_none());
        Ok(())
    }
    /// Parameter-field ground constants cancel without an unrelated 65-bit pseudo-product.
    /// ´test:unit:constant-fraction-reduction-bound´
    #[test]
    fn constant_fraction_reduction_bound() -> Result<(), Box<dyn std::error::Error>> {
        let (fixture, _) = crate::testing::solve_blind::symbolic(&["p"])?;
        let mut budget = Budget::new(crate::limits::Limits::blind_cubic())?;
        let engine = Engine {
            state: &fixture.cas.state,
            domain: &fixture.algebra,
        };
        assert_eq!(engine.fraction_shape()?.1, 1);
        let small = Rational::from_ratio(50_031_545_098_999_707, 4096, &mut budget)?;
        let coefficient = Rational::from_ratio(729, 4, &mut budget)?;
        let overflow = small
            .mul(&coefficient, &mut budget)
            .expect_err("pseudo-scaling exceeds component bound");
        assert!(matches!(
            overflow.kind,
            crate::FaultKind::Exhausted {
                budget: BudgetKind::ComponentBits,
                ..
            }
        ));
        let variable = Form::variable(Variable::Slot(0), &engine, &mut budget)?;
        let constant = Form::constant(engine.rational(&small, &mut budget)?, &engine, &mut budget)?;
        let dividend = variable.add(&constant, &engine, &mut budget)?;
        let scalar = Form::constant(
            engine.rational(&coefficient, &mut budget)?,
            &engine,
            &mut budget,
        )?;
        let mut divisor = variable.mul(&scalar, &engine, &mut budget)?;
        let mut remainder = dividend.copy(&engine, &mut budget)?;
        assert_eq!(
            reduce(&mut remainder, &divisor, &engine, &mut budget)?,
            Some(super::Scale::Unit)
        );
        let [actual] = remainder.terms.as_slice() else {
            return Err("field remainder extent".into());
        };
        assert_eq!(actual.powers, [] as [(Variable, u32); 0]);
        let [expected] = constant.terms.as_slice() else {
            return Err("field expected constant".into());
        };
        assert!(engine.formal_equal(&actual.coefficient, &expected.coefficient, &mut budget)?);
        fraction_refusals(&dividend, &mut divisor, &coefficient, &engine, &mut budget)?;
        Ok(())
    }

    /// Refuse a symbolic denominator and a changed zero leading coefficient independently.
    fn fraction_refusals(
        dividend: &Form,
        divisor: &mut Form,
        coefficient: &Rational,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let [leading] = divisor.terms.as_mut_slice() else {
            return Err("field divisor extent".into());
        };
        let pinned = engine.rational(coefficient, budget)?;
        assert!(engine.formal_equal(&leading.coefficient, &pinned, budget)?);
        let zero = engine.constant(false, budget)?;
        assert!(!engine.formal_equal(&leading.coefficient, &zero, budget)?);
        let parameter = Polynomial::new(
            1,
            Ground::Rational,
            vec![Term {
                powers: vec![1],
                coefficient: Scalar::Rational(Rational::one()),
            }],
            budget,
        )?;
        let one = parameter.one(budget)?;
        let symbolic = engine.element(Value::Fraction(Fraction::new(&one, &parameter, budget)?));
        assert!(!engine.formal_equal(&leading.coefficient, &symbolic, budget)?);
        leading.coefficient = engine.copy(&symbolic, budget)?;
        let mut remainder = dividend.copy(engine, budget)?;
        assert!(!super::constant_fraction(
            &mut remainder,
            divisor,
            &[],
            engine,
            budget
        )?);
        let [leading] = divisor.terms.as_mut_slice() else {
            return Err("field divisor extent".into());
        };
        assert!(engine.formal_equal(&leading.coefficient, &symbolic, budget)?);
        assert!(!engine.formal_equal(&leading.coefficient, &zero, budget)?);
        leading.coefficient = zero;
        assert!(reduce(&mut remainder, divisor, engine, budget)?.is_none());
        Ok(())
    }
}
