// SPDX-License-Identifier: AGPL-3.0-only
//! Construct companion products and formal difference quotients through exact descriptor algebra.

use nrz_cas::Rational;

use super::{Entry, NamedReference, PairingError, PairingWitness};
use crate::exact::budget;
use crate::{Descriptor, Measure, Scale, UncertaintyMode, UnitError, uncertainty};

impl PairingWitness {
    /// Derive a componentwise uncertainty or one ordered covariance entry.
    ///
    /// Covariance requires both targets; variance repeats its one target. Affine
    /// offsets affect neither product dimensions nor difference scales. Relative
    /// modes require a named reference; non-relative modes refuse one.
    ///
    /// # Errors
    /// Refuses malformed target counts, missing or incompatible references, invalid
    /// scalar uncertainty kinds and exhausted exact descriptor arithmetic.
    pub fn uncertainty(
        mode: UncertaintyMode,
        target: &Descriptor,
        other: Option<&Descriptor>,
        reference: Option<&NamedReference>,
    ) -> Result<Self, PairingError> {
        let descriptor = uncertainty(
            mode,
            target,
            other,
            reference.map(|reference| (reference.descriptor(), true)),
        )?;
        Ok(Self {
            entry: Entry::Uncertainty { mode, descriptor },
            target: target.clone(),
            other: other.cloned(),
            reference: reference.cloned(),
        })
    }

    /// Derive a formal sensitivity entry with output-over-input difference coordinates.
    ///
    /// This is a semantic pairing contract, not proof of a derivative or permission
    /// to multiply unrelated scalar kinds. Point targets retain their origins, while
    /// the entry itself has zero offset. Discrete counts need a separately declared
    /// continuous quantity before a differential relation can be admitted.
    ///
    /// # Errors
    /// Refuses discrete target kinds, unnamed relative coordinates and exact overflow.
    pub fn sensitivity(output: &Descriptor, input: &Descriptor) -> Result<Self, PairingError> {
        for descriptor in [output, input] {
            if descriptor.kind().is_relative() {
                return Err(UnitError::RelativeReference.into());
            }
            if descriptor.kind().count_domain().is_some() {
                return Err(UnitError::CountDomain.into());
            }
        }
        let dimension = output.dimension().quotient(input.dimension())?;
        let measure = quotient_measure(output, input)?;
        Ok(Self {
            entry: Entry::Sensitivity { dimension, measure },
            target: output.clone(),
            other: Some(input.clone()),
            reference: None,
        })
    }
}

/// Difference-coordinate quotient with the existing bounded exact arithmetic budget.
fn quotient_measure(output: &Descriptor, input: &Descriptor) -> Result<Measure, UnitError> {
    let scale = output
        .measure()
        .scale()
        .div(input.measure().scale(), &mut budget()?)?;
    Measure::new("sensitivity", Scale::Exact(scale), Rational::zero())
}
