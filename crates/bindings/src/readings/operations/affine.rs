// SPDX-License-Identifier: AGPL-3.0-only
//! Affine operation equality is reconstructed from the original input and exact output map.

use super::super::{PhysicalReading, ReadingError, meaning};
use super::ReadingOperationError;
use nrz_cas::{
    Cas, Handle,
    domain::Layer,
    limits::Budget,
    proof::{DomainContract, Judgment, Statement, VerifiedWitness},
    readings::AffineMap,
};

/// Whole-family relation for copy, projection or exact affine coordinate transport.
#[derive(Clone, Debug)]
pub struct AffineOperation {
    /// Exact original operand with every physical coordinate retained.
    input: PhysicalReading,
    /// Exact result reading independently checked against the operation definition.
    output: PhysicalReading,
    /// Ordered input-result coordinate map, including offsets.
    map: AffineMap,
    /// Actual CAS operation expression reconstructed from the input.
    expression: Handle,
    /// Complete output-operation equality on the original family domain.
    witness: VerifiedWitness,
}

impl AffineOperation {
    /// Prove the original result equals this closed affine operation over the complete family.
    /// # Errors
    /// Refuses changed targets, unrelated families, physical maps or original invalid operands.
    pub fn prove(
        cas: &mut Cas,
        input: &PhysicalReading,
        output: &PhysicalReading,
        map: AffineMap,
        budget: &mut Budget,
    ) -> Result<Self, ReadingOperationError> {
        let (expression, _) = expected(cas, input, output, &map, budget)?;
        let witness = cas.prove_reading_equality(
            input.family().context(),
            output.node(),
            &expression,
            budget,
        )?;
        Ok(Self {
            input: input.clone(),
            output: output.clone(),
            map,
            expression,
            witness,
        })
    }
    /// Rebuild the exact original operation before replaying an independently supplied equation.
    /// # Errors
    /// Refuses changed maps, copies, domains, original operands and exhausted finite limits.
    pub fn replay(
        cas: &mut Cas,
        input: &PhysicalReading,
        output: &PhysicalReading,
        map: AffineMap,
        bytes: &[u8],
        budget: &mut Budget,
    ) -> Result<Self, ReadingOperationError> {
        let (expression, claim) = expected(cas, input, output, &map, budget)?;
        let witness = cas.replay(bytes, &claim, budget)?;
        Ok(Self {
            input: input.clone(),
            output: output.clone(),
            map,
            expression,
            witness,
        })
    }
    /// Actual original input; matching a printed symbol is insufficient.
    #[must_use]
    pub const fn input(&self) -> &PhysicalReading {
        &self.input
    }
    /// Independently checked exact result reading.
    #[must_use]
    pub const fn output(&self) -> &PhysicalReading {
        &self.output
    }
    /// Closed kernel's exact mathematical coordinate map.
    #[must_use]
    pub const fn map(&self) -> &AffineMap {
        &self.map
    }
    /// Actual operation expression used in the expected equation.
    #[must_use]
    pub const fn expression(&self) -> &Handle {
        &self.expression
    }
    /// Complete result equality, with all original domains retained.
    #[must_use]
    pub const fn witness(&self) -> &VerifiedWitness {
        &self.witness
    }
}

/// All physical checking precedes constructing the mathematical operation expression.
fn expected(
    cas: &mut Cas,
    input: &PhysicalReading,
    output: &PhysicalReading,
    map: &AffineMap,
    budget: &mut Budget,
) -> Result<(Handle, Judgment), ReadingOperationError> {
    let context = input.family().context();
    if context != output.family().context() || input.family().root() != output.family().root() {
        return Err(ReadingError::Unrelated.into());
    }
    if input.family().root_entries != output.family().root_entries {
        return Err(ReadingError::Meaning.into());
    }
    meaning::validate(input.family().entries(), budget)?;
    meaning::validate(output.family().entries(), budget)?;
    meaning::map_meanings(map, input.entries(), output.entries(), budget)?;
    let expression = cas.define_reading(output.symbol(), input.node(), map, budget)?;
    let claim = Judgment {
        context: context.clone(),
        layer: Layer::Pointwise,
        space: None,
        statement: Statement::Equality {
            left: output.node().clone(),
            right: expression.clone(),
            domain: cas.admissible_region(context)?,
            domain_contract: DomainContract::DefinedAndEqual,
        },
        residuals: Vec::new(),
    };
    Ok((expression, claim))
}
