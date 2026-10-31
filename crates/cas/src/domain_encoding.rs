// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded structural encodings for semantic domain objects and their drafts.

use crate::domain_model::{AmbientDomain, ContextDraft, HeaderDraft, RegionExpr};
use crate::encoding::{self, Encoder};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Draft, LogicalId, Reference, Vertex, encode_id};
use crate::model::{AtomicSort, Field, Sort};
use crate::source_model::SourceFormKind;

/// Encode a draft reference without trusting it as a graph lookup.
pub(crate) fn reference(value: &Reference, out: &mut Encoder<'_>) -> Result<(), Fault> {
    match value {
        Reference::Existing(handle) => {
            out.byte(0)?;
            encode_id(handle.logical_id(), out)
        }
        Reference::Local(index) => {
            out.byte(1)?;
            out.unsigned(u64::from(*index))
        }
    }
}

/// Encode the complete coordinate interpretation.
pub(crate) fn ambient(value: &AmbientDomain, out: &mut Encoder<'_>) -> Result<(), Fault> {
    match value {
        AmbientDomain::Product { coordinates } => {
            out.byte(4)?;
            encode_id(coordinates.logical_id(), out)
        }
        AmbientDomain::Assignments => out.byte(0),
        AmbientDomain::Real(key) => {
            out.byte(1)?;
            out.name(key)
        }
        AmbientDomain::Integer(key) => {
            out.byte(2)?;
            out.name(key)
        }
        AmbientDomain::Opaque(key) => {
            out.byte(3)?;
            out.name(key)
        }
    }
}

/// Encode a region constructor using either draft or resolved references.
pub(crate) fn region<R>(
    value: &RegionExpr<R>,
    out: &mut Encoder<'_>,
    mut write: impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    match value {
        RegionExpr::Ambient => out.byte(0),
        RegionExpr::Empty => out.byte(1),
        RegionExpr::GuardTrue { base, predicate } | RegionExpr::GuardFalse { base, predicate } => {
            out.byte(if matches!(value, RegionExpr::GuardTrue { .. }) {
                2
            } else {
                3
            })?;
            write(base, out)?;
            write(predicate, out)
        }
        RegionExpr::Meet(members) | RegionExpr::Union(members) => {
            out.byte(if matches!(value, RegionExpr::Meet(_)) {
                4
            } else {
                5
            })?;
            out.count(members.len())?;
            for member in members {
                write(member, out)?;
            }
            Ok(())
        }
        RegionExpr::Difference { base, excluded } => {
            out.byte(6)?;
            write(base, out)?;
            write(excluded, out)
        }
    }
}

/// Adapt a borrowed logical identity to the shared constructor encoder.
fn logical(value: &LogicalId, out: &mut Encoder<'_>) -> Result<(), Fault> {
    encode_id(*value, out)
}

/// Encode exactly the graph-owned semantic payload.
pub(crate) fn vertex(value: &Vertex, out: &mut Encoder<'_>) -> Result<(), Fault> {
    match value {
        Vertex::Header(value) => {
            out.byte(u8::from(value.parent.is_some()))?;
            if let Some(parent) = value.parent {
                encode_id(parent, out)?;
            }
            value.environment.encode(out)?;
            out.count(value.scalar_spaces.len())?;
            for space in &value.scalar_spaces {
                encode_id(*space, out)?;
            }
            out.count(value.spaces.len())?;
            for (key, space) in &value.spaces {
                out.name(key)?;
                encode_id(*space, out)?;
            }
        }
        Vertex::Region(value) => {
            encode_id(value.header, out)?;
            encoding::scope(&value.scope, out)?;
            ambient(&value.ambient, out)?;
            region(&value.expression, out, logical)?;
            out.byte(u8::from(value.lies_over.is_some()))?;
            if let Some(projection) = &value.lies_over {
                encode_id(projection.family, out)?;
                encode_id(projection.stratum, out)?;
            }
        }
        Vertex::Space(value) => {
            encode_id(value.header, out)?;
            encode_id(value.domain, out)?;
            out.name(&value.coordinate)?;
            encoding::field(&value.field, out)?;
            out.name(&value.test_family)?;
        }
        Vertex::CaseArm(value) => {
            encode_id(value.header, out)?;
            encoding::scope(&value.scope, out)?;
            encode_id(value.region, out)?;
            encode_id(value.value, out)?;
            encoding::sort(&value.sort, out)?;
            out.count(value.placements.len())?;
            for pair in &value.placements {
                encode_id(pair.operand, out)?;
                encode_id(pair.piece, out)?;
            }
        }
        Vertex::Boundary(value) => {
            encode_id(value.header, out)?;
            encoding::scope(&value.scope, out)?;
            ambient(&value.ambient, out)?;
            encode_id(value.location, out)?;
        }
        Vertex::SourceForm(value) => {
            encode_id(value.header, out)?;
            encoding::scope(&value.scope, out)?;
            out.byte(source_kind(value.kind))?;
            encode_id(value.domain, out)?;
            out.count(value.clauses.len())?;
            for (selector, branch) in &value.clauses {
                encode_id(*selector, out)?;
                encode_id(*branch, out)?;
            }
            out.byte(u8::from(value.otherwise.is_some()))?;
            if let Some(otherwise) = value.otherwise {
                encode_id(otherwise, out)?;
            }
        }
        _ => {
            return Err(Fault::invalid(
                InvalidCode::Reference,
                Location::new("domain key"),
            ));
        }
    }
    Ok(())
}

/// Stable original form tags, distinct from native case arm order.
pub(crate) const fn source_kind(value: SourceFormKind) -> u8 {
    match value {
        SourceFormKind::OrderedCases => 0,
        SourceFormKind::CaseLiteral => 1,
    }
}

/// Fixed result enums fit the draft's prepaid 256-byte record; owned names still require a bound.
/// This sink measures draft storage, not identity: the expression key always encodes the full sort.
fn case_result_payload(value: &Sort, out: &mut Encoder<'_>) -> Result<(), Fault> {
    match value {
        Sort::Value(
            AtomicSort::Integer
            | AtomicSort::Scalar(
                Field::Real | Field::Complex | Field::Rational | Field::PositiveCharacteristic(_),
            ),
        ) => Ok(()),
        _ => encoding::sort(value, out),
    }
}

/// Bound every new draft payload before graph or owned collection copies.
pub(crate) fn draft(value: &Draft, out: &mut Encoder<'_>) -> Result<(), Fault> {
    match value {
        Draft::Cases(value) => {
            case_result_payload(&value.sort, out)?;
            reference(&value.header, out)?;
            encoding::scope(&value.scope, out)?;
            reference(&value.domain, out)?;
            out.count(value.arms.len())?;
            for arm in &value.arms {
                reference(arm, out)?;
            }
        }
        Draft::Header(value) => header_draft(value, out)?,
        Draft::Context(value) => context_draft(value, out)?,
        Draft::FamilyRegion {
            region: value,
            lies_over,
        } => {
            reference(&value.header, out)?;
            encoding::scope(&value.scope, out)?;
            ambient(&value.ambient, out)?;
            region(&value.expression, out, reference)?;
            reference(&lies_over.family, out)?;
            reference(&lies_over.stratum, out)?;
        }
        Draft::Region(value) => {
            reference(&value.header, out)?;
            encoding::scope(&value.scope, out)?;
            ambient(&value.ambient, out)?;
            region(&value.expression, out, reference)?;
        }
        Draft::Space(value) => {
            reference(&value.header, out)?;
            reference(&value.domain, out)?;
            out.name(&value.coordinate)?;
            encoding::field(&value.field, out)?;
            out.name(&value.test_family)?;
        }
        Draft::CaseArm(value) => {
            reference(&value.header, out)?;
            encoding::scope(&value.scope, out)?;
            reference(&value.region, out)?;
            reference(&value.value, out)?;
            out.count(value.placements.len())?;
            for pair in &value.placements {
                reference(&pair.operand, out)?;
                reference(&pair.piece, out)?;
            }
        }
        Draft::Boundary(value) => {
            reference(&value.header, out)?;
            encoding::scope(&value.scope, out)?;
            ambient(&value.ambient, out)?;
            reference(&value.location, out)?;
        }
        Draft::SourceForm(value) => {
            reference(&value.header, out)?;
            encoding::scope(&value.scope, out)?;
            out.byte(source_kind(value.kind))?;
            reference(&value.domain, out)?;
            out.count(value.clauses.len())?;
            for clause in &value.clauses {
                reference(&clause.selector, out)?;
                reference(&clause.value, out)?;
            }
            out.byte(u8::from(value.otherwise.is_some()))?;
            if let Some(otherwise) = &value.otherwise {
                reference(otherwise, out)?;
            }
        }
        _ => {
            return Err(Fault::invalid(
                InvalidCode::Reference,
                Location::new("domain draft"),
            ));
        }
    }
    Ok(())
}

/// Measure header signatures without resolving their references.
fn header_draft(value: &HeaderDraft, out: &mut Encoder<'_>) -> Result<(), Fault> {
    match value {
        HeaderDraft::Core(environment) => {
            out.byte(0)?;
            encoding::environment(environment, out)
        }
        HeaderDraft::Frozen(environment) => {
            // Draft scratch owns a shared authority, not another declaration-table copy.
            out.byte(2)?;
            out.unsigned(crate::graph::size(
                environment.canonical_bytes().len(),
                Location::new("frozen header draft"),
            )?)?;
            out.unsigned(crate::graph::size(
                environment.declarations.len(),
                Location::new("frozen header draft"),
            )?)
        }
        HeaderDraft::Extend {
            parent,
            declarations,
            spaces,
        } => {
            out.byte(1)?;
            reference(parent, out)?;
            out.count(declarations.len())?;
            for declaration in declarations {
                out.name(&declaration.key)?;
                out.name(&declaration.display_name)?;
                encoding::sort(&declaration.sort, out)?;
            }
            out.count(spaces.len())?;
            for item in spaces {
                out.name(&item.key)?;
                reference(&item.space, out)?;
            }
            Ok(())
        }
    }
}

/// Measure complete context frames, including provenance and parent references.
fn context_draft(value: &ContextDraft, out: &mut Encoder<'_>) -> Result<(), Fault> {
    let hypotheses = match value {
        ContextDraft::Certified {
            parent,
            region,
            introduction,
        } => {
            out.byte(2)?;
            reference(parent, out)?;
            reference(region, out)?;
            return reference(introduction, out);
        }
        ContextDraft::Root {
            header,
            admissible,
            assumptions,
        } => {
            out.byte(0)?;
            reference(header, out)?;
            reference(admissible, out)?;
            assumptions
        }
        ContextDraft::Local {
            parent,
            region,
            hypotheses,
        } => {
            out.byte(1)?;
            reference(parent, out)?;
            reference(region, out)?;
            hypotheses
        }
    };
    out.count(hypotheses.len())?;
    for hypothesis in hypotheses {
        out.name(&hypothesis.key)?;
        reference(&hypothesis.condition, out)?;
    }
    Ok(())
}
