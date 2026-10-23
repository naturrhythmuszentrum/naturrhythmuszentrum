// SPDX-License-Identifier: AGPL-3.0-only
//! A closed componentwise proof joins Complex multiplication to its complete Real block law.

mod coefficients;
mod result;
mod syntax;
pub use result::PhasorResultDefinition;

use super::{ReadingError, ReadingProjection, Representation, RepresentationKind, support};
use nrz_cas::{
    Cas, Handle,
    limits::{Budget, BudgetKind},
    readings::AffineMap,
};
use std::rc::Rc;

/// Time and passive-sign convention retained by the closed LC phasor lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhasorConvention {
    /// Passive voltage/current signs with `Re(V*exp(i*omega*t/radian))`.
    PositiveTimePassive,
    /// Opposite time exponent; this LC lowering explicitly refuses this convention.
    NegativeTimePassive,
}

/// Exact coordinate correspondence for two complete Complex block representations.
///
/// Bindings checks all four bilinear entry equations by exact rational distribution over
/// the retained component projections. CAS checks each full-family reconstruction; bindings
/// checks conjugation against the complete transposed expansion. This is a closed coordinate law, not a general Complex operation,
/// a physical result declaration, a solve-domain proof or a numerical enclosure certificate.
#[derive(Clone, Debug)]
pub struct PhasorBlockCorrespondence(Rc<Correspondence>);

/// All original syntax and exact checks remain associated with their ordered operands.
#[derive(Debug)]
struct Correspondence {
    /// Admitted original time and passive-sign convention.
    convention: PhasorConvention,
    /// Complete whole-family reconstruction witnesses, including every stored copy.
    operands: [Representation; 2],
    /// Ordered real/imaginary projections of each original physical reading.
    components: [[ReadingProjection; 2]; 2],
    /// Original Complex product followed by the native ordered Real block product.
    products: [Handle; 2],
    /// Four native component equalities, with Complex expansion before selected block entry.
    equations: [[Handle; 2]; 4],
    /// Four checked coefficient rows on the retained ordered bilinear component basis.
    coefficients: AffineMap,
    /// Full-family `R(conj(z))=R(z)^T` witnesses for both operands.
    conjugation: [PhasorConjugation; 2],
    /// Retained exact metadata allowance for consumers.
    bytes: u64,
}

impl PhasorBlockCorrespondence {
    /// Check complete representations, the explicit convention and all four block-product entries.
    ///
    /// The coefficient basis consists of products of these exact component projections; it
    /// creates no independent source coordinate and is valid for overlapping operands too.
    /// # Errors
    /// Refuses another convention, missing block witnesses, point roles, foreign contexts and limits.
    pub fn prove(
        cas: &mut Cas,
        operands: [Representation; 2],
        convention: PhasorConvention,
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        check(&operands, convention, budget)?;
        let retained = storage(&operands, budget)?;
        let (components, products, equations, coefficients, conjugation) = budget
            .with_retained_payload(retained, support::AT, |budget| {
                let components = [
                    syntax::components(cas, &operands[0], budget)?,
                    syntax::components(cas, &operands[1], budget)?,
                ];
                let products = syntax::products(cas, &operands, budget)?;
                let equations = syntax::equations(
                    cas,
                    operands[0].reading().family().context(),
                    &products,
                    &operands[0],
                    budget,
                )?;
                let coefficients =
                    coefficients::check(operands[0].expansion(), operands[1].expansion(), budget)?;
                let first = syntax::conjugation(cas, &operands[0], budget)?;
                let second = syntax::conjugation(cas, &operands[1], budget)?;
                Ok::<_, ReadingError>((
                    components,
                    products,
                    equations,
                    coefficients,
                    [first, second],
                ))
            })?;
        let bytes = storage(&operands, budget)?;
        budget.check(BudgetKind::OutputBytes, bytes, support::AT)?;
        Ok(Self(Rc::new(Correspondence {
            convention,
            operands,
            components,
            products,
            equations,
            coefficients,
            conjugation,
            bytes,
        })))
    }
    /// Match the exact ordered input readings before numerical execution, including selected copies.
    /// # Errors
    /// Refuses reordered, substituted or newly selected representations even when their values agree.
    pub fn check_operands(
        &self,
        operands: &[Representation; 2],
        budget: &mut Budget,
    ) -> Result<(), ReadingError> {
        for (expected, actual) in self.0.operands.iter().zip(operands) {
            for reading in [actual.reading(), expected.reading()] {
                for count in [
                    reading.entries().len(),
                    reading.family().entries().len(),
                    reading.family().original_entries().len(),
                    reading.selection().indices().len(),
                    reading.map().coefficients().len(),
                    reading.map().offsets().len(),
                ] {
                    budget.work(
                        u64::try_from(count).map_err(|_| ReadingError::Shape)?,
                        support::AT,
                    )?;
                }
            }
            let reading = actual.reading() == expected.reading();
            let reconstruction = actual.reconstruction() == expected.reconstruction();
            let kind = actual.kind() == expected.kind();
            if !reading || !reconstruction || !kind {
                return Err(ReadingError::Relation);
            }
        }
        Ok(())
    }
    /// Complete ordered operands with their original family and reconstruction evidence.
    #[must_use]
    pub fn operands(&self) -> &[Representation; 2] {
        &self.0.operands
    }
    /// Exact real/imaginary projections, with original source and selected-copy identity.
    #[must_use]
    pub fn components(&self) -> &[[ReadingProjection; 2]; 2] {
        &self.0.components
    }
    /// Original declared Complex multiplication expression.
    #[must_use]
    pub fn original(&self) -> &Handle {
        &self.0.products[0]
    }
    /// Ordered native Real matrix multiplication expression.
    #[must_use]
    pub fn lowered(&self) -> &Handle {
        &self.0.products[1]
    }
    /// Native equations `R(z*w)[row,column] = (R(z)*R(w))[row,column]` in logical column order.
    #[must_use]
    pub fn equations(&self) -> &[[Handle; 2]; 4] {
        &self.0.equations
    }
    /// Checked logical-column rows on `(a*u,a*v,b*u,b*v)`; no independent source is declared.
    #[must_use]
    pub fn coefficients(&self) -> &AffineMap {
        &self.0.coefficients
    }
    /// Closed exact conjugation checks on both actual operands and their complete reconstructions.
    #[must_use]
    pub fn conjugation(&self) -> &[PhasorConjugation; 2] {
        &self.0.conjugation
    }
    /// Retained exact evidence allowance; numerical clients reserve it beside their own payloads.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
    /// The only admitted time/passive-sign convention.
    #[must_use]
    pub fn convention(&self) -> PhasorConvention {
        self.0.convention
    }
}

/// A sealed complete block reconstruction cannot be replaced by sampled center structure.
fn check(
    operands: &[Representation; 2],
    convention: PhasorConvention,
    budget: &mut Budget,
) -> Result<(), ReadingError> {
    if convention != PhasorConvention::PositiveTimePassive {
        return Err(ReadingError::Unsupported);
    }
    if operands[0].reading().family().context() != operands[1].reading().family().context() {
        return Err(ReadingError::Unrelated);
    }
    for operand in operands {
        budget.work(1, support::AT)?;
        if operand.kind() != RepresentationKind::ComplexBlock || !operand.witness().is_discharged()
        {
            return Err(ReadingError::Definition);
        }
        for entry in operand.reading().entries() {
            budget.work(1, support::AT)?;
            if entry.descriptor().role() != crate::Role::Difference {
                return Err(ReadingError::Role);
            }
        }
    }
    Ok(())
}

/// Reserve the original reconstruction witnesses and fixed-size exact conjugation metadata.
fn storage(operands: &[Representation; 2], budget: &mut Budget) -> Result<u64, ReadingError> {
    let mut bytes = 8192_u64;
    for witness in operands.iter().map(Representation::witness) {
        budget.work(1, support::AT)?;
        let size = u64::try_from(witness.exported_byte_len()).map_err(|_| ReadingError::Shape)?;
        bytes = bytes
            .checked_add(size.checked_mul(8).ok_or(ReadingError::Shape)?)
            .ok_or(ReadingError::Shape)?;
    }
    budget.check(BudgetKind::StagedPayloadBytes, bytes, support::AT)?;
    Ok(bytes)
}

/// Exact full-block conjugation correspondence checked by rational coefficient composition.
#[derive(Clone, Debug)]
pub struct PhasorConjugation {
    /// Original physical reading with its whole-family reconstruction witness.
    operand: Representation,
    /// Native reconstructed conjugate and native transpose endpoints.
    endpoints: [Handle; 2],
    /// Exact checked rows of the conjugated block on the original real/imaginary basis.
    coefficients: AffineMap,
}
impl PhasorConjugation {
    /// Original complete representation, preserving the source and selected-copy identities.
    #[must_use]
    pub const fn operand(&self) -> &Representation {
        &self.operand
    }
    /// Native `R(conj(z))` and `R(z)^T` expressions, in that order.
    #[must_use]
    pub const fn endpoints(&self) -> &[Handle; 2] {
        &self.endpoints
    }
    /// Checked exact rows on the original ordered real/imaginary basis.
    #[must_use]
    pub const fn coefficients(&self) -> &AffineMap {
        &self.coefficients
    }
}
