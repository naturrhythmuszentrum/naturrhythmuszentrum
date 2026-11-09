// SPDX-License-Identifier: AGPL-3.0-only
//! Checked addition of one, conditional on an admitted expm1 basis.

use super::{Expm1Failure, Expm1Input, Expm1Program, PointReading};
use crate::{ArithmeticContext, ArithmeticFailure, Enclosure, Finite};

/// Native exp application outcomes, independent of basis preparation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ExpFailure {
    /// The basis rejected the original operand or environment.
    #[error("exp basis application: {0}")]
    Basis(#[from] Expm1Failure),
    /// The requested absolute target differs from this program's target.
    #[error("exp target mismatch")]
    Target,
    /// Checked endpoint addition did not certify the absolute endpoint target.
    #[error("exp absolute endpoint target not certifiable")]
    TargetNotCertifiable {
        /// Original endpoint index: zero for lower, one for upper.
        endpoint: u8,
        /// Binary64 encodings of the checked point-image enclosure, in L/U order.
        image: [u64; 2],
        /// Binary64 encoding of the outward width used in the sufficient check.
        width: u64,
        /// Requested absolute endpoint target exponent.
        bits: u32,
    },
    /// Outward finite arithmetic could not be completed.
    #[error("exp arithmetic: {0}")]
    Arithmetic(#[from] ArithmeticFailure),
}

/// Exact represented operands and the absolute endpoint target.
#[derive(Clone, Copy, Debug)]
pub struct ExpInput {
    /// Ordered binary64 operand endpoints.
    pub endpoints: [f64; 2],
    /// Nominal operand, evaluated independently of the image midpoint.
    pub center: f64,
    /// Absolute endpoint target exponent.
    pub bits: u32,
}

/// Conditional exp readings; exact authority remains with the admitted owner.
#[derive(Clone, Copy, Debug)]
pub struct ExpOutput {
    /// Ordered stored binary64 image endpoints.
    pub endpoints: [f64; 2],
    /// Rounded addition of one at the nominal operand.
    pub center: f64,
    /// Complete whole-image radius, including the numerical reading once.
    pub total_radius: f64,
    /// Independently bounded error at the nominal operand.
    pub numerical_error: f64,
}

/// Borrow an admitted expm1 program without copying coefficients or exact work.
#[derive(Debug)]
pub struct ExpProgram<'a> {
    /// Conditional numerical basis; the owner retains its checked authority.
    basis: &'a Expm1Program,
    /// Exp's own absolute target, unrelated to success of the auxiliary target.
    bits: u32,
}

impl<'a> ExpProgram<'a> {
    /// Compose an already admitted basis with checked addition of one.
    /// This creates no certificate and performs no preparation or allocation.
    #[must_use]
    pub const fn new(basis: &'a Expm1Program, bits: u32) -> Self {
        Self { basis, bits }
    }

    /// Check original operands, evaluate endpoints and nominal centre, then test exp's target.
    /// # Errors
    /// Preserves basis refusals and separately refuses target mismatch or an
    /// uncertified absolute endpoint bound. No relative target implies success.
    pub fn evaluate(
        &self,
        input: ExpInput,
        context: &ArithmeticContext,
    ) -> Result<ExpOutput, ExpFailure> {
        let _span =
            tracing::trace_span!("nrz.kernel", family = "prepared-exp", revision = 1_u64).entered();
        if input.bits != self.bits {
            return Err(ExpFailure::Target);
        }
        // Validation precedes even the exact zero shortcut. This also admits
        // subnormal nominal interiors without admitting below-normal endpoints.
        {
            let _validation =
                tracing::trace_span!("nrz.validation", family = "prepared-exp", revision = 1_u64)
                    .entered();
            self.basis.validate(
                Expm1Input {
                    endpoints: input.endpoints,
                    center: input.center,
                    bits: self.basis.bits,
                },
                context,
            )?;
        }
        let points = [
            self.basis.point(input.endpoints[0]),
            self.basis.point(input.endpoints[1]),
            self.basis.point(input.center),
        ];
        // Conditional programs can fail these checks even after operand admission.
        // Preserve the basis cause and its precedence before composing any image.
        Expm1Program::readings(points, context)?;
        let left = Self::image(input.endpoints[0], points[0], context)?;
        let right = Self::image(input.endpoints[1], points[1], context)?;
        let nominal = Self::image(input.center, points[2], context)?;
        let epsilon = if self.bits <= 1022 {
            f64::from_bits(u64::from(1023 - self.bits) << 52)
        } else if self.bits <= 1074 {
            f64::from_bits(1_u64 << (1074 - self.bits))
        } else {
            0.0
        };
        // Width bounds each endpoint's slack; intrinsic box width is not tested.
        for (endpoint, interval) in [(0, left.0), (1, right.0)] {
            let width = Enclosure::singleton(interval.upper())
                .sub(Enclosure::singleton(interval.lower()), context)?
                .upper()
                .get();
            if width > epsilon {
                return Err(ExpFailure::TargetNotCertifiable {
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
        let numerical_error = nominal
            .0
            .displacement_from(Finite::new(nominal.1)?, context)?
            .get();
        let displacement = Enclosure::new(endpoints[0], endpoints[1])?
            .displacement_from(Finite::new(nominal.1)?, context)?
            .get();
        Ok(ExpOutput {
            endpoints,
            center: nominal.1,
            total_radius: displacement.max(numerical_error),
            numerical_error,
        })
    }

    /// Outward addition, with the validated exact zero identity as the only shortcut.
    fn image(
        x: f64,
        (endpoints, center, _): PointReading,
        context: &ArithmeticContext,
    ) -> Result<(Enclosure<f64>, f64), ArithmeticFailure> {
        if x == 0.0 {
            return Ok((Enclosure::new(1.0, 1.0)?, 1.0));
        }
        let image =
            Enclosure::new(endpoints[0], endpoints[1])?.add(Enclosure::new(1.0, 1.0)?, context)?;
        Ok((image, center + 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Expm1Parameters;

    /// Operand checks keep their typed order, including zero and subnormal nominal interiors.
    /// # Panics
    /// Panics if shared validation changes a refusal or a zero bypasses admission.
    /// ´claim:scalar:prepared-exp-validation-order´
    /// ´test:unit:prepared-exp-validation-order´
    #[test]
    fn prepared_exp_validation_order() -> Result<(), Box<dyn std::error::Error>> {
        let context = ArithmeticContext::new()?;
        let mut basis = Expm1Program::new(
            &[1.0],
            Expm1Parameters {
                domain: [-2.0, 0.0],
                bits: 40,
                radius_bits: 45,
                absolute_error: 0.0,
                environment: 1,
            },
            &context,
        )?;
        let zero = ExpInput {
            endpoints: [0.0, 0.0],
            center: 0.0,
            bits: 40,
        };
        let program = ExpProgram::new(&basis, 40);
        for (endpoints, center, cause) in [
            ([0.0, -1.0], 0.0, Expm1Failure::OutsideDomain),
            ([-1.0, 0.0], 0.25, Expm1Failure::OutsideDomain),
            ([f64::NAN, 0.0], 0.0, Expm1Failure::OutsideDomain),
            ([-2.0, 0.0], f64::INFINITY, Expm1Failure::OutsideDomain),
            ([0.0, f64::MIN_POSITIVE], 0.0, Expm1Failure::OutsideDomain),
            (
                [-f64::from_bits(1), 0.0],
                0.0,
                Expm1Failure::BelowNormalOperand,
            ),
        ] {
            assert_eq!(
                program
                    .evaluate(
                        ExpInput {
                            endpoints,
                            center,
                            ..zero
                        },
                        &context
                    )
                    .expect_err("operand admission"),
                ExpFailure::Basis(cause)
            );
        }
        let interior = program.evaluate(
            ExpInput {
                endpoints: [-f64::MIN_POSITIVE, 0.0],
                center: -f64::MIN_POSITIVE / 2.0,
                ..zero
            },
            &context,
        )?;
        assert_eq!(interior.center, 1.0);
        basis.domain = [-2.0, -0.25];
        assert_eq!(
            ExpProgram::new(&basis, 40)
                .evaluate(zero, &context)
                .expect_err("zero outside domain"),
            ExpFailure::Basis(Expm1Failure::OutsideDomain)
        );
        basis.revision = 2;
        assert_eq!(
            ExpProgram::new(&basis, 40)
                .evaluate(zero, &context)
                .expect_err("environment first"),
            ExpFailure::Basis(Expm1Failure::Environment)
        );
        assert_eq!(
            ExpProgram::new(&basis, 39)
                .evaluate(zero, &context)
                .expect_err("exp target first"),
            ExpFailure::Target
        );
        Ok(())
    }

    /// Structural admission alone does not establish monotonicity or finite nominal exports.
    /// # Panics
    /// Panics if operand admission or composition changes an arithmetic refusal.
    /// ´claim:scalar:prepared-exp-basis-readings´
    /// ´test:unit:prepared-exp-basis-readings´
    #[test]
    fn prepared_exp_basis_readings() -> Result<(), Box<dyn std::error::Error>> {
        let context = ArithmeticContext::new()?;
        let basis = Expm1Program::new(
            &[1.0, 1.0],
            Expm1Parameters {
                domain: [-2.0, 0.0],
                bits: 40,
                radius_bits: 45,
                absolute_error: f64::MAX,
                environment: 1,
            },
            &context,
        )?;
        let program = ExpProgram::new(&basis, 40);
        assert_eq!(
            program
                .evaluate(
                    ExpInput {
                        endpoints: [-2.0, 0.0],
                        center: -1.0,
                        bits: 40
                    },
                    &context
                )
                .expect_err("nonmonotone conditional basis"),
            ExpFailure::Basis(Expm1Failure::Arithmetic(
                ArithmeticFailure::InvertedEndpoints
            ))
        );
        assert_eq!(
            program
                .evaluate(
                    ExpInput {
                        endpoints: [-f64::MIN_POSITIVE, 0.0],
                        center: -f64::MIN_POSITIVE / 2.0,
                        bits: 40,
                    },
                    &context
                )
                .expect_err("nonfinite nominal export"),
            ExpFailure::Arithmetic(ArithmeticFailure::NonFinite)
        );
        Ok(())
    }

    /// Basis inversion takes precedence over a competing nonfinite composition.
    /// # Panics
    /// Panics if composition runs before the complete basis reading checks.
    /// ´claim:scalar:prepared-exp-competing-failures´
    /// ´test:unit:prepared-exp-competing-failures´
    #[test]
    fn prepared_exp_competing_failures() -> Result<(), Box<dyn std::error::Error>> {
        let context = ArithmeticContext::new()?;
        let basis = Expm1Program::new(
            &[1.0, 1.0],
            Expm1Parameters {
                domain: [-2.0, 0.0],
                bits: 40,
                radius_bits: 45,
                absolute_error: f64::MAX,
                environment: 1,
            },
            &context,
        )?;
        assert_eq!(
            ExpProgram::new(&basis, 40)
                .evaluate(
                    ExpInput {
                        endpoints: [-2.0, 0.0],
                        center: -f64::MIN_POSITIVE / 2.0,
                        bits: 40,
                    },
                    &context,
                )
                .expect_err("basis inversion before nonfinite composition"),
            ExpFailure::Basis(Expm1Failure::Arithmetic(
                ArithmeticFailure::InvertedEndpoints
            ))
        );
        Ok(())
    }

    /// Final target checks apply to exp's addition, independently of auxiliary bits.
    /// A conditional degree-one program is enough to expose addition spacing:
    /// near one the checked addition has nonzero width even when expm1 is tiny.
    /// # Panics
    /// Panics if exact zero bypasses validation or final addition loses outward rounding.
    /// ´claim:scalar:prepared-exp-final-target´
    /// ´test:unit:prepared-exp-final-target´
    #[test]
    fn prepared_exp_final_target() -> Result<(), Box<dyn std::error::Error>> {
        let context = ArithmeticContext::new()?;
        let mut basis = Expm1Program::new(
            &[1.0],
            Expm1Parameters {
                domain: [-2.0, 0.0],
                bits: 40,
                radius_bits: 40,
                absolute_error: 1.0,
                environment: 1,
            },
            &context,
        )?;
        let program = ExpProgram::new(&basis, 60);
        let point = |x| ExpInput {
            endpoints: [x, x],
            center: x,
            bits: 60,
        };
        assert_eq!(
            program.evaluate(point(0.0), &context)?.endpoints,
            [1.0, 1.0]
        );
        assert!(matches!(
            program.evaluate(point(-2.0_f64.powi(-60)), &context),
            Err(ExpFailure::TargetNotCertifiable { bits: 60, .. })
        ));
        assert!(matches!(
            program.evaluate(point(-0.5), &context),
            Err(ExpFailure::TargetNotCertifiable { bits: 60, .. })
        ));
        basis.revision = 2;
        assert_eq!(
            ExpProgram::new(&basis, 60)
                .evaluate(point(0.0), &context)
                .expect_err("environment before zero"),
            ExpFailure::Basis(Expm1Failure::Environment)
        );
        Ok(())
    }
}
