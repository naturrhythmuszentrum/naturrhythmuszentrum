// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`dyadic_power_admission`] | scalar | Exact reduction and isolated refusals precede the validated one shortcut. |
//! | [`dyadic_power_inventory`] | scalar | Each independent inclusive inventory is enforced, even for exact one. |

//! Point numerical execution of the conditional dyadic-power schedule.

use crate::{ArithmeticContext, ArithmeticFailure, Enclosure, Finite};

/// Independent inclusive bounds for exponent traversal, roots and products.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DyadicPowerLimits {
    /// Maximum bits in the reduced positive numerator.
    pub exponent_bits: usize,
    /// Maximum outward square-root calls.
    pub root_steps: usize,
    /// Maximum squares plus set-bit multiplications.
    pub product_steps: usize,
}

/// Finite point image and its separately bounded midpoint reading.
#[derive(Clone, Copy, Debug)]
pub struct DyadicPowerOutput {
    /// Outward image endpoints.
    pub endpoints: [f64; 2],
    /// Rounded midpoint of the image.
    pub center: f64,
    /// Outward displacement from the midpoint.
    pub total_radius: f64,
    /// Numerical error, equal to the point displacement.
    pub numerical_error: f64,
}

/// Ordered numerical, resource and engine refusals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DyadicPowerFailure {
    /// Exponent or original base lies outside the admitted positive domain.
    #[error("dyadic power outside admitted domain")]
    OutsideDomain,
    /// An original base has nonzero below-normal magnitude.
    #[error("dyadic power operand below normal magnitude")]
    BelowNormalOperand,
    /// The reduced in-range exponent is not dyadic.
    #[error("dyadic power exponent is not dyadic")]
    NonDyadicExponent,
    /// A finite outward upper bound is unavailable.
    #[error("dyadic power target not certifiable: {failure}")]
    TargetNotCertifiable {
        /// Preserved arithmetic cause.
        failure: ArithmeticFailure,
    },
    /// A separate inclusive inventory bound is exceeded.
    #[error("dyadic power inventory limit {limit}, needed {needed}")]
    InventoryLimit {
        /// Supplied capacity for this inventory.
        limit: usize,
        /// Derived required capacity.
        needed: usize,
    },
    /// The exponent representation has a zero denominator.
    #[error("malformed dyadic power program")]
    MalformedPlan,
    /// The arithmetic proof revision does not match.
    #[error("unsupported dyadic power arithmetic environment")]
    Environment,
    /// A non-certification arithmetic invariant failed.
    #[error("dyadic power arithmetic: {failure}")]
    Engine {
        /// Preserved arithmetic cause.
        failure: ArithmeticFailure,
    },
}

/// Immutable numerical plan; original-expression authority stays with its owner.
#[derive(Clone, Copy, Debug)]
pub struct DyadicPowerProgram {
    /// Original represented point domain.
    domain: [f64; 2],
    /// Positive reduced numerator.
    numerator: u64,
    /// Degree of the canonical power-of-two denominator.
    root_steps: u32,
    /// Required roots, squares and set-bit products.
    inventory: [usize; 3],
    /// Supplied independent capacities retained with the numerical plan.
    limits: DyadicPowerLimits,
    /// Admitted arithmetic environment revision.
    revision: u32,
}

impl DyadicPowerProgram {
    /// Admit an exact represented exponent and point execution domain.
    ///
    /// The plan authenticates no original operation or retained enclosure.
    /// # Errors
    /// Refuses malformed, out-of-range or non-dyadic exponents, exhausted
    /// inventories, environment mismatch and invalid original domain endpoints.
    pub fn new(
        domain: [f64; 2],
        numerator: i64,
        denominator: u64,
        limits: DyadicPowerLimits,
        context: &ArithmeticContext,
    ) -> Result<Self, DyadicPowerFailure> {
        let span = tracing::trace_span!(
            "nrz.lower",
            family = "dyadic-power",
            argument_revision = 1_u32,
            environment_revision = context.revision(),
            numerator,
            denominator,
            ?domain,
            ?limits,
            canonical_numerator = tracing::field::Empty,
            canonical_root_steps = tracing::field::Empty,
            result = tracing::field::Empty
        );
        let _entered = span.enter();
        let result = Self::admit(domain, numerator, denominator, limits, context);
        if let Ok(program) = &result {
            span.record("canonical_numerator", program.numerator);
            span.record("canonical_root_steps", program.root_steps);
        }
        span.record("result", tracing::field::debug(&result));
        result
    }

    /// Keep exact classification ahead of environment and original domain checks.
    fn admit(
        domain: [f64; 2],
        numerator: i64,
        denominator: u64,
        limits: DyadicPowerLimits,
        context: &ArithmeticContext,
    ) -> Result<Self, DyadicPowerFailure> {
        if denominator == 0 {
            return Err(DyadicPowerFailure::MalformedPlan);
        }
        let magnitude = numerator.unsigned_abs();
        let mut left = magnitude;
        let mut right = denominator;
        while right != 0 {
            (left, right) = (right, left % right);
        }
        let reduced_numerator = magnitude / left;
        let reduced_denominator = denominator / left;
        if numerator <= 0 || reduced_numerator > reduced_denominator {
            return Err(DyadicPowerFailure::OutsideDomain);
        }
        if !reduced_denominator.is_power_of_two() {
            return Err(DyadicPowerFailure::NonDyadicExponent);
        }
        let root_steps = reduced_denominator.trailing_zeros();
        let bits = reduced_numerator.bit_width() as usize;
        let squares = bits - 1;
        let multiplies = reduced_numerator.count_ones() as usize - 1;
        let products = squares + multiplies;
        for (limit, needed) in [
            (limits.exponent_bits, bits),
            (limits.root_steps, root_steps as usize),
            (limits.product_steps, products),
        ] {
            if needed > limit {
                return Err(DyadicPowerFailure::InventoryLimit { limit, needed });
            }
        }
        if context.revision() != 1 {
            return Err(DyadicPowerFailure::Environment);
        }
        if domain
            .iter()
            .any(|x| *x != 0.0 && x.abs() < f64::MIN_POSITIVE)
        {
            return Err(DyadicPowerFailure::BelowNormalOperand);
        }
        if !domain.iter().all(|x| x.is_finite())
            || domain[0] <= 0.0
            || domain[0] > domain[1]
            || domain[1] > 2.0
        {
            return Err(DyadicPowerFailure::OutsideDomain);
        }
        Ok(Self {
            domain,
            numerator: reduced_numerator,
            root_steps,
            inventory: [root_steps as usize, squares, multiplies],
            limits,
            revision: context.revision(),
        })
    }

    /// Original represented domain admitted at construction.
    #[must_use]
    pub const fn domain(&self) -> [f64; 2] {
        self.domain
    }

    /// Canonical exact exponent as positive numerator and power-of-two degree.
    #[must_use]
    pub const fn exponent(&self) -> (u64, u32) {
        (self.numerator, self.root_steps)
    }

    /// Required root, square and set-bit multiplication calls.
    #[must_use]
    pub const fn inventory(&self) -> [usize; 3] {
        self.inventory
    }

    /// Native argument proof revision.
    #[must_use]
    pub const fn argument_revision(&self) -> u32 {
        1
    }

    /// Validate an original represented point and execute its admitted schedule.
    /// # Errors
    /// Refuses an environment mismatch, invalid original point, absent finite
    /// outward upper bound, or an unexpected arithmetic invariant failure.
    pub fn evaluate(
        &self,
        x: f64,
        context: &ArithmeticContext,
    ) -> Result<DyadicPowerOutput, DyadicPowerFailure> {
        let span = tracing::trace_span!(
            "nrz.kernel",
            family = "dyadic-power",
            argument_revision = 1_u32,
            environment_revision = context.revision(),
            canonical_numerator = self.numerator,
            canonical_root_steps = self.root_steps,
            limits = ?self.limits,
            x,
            roots = 0_u64,
            squares = 0_u64,
            multiplies = 0_u64,
            midpoint_steps = 0_u64,
            displacement_steps = 0_u64,
            result = tracing::field::Empty
        );
        let _entered = span.enter();
        let result = self.apply(x, context, &span);
        span.record("result", tracing::field::debug(&result));
        result
    }

    /// Execute validated point instructions and record each completed stage.
    #[expect(
        clippy::float_cmp,
        reason = "exact one is a represented identity after validation"
    )]
    fn apply(
        &self,
        x: f64,
        context: &ArithmeticContext,
        span: &tracing::Span,
    ) -> Result<DyadicPowerOutput, DyadicPowerFailure> {
        {
            let validation = tracing::trace_span!(
                "nrz.validation",
                family = "dyadic-power",
                argument_revision = 1_u32,
                x,
                result = tracing::field::Empty
            );
            let _entered = validation.enter();
            let result = self.validate(x, context);
            validation.record("result", tracing::field::debug(&result));
            result?;
        }
        if x == 1.0 {
            return Ok(DyadicPowerOutput {
                endpoints: [1.0, 1.0],
                center: 1.0,
                total_radius: 0.0,
                numerical_error: 0.0,
            });
        }
        let mut roots = 0_u64;
        let mut squares = 0_u64;
        let mut multiplies = 0_u64;
        let mut image = Enclosure::singleton(Finite::new(x).map_err(Self::engine)?);
        for _ in 0..self.root_steps {
            image = image.sqrt(context).map_err(Self::schedule_failure)?;
            roots += 1;
            span.record("roots", roots);
        }
        let root = image;
        let bits = self.numerator.bit_width();
        for shift in (0..bits - 1).rev() {
            image = image.square(context).map_err(Self::schedule_failure)?;
            squares += 1;
            span.record("squares", squares);
            if (self.numerator >> shift) & 1 != 0 {
                image = image.mul(root, context).map_err(Self::schedule_failure)?;
                if image.lower().get() < 0.0 {
                    image = Enclosure::new(0.0, image.upper().get()).map_err(Self::engine)?;
                }
                multiplies += 1;
                span.record("multiplies", multiplies);
            }
        }
        let lower = image.lower().get();
        let upper = image.upper().get();
        let difference = upper - lower;
        span.record("midpoint_steps", 1_u64);
        let half = difference * 0.5;
        span.record("midpoint_steps", 2_u64);
        let center = lower + half;
        span.record("midpoint_steps", 3_u64);
        let center = Finite::new(center).map_err(Self::engine)?;
        let error = image
            .displacement_from(center, context)
            .map_err(Self::engine)?
            .get();
        span.record("displacement_steps", 2_u64);
        Ok(DyadicPowerOutput {
            endpoints: [lower, upper],
            center: center.get(),
            total_radius: error,
            numerical_error: error,
        })
    }

    /// Refuse a changed environment or original point before any shortcut.
    fn validate(&self, x: f64, context: &ArithmeticContext) -> Result<(), DyadicPowerFailure> {
        if context.revision() != self.revision {
            return Err(DyadicPowerFailure::Environment);
        }
        if x != 0.0 && x.abs() < f64::MIN_POSITIVE {
            return Err(DyadicPowerFailure::BelowNormalOperand);
        }
        if !x.is_finite() || x < self.domain[0] || x > self.domain[1] {
            return Err(DyadicPowerFailure::OutsideDomain);
        }
        Ok(())
    }

    /// Preserve an arithmetic invariant failure outside numerical certification.
    const fn engine(failure: ArithmeticFailure) -> DyadicPowerFailure {
        DyadicPowerFailure::Engine { failure }
    }

    /// Reserve certification refusal for absent finite outward upper bounds.
    fn schedule_failure(failure: ArithmeticFailure) -> DyadicPowerFailure {
        if failure == ArithmeticFailure::Overflow {
            DyadicPowerFailure::TargetNotCertifiable { failure }
        } else {
            Self::engine(failure)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Capacities sufficient for every admission fixture in this module.
    const ENOUGH: DyadicPowerLimits = DyadicPowerLimits {
        exponent_bits: 63,
        root_steps: 63,
        product_steps: 124,
    };

    /// Exact reduction and isolated refusals precede the validated one shortcut.
    /// # Panics
    /// Panics if canonical admission or any named refusal changes.
    /// ´claim:scalar:dyadic-power-admission´
    /// ´test:unit:dyadic-power-admission´
    #[test]
    fn dyadic_power_admission() -> Result<(), Box<dyn std::error::Error>> {
        let context = ArithmeticContext::new()?;
        let domain = [f64::MIN_POSITIVE, 2.0];
        let program = DyadicPowerProgram::new(domain, 3, 6, ENOUGH, &context)?;
        assert_eq!(program.exponent(), (1, 1));
        assert_eq!(program.domain(), domain);
        assert_eq!(program.argument_revision(), 1);
        assert_eq!(program.evaluate(1.0, &context)?.endpoints, [1.0, 1.0]);
        for (numerator, denominator, expected) in [
            (1, 0, DyadicPowerFailure::MalformedPlan),
            (0, 1, DyadicPowerFailure::OutsideDomain),
            (-1, 2, DyadicPowerFailure::OutsideDomain),
            (2, 1, DyadicPowerFailure::OutsideDomain),
            (4, 3, DyadicPowerFailure::OutsideDomain),
            (-1, 3, DyadicPowerFailure::OutsideDomain),
            (1, 3, DyadicPowerFailure::NonDyadicExponent),
            (i64::MIN, 1, DyadicPowerFailure::OutsideDomain),
        ] {
            assert_eq!(
                DyadicPowerProgram::new(domain, numerator, denominator, ENOUGH, &context)
                    .expect_err("exponent refusal"),
                expected
            );
        }
        for bad_domain in [
            [0.0, 2.0],
            [-1.0, 2.0],
            [2.0, 1.0],
            [f64::NAN, 2.0],
            [f64::MIN_POSITIVE, f64::INFINITY],
        ] {
            assert_eq!(
                DyadicPowerProgram::new(bad_domain, 1, 2, ENOUGH, &context)
                    .expect_err("domain refusal"),
                DyadicPowerFailure::OutsideDomain
            );
        }
        for bad_domain in [
            [f64::MIN_POSITIVE / 2.0, 2.0],
            [-f64::MIN_POSITIVE / 2.0, 2.0],
        ] {
            assert_eq!(
                DyadicPowerProgram::new(bad_domain, 1, 2, ENOUGH, &context)
                    .expect_err("original magnitude refusal"),
                DyadicPowerFailure::BelowNormalOperand
            );
        }
        for value in [0.0, -1.0, 2.0_f64.next_up(), f64::NAN, f64::INFINITY] {
            assert_eq!(
                program.evaluate(value, &context).expect_err("point domain"),
                DyadicPowerFailure::OutsideDomain
            );
        }
        for value in [f64::from_bits(1), -f64::from_bits(1)] {
            assert_eq!(
                program
                    .evaluate(value, &context)
                    .expect_err("point magnitude"),
                DyadicPowerFailure::BelowNormalOperand
            );
        }
        let restricted = DyadicPowerProgram::new([0.25, 0.5], 3, 4, ENOUGH, &context)?;
        assert_eq!(
            restricted
                .evaluate(1.0, &context)
                .expect_err("domain before one"),
            DyadicPowerFailure::OutsideDomain
        );
        let mut changed = program;
        changed.revision = 2;
        assert_eq!(
            changed.evaluate(1.0, &context).expect_err("environment"),
            DyadicPowerFailure::Environment
        );
        Ok(())
    }

    /// Each independent inclusive inventory is enforced, even for exact one.
    /// # Panics
    /// Panics if a required root or product is skipped or a limit is bypassed.
    /// ´claim:scalar:dyadic-power-inventory´
    /// ´test:unit:dyadic-power-inventory´
    #[test]
    fn dyadic_power_inventory() -> Result<(), Box<dyn std::error::Error>> {
        let context = ArithmeticContext::new()?;
        let domain = [f64::MIN_POSITIVE, 2.0];
        let exact = DyadicPowerLimits {
            exponent_bits: 2,
            root_steps: 2,
            product_steps: 2,
        };
        let program = DyadicPowerProgram::new(domain, 3, 4, exact, &context)?;
        assert_eq!(program.exponent(), (3, 2));
        assert_eq!(program.inventory(), [2, 1, 1]);
        assert_eq!(program.evaluate(1.0, &context)?.endpoints, [1.0, 1.0]);
        for limits in [
            DyadicPowerLimits {
                exponent_bits: 1,
                ..exact
            },
            DyadicPowerLimits {
                root_steps: 1,
                ..exact
            },
            DyadicPowerLimits {
                product_steps: 1,
                ..exact
            },
        ] {
            assert_eq!(
                DyadicPowerProgram::new(domain, 3, 4, limits, &context)
                    .expect_err("inventory before exact one"),
                DyadicPowerFailure::InventoryLimit {
                    limit: 1,
                    needed: 2
                }
            );
        }
        let distinct = DyadicPowerLimits {
            exponent_bits: 4,
            root_steps: 6,
            product_steps: 5,
        };
        let program = DyadicPowerProgram::new(domain, 13, 64, distinct, &context)?;
        assert_eq!(program.exponent(), (13, 6));
        assert_eq!(program.inventory(), [6, 3, 2]);
        assert_eq!(program.evaluate(1.0, &context)?.endpoints, [1.0, 1.0]);
        for (limits, limit, needed) in [
            (
                DyadicPowerLimits {
                    exponent_bits: 3,
                    ..distinct
                },
                3,
                4,
            ),
            (
                DyadicPowerLimits {
                    root_steps: 5,
                    ..distinct
                },
                5,
                6,
            ),
            (
                DyadicPowerLimits {
                    product_steps: 4,
                    ..distinct
                },
                4,
                5,
            ),
            (
                DyadicPowerLimits {
                    root_steps: 5,
                    product_steps: 4,
                    ..distinct
                },
                5,
                6,
            ),
        ] {
            assert_eq!(
                DyadicPowerProgram::new(domain, 13, 64, limits, &context)
                    .expect_err("distinct inventory need"),
                DyadicPowerFailure::InventoryLimit { limit, needed }
            );
        }
        let identity = DyadicPowerProgram::new(
            domain,
            1,
            1,
            DyadicPowerLimits {
                exponent_bits: 1,
                root_steps: 0,
                product_steps: 0,
            },
            &context,
        )?;
        assert_eq!(identity.inventory(), [0, 0, 0]);
        assert_eq!(identity.evaluate(2.0, &context)?.endpoints, [2.0, 2.0]);
        Ok(())
    }
}
