// SPDX-License-Identifier: AGPL-3.0-only
//! Records 39–41 retain the ordinary checked judgment and its complete mixed closure.

use super::check::{AT, invalid, unsupported};
use crate::budget::{Budget, BudgetKind};
use crate::domain_model::{HypothesisRef, Layer};
use crate::encoding::Encoder;
use crate::error::Fault;
use crate::graph::{Cas, Handle, LogicalId, Vertex};
use crate::proof_model::{ApplicationDraft, Certificate, Judgment, Statement};
use crate::rule::Primitive;
use crate::wire_build::{Entity, Record};
use crate::wire_decode::{Decoded, object, reference};
use crate::wire_read::Reader;
use std::collections::BTreeSet;

/// Untrusted declaration of all conditions and the actual used-schema closure.
#[derive(Debug)]
pub(crate) struct Claim {
    /// Ordinary expected mathematical judgment.
    judgment: Judgment,
    /// Exact qualified hypotheses, independently reconstructed at conclusion checking.
    hypotheses: Vec<HypothesisRef>,
    /// Sorted exact schema closure, never executable callbacks.
    rules: Vec<Primitive>,
}

/// Identify the dedicated proof category without copying or traversing ordinary applications.
pub(crate) fn is_series(cas: &Cas, proof: LogicalId) -> Result<bool, Fault> {
    let Vertex::Proof(application) = cas.state.node(proof)? else {
        return Ok(false);
    };
    if matches!(cas.state.node(*application)?, Vertex::AnonymousLemma(_)) {
        cas.series_application(*application)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Full used-rule closure reconstructed from checked graph citations.
fn rules(cas: &Cas, proof: LogicalId, budget: &mut Budget) -> Result<Vec<Primitive>, Fault> {
    let citations = cas.proof_citations(&cas.handle(proof), budget)?;
    let count = crate::graph::size(citations.len(), AT)?;
    // The closed registry bounds comparison depth and schema-key length. This
    // per-entry allowance covers insertion, sorting and the returned rule vector.
    budget.work(count.saturating_mul(512), AT)?;
    let previous = budget.reserve_temporary(count.saturating_mul(128), AT)?;
    let mut unique = BTreeSet::new();
    for citation in citations {
        unique.insert(citation.primitive);
    }
    let mut result: Vec<_> = unique.into_iter().collect();
    result.sort_by_key(|rule| rule.specification().key);
    budget.restore_limits(previous);
    Ok(result)
}

/// Append a symbolic dependency whose index is filled only after canonical ordering.
fn reference_out(out: &mut Encoder<'_>, record: &mut Record, value: Entity) -> Result<(), Fault> {
    record
        .references
        .try_reserve(1)
        .map_err(|_| Fault::allocation(AT))?;
    record.references.push((out.len(), value));
    crate::wire::index(0, out)
}

/// Encode exact versions in the retained closure independently of table-local manifest indices.
fn encode_rules(values: &[Primitive], out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.count(values.len())?;
    for rule in values {
        let spec = rule.specification();
        out.name(spec.key)?;
        crate::wire::index(spec.version, out)?;
    }
    Ok(())
}

/// Series statements retain their own qualified conditions as well as the enclosing conclusion.
fn encode_claim(
    cas: &Cas,
    claim: &Claim,
    record: &mut Record,
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    let Statement::Series(value) = &claim.judgment.statement else {
        return Err(invalid());
    };
    record.tag = 39;
    record.rule = Some(value.primitive());
    crate::wire::index(0, out)?;
    let spec = value.primitive().specification();
    out.name(spec.key)?;
    crate::wire::index(spec.version, out)?;
    reference_out(
        out,
        record,
        Entity::Object(cas.checked(&claim.judgment.context)?),
    )?;
    super::codec::statement(value, out, &mut |handle, out| {
        reference_out(out, record, Entity::Object(handle.logical_id()))
    })?;
    out.count(claim.hypotheses.len())?;
    for hypothesis in &claim.hypotheses {
        reference_out(
            out,
            record,
            Entity::Object(cas.checked(&hypothesis.context)?),
        )?;
        crate::wire::index(hypothesis.slot, out)?;
    }
    out.count(claim.judgment.residuals.len())?;
    let start = record.references.len();
    for residual in &claim.judgment.residuals {
        reference_out(out, record, Entity::Guard(Box::new(residual.clone())))?;
    }
    record.sets.push((start..record.references.len()).collect());
    encode_rules(&claim.rules, out)
}

/// Describe only the additive series language; existing ordinary records keep their exact bytes.
pub(crate) fn describe(cas: &Cas, entity: &Entity, budget: &mut Budget) -> Result<Record, Fault> {
    let bytes = match entity {
        Entity::SeriesStatement(proof)
        | Entity::SeriesCertificate(proof)
        | Entity::Object(proof) => crate::proof_view::application_bytes(cas, *proof, budget)?,
        Entity::Statement(judgment) => {
            let mut out = Encoder::new(budget, AT);
            crate::statement::judgment(judgment, &mut out)?;
            crate::graph::size(out.finish().len(), AT)?
                .checked_mul(4)
                .and_then(|n| n.checked_add(256))
                .ok_or_else(invalid)?
        }
        _ => return Err(invalid()),
    };
    let bytes = bytes.checked_mul(2).ok_or_else(invalid)?;
    budget.work(bytes, AT)?;
    budget.with_retained_payload(bytes, AT, |budget| describe_inner(cas, entity, budget))
}

/// Retain the checked payload beside the encoded descriptor and copied guarded dependencies.
fn describe_inner(cas: &Cas, entity: &Entity, budget: &mut Budget) -> Result<Record, Fault> {
    let proof = match entity {
        Entity::SeriesStatement(proof)
        | Entity::SeriesCertificate(proof)
        | Entity::Object(proof) => Some(*proof),
        _ => None,
    };
    let application = proof
        .map(|proof| crate::proof_view::application(cas, proof, budget))
        .transpose()?;
    let closure = proof
        .map(|proof| rules(cas, proof, budget))
        .transpose()?
        .unwrap_or_default();
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
    let mut out = Encoder::witness(budget, AT);
    match entity {
        Entity::Statement(judgment) => encode_claim(
            cas,
            &Claim {
                judgment: judgment.as_ref().clone(),
                hypotheses: Vec::new(),
                rules: Vec::new(),
            },
            &mut record,
            &mut out,
        )?,
        Entity::SeriesStatement(_) => {
            let application = application.ok_or_else(invalid)?;
            encode_claim(
                cas,
                &Claim {
                    judgment: application.judgment,
                    hypotheses: application.conditions.hypotheses,
                    rules: closure,
                },
                &mut record,
                &mut out,
            )?;
        }
        Entity::SeriesCertificate(proof) => {
            let application = application.ok_or_else(invalid)?;
            let Certificate::Series(certificate) = &application.certificate else {
                return Err(invalid());
            };
            record.tag = 40;
            reference_out(&mut out, &mut record, Entity::SeriesStatement(*proof))?;
            out.byte(certificate.mode())?;
            out.count(application.premises.len())?;
            for premise in &application.premises {
                reference_out(&mut out, &mut record, Entity::Object(*premise))?;
            }
            super::codec::certificate(certificate, &mut out)?;
        }
        Entity::Object(proof) => {
            let application = application.ok_or_else(invalid)?;
            record.tag = 41;
            reference_out(&mut out, &mut record, Entity::SeriesStatement(*proof))?;
            reference_out(&mut out, &mut record, Entity::SeriesCertificate(*proof))?;
            out.count(application.premises.len())?;
            for premise in &application.premises {
                reference_out(&mut out, &mut record, Entity::Object(*premise))?;
            }
            encode_rules(&closure, &mut out)?;
        }
        _ => return Err(invalid()),
    }
    record.payload = out.finish();
    Ok(record)
}

/// Only exact manifest members with increasing full keys may describe a used closure.
fn read_rules(
    reader: &mut Reader<'_, '_>,
    manifest: &[Primitive],
) -> Result<Vec<Primitive>, Fault> {
    let count = reader.count(BudgetKind::ProofNodes, 8, 64)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    let mut previous = "";
    for _ in 0..count {
        let key = reader.name()?;
        let version = reader.index()?;
        reader.budget.work(
            crate::graph::size(manifest.len(), AT)?.saturating_mul(64),
            AT,
        )?;
        let rule = manifest
            .iter()
            .copied()
            .find(|rule| rule.specification().key == key && rule.specification().version == version)
            .ok_or_else(unsupported)?;
        let key = rule.specification().key;
        if key <= previous {
            return Err(invalid());
        }
        previous = key;
        values.push(rule);
    }
    Ok(values)
}

/// Ordered premise slots admit ordinary applications and checked series conclusions only.
pub(crate) fn proof<'a>(
    reader: &mut Reader<'_, '_>,
    records: &'a [Decoded],
) -> Result<&'a Handle, Fault> {
    match reference(reader, records)? {
        Decoded::Application(proof) => Ok(proof),
        Decoded::Conclusion(witness) => Ok(witness.proof()),
        _ => Err(invalid()),
    }
}

/// Decode each additive record through the ordinary checking and sealing transaction.
pub(crate) fn decode(
    cas: &mut Cas,
    tag: u8,
    layer: Layer,
    space: Option<&Handle>,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
    manifest: &[Primitive],
) -> Result<Decoded, Fault> {
    if layer != Layer::Logic || space.is_some() {
        return Err(invalid());
    }
    match tag {
        39 => decode_claim(cas, layer, reader, records, manifest),
        40 => decode_certificate(cas, reader, records),
        41 => decode_conclusion(cas, reader, records, manifest),
        _ => Err(unsupported()),
    }
}

/// Decode the typed expected judgment and its complete declared condition inventory.
fn decode_claim(
    cas: &Cas,
    layer: Layer,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
    manifest: &[Primitive],
) -> Result<Decoded, Fault> {
    let rule = *manifest
        .get(usize::try_from(reader.index()?).map_err(|_| invalid())?)
        .ok_or_else(invalid)?;
    let key = reader.name()?;
    let version = reader.index()?;
    if rule.specification().key != key || rule.specification().version != version {
        return Err(unsupported());
    }
    let context = object(reader, records)?;
    let statement = super::codec::read_statement(reader, records)?;
    if statement.primitive() != rule {
        return Err(invalid());
    }
    let count = reader.count(BudgetKind::ConditionAtoms, 8, 96)?;
    let mut hypotheses = Vec::new();
    hypotheses
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        hypotheses.push(HypothesisRef {
            context: object(reader, records)?,
            slot: reader.index()?,
        });
    }
    let count = reader.count(BudgetKind::ConditionAtoms, 4, 512)?;
    let mut residuals = Vec::new();
    residuals
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        let Decoded::Guard(guard) = reference(reader, records)? else {
            return Err(invalid());
        };
        let mut out = Encoder::new(reader.budget, AT);
        crate::statement::guard(guard, &mut out, crate::statement::handle)?;
        let bytes = crate::graph::size(out.finish().len(), AT)?
            .checked_mul(4)
            .and_then(|n| n.checked_add(256))
            .ok_or_else(invalid)?;
        reader.allocation(bytes)?;
        residuals.push(guard.clone());
    }
    let judgment = Judgment {
        context,
        layer,
        space: None,
        statement: Statement::Series(Box::new(statement)),
        residuals,
    };
    crate::proof_view::validate_judgment(cas, &judgment, reader.budget)?;
    Ok(Decoded::SeriesStatement(Box::new(Claim {
        judgment,
        hypotheses,
        rules: read_rules(reader, manifest)?,
    })))
}

/// Replay the ordered mixed proof slots through the independent finite checker.
fn decode_certificate(
    cas: &mut Cas,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Decoded, Fault> {
    let Decoded::SeriesStatement(claim) = reference(reader, records)? else {
        return Err(invalid());
    };
    let mode = reader.byte()?;
    let Statement::Series(statement) = &claim.judgment.statement else {
        return Err(invalid());
    };
    let count = if matches!(
        statement.as_ref(),
        super::SeriesStatement::Reciprocal { .. }
            | super::SeriesStatement::Source(_)
            | super::SeriesStatement::EvaluatedResidual { .. }
    ) {
        let count = usize::try_from(reader.index()?).map_err(|_| invalid())?;
        if count != super::check::premise_count(cas, statement)? {
            return Err(invalid());
        }
        reader
            .budget
            .check(BudgetKind::ProofNodes, crate::graph::size(count, AT)?, AT)?;
        reader.allocation(
            crate::graph::size(count, AT)?
                .checked_mul(512)
                .ok_or_else(invalid)?,
        )?;
        count
    } else {
        reader.count(BudgetKind::ProofNodes, 4, 512)?
    };
    if count != super::check::premise_count(cas, statement)? {
        return Err(invalid());
    }
    let mut ids = Vec::new();
    let mut slots = Vec::new();
    ids.try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    slots
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    let mut bytes = 0_u64;
    for index in 0..count {
        let proof = proof(reader, records)?;
        let id = cas.checked(proof)?;
        bytes = bytes
            .checked_add(crate::proof_view::application_bytes(
                cas,
                id,
                reader.budget,
            )?)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        ids.push(id);
        slots.push(u32::try_from(index).map_err(|_| invalid())?);
    }
    if let super::SeriesStatement::Source(value) = statement.as_ref() {
        let proofs: Vec<_> = ids.iter().map(|id| cas.handle(*id)).collect();
        super::source::preflight_slots(cas, value.clause, &proofs, reader.budget)?;
    }
    if let super::SeriesStatement::Reciprocal { input, .. } = statement.as_ref() {
        // A proved nonunit precedes decoding a claimed inverse or allocating its rows.
        super::reciprocal::nonunit(cas, input, reader.budget)?;
    }
    let certificate = super::codec::read_certificate(reader)?;
    if mode != certificate.mode() {
        return Err(invalid());
    }
    let draft = ApplicationDraft {
        rule: statement.primitive(),
        context: claim.judgment.context.clone(),
        conclusion: claim.judgment.statement.clone(),
        premises: slots,
        substitutions: Vec::new(),
        certificate: Certificate::Series(Box::new(certificate)),
    };
    // Dynamic proof payloads remain reserved together, including nested checking/publication.
    reader.budget.with_retained_payload(bytes, AT, |budget| {
        let mut premises = Vec::new();
        premises
            .try_reserve_exact(count)
            .map_err(|_| Fault::allocation(AT))?;
        for id in &ids {
            premises.push(crate::proof_view::application(cas, *id, budget)?);
        }
        let mut checked = crate::scoped_check::check_application(cas, &draft, &premises, budget)?;
        checked.premises = ids;
        Ok(Decoded::Application(cas.publish_scoped(checked, budget)?))
    })
}

/// Authenticate every declared dependency and condition before sealing the ordinary witness.
fn decode_conclusion(
    cas: &Cas,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
    manifest: &[Primitive],
) -> Result<Decoded, Fault> {
    let Decoded::SeriesStatement(claim) = reference(reader, records)? else {
        return Err(invalid());
    };
    let Decoded::Application(proof) = reference(reader, records)? else {
        return Err(invalid());
    };
    if matches!(&claim.judgment.statement, Statement::Series(value) if matches!(value.as_ref(), super::SeriesStatement::Source(_)))
    {
        return source_conclusion(cas, claim, proof, reader, records, manifest);
    }
    let application = crate::proof_view::application(cas, cas.checked(proof)?, reader.budget)?;
    if !matches!(application.judgment.statement, Statement::Series(_)) {
        return Err(invalid());
    }
    let count = reader.count(BudgetKind::ProofNodes, 4, 64)?;
    if count != application.premises.len() {
        return Err(invalid());
    }
    for expected in &application.premises {
        if cas.checked(self::proof(reader, records)?)? != *expected {
            return Err(invalid());
        }
    }
    let declared = read_rules(reader, manifest)?;
    let mut out = Encoder::new(reader.budget, AT);
    crate::statement::judgment(&claim.judgment, &mut out)?;
    let bytes = crate::graph::size(out.finish().len(), AT)?;
    reader.budget.work(
        bytes.saturating_add(crate::graph::size(claim.hypotheses.len(), AT)?.saturating_mul(32)),
        AT,
    )?;
    if application.judgment != claim.judgment
        || application.conditions.hypotheses != claim.hypotheses
        || declared != claim.rules
        || declared != rules(cas, proof.logical_id(), reader.budget)?
    {
        return Err(invalid());
    }
    Ok(Decoded::Conclusion(cas.seal_scoped(proof, reader.budget)?))
}

/// Borrow the new source application's typed arity before sizing or copying its payload.
fn source_conclusion(
    cas: &Cas,
    claim: &Claim,
    proof: &Handle,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
    manifest: &[Primitive],
) -> Result<Decoded, Fault> {
    let Vertex::Proof(id) = cas.state.node(cas.checked(proof)?)? else {
        return Err(invalid());
    };
    let application = cas.series_application(*id)?;
    let Statement::Series(statement) = &claim.judgment.statement else {
        return Err(invalid());
    };
    let count = usize::try_from(reader.index()?).map_err(|_| invalid())?;
    if count != super::check::premise_count(cas, statement)? || count != application.premises.len()
    {
        return Err(invalid());
    }
    for expected in &application.premises {
        if cas.checked(self::proof(reader, records)?)? != *expected {
            return Err(invalid());
        }
    }
    let declared = read_rules(reader, manifest)?;
    let mut out = Encoder::new(reader.budget, AT);
    crate::statement::judgment(&claim.judgment, &mut out)?;
    let bytes = crate::graph::size(out.finish().len(), AT)?;
    reader.budget.work(
        bytes
            .checked_add(
                crate::graph::size(claim.hypotheses.len(), AT)?
                    .checked_mul(32)
                    .ok_or_else(invalid)?,
            )
            .ok_or_else(invalid)?,
        AT,
    )?;
    if application.judgment != claim.judgment
        || application.conditions.hypotheses != claim.hypotheses
        || declared != claim.rules
        || declared != rules(cas, proof.logical_id(), reader.budget)?
    {
        return Err(invalid());
    }
    let bytes = crate::proof_view::application_bytes(cas, cas.checked(proof)?, reader.budget)?;
    reader.budget.with_retained_payload(bytes, AT, |budget| {
        Ok(Decoded::Conclusion(cas.seal_scoped(proof, budget)?))
    })
}
