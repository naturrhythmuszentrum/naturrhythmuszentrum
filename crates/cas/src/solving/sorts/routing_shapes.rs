// SPDX-License-Identifier: AGPL-3.0-only
//! Reconstruct local scalar claims without traversing any subtree a second time.

use super::FamilyArgumentRole;
use super::routing::invalid;
use crate::budget::Budget;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Expression, Handle, Vertex, size};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::solving::SolveDraft;

/// Location for an inconsistent retained ambient edge.
const AT: Location = Location::new("declared operator signature");

/// Compare ordinary arithmetic edges against the operator's registered input/output spaces.
pub(super) fn expression(cas: &Cas, value: &Expression, budget: &mut Budget) -> Result<(), Fault> {
    let Some(ambient) = value.ambient else {
        return Ok(());
    };
    let component = matches!(value.operator, Operator::RealPart | Operator::ImagPart);
    let arithmetic = matches!(
        value.operator,
        Operator::Neg
            | Operator::Add
            | Operator::Sub
            | Operator::Mul
            | Operator::Div
            | Operator::IntegerPower(_)
            | Operator::Conjugate
    );
    if !component && !arithmetic {
        return Ok(());
    }
    if component && value.sort != Sort::Value(AtomicSort::Scalar(Field::Real)) {
        return Err(invalid());
    }
    for operand in &value.operands {
        budget.work(1, AT)?;
        let child = cas.state.expression(*operand)?;
        let valid = if component {
            child.sort == Sort::Value(AtomicSort::Scalar(Field::Complex))
        } else {
            child.ambient == Some(ambient)
        };
        if !valid {
            return Err(Fault::invalid(InvalidCode::AmbientSpaceMismatch, AT));
        }
    }
    Ok(())
}

/// Inspect local roles and ordered map slots using their actual registered references.
pub(super) fn solve(
    cas: &Cas,
    value: &SolveDraft<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    match value {
        SolveDraft::CoordinateSpace(value) => {
            if value.coordinates.len() != value.scalar_spaces.len() {
                return Err(invalid());
            }
            for (slot, claimed) in value.coordinates.iter().zip(&value.scalar_spaces) {
                budget.work(size(slot.key.len(), AT)?.saturating_add(1), AT)?;
                let expected = super::ambient(
                    &cas.state,
                    value.header.logical_id(),
                    &Sort::Value(AtomicSort::Scalar(slot.field.clone())),
                )?;
                if claimed.as_ref().map(Handle::logical_id) != expected {
                    return Err(invalid());
                }
            }
        }
        SolveDraft::CoefficientMap(value) => mapping(cas, value, budget)?,
        SolveDraft::SpacePlacement(value) => super::registry::placement(&cas.state, value)?,
        SolveDraft::ScalarPiece(value) => super::incidence::validate(&cas.state, value)?,
        SolveDraft::FamilyExpression(value) => family(cas, value, budget)?,
        SolveDraft::CoefficientEvaluation(value) => {
            super::admission::check_evaluation(&cas.state, value, budget)?;
        }
        _ => {}
    }
    Ok(())
}

/// Local family bodies preserve the distinct Base, Fibre and nested Family edges.
fn family(
    cas: &Cas,
    value: &super::FamilyExpressionData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let family = super::admission::family(&cas.state, &value.family)?;
    super::admission::projection_matches(&cas.state, &value.region, &value.family, None)?;
    let output = super::field(&cas.state, &value.ambient)?;
    if value.result != Sort::Value(AtomicSort::Scalar(output)) {
        return Err(invalid());
    }
    for argument in &value.arguments {
        budget.work(1, AT)?;
        match (argument.role, cas.state.node(argument.value.logical_id())?) {
            (FamilyArgumentRole::Base, Vertex::Expression(term))
                if term.sort == Sort::Value(AtomicSort::Scalar(Field::Real)) => {}
            (FamilyArgumentRole::Fibre, Vertex::Expression(term))
                if term.ambient == Some(family.fibre_space.logical_id()) => {}
            (FamilyArgumentRole::Family, Vertex::Solve(term)) => {
                let SolveDraft::FamilyExpression(term) = term.as_ref() else {
                    return Err(invalid());
                };
                if term.family != value.family
                    || term.region != value.region
                    || term.scope != value.scope
                {
                    return Err(invalid());
                }
            }
            _ => return Err(invalid()),
        }
    }
    Ok(())
}

/// Source coordinate declarations remain exact even when a later map checker is unavailable.
fn mapping(
    cas: &Cas,
    value: &crate::solving::CoefficientMapData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    use crate::solving::{CoefficientDomainData, MapTarget};
    let MapTarget::ScalarAmbient { field, .. } = &value.target else {
        return Ok(());
    };
    let SolveDraft::CoefficientDomain(source) =
        crate::solving::graph::object(&cas.state, value.source.logical_id())?
    else {
        return Err(invalid());
    };
    let coordinates = match source {
        CoefficientDomainData::Fraction { coordinates, .. } => Some(
            crate::solving::graph::coordinates(&cas.state, coordinates.logical_id())?,
        ),
        _ => None,
    };
    for (position, image) in value.images.iter().enumerate() {
        budget.work(1, AT)?;
        let expression = cas.state.expression(image.logical_id())?;
        if expression.sort != Sort::Value(AtomicSort::Scalar(field.clone())) {
            return Err(invalid());
        }
        if let Some(slot) =
            coordinates.and_then(|coordinates| coordinates.coordinates.get(position))
            && expression.sort != Sort::Value(AtomicSort::Scalar(slot.field.clone()))
        {
            return Err(invalid());
        }
    }
    Ok(())
}
