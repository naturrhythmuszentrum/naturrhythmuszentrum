// SPDX-License-Identifier: AGPL-3.0-only
//! Local Real-in-Complex operations and expanded native case recipes.

#[expect(
    clippy::redundant_pub_crate,
    reason = "native cases share the family interpreter"
)]
pub(crate) mod components;
#[expect(
    clippy::redundant_pub_crate,
    reason = "native cases reconstruct their existing payload"
)]
pub(crate) mod native;
#[expect(
    clippy::redundant_pub_crate,
    reason = "region constructors share exact family projection checks"
)]
pub(crate) mod region;

mod build;
#[expect(
    clippy::redundant_pub_crate,
    reason = "native case admission validates placement propositions"
)]
pub(crate) mod membership;
mod query;
#[expect(
    clippy::redundant_pub_crate,
    reason = "native request scheduling shares the expanded family recipes"
)]
pub(crate) mod recipes;
#[expect(
    clippy::redundant_pub_crate,
    reason = "context weakening dispatches explicit family pullbacks"
)]
pub(crate) mod restriction;

use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, LogicalId, Vertex};
use crate::solving::{FamilyExpressionData, SolveDraft};

/// Attribution of bounded placement evaluation.
pub(crate) const AT: Location = Location::new("local family evaluation");

/// No missing incidence or projection is an implicit coercion.
pub(crate) const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Rule, AT)
}

/// Borrow only the new local body shape.
pub(crate) fn body(
    cas: &Cas,
    id: LogicalId,
) -> Result<Option<&FamilyExpressionData<crate::graph::Handle>>, Fault> {
    match cas.state.node(id)? {
        Vertex::Solve(value) => match value.as_ref() {
            SolveDraft::FamilyExpression(value) => Ok(Some(value)),
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}

/// Native family cases have family bodies in every association.
pub(crate) fn local(cas: &Cas, id: LogicalId) -> Result<bool, Fault> {
    if body(cas, id)?.is_some() {
        return Ok(true);
    }
    if let Vertex::Expression(value) = cas.state.node(id)?
        && let Some(first) = value.arms.first()
    {
        return Ok(body(cas, cas.state.arm(*first)?.value)?.is_some());
    }
    Ok(false)
}
