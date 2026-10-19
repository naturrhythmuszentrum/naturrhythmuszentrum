// SPDX-License-Identifier: AGPL-3.0-only
//! Closed tags and exact dependency handling for local space syntax.

use super::model::{EvaluationTarget, FamilyArgumentRole, RealEndpoint};
use super::proofs::{RealSignConclusion, RealSignRule};
use crate::encoding::Encoder;
use crate::error::Fault;
use crate::graph::Handle;
use crate::wire_decode::{Decoded, object};
use crate::wire_read::{Reader, invalid};

impl FamilyArgumentRole {
    /// Stable discriminator independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::Base => 0,
            Self::Fibre => 1,
            Self::Family => 2,
        }
    }
    /// Reject every unregistered tag.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::Base,
            1 => Self::Fibre,
            2 => Self::Family,
            _ => return Err(invalid()),
        })
    }
}

impl RealSignConclusion {
    /// Stable discriminator independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::Negative => 0,
            Self::Zero => 1,
            Self::Positive => 2,
        }
    }
    /// Reject every unregistered tag.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::Negative,
            1 => Self::Zero,
            2 => Self::Positive,
            _ => return Err(invalid()),
        })
    }
}

impl RealSignRule {
    /// Stable discriminator independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::Rational => 0,
            Self::Identity => 1,
            Self::Negation => 2,
            Self::Sum => 3,
            Self::Product => 4,
            Self::EvenPower => 5,
            Self::OddPowerReflection => 6,
            Self::Hypothesis => 7,
            Self::Root => 8,
        }
    }
    /// Reject every unregistered tag.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::Rational,
            1 => Self::Identity,
            2 => Self::Negation,
            3 => Self::Sum,
            4 => Self::Product,
            5 => Self::EvenPower,
            6 => Self::OddPowerReflection,
            7 => Self::Hypothesis,
            8 => Self::Root,
            _ => return Err(invalid()),
        })
    }
}

impl<R> RealEndpoint<R> {
    /// Transport only the finite endpoint reference.
    pub(crate) fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<RealEndpoint<S>, Fault> {
        Ok(match self {
            Self::NegativeInfinity => RealEndpoint::NegativeInfinity,
            Self::RealExpression(value) => RealEndpoint::RealExpression(write(value)?),
            Self::PositiveInfinity => RealEndpoint::PositiveInfinity,
        })
    }
    /// Signed infinities have no concealed dependencies.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        if let Self::RealExpression(value) = self {
            result.push(value);
        }
    }
    /// Encode the signed endpoint vocabulary.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        match self {
            Self::NegativeInfinity => out.byte(0),
            Self::RealExpression(value) => {
                out.byte(1)?;
                write(value, out)
            }
            Self::PositiveInfinity => out.byte(2),
        }
    }
}

impl RealEndpoint<Handle> {
    /// Decode a finite earlier expression or an exact signed infinity.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::NegativeInfinity,
            1 => Self::RealExpression(object(reader, records)?),
            2 => Self::PositiveInfinity,
            _ => return Err(invalid()),
        })
    }
}

impl<R> EvaluationTarget<R> {
    /// Keep family targets distinct from ordinary scalar slots.
    pub(crate) fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<EvaluationTarget<S>, Fault> {
        Ok(match self {
            Self::Scalar { header, space } => EvaluationTarget::Scalar {
                header: write(header)?,
                space: write(space)?,
            },
            Self::Family { family } => EvaluationTarget::Family {
                family: write(family)?,
            },
        })
    }
    /// Preserve every exact target dependency.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        match self {
            Self::Scalar { header, space } => result.extend([header, space]),
            Self::Family { family } => result.push(family),
        }
    }
    /// Encode one closed target kind with its exact references.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        match self {
            Self::Scalar { header, space } => {
                out.byte(0)?;
                write(header, out)?;
                write(space, out)
            }
            Self::Family { family } => {
                out.byte(1)?;
                write(family, out)
            }
        }
    }
}

impl EvaluationTarget<Handle> {
    /// No target can infer a placement, role change or scalar conversion.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::Scalar {
                header: object(reader, records)?,
                space: object(reader, records)?,
            },
            1 => Self::Family {
                family: object(reader, records)?,
            },
            _ => return Err(invalid()),
        })
    }
}
