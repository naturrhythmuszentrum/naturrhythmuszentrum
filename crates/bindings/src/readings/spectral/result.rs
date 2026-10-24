// SPDX-License-Identifier: AGPL-3.0-only
//! Complete conjugate roots and their component readings precede numerical publication.

use super::{AT, LcSpectralLaw, SpectralLawError as Error};
use crate::readings::{ProjectionSpace, ReadingRelation, Representation, RepresentationKind};
use nrz_cas::{
    Cas, Rational,
    limits::{Budget, BudgetKind},
    syntax::{AtomicSort, Field, Sort},
};
use std::rc::Rc;

/// Fresh output interpretation of both simple conjugate rates from one checked LC law.
#[derive(Clone, Debug)]
pub struct LcSpectralResultDefinition(Rc<Definition>);

/// Shared operation owner and the complete predeclared result family.
#[derive(Debug)]
struct Definition {
    /// Original complete-root theorem and explicit similarity/time transport.
    law: LcSpectralLaw,
    /// Two Complex rates, their two exact real zeros and two signed imaginary components.
    outputs: [Representation; 6],
    /// Four checked logical projections connecting the Complex roots to their Real components.
    components: [ReadingRelation; 4],
    /// Conservative owned metadata and native evidence bytes, excluding the shared law.
    bytes: u64,
}

impl LcSpectralResultDefinition {
    /// Check positive/negative Complex roots, their exact Real zeros and signed imaginary parts.
    ///
    /// Outputs are ordered positive root, negative root, positive real part, negative real part,
    /// positive imaginary part, negative imaginary part. All numerical views select one common
    /// positive rate coordinate; the exact signed maps define their different logical values.
    /// Component equations are ordered positive real, positive imaginary, negative real, negative imaginary.
    /// # Errors
    /// Refuses missing views, wrong signs, sorts, projections, physical meanings or caller limits.
    pub fn new(
        cas: &Cas,
        law: LcSpectralLaw,
        outputs: [Representation; 6],
        components: [ReadingRelation; 4],
        budget: &mut Budget,
    ) -> Result<Self, Error> {
        let first = outputs[0].reading();
        let family = first.family();
        if family.original_entries().len() != 1 || family.node() != family.root() {
            return Err(Error::Structure);
        }
        super::output_context::check(cas, &law, family.context(), budget)?;
        for (index, output) in outputs.iter().enumerate() {
            if output.reading().family() != family || !output.witness().is_unconditional() {
                return Err(Error::Structure);
            }
            view(output, index, budget)?;
            for prior in &outputs[..index] {
                budget.work(1, AT)?;
                if prior.reading().symbol() == output.reading().symbol() {
                    return Err(Error::Structure);
                }
            }
        }
        projections(&outputs, &components, budget)?;
        let bytes = storage(&outputs, &components, budget)?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        Ok(Self(Rc::new(Definition {
            law,
            outputs,
            components,
            bytes,
        })))
    }
    /// Original complete root law, including its retained transport assumptions.
    #[must_use]
    pub fn law(&self) -> &LcSpectralLaw {
        &self.0.law
    }
    /// Both roots, both exact real zeros and both imaginary components in the declared order.
    #[must_use]
    pub fn outputs(&self) -> &[Representation; 6] {
        &self.0.outputs
    }
    /// Actual checked logical component equations, distinct from shared storage overlaps.
    #[must_use]
    pub fn components(&self) -> &[ReadingRelation; 4] {
        &self.0.components
    }
    /// Newly owned complete result metadata; the original law remains shared.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}

/// Logical signs and exact zeros are complete affine maps over the shared positive rate.
fn view(output: &Representation, index: usize, budget: &mut Budget) -> Result<(), Error> {
    let reading = output.reading();
    let (field, coefficients, kind): (Field, &[i64], RepresentationKind) = match index {
        0 => (Field::Complex, &[0, 1], RepresentationKind::Defined),
        1 => (Field::Complex, &[0, -1], RepresentationKind::Defined),
        2 | 3 => (Field::Real, &[0], RepresentationKind::None),
        4 => (Field::Real, &[1], RepresentationKind::Defined),
        5 => (Field::Real, &[-1], RepresentationKind::Defined),
        _ => return Err(Error::Structure),
    };
    if output.kind() != kind
        || reading.sort() != &Sort::Value(AtomicSort::Scalar(field))
        || reading.map().inputs() != 1
        || reading.map().coefficients().len() != coefficients.len()
        || reading.selection().indices().len() != 1
    {
        return Err(Error::Structure);
    }
    for (actual, expected) in reading.map().coefficients().iter().zip(coefficients) {
        if actual != &Rational::from_ratio(*expected, 1, budget)? {
            return Err(Error::Sign);
        }
    }
    for offset in reading.map().offsets() {
        budget.work(1, AT)?;
        if !offset.is_zero() {
            return Err(Error::Structure);
        }
    }
    for entry in reading
        .entries()
        .iter()
        .chain(reading.family().original_entries())
    {
        budget.work(1, AT)?;
        super::transport::meaning(entry, "rate", [0, 0, -1, 0, 0, 0, 0], budget)?;
        if entry.magnitude().as_rational() != Some(&Rational::one()) {
            return Err(Error::Transport);
        }
    }
    Ok(())
}

/// Native component equations must name the actual root and Real symbol in the exact order.
fn projections(
    outputs: &[Representation; 6],
    components: &[ReadingRelation; 4],
    budget: &mut Budget,
) -> Result<(), Error> {
    for (relation, (root, component, coordinate)) in
        components
            .iter()
            .zip([(0, 2, 0), (0, 4, 1), (1, 3, 0), (1, 5, 1)])
    {
        budget.work(1, AT)?;
        let left = relation.left();
        let right = relation.right();
        if left.space() != ProjectionSpace::Value
            || right.space() != ProjectionSpace::Value
            || left.reading() != outputs[root].reading()
            || right.reading() != outputs[component].reading()
            || left.coordinates() != [coordinate]
            || right.coordinates() != [0]
            || !relation.witness().is_unconditional()
        {
            return Err(Error::Structure);
        }
    }
    Ok(())
}

/// Conservative owned representation and proof storage excludes the shared original law.
/// Ten distinct native witnesses own their canonical buffers; the established eight-times
/// envelope covers those buffers and dynamic endpoint/map metadata. CAS contexts are handles,
/// coordinate meanings are shared `Rc` slices, and the original physical law/frame is one `Rc`.
fn storage(
    outputs: &[Representation; 6],
    components: &[ReadingRelation; 4],
    budget: &mut Budget,
) -> Result<u64, Error> {
    let mut bytes =
        u64::try_from(std::mem::size_of::<Definition>() + 2 * std::mem::size_of::<usize>())
            .map_err(|_| crate::readings::ReadingError::Shape)?;
    for witness in outputs
        .iter()
        .map(Representation::witness)
        .chain(components.iter().map(ReadingRelation::witness))
    {
        budget.work(1, AT)?;
        let size = u64::try_from(witness.exported_byte_len())
            .map_err(|_| crate::readings::ReadingError::Shape)?;
        bytes = bytes
            .checked_add(
                size.checked_mul(8)
                    .ok_or(crate::readings::ReadingError::Shape)?,
            )
            .ok_or(crate::readings::ReadingError::Shape)?;
    }
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    Ok(bytes)
}
