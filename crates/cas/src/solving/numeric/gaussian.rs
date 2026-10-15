// SPDX-License-Identifier: AGPL-3.0-only
//! Componentwise field operations; `Complex` is storage only.

use num_complex::Complex;

use super::Gaussian;
use super::plan::{Plan, Scalar, Size, cost};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::number::Rational;

/// Count an attempted numeric coefficient operation before inspecting its components.
fn begin(budget: &mut Budget, at: Location) -> Result<(), Fault> {
    budget.solving(BudgetKind::CoefficientOps, 1, at)
}

/// A clone after the complete operation has reserved its bounds and retained bytes.
fn copy(value: &Rational, budget: &mut Budget, at: Location) -> Result<Rational, Fault> {
    let size = Size::of(value, budget, at)?;
    budget.arithmetic(cost(size.bits(), at)?, at)?;
    Ok(value.clone())
}

/// Plan the sum of two squares, propagating both product bounds into the addition.
fn norm_plan(plan: &mut Plan, parts: [Size; 2], at: Location) -> Result<Size, Fault> {
    let [real, imaginary] = parts;
    let real_square = plan.scalar(Scalar::Mul, real, real, at)?;
    let imaginary_square = plan.scalar(Scalar::Mul, imaginary, imaginary, at)?;
    plan.scalar(Scalar::Add, real_square, imaginary_square, at)
}

/// Plan all four products and both sums of the field product.
fn product_plan(
    plan: &mut Plan,
    left: [Size; 2],
    right: [Size; 2],
    at: Location,
) -> Result<[Size; 2], Fault> {
    let [a, b] = left;
    let [c, d] = right;
    let diagonal = plan.scalar(Scalar::Mul, a, c, at)?;
    let off_diagonal = plan.scalar(Scalar::Mul, b, d, at)?;
    let forward = plan.scalar(Scalar::Mul, a, d, at)?;
    let backward = plan.scalar(Scalar::Mul, b, c, at)?;
    let real = plan.scalar(Scalar::Sub, diagonal, off_diagonal, at)?;
    let imaginary = plan.scalar(Scalar::Add, forward, backward, at)?;
    Ok([real, imaginary])
}

/// Plan the norm, both divisions and the imaginary negation of a nonzero inverse.
fn inverse_plan(plan: &mut Plan, parts: [Size; 2], at: Location) -> Result<[Size; 2], Fault> {
    let norm = norm_plan(plan, parts, at)?;
    let [real, imaginary] = parts;
    let quotient_real = plan.scalar(Scalar::Div, real, norm, at)?;
    let quotient_imaginary = plan.scalar(Scalar::Div, imaginary, norm, at)?;
    plan.copy(quotient_imaginary, at)?;
    Ok([quotient_real, quotient_imaginary])
}

/// Execute a prepaid norm without calling a `Complex` operator.
fn norm_value(value: &Gaussian, budget: &mut Budget) -> Result<Rational, Fault> {
    let real = value.value.re.mul(&value.value.re, budget)?;
    let imaginary = value.value.im.mul(&value.value.im, budget)?;
    real.add(&imaginary, budget)
}

/// Execute a prepaid field product with the ordinary rational ledger.
fn product_value(
    left: &Gaussian,
    right: &Gaussian,
    budget: &mut Budget,
) -> Result<Gaussian, Fault> {
    let diagonal = left.value.re.mul(&right.value.re, budget)?;
    let off_diagonal = left.value.im.mul(&right.value.im, budget)?;
    let forward = left.value.re.mul(&right.value.im, budget)?;
    let backward = left.value.im.mul(&right.value.re, budget)?;
    Ok(Gaussian {
        value: Complex {
            re: diagonal.sub(&off_diagonal, budget)?,
            im: forward.add(&backward, budget)?,
        },
    })
}

/// Execute a prepaid inverse after the caller has rejected zero.
fn inverse_value(value: &Gaussian, budget: &mut Budget) -> Result<Gaussian, Fault> {
    let norm = norm_value(value, budget)?;
    let real = value.value.re.div(&norm, budget)?;
    let imaginary = value.value.im.div(&norm, budget)?.negated(budget)?;
    Ok(Gaussian {
        value: Complex {
            re: real,
            im: imaginary,
        },
    })
}

impl Gaussian {
    /// Copy two admitted rationals into the exact Gaussian representation.
    pub(crate) fn from_parts(
        real: &Rational,
        imaginary: &Rational,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let at = Location::new("Gaussian components");
        begin(budget, at)?;
        let mut plan = Plan::default();
        plan.copy(Size::of(real, budget, at)?, at)?;
        plan.copy(Size::of(imaginary, budget, at)?, at)?;
        plan.reserve(budget, at)?;
        Ok(Self {
            value: Complex {
                re: copy(real, budget, at)?,
                im: copy(imaginary, budget, at)?,
            },
        })
    }

    /// Construct the additive identity under the active size and work ceilings.
    pub(crate) fn zero(budget: &mut Budget) -> Result<Self, Fault> {
        Self::constant(false, budget)
    }

    /// Construct the multiplicative identity under the active size and work ceilings.
    pub(crate) fn one(budget: &mut Budget) -> Result<Self, Fault> {
        Self::constant(true, budget)
    }

    /// Reserve both constant components before constructing either one.
    fn constant(nonzero: bool, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("Gaussian constant");
        begin(budget, at)?;
        budget.check(BudgetKind::ComponentBits, 1, at)?;
        let mut plan = Plan::default();
        plan.copy(Size::constant(nonzero), at)?;
        plan.copy(Size::constant(false), at)?;
        plan.reserve(budget, at)?;
        budget.arithmetic(cost(1, at)?, at)?;
        budget.arithmetic(cost(1, at)?, at)?;
        Ok(Self {
            value: Complex {
                re: if nonzero {
                    Rational::one()
                } else {
                    Rational::zero()
                },
                im: Rational::zero(),
            },
        })
    }

    /// Validate both components and return their size bounds without copying.
    fn sizes(&self, budget: &Budget, at: Location) -> Result<[Size; 2], Fault> {
        Ok([
            Size::of(&self.value.re, budget, at)?,
            Size::of(&self.value.im, budget, at)?,
        ])
    }

    /// Copy both components with the same ledger as arithmetic.
    pub(crate) fn copied(&self, budget: &mut Budget) -> Result<Self, Fault> {
        Self::from_parts(&self.value.re, &self.value.im, budget)
    }

    /// Negate both canonical rational components.
    pub(crate) fn negated(&self, budget: &mut Budget) -> Result<Self, Fault> {
        self.signed_copy(true, budget)
    }

    /// Fix the real component and negate the imaginary component.
    pub(crate) fn conjugate(&self, budget: &mut Budget) -> Result<Self, Fault> {
        self.signed_copy(false, budget)
    }

    /// Share the complete two-copy reservation for negation and conjugation.
    fn signed_copy(&self, negate_real: bool, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("Gaussian signed copy");
        begin(budget, at)?;
        let mut plan = Plan::default();
        for size in self.sizes(budget, at)? {
            plan.copy(size, at)?;
        }
        plan.reserve(budget, at)?;
        Ok(Self {
            value: Complex {
                re: if negate_real {
                    self.value.re.negated(budget)?
                } else {
                    copy(&self.value.re, budget, at)?
                },
                im: self.value.im.negated(budget)?,
            },
        })
    }

    /// Copy the real part with explicit component, storage and work checks.
    pub(crate) fn real(&self, budget: &mut Budget) -> Result<Rational, Fault> {
        self.component(false, budget)
    }

    /// Copy the imaginary part with explicit component, storage and work checks.
    pub(crate) fn imaginary(&self, budget: &mut Budget) -> Result<Rational, Fault> {
        self.component(true, budget)
    }

    /// Validate the value before extracting either one of its components.
    fn component(&self, imaginary: bool, budget: &mut Budget) -> Result<Rational, Fault> {
        let at = Location::new("Gaussian component copy");
        begin(budget, at)?;
        self.sizes(budget, at)?;
        let component = if imaginary {
            &self.value.im
        } else {
            &self.value.re
        };
        let mut plan = Plan::default();
        plan.copy(Size::of(component, budget, at)?, at)?;
        plan.reserve(budget, at)?;
        copy(component, budget, at)
    }

    /// Add both components after reserving both complete scalar operations.
    pub(crate) fn add(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        self.sum(other, false, budget)
    }

    /// Subtract both components, including the scalar negation reservations.
    pub(crate) fn sub(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        self.sum(other, true, budget)
    }

    /// Plan and execute both scalar additions or subtractions.
    fn sum(&self, other: &Self, subtract: bool, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("Gaussian sum");
        begin(budget, at)?;
        let mut plan = Plan::default();
        let operation = if subtract { Scalar::Sub } else { Scalar::Add };
        let [real, imaginary] = self.sizes(budget, at)?;
        let [other_real, other_imaginary] = other.sizes(budget, at)?;
        plan.scalar(operation, real, other_real, at)?;
        plan.scalar(operation, imaginary, other_imaginary, at)?;
        plan.reserve(budget, at)?;
        let operation = if subtract {
            Rational::sub
        } else {
            Rational::add
        };
        Ok(Self {
            value: Complex {
                re: operation(&self.value.re, &other.value.re, budget)?,
                im: operation(&self.value.im, &other.value.im, budget)?,
            },
        })
    }

    /// Multiply with four rational products and two rational sums.
    pub(crate) fn mul(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("Gaussian product");
        begin(budget, at)?;
        let mut plan = Plan::default();
        product_plan(
            &mut plan,
            self.sizes(budget, at)?,
            other.sizes(budget, at)?,
            at,
        )?;
        plan.reserve(budget, at)?;
        product_value(self, other, budget)
    }

    /// Return the rational norm `a² + b²` after reserving the complete sequence.
    pub(crate) fn norm(&self, budget: &mut Budget) -> Result<Rational, Fault> {
        let at = Location::new("Gaussian norm");
        begin(budget, at)?;
        let mut plan = Plan::default();
        norm_plan(&mut plan, self.sizes(budget, at)?, at)?;
        plan.reserve(budget, at)?;
        norm_value(self, budget)
    }

    /// Refuse zero before reserving any norm or division arithmetic.
    pub(crate) fn inverse(&self, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("Gaussian inverse");
        begin(budget, at)?;
        self.nonzero(budget, at)?;
        let mut plan = Plan::default();
        inverse_plan(&mut plan, self.sizes(budget, at)?, at)?;
        plan.reserve(budget, at)?;
        inverse_value(self, budget)
    }

    /// Divide after reserving the inverse and the product as a single sequence.
    pub(crate) fn div(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("Gaussian division");
        begin(budget, at)?;
        other.nonzero(budget, at)?;
        let mut plan = Plan::default();
        let inverse = inverse_plan(&mut plan, other.sizes(budget, at)?, at)?;
        product_plan(&mut plan, self.sizes(budget, at)?, inverse, at)?;
        plan.reserve(budget, at)?;
        product_value(self, &inverse_value(other, budget)?, budget)
    }

    /// Canonical component equality; no rational cross multiplication is needed.
    pub(crate) fn equals(&self, other: &Self, budget: &mut Budget) -> Result<bool, Fault> {
        let at = Location::new("Gaussian equality");
        begin(budget, at)?;
        let [real, imaginary] = self.sizes(budget, at)?;
        let [other_real, other_imaginary] = other.sizes(budget, at)?;
        let mut plan = Plan::default();
        for size in [real, imaginary, other_real, other_imaginary] {
            plan.copy(size, at)?;
        }
        plan.reserve(budget, at)?;
        for size in [real, imaginary, other_real, other_imaginary] {
            budget.arithmetic(cost(size.bits(), at)?, at)?;
        }
        Ok(self.value.re == other.value.re && self.value.im == other.value.im)
    }

    /// A metered exact zero decision on the canonical rational numerators.
    pub(crate) fn is_zero(&self, budget: &mut Budget) -> Result<bool, Fault> {
        let at = Location::new("Gaussian zero test");
        begin(budget, at)?;
        self.zero_value(budget, at)
    }

    /// Charge the complete scan before inspecting either numerator.
    fn zero_value(&self, budget: &mut Budget, at: Location) -> Result<bool, Fault> {
        let [real, imaginary] = self.sizes(budget, at)?;
        let mut plan = Plan::default();
        plan.copy(real, at)?;
        plan.copy(imaginary, at)?;
        plan.reserve(budget, at)?;
        budget.arithmetic(cost(real.bits(), at)?, at)?;
        budget.arithmetic(cost(imaginary.bits(), at)?, at)?;
        Ok(self.value.re.is_zero() && self.value.im.is_zero())
    }

    /// Internal divisor validation shares its parent's coefficient invocation.
    fn nonzero(&self, budget: &mut Budget, at: Location) -> Result<(), Fault> {
        if self.zero_value(budget, at)? {
            return Err(Fault::invalid(InvalidCode::ZeroDenominator, at));
        }
        Ok(())
    }
}
