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
    reason = "radical construction dispatches only explicit family regions"
)]
pub(crate) mod construction;
#[expect(
    clippy::redundant_pub_crate,
    reason = "local contexts preserve the exact selected family stratum"
)]
pub(crate) mod context;
#[expect(
    clippy::redundant_pub_crate,
    reason = "scalar evaluation dispatches explicit family bodies"
)]
pub(crate) mod evaluate;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared coefficient consumers dispatch only the explicit family target"
)]
pub(crate) mod mapping;
#[expect(
    clippy::redundant_pub_crate,
    reason = "native case admission validates placement propositions"
)]
pub(crate) mod membership;
#[expect(
    clippy::redundant_pub_crate,
    reason = "scalar parameter readers dispatch only explicit family regions and maps"
)]
pub(crate) mod parameters;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the mixed polynomial reader retains native family source syntax"
)]
pub(crate) mod polynomial;
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

#[expect(
    clippy::redundant_pub_crate,
    reason = "the mixed route constructs complete algebraic cubic pieces"
)]
pub(crate) mod algebraic;
#[expect(
    clippy::redundant_pub_crate,
    reason = "family coverage preserves repeated factor multiplicity"
)]
pub(crate) mod coverage;

#[expect(
    clippy::redundant_pub_crate,
    reason = "family root construction retains base-local Real certificates"
)]
pub(crate) mod real_roots;

mod root_selection;

#[expect(
    clippy::redundant_pub_crate,
    reason = "the public solving facade exports the expanded family cover recipe"
)]
pub(crate) mod cover;
