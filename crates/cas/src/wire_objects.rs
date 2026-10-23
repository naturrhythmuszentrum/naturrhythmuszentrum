// SPDX-License-Identifier: AGPL-3.0-only
//! Typed object reconstruction through the normal admission checks.

use crate::budget::BudgetKind;
use crate::domain_model::{
    AmbientDomain, BoundaryDraft, CaseArmDraft, ContextDraft, DeclarationSpaceDraft, HeaderDraft,
    Layer, RegionDraft, RegionExpr, SpaceDraft,
};
use crate::error::{Fault, Location};
use crate::graph::AssumptionDraft;
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, Reference, Vertex};
use crate::source_model::{SourceClauseDraft, SourceFormDraft, SourceFormKind};
use crate::wire_decode::{Decoded, object, objects, optional};
use crate::wire_read::{Reader, invalid};

/// Typed object input location.
const AT: Location = Location::new("typed object decoding");

/// Convert an earlier semantic record to an admission reference.
fn reference(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Reference, Fault> {
    Ok(Reference::Existing(object(reader, records)?))
}

/// Read a closed coordinate-domain discriminator.
fn ambient(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<AmbientDomain, Fault> {
    Ok(match reader.byte()? {
        0 => AmbientDomain::Assignments,
        1 => AmbientDomain::Real(reader.name()?),
        2 => AmbientDomain::Integer(reader.name()?),
        3 => AmbientDomain::Opaque(reader.name()?),
        4 => AmbientDomain::Product {
            coordinates: object(reader, records)?,
        },
        _ => return Err(invalid()),
    })
}

/// Decode exact set constructors; normalization belongs to ordinary admission.
fn region(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<RegionExpr, Fault> {
    Ok(match reader.byte()? {
        0 => RegionExpr::Ambient,
        1 => RegionExpr::Empty,
        2 => RegionExpr::GuardTrue {
            base: reference(reader, records)?,
            predicate: reference(reader, records)?,
        },
        3 => RegionExpr::GuardFalse {
            base: reference(reader, records)?,
            predicate: reference(reader, records)?,
        },
        tag @ (4 | 5) => {
            let values = objects(reader, records, BudgetKind::RegionNodes)?
                .into_iter()
                .map(Reference::Existing)
                .collect();
            if tag == 4 {
                RegionExpr::Meet(values)
            } else {
                RegionExpr::Union(values)
            }
        }
        6 => RegionExpr::Difference {
            base: reference(reader, records)?,
            excluded: reference(reader, records)?,
        },
        _ => return Err(invalid()),
    })
}

/// Admit exactly one decoded draft in the unpublished replay arena.
fn admit(cas: &mut Cas, draft: Draft, reader: &mut Reader<'_, '_>) -> Result<Handle, Fault> {
    cas.admit(&[draft], reader.budget)?
        .into_iter()
        .next()
        .ok_or_else(invalid)
}

/// Reconstruct headers with earlier space references and fresh ordered declarations.
fn header(
    cas: &mut Cas,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Handle, Fault> {
    let parent = optional(reader, records)?;
    let environment = reader.environment()?;
    let scalar_spaces = objects(reader, records, BudgetKind::Edges)?;
    let count = reader.count(BudgetKind::Vertices, 8, 96)?;
    let mut spaces = Vec::new();
    spaces
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        spaces.push(DeclarationSpaceDraft {
            key: reader.name()?,
            space: reference(reader, records)?,
        });
    }
    let draft = if let Some(parent) = parent {
        let previous = cas.state.header(cas.checked(&parent)?)?;
        if previous.environment.field != environment.field
            || previous.environment.branch != environment.branch
        {
            return Err(invalid());
        }
        HeaderDraft::Extend {
            parent: Reference::Existing(parent),
            declarations: environment.declarations,
            spaces,
        }
    } else {
        if !spaces.is_empty() {
            return Err(invalid());
        }
        HeaderDraft::Core(environment)
    };
    let handle = admit(cas, Draft::Header(draft), reader)?;
    let actual = &cas.state.header(cas.checked(&handle)?)?.scalar_spaces;
    if !actual
        .iter()
        .copied()
        .eq(scalar_spaces.iter().map(Handle::logical_id))
    {
        return Err(invalid());
    }
    Ok(handle)
}

/// Replay syntax records while keeping their actual header and typed links.
fn syntax(
    cas: &mut Cas,
    tag: u8,
    space: Option<&Handle>,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Handle, Fault> {
    let header = reference(reader, records)?;
    let scope = reader.scope()?;
    let draft = match tag {
        1 => return region_object(cas, header, scope, reader, records),
        3 => return expression(cas, header, &scope, space, reader, records),
        4 => return arm_object(cas, header, scope, reader, records),
        5 => Draft::Boundary(BoundaryDraft {
            header,
            scope,
            ambient: ambient(reader, records)?,
            location: reference(reader, records)?,
        }),
        6 => return source(cas, header, scope, reader, records),
        7 => Draft::Condition {
            header,
            scope,
            kind: reader.predicate()?,
            arguments: objects(reader, records, BudgetKind::ConditionAtoms)?
                .into_iter()
                .map(Reference::Existing)
                .collect(),
        },
        _ => return Err(invalid()),
    };
    admit(cas, draft, reader)
}

/// Preserve written clause order and an explicit otherwise marker.
fn source(
    cas: &mut Cas,
    header: Reference,
    scope: crate::model::Scope,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Handle, Fault> {
    let kind = match reader.byte()? {
        0 => SourceFormKind::OrderedCases,
        1 => SourceFormKind::CaseLiteral,
        _ => return Err(invalid()),
    };
    let domain = reference(reader, records)?;
    let count = reader.count(BudgetKind::CaseArms, 8, 128)?;
    let mut clauses = Vec::new();
    clauses
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        clauses.push(SourceClauseDraft {
            selector: reference(reader, records)?,
            value: reference(reader, records)?,
        });
    }
    let otherwise = optional(reader, records)?.map(Reference::Existing);
    admit(
        cas,
        Draft::SourceForm(SourceFormDraft {
            header,
            scope,
            kind,
            domain,
            clauses,
            otherwise,
        }),
        reader,
    )
}

/// Reconstruct the local frame and independently verify its exact meet region.
fn context(
    cas: &mut Cas,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Handle, Fault> {
    let header = object(reader, records)?;
    let parent = optional(reader, records)?;
    let admissible = object(reader, records)?;
    let restriction = optional(reader, records)?;
    let count = reader.count(BudgetKind::ConditionAtoms, 8, 96)?;
    let mut assumptions = Vec::new();
    assumptions
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        assumptions.push(AssumptionDraft {
            key: reader.name()?,
            condition: reference(reader, records)?,
        });
    }
    let draft = match (parent, restriction) {
        (None, None) => ContextDraft::Root {
            header: Reference::Existing(header.clone()),
            admissible: Reference::Existing(admissible.clone()),
            assumptions,
        },
        (Some(parent), Some(region)) => ContextDraft::Local {
            parent: Reference::Existing(parent),
            region: Reference::Existing(region),
            hypotheses: assumptions,
        },
        _ => return Err(invalid()),
    };
    let value = admit(cas, Draft::Context(draft), reader)?;
    let context = cas.state.context(cas.checked(&value)?)?;
    if context.header_id != cas.checked(&header)?
        || context.admissible != cas.checked(&admissible)?
    {
        return Err(invalid());
    }
    Ok(value)
}

/// Decode a semantic object and compare the common layer/space prefix to its reconstructed meaning.
pub(crate) fn decode(
    cas: &mut Cas,
    tag: u8,
    layer: Layer,
    space: Option<&Handle>,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Handle, Fault> {
    #[cfg(test)]
    let _attribution = crate::testing::work_trace::Scope::phase("syntax admission");
    let previous = reader.budget.usage();
    let value = match tag {
        0 => header(cas, reader, records)?,
        2 => {
            if reader.byte()? != 0 {
                return Err(invalid());
            }
            let header = reference(reader, records)?;
            let coordinate = reader.name()?;
            let field = reader.field()?;
            let domain = reference(reader, records)?;
            let test_family = reader.name()?;
            admit(
                cas,
                Draft::Space(SpaceDraft {
                    header,
                    domain,
                    coordinate,
                    field,
                    test_family,
                }),
                reader,
            )?
        }
        8 => context(cas, reader, records)?,
        13..=36 => {
            if layer != Layer::Logic || space.is_some() {
                return Err(invalid());
            }
            crate::solving::wire::decode_object(cas, tag, reader, records)?
        }
        _ => syntax(cas, tag, space, reader, records)?,
    };
    if tag == 1 && reader.budget.usage().region_nodes == previous.region_nodes {
        reader
            .budget
            .specialized(BudgetKind::RegionNodes, 1, true, AT)?;
    }
    if tag == 5 && reader.budget.usage().boundary_nodes == previous.boundary_nodes {
        reader
            .budget
            .specialized(BudgetKind::BoundaryNodes, 1, true, AT)?;
    }
    if tag == 8
        && cas.state.context(cas.checked(&value)?)?.parent.is_some()
        && reader.budget.usage().local_contexts == previous.local_contexts
    {
        reader
            .budget
            .specialized(BudgetKind::LocalContexts, 1, true, AT)?;
    }
    let (expected_layer, expected_space) = match cas.state.node(cas.checked(&value)?)? {
        Vertex::Expression(value) => (value.layer, value.space.map(|id| cas.handle(id))),
        Vertex::CaseArm(_) | Vertex::SourceForm(_) => (Layer::Pointwise, None),
        _ => (Layer::Logic, None),
    };
    if layer != expected_layer || space != expected_space.as_ref() {
        return Err(invalid());
    }
    Ok(value)
}

/// Read a literal attachment only when its operator explicitly requires that exact kind.
fn read_attachment(
    header: &Reference,
    operator: &crate::model::Operator,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Option<Draft>, Fault> {
    Ok(match reader.byte()? {
        0 => None,
        1 => {
            let crate::model::Operator::RootValue { slot } = operator else {
                return Err(invalid());
            };
            Some(Draft::RootValue {
                header: header.clone(),
                selection: reference(reader, records)?,
                slot: *slot,
            })
        }
        2 => {
            if *operator != crate::model::Operator::ExactCoefficient {
                return Err(invalid());
            }
            Some(Draft::ExactCoefficient {
                header: header.clone(),
                value: reference(reader, records)?,
                map: reference(reader, records)?,
            })
        }
        _ => return Err(invalid()),
    })
}

/// Rebuild scalar, attached-literal or native-case syntax through ordinary admission.
fn expression(
    cas: &mut Cas,
    header: Reference,
    scope: &crate::model::Scope,
    space: Option<&Handle>,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Handle, Fault> {
    let claimed_ambient = optional(reader, records)?;
    let sort = reader.sort()?;
    let operator = reader.operator()?;
    let binding = reader.binding()?;
    let operands: Vec<Reference> = objects(reader, records, BudgetKind::Edges)?
        .into_iter()
        .map(Reference::Existing)
        .collect();
    let attached = read_attachment(&header, &operator, reader, records)?;
    let domain = optional(reader, records)?;
    let arms = objects(reader, records, BudgetKind::CaseArms)?;
    let draft = if let Some(attached) = attached {
        if binding.is_some()
            || !operands.is_empty()
            || domain.is_some()
            || !arms.is_empty()
            || space.is_some()
        {
            return Err(invalid());
        }
        attached
    } else if operator == crate::model::Operator::Cases {
        if binding.is_some() || !operands.is_empty() {
            return Err(invalid());
        }
        Draft::Cases(crate::domain_model::CasesDraft {
            header,
            scope: scope.clone(),
            domain: Reference::Existing(domain.ok_or_else(invalid)?),
            arms: arms.into_iter().map(Reference::Existing).collect(),
        })
    } else {
        if domain.is_some() || !arms.is_empty() {
            return Err(invalid());
        }
        Draft::Expression(ExpressionDraft {
            space: space.cloned().map(Reference::Existing),
            operator,
            sort: sort.clone(),
            header,
            scope: scope.clone(),
            binding,
            operands,
        })
    };
    let value = admit(cas, draft, reader)?;
    let actual = cas.state.expression(cas.checked(&value)?)?;
    if actual.ambient != claimed_ambient.as_ref().map(Handle::logical_id) {
        return Err(crate::error::Fault::invalid(
            crate::error::InvalidCode::AmbientSpaceMismatch,
            AT,
        ));
    }
    if actual.sort != sort || actual.scope != *scope {
        return Err(invalid());
    }
    Ok(value)
}

/// Family projection is an explicit suffix on the sole current region record.
fn region_object(
    cas: &mut Cas,
    header: Reference,
    scope: crate::model::Scope,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Handle, Fault> {
    let region = RegionDraft {
        header,
        scope,
        ambient: ambient(reader, records)?,
        expression: region(reader, records)?,
    };
    let draft = match reader.byte()? {
        0 => Draft::Region(region),
        1 => Draft::FamilyRegion {
            region,
            lies_over: crate::solving::LiesOver {
                family: reference(reader, records)?,
                stratum: reference(reader, records)?,
            },
        },
        _ => return Err(invalid()),
    };
    admit(cas, draft, reader)
}

/// Each placement association remains paired with its original operand on replay.
fn arm_object(
    cas: &mut Cas,
    header: Reference,
    scope: crate::model::Scope,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Handle, Fault> {
    let region = reference(reader, records)?;
    let value = reference(reader, records)?;
    let sort = reader.sort()?;
    let count = reader.count(BudgetKind::Edges, 8, 128)?;
    let mut placements = Vec::new();
    placements
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        placements.push(crate::solving::PlacementUse {
            operand: reference(reader, records)?,
            piece: reference(reader, records)?,
        });
    }
    let handle = admit(
        cas,
        Draft::CaseArm(CaseArmDraft {
            placements,
            header,
            scope,
            region,
            value,
        }),
        reader,
    )?;
    if cas.state.arm(cas.checked(&handle)?)?.sort != sort {
        return Err(invalid());
    }
    Ok(handle)
}
