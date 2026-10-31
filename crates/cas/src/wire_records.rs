// SPDX-License-Identifier: AGPL-3.0-only
//! Typed structural record descriptions with symbolic, strictly semantic dependencies.

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::{Layer, RegionExpr};
use crate::encoding::{self, Encoder};
use crate::error::{Fault, Location};
use crate::graph::{Cas, LogicalId, Vertex};
use crate::wire_build::{Entity, Record};
use crate::wire_read::invalid;

/// Measured record encoding location.
const AT: Location = Location::new("typed witness record");

/// Append a measured symbolic reference whose index is assigned only by canonical ordering.
fn reference(
    out: &mut Encoder<'_>,
    references: &mut Vec<(usize, Entity)>,
    value: Entity,
) -> Result<(), Fault> {
    references
        .try_reserve(1)
        .map_err(|_| Fault::allocation(AT))?;
    references.push((out.len(), value));
    crate::wire::index(0, out)
}

/// Append an ordinary semantic graph dependency.
fn object(
    out: &mut Encoder<'_>,
    references: &mut Vec<(usize, Entity)>,
    value: LogicalId,
) -> Result<(), Fault> {
    reference(out, references, Entity::Object(value))
}

/// Build one reachable record without admitting any new mathematics.
pub(crate) fn describe(cas: &Cas, value: &Entity, budget: &mut Budget) -> Result<Record, Fault> {
    let application = match value {
        Entity::Object(id) if matches!(cas.state.node(*id)?, Vertex::Proof(_)) => {
            Some(crate::proof_view::application(cas, *id, budget)?)
        }
        _ => None,
    };
    let selection_cover = selection_cover(value, budget)?;
    let mut out = Encoder::witness(budget, AT);
    let mut record = Record {
        tag: 0,
        layer: Layer::Logic,
        space: None,
        payload: Vec::new(),
        references: Vec::new(),
        sets: Vec::new(),
        row_sets: Vec::new(),
        rule: None,
    };
    if let Some(application) = application {
        record.tag = 11;
        record.layer = application.judgment.layer;
        record.space = application
            .judgment
            .space
            .as_ref()
            .map(|value| Entity::Object(value.logical_id()));
        record.rule = Some(application.rule);
        crate::wire::index(0, &mut out)?;
        reference(
            &mut out,
            &mut record.references,
            Entity::Statement(Box::new(application.judgment.clone())),
        )?;
        out.count(application.premises.len())?;
        for premise in application.premises {
            object(&mut out, &mut record.references, premise)?;
        }
        out.count(application.substitutions.len())?;
        for substitution in application.substitutions {
            out.name(&substitution.key)?;
            object(
                &mut out,
                &mut record.references,
                cas.checked(&substitution.replacement)?,
            )?;
        }
        if let (
            crate::proof_model::Certificate::Region(certificate),
            crate::proof_model::Statement::Cover { members, .. },
        ) = (&application.certificate, &application.judgment.statement)
        {
            out.byte(1)?;
            out.count(certificate.cuts.len())?;
            for cut in &certificate.cuts {
                out.number(cut)?;
            }
            out.count(certificate.membership.len())?;
            let mut start = 0;
            for (index, row) in certificate.membership.iter().enumerate() {
                if index == 1 {
                    start = out.len();
                }
                out.count(row.len())?;
                for value in row {
                    out.byte(u8::from(*value))?;
                }
            }
            let width = certificate
                .membership
                .first()
                .ok_or_else(invalid)?
                .len()
                .checked_add(4)
                .ok_or_else(invalid)?;
            record.row_sets.push((
                start,
                width,
                members
                    .iter()
                    .map(|member| Entity::Object(member.logical_id()))
                    .collect(),
            ));
        } else {
            crate::statement::certificate(&application.certificate, &mut out, |handle, out| {
                object(out, &mut record.references, cas.checked(handle)?)
            })?;
        }
    } else {
        describe_entity(cas, value, selection_cover, &mut record, &mut out)?;
    }
    record.payload = out.finish();
    Ok(record)
}

/// A lattice selection cites the already retained complete cover proposition once.
fn selection_cover(
    entity: &Entity,
    budget: &mut Budget,
) -> Result<Option<crate::proof_model::Judgment>, Fault> {
    let Entity::Statement(judgment) = entity else {
        return Ok(None);
    };
    let (crate::proof_model::Statement::LatticeSelection(selection)
    | crate::proof_model::Statement::LatticeNearestSelection(selection)) = &judgment.statement
    else {
        return Ok(None);
    };
    let bytes = crate::lattice::cover::check::payload(&selection.cover, budget)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(bytes, AT)?;
    Ok(Some(crate::proof_model::Judgment {
        context: selection.cover.index.basis.context.clone(),
        layer: Layer::Logic,
        space: None,
        statement: crate::proof_model::Statement::LatticeCover(Box::new(selection.cover.clone())),
        residuals: Vec::new(),
    }))
}

/// Describe synthetic guarded, statement and conclusion records.
fn describe_entity(
    cas: &Cas,
    entity: &Entity,
    selection_cover: Option<crate::proof_model::Judgment>,
    record: &mut Record,
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    match entity {
        Entity::Object(id) => describe_object(cas, *id, record, out),
        Entity::Guard(value) => {
            record.tag = 9;
            record.layer = value.layer;
            record.space = value
                .space
                .as_ref()
                .map(|value| Entity::Object(value.logical_id()));
            object(out, &mut record.references, cas.checked(&value.home)?)?;
            encoding::scope(&value.scope, out)?;
            object(out, &mut record.references, cas.checked(&value.region)?)?;
            object(out, &mut record.references, cas.checked(&value.predicate)?)
        }
        Entity::Statement(value) => describe_statement(cas, value, selection_cover, record, out),
        Entity::Conclusion(value) => {
            let crate::wire_build::Conclusion {
                proof,
                judgment,
                hypotheses,
            } = value.as_ref();
            record.tag = 12;
            record.layer = judgment.layer;
            record.space = judgment
                .space
                .as_ref()
                .map(|value| Entity::Object(value.logical_id()));
            out.byte(u8::from(proof.is_some()))?;
            if let Some(proof) = proof {
                object(out, &mut record.references, *proof)?;
            }
            reference(
                out,
                &mut record.references,
                Entity::Statement(Box::new(judgment.clone())),
            )?;
            out.count(judgment.residuals.len())?;
            let start = record.references.len();
            for residual in &judgment.residuals {
                reference(
                    out,
                    &mut record.references,
                    Entity::Guard(Box::new(residual.clone())),
                )?;
            }
            record.sets.push((start..record.references.len()).collect());
            out.count(hypotheses.len())?;
            for hypothesis in hypotheses {
                object(
                    out,
                    &mut record.references,
                    cas.checked(&hypothesis.context)?,
                )?;
                out.unsigned(u64::from(hypothesis.slot))?;
            }
            Ok(())
        }
    }
}

/// Selection references share the cover statement but confer no cover proof authority.
fn describe_statement(
    cas: &Cas,
    value: &crate::proof_model::Judgment,
    selection_cover: Option<crate::proof_model::Judgment>,
    record: &mut Record,
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    use crate::proof_model::Statement;
    record.tag = 10;
    record.layer = value.layer;
    record.space = value
        .space
        .as_ref()
        .map(|value| Entity::Object(value.logical_id()));
    object(out, &mut record.references, cas.checked(&value.context)?)?;
    let start = record.references.len();
    if let Statement::LatticeSelection(selection) | Statement::LatticeNearestSelection(selection) =
        &value.statement
    {
        out.byte(
            if matches!(value.statement, Statement::LatticeNearestSelection(_)) {
                30
            } else {
                29
            },
        )?;
        reference(
            out,
            &mut record.references,
            Entity::Statement(Box::new(selection_cover.ok_or_else(invalid)?)),
        )?;
        object(out, &mut record.references, cas.checked(&selection.target)?)?;
        out.count(selection.values.len())?;
        for selected in &selection.values {
            object(out, &mut record.references, cas.checked(selected)?)?;
        }
    } else {
        crate::statement::encode(&value.statement, out, |value, out| {
            object(out, &mut record.references, cas.checked(value)?)
        })?;
    }
    if let Statement::Cover { members, .. } = &value.statement {
        record
            .sets
            .push((start.checked_add(1).ok_or_else(invalid)?..record.references.len()).collect());
        if members.len().checked_add(1).ok_or_else(invalid)?
            != record
                .references
                .len()
                .checked_sub(start)
                .ok_or_else(invalid)?
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Describe headers, exact regions and immutable syntax dependencies.
fn describe_object(
    cas: &Cas,
    id: LogicalId,
    record: &mut Record,
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    match cas.state.node(id)? {
        Vertex::Header(value) => describe_header(value, record, out)?,

        Vertex::Region(value) => describe_region(value, record, out)?,

        Vertex::Space(value) => {
            record.tag = 2;
            out.byte(0)?;
            object(out, &mut record.references, value.header)?;
            out.name(&value.coordinate)?;
            encoding::field(&value.field, out)?;
            object(out, &mut record.references, value.domain)?;
            out.name(&value.test_family)?;
        }
        Vertex::Expression(value) => {
            record.tag = 3;
            record.layer = value.layer;
            record.space = value.space.map(Entity::Object);
            object(out, &mut record.references, value.header)?;
            encoding::scope(&value.scope, out)?;
            out.byte(u8::from(value.ambient.is_some()))?;
            if let Some(ambient) = value.ambient {
                object(out, &mut record.references, ambient)?;
            }
            encoding::sort(&value.sort, out)?;
            encoding::operator(&value.operator, out)?;
            encoding::binding(value.binding.as_ref(), out)?;
            out.count(value.operands.len())?;
            for operand in &value.operands {
                object(out, &mut record.references, *operand)?;
            }
            attachment(value.attachment.as_ref(), record, out)?;
            out.byte(u8::from(value.domain.is_some()))?;
            if let Some(domain) = value.domain {
                object(out, &mut record.references, domain)?;
            }
            out.count(value.arms.len())?;
            let start = record.references.len();
            for arm in &value.arms {
                object(out, &mut record.references, *arm)?;
            }
            record.sets.push((start..record.references.len()).collect());
        }
        _ => return describe_remaining(cas, id, record, out),
    }
    Ok(())
}

/// Describe the remaining typed objects without hiding source order or context restrictions.
fn describe_remaining(
    cas: &Cas,
    id: LogicalId,
    record: &mut Record,
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    match cas.state.node(id)? {
        Vertex::Solve(value) => describe_solve(cas, value, record, out)?,
        Vertex::CaseArm(value) => {
            record.tag = 4;
            record.layer = Layer::Pointwise;
            object(out, &mut record.references, value.header)?;
            encoding::scope(&value.scope, out)?;
            object(out, &mut record.references, value.region)?;
            object(out, &mut record.references, value.value)?;
            encoding::sort(&value.sort, out)?;
            out.count(value.placements.len())?;
            for pair in &value.placements {
                object(out, &mut record.references, pair.operand)?;
                object(out, &mut record.references, pair.piece)?;
            }
        }
        Vertex::Boundary(value) => {
            record.tag = 5;
            object(out, &mut record.references, value.header)?;
            encoding::scope(&value.scope, out)?;
            ambient(&value.ambient, record, out)?;
            object(out, &mut record.references, value.location)?;
        }
        Vertex::SourceForm(value) => {
            record.tag = 6;
            record.layer = Layer::Pointwise;
            object(out, &mut record.references, value.header)?;
            encoding::scope(&value.scope, out)?;
            out.byte(crate::domain_encoding::source_kind(value.kind))?;
            object(out, &mut record.references, value.domain)?;
            out.count(value.clauses.len())?;
            for (selector, value) in &value.clauses {
                object(out, &mut record.references, *selector)?;
                object(out, &mut record.references, *value)?;
            }
            out.byte(u8::from(value.otherwise.is_some()))?;
            if let Some(otherwise) = value.otherwise {
                object(out, &mut record.references, otherwise)?;
            }
        }
        Vertex::Condition(value) => {
            record.tag = 7;
            object(out, &mut record.references, value.header)?;
            encoding::scope(&value.scope, out)?;
            encoding::predicate(&value.kind, out)?;
            out.count(value.arguments.len())?;
            for argument in &value.arguments {
                object(out, &mut record.references, *argument)?;
            }
        }
        Vertex::Context(value) => {
            record.tag = if value.introduction.is_some() { 37 } else { 8 };
            if let Some(introduction) = value.introduction {
                object(out, &mut record.references, introduction)?;
            }
            object(out, &mut record.references, value.header_id)?;
            out.byte(u8::from(value.parent.is_some()))?;
            if let Some(parent) = value.parent {
                object(out, &mut record.references, parent)?;
            }
            object(out, &mut record.references, value.admissible)?;
            out.byte(u8::from(value.restriction.is_some()))?;
            if let Some(restriction) = value.restriction {
                object(out, &mut record.references, restriction)?;
            }
            let frame = if value.parent.is_some() {
                &value.frame
            } else {
                &value.assumptions
            };
            out.count(frame.len())?;
            for (name, predicate) in frame {
                out.name(name)?;
                object(out, &mut record.references, *predicate)?;
            }
        }
        _ => return Err(invalid()),
    }
    Ok(())
}

/// Product coordinate identities are ordinary earlier wire dependencies.
fn ambient(
    value: &crate::domain_model::AmbientDomain,
    record: &mut Record,
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    if let crate::domain_model::AmbientDomain::Product { coordinates } = value {
        out.byte(4)?;
        object(out, &mut record.references, coordinates.logical_id())
    } else {
        crate::domain_encoding::ambient(value, out)
    }
}

/// Every expression has an explicit attachment byte, including ordinary scalar leaves.
fn attachment(
    value: Option<&crate::graph::ExpressionAttachment>,
    record: &mut Record,
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    match value {
        None => out.byte(0),
        Some(crate::graph::ExpressionAttachment::SelectedRoot(selection)) => {
            out.byte(1)?;
            object(out, &mut record.references, *selection)
        }
        Some(crate::graph::ExpressionAttachment::ExactCoefficient { value, map }) => {
            out.byte(2)?;
            object(out, &mut record.references, *value)?;
            object(out, &mut record.references, *map)
        }
    }
}

/// Encode the specified new graph schema and only its explicitly unordered families.
fn describe_solve(
    cas: &Cas,
    value: &crate::solving::SolveDraft<crate::graph::Handle>,
    record: &mut Record,
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    use crate::solving::{SolutionBranchKind, SolveDraft};
    record.tag = value.record_tag();
    let start = record.references.len();
    value.encode(out, &mut |handle, out| {
        object(out, &mut record.references, cas.checked(handle)?)
    })?;
    let set_start = match value {
        SolveDraft::SolutionSet(_) => Some(start.checked_add(1).ok_or_else(invalid)?),
        SolveDraft::CoefficientMap(value) => Some(
            start
                .checked_add(3)
                .and_then(|n| n.checked_add(value.images.len()))
                .ok_or_else(invalid)?,
        ),
        SolveDraft::SolutionBranch(value)
            if matches!(value.kind, SolutionBranchKind::Finite { .. }) =>
        {
            Some(start.checked_add(3).ok_or_else(invalid)?)
        }
        _ => None,
    };
    if let Some(start) = set_start {
        record.sets.push((start..record.references.len()).collect());
    }
    Ok(())
}

/// Encode all exact header dependencies in the sole current record layout.
fn describe_header(
    value: &crate::domain_model::Header,
    record: &mut Record,
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    record.tag = 0;
    out.byte(u8::from(value.parent.is_some()))?;
    if let Some(parent) = value.parent {
        object(out, &mut record.references, parent)?;
    }
    let mut environment = value.environment.environment().clone();
    if value.parent.is_some() {
        environment.declarations = value.declarations().to_vec();
    }
    encoding::environment(&environment, out)?;
    out.count(value.scalar_spaces.len())?;
    for space in &value.scalar_spaces {
        object(out, &mut record.references, *space)?;
    }
    out.count(value.spaces.len())?;
    for (key, space) in &value.spaces {
        out.name(key)?;
        object(out, &mut record.references, *space)?;
    }

    Ok(())
}

/// Encode all exact region dependencies in the sole current record layout.
fn describe_region(
    value: &crate::domain_model::Region,
    record: &mut Record,
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    record.tag = 1;
    object(out, &mut record.references, value.header)?;
    encoding::scope(&value.scope, out)?;
    ambient(&value.ambient, record, out)?;
    let start = record.references.len();
    crate::domain_encoding::region(&value.expression, out, |id, out| {
        object(out, &mut record.references, *id)
    })?;
    if matches!(value.expression, RegionExpr::Meet(_) | RegionExpr::Union(_)) {
        record.sets.push((start..record.references.len()).collect());
    }
    out.byte(u8::from(value.lies_over.is_some()))?;
    if let Some(projection) = &value.lies_over {
        object(out, &mut record.references, projection.family)?;
        object(out, &mut record.references, projection.stratum)?;
    }

    Ok(())
}
