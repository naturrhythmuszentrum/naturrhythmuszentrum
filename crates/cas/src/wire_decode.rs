// SPDX-License-Identifier: AGPL-3.0-only
//! Independent bounded import of the sole typed witness language.

use std::collections::BTreeSet;

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::{HypothesisRef, Layer};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, size};
use crate::proof_model::{ApplicationDraft, GuardedObligation, Judgment, VerifiedWitness};
use crate::rule::{PRIMITIVES, Primitive};
use crate::wire_read::{Reader, invalid};

/// Static untrusted-input location.
const AT: Location = Location::new("typed witness import");

/// Only earlier, successfully admitted records can be referenced.
#[derive(Debug)]
pub(crate) enum Decoded {
    /// Mathematical graph object, never a receipt.
    Object(Handle),
    /// Complete guarded predicate.
    Guard(GuardedObligation),
    /// Typed proposition with empty residuals until conclusion reconstruction.
    Statement(Judgment),
    /// Independently checked application proof.
    Application(Handle),
    /// Complete checked root theorem.
    Conclusion(VerifiedWitness),
}

/// A typed reference cannot target a later record or the wrong record category.
pub(crate) fn reference<'a>(
    reader: &mut Reader<'_, '_>,
    records: &'a [Decoded],
) -> Result<&'a Decoded, Fault> {
    let index = usize::try_from(reader.index()?).map_err(|_| invalid())?;
    records.get(index).ok_or_else(invalid)
}

/// Read an earlier semantic object reference.
pub(crate) fn object(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Handle, Fault> {
    let Decoded::Object(value) = reference(reader, records)? else {
        return Err(invalid());
    };
    Ok(value.clone())
}

/// Read an explicitly absent or present semantic object.
pub(crate) fn optional(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Option<Handle>, Fault> {
    if reader.boolean()? {
        Ok(Some(object(reader, records)?))
    } else {
        Ok(None)
    }
}

/// Bound a semantic object vector before reservation or iteration.
pub(crate) fn objects(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
    kind: BudgetKind,
) -> Result<Vec<Handle>, Fault> {
    let count = reader.count(kind, 4, 64)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        result.push(object(reader, records)?);
    }
    Ok(result)
}

/// Read a sorted manifest containing only executable exact semantic versions.
fn manifest(reader: &mut Reader<'_, '_>) -> Result<Vec<Primitive>, Fault> {
    let count = reader.count(BudgetKind::ProofNodes, 8, 64)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    let mut previous = String::new();
    for _ in 0..count {
        let key = reader.name()?;
        let version = reader.index()?;
        if key <= previous {
            return Err(invalid());
        }
        let primitive = PRIMITIVES
            .iter()
            .find(|rule| {
                let spec = rule.specification();
                spec.key == key && spec.version == version && spec.available
            })
            .copied()
            .ok_or_else(|| Fault::invalid(InvalidCode::UnsupportedVersion, AT))?;
        previous = key;
        result.push(primitive);
    }
    Ok(result)
}

/// Reconstruct the entire table, then require exact canonical bytes before the caller publishes it.
pub(crate) fn inspect(
    cas: &mut Cas,
    bytes: &[u8],
    budget: &mut Budget,
) -> Result<VerifiedWitness, Fault> {
    #[cfg(test)]
    let _attribution = crate::testing::work_trace::Scope::phase("wire admission");
    magic(bytes)?;
    let mut reader = Reader::new(bytes, budget)?;
    reader.take(8)?;
    let manifest = manifest(&mut reader)?;
    let count = reader.count(BudgetKind::Vertices, 11, 512)?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    let mut unique = BTreeSet::new();
    for _ in 0..count {
        let tag = reader.byte()?;
        let length = reader.count(BudgetKind::WitnessBytes, 1, 2)?;
        let payload = reader.take(length)?;
        reader.budget.work(
            size(payload.len(), AT)?
                .checked_mul(size(unique.len(), AT)?.checked_add(1).ok_or_else(invalid)?)
                .ok_or_else(invalid)?,
            AT,
        )?;
        if !unique.insert((tag, payload)) {
            return Err(invalid());
        }
        let mut part = Reader::new(payload, reader.budget)?;
        if part.index()? != 1 {
            return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
        }
        let layer = match part.byte()? {
            0 => Layer::Logic,
            1 => Layer::Pointwise,
            2 => Layer::Distribution,
            _ => return Err(invalid()),
        };
        let space = optional(&mut part, &records)?;
        if let Some(space) = &space {
            cas.state.space(cas.checked(space)?)?;
        }
        let record = decode(cas, tag, layer, space, &mut part, &records, &manifest)?;
        part.finished()?;
        records.push(record);
    }
    let root = usize::try_from(reader.index()?).map_err(|_| invalid())?;
    reader.finished()?;
    let Decoded::Conclusion(witness) = records.get(root).ok_or_else(invalid)? else {
        return Err(invalid());
    };
    let canonical = crate::wire::encode(cas, cas.checked(witness.proof())?, reader.budget)?;
    if canonical != bytes {
        return Err(invalid());
    }
    Ok(witness.clone())
}

/// Interpret only the admitted record language, never serialized proof-status flags.
fn decode(
    cas: &mut Cas,
    tag: u8,
    layer: Layer,
    space: Option<Handle>,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
    manifest: &[Primitive],
) -> Result<Decoded, Fault> {
    match tag {
        0..=8 | 13..=36 => {
            crate::wire_objects::decode(cas, tag, layer, space.as_ref(), reader, records)
                .map(Decoded::Object)
        }
        9 => {
            let value = GuardedObligation {
                home: object(reader, records)?,
                scope: reader.scope()?,
                layer,
                space,
                region: object(reader, records)?,
                predicate: object(reader, records)?,
            };
            crate::statement::validate_guard(cas, &value, reader.budget)?;
            Ok(Decoded::Guard(value))
        }
        10 => {
            let value = Judgment {
                context: object(reader, records)?,
                layer,
                space,
                statement: crate::wire_statement::statement(reader, records)?,
                residuals: Vec::new(),
            };
            crate::proof_view::validate_judgment(cas, &value, reader.budget)?;
            Ok(Decoded::Statement(value))
        }
        11 => application(cas, layer, space.as_ref(), reader, records, manifest),
        12 => conclusion(cas, layer, space.as_ref(), reader, records),
        _ => Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT)),
    }
}

/// Invoke the same independent primitive matcher on already checked premise records.
fn application(
    cas: &mut Cas,
    layer: Layer,
    space: Option<&Handle>,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
    manifest: &[Primitive],
) -> Result<Decoded, Fault> {
    #[cfg(test)]
    let _attribution = crate::testing::work_trace::Scope::phase("application reconstruction");
    let rule = *manifest
        .get(usize::try_from(reader.index()?).map_err(|_| invalid())?)
        .ok_or_else(invalid)?;
    let Decoded::Statement(judgment) = reference(reader, records)? else {
        return Err(invalid());
    };
    if judgment.layer != layer || judgment.space.as_ref() != space {
        return Err(invalid());
    }
    let count = reader.count(BudgetKind::ProofNodes, 4, 512)?;
    let mut premises = Vec::new();
    let mut ids = Vec::new();
    premises
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    ids.try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        let Decoded::Application(proof) = reference(reader, records)? else {
            return Err(invalid());
        };
        ids.push(cas.checked(proof)?);
        premises.push(crate::proof_view::application(
            cas,
            proof.logical_id(),
            reader.budget,
        )?);
    }
    let substitutions = crate::wire_statement::substitutions(reader, records)?;
    let certificate = crate::wire_statement::certificate(reader, records)?;
    let local: Vec<_> = (0..count)
        .map(|index| u32::try_from(index).map_err(|_| invalid()))
        .collect::<Result<_, _>>()?;
    let draft = ApplicationDraft {
        rule,
        context: judgment.context.clone(),
        conclusion: judgment.statement.clone(),
        premises: local,
        substitutions,
        certificate,
    };
    #[cfg(test)]
    let before = reader.budget.usage();
    let mut checked =
        crate::scoped_check::check_application(cas, &draft, &premises, reader.budget)?;
    #[cfg(test)]
    crate::testing::replay_answer::record(rule, &before, &reader.budget.usage())?;
    checked.premises = ids;
    Ok(Decoded::Application(
        cas.publish_scoped(checked, reader.budget)?,
    ))
}

/// Compare reconstructed conditions and qualified hypotheses before sealing the root theorem.
fn conclusion(
    cas: &Cas,
    layer: Layer,
    space: Option<&Handle>,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Decoded, Fault> {
    #[cfg(test)]
    let _attribution = crate::testing::work_trace::Scope::phase("conclusion reconstruction");
    if !reader.boolean()? {
        return Err(invalid());
    }
    let Decoded::Application(proof) = reference(reader, records)? else {
        return Err(invalid());
    };
    let Decoded::Statement(statement) = reference(reader, records)? else {
        return Err(invalid());
    };
    let mut expected = statement.clone();
    let count = reader.count(BudgetKind::ConditionAtoms, 4, 512)?;
    expected
        .residuals
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        let Decoded::Guard(guard) = reference(reader, records)? else {
            return Err(invalid());
        };
        expected.residuals.push(guard.clone());
    }
    let count = reader.count(BudgetKind::ConditionAtoms, 12, 96)?;
    let mut hypotheses = Vec::new();
    hypotheses
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        hypotheses.push(HypothesisRef {
            context: object(reader, records)?,
            slot: reader.small()?,
        });
    }
    let witness = cas.seal_scoped(proof, reader.budget)?;
    expected.residuals = crate::statement::union(cas, &expected.residuals, reader.budget)?;
    if expected.layer != layer
        || expected.space.as_ref() != space
        || expected != witness.judgment
        || hypotheses != witness.hypotheses
    {
        return Err(Fault::invalid(InvalidCode::ConditionMismatch, AT));
    }
    Ok(Decoded::Conclusion(witness))
}

/// Reject every noncurrent magic before any record, manifest or replay-store allocation.
pub(crate) fn magic(bytes: &[u8]) -> Result<(), Fault> {
    if bytes.get(..8) != Some(b"NRZCAS04".as_slice()) {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
    }
    Ok(())
}
