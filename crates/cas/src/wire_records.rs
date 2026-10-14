// SPDX-License-Identifier: AGPL-3.0-only
//! Typed structural record descriptions with symbolic, strictly semantic dependencies.

use crate::budget::Budget;
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
            Entity::Statement(application.judgment.clone()),
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
        describe_entity(cas, value, &mut record, &mut out)?;
    }
    record.payload = out.finish();
    Ok(record)
}

/// Describe synthetic guarded, statement and conclusion records.
fn describe_entity(
    cas: &Cas,
    entity: &Entity,
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
        Entity::Statement(value) => {
            record.tag = 10;
            record.layer = value.layer;
            record.space = value
                .space
                .as_ref()
                .map(|value| Entity::Object(value.logical_id()));
            object(out, &mut record.references, cas.checked(&value.context)?)?;
            let start = record.references.len();
            crate::statement::encode(&value.statement, out, |value, out| {
                object(out, &mut record.references, cas.checked(value)?)
            })?;
            if let crate::proof_model::Statement::Cover { members, .. } = &value.statement {
                record.sets.push(
                    (start.checked_add(1).ok_or_else(invalid)?..record.references.len()).collect(),
                );
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
        Entity::Conclusion {
            proof,
            judgment,
            hypotheses,
        } => {
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
                Entity::Statement(judgment.clone()),
            )?;
            out.count(judgment.residuals.len())?;
            let start = record.references.len();
            for residual in &judgment.residuals {
                reference(out, &mut record.references, Entity::Guard(residual.clone()))?;
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

/// Describe headers, exact regions and immutable syntax dependencies.
fn describe_object(
    cas: &Cas,
    id: LogicalId,
    record: &mut Record,
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    match cas.state.node(id)? {
        Vertex::Header(value) => {
            record.tag = 0;
            out.byte(u8::from(value.parent.is_some()))?;
            if let Some(parent) = value.parent {
                object(out, &mut record.references, parent)?;
            }
            let mut environment = value.environment.clone();
            if value.parent.is_some() {
                environment.declarations.clone_from(&value.declarations);
            }
            encoding::environment(&environment, out)?;
            out.count(value.spaces.len())?;
            for (key, space) in &value.spaces {
                out.name(key)?;
                object(out, &mut record.references, *space)?;
            }
        }
        Vertex::Region(value) => {
            record.tag = 1;
            object(out, &mut record.references, value.header)?;
            encoding::scope(&value.scope, out)?;
            crate::domain_encoding::ambient(&value.ambient, out)?;
            let start = record.references.len();
            crate::domain_encoding::region(&value.expression, out, |id, out| {
                object(out, &mut record.references, *id)
            })?;
            if matches!(value.expression, RegionExpr::Meet(_) | RegionExpr::Union(_)) {
                record.sets.push((start..record.references.len()).collect());
            }
        }
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
            encoding::sort(&value.sort, out)?;
            encoding::operator(&value.operator, out)?;
            encoding::binding(value.binding.as_ref(), out)?;
            out.count(value.operands.len())?;
            for operand in &value.operands {
                object(out, &mut record.references, *operand)?;
            }
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
        Vertex::CaseArm(value) => {
            record.tag = 4;
            record.layer = Layer::Pointwise;
            object(out, &mut record.references, value.header)?;
            encoding::scope(&value.scope, out)?;
            object(out, &mut record.references, value.region)?;
            object(out, &mut record.references, value.value)?;
            encoding::sort(&value.sort, out)?;
        }
        Vertex::Boundary(value) => {
            record.tag = 5;
            object(out, &mut record.references, value.header)?;
            encoding::scope(&value.scope, out)?;
            crate::domain_encoding::ambient(&value.ambient, out)?;
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
            record.tag = 8;
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
