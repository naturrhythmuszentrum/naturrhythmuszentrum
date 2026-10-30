// SPDX-License-Identifier: AGPL-3.0-only
//! Physical affine definitions retain original selected sources and exact coherent entry meanings.

use super::{DecisionError, DecisionExpression, storage};
use crate::{CheckedDeclarations, Measure, Role, readings::PhysicalReading};
use nrz_cas::{
    Cas, ExactConstant, Handle,
    decision::{ScalarKernelFamily, ScalarKernelReading},
    limits::Budget,
    syntax::{AtomicSort, Field, Operator, Sort},
};

/// Validate the complete canonical reading inventory without adding cost to an unmapped family.
pub(super) fn check(
    cas: &Cas,
    owner: &CheckedDeclarations,
    context: &Handle,
    inputs: &[(String, ScalarKernelFamily)],
    readings: &[(String, PhysicalReading)],
    budget: &mut Budget,
) -> Result<u64, DecisionError> {
    let mut bytes = 0_u64;
    let mut previous: Option<&str> = None;
    for (name, reading) in readings {
        let size = metadata(reading)
            .saturating_add(u64::try_from(name.len()).map_err(|_| DecisionError::Capacity)?);
        budget.work(size.saturating_add(5), storage::AT)?;
        if previous.is_some_and(|key| key >= name.as_str()) {
            return Err(DecisionError::Source);
        }
        previous = Some(name);
        let (_, family) = inputs
            .iter()
            .find(|(key, _)| key == name)
            .ok_or(DecisionError::Source)?;
        let original = owner.checked(name)?;
        let shape = reading.family().shape() == [1, 1]
            && reading.map().inputs() == 1
            && reading.map().outputs() == 1;
        let scalar = reading.sort() == &Sort::Value(AtomicSort::Scalar(Field::Real));
        if !shape
            || !scalar
            || reading.family().context() != context
            || reading.selected() != &family.source
            || reading.family().node() != reading.family().root()
        {
            return Err(DecisionError::Source);
        }
        let (Operator::AffineReading(map), _) = cas.expression(reading.node())? else {
            return Err(DecisionError::Source);
        };
        if map.as_ref() != reading.map() {
            return Err(DecisionError::Source);
        }
        let [input] = reading.family().entries() else {
            return Err(DecisionError::Source);
        };
        let [output] = reading.entries() else {
            return Err(DecisionError::Source);
        };
        input.check(output)?;
        original.descriptor().compatible_kind(output.descriptor())?;
        if output.descriptor().role() != Role::Difference
            || output.descriptor().measure() != &Measure::coherent()
            || output.magnitude() != &ExactConstant::one()
            || output.shift() != &ExactConstant::zero()
        {
            return Err(DecisionError::Source);
        }
        bytes = bytes.saturating_add(size);
    }
    Ok(bytes)
}

/// Exact coefficient payloads and fixed source metadata remain owned by the retained reading.
fn metadata(reading: &PhysicalReading) -> u64 {
    reading
        .map()
        .coefficients()
        .iter()
        .chain(reading.map().offsets())
        .fold(512_u64, |total, value| {
            total.saturating_add(storage::rational_bytes(value))
        })
}

/// A valid mathematical map must still name the identical declared physical reading.
pub(super) fn authenticate(
    source: &DecisionExpression,
    value: &ScalarKernelReading,
    budget: &mut Budget,
) -> Result<(), DecisionError> {
    for (_, reading) in source.input_readings() {
        budget.work(metadata(reading), storage::AT)?;
        if reading.node() == &value.definition
            && reading.map().coefficients() == std::slice::from_ref(&value.scale)
            && reading.map().offsets() == std::slice::from_ref(&value.offset)
        {
            return Ok(());
        }
    }
    Err(DecisionError::Source)
}
