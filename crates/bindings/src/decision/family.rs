// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit physical source correspondence precedes every conditional numerical trace.

mod reading;

use super::{DecisionError, DecisionExpression, IntegerDecisionSpec, storage};
use crate::{CheckedDeclarations, Declaration, Kind, Role};
use nrz_cas::{
    Cas, ExactConstant, Handle,
    decision::{EnclosureOrigin, RationalEnclosure, ScalarKernelFamily, ScalarKernelLaw},
    limits::Budget,
    syntax::Operator,
};

/// Immutable source boxes keyed by original declaration within one frozen physical owner.
#[derive(Debug)]
pub(super) struct Inputs {
    /// Scope in which every qualified endpoint assumption is available.
    pub(super) context: Handle,
    /// Complete canonical original input inventory and its explicit mathematical correspondence.
    pub(super) families: Vec<(String, ScalarKernelFamily)>,
    /// Original affine definitions and physical coordinate meanings for mapped operands.
    pub(super) readings: Vec<(String, crate::readings::PhysicalReading)>,
}

impl Inputs {
    /// Authenticate every varying original and prepay complete retained metadata before publishing.
    pub(super) fn check(
        cas: &Cas,
        owner: &CheckedDeclarations,
        name: &str,
        context: &Handle,
        inputs: Vec<(String, ScalarKernelFamily)>,
        readings: Vec<(String, crate::readings::PhysicalReading)>,
        budget: &mut Budget,
    ) -> Result<Self, DecisionError> {
        let original = owner.checked(name)?;
        let descriptor = original.descriptor();
        if descriptor.kind() != &Kind::Ratio
            || descriptor.role() != Role::Difference
            || inputs.is_empty()
        {
            return Err(DecisionError::Source);
        }
        original_order(owner, name, budget)?;
        let mut count = 0;
        for (key, declaration) in original.declarations() {
            budget.work(1, storage::AT)?;
            if matches!(declaration, Declaration::Input { .. }) {
                let Some((actual, family)) = inputs.get(count) else {
                    return Err(DecisionError::Source);
                };
                if key != actual {
                    return Err(DecisionError::Source);
                }
                let descriptor = declaration.descriptor().ok_or(DecisionError::Source)?;
                if descriptor.role() != Role::Difference
                    || descriptor.kind().count_domain().is_some()
                {
                    return Err(DecisionError::Source);
                }
                cas.check_scalar_kernel_family(context, family, budget)?;
                count += 1;
            }
        }
        if count != inputs.len() {
            return Err(DecisionError::Source);
        }
        let mut bytes =
            256_u64.saturating_add(u64::try_from(name.len()).map_err(|_| DecisionError::Capacity)?);
        for (name, family) in &inputs {
            bytes = bytes
                .saturating_add(u64::try_from(name.len()).map_err(|_| DecisionError::Capacity)?)
                .saturating_add(256)
                .saturating_add(storage::rational_bytes(&family.lower))
                .saturating_add(storage::rational_bytes(&family.upper));
        }
        bytes = bytes.saturating_add(reading::check(
            cas, owner, context, &inputs, &readings, budget,
        )?);
        storage::retained(bytes, budget)?;
        budget.work(bytes, storage::AT)?;
        Ok(Self {
            context: context.clone(),
            families: inputs,
            readings,
        })
    }
}

/// Only the original ordered quotient of two distinct declarations is admitted in this subset.
fn original_order<'a>(
    owner: &'a CheckedDeclarations,
    name: &str,
    budget: &mut Budget,
) -> Result<[&'a str; 2], DecisionError> {
    let Some(Declaration::Derived(observable)) = owner.declarations().get(name) else {
        return Err(DecisionError::Source);
    };
    let graph = owner.cas()?;
    budget.work(4, storage::AT)?;
    if graph.expression(&observable.expression.syntax)?.0 != &Operator::Div {
        return Err(DecisionError::Source);
    }
    let operands = graph.operands(&observable.expression.syntax, budget)?;
    let [first, second] = operands.as_slice() else {
        return Err(DecisionError::Source);
    };
    let name = |operand: &Handle| -> Result<&'a str, DecisionError> {
        let (Operator::Free(name), _) = graph.expression(operand)? else {
            return Err(DecisionError::Source);
        };
        let Some((key, Declaration::Input { .. } | Declaration::Constant { .. })) =
            owner.declarations().get_key_value(name)
        else {
            return Err(DecisionError::Source);
        };
        Ok(key.as_str())
    };
    let names = [name(first)?, name(second)?];
    drop(graph);
    let [first, second] = names;
    if first == second {
        return Err(DecisionError::Source);
    }
    Ok([first, second])
}

/// Original physical operands map exactly to their explicit CAS sources or coherent constants.
pub(super) fn correspondence(
    cas: &Cas,
    source: &DecisionExpression,
    spec: &IntegerDecisionSpec,
    budget: &mut Budget,
) -> Result<(), DecisionError> {
    if source.family_context() != Some(&spec.context) {
        return Err(DecisionError::Case);
    }
    let names = original_order(source.owner(), source.name(), budget)?;
    if cas.expression(&spec.target)?.0 != &Operator::Div {
        return Err(DecisionError::Target);
    }
    let operands = cas.operands(&spec.target, budget)?;
    if operands.len() != 2 {
        return Err(DecisionError::Target);
    }
    for (name, target) in names.iter().zip(&operands) {
        budget.work(1, storage::AT)?;
        if let Some((_, family)) = source.input_families().iter().find(|(key, _)| key == *name) {
            let mapped = source.input_readings().iter().find(|(key, _)| key == *name);
            let expected = mapped.map_or(&family.source, |(_, reading)| reading.node());
            if expected != target {
                return Err(DecisionError::Target);
            }
            cas.check_scalar_kernel_family(&spec.context, family, budget)?;
        } else {
            closed_operand(cas, source, name, target, budget)?;
        }
    }
    Ok(())
}

/// Preserve the original measure when checking a closed companion of a varying source.
fn closed_operand(
    cas: &Cas,
    source: &DecisionExpression,
    name: &str,
    target: &Handle,
    budget: &mut Budget,
) -> Result<(), DecisionError> {
    let Some(Declaration::Constant {
        descriptor, value, ..
    }) = source.owner().declarations().get(name)
    else {
        return Err(DecisionError::Source);
    };
    let difference = descriptor.role() == Role::Difference;
    let zero_offset = descriptor.measure().offset().is_zero();
    if !(difference && zero_offset) {
        return Err(DecisionError::Source);
    }
    let expected = descriptor
        .measure()
        .scale()
        .mul(&value.clone().into(), budget)?;
    let actual = ExactConstant::from_expression(cas, target, budget)?;
    budget.work(
        storage::constant_bytes(&actual).max(storage::constant_bytes(&expected)),
        storage::AT,
    )?;
    if actual != expected {
        return Err(DecisionError::Target);
    }
    Ok(())
}

/// A valid mathematical witness must still retain the identical complete physical source boxes.
pub(super) fn authenticate(
    source: &DecisionExpression,
    enclosure: &RationalEnclosure,
    budget: &mut Budget,
) -> Result<(), DecisionError> {
    if source.input_families().is_empty() {
        return Ok(());
    }
    let EnclosureOrigin::ScalarKernel(certificate) = &enclosure.origin else {
        return Err(DecisionError::Source);
    };
    let mut count = 0;
    let mut mapped = 0;
    for record in &certificate.records {
        budget.work(1, storage::AT)?;
        if let ScalarKernelLaw::Reading(value) = &record.law {
            reading::authenticate(source, value, budget)?;
            mapped += 1;
        }
        if let ScalarKernelLaw::Family(input) = &record.law {
            budget.work(
                u64::try_from(source.input_families().len())
                    .map_err(|_| DecisionError::Capacity)?,
                storage::AT,
            )?;
            if !source
                .input_families()
                .iter()
                .any(|(_, expected)| expected == &input.family)
            {
                return Err(DecisionError::Source);
            }
            count += 1;
        }
    }
    if count != source.input_families().len() || mapped != source.input_readings().len() {
        return Err(DecisionError::Source);
    }
    Ok(())
}
