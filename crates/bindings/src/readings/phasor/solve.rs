// SPDX-License-Identifier: AGPL-3.0-only
//! The original equation and its complete block correspondence define a fresh current result.

use super::{PhasorBlockCorrespondence, ReadingError, Representation, RepresentationKind, result};
use crate::{Budget, Registry, readings::support};
use nrz_cas::{Cas, Rational, limits::BudgetKind};
use std::rc::Rc;

/// Closed LC solve request `Z*I=V`, retaining the actual original operands and output slot.
///
/// This specifies an equation to solve; it asserts neither a free-output equality nor
/// invertibility. The exact real part of impedance is zero throughout its family.
/// Numerical execution must prove separated reactance and certify the Real block solve.
#[derive(Clone, Debug)]
pub struct PhasorSolveDefinition(Rc<Definition>);

/// Exact equation, checked physical product and fresh whole-family result interpretation.
#[derive(Debug)]
struct Definition {
    /// Multiplication correspondence for the prescribed impedance and unknown current.
    law: PhasorBlockCorrespondence,
    /// Declared voltage, including all repeated copies and their reconstruction.
    voltage: Representation,
    /// Exact fixed real/imaginary components for impedance and voltage, where proved.
    fixed: [[Option<Rational>; 2]; 2],
    /// Retained exact evidence allowance.
    bytes: u64,
}

impl PhasorSolveDefinition {
    /// Check the original LC equation, ordered physical product and fresh current slot.
    /// # Errors
    /// Refuses nonreactive families, wrong units, reused result roots, contexts and limits.
    pub fn new(
        cas: &Cas,
        law: PhasorBlockCorrespondence,
        voltage: Representation,
        registry: &Registry,
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        check(&law, &voltage, budget)?;
        let descriptor = result::physical(&law, registry, budget)?;
        for entry in voltage.reading().entries() {
            budget.work(1, support::AT)?;
            if entry.descriptor() != &descriptor {
                return Err(ReadingError::Meaning);
            }
        }
        let fixed = [
            fixed_components(cas, &law.operands()[0], budget)?,
            fixed_components(cas, &voltage, budget)?,
        ];
        if fixed[0][0].as_ref().is_none_or(|value| !value.is_zero()) {
            return Err(ReadingError::Unsupported);
        }
        let bytes = storage(&law, &voltage, &fixed, budget)?;
        Ok(Self(Rc::new(Definition {
            law,
            voltage,
            fixed,
            bytes,
        })))
    }
    /// Exact multiplication law whose inverse problem is being solved.
    #[must_use]
    pub fn correspondence(&self) -> &PhasorBlockCorrespondence {
        &self.0.law
    }
    /// Complete original impedance block, with a proved identically zero real part.
    #[must_use]
    pub fn impedance(&self) -> &Representation {
        &self.0.law.operands()[0]
    }
    /// Declared voltage and its complete original representation.
    #[must_use]
    pub fn voltage(&self) -> &Representation {
        &self.0.voltage
    }
    /// Fresh current slot; only complete certified execution may initialize it.
    #[must_use]
    pub fn output(&self) -> &Representation {
        &self.0.law.operands()[1]
    }
    /// Fixed real/imaginary components of impedance and voltage, requiring numerical import coverage.
    #[must_use]
    pub fn fixed_inputs(&self) -> &[[Option<Rational>; 2]; 2] {
        &self.0.fixed
    }
    /// Conservative retained exact evidence allowance.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}

/// A fresh current family cannot also supply impedance or the declared voltage.
fn check(
    law: &PhasorBlockCorrespondence,
    voltage: &Representation,
    budget: &mut Budget,
) -> Result<(), ReadingError> {
    if voltage.kind() != RepresentationKind::ComplexBlock || !voltage.witness().is_discharged() {
        return Err(ReadingError::Definition);
    }
    for operand in [voltage, &law.operands()[0], &law.operands()[1]] {
        budget.work(1, support::AT)?;
        let entries = operand.reading().entries();
        entries[0].check(&entries[1])?;
    }
    let current = &law.operands()[1];
    if current.reading().family().original_entries().len() != 2 {
        return Err(ReadingError::Definition);
    }
    for operand in [&law.operands()[0], voltage] {
        budget.work(1, support::AT)?;
        let context = operand.reading().family().context() == current.reading().family().context();
        let fresh = operand.reading().family().root() != current.reading().family().root();
        if !context || !fresh {
            return Err(ReadingError::Unrelated);
        }
    }
    Ok(())
}

/// Constant components are exact source-free offsets, never inferred from represented centers.
fn fixed_components(
    cas: &Cas,
    representation: &Representation,
    budget: &mut Budget,
) -> Result<[Option<Rational>; 2], ReadingError> {
    let map = representation.reading().family_map(cas, budget)?;
    let width = usize::try_from(map.inputs()).map_err(|_| ReadingError::Shape)?;
    let mut result = [None, None];
    for (row, value) in result.iter_mut().enumerate() {
        let mut constant = true;
        for coefficient in &map.coefficients()[row * width..(row + 1) * width] {
            budget.work(1, support::AT)?;
            constant &= coefficient.is_zero();
        }
        if constant {
            *value = Some(map.offsets()[row].clone());
        }
    }
    Ok(result)
}

/// Retain the original reconstruction and every fixed exact component beside the equation.
fn storage(
    law: &PhasorBlockCorrespondence,
    voltage: &Representation,
    fixed: &[[Option<Rational>; 2]; 2],
    budget: &mut Budget,
) -> Result<u64, ReadingError> {
    let proof =
        u64::try_from(voltage.witness().exported_byte_len()).map_err(|_| ReadingError::Shape)?;
    let mut bytes = law
        .retained_payload_bytes()
        .checked_add(proof.checked_mul(8).ok_or(ReadingError::Shape)?)
        .and_then(|size| size.checked_add(2048))
        .ok_or(ReadingError::Shape)?;
    for value in fixed.iter().flatten().flatten() {
        budget.work(1, support::AT)?;
        let (top, bottom) = value.component_bits();
        let limbs = top
            .div_ceil(64)
            .checked_add(bottom.div_ceil(64))
            .ok_or(ReadingError::Shape)?;
        bytes = bytes
            .checked_add(limbs.checked_mul(16).ok_or(ReadingError::Shape)?)
            .and_then(|size| size.checked_add(256))
            .ok_or(ReadingError::Shape)?;
    }
    budget.check(BudgetKind::StagedPayloadBytes, bytes, support::AT)?;
    budget.check(BudgetKind::OutputBytes, bytes, support::AT)?;
    Ok(bytes)
}
