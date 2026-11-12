// SPDX-License-Identifier: AGPL-3.0-only
//! Native execution of an externally checked, separate-operation Horner program.
//!
//! `Expm1Program` is a numerical program, not exact authority. Its owner must
//! establish the coefficient, radius, target and whole-domain correspondence
//! before interpreting its output as expm1 evidence. Matrix owns that admission.
//! Construction checks finite structural premises; it does not manufacture a
//! certificate for arbitrary caller-supplied coefficient or error bounds.

use crate::{ArithmeticContext, ArithmeticFailure, Enclosure, Finite};

#[path = "prepared_exp.rs"]
mod exp;
pub use exp::{ExpFailure, ExpInput, ExpOutput, ExpProgram};
pub use exp::{SigmoidFailure, SigmoidInput, SigmoidOutput, SigmoidProgram};

/// One rounded schedule's endpoints, nominal value and numerical radius.
type PointReading = ([f64; 2], f64, f64);

/// Structural and native application refusals, without an exact Work ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Expm1Failure {
    /// The fixed native coefficient inventory cannot hold the supplied program.
    #[error("expm1 coefficient inventory limit {limit}, needed {needed}")]
    InventoryLimit {
        /// Inclusive coefficient capacity.
        limit: usize,
        /// Supplied coefficient count, checked before traversal.
        needed: usize,
    },
    /// A supplied numerical program has an invalid finite inventory or parameter.
    #[error("malformed expm1 program")]
    MalformedPlan,
    /// The admitted arithmetic revision does not match the program.
    #[error("unsupported expm1 arithmetic environment")]
    Environment,
    /// An operand or nominal centre lies outside its ordered admitted domain.
    #[error("expm1 operand outside admitted domain")]
    OutsideDomain,
    /// An original nonzero endpoint has magnitude below the smallest normal.
    #[error("expm1 operand below normal magnitude")]
    BelowNormalOperand,
    /// The requested endpoint target differs from the prepared target.
    #[error("expm1 target mismatch")]
    Target,
    /// Native enclosure construction refused.
    #[error("expm1 native arithmetic: {0}")]
    Arithmetic(#[from] ArithmeticFailure),
}

/// Finite inputs to an externally checked program; no descriptor or heap handle.
#[derive(Clone, Copy, Debug)]
pub struct Expm1Input {
    /// Ordered represented endpoints of the original operand box.
    pub endpoints: [f64; 2],
    /// The represented nominal input, independently evaluated for numerical error.
    pub center: f64,
    /// Relative per-endpoint target; must equal the admitted target.
    pub bits: u32,
}

/// Conditional native output, interpreted as evidence only by its admitted owner.
#[derive(Clone, Copy, Debug)]
pub struct Expm1Output {
    /// Ordered stored binary64 endpoints.
    pub endpoints: [f64; 2],
    /// Rounded evaluation at the supplied nominal input centre.
    pub center: f64,
    /// Outward whole-box radius, already including the numerical error.
    pub total_radius: f64,
    /// Separate error at the nominal input, not endpoint slack.
    pub numerical_error: f64,
}

/// Bounded immutable native program, conditional on its owner's checked theorem.
///
/// No method here consults exact authority, constructs a certificate, allocates
/// heap storage, or charges exact Work. Coefficients are in factorial order.
#[derive(Debug)]
pub struct Expm1Program {
    /// Fixed storage avoids allocation while lowering or applying this program.
    coefficients: [f64; 128],
    /// Admitted active coefficient count.
    degree: usize,
    /// Exact power-of-two multiplier for the schedule's radius export.
    radius_scale: f64,
    /// Uniform absolute bound for nominal centres that are subnormal interiors.
    absolute_error: f64,
    /// Represented execution domain, checked against the original exact domain.
    domain: [f64; 2],
    /// Original target encoding.
    bits: u32,
    /// Arithmetic context revision.
    revision: u32,
}

/// Bounded parameters transferred from the authority-owning lowering layer.
#[derive(Clone, Copy, Debug)]
pub struct Expm1Parameters {
    /// Finite binary64 subdomain of the original checked claim.
    pub domain: [f64; 2],
    /// Checked relative endpoint target.
    pub bits: u32,
    /// Exact radius multiplier is two to minus this exponent.
    pub radius_bits: u32,
    /// Outward bound for the final stage error plus absolute analytic tail.
    pub absolute_error: f64,
    /// Required arithmetic revision.
    pub environment: u32,
}

impl Expm1Program {
    /// Copy an already prepaid, checked coefficient inventory into fixed storage.
    ///
    /// The caller must have checked the schedule theorem and exact correspondence.
    /// This constructor grants no such authority. Count checks precede traversal.
    /// # Errors
    /// Refuses malformed parameters or an unsupported arithmetic revision.
    pub fn new(
        coefficients: &[f64],
        parameters: Expm1Parameters,
        context: &ArithmeticContext,
    ) -> Result<Self, Expm1Failure> {
        if coefficients.len() > 128 {
            return Err(Expm1Failure::InventoryLimit {
                limit: 128,
                needed: coefficients.len(),
            });
        }
        if coefficients.is_empty() {
            return Err(Expm1Failure::MalformedPlan);
        }
        if parameters.environment != 1 || parameters.environment != context.revision() {
            return Err(Expm1Failure::Environment);
        }
        let valid_domain = parameters.domain.iter().all(|x| x.is_finite())
            && parameters.domain[0] >= -2.0
            && parameters.domain[1] <= 0.0
            && parameters.domain[0] <= parameters.domain[1];
        let valid_bounds = parameters.radius_bits <= 1074
            && parameters.absolute_error.is_finite()
            && parameters.absolute_error >= 0.0;
        if !valid_domain || !valid_bounds || coefficients[0].to_bits() != 1.0_f64.to_bits() {
            return Err(Expm1Failure::MalformedPlan);
        }
        let mut stored = [0.0; 128];
        for (target, &source) in stored.iter_mut().zip(coefficients) {
            if !source.is_finite() || source <= 0.0 || source > 1.0 {
                return Err(Expm1Failure::MalformedPlan);
            }
            *target = source;
        }
        let radius_scale = if parameters.radius_bits <= 1022 {
            f64::from_bits(u64::from(1023 - parameters.radius_bits) << 52)
        } else {
            f64::from_bits(1_u64 << (1074 - parameters.radius_bits))
        };
        Ok(Self {
            coefficients: stored,
            degree: coefficients.len(),
            radius_scale,
            absolute_error: parameters.absolute_error,
            domain: parameters.domain,
            bits: parameters.bits,
            revision: parameters.environment,
        })
    }

    /// Execute fresh endpoint and nominal evaluations without reaching the authority owner.
    ///
    /// Original binary64 operands are exact imports. Nonzero original endpoints
    /// below normal refuse; a subnormal nominal interior is evaluated using the
    /// separately checked uniform absolute error, not the normal-input ratio.
    /// # Errors
    /// Refuses mismatched targets/environments, invalid domains, below-normal
    /// endpoints or unavailable finite outward arithmetic.
    pub fn evaluate(
        &self,
        input: Expm1Input,
        context: &ArithmeticContext,
    ) -> Result<Expm1Output, Expm1Failure> {
        let _span = tracing::trace_span!("nrz.kernel", family = "prepared-expm1", revision = 1_u64)
            .entered();
        self.validate(input, context)?;
        Self::readings(
            [
                self.point(input.endpoints[0]),
                self.point(input.endpoints[1]),
                self.point(input.center),
            ],
            context,
        )
    }

    /// Validate original operands in refusal order before any point shortcut.
    fn validate(&self, input: Expm1Input, context: &ArithmeticContext) -> Result<(), Expm1Failure> {
        if context.revision() != self.revision {
            return Err(Expm1Failure::Environment);
        }
        if input.bits != self.bits {
            return Err(Expm1Failure::Target);
        }
        let [lower, upper] = input.endpoints;
        let valid = lower.is_finite()
            && upper.is_finite()
            && input.center.is_finite()
            && lower >= self.domain[0]
            && upper <= self.domain[1]
            && lower <= input.center
            && input.center <= upper;
        if !valid {
            return Err(Expm1Failure::OutsideDomain);
        }
        for endpoint in input.endpoints {
            if endpoint != 0.0 && endpoint.abs() < f64::MIN_POSITIVE {
                return Err(Expm1Failure::BelowNormalOperand);
            }
        }
        Ok(())
    }

    /// Preserve finite endpoint ordering and nominal displacement checks for conditional programs.
    fn readings(
        [left, right, nominal]: [PointReading; 3],
        context: &ArithmeticContext,
    ) -> Result<Expm1Output, Expm1Failure> {
        let endpoints = [left.0[0], right.0[1]];
        let enclosure = Enclosure::new(endpoints[0], endpoints[1])?;
        let displacement = enclosure
            .displacement_from(Finite::new(nominal.1)?, context)?
            .get();
        // The numerical reading is already covered, never added to a complete radius.
        let total_radius = displacement.max(nominal.2);
        Ok(Expm1Output {
            endpoints,
            center: nominal.1,
            total_radius,
            numerical_error: nominal.2,
        })
    }

    /// One separate multiply/add schedule and the exact checked neighbour export.
    fn point(&self, x: f64) -> PointReading {
        if x == 0.0 {
            return ([0.0, 0.0], 0.0, 0.0);
        }
        let mut value = self.coefficients[self.degree - 1];
        for coefficient in self.coefficients[..self.degree - 1].iter().rev() {
            value *= x;
            value += coefficient;
        }
        value *= x;
        let radius = if x.abs() < f64::MIN_POSITIVE {
            self.absolute_error
        } else {
            (x.abs() * self.radius_scale).next_up()
        };
        let endpoints = [(value - radius).next_down(), (value + radius).next_up()];
        (endpoints, value, radius)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Structural admission, coefficient capacity and environment checks precede any zero result.
    /// # Panics
    /// Panics if an invalid inventory or environment reaches native evaluation.
    /// ´claim:scalar:prepared-expm1-program-admission´
    /// ´test:unit:prepared-expm1-program-admission´
    #[test]
    fn prepared_expm1_program_admission() -> Result<(), Box<dyn std::error::Error>> {
        let context = ArithmeticContext::new()?;
        let parameters = Expm1Parameters {
            domain: [-2.0, 0.0],
            bits: 40,
            radius_bits: 20,
            absolute_error: 1.0,
            environment: 1,
        };
        // These are structural program tests; these dummy arrays are not expm1 authority.
        Expm1Program::new(&[1.0; 128], parameters, &context)?;
        assert_eq!(
            Expm1Program::new(&[1.0; 129], parameters, &context).expect_err("capacity"),
            Expm1Failure::InventoryLimit {
                limit: 128,
                needed: 129
            }
        );
        for values in [&[][..], &[0.5][..], &[1.0, f64::NAN][..], &[1.0, -0.5][..]] {
            assert_eq!(
                Expm1Program::new(values, parameters, &context).expect_err("malformed"),
                Expm1Failure::MalformedPlan
            );
        }
        for p in [
            Expm1Parameters {
                radius_bits: 1075,
                ..parameters
            },
            Expm1Parameters {
                absolute_error: -1.0,
                ..parameters
            },
            Expm1Parameters {
                domain: [-3.0, 0.0],
                ..parameters
            },
        ] {
            assert_eq!(
                Expm1Program::new(&[1.0], p, &context).expect_err("parameters"),
                Expm1Failure::MalformedPlan
            );
        }
        assert_eq!(
            Expm1Program::new(
                &[1.0],
                Expm1Parameters {
                    environment: 2,
                    ..parameters
                },
                &context
            )
            .expect_err("environment"),
            Expm1Failure::Environment
        );
        let mut program = Expm1Program::new(&[1.0], parameters, &context)?;
        program.revision = 2;
        assert_eq!(
            program
                .evaluate(
                    Expm1Input {
                        endpoints: [0.0, 0.0],
                        center: 0.0,
                        bits: 40
                    },
                    &context
                )
                .expect_err("zero environment"),
            Expm1Failure::Environment
        );
        Ok(())
    }
}

#[path = "prepared_tanh.rs"]
mod tanh;
pub use tanh::{TanhFailure, TanhInput, TanhOutput, TanhProgram};
