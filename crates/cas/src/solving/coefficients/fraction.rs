// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical rational functions and exact finite evaluations.

use super::gcd::{exact, gcd};
use super::polynomial::Polynomial;
use super::scalar::Scalar;
use super::storage::{begin, invalid};
use crate::budget::Budget;
use crate::error::Fault;

/// A coprime numerator/denominator pair with monic denominator.
#[derive(Debug)]
pub(crate) struct Fraction {
    /// Canonical numerator in the fixed variable order.
    pub(crate) numerator: Polynomial,
    /// Formally nonzero monic denominator; regional definedness is separate.
    pub(crate) denominator: Polynomial,
}

impl Fraction {
    /// Remove recursive polynomial content and common primitive factors, then make the denominator monic.
    pub(crate) fn new(
        numerator: &Polynomial,
        denominator: &Polynomial,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        begin(budget)?;
        if denominator.zero(budget)? {
            return Err(invalid());
        }
        if numerator.variables != denominator.variables || numerator.ground != denominator.ground {
            return Err(invalid());
        }
        if numerator.zero(budget)? {
            return Ok(Self {
                numerator: numerator.copy(budget)?,
                denominator: numerator.one(budget)?,
            });
        }
        if let Some(scalar) = denominator.scalar(budget)? {
            return Ok(Self {
                numerator: numerator.scale_down(&scalar, budget)?,
                denominator: denominator.one(budget)?,
            });
        }
        let common = gcd(numerator, denominator, budget)?;
        let reduced_numerator = exact(numerator, &common, budget)?.ok_or_else(invalid)?;
        let reduced_denominator = exact(denominator, &common, budget)?.ok_or_else(invalid)?;
        let leading = &reduced_denominator
            .terms
            .first()
            .ok_or_else(invalid)?
            .coefficient;
        Ok(Self {
            numerator: reduced_numerator.scale_down(leading, budget)?,
            denominator: reduced_denominator.scale_down(leading, budget)?,
        })
    }

    /// Copy a canonical value without silently normalizing an untrusted input.
    pub(crate) fn copy(&self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        Ok(Self {
            numerator: self.numerator.copy(budget)?,
            denominator: self.denominator.copy(budget)?,
        })
    }

    /// Canonical operands must belong to the same ordered parameter algebra.
    fn compatible(&self, other: &Self) -> Result<(), Fault> {
        if self.numerator.variables != other.numerator.variables
            || self.numerator.ground != other.numerator.ground
        {
            return Err(invalid());
        }
        Ok(())
    }

    /// Canonical sums cancel only factors in the denominator gcd, before forming the final denominator.
    pub(crate) fn add(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        self.compatible(other)?;
        if self.numerator.zero(budget)? {
            return other.copy(budget);
        }
        if other.numerator.zero(budget)? {
            return self.copy(budget);
        }
        if self.denominator.equals(&other.denominator, budget)? {
            return Self::new(
                &self.numerator.add(&other.numerator, budget)?,
                &self.denominator,
                budget,
            );
        }
        if self.unit_denominator(budget)? {
            return self.polynomial_sum(other, budget);
        }
        if other.unit_denominator(budget)? {
            return other.polynomial_sum(self, budget);
        }
        let common = gcd(&self.denominator, &other.denominator, budget)?;
        let first = exact(&self.denominator, &common, budget)?.ok_or_else(invalid)?;
        let second = exact(&other.denominator, &common, budget)?.ok_or_else(invalid)?;
        let sum = self
            .numerator
            .mul(&second, budget)?
            .add(&other.numerator.mul(&first, budget)?, budget)?;
        if sum.zero(budget)? {
            return Ok(Self {
                denominator: sum.one(budget)?,
                numerator: sum,
            });
        }
        let (numerator, retained) = Self::cross_pair(&sum, &common, budget)?;
        Ok(Self {
            numerator,
            denominator: first.mul(&second, budget)?.mul(&retained, budget)?,
        })
    }

    /// Adding a polynomial cannot introduce a common factor with the other canonical denominator.
    fn polynomial_sum(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        Ok(Self {
            numerator: self
                .numerator
                .mul(&other.denominator, budget)?
                .add(&other.numerator, budget)?,
            denominator: other.denominator.copy(budget)?,
        })
    }

    /// Canonical sign change preserves the denominator.
    pub(crate) fn neg(&self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        Ok(Self {
            numerator: self.numerator.neg(budget)?,
            denominator: self.denominator.copy(budget)?,
        })
    }

    /// A polynomial denominator is reused only after exact comparison with the ground unit.
    fn unit_denominator(&self, budget: &mut Budget) -> Result<bool, Fault> {
        let Some(value) = self.denominator.scalar(budget)? else {
            return Ok(false);
        };
        value.equals(&Scalar::constant(value.ground(), true, budget)?, budget)
    }

    /// Cancel one cross pair before products grow; only exact polynomial factors are removed.
    fn cross_pair(
        numerator: &Polynomial,
        denominator: &Polynomial,
        budget: &mut Budget,
    ) -> Result<(Polynomial, Polynomial), Fault> {
        if numerator.equals(denominator, budget)? {
            return Ok((numerator.one(budget)?, denominator.one(budget)?));
        }
        if let Some(scalar) = denominator.scalar(budget)?
            && scalar.equals(&Scalar::constant(scalar.ground(), true, budget)?, budget)?
        {
            return Ok((numerator.copy(budget)?, denominator.copy(budget)?));
        }
        let common = gcd(numerator, denominator, budget)?;
        Ok((
            exact(numerator, &common, budget)?.ok_or_else(invalid)?,
            exact(denominator, &common, budget)?.ok_or_else(invalid)?,
        ))
    }

    /// Canonical operands need only cross cancellation; monic denominator quotients preserve the canonical result.
    pub(crate) fn mul(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        self.compatible(other)?;
        if self.numerator.zero(budget)? {
            return self.copy(budget);
        }
        if other.numerator.zero(budget)? {
            return other.copy(budget);
        }
        if self.numerator.equals(&self.denominator, budget)? {
            return other.copy(budget);
        }
        if other.numerator.equals(&other.denominator, budget)? {
            return self.copy(budget);
        }
        if self.unit_denominator(budget)? && other.unit_denominator(budget)? {
            return Ok(Self {
                numerator: self.numerator.mul(&other.numerator, budget)?,
                denominator: self.denominator.copy(budget)?,
            });
        }
        let (first, second_denominator) =
            Self::cross_pair(&self.numerator, &other.denominator, budget)?;
        let (second, first_denominator) =
            Self::cross_pair(&other.numerator, &self.denominator, budget)?;
        Ok(Self {
            numerator: first.mul(&second, budget)?,
            denominator: first_denominator.mul(&second_denominator, budget)?,
        })
    }

    /// Coprimality survives swapping; only monic rescaling remains after the caller proves a regional unit.
    pub(crate) fn reciprocal(&self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        let leading = &self
            .numerator
            .terms
            .first()
            .ok_or_else(invalid)?
            .coefficient;
        Ok(Self {
            numerator: self.denominator.scale_down(leading, budget)?,
            denominator: self.numerator.scale_down(leading, budget)?,
        })
    }

    /// An exact finite evaluation rejects a zero specialized denominator.
    pub(crate) fn evaluate(
        &self,
        images: &[Scalar],
        budget: &mut Budget,
    ) -> Result<Option<Scalar>, Fault> {
        begin(budget)?;
        let denominator = evaluate(&self.denominator, images, budget)?;
        if denominator.zero(budget)? {
            return Ok(None);
        }
        Ok(Some(
            evaluate(&self.numerator, images, budget)?.div(&denominator, budget)?,
        ))
    }
}

/// Evaluate each formal monomial independently; no injectivity is inferred from evaluation.
pub(crate) fn evaluate(
    value: &Polynomial,
    images: &[Scalar],
    budget: &mut Budget,
) -> Result<Scalar, Fault> {
    begin(budget)?;
    budget.work(
        crate::graph::size(images.len(), super::storage::AT)?,
        super::storage::AT,
    )?;
    let ground = images.first().map_or(value.ground, Scalar::ground);
    if value.variables != images.len() || images.iter().any(|image| image.ground() != ground) {
        return Err(invalid());
    }
    let mut result = Scalar::constant(ground, false, budget)?;
    for term in &value.terms {
        let mut product = super::maps::ground_image(&term.coefficient, ground, budget)?;
        for (image, exponent) in images.iter().zip(&term.powers) {
            product = product.mul(&image.pow(*exponent, budget)?, budget)?;
        }
        result = result.add(&product, budget)?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{Fraction, Polynomial, Scalar};
    use crate::solving::coefficients::{polynomial::Term, scalar::Ground};
    use crate::{Fault, Rational, budget::Budget, limits::Limits};

    fn polynomial(terms: &[(u32, i64)], budget: &mut Budget) -> Result<Polynomial, Fault> {
        let mut entries = Vec::new();
        for (power, value) in terms {
            entries.push(Term {
                powers: vec![*power],
                coefficient: Scalar::Rational(Rational::from_ratio(*value, 1, budget)?),
            });
        }
        Polynomial::new(1, Ground::Rational, entries, budget)
    }

    /// Ground units and zero have independently specified canonical numerator and denominator pairs.
    /// ´test:unit:fraction-ground-denominator´
    #[test]
    fn fraction_ground_denominator() -> Result<(), Fault> {
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let numerator = polynomial(&[(1, 2), (0, 2)], &mut budget)?;
        let denominator = polynomial(&[(0, 2)], &mut budget)?;
        let result = Fraction::new(&numerator, &denominator, &mut budget)?;
        assert!(
            result
                .numerator
                .equals(&polynomial(&[(1, 1), (0, 1)], &mut budget)?, &mut budget)?
        );
        assert!(
            result
                .denominator
                .equals(&polynomial(&[(0, 1)], &mut budget)?, &mut budget)?
        );
        let zero = polynomial(&[], &mut budget)?;
        assert!(Fraction::new(&numerator, &zero, &mut budget).is_err());
        let result = Fraction::new(&zero, &numerator, &mut budget)?;
        assert_eq!(
            result
                .numerator
                .terms
                .iter()
                .map(|term| &term.powers)
                .collect::<Vec<_>>(),
            [] as [&Vec<u32>; 0]
        );
        assert!(
            result
                .denominator
                .equals(&polynomial(&[(0, 1)], &mut budget)?, &mut budget)?
        );
        let alien = Polynomial::constant(
            2,
            Scalar::constant(Ground::Rational, true, &mut budget)?,
            &mut budget,
        )?;
        assert!(Fraction::new(&zero, &alien, &mut budget).is_err());
        Ok(())
    }

    /// Reusing a canonical operand preserves its nonconstant denominator and performs no new gcd.
    /// ´test:unit:fraction-identity-reuses-canonical-pair´
    #[test]
    fn fraction_identity_reuses_canonical_pair() -> Result<(), Fault> {
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let numerator = polynomial(&[(1, 1), (0, 2)], &mut budget)?;
        let denominator = polynomial(&[(1, 1), (0, -3)], &mut budget)?;
        let value = Fraction::new(&numerator, &denominator, &mut budget)?;
        let unity = polynomial(&[(0, 1)], &mut budget)?;
        let zero = Fraction::new(&polynomial(&[], &mut budget)?, &unity, &mut budget)?;
        let one = Fraction::new(&unity, &unity, &mut budget)?;
        let before = budget.usage().gcd_steps;
        for result in [
            value.add(&zero, &mut budget)?,
            zero.add(&value, &mut budget)?,
            value.mul(&one, &mut budget)?,
            one.mul(&value, &mut budget)?,
        ] {
            assert!(result.numerator.equals(&numerator, &mut budget)?);
            assert!(result.denominator.equals(&denominator, &mut budget)?);
        }
        for result in [
            value.mul(&zero, &mut budget)?,
            zero.mul(&value, &mut budget)?,
        ] {
            assert_eq!(
                result
                    .numerator
                    .terms
                    .iter()
                    .map(|term| &term.powers)
                    .collect::<Vec<_>>(),
                [] as [&Vec<u32>; 0]
            );
            assert!(result.denominator.equals(&unity, &mut budget)?);
        }
        assert_eq!(budget.usage().gcd_steps, before);
        Ok(())
    }
    /// Adding over the same nonconstant denominator still cancels its resulting common factor exactly.
    /// ´test:unit:fraction-shared-denominator´
    #[test]
    fn fraction_shared_denominator() -> Result<(), Fault> {
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let one = polynomial(&[(0, 1)], &mut budget)?;
        let denominator = polynomial(&[(1, 1), (0, -3)], &mut budget)?;
        let left = Fraction::new(&one, &denominator, &mut budget)?;
        let right = Fraction::new(
            &polynomial(&[(1, 1), (0, -4)], &mut budget)?,
            &denominator,
            &mut budget,
        )?;
        let sum = left.add(&right, &mut budget)?;
        assert!(sum.numerator.equals(&one, &mut budget)?);
        assert!(sum.denominator.equals(&one, &mut budget)?);
        Ok(())
    }

    /// Unit denominators need no new gcd; the independent product retains both its nonzero coefficients.
    /// ´test:unit:fraction-polynomial-product´
    #[test]
    fn fraction_polynomial_product() -> Result<(), Fault> {
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let one = polynomial(&[(0, 1)], &mut budget)?;
        let left = Fraction::new(
            &polynomial(&[(1, 1), (0, 1)], &mut budget)?,
            &one,
            &mut budget,
        )?;
        let right = Fraction::new(
            &polynomial(&[(1, 1), (0, -1)], &mut budget)?,
            &one,
            &mut budget,
        )?;
        let before = budget.usage().gcd_steps;
        let result = left.mul(&right, &mut budget)?;
        assert_eq!(budget.usage().gcd_steps, before);
        assert!(
            result
                .numerator
                .equals(&polynomial(&[(2, 1), (0, -1)], &mut budget)?, &mut budget)?
        );
        assert!(result.denominator.equals(&one, &mut budget)?);
        let fractional = Fraction::new(
            &one,
            &polynomial(&[(1, 1), (0, -1)], &mut budget)?,
            &mut budget,
        )?;
        let inverse_product = fractional.mul(&right, &mut budget)?;
        assert!(inverse_product.numerator.equals(&one, &mut budget)?);
        assert!(inverse_product.denominator.equals(&one, &mut budget)?);
        Ok(())
    }

    /// Cross cancellation avoids an unnecessary degree-80 product; a genuine degree-80 result still refuses.
    /// ´test:unit:fraction-cross-product-bound´
    #[test]
    fn fraction_cross_product_bound() -> Result<(), Fault> {
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let numerator = polynomial(&[(40, 1), (0, 1)], &mut budget)?;
        let denominator = polynomial(&[(40, 1), (0, -1)], &mut budget)?;
        let left = Fraction::new(&numerator, &denominator, &mut budget)?;
        let right = Fraction::new(&denominator, &numerator, &mut budget)?;
        let result = left.mul(&right, &mut budget)?;
        let one = polynomial(&[(0, 1)], &mut budget)?;
        assert!(result.numerator.equals(&one, &mut budget)?);
        assert!(result.denominator.equals(&one, &mut budget)?);
        let mut bounded = Budget::new(Limits::blind_cubic())?;
        assert!(left.mul(&left, &mut bounded).is_err());
        let first = Fraction::new(
            &polynomial(&[(1, 1), (0, 1)], &mut budget)?,
            &polynomial(&[(1, 1), (0, -1)], &mut budget)?,
            &mut budget,
        )?;
        let second = Fraction::new(
            &polynomial(&[(1, 1), (0, -1)], &mut budget)?,
            &polynomial(&[(1, 1), (0, 2)], &mut budget)?,
            &mut budget,
        )?;
        let result = first.mul(&second, &mut budget)?;
        assert!(
            result
                .numerator
                .equals(&polynomial(&[(1, 1), (0, 1)], &mut budget)?, &mut budget)?
        );
        assert!(
            result
                .denominator
                .equals(&polynomial(&[(1, 1), (0, 2)], &mut budget)?, &mut budget)?
        );
        Ok(())
    }

    /// Shared factors are retained once and canceled only when the independent sum requires it.
    /// ´test:unit:fraction-shared-factor-sum-bound´
    #[test]
    fn fraction_shared_factor_sum_bound() -> Result<(), Fault> {
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let one = polynomial(&[(0, 1)], &mut budget)?;
        let first = Fraction::new(&one, &polynomial(&[(40, 1)], &mut budget)?, &mut budget)?;
        let second = Fraction::new(
            &polynomial(&[(20, 1), (0, -1)], &mut budget)?,
            &polynomial(&[(60, 1)], &mut budget)?,
            &mut budget,
        )?;
        let sum = first.add(&second, &mut budget)?;
        assert!(
            sum.numerator
                .equals(&polynomial(&[(20, 2), (0, -1)], &mut budget)?, &mut budget)?
        );
        assert!(
            sum.denominator
                .equals(&polynomial(&[(60, 1)], &mut budget)?, &mut budget)?
        );
        let first = Fraction::new(
            &one,
            &polynomial(&[(2, 1), (1, 1)], &mut budget)?,
            &mut budget,
        )?;
        let second = Fraction::new(
            &one,
            &polynomial(&[(2, 1), (1, -1)], &mut budget)?,
            &mut budget,
        )?;
        let sum = first.add(&second, &mut budget)?;
        assert!(
            sum.numerator
                .equals(&polynomial(&[(0, 2)], &mut budget)?, &mut budget)?
        );
        assert!(
            sum.denominator
                .equals(&polynomial(&[(2, 1), (0, -1)], &mut budget)?, &mut budget)?
        );
        let first = Fraction::new(
            &one,
            &polynomial(&[(40, 1), (0, 1)], &mut budget)?,
            &mut budget,
        )?;
        let second = Fraction::new(
            &one,
            &polynomial(&[(40, 1), (0, -1)], &mut budget)?,
            &mut budget,
        )?;
        assert!(
            first
                .add(&second, &mut Budget::new(Limits::blind_cubic())?)
                .is_err()
        );
        Ok(())
    }

    /// Polynomial addition retains the exact nonconstant denominator in either operand order.
    /// ´test:unit:fraction-polynomial-sum´
    #[test]
    fn fraction_polynomial_sum() -> Result<(), Fault> {
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let one = polynomial(&[(0, 1)], &mut budget)?;
        let denominator = polynomial(&[(1, 1), (0, 1)], &mut budget)?;
        let rational = Fraction::new(&one, &denominator, &mut budget)?;
        let integer = Fraction::new(&polynomial(&[(1, 1)], &mut budget)?, &one, &mut budget)?;
        for sum in [
            rational.add(&integer, &mut budget)?,
            integer.add(&rational, &mut budget)?,
        ] {
            assert!(sum.numerator.equals(
                &polynomial(&[(2, 1), (1, 1), (0, 1)], &mut budget)?,
                &mut budget
            )?);
            assert!(sum.denominator.equals(&denominator, &mut budget)?);
        }
        Ok(())
    }

    /// A reciprocal swaps the coprime pair and applies its independently expected monic scaling; zero refuses.
    /// ´test:unit:fraction-canonical-reciprocal´
    #[test]
    fn fraction_canonical_reciprocal() -> Result<(), Fault> {
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let numerator = polynomial(&[(1, 2), (0, 4)], &mut budget)?;
        let denominator = polynomial(&[(1, 1), (0, -3)], &mut budget)?;
        let original = Fraction::new(&numerator, &denominator, &mut budget)?;
        let expected_numerator = Polynomial::new(
            1,
            Ground::Rational,
            vec![
                Term {
                    powers: vec![1],
                    coefficient: Scalar::Rational(Rational::from_ratio(1, 2, &mut budget)?),
                },
                Term {
                    powers: vec![0],
                    coefficient: Scalar::Rational(Rational::from_ratio(-3, 2, &mut budget)?),
                },
            ],
            &mut budget,
        )?;
        let result = original.reciprocal(&mut budget)?;
        assert!(result.numerator.equals(&expected_numerator, &mut budget)?);
        assert!(
            result
                .denominator
                .equals(&polynomial(&[(1, 1), (0, 2)], &mut budget)?, &mut budget)?
        );
        let repeated = result.reciprocal(&mut budget)?;
        assert!(repeated.numerator.equals(&numerator, &mut budget)?);
        assert!(repeated.denominator.equals(&denominator, &mut budget)?);
        let zero = Fraction::new(&polynomial(&[], &mut budget)?, &denominator, &mut budget)?;
        assert!(zero.reciprocal(&mut budget).is_err());
        Ok(())
    }
}
