// SPDX-License-Identifier: AGPL-3.0-only
//! Family coefficient interpretation preserves every original coordinate declaration.

use super::{AT, invalid};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle, State};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::solving::{CoefficientDomainData, CoefficientMapData, MapTarget, SolveDraft};

/// Borrow the declared codomain without inventing a scalar coercion.
pub(crate) fn scalar_target<'a>(
    state: &'a State,
    map: &'a CoefficientMapData<Handle>,
) -> Result<(&'a Handle, Field), Fault> {
    match &map.target {
        MapTarget::ScalarAmbient { header, field } => Ok((header, field.clone())),
        MapTarget::Family { family } => {
            let shape = family_data(state, family)?;
            let base = base_data(state, &shape.base)?;
            if crate::solving::sorts::field(state, &shape.fibre_space)? != Field::Complex {
                return Err(invalid());
            }
            Ok((&base.header, Field::Complex))
        }
        MapTarget::CoefficientDomain { .. } => Err(invalid()),
    }
}

/// Existing scalar relation equality is unchanged; family relations retain their explicit target.
pub(crate) fn relation_target(
    state: &State,
    map: &CoefficientMapData<Handle>,
    header: &Handle,
    ambient: &Field,
) -> Result<bool, Fault> {
    if let MapTarget::Family { .. } = &map.target {
        let (actual, field) = scalar_target(state, map)?;
        return Ok(actual == header && &field == ambient);
    }
    Ok(map.target
        == (MapTarget::ScalarAmbient {
            header: header.clone(),
            field: ambient.clone(),
        }))
}

/// The full original parameter frame is the base followed by the fibre coefficients.
fn frame(state: &State, family: &Handle, coordinates: &Handle) -> Result<(), Fault> {
    let shape = family_data(state, family)?;
    let base = base_data(state, &shape.base)?;
    let original = crate::solving::graph::coordinates(state, base.coordinates.logical_id())?;
    let fibre = crate::solving::graph::coordinates(state, shape.coefficients.logical_id())?;
    let actual = crate::solving::graph::coordinates(state, coordinates.logical_id())?;
    if !actual
        .coordinates
        .iter()
        .eq(original.coordinates.iter().chain(&fibre.coordinates))
    {
        return Err(invalid());
    }
    Ok(())
}

/// Admission keeps source keys, own fields, exact family and exact projected region.
pub(crate) fn validate(
    state: &State,
    map: &CoefficientMapData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let MapTarget::Family { family } = &map.target else {
        return Err(invalid());
    };
    let (header, _) = scalar_target(state, map)?;
    let region = state.region(map.region.logical_id())?;
    if region.header != header.logical_id()
        || super::region::family_state(state, map.region.logical_id(), budget)?
            != family.logical_id()
    {
        return Err(invalid());
    }
    let domain = crate::solving::coefficients::storage::domain(state, &map.source)?;
    let coordinates = if let CoefficientDomainData::Fraction { coordinates, .. } = domain {
        frame(state, family, coordinates)?;
        Some(crate::solving::graph::coordinates(
            state,
            coordinates.logical_id(),
        )?)
    } else {
        None
    };
    for (index, image) in map.images.iter().enumerate() {
        budget.work(1, AT)?;
        let expression = state.expression(image.logical_id())?;
        let field = coordinates
            .and_then(|value| value.coordinates.get(index))
            .map_or(Field::Complex, |value| value.field.clone());
        if expression.header != region.header
            || expression.scope != region.scope
            || expression.sort != Sort::Value(AtomicSort::Scalar(field))
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Canonical parameter maps use exact ordered keys, with no retagging or sampled image.
pub(crate) fn parameters(
    cas: &Cas,
    map: &CoefficientMapData<Handle>,
    coordinates: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    parameters_state(&cas.state, map, coordinates, budget)
}

/// Guard recognition shares the exact ordered identity map without claiming projection evidence.
pub(crate) fn parameters_state(
    state: &State,
    map: &CoefficientMapData<Handle>,
    coordinates: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    validate(state, map, budget)?;
    let product = crate::solving::graph::coordinates(state, coordinates.logical_id())?;
    if map.mode != crate::solving::MapMode::Specialization
        || product.coordinates.len() != map.images.len()
    {
        return Err(invalid());
    }
    for (coordinate, image) in product.coordinates.iter().zip(&map.images) {
        budget.work(crate::graph::size(coordinate.key.len(), AT)?, AT)?;
        if !matches!(&state.expression(image.logical_id())?.operator, Operator::Free(key) if key == &coordinate.key)
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// A ground family literal still uses only the registered rational/Gaussian scalar laws.
pub(crate) fn ground(
    cas: &Cas,
    map: &CoefficientMapData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    validate(&cas.state, map, budget)?;
    if !map.images.is_empty()
        || !matches!(
            crate::solving::coefficients::storage::domain(&cas.state, &map.source)?,
            CoefficientDomainData::Rational | CoefficientDomainData::Gaussian
        )
    {
        return Err(invalid());
    }
    Ok(())
}

/// Original family syntax remains the authority for role assignments.
pub(crate) fn family_data<'a>(
    state: &'a State,
    handle: &Handle,
) -> Result<&'a crate::solving::SpaceFamilyData<Handle>, Fault> {
    match crate::solving::graph::object(state, handle.logical_id())? {
        SolveDraft::SpaceFamily(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// Borrow the immutable Real parameter base.
pub(crate) fn base_data<'a>(
    state: &'a State,
    handle: &Handle,
) -> Result<&'a crate::solving::ParameterBaseData<Handle>, Fault> {
    match crate::solving::graph::object(state, handle.logical_id())? {
        SolveDraft::ParameterBase(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// A Real base map is interpreted only through the explicitly checked family projection.
pub(crate) fn base_map(
    evaluation: &crate::solving::elementary::evaluate::Evaluation<'_>,
    map: &CoefficientMapData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if matches!(map.target, MapTarget::Family { .. }) {
        return restrict_map(evaluation, map, budget);
    }
    if ground_restriction(evaluation, map)? {
        return Ok(());
    }
    if !matches!(
        map.target,
        MapTarget::ScalarAmbient {
            field: Field::Real,
            ..
        }
    ) {
        return Err(invalid());
    }
    super::restriction::source(
        evaluation.cas,
        evaluation.region,
        &map.region,
        evaluation.premises,
        budget,
    )
}

/// Restriction preserves the exact family map and its original checked support.
pub(crate) fn restrict_map(
    evaluation: &crate::solving::elementary::evaluate::Evaluation<'_>,
    map: &CoefficientMapData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let MapTarget::Family { family } = &map.target else {
        return Err(invalid());
    };
    if super::region::family(evaluation.cas, evaluation.region.logical_id(), budget)?
        != family.logical_id()
        || !super::region::subset(
            evaluation.cas,
            evaluation.region.logical_id(),
            map.region.logical_id(),
            budget,
        )?
    {
        return Err(invalid());
    }
    super::region::projection(evaluation.cas, &map.region, evaluation.premises, budget)
}

/// A global ground Complex constant has no base or fibre parameter dependencies.
fn ground_restriction(
    evaluation: &crate::solving::elementary::evaluate::Evaluation<'_>,
    map: &CoefficientMapData<Handle>,
) -> Result<bool, Fault> {
    let actual = evaluation
        .cas
        .state
        .region(evaluation.region.logical_id())?;
    let original = evaluation.cas.state.region(map.region.logical_id())?;
    let missing_family = actual.lies_over.is_none();
    let conditional_constant = original.lies_over.is_some();
    if missing_family
        || conditional_constant
        || actual.header != original.header
        || actual.scope != original.scope
        || actual.ambient != original.ambient
    {
        return Ok(false);
    }
    Ok(matches!(
        original.expression,
        crate::domain_model::RegionExpr::Ambient
    ) && matches!(
        map.target,
        MapTarget::ScalarAmbient {
            field: Field::Complex,
            ..
        }
    ) && map.images.is_empty()
        && map.denominators.is_empty()
        && matches!(
            crate::solving::coefficients::storage::domain(&evaluation.cas.state, &map.source)?,
            CoefficientDomainData::Rational | CoefficientDomainData::Gaussian
        ))
}
