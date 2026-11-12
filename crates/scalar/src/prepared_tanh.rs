// SPDX-License-Identifier: AGPL-3.0-only
//! Conditional tanh composition, native argument revision one.

use crate::{
    ArithmeticContext, ArithmeticFailure, Enclosure, Expm1Failure, Expm1Input, Expm1Program, Finite,
};

/// Original represented operands and a relative endpoint target.
#[derive(Clone, Copy, Debug)]
pub struct TanhInput {
    /// Ordered original endpoints, before scaling.
    pub endpoints: [f64; 2],
    /// Independent nominal operand, including admitted subnormal interiors.
    pub center: f64,
    /// Relative endpoint target exponent.
    pub bits: u32,
}

/// Stored endpoints and separately checked nominal readings.
#[derive(Clone, Copy, Debug)]
pub struct TanhOutput {
    /// Ordered image endpoints with their original operand association.
    pub endpoints: [f64; 2],
    /// Rounded nominal quotient of the negated basis value by two plus that value.
    pub center: f64,
    /// Complete radius, already including numerical error once.
    pub total_radius: f64,
    /// Outward nominal displacement, independent of image width.
    pub numerical_error: f64,
}

/// Ordered admission and composition refusals; no failure proves impossibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TanhFailure {
    /// The requested target differs from this program's target.
    #[error("tanh target mismatch")]
    Target,
    /// Original endpoints or centre do not belong to the admitted ordered domain.
    #[error("tanh operand outside admitted domain")]
    OutsideDomain,
    /// An original nonzero endpoint has below-normal magnitude.
    #[error("tanh operand below normal magnitude")]
    BelowNormalOperand,
    /// The numerical basis does not match the exact scaled negative original domain and policy.
    #[error("tanh basis correspondence mismatch")]
    Correspondence,
    /// The conditional basis refused; policy revision one requests min(bits,40)+4 bits.
    #[error("tanh basis refused")]
    Basis {
        /// Preserved numerical basis cause.
        cause: Expm1Failure,
        /// Submitted relative basis bits, not final-target evidence.
        requested_bits: u32,
        /// Transformed domain endpoints, as binary64 encodings.
        domain: [u64; 2],
    },
    /// The rounded denominator fails the lower bound of one.
    #[error("tanh denominator not certified positive")]
    Denominator,
    /// The composed point interval does not certify the final target.
    #[error("tanh relative endpoint target not certifiable")]
    TargetNotCertifiable {
        /// Original endpoint index, zero for lower and one for upper.
        endpoint: u8,
        /// Point enclosure, as binary64 encodings.
        image: [u64; 2],
        /// Outward point width, as a binary64 encoding.
        width: u64,
        /// Final relative target exponent.
        bits: u32,
    },
    /// The conditional basis image has no admissible nonzero negative value.
    #[error("tanh basis image outside proved range")]
    Range,
    /// Finite outward arithmetic refused.
    #[error("tanh arithmetic: {0}")]
    Arithmetic(#[from] ArithmeticFailure),
}

/// Borrowed numerical composition, conditional on its owner's checked basis authority.
///
/// The owner establishes exact original-expression correspondence. This leaf
/// binds the original represented domain to its scaled negative execution domain; it
/// authenticates no supplied CAS object and allocates or charges no exact Work.
#[derive(Debug)]
pub struct TanhProgram<'a> {
    /// Borrowed numerical schedule whose exact authority remains with the owner.
    basis: &'a Expm1Program,
    /// Original represented domain before scaling.
    domain: [f64; 2],
    /// Final relative endpoint exponent, independent of the auxiliary target.
    bits: u32,
}

impl<'a> TanhProgram<'a> {
    /// Bind an expm1 basis to tanh's original domain and finite auxiliary policy.
    /// # Errors
    /// Refuses invalid domains, different basis domains or auxiliary targets.
    pub fn new(basis: &'a Expm1Program, domain: [f64; 2], bits: u32) -> Result<Self, TanhFailure> {
        let _span = tracing::trace_span!(
            "nrz.lower",
            family = "prepared-tanh",
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
    fn admit(basis: &'a Expm1Program, domain: [f64; 2], bits: u32) -> Result<Self, TanhFailure> {
        if !domain.iter().all(|x| x.is_finite())
            || domain[0] < 0.0
            || domain[1] > 1.0
            || domain[0] > domain[1]
        {
            return Err(TanhFailure::OutsideDomain);
        }
        let auxiliary = bits.min(40) + 4;
        if basis.domain != [-2.0 * domain[1], -2.0 * domain[0]] || basis.bits != auxiliary {
            return Err(TanhFailure::Correspondence);
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
        input: TanhInput,
        context: &ArithmeticContext,
    ) -> Result<TanhOutput, TanhFailure> {
        let _span = tracing::trace_span!(
            "nrz.kernel",
            family = "prepared-tanh",
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
        input: TanhInput,
        context: &ArithmeticContext,
    ) -> Result<TanhOutput, TanhFailure> {
        if input.bits != self.bits {
            return Err(TanhFailure::Target);
        }
        {
            let _validation =
                tracing::trace_span!("nrz.validation", family = "prepared-tanh", revision = 1_u64)
                    .entered();
            for x in input.endpoints {
                if x != 0.0 && x.abs() < f64::MIN_POSITIVE {
                    return Err(TanhFailure::BelowNormalOperand);
                }
            }
            if !input.endpoints.iter().all(|x| x.is_finite())
                || !input.center.is_finite()
                || input.endpoints[0] < self.domain[0]
                || input.endpoints[1] > self.domain[1]
                || input.endpoints[0] > input.center
                || input.center > input.endpoints[1]
            {
                return Err(TanhFailure::OutsideDomain);
            }
        }
        let transformed = Expm1Input {
            endpoints: [-2.0 * input.endpoints[1], -2.0 * input.endpoints[0]],
            center: -2.0 * input.center,
            bits: self.basis.bits,
        };
        let failure = |cause| TanhFailure::Basis {
            cause,
            requested_bits: transformed.bits,
            domain: self.basis.domain.map(f64::to_bits),
        };
        self.basis.validate(transformed, context).map_err(failure)?;
        let points = [
            self.basis.point(transformed.endpoints[0]),
            self.basis.point(transformed.endpoints[1]),
            self.basis.point(transformed.center),
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
            let relative = if interval.upper().get() == 0.0 && interval.lower().get() == 0.0 {
                0.0
            } else if interval.lower().get() > 0.0 {
                Enclosure::singleton(Finite::new(width)?)
                    .div(Enclosure::singleton(interval.lower()), context)?
                    .upper()
                    .get()
            } else {
                f64::INFINITY
            };
            if relative > epsilon {
                return Err(TanhFailure::TargetNotCertifiable {
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
        Ok(TanhOutput {
            endpoints,
            center: center.get(),
            total_radius: displacement.max(numerical_error),
            numerical_error,
        })
    }

    /// Compose the consistent extreme pairs after range and denominator admission.
    fn image(
        x: f64,
        point: super::PointReading,
        context: &ArithmeticContext,
    ) -> Result<(Enclosure<f64>, f64), TanhFailure> {
        if x == 0.0 {
            return Ok((Enclosure::new(0.0, 0.0)?, 0.0));
        }
        let lower = point.0[0].max(-1.0);
        let upper = point.0[1].min(0.0);
        if lower >= 0.0 || lower > upper || !(-1.0..=0.0).contains(&point.1) {
            return Err(TanhFailure::Range);
        }
        let image = Enclosure::new(lower, upper)?;
        let denominator = image.add(Enclosure::new(2.0, 2.0)?, context)?;
        if denominator.lower().get() < 1.0 {
            return Err(TanhFailure::Denominator);
        }
        let low = crate::primitive::div(-upper, denominator.upper().get())?
            .lower()
            .get();
        let high = crate::primitive::div(-lower, denominator.lower().get())?
            .upper()
            .get();
        Ok((Enclosure::new(low, high)?, -point.1 / (2.0 + point.1)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Expm1Parameters;

    /// Original magnitude and nominal inclusion precede scaling and zero; the transformed request is bound exactly.
    /// # Panics
    /// Panics if an ordered refusal, correspondence or zero identity changes.
    /// ´claim:scalar:prepared-tanh-admission´
    /// ´test:unit:prepared-tanh-admission´
    #[test]
    fn prepared_tanh_admission() -> Result<(), Box<dyn std::error::Error>> {
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
        let program = TanhProgram::new(&basis, [0.0, 1.0], 40)?;
        let input = TanhInput {
            endpoints: [0.0, 0.0],
            center: 0.0,
            bits: 40,
        };
        assert_eq!(program.evaluate(input, &context)?.endpoints, [0.0, 0.0]);
        assert_eq!(
            program
                .evaluate(
                    TanhInput {
                        endpoints: [f64::MIN_POSITIVE / 2.0; 2],
                        center: f64::MIN_POSITIVE / 2.0,
                        ..input
                    },
                    &context
                )
                .expect_err("original magnitude before scaling"),
            TanhFailure::BelowNormalOperand
        );
        assert_eq!(
            program
                .evaluate(
                    TanhInput {
                        center: 1.0,
                        ..input
                    },
                    &context
                )
                .expect_err("nominal before zero"),
            TanhFailure::OutsideDomain
        );
        assert_eq!(
            program
                .evaluate(TanhInput { bits: 39, ..input }, &context)
                .expect_err("target first"),
            TanhFailure::Target
        );
        assert_eq!(
            TanhProgram::new(&basis, [0.0, 0.5], 40).expect_err("scaled domain correspondence"),
            TanhFailure::Correspondence
        );
        assert_eq!(
            TanhProgram::new(&basis, [0.0, 1.0], 39).expect_err("auxiliary policy correspondence"),
            TanhFailure::Correspondence
        );
        Ok(())
    }

    /// Conditional basis reading failures preserve their actual request and cause.
    /// # Panics
    /// Panics if malformed basis ordering is replaced by a composed target refusal.
    /// ´claim:scalar:prepared-tanh-basis-failure´
    /// ´test:unit:prepared-tanh-basis-failure´
    #[test]
    fn prepared_tanh_basis_failure() -> Result<(), Box<dyn std::error::Error>> {
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
        let program = TanhProgram::new(&basis, [0.0, 1.0], 40)?;
        assert_eq!(
            program
                .evaluate(
                    TanhInput {
                        endpoints: [0.0, 1.0],
                        center: 0.5,
                        bits: 40
                    },
                    &context
                )
                .expect_err("basis readings before composition"),
            TanhFailure::Basis {
                cause: Expm1Failure::Arithmetic(ArithmeticFailure::InvertedEndpoints),
                requested_bits: 44,
                domain: [(-2.0_f64).to_bits(), 0.0_f64.to_bits()]
            }
        );
        Ok(())
    }

    /// A conditional range crossing zero keeps its negative part; rounded denominator admission precedes division.
    /// # Panics
    /// Panics if a range or denominator refusal disappears or uncertainty becomes an exact zero.
    /// ´claim:scalar:prepared-tanh-range-denominator´
    /// ´test:unit:prepared-tanh-range-denominator´
    #[test]
    fn prepared_tanh_range_denominator() -> Result<(), Box<dyn std::error::Error>> {
        let context = ArithmeticContext::new()?;
        assert_eq!(
            TanhProgram::image(-1.0, ([-1.0, -0.5], -0.75, 0.25), &context)
                .expect_err("rounded denominator below one"),
            TanhFailure::Denominator
        );
        assert_eq!(
            TanhProgram::image(-1.0, ([0.0, 0.25], 0.0, 0.25), &context)
                .expect_err("nonzero sign cannot collapse"),
            TanhFailure::Range
        );
        let (image, _) = TanhProgram::image(
            -f64::from_bits(1),
            ([-0.25, 0.25], -f64::from_bits(1), 0.25),
            &context,
        )?;
        assert_eq!(image.lower().get(), 0.0);
        assert!(
            image.upper().get() > 0.14,
            "uncertain sign retains negative range"
        );
        let (image, _) = TanhProgram::image(-0.5, ([-0.75, -0.5], -0.625, 0.125), &context)?;
        assert!(
            image.lower().get() < 1.0 / 3.0 && image.upper().get() > 0.6,
            "consistent quotient endpoint pairing"
        );
        Ok(())
    }
}
