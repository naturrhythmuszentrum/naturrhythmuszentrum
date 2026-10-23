// SPDX-License-Identifier: AGPL-3.0-only
//! Typed input blocks lower one complete normalized coefficient recipe before execution.

use super::KernelDraft;
use crate::kernels::{contract::Recipe, reciprocal::Schedule};
use crate::{CheckedShape, KernelError, OperationDraft, OperationError, RegionContract};
use nrz_bindings::{
    Budget, Descriptor,
    readings::{
        PositiveReciprocalMap, ReadingError,
        algebra::{Cas, Location, Rational, limits::BudgetKind},
    },
};
use nrz_scalar::LiteralLimits;
use std::rc::Rc;

impl OperationDraft {
    /// Lower a checked positive reciprocal operator over separately typed homogeneous input blocks.
    ///
    /// The complete exact law is `map(reference[i]/input[i])`, with column-ordered
    /// dimensionless result coefficients. Every input box must be strictly positive,
    /// even if its coefficient column is zero. Total and prior numerical errors follow
    /// separate interval images. Statistical operands require another recipe and refuse.
    /// # Errors
    /// Refuses foreign exact identity, wrong input order/meaning, incomplete input blocks,
    /// non-ratio outputs, non-rational coefficients and cumulative export resources.
    pub fn positive_reciprocal_map(
        graph: &Cas,
        definition: PositiveReciprocalMap,
        inputs: Vec<CheckedShape>,
        output: CheckedShape,
        literals: LiteralLimits,
        budget: &mut Budget,
    ) -> Result<Self, OperationError> {
        graph.validate_reuse_key(definition.reuse_key(), budget)?;
        let inputs = check_inputs(&definition, inputs, &output, budget)?;
        let output = RegionContract::check(output)?;
        let [rows, columns] = definition.shape();
        let dimensions = [
            output.shape().extent().rows(),
            output.shape().extent().columns(),
        ];
        if dimensions
            != [
                usize::try_from(rows).map_err(|_| OperationError::ResourceLimit)?,
                usize::try_from(columns).map_err(|_| OperationError::ResourceLimit)?,
            ]
            || output.descriptor() != &Descriptor::ratio()
        {
            return Err(KernelError::Contract.into());
        }
        let containers = inputs
            .len()
            .checked_add(1)
            .and_then(|value| value.checked_mul(std::mem::size_of::<RegionContract>()))
            .ok_or(OperationError::ResourceLimit)?;
        let retained = definition
            .retained_payload_bytes()
            .checked_add(u64::try_from(containers).map_err(|_| OperationError::ResourceLimit)?)
            .ok_or(OperationError::ResourceLimit)?;
        let (schedule, work, bytes) = budget.with_retained_payload(
            retained,
            Location::new("reciprocal coefficient export"),
            |budget| prepare(&definition, literals, budget),
        )?;
        budget.check(
            BudgetKind::OutputBytes,
            retained
                .checked_add(bytes)
                .ok_or(OperationError::ResourceLimit)?,
            Location::new("reciprocal recipe result"),
        )?;
        Ok(super::draft::kernel(KernelDraft {
            inputs: inputs.into(),
            output,
            recipe: Recipe::PositiveReciprocal(Box::new(schedule)),
            work,
            reciprocal: Some(Rc::new(definition)),
        }))
    }
}

/// Physical admission precedes exporting any source-free coefficient or accessing floats.
fn check_inputs(
    definition: &PositiveReciprocalMap,
    inputs: Vec<CheckedShape>,
    output: &CheckedShape,
    budget: &mut Budget,
) -> Result<Vec<RegionContract>, OperationError> {
    let count = inputs
        .len()
        .checked_add(definition.frame().entries().len())
        .ok_or(OperationError::ResourceLimit)?;
    budget.work(
        u64::try_from(count).map_err(|_| OperationError::ResourceLimit)?,
        Location::new("reciprocal input slots"),
    )?;
    let bytes = inputs
        .len()
        .checked_mul(std::mem::size_of::<RegionContract>())
        .ok_or(OperationError::ResourceLimit)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        u64::try_from(bytes).map_err(|_| OperationError::ResourceLimit)?,
        Location::new("reciprocal input slots"),
    )?;
    let mut result = crate::binding::buffer(inputs.len())?;
    let mut offset = 0_usize;
    for shape in inputs {
        shape.same_snapshot(output)?;
        let contract = RegionContract::check(shape)?;
        let end = offset
            .checked_add(contract.shape().extent().entries())
            .ok_or(OperationError::ResourceLimit)?;
        let entries = definition
            .frame()
            .entries()
            .get(offset..end)
            .ok_or(OperationError::InputCount)?;
        if entries
            .iter()
            .any(|entry| contract.descriptor() != entry.descriptor())
        {
            return Err(KernelError::Contract.into());
        }
        result.push(contract);
        offset = end;
    }
    if offset != definition.frame().entries().len() {
        return Err(OperationError::InputCount);
    }
    Ok(result)
}

/// All exact rational spellings and the finite decoding schedule precede numerical input access.
fn prepare(
    definition: &PositiveReciprocalMap,
    literals: LiteralLimits,
    budget: &mut Budget,
) -> Result<(Schedule, usize, u64), OperationError> {
    let mut rationals = crate::binding::buffer(definition.frame().entries().len())?;
    for entry in definition.frame().entries() {
        rationals.push(
            entry
                .magnitude()
                .as_rational()
                .ok_or(nrz_bindings::UnitError::UnsupportedExactConstantExport)
                .map_err(ReadingError::from)?,
        );
    }
    let mut bytes = 0_u64;
    let mut work = 0_usize;
    let references = export(&rationals, &mut bytes, &mut work, budget)?;
    let coefficients = export(
        &definition.map().coefficients().iter().collect::<Vec<_>>(),
        &mut bytes,
        &mut work,
        budget,
    )?;
    let offsets = export(
        &definition.map().offsets().iter().collect::<Vec<_>>(),
        &mut bytes,
        &mut work,
        budget,
    )?;
    let terms = references
        .len()
        .checked_mul(offsets.len())
        .and_then(|value| value.checked_mul(48))
        .and_then(|value| value.checked_add(references.len().checked_mul(48)?))
        .and_then(|value| value.checked_add(offsets.len().checked_mul(16)?))
        .ok_or(OperationError::ResourceLimit)?;
    work = work
        .checked_add(terms)
        .ok_or(OperationError::ResourceLimit)?;
    Ok((
        Schedule {
            references,
            coefficients,
            offsets,
            literals,
        },
        work,
        bytes,
    ))
}

/// Each limb-to-spelling conversion shares the caller ledger and retains all earlier strings.
fn export(
    values: &[&Rational],
    bytes: &mut u64,
    work: &mut usize,
    budget: &mut Budget,
) -> Result<Vec<String>, OperationError> {
    let allocation = u64::try_from(values.len())
        .map_err(|_| OperationError::ResourceLimit)?
        .checked_mul(32)
        .ok_or(OperationError::ResourceLimit)?;
    *bytes = bytes
        .checked_add(allocation)
        .ok_or(OperationError::ResourceLimit)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        *bytes,
        Location::new("reciprocal coefficients"),
    )?;
    let mut result = crate::binding::buffer(values.len())?;
    for value in values {
        let text = budget.with_retained_payload(
            *bytes,
            Location::new("reciprocal coefficients"),
            |budget| value.exact_string(budget),
        )?;
        *bytes = bytes
            .checked_add(u64::try_from(text.len()).map_err(|_| OperationError::ResourceLimit)?)
            .ok_or(OperationError::ResourceLimit)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            *bytes,
            Location::new("reciprocal coefficients"),
        )?;
        let cost = text
            .len()
            .checked_mul(12)
            .and_then(|value| value.checked_add(8))
            .ok_or(OperationError::ResourceLimit)?;
        *work = work
            .checked_add(cost)
            .ok_or(OperationError::ResourceLimit)?;
        result.push(text);
    }
    budget.check(
        BudgetKind::OutputBytes,
        *bytes,
        Location::new("reciprocal coefficients"),
    )?;
    Ok(result)
}
