// SPDX-License-Identifier: AGPL-3.0-only
//! Independent coefficient interpretation through an explicitly checked quotient specialization.

use super::arithmetic::Engine;
use super::model::{CoefficientFailure, Element};
use super::storage::{invalid, value};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::solving::{CoefficientDomainData, CoefficientMapData, MapMode, MapTarget, SolveDraft};

/// Existing map routes retain their source algebra; quotient specializations calculate in the target.
pub(crate) fn domain<'a>(
    cas: &Cas,
    map: &'a CoefficientMapData<Handle>,
) -> Result<&'a Handle, Fault> {
    if let MapTarget::CoefficientDomain { domain: target } = &map.target
        && map.mode == MapMode::Specialization
        && !map.images.is_empty()
        && matches!(
            super::storage::domain(&cas.state, &map.source)?,
            CoefficientDomainData::Quotient { .. }
        )
    {
        return Ok(target);
    }
    Ok(&map.source)
}

/// Source values cross the checked map; target values are read directly in their own algebra.
pub(crate) fn coefficient(
    cas: &Cas,
    map: &CoefficientMapData<Handle>,
    engine: &Engine<'_>,
    handle: &Handle,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    let stored = value(&cas.state, handle)?;
    if stored.domain == *engine.domain {
        return engine.read(handle, budget);
    }
    if stored.domain != map.source || domain(cas, map)? != engine.domain {
        return Err(invalid().into());
    }
    let source = Engine {
        state: &cas.state,
        domain: &map.source,
    };
    let element = source.read(handle, budget)?;
    super::maps::quotient(&cas.state, &element, map, engine.domain, budget)
}

/// Transport preserves the complete ordered polynomial frame, including unused slots and sorts.
pub(crate) fn frames(
    cas: &Cas,
    map: &CoefficientMapData<Handle>,
    certificate: &crate::solving::CertificateCoefficientIdentity,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if domain(cas, map)? == &map.source {
        return Ok(());
    }
    let mut frame = None;
    let endpoints = [&certificate.left, &certificate.right].into_iter();
    let combinations = certificate
        .combinations
        .iter()
        .flat_map(|entry| [&entry.multiplier, &entry.relation]);
    for handle in endpoints.chain(combinations) {
        budget.work(1, super::storage::AT)?;
        if handle.logical_id().kind == crate::graph::VertexKind::Expression {
            continue;
        }
        if let SolveDraft::Polynomial(polynomial) =
            crate::solving::graph::object(&cas.state, cas.checked(handle)?)?
        {
            budget.work(
                crate::graph::size(polynomial.variables.len(), super::storage::AT)?,
                super::storage::AT,
            )?;
            if frame.is_some_and(|previous| previous != &polynomial.variables) {
                return Err(invalid());
            }
            frame = Some(&polynomial.variables);
        }
    }
    Ok(())
}

/// A strict coefficient reciprocal requires both canonical factors to be nonzero locally.
/// Original expression readers call this before cancellation can remove a divisor.
pub(crate) fn reciprocal(
    engine: &Engine<'_>,
    element: &Element,
    region: &Handle,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    use super::model::Value;
    if !nonzero(engine, element, region, &[], budget)? {
        return Err(CoefficientFailure::Unsupported(
            crate::solving::SolveReason::MissingUnitProof,
        ));
    }
    let value = match &element.value {
        Value::Scalar(value) => Value::Scalar(
            super::scalar::Scalar::constant(value.ground(), true, budget)?.div(value, budget)?,
        ),
        Value::Fraction(value) => Value::Fraction(value.reciprocal(budget)?),
        Value::Tower(_) => {
            return Err(CoefficientFailure::Unsupported(
                crate::solving::SolveReason::CoefficientDomainUnavailable,
            ));
        }
    };
    Ok(engine.element(value))
}

/// Existing regional factor evidence checks a constant form's complete fraction domain.
pub(crate) fn nonzero(
    engine: &Engine<'_>,
    element: &Element,
    region: &Handle,
    premises: &[crate::proof_model::CheckedApplication],
    budget: &mut Budget,
) -> Result<bool, Fault> {
    super::storage::begin(budget)?;
    if element.domain != *engine.domain {
        return Err(invalid());
    }
    match &element.value {
        super::model::Value::Scalar(value) => Ok(!value.zero(budget)?),
        super::model::Value::Fraction(value) => {
            for polynomial in [&value.denominator, &value.numerator] {
                if !super::nonzero::proves(
                    engine.state,
                    engine.domain,
                    polynomial,
                    region,
                    premises.iter().map(|premise| &premise.judgment.statement),
                    budget,
                )? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        super::model::Value::Tower(_) => Ok(false),
    }
}
