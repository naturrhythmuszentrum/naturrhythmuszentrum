// SPDX-License-Identifier: AGPL-3.0-only
//! Conditional sigmoid composition, native argument revision one.

use super::ExpProgram;
use crate::{
    ArithmeticContext, ArithmeticFailure, Enclosure, Expm1Failure, Expm1Input, Expm1Program, Finite,
};

/// Original represented operands and an absolute endpoint target.
#[derive(Clone, Copy, Debug)]
pub struct SigmoidInput {
    /// Ordered original endpoints, before negation.
    pub endpoints: [f64; 2],
    /// Independent nominal operand, including admitted subnormal interiors.
    pub center: f64,
    /// Absolute endpoint target exponent.
    pub bits: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Expm1Parameters;

    /// Original admission precedes transformation and exact zero; correspondence binds the domain and policy.
    /// # Panics
    /// Panics if a typed admission changes or a nonzero operand becomes an exact singleton.
    /// ´claim:scalar:prepared-sigmoid-admission´
    /// ´test:unit:prepared-sigmoid-admission´
    #[test]
    fn prepared_sigmoid_admission() -> Result<(), Box<dyn std::error::Error>> {
        let context = ArithmeticContext::new()?;
        let basis = Expm1Program::new(
            &[1.0],
            Expm1Parameters {
                domain: [-2.0, 0.0],
                bits: 44,
                radius_bits: 44,
                absolute_error: 2.0_f64.powi(-44),
                environment: 1,
            },
            &context,
        )?;
        let exp = ExpProgram::new(&basis, 44);
        let program = SigmoidProgram::new(&exp, [0.0, 2.0], 128)?;
        let zero = SigmoidInput {
            endpoints: [0.0, 0.0],
            center: 0.0,
            bits: 128,
        };
        assert_eq!(program.evaluate(zero, &context)?.endpoints, [0.5, 0.5]);
        assert_eq!(
            program
                .evaluate(
                    SigmoidInput {
                        endpoints: [-f64::from_bits(1), 0.0],
                        ..zero
                    },
                    &context
                )
                .expect_err("original magnitude before transformation"),
            SigmoidFailure::BelowNormalOperand
        );
        assert_eq!(
            program
                .evaluate(
                    SigmoidInput {
                        center: 1.0,
                        ..zero
                    },
                    &context
                )
                .expect_err("nominal before zero"),
            SigmoidFailure::OutsideDomain
        );
        assert_eq!(
            SigmoidProgram::new(&exp, [0.0, 1.0], 40).expect_err("domain correspondence"),
            SigmoidFailure::Correspondence
        );
        assert_eq!(
            SigmoidProgram::new(&exp, [0.0, 2.0], 39).expect_err("policy correspondence"),
            SigmoidFailure::Correspondence
        );
        assert!(matches!(
            program.evaluate(
                SigmoidInput {
                    endpoints: [2.0_f64.powi(-60); 2],
                    center: 2.0_f64.powi(-60),
                    ..zero
                },
                &context
            ),
            Err(SigmoidFailure::TargetNotCertifiable { .. })
        ));
        Ok(())
    }

    /// Basis validation preserves its cause and actual transformed numerical request.
    /// # Panics
    /// Panics if a conditional basis failure is reported as final-target impossibility.
    /// ´claim:scalar:prepared-sigmoid-basis-failure´
    /// ´test:unit:prepared-sigmoid-basis-failure´
    #[test]
    fn prepared_sigmoid_basis_failure() -> Result<(), Box<dyn std::error::Error>> {
        let context = ArithmeticContext::new()?;
        let basis = Expm1Program::new(
            &[1.0, 1.0],
            Expm1Parameters {
                domain: [-2.0, 0.0],
                bits: 44,
                radius_bits: 44,
                absolute_error: 2.0_f64.powi(-44),
                environment: 1,
            },
            &context,
        )?;
        let exp = ExpProgram::new(&basis, 44);
        let program = SigmoidProgram::new(&exp, [0.0, 2.0], 40)?;
        assert_eq!(
            program
                .evaluate(
                    SigmoidInput {
                        endpoints: [0.0, 2.0],
                        center: 1.0,
                        bits: 40,
                    },
                    &context
                )
                .expect_err("basis reading before composition"),
            SigmoidFailure::Basis {
                cause: Expm1Failure::Arithmetic(ArithmeticFailure::InvertedEndpoints),
                requested_bits: 44,
                domain: [-2.0_f64, 0.0].map(f64::to_bits),
            }
        );
        Ok(())
    }

    /// Conditional malformed images require a nonnegative exponential and a strictly positive denominator.
    /// # Panics
    /// Panics if a negative denominator reaches division or is confused with final target failure.
    /// ´claim:scalar:prepared-sigmoid-positive-denominator´
    /// ´test:unit:prepared-sigmoid-positive-denominator´
    #[test]
    fn prepared_sigmoid_positive_denominator() -> Result<(), Box<dyn std::error::Error>> {
        for value in [-3.0, -1.5] {
            assert_eq!(
                SigmoidProgram::image(
                    -1.0,
                    ([value, value], value, 0.0),
                    &ArithmeticContext::new()?
                )
                .expect_err("nonnegative exponential and positive denominator before reciprocal"),
                SigmoidFailure::Denominator
            );
        }
        Ok(())
    }
}

/// Stored endpoints and separately checked nominal readings.
#[derive(Clone, Copy, Debug)]
pub struct SigmoidOutput {
    /// Ordered image endpoints with their original operand association.
    pub endpoints: [f64; 2],
    /// Rounded nominal reciprocal of one plus the nominal exponential.
    pub center: f64,
    /// Complete radius, already including numerical error once.
    pub total_radius: f64,
    /// Outward nominal displacement, independent of image width.
    pub numerical_error: f64,
}

/// Ordered admission and composition refusals; no failure proves impossibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SigmoidFailure {
    /// The requested target differs from this program's target.
    #[error("sigmoid target mismatch")]
    Target,
    /// Original endpoints or centre do not belong to the admitted ordered domain.
    #[error("sigmoid operand outside admitted domain")]
    OutsideDomain,
    /// An original nonzero endpoint has below-normal magnitude.
    #[error("sigmoid operand below normal magnitude")]
    BelowNormalOperand,
    /// The numerical basis does not match the exact negated original domain and policy.
    #[error("sigmoid basis correspondence mismatch")]
    Correspondence,
    /// The conditional basis refused; policy revision one requests min(bits,40)+4 bits.
    #[error("sigmoid basis refused")]
    Basis {
        /// Preserved numerical basis cause.
        cause: Expm1Failure,
        /// Submitted relative basis bits, not final-target evidence.
        requested_bits: u32,
        /// Transformed domain endpoints, as binary64 encodings.
        domain: [u64; 2],
    },
    /// The exponential image or denominator fails the reciprocal positivity premises.
    #[error("sigmoid denominator not certified positive")]
    Denominator,
    /// The composed point interval does not certify the final target.
    #[error("sigmoid absolute endpoint target not certifiable")]
    TargetNotCertifiable {
        /// Original endpoint index, zero for lower and one for upper.
        endpoint: u8,
        /// Point enclosure, as binary64 encodings.
        image: [u64; 2],
        /// Outward point width, as a binary64 encoding.
        width: u64,
        /// Final absolute target exponent.
        bits: u32,
    },
    /// Finite outward arithmetic refused.
    #[error("sigmoid arithmetic: {0}")]
    Arithmetic(#[from] ArithmeticFailure),
}

/// Borrowed numerical composition, conditional on its owner's checked basis authority.
///
/// The owner establishes exact original-expression correspondence. This leaf
/// binds the original represented domain to its negated execution domain; it
/// authenticates no supplied CAS object and allocates or charges no exact Work.
#[derive(Debug)]
pub struct SigmoidProgram<'a> {
    /// Borrowed numerical schedule whose exact authority remains with the owner.
    basis: &'a ExpProgram<'a>,
    /// Original represented domain before negation.
    domain: [f64; 2],
    /// Final absolute endpoint exponent, independent of the auxiliary target.
    bits: u32,
}

impl<'a> SigmoidProgram<'a> {
    /// Bind an exp basis to sigmoid's original domain and finite auxiliary policy.
    /// # Errors
    /// Refuses invalid domains, different basis domains or auxiliary targets.
    pub fn new(
        basis: &'a ExpProgram<'a>,
        domain: [f64; 2],
        bits: u32,
    ) -> Result<Self, SigmoidFailure> {
        let _span = tracing::trace_span!(
            "nrz.lower",
            family = "prepared-sigmoid",
            revision = 1_u64,
            basis_bits = basis.bits,
            ?domain,
            bits
        )
        .entered();
        let result = Self::admit(basis, domain, bits);
        tracing::trace!(error = ?result.as_ref().err());
        result
    }

    /// Bind exact numerical domains and the finite auxiliary policy, without authenticating a proof.
    #[expect(
        clippy::float_cmp,
        reason = "exact numeric domain equality is required; signed zeros denote the same operand"
    )]
    fn admit(
        basis: &'a ExpProgram<'a>,
        domain: [f64; 2],
        bits: u32,
    ) -> Result<Self, SigmoidFailure> {
        if !domain.iter().all(|x| x.is_finite())
            || domain[0] < 0.0
            || domain[1] > 2.0
            || domain[0] > domain[1]
        {
            return Err(SigmoidFailure::OutsideDomain);
        }
        let auxiliary = bits.min(40) + 4;
        if basis.basis.domain != [-domain[1], -domain[0]]
            || basis.bits != auxiliary
            || basis.basis.bits != auxiliary
        {
            return Err(SigmoidFailure::Correspondence);
        }
        Ok(Self {
            basis,
            domain,
            bits,
        })
    }

    /// Validate original operands, then compose three independent point images.
    /// # Errors
    /// Preserves original operand refusals before transformation, basis causes,
    /// denominator positivity, finite arithmetic and final target evidence.
    pub fn evaluate(
        &self,
        input: SigmoidInput,
        context: &ArithmeticContext,
    ) -> Result<SigmoidOutput, SigmoidFailure> {
        let _span = tracing::trace_span!(
            "nrz.kernel",
            family = "prepared-sigmoid",
            revision = 1_u64,
            domain = ?self.domain,
            bits = self.bits,
            ?input,
            ?context
        )
        .entered();
        let result = self.apply(input, context);
        tracing::trace!(?result);
        result
    }

    /// Execute original admission, point composition and final numerical evidence.
    fn apply(
        &self,
        input: SigmoidInput,
        context: &ArithmeticContext,
    ) -> Result<SigmoidOutput, SigmoidFailure> {
        if input.bits != self.bits {
            return Err(SigmoidFailure::Target);
        }
        {
            let _validation = tracing::trace_span!(
                "nrz.validation",
                family = "prepared-sigmoid",
                revision = 1_u64
            )
            .entered();
            for x in input.endpoints {
                if x != 0.0 && x.abs() < f64::MIN_POSITIVE {
                    return Err(SigmoidFailure::BelowNormalOperand);
                }
            }
            if !input.endpoints.iter().all(|x| x.is_finite())
                || !input.center.is_finite()
                || input.endpoints[0] < self.domain[0]
                || input.endpoints[1] > self.domain[1]
                || input.endpoints[0] > input.center
                || input.center > input.endpoints[1]
            {
                return Err(SigmoidFailure::OutsideDomain);
            }
        }
        let transformed = Expm1Input {
            endpoints: [-input.endpoints[1], -input.endpoints[0]],
            center: -input.center,
            bits: self.basis.basis.bits,
        };
        let failure = |cause| SigmoidFailure::Basis {
            cause,
            requested_bits: transformed.bits,
            domain: self.basis.basis.domain.map(f64::to_bits),
        };
        self.basis
            .basis
            .validate(transformed, context)
            .map_err(failure)?;
        let points = [
            self.basis.basis.point(transformed.endpoints[0]),
            self.basis.basis.point(transformed.endpoints[1]),
            self.basis.basis.point(transformed.center),
        ];
        Expm1Program::readings(points, context).map_err(failure)?;
        let right = Self::image(transformed.endpoints[0], points[0], context)?;
        let left = Self::image(transformed.endpoints[1], points[1], context)?;
        let nominal = Self::image(transformed.center, points[2], context)?;
        let epsilon = if self.bits <= 1022 {
            f64::from_bits(u64::from(1023 - self.bits) << 52)
        } else if self.bits <= 1074 {
            f64::from_bits(1_u64 << (1074 - self.bits))
        } else {
            0.0
        };
        for (endpoint, interval) in [(0, left.0), (1, right.0)] {
            let width = Enclosure::singleton(interval.upper())
                .sub(Enclosure::singleton(interval.lower()), context)?
                .upper()
                .get();
            if width > epsilon {
                return Err(SigmoidFailure::TargetNotCertifiable {
                    endpoint,
                    image: [
                        interval.lower().get().to_bits(),
                        interval.upper().get().to_bits(),
                    ],
                    width: width.to_bits(),
                    bits: self.bits,
                });
            }
        }
        let endpoints = [left.0.lower().get(), right.0.upper().get()];
        let center = Finite::new(nominal.1)?;
        let numerical_error = nominal.0.displacement_from(center, context)?.get();
        let displacement = Enclosure::new(endpoints[0], endpoints[1])?
            .displacement_from(center, context)?
            .get();
        Ok(SigmoidOutput {
            endpoints,
            center: center.get(),
            total_radius: displacement.max(numerical_error),
            numerical_error,
        })
    }

    /// Enclose one reciprocal image and its independently rounded nominal centre.
    fn image(
        x: f64,
        point: super::super::PointReading,
        context: &ArithmeticContext,
    ) -> Result<(Enclosure<f64>, f64), SigmoidFailure> {
        if x == 0.0 {
            return Ok((Enclosure::new(0.5, 0.5)?, 0.5));
        }
        let (exponential, center) = ExpProgram::image(x, point, context)?;
        let one = Enclosure::new(1.0, 1.0)?;
        let denominator = exponential.add(one, context)?;
        if exponential.lower().get() < 0.0 || denominator.lower().get() <= 0.0 {
            return Err(SigmoidFailure::Denominator);
        }
        Ok((one.div(denominator, context)?, 1.0 / (1.0 + center)))
    }
}
