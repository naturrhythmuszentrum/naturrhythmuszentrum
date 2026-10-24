// SPDX-License-Identifier: AGPL-3.0-only
//! Exact request-local statement nodes cite their original checked proof and evidence closure.

use crate::budget::{Budget, BudgetKind, Limits};
use crate::encoding::Encoder;
use crate::error::{Fault, Location};
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::proof_model::{ApplicationDraft, Judgment, VerifiedWitness};
use crate::rule::Primitive;
use crate::wire_read::invalid;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Key comparisons and retained graph citations remain visible on the caller ledger.
const AT: Location = Location::new("reading proof memo");

/// Canonical local statement nodes are interpreted only in their immutable graph namespace.
#[derive(Debug, Default)]
pub(crate) struct Cache {
    /// Store identity is retained, so an allocator cannot recycle it into a false match.
    owner: Option<Arc<()>>,
    /// Rollbacks invalidate every citation before a logical serial can be reused.
    epoch: u64,
    /// Complete statements cite the original checked proof, never a reconstructed weaker claim.
    entries: BTreeMap<Vec<u8>, Entry>,
    /// Actual keys plus conservative tree, policy and handle allocation allowance.
    pub(crate) bytes: u64,
}

/// Exact policy equality prevents reuse from bypassing a newly restricted structural ceiling.
#[derive(Debug)]
struct Entry {
    /// Original checked evidence remains reachable in the immutable source graph.
    proof: Handle,
    /// Resource frame under which the source theorem was checked.
    limits: Limits,
}

/// Expired graph namespaces cannot retain usable facts or consume stale cache allowance.
fn prepare(cas: &Cas, budget: &mut Budget) {
    let cache = &mut budget.reading_proofs;
    let same_owner = cache
        .owner
        .as_ref()
        .is_some_and(|owner| Arc::ptr_eq(owner, &cas.memo_owner));
    if !same_owner || cache.epoch != cas.memo_epoch {
        cache.entries.clear();
        cache.bytes = 0;
        cache.owner = Some(Arc::clone(&cas.memo_owner));
        cache.epoch = cas.memo_epoch;
    }
}

/// The complete graph statement contains context, declarations, stratum and every explicit premise.
/// Immutable canonical graph references identify full subgraphs inside this owner/epoch only.
/// Representation and the sole producer rule revision qualify that graph identity explicitly.
pub(crate) fn key(
    judgment: &Judgment,
    premises: &[LogicalId],
    representation: &[u8],
    revision: u32,
    budget: &mut Budget,
) -> Result<Vec<u8>, Fault> {
    let mut out = Encoder::new(budget, AT);
    out.count(representation.len())?;
    out.raw(representation)?;
    out.name(Primitive::Jets.specification().key)?;
    out.unsigned(u64::from(revision))?;
    crate::statement::judgment(judgment, &mut out)?;
    out.count(premises.len())?;
    for premise in premises {
        crate::graph::encode_id(*premise, &mut out)?;
    }
    // This producer has no substitutions, auxiliary certificate or imported hypotheses.
    out.count(0)?;
    crate::statement::certificate(
        &crate::proof_model::Certificate::None,
        &mut out,
        crate::statement::handle,
    )?;
    out.count(0)?;
    Ok(out.finish())
}

/// Cite a complete statement under the same policy, or report whether its key has no prior entry.
/// A policy mismatch keeps the original citation without charging duplicate retained storage.
fn lookup(cas: &Cas, key: &[u8], budget: &mut Budget) -> Result<(Option<Handle>, bool), Fault> {
    if budget.reading_proofs.entries.is_empty() {
        return Ok((None, true));
    }
    let comparisons = size(budget.reading_proofs.entries.len(), AT)?
        .bit_width()
        .max(1);
    budget.work(
        size(key.len(), AT)?.saturating_mul(u64::from(comparisons)),
        AT,
    )?;
    budget.check(BudgetKind::StagedPayloadBytes, 0, AT)?;
    let Some(entry) = budget.reading_proofs.entries.get(key) else {
        return Ok((None, true));
    };
    let policy = entry.limits.clone();
    let proof = entry.proof.clone();
    budget.work(size(std::mem::size_of::<Limits>(), AT)?, AT)?;
    if policy != *budget.limits() {
        return Ok((None, false));
    }
    cas.checked(&proof)?;

    Ok((Some(proof), false))
}

/// Reserve the complete citation before checking, so successful graph publication has no fallible tail.
fn reserve(key: &[u8], capacity: usize, budget: &mut Budget) -> Result<(u64, u64), Fault> {
    let count = size(budget.reading_proofs.entries.len(), AT)?;
    let bytes = size(capacity, AT)?
        .saturating_add(size(std::mem::size_of::<Entry>(), AT)?)
        .saturating_add(128);
    budget.check(BudgetKind::Vertices, count.saturating_add(1), AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(
        size(key.len(), AT)?
            .saturating_mul(u64::from(count.bit_width()))
            .saturating_add(1),
        AT,
    )?;
    let total = budget
        .reading_proofs
        .bytes
        .checked_add(bytes)
        .ok_or_else(invalid)?;
    Ok((bytes, total))
}

/// Move a completely reserved citation into the memo after ordinary proof publication succeeds.
fn retain(
    key: Vec<u8>,
    witness: &VerifiedWitness,
    limits: Limits,
    total: u64,
    budget: &mut Budget,
) {
    budget.reading_proofs.entries.insert(
        key,
        Entry {
            proof: witness.proof().clone(),
            limits,
        },
    );
    budget.reading_proofs.bytes = total;
}

/// Successful ordered-reading checks retain their exact proof root; all misses use the ordinary checker.
pub(crate) fn prove(
    cas: &mut Cas,
    step: &ApplicationDraft,
    budget: &mut Budget,
) -> Result<VerifiedWitness, Fault> {
    cas.proof_limits(budget, |cas, budget| {
        prepare(cas, budget);
        if !budget.reading_proofs.entries.is_empty() {
            budget.witness_records.enabled = true;
        }
        let expected = Judgment {
            context: step.context.clone(),
            layer: crate::statement::layer(&step.conclusion),
            space: crate::statement::space(&step.conclusion),
            statement: step.conclusion.clone(),
            residuals: Vec::new(),
        };
        let key = key(
            &expected,
            &[],
            crate::wire::MAGIC,
            Primitive::Jets.specification().version,
            budget,
        )?;
        budget.with_retained_payload(size(key.capacity(), AT)?, AT, |budget| {
            let (proof, vacant) = lookup(cas, &key, budget)?;
            if let Some(proof) = proof {
                return cas.seal_scoped(&proof, budget);
            }
            if !vacant {
                return cas.check_proof(&step.context, std::slice::from_ref(step), budget);
            }
            let limits = budget.limits().clone();
            let (bytes, total) = reserve(&key, key.capacity(), budget)?;
            let witness = budget.with_retained_payload(bytes, AT, |budget| {
                cas.check_proof(&step.context, std::slice::from_ref(step), budget)
            })?;
            let exact = witness.judgment() == &expected;
            if exact && witness.is_unconditional() && cas.memo_epoch != u64::MAX {
                retain(key, &witness, limits, total, budget);
            }
            Ok(witness)
        })
    })
}
