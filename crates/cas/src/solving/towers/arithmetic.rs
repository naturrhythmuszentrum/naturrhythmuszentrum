// SPDX-License-Identifier: AGPL-3.0-only
//! Finite quotient arithmetic: parent operations precede descending monic reduction.

use super::dense::Dense;
use crate::budget::{Budget, BudgetKind};
use crate::graph::Handle;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::model::{CoefficientFailure, Element, Value};
use crate::solving::coefficients::storage::{AT, begin, domain, invalid, vector};
use crate::solving::{CoefficientDomainData, ExtensionLevelData, SolveDraft};

/// Borrow an earlier monic extension level, without claiming irreducibility or reducedness.
pub(crate) fn level<'a>(
    engine: &Engine<'a>,
) -> Result<(&'a Handle, &'a ExtensionLevelData<Handle>), CoefficientFailure> {
    let CoefficientDomainData::Quotient { level } = domain(engine.state, engine.domain)? else {
        return Err(invalid().into());
    };
    let SolveDraft::ExtensionLevel(data) =
        crate::solving::graph::object(engine.state, level.logical_id())?
    else {
        return Err(invalid().into());
    };
    Ok((level, data))
}

/// A constant enters the parent first; zero has the empty reduced representation.
pub(crate) fn constant(
    engine: &Engine<'_>,
    nonzero: bool,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    begin(budget)?;
    let (_, extension) = level(engine)?;
    let parent = Engine {
        state: engine.state,
        domain: &extension.parent,
    };
    let mut coefficients = vector(usize::from(nonzero), budget)?;
    if nonzero {
        coefficients.push(parent.constant(true, budget)?);
    }
    Ok(engine.element(Value::Tower(coefficients)))
}

/// Import graph coefficients recursively in parent-before-child order.
pub(crate) fn read(
    engine: &Engine<'_>,
    handles: &[Handle],
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    begin(budget)?;
    let (_, extension) = level(engine)?;
    let parent = Engine {
        state: engine.state,
        domain: &extension.parent,
    };
    let mut coefficients = vector(handles.len(), budget)?;
    for handle in handles {
        coefficients.push(parent.read(handle, budget)?);
    }
    reduced(engine, Dense::new(&parent, coefficients, budget)?, budget)
}

/// Copy one reduced vector without copying graph state.
pub(crate) fn dense(
    engine: &Engine<'_>,
    operand: &Element,
    budget: &mut Budget,
) -> Result<Dense, CoefficientFailure> {
    begin(budget)?;
    if operand.domain != *engine.domain {
        return Err(invalid().into());
    }
    let Value::Tower(entries) = &operand.value else {
        return Err(invalid().into());
    };
    let (_, extension) = level(engine)?;
    let parent = Engine {
        state: engine.state,
        domain: &extension.parent,
    };
    let mut coefficients = vector(entries.len(), budget)?;
    for entry in entries {
        coefficients.push(parent.copy(entry, budget)?);
    }
    Dense::new(&parent, coefficients, budget)
}

/// Descending monic reduction requires no parent inverses and strictly decreases degree.
pub(crate) fn remainder(
    mut value: Dense,
    modulus: &Dense,
    engine: &Engine<'_>,
    budget: &mut Budget,
) -> Result<Dense, CoefficientFailure> {
    let degree = modulus
        .coefficients
        .len()
        .checked_sub(1)
        .ok_or_else(invalid)?;
    if degree == 0 || value.domain != modulus.domain {
        return Err(invalid().into());
    }
    let parent = Engine {
        state: engine.state,
        domain: &modulus.domain,
    };
    let unit = parent.constant(true, budget)?;
    let leading = modulus.coefficients.last().ok_or_else(invalid)?;
    if !parent.formal_equal(leading, &unit, budget)? {
        return Err(invalid().into());
    }
    while value.coefficients.len() > degree {
        budget.solving(BudgetKind::TowerReductions, 1, AT)?;
        let before = value.coefficients.len();
        let shift = before
            .checked_sub(modulus.coefficients.len())
            .ok_or_else(invalid)?;
        let coefficient = parent.copy(value.coefficients.last().ok_or_else(invalid)?, budget)?;
        value = value.sum(
            &modulus.scaled(&coefficient, shift, engine.state, budget)?,
            true,
            engine.state,
            budget,
        )?;
        if value.coefficients.len() >= before {
            return Err(invalid().into());
        }
    }
    Ok(value)
}

/// Normalize an arithmetic result against precisely its earlier defining polynomial.
pub(crate) fn reduced(
    engine: &Engine<'_>,
    value: Dense,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    begin(budget)?;
    let (_, extension) = level(engine)?;
    let modulus = Dense::read(engine.state, &extension.polynomial, budget)?;
    let result = remainder(value, &modulus, engine, budget)?;
    Ok(engine.element(Value::Tower(result.coefficients)))
}

/// Sum two vectors before reducing in the child algebra.
pub(crate) fn add(
    engine: &Engine<'_>,
    left: &Element,
    right: &Element,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    let result = dense(engine, left, budget)?.sum(
        &dense(engine, right, budget)?,
        false,
        engine.state,
        budget,
    )?;
    reduced(engine, result, budget)
}

/// Convolve vectors in the parent, then perform bounded monic cancellations.
pub(crate) fn mul(
    engine: &Engine<'_>,
    left: &Element,
    right: &Element,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    let result =
        dense(engine, left, budget)?.mul(&dense(engine, right, budget)?, engine.state, budget)?;
    reduced(engine, result, budget)
}

/// Sign changes are recursive parent operations.
pub(crate) fn neg(
    engine: &Engine<'_>,
    operand: &Element,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    let value = dense(engine, operand, budget)?;
    let parent = Engine {
        state: engine.state,
        domain: &value.domain,
    };
    let negative = parent.neg(&parent.constant(true, budget)?, budget)?;
    reduced(
        engine,
        value.scaled(&negative, 0, engine.state, budget)?,
        budget,
    )
}
