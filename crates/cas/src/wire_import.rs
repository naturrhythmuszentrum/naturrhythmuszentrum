// SPDX-License-Identifier: AGPL-3.0-only
//! Unpublished mathematical table import; every claim is reconstructed afterward.

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{AssumptionDraft, Cas, Draft, ExpressionDraft, Handle, LogicalId, Reference};
use crate::model::{PredicateKind, Scope};
use crate::proof_model::ProofStep;
use crate::rule::{PRIMITIVES, Primitive};
use crate::substitution::Substitution;
use crate::wire_read::{Reader, invalid};

/// Decoder location for local-reference failures.
const AT: Location = Location::new("witness table import");

/// A raw hypothesis whose term references are checked after the bounded term table.
struct Hypothesis {
    /// Exact declared provenance key.
    key: String,
    /// Typed predicate syntax.
    kind: PredicateKind,
    /// Explicit scope.
    scope: Scope,
    /// Portable term indices.
    arguments: Vec<u32>,
}

/// Decoded input to the independent primitive checker, with no trusted status flags.
pub(crate) struct Decoded {
    /// Full imported context.
    pub(crate) context: LogicalId,
    /// Ordered untrusted proof applications.
    pub(crate) steps: Vec<ProofStep>,
}

/// A bounded mathematical import and its local-to-store handle map.
struct Mathematics {
    /// Full context after all condition arguments are admitted.
    context: LogicalId,
    /// Term table in exactly the untrusted wire order.
    terms: Vec<Handle>,
}

/// Refuse unknown primitive keys or semantic versions instead of guessing an upgrade.
fn manifest(reader: &mut Reader<'_, '_>) -> Result<Vec<Primitive>, Fault> {
    let count = reader.count(BudgetKind::ProofNodes, 8, 16)?;
    let mut rules = Vec::new();
    rules
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        let key = reader.name()?;
        let version = reader.index()?;
        let primitive = PRIMITIVES
            .iter()
            .copied()
            .find(|primitive| {
                let specification = primitive.specification();
                specification.key == key && specification.version == version
            })
            .ok_or_else(|| Fault::invalid(InvalidCode::UnsupportedVersion, AT))?;
        rules.push(primitive);
    }
    Ok(rules)
}

/// Decode condition records before resolving their cross-table references.
fn hypotheses(reader: &mut Reader<'_, '_>) -> Result<Vec<Hypothesis>, Fault> {
    let count = reader.count(BudgetKind::ConditionAtoms, 13, 256)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        result.push(Hypothesis {
            key: reader.name()?,
            kind: reader.predicate()?,
            scope: reader.scope()?,
            arguments: reader.indices(BudgetKind::Edges)?,
        });
    }
    Ok(result)
}

/// Convert an earlier wire term reference to its draft offset after the base context.
fn local_term(index: u32, count: usize) -> Result<Reference, Fault> {
    if usize::try_from(index).map_err(|_| invalid())? >= count {
        return Err(invalid());
    }
    Ok(Reference::Local(index.checked_add(1).ok_or_else(invalid)?))
}

/// Decode mathematical syntax through the ordinary checked graph-admission boundary.
fn mathematics(cas: &mut Cas, reader: &mut Reader<'_, '_>) -> Result<Mathematics, Fault> {
    let environment = reader.environment()?;
    let hypotheses = hypotheses(reader)?;
    let count = reader.count(BudgetKind::Vertices, 12, 512)?;
    let mut drafts = Vec::new();
    drafts
        .try_reserve_exact(
            count
                .checked_add(hypotheses.len())
                .and_then(|n| n.checked_add(2))
                .ok_or_else(invalid)?,
        )
        .map_err(|_| Fault::allocation(AT))?;
    drafts.push(Draft::Context {
        environment: environment.clone(),
        assumptions: Vec::new(),
    });
    for current in 0..count {
        let operator = reader.operator()?;
        let sort = reader.sort()?;
        let scope = reader.scope()?;
        let binding = reader.binding()?;
        let references = reader.indices(BudgetKind::Edges)?;
        let mut operands = Vec::new();
        operands
            .try_reserve_exact(references.len())
            .map_err(|_| Fault::allocation(AT))?;
        for reference in references {
            operands.push(local_term(reference, current)?);
        }
        drafts.push(Draft::Expression(ExpressionDraft {
            operator,
            sort,
            context: Reference::Local(0),
            scope,
            binding,
            operands,
        }));
    }
    let assumptions = append_hypotheses(&mut drafts, hypotheses, count)?;
    drafts.push(Draft::Context {
        environment,
        assumptions,
    });
    let previous = reader
        .budget
        .reserve_temporary(cas.counts().payload_bytes, AT)?;
    let admitted = cas.admit(&drafts, reader.budget);
    reader.budget.restore_limits(previous);
    let admitted = admitted?;
    let context = cas.checked(admitted.last().ok_or_else(invalid)?)?;
    let mut terms = Vec::new();
    terms
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for handle in admitted.iter().skip(1).take(count) {
        terms.push(handle.clone());
    }
    Ok(Mathematics { context, terms })
}

/// Add typed assumptions after all of their referenced terms exist in the draft batch.
fn append_hypotheses(
    drafts: &mut Vec<Draft>,
    hypotheses: Vec<Hypothesis>,
    count: usize,
) -> Result<Vec<AssumptionDraft>, Fault> {
    let mut assumptions = Vec::new();
    assumptions
        .try_reserve_exact(hypotheses.len())
        .map_err(|_| Fault::allocation(AT))?;
    for hypothesis in hypotheses {
        let mut arguments = Vec::new();
        arguments
            .try_reserve_exact(hypothesis.arguments.len())
            .map_err(|_| Fault::allocation(AT))?;
        for reference in hypothesis.arguments {
            arguments.push(local_term(reference, count)?);
        }
        assumptions.push(AssumptionDraft {
            key: hypothesis.key,
            condition: Reference::Local(u32::try_from(drafts.len()).map_err(|_| invalid())?),
        });
        drafts.push(Draft::Condition {
            kind: hypothesis.kind,
            context: Reference::Local(0),
            scope: hypothesis.scope,
            arguments,
        });
    }
    Ok(assumptions)
}

/// Resolve a local term without truncation or unchecked indexing.
fn term(terms: &[Handle], reference: u32) -> Result<Handle, Fault> {
    usize::try_from(reference)
        .ok()
        .and_then(|index| terms.get(index))
        .cloned()
        .ok_or_else(invalid)
}

/// Read displayed claims as untrusted metadata; re-encoding will compare every byte.
fn conclusion(reader: &mut Reader<'_, '_>, terms: &[Handle]) -> Result<(), Fault> {
    let _relation = reader.relation()?;
    let _left = term(terms, reader.index()?)?;
    let _right = term(terms, reader.index()?)?;
    let count = reader.count(BudgetKind::ConditionAtoms, 5, 16)?;
    for _ in 0..count {
        if reader.byte()? > 1 {
            return Err(invalid());
        }
        let _term = term(terms, reader.index()?)?;
    }
    let count = reader.count(BudgetKind::ConditionAtoms, 4, 32)?;
    for _ in 0..count {
        let _key = reader.name()?;
    }
    Ok(())
}

/// Decode one backward-referencing application without trusting its displayed endpoints.
fn proof_step(
    reader: &mut Reader<'_, '_>,
    rules: &[Primitive],
    terms: &[Handle],
    current: usize,
) -> Result<ProofStep, Fault> {
    let primitive = usize::try_from(reader.index()?)
        .ok()
        .and_then(|index| rules.get(index))
        .copied()
        .ok_or_else(invalid)?;
    let relation = reader.relation()?;
    let left = term(terms, reader.index()?)?;
    let right = term(terms, reader.index()?)?;
    let premises = reader.indices(BudgetKind::ProofNodes)?;
    if premises
        .iter()
        .any(|index| usize::try_from(*index).map_or(true, |index| index >= current))
    {
        return Err(invalid());
    }
    let path = reader.indices(BudgetKind::Depth)?;
    let count = reader.count(BudgetKind::Vertices, 8, 128)?;
    let mut substitutions = Vec::new();
    substitutions
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        substitutions.push(Substitution {
            key: reader.name()?,
            replacement: term(terms, reader.index()?)?,
        });
    }
    Ok(ProofStep {
        primitive,
        relation,
        left,
        right,
        premises,
        path,
        substitutions,
    })
}

/// Decode only in the caller's private transaction candidate; no prefix is a proof.
pub(crate) fn decode(cas: &mut Cas, bytes: &[u8], budget: &mut Budget) -> Result<Decoded, Fault> {
    let mut reader = Reader::new(bytes, budget)?;
    if reader.take(8)? != b"NRZCAS01" {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
    }
    let rules = manifest(&mut reader)?;
    let mathematics = mathematics(cas, &mut reader)?;
    if reader.boolean()? {
        return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
    }
    conclusion(&mut reader, &mathematics.terms)?;
    let count = reader.count(BudgetKind::ProofNodes, 25, 512)?;
    let mut steps = Vec::new();
    steps
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for current in 0..count {
        steps.push(proof_step(
            &mut reader,
            &rules,
            &mathematics.terms,
            current,
        )?);
    }
    let final_index = usize::try_from(reader.index()?).map_err(|_| invalid())?;
    if count.checked_sub(1) != Some(final_index) {
        return Err(invalid());
    }
    reader.finished()?;
    Ok(Decoded {
        context: mathematics.context,
        steps,
    })
}
