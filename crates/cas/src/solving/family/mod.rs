// SPDX-License-Identifier: AGPL-3.0-only
//! Local Real-in-Complex operations and expanded native case recipes.
//!
//! # Native family operations and checked regions
//!
//! `Cas::family_expression` builds a native incomplete case from a checked local operation, retaining every original operand and registered placement association. Component evaluation checks addition, subtraction, negation, multiplication, positive powers, conjugation and real/imaginary projection on the exact piece; reciprocals require actual regional nonvanishing before cancellation. A Real operand contributes its original Real value and zero imaginary component. An independent Complex operand retains both components. Native selection, equality, simplification and definedness requests use these same checked cases.
//!
//! `Cas::family_identity` and `Cas::family_agreement` expand into ordinary projection-subset and case-congruence applications. `Cas::family_restrict` uses explicit subset evidence and context weakening, including a predicate-for-predicate preimage when transporting a closed base-local equality or predicate to its total family region. The region annotation supplies no membership authority. Base domains and strata inspect the full original dependency closure, so a guard containing `RealPart(c)` for a Complex fibre coefficient cannot be classified as base-only. Restriction retains the exact theorem region and cannot widen a piece theorem.
//!
//! `Cas::placement_membership` takes a separate Complex query and the original placement instance. `PlacementOutcome::Inside` retains a same-Real local argument and ordinary equality witnesses for both incidence equations. `Refused(NoApplicablePiece)` retains the strict reciprocal equality proving a nonzero imaginary component; `Unresolved(PieceMembershipPending)` supplies neither a value nor an exclusion theorem. These are public outcomes derived from the existing proofs, with the placement dependencies retained in their native associations. They add no statement tag, certificate subtype, primitive or policy version. Each query shares one generation ledger and one checker ledger across its incidence and exclusion attempts.
//!
//! Mixed univariate solving uses the original Complex unknown space and keeps Real coefficients in their original base declarations. `MapTarget::Family` retains the exact family, ordered base/fibre coefficient frame and total region; its current wire tag is 2. Every family map is checked independently and needs explicit projection evidence. Linear and binomial pieces retain their existing exact presentations; general cubic pieces use exact algebraic selections, explicit equations for every returned root, and independently checked complete factor products. A stronger radical presentation remains a separate obligation. Leading coefficients and every actual divisor require regional unit evidence. The unchanged scalar routes retain their original maps and accounting.
//!
//! For `z²-r=0`, separate requests on `r<0`, `r=0` and `r>0` yield complete local pieces without a cover of the base. The zero piece retains one zero tuple and a factor of multiplicity two. Each nonzero piece retains two opposite roots. On the positive stratum both roots are fixed by standard conjugation; on the negative stratum an existing Real positive-square certificate for `-r` supplies the Real witness used in the imaginary components. That Real proof remains over its original base stratum and enters the total region only through an explicit checked projection.
//!
//! `Cas::family_cover` takes a `FamilyCoverRequest` with independently complete piece witnesses, a separate optional cover of the original base, and a separate optional fibre cover for each stratum group. `FamilyCoverResult` retains exact `FamilyPiece` descriptors and their coverage witnesses, including proved empty pieces. Missing base evidence or a missing stratum yields `BaseCoveragePending`; missing fibre evidence yields `FibreCoveragePending`. Once both inventories match, the recipe expands into existing region construction, context weakening and closure, and total-cover elimination. Omitting the zero stratum never produces whole-family coverage and leaves the supplied nonzero pieces valid. The recipe adds no primitive, certificate subtype or statement shape; `NRZCAS04` and logical policy 10 remain the sole current wire and policy.

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
