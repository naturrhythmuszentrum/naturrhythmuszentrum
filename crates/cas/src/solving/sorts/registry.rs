// SPDX-License-Identifier: AGPL-3.0-only
//! Closed scalar specifications and exact directed placement definitions.

use super::{ScalarSpaceData, SpacePlacementData};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Handle, State};
use crate::model::Field;
use crate::solving::SolveDraft;

/// Location for bounded scalar-space validation failures.
const AT: Location = Location::new("scalar space registration");

/// Resolve declaration syntax to a registered specification, without a law supplied by a caller.
pub(crate) fn specification(value: &Field) -> Result<ScalarSpaceData, Fault> {
    let name = match value {
        Field::Real => "scalar.real",
        Field::Complex => "scalar.complex",
        _ => {
            return Err(Fault::invalid(
                InvalidCode::SpaceRegistrationUnavailable,
                AT,
            ));
        }
    };
    Ok(ScalarSpaceData {
        specification: name.to_owned(),
        version: 1,
    })
}

/// Validate one exact installed registry identity.
pub(super) fn validate(value: &ScalarSpaceData) -> Result<Field, Fault> {
    if value.specification.is_empty()
        || !value
            .specification
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || matches!(byte, b'.' | b'-'))
    {
        return Err(Fault::invalid(InvalidCode::Encoding, AT));
    }
    if value.version != 1 {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
    }
    match value.specification.as_str() {
        "scalar.real" => Ok(Field::Real),
        "scalar.complex" => Ok(Field::Complex),
        _ => Err(Fault::invalid(
            InvalidCode::SpaceRegistrationUnavailable,
            AT,
        )),
    }
}

/// Inspect a checked space node rather than treating a field syntax value as its identity.
pub(crate) fn field(state: &State, value: &Handle) -> Result<Field, Fault> {
    match crate::solving::graph::object(state, value.logical_id())? {
        SolveDraft::ScalarSpace(value) => validate(value),
        _ => Err(Fault::invalid(InvalidCode::Reference, AT)),
    }
}

/// There is one directed edge and no implicit identity, reverse or transitive placement.
pub(super) fn placement(state: &State, value: &SpacePlacementData<Handle>) -> Result<(), Fault> {
    if value.specification != "placement.real-in-complex" {
        return Err(Fault::invalid(InvalidCode::SpacePlacementUnavailable, AT));
    }
    if value.version != 1 {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
    }
    if field(state, &value.source)? != Field::Real
        || field(state, &value.ambient)? != Field::Complex
    {
        return Err(Fault::invalid(InvalidCode::AmbientSpaceMismatch, AT));
    }
    Ok(())
}

impl crate::graph::Cas {
    /// Intern the installed registry nodes before the header that refers to them.
    pub(crate) fn registered_scalar_spaces(
        &self,
        state: &mut State,
        pending: u64,
        budget: &mut crate::budget::Budget,
    ) -> Result<Vec<crate::graph::LogicalId>, Fault> {
        let mut result = Vec::new();
        result
            .try_reserve_exact(2)
            .map_err(|_| Fault::allocation(AT))?;
        for field in [Field::Real, Field::Complex] {
            result.push(self.admit_solve(
                state,
                &SolveDraft::ScalarSpace(specification(&field)?),
                &[],
                pending,
                budget,
            )?);
        }
        Ok(result)
    }

    /// Read the checked scalar ambient of an ordinary value, distinct from a distribution space.
    ///
    /// # Errors
    /// Refuses foreign handles and non-expression objects.
    pub fn scalar_ambient(&self, expression: &Handle) -> Result<Option<Handle>, Fault> {
        Ok(self
            .state
            .expression(self.checked(expression)?)?
            .ambient
            .map(|id| self.handle(id)))
    }
}

/// Resolve a value's requested field through the header's explicit registry nodes.
pub(crate) fn ambient(
    state: &State,
    header: crate::graph::LogicalId,
    sort: &crate::model::Sort,
) -> Result<Option<crate::graph::LogicalId>, Fault> {
    let crate::model::Sort::Value(crate::model::AtomicSort::Scalar(requested)) = sort else {
        return Ok(None);
    };
    let Ok(expected) = specification(requested) else {
        return Ok(None);
    };
    for id in &state.header(header)?.scalar_spaces {
        if let SolveDraft::ScalarSpace(actual) = crate::solving::graph::object(state, *id)?
            && *actual == expected
        {
            return Ok(Some(*id));
        }
    }
    Err(Fault::invalid(
        InvalidCode::SpaceRegistrationUnavailable,
        AT,
    ))
}

impl crate::graph::Cas {
    /// Every coordinate slot retains its exact declared registered node in the graph and wire.
    pub(crate) fn resolve_coordinate_spaces(
        &self,
        state: &State,
        draft: &mut SolveDraft<Handle>,
    ) -> Result<(), Fault> {
        let SolveDraft::CoordinateSpace(value) = draft else {
            return Ok(());
        };
        let mut resolved = Vec::new();
        resolved
            .try_reserve_exact(value.coordinates.len())
            .map_err(|_| Fault::allocation(AT))?;
        for coordinate in &value.coordinates {
            let sort = crate::model::Sort::Value(crate::model::AtomicSort::Scalar(
                coordinate.field.clone(),
            ));
            resolved
                .push(ambient(state, value.header.logical_id(), &sort)?.map(|id| self.handle(id)));
        }
        if value.scalar_spaces.is_empty() {
            value.scalar_spaces = resolved;
        } else if value.scalar_spaces != resolved {
            return Err(Fault::invalid(InvalidCode::AmbientSpaceMismatch, AT));
        }
        Ok(())
    }
}
