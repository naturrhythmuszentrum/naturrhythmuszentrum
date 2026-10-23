// SPDX-License-Identifier: AGPL-3.0-only
//! A fresh result slot is declared for one checked native product before numerical publication.

use super::super::support;
use super::{PhasorBlockCorrespondence, ReadingError, Representation, RepresentationKind};
use crate::{BinaryOp, Descriptor, Measure, Registry};
use nrz_cas::{
    Handle,
    limits::{Budget, BudgetKind},
};
use std::rc::Rc;

/// Predeclared storage interpretation of one actual checked phasor product.
///
/// The result root is a fresh output slot, not an independently assumed input or a CAS
/// equality between a free symbol and an operation. Only numerical execution of this
/// retained operation may initialize it. Its full reconstruction is a reusable storage law.
#[derive(Clone, Debug)]
pub struct PhasorResultDefinition(Rc<Definition>);

/// Exact operation identity, physical product rule and whole-family output interpretation.
#[derive(Debug)]
struct Definition {
    /// Original native operation and checked ordered block correspondence.
    law: PhasorBlockCorrespondence,
    /// Complete fresh result representation prepared before numerical execution.
    output: Representation,
    /// Physically checked coherent output kind.
    descriptor: Descriptor,
    /// Retained exact storage metadata allowance.
    bytes: u64,
}

impl PhasorResultDefinition {
    /// Match a fresh whole-family output slot to the checked ordered physical product.
    /// # Errors
    /// Refuses reused input roots, contexts, noncoherent inputs, unregistered kinds and wrong outputs.
    pub fn new(
        law: PhasorBlockCorrespondence,
        output: Representation,
        registry: &Registry,
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        let reading = output.reading();
        if output.kind() != RepresentationKind::ComplexBlock
            || !output.witness().is_discharged()
            || reading.family().original_entries().len() != 2
        {
            return Err(ReadingError::Definition);
        }
        for operand in law.operands() {
            budget.work(1, support::AT)?;
            if operand.reading().family().context() != reading.family().context()
                || operand.reading().family().root() == reading.family().root()
            {
                return Err(ReadingError::Unrelated);
            }
        }
        let descriptor = physical(&law, registry, budget)?;
        for entry in reading
            .entries()
            .iter()
            .chain(reading.family().entries())
            .chain(reading.family().original_entries())
        {
            budget.work(1, support::AT)?;
            if entry.descriptor() != &descriptor {
                return Err(ReadingError::Meaning);
            }
        }
        let witness =
            u64::try_from(output.witness().exported_byte_len()).map_err(|_| ReadingError::Shape)?;
        let bytes = law
            .retained_payload_bytes()
            .checked_add(witness.checked_mul(8).ok_or(ReadingError::Shape)?)
            .and_then(|size| size.checked_add(2048))
            .ok_or(ReadingError::Shape)?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, support::AT)?;
        budget.check(BudgetKind::OutputBytes, bytes, support::AT)?;
        Ok(Self(Rc::new(Definition {
            law,
            output,
            descriptor,
            bytes,
        })))
    }
    /// Actual original Complex product whose execution initializes the fresh output slot.
    #[must_use]
    pub fn operation(&self) -> &Handle {
        self.0.law.original()
    }
    /// Exact whole-family storage reconstruction; it asserts no free-input equality to the operation.
    #[must_use]
    pub fn output(&self) -> &Representation {
        &self.0.output
    }
    /// Complete ordered multiplication correspondence retained by the output definition.
    #[must_use]
    pub fn correspondence(&self) -> &PhasorBlockCorrespondence {
        &self.0.law
    }
    /// Coherent physical product kind checked through the explicit semantic registry.
    #[must_use]
    pub fn descriptor(&self) -> &Descriptor {
        &self.0.descriptor
    }
    /// Conservative retained exact evidence allowance.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}

/// Numerical block multiplication consumes coherent difference coordinates.
fn physical(
    law: &PhasorBlockCorrespondence,
    registry: &Registry,
    budget: &mut Budget,
) -> Result<Descriptor, ReadingError> {
    let mut descriptors = Vec::new();
    for operand in law.operands() {
        budget.work(1, support::AT)?;
        let descriptor = operand
            .reading()
            .entries()
            .first()
            .ok_or(ReadingError::Shape)?
            .descriptor();
        if descriptor.measure() != &Measure::coherent() {
            return Err(ReadingError::Measure);
        }
        descriptors.push(descriptor);
    }
    Ok(registry.binary(BinaryOp::Multiply, descriptors[0], descriptors[1])?)
}
