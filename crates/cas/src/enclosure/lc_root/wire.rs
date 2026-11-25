// SPDX-License-Identifier: AGPL-3.0-only
//! Portable original syntax and explicit closed-schema fields; no source-store identities.

use super::{
    AT, Candidate, Events, Native, Predicate, RECIPE, Relation, Request, Step, Term, invalid,
};
use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::wire_read::Reader;
use crate::{Cas, Fault, InvalidCode};

/// Bind every original slot to canonical syntax, including the complete conditional context.
pub(super) fn scope(cas: &Cas, request: &Request, budget: &mut Budget) -> Result<Vec<u8>, Fault> {
    super::capacity(request, budget)?;
    let count = request
        .inputs
        .len()
        .saturating_add(request.constraints.len())
        .saturating_add(11);
    let headers = (count as u64)
        .saturating_add(1)
        .saturating_mul(size_of::<Vec<u8>>() as u64);
    budget.check(BudgetKind::StagedPayloadBytes, headers, AT)?;
    let mut blobs = Vec::new();
    blobs
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    let mut live = headers;
    for handle in request.handles() {
        let blob = budget.with_retained_payload(live, AT, |budget| {
            crate::wire_build::syntax_key(cas, cas.checked(handle, budget)?, budget)
        })?;
        live = live.saturating_add(blob.capacity() as u64);
        budget.check(BudgetKind::StagedPayloadBytes, live, AT)?;
        blobs.push(blob);
    }
    budget.with_retained_payload(live, AT, |budget| {
        let mut out = Encoder::witness(budget, AT);
        out.unsigned(u64::from(request.arithmetic))?;
        out.count(request.inputs.len())?;
        out.count(request.constraints.len())?;
        for position in request.positions {
            out.count(position)?;
        }
        for blob in &blobs {
            out.count(blob.len())?;
            out.raw(blob)?;
        }
        Ok(out.finish())
    })
}

/// Encode the proposed mathematical fields, not a producer-selected receipt identity.
pub(super) fn encode(
    candidate: &Candidate,
    scope: &[u8],
    budget: &mut Budget,
) -> Result<Vec<u8>, Fault> {
    let mut indices = Vec::new();
    let bytes = (candidate.universal.len() as u64)
        .saturating_mul(4)
        .saturating_add(size_of::<Vec<u32>>() as u64);
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    indices
        .try_reserve_exact(candidate.universal.len())
        .map_err(|_| Fault::allocation(AT))?;
    for binder in &candidate.universal {
        let mut found = None;
        for (index, original) in candidate.original.handles().enumerate() {
            budget.work(1, AT)?;
            if original == binder {
                found = Some(index);
                break;
            }
        }
        indices.push(u32::try_from(found.ok_or_else(invalid)?).map_err(|_| invalid())?);
    }
    budget.with_retained_payload(bytes, AT, |budget| {
        let mut out = Encoder::witness(budget, AT);
        out.raw(b"NRZLCR01")?;
        out.name(candidate.recipe)?;
        out.unsigned(u64::from(candidate.version))?;
        out.count(scope.len())?;
        out.raw(scope)?;
        out.count(indices.len())?;
        for index in indices {
            out.count(index as usize)?;
        }
        for predicate in candidate.premises.iter().chain(&candidate.conclusions) {
            term(predicate.0, &mut out)?;
            out.byte(match predicate.1 {
                Relation::Less => 0,
                Relation::AtMost => 1,
                Relation::Equal => 2,
            })?;
            term(predicate.2, &mut out)?;
        }
        for native in candidate.native {
            out.byte(match native {
                Native::ProductTotal => 0,
                Native::RootTotal => 1,
                Native::ReciprocalTotal => 2,
                Native::ReciprocalLocalOperation => 3,
            })?;
        }
        for step in candidate.expansion {
            out.byte(match step {
                Step::ProductPositive => 0,
                Step::RootUnique => 1,
                Step::RootBracket => 2,
                Step::ReciprocalOrder => 3,
                Step::ReciprocalBracket => 4,
            })?;
        }
        Ok(out.finish())
    })
}

/// Stable two-byte mathematical term encoding.
fn term(term: Term, out: &mut Encoder<'_>) -> Result<(), Fault> {
    let (tag, index) = match term {
        Term::Zero => (0, 0),
        Term::L => (1, 0),
        Term::C => (2, 0),
        Term::Product => (3, 0),
        Term::Root => (4, 0),
        Term::Reciprocal => (5, 0),
        Term::Reverse => (6, 0),
        Term::Endpoint(index) => (7, index),
        Term::Square(index) => (8, index),
        Term::Inverse(index) => (9, index),
    };
    out.byte(tag)?;
    out.byte(index)
}

/// Decode against independently reconstructed receiving scope without calling the producer.
pub(super) fn decode(
    request: &Request,
    scope: &[u8],
    bytes: &[u8],
    budget: &mut Budget,
) -> Result<Candidate, Fault> {
    let original = super::claim::copy(request, budget, &mut Events::default())?;
    let mut reader = Reader::new(bytes, budget)?;
    if reader.take(8)? != b"NRZLCR01" || reader.name()? != RECIPE {
        return Err(invalid());
    }
    if reader.unsigned()? != 1 {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
    }
    let length = reader.index()? as usize;
    let supplied_scope = reader.take(length)?;
    let revision: [u8; 8] = supplied_scope
        .get(..8)
        .ok_or_else(invalid)?
        .try_into()
        .map_err(|_| invalid())?;
    if u64::from_le_bytes(revision) != 1 {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
    }
    if supplied_scope != scope {
        return Err(invalid());
    }
    let count = reader.count(
        BudgetKind::MatrixEntries,
        4,
        size_of::<crate::Handle>() as u64,
    )?;
    let mut universal = Vec::new();
    universal
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        let index = reader.index()? as usize;
        let handle = request.handles().nth(index).ok_or_else(invalid)?;
        universal.push(handle.clone());
    }
    let mut premises = [Predicate(Term::Zero, Relation::Equal, Term::Zero); 11];
    let mut conclusions = [Predicate(Term::Zero, Relation::Equal, Term::Zero); 7];
    for predicate in premises.iter_mut().chain(&mut conclusions) {
        let left = read_term(&mut reader)?;
        let relation = match reader.byte()? {
            0 => Relation::Less,
            1 => Relation::AtMost,
            2 => Relation::Equal,
            _ => return Err(invalid()),
        };
        *predicate = Predicate(left, relation, read_term(&mut reader)?);
    }
    let mut native = [Native::ProductTotal; 4];
    for value in &mut native {
        *value = match reader.byte()? {
            0 => Native::ProductTotal,
            1 => Native::RootTotal,
            2 => Native::ReciprocalTotal,
            3 => Native::ReciprocalLocalOperation,
            _ => return Err(invalid()),
        };
    }
    let mut expansion = [Step::ProductPositive; 5];
    for value in &mut expansion {
        *value = match reader.byte()? {
            0 => Step::ProductPositive,
            1 => Step::RootUnique,
            2 => Step::RootBracket,
            3 => Step::ReciprocalOrder,
            4 => Step::ReciprocalBracket,
            _ => return Err(invalid()),
        };
    }
    reader.finished()?;
    Ok(Candidate {
        recipe: RECIPE,
        version: 1,
        original,
        universal,
        premises,
        native,
        conclusions,
        expansion,
    })
}

/// Reject unknown terms and noncanonical padding before mathematical checking.
fn read_term(reader: &mut Reader<'_, '_>) -> Result<Term, Fault> {
    let tag = reader.byte()?;
    let index = reader.byte()?;
    if tag <= 6 && index != 0 {
        return Err(invalid());
    }
    Ok(match tag {
        0 => Term::Zero,
        1 => Term::L,
        2 => Term::C,
        3 => Term::Product,
        4 => Term::Root,
        5 => Term::Reciprocal,
        6 => Term::Reverse,
        7 => Term::Endpoint(index),
        8 => Term::Square(index),
        9 => Term::Inverse(index),
        _ => return Err(invalid()),
    })
}
