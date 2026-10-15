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

    /// Addition reserves both cross products and their sum before gcd normalization.
    pub(crate) fn add(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        let left = self.numerator.mul(&other.denominator, budget)?;
        let right = other.numerator.mul(&self.denominator, budget)?;
        Self::new(
            &left.add(&right, budget)?,
            &self.denominator.mul(&other.denominator, budget)?,
            budget,
        )
    }

    /// Canonical sign change preserves the denominator.
    pub(crate) fn neg(&self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        Ok(Self {
            numerator: self.numerator.neg(budget)?,
            denominator: self.denominator.copy(budget)?,
        })
    }

    /// Multiplication normalizes after accounting for both full polynomial products.
    pub(crate) fn mul(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        Self::new(
            &self.numerator.mul(&other.numerator, budget)?,
            &self.denominator.mul(&other.denominator, budget)?,
            budget,
        )
    }

    /// Formal reciprocal for use only after the caller has separately proved a regional unit.
    pub(crate) fn reciprocal(&self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        Self::new(&self.denominator, &self.numerator, budget)
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
