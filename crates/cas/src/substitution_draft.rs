// SPDX-License-Identifier: AGPL-3.0-only
//! Typed draft reconstruction for substitution and exact axis renaming.

use crate::domain_model::{
    BoundaryDraft, CaseArmDraft, CasesDraft, RegionDraft, RegionExpr, SpaceDraft,
};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Draft, ExpressionDraft, LogicalId, Reference, Vertex};
use crate::model::{Operator, Scope};
use crate::source_model::{SourceClauseDraft, SourceFormDraft};

/// Rebuild one completed node; all child references are already staged.
pub(crate) fn rebuild(
    cas: &Cas,
    id: LogicalId,
    scope: Scope,
    operator: Option<Operator>,
    resolve: &mut impl FnMut(LogicalId) -> Result<Reference, Fault>,
) -> Result<Draft, Fault> {
    let header = Reference::Existing(cas.handle(crate::descriptor::metadata(&cas.state, id)?.0));
    match cas.state.node(id)? {
        Vertex::Expression(v) => expression(v, header, scope, operator, resolve),
        Vertex::Solve(value) => solve(value, scope, resolve),
        Vertex::Condition(v) => Ok(Draft::Condition {
            header,
            scope,
            kind: v.kind.clone(),
            arguments: v
                .arguments
                .iter()
                .copied()
                .map(&mut *resolve)
                .collect::<Result<_, _>>()?,
        }),
        Vertex::Region(v) => region_draft(v, header, scope, resolve),
        Vertex::CaseArm(v) => Ok(Draft::CaseArm(CaseArmDraft {
            placements: v
                .placements
                .iter()
                .map(|pair| pair.resolve(&mut |id| resolve(*id)))
                .collect::<Result<_, _>>()?,
            header,
            scope,
            region: resolve(v.region)?,
            value: resolve(v.value)?,
        })),
        Vertex::Boundary(v) => Ok(Draft::Boundary(BoundaryDraft {
            header,
            scope,
            ambient: v.ambient.clone(),
            location: resolve(v.location)?,
        })),
        Vertex::Space(v) => Ok(Draft::Space(SpaceDraft {
            header,
            domain: resolve(v.domain)?,
            coordinate: v.coordinate.clone(),
            field: v.field.clone(),
            test_family: v.test_family.clone(),
        })),
        Vertex::SourceForm(v) => source(v, header, scope, resolve),
        _ => Err(invalid()),
    }
}

/// Native cases retain their typed domain and association links.
fn expression(
    v: &crate::graph::Expression,
    header: Reference,
    scope: Scope,
    operator: Option<Operator>,
    resolve: &mut impl FnMut(LogicalId) -> Result<Reference, Fault>,
) -> Result<Draft, Fault> {
    if let Some(attachment) = &v.attachment {
        return attached(attachment, &v.operator, header, resolve);
    }
    if let Some(domain) = v.domain {
        return Ok(Draft::Cases(CasesDraft {
            sort: v.sort.clone(),
            header,
            scope,
            domain: resolve(domain)?,
            arms: v
                .arms
                .iter()
                .copied()
                .map(&mut *resolve)
                .collect::<Result<_, _>>()?,
        }));
    }
    Ok(Draft::Expression(ExpressionDraft {
        space: v.space.map(&mut *resolve).transpose()?,
        header,
        scope,
        operator: operator.ok_or_else(invalid)?,
        sort: v.sort.clone(),
        binding: v.binding.clone(),
        operands: v
            .operands
            .iter()
            .copied()
            .map(&mut *resolve)
            .collect::<Result<_, _>>()?,
    }))
}

/// Preserve source order while rebuilding both sides of every association.
fn source(
    v: &crate::source_model::SourceForm,
    header: Reference,
    scope: Scope,
    resolve: &mut impl FnMut(LogicalId) -> Result<Reference, Fault>,
) -> Result<Draft, Fault> {
    let mut clauses = Vec::new();
    clauses
        .try_reserve_exact(v.clauses.len())
        .map_err(|_| Fault::allocation(Location::new("source substitution")))?;
    for (selector, value) in &v.clauses {
        clauses.push(SourceClauseDraft {
            selector: resolve(*selector)?,
            value: resolve(*value)?,
        });
    }
    Ok(Draft::SourceForm(SourceFormDraft {
        header,
        scope,
        kind: v.kind,
        domain: resolve(v.domain)?,
        clauses,
        otherwise: v.otherwise.map(resolve).transpose()?,
    }))
}

/// Rebuild set constructors without turning partial guard falsehood into complement.
pub(crate) fn region(
    value: &RegionExpr<LogicalId>,
    resolve: &mut impl FnMut(LogicalId) -> Result<Reference, Fault>,
) -> Result<RegionExpr, Fault> {
    Ok(match value {
        RegionExpr::Ambient => RegionExpr::Ambient,
        RegionExpr::Empty => RegionExpr::Empty,
        RegionExpr::GuardTrue { base, predicate } => RegionExpr::GuardTrue {
            base: resolve(*base)?,
            predicate: resolve(*predicate)?,
        },
        RegionExpr::GuardFalse { base, predicate } => RegionExpr::GuardFalse {
            base: resolve(*base)?,
            predicate: resolve(*predicate)?,
        },
        RegionExpr::Meet(values) => RegionExpr::Meet(
            values
                .iter()
                .copied()
                .map(&mut *resolve)
                .collect::<Result<_, _>>()?,
        ),
        RegionExpr::Union(values) => RegionExpr::Union(
            values
                .iter()
                .copied()
                .map(&mut *resolve)
                .collect::<Result<_, _>>()?,
        ),
        RegionExpr::Difference { base, excluded } => RegionExpr::Difference {
            base: resolve(*base)?,
            excluded: resolve(*excluded)?,
        },
    })
}

/// No draft may erase an unrecognized semantic dependency.
const fn invalid() -> Fault {
    Fault::invalid(
        InvalidCode::Reference,
        Location::new("typed substitution draft"),
    )
}

/// Rebuild attachments through their dedicated checked constructors.
fn attached(
    attachment: &crate::graph::ExpressionAttachment,
    operator: &Operator,
    header: Reference,
    resolve: &mut impl FnMut(LogicalId) -> Result<Reference, Fault>,
) -> Result<Draft, Fault> {
    Ok(match attachment {
        crate::graph::ExpressionAttachment::SelectedRoot(selection) => {
            let Operator::RootValue { slot } = operator else {
                return Err(invalid());
            };
            Draft::RootValue {
                header,
                selection: resolve(*selection)?,
                slot: *slot,
            }
        }
        crate::graph::ExpressionAttachment::ExactCoefficient { value, map } => {
            Draft::ExactCoefficient {
                header,
                value: resolve(*value)?,
                map: resolve(*map)?,
            }
        }
    })
}

/// A transformed total region keeps the exact family and its transformed base stratum.
fn region_draft(
    value: &crate::domain_model::Region,
    header: Reference,
    scope: Scope,
    resolve: &mut impl FnMut(LogicalId) -> Result<Reference, Fault>,
) -> Result<Draft, Fault> {
    let payload = RegionDraft {
        header,
        scope,
        ambient: value.ambient.clone(),
        expression: region(&value.expression, resolve)?,
    };
    if let Some(projection) = &value.lies_over {
        Ok(Draft::FamilyRegion {
            region: payload,
            lies_over: crate::solving::LiesOver {
                family: resolve(projection.family)?,
                stratum: resolve(projection.stratum)?,
            },
        })
    } else {
        Ok(Draft::Region(payload))
    }
}

/// Scoped solving syntax follows the same exact-sort substitution as its ordinary operands.
fn solve(
    value: &crate::solving::SolveDraft<crate::graph::Handle>,
    scope: Scope,
    resolve: &mut impl FnMut(LogicalId) -> Result<Reference, Fault>,
) -> Result<Draft, Fault> {
    use crate::solving::SolveDraft;
    let mut value = value.resolve(&mut |handle| {
        if handle.logical_id().kind == crate::graph::VertexKind::Header {
            Ok(Reference::Existing(handle.clone()))
        } else {
            resolve(handle.logical_id())
        }
    })?;
    match &mut value {
        SolveDraft::Tuple(value) => value.scope = scope,
        SolveDraft::FamilyMap(value) => value.scope = scope,
        SolveDraft::RootRelation(value) => value.scope = scope,
        SolveDraft::RootSelection(value) => value.scope = scope,
        SolveDraft::SolveProblem(value) => value.scope = scope,
        SolveDraft::RealCell(value) => value.scope = scope,
        SolveDraft::FamilyExpression(value) => value.scope = scope,
        _ => {}
    }
    Ok(Draft::Solve(value))
}
