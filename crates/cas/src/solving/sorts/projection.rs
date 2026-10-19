// SPDX-License-Identifier: AGPL-3.0-only
//! Exact family projection references on ordinary total regions.

use super::LiesOver;
use super::admission::{base, family, invalid};
use crate::domain_model::AmbientDomain;
use crate::error::Fault;
use crate::graph::{Cas, Edge, LogicalId, Reference, ScalarEdge, State};

/// Resolve the acyclic family/base links and check the total coordinate product.
pub(crate) fn projection(
    cas: &Cas,
    state: &State,
    draft: Option<&LiesOver<Reference>>,
    header: LogicalId,
    ambient: &AmbientDomain,
    locals: &[LogicalId],
    edges: &mut Vec<(LogicalId, Edge)>,
) -> Result<Option<LiesOver<LogicalId>>, Fault> {
    let Some(draft) = draft else {
        return Ok(None);
    };
    let family_id = cas.resolve(state, &draft.family, locals)?;
    let stratum = cas.resolve(state, &draft.stratum, locals)?;
    let family_handle = cas.handle(family_id);
    let shape = family(state, &family_handle)?;
    let original = base(state, &shape.base)?;
    let stratum_data = state.region(stratum)?;
    let domain = state.region(original.domain.logical_id())?;
    if original.header.logical_id() != header
        || stratum_data.header != header
        || stratum_data.ambient != domain.ambient
        || stratum_data.scope != domain.scope
        || stratum_data.lies_over.is_some()
    {
        return Err(invalid());
    }
    product(state, ambient, original, shape)?;
    edges
        .try_reserve(2)
        .map_err(|_| Fault::allocation(super::admission::AT))?;
    edges.push((family_id, Edge::Scalar(ScalarEdge::Family)));
    edges.push((stratum, Edge::Scalar(ScalarEdge::BaseStratum)));
    Ok(Some(LiesOver {
        family: family_id,
        stratum,
    }))
}

/// Match the total product to the original base followed by fibre coefficients.
fn product(
    state: &State,
    ambient: &AmbientDomain,
    original: &super::ParameterBaseData<crate::graph::Handle>,
    shape: &super::SpaceFamilyData<crate::graph::Handle>,
) -> Result<(), Fault> {
    let AmbientDomain::Product { coordinates } = ambient else {
        return Err(invalid());
    };
    let total = crate::solving::graph::coordinates(state, coordinates.logical_id())?;
    let base = crate::solving::graph::coordinates(state, original.coordinates.logical_id())?;
    let fibre = crate::solving::graph::coordinates(state, shape.coefficients.logical_id())?;
    if !total
        .coordinates
        .iter()
        .eq(base.coordinates.iter().chain(&fibre.coordinates))
    {
        return Err(invalid());
    }
    Ok(())
}
