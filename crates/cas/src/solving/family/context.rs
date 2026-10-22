// SPDX-License-Identifier: AGPL-3.0-only
//! Local family contexts retain the child's exact base stratum and dependency edges.

use super::invalid;
use crate::error::Fault;
use crate::graph::{Edge, LogicalId, ScalarEdge, State};
use crate::solving::LiesOver;

/// A family meet may narrow its base but may never change the original family.
pub(crate) fn projection(
    state: &State,
    parent: LogicalId,
    child: LogicalId,
    edges: &mut Vec<(LogicalId, Edge)>,
) -> Result<Option<LiesOver<LogicalId>>, Fault> {
    let enclosing = state
        .region(parent)?
        .lies_over
        .as_ref()
        .ok_or_else(invalid)?;
    let selected = state.region(child)?.lies_over.as_ref().unwrap_or(enclosing);
    if selected.family != enclosing.family {
        return Err(invalid());
    }
    edges
        .try_reserve(2)
        .map_err(|_| Fault::allocation(super::AT))?;
    edges.push((selected.family, Edge::Scalar(ScalarEdge::Family)));
    edges.push((selected.stratum, Edge::Scalar(ScalarEdge::BaseStratum)));
    Ok(Some(selected.clone()))
}
