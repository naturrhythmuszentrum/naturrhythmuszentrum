// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical proof tables, exact expected-claim keys and independent import.

use std::collections::{BTreeMap, BTreeSet};

use crate::budget::{Budget, BudgetKind};
use crate::conditions::GuardKind;
use crate::encoding::{self, Encoder};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, LogicalId, Vertex, size};
use crate::proof_model::{Claim, Compiled, Guard, Relation};
use crate::rule::Primitive;

pub(crate) use crate::wire_import::decode;

/// Stable codec location.
const AT: Location = Location::new("canonical witness");

/// Checked local references never depend on a graph's physical indices or admission serials.
#[derive(Default)]
pub(crate) struct Terms {
    /// Canonical child-before-parent term order.
    ids: Vec<LogicalId>,
    /// Portable u32 index for each already emitted expression.
    indices: BTreeMap<LogicalId, u32>,
}

impl Terms {
    /// Add one ordered root by explicit postorder traversal.
    fn add(&mut self, cas: &Cas, root: LogicalId, budget: &mut Budget) -> Result<(), Fault> {
        let mut pending = vec![(root, false)];
        while let Some((id, expanded)) = pending.pop() {
            budget.work(
                size(self.ids.len(), AT)?
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
                AT,
            )?;
            if self.indices.contains_key(&id) {
                continue;
            }
            let term = cas.state.expression(id)?;
            if expanded {
                budget.check(
                    BudgetKind::Vertices,
                    size(self.ids.len(), AT)?
                        .checked_add(1)
                        .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, AT))?,
                    AT,
                )?;
                let index = u32::try_from(self.ids.len()).map_err(|_| invalid())?;
                self.ids.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
                self.ids.push(id);
                self.indices.insert(id, index);
            } else {
                pending
                    .try_reserve(
                        term.operands
                            .len()
                            .checked_add(1)
                            .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, AT))?,
                    )
                    .map_err(|_| Fault::allocation(AT))?;
                pending.push((id, true));
                pending.extend(term.operands.iter().rev().map(|child| (*child, false)));
            }
        }
        Ok(())
    }

    /// Resolve only an expression already included by the canonical root walk.
    fn index(&self, id: LogicalId) -> Result<u32, Fault> {
        self.indices.get(&id).copied().ok_or_else(invalid)
    }

    /// Encode the graph-owned mathematical records with strictly backward local references.
    fn encode(&self, cas: &Cas, out: &mut Encoder<'_>) -> Result<(), Fault> {
        out.count(self.ids.len())?;
        for id in &self.ids {
            let term = cas.state.expression(*id)?;
            encoding::operator(&term.operator, out)?;
            encoding::sort(&term.sort, out)?;
            encoding::scope(&term.scope, out)?;
            encoding::binding(term.binding.as_ref(), out)?;
            out.count(term.operands.len())?;
            for child in &term.operands {
                index(self.index(*child)?, out)?;
            }
        }
        Ok(())
    }
}

/// A structural codec failure carries no user-controlled diagnostic text.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Encoding, AT)
}

/// Encode one fixed-width u32 local reference.
pub(crate) fn index(value: u32, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.raw(&value.to_le_bytes())
}

/// Encode the exact admitted relation and ordered variable payload.
pub(crate) fn encode_relation(value: &Relation, out: &mut Encoder<'_>) -> Result<(), Fault> {
    match value {
        Relation::EqualOnDomain => out.byte(0),
        Relation::Derivative { variable } => {
            out.byte(1)?;
            out.name(variable)
        }
        Relation::SquareFree { variable, field } => {
            out.byte(3)?;
            out.name(variable)?;
            encoding::field(field, out)
        }
        Relation::Jacobian { variables } => {
            out.byte(2)?;
            out.count(variables.len())?;
            for variable in variables {
                out.name(variable)?;
            }
            Ok(())
        }
    }
}

/// Include context predicate roots before conclusion roots, as required by format one.
fn context_terms(cas: &Cas, context: LogicalId, budget: &mut Budget) -> Result<Terms, Fault> {
    let mut terms = Terms::default();
    for (_, id) in &cas.state.context(context)?.assumptions {
        let Vertex::Condition(condition) = cas.state.node(*id)? else {
            return Err(invalid());
        };
        for argument in &condition.arguments {
            terms.add(cas, *argument, budget)?;
        }
    }
    Ok(terms)
}

/// Encode declarations and assumption records before their cross-referenced term table.
fn encode_math(
    cas: &Cas,
    context: LogicalId,
    terms: &Terms,
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    let context = cas.state.context(context)?;
    encoding::environment(&context.environment, out)?;
    out.count(context.assumptions.len())?;
    for (key, id) in &context.assumptions {
        let Vertex::Condition(condition) = cas.state.node(*id)? else {
            return Err(invalid());
        };
        out.name(key)?;
        encoding::predicate(&condition.kind, out)?;
        encoding::scope(&condition.scope, out)?;
        out.count(condition.arguments.len())?;
        for argument in &condition.arguments {
            index(terms.index(*argument)?, out)?;
        }
    }
    terms.encode(cas, out)
}

/// Canonicalize residual root order independently of admission IDs and input vector order.
fn guard_order(
    cas: &Cas,
    guards: &[Guard],
    budget: &mut Budget,
) -> Result<Vec<(GuardKind, LogicalId)>, Fault> {
    budget.check(BudgetKind::ConditionAtoms, size(guards.len(), AT)?, AT)?;
    let mut ordered = BTreeMap::new();
    let mut total = 0_u64;
    for guard in guards {
        let id = cas.checked(&guard.term)?;
        let mut terms = Terms::default();
        terms.add(cas, id, budget)?;
        let mut out = Encoder::new(budget, AT);
        out.byte(guard_tag(guard.kind))?;
        terms.encode(cas, &mut out)?;
        let key = out.finish();
        total = total
            .checked_add(size(key.len(), AT)?)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, total, AT)?;
        budget.work(
            size(ordered.len(), AT)?
                .checked_add(1)
                .and_then(|n| n.checked_mul(size(key.len(), AT).ok()?))
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        if ordered.insert(key, (guard.kind, id)).is_some() {
            return Err(invalid());
        }
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(ordered.len())
        .map_err(|_| Fault::allocation(AT))?;
    result.extend(ordered.into_values());
    Ok(result)
}

/// Fixed domain-predicate tags, independent of enum layout.
const fn guard_tag(kind: GuardKind) -> u8 {
    match kind {
        GuardKind::Defined => 0,
        GuardKind::NonZero => 1,
    }
}

/// Write the final relation, endpoints and canonical residual local references.
fn encode_claim(
    cas: &Cas,
    claim: &Claim,
    guards: &[(GuardKind, LogicalId)],
    terms: &Terms,
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    encode_relation(&claim.relation, out)?;
    index(terms.index(cas.checked(&claim.left)?)?, out)?;
    index(terms.index(cas.checked(&claim.right)?)?, out)?;
    out.count(guards.len())?;
    for (kind, term) in guards {
        out.byte(guard_tag(*kind))?;
        index(terms.index(*term)?, out)?;
    }
    Ok(())
}

/// An expected claim cannot silently reinterpret a term under another semantic header.
fn validate_claim(cas: &Cas, claim: &Claim, budget: &mut Budget) -> Result<LogicalId, Fault> {
    let context = cas.checked(&claim.context)?;
    let header = &cas.state.context(context)?.header;
    for handle in [&claim.left, &claim.right]
        .into_iter()
        .chain(claim.residuals.iter().map(|guard| &guard.term))
    {
        let term = cas.state.expression(cas.checked(handle)?)?;
        budget.work(size(header.len(), AT)?, AT)?;
        if cas.state.context(term.context)?.header != *header {
            return Err(Fault::invalid(InvalidCode::Context, AT));
        }
    }
    let left = cas.state.expression(cas.checked(&claim.left)?)?;
    let right = cas.state.expression(cas.checked(&claim.right)?)?;
    if left.scope != right.scope
        || (!matches!(claim.relation, Relation::Jacobian { .. }) && left.sort != right.sort)
    {
        return Err(Fault::invalid(InvalidCode::Sort, AT));
    }
    Ok(context)
}

/// Exact expected-claim comparison includes full Gamma, relations and residuals.
pub(crate) fn claim_key(cas: &Cas, claim: &Claim, budget: &mut Budget) -> Result<Vec<u8>, Fault> {
    let context = validate_claim(cas, claim, budget)?;
    let mut terms = context_terms(cas, context, budget)?;
    terms.add(cas, cas.checked(&claim.left)?, budget)?;
    terms.add(cas, cas.checked(&claim.right)?, budget)?;
    let guards = guard_order(cas, &claim.residuals, budget)?;
    for (_, term) in &guards {
        terms.add(cas, *term, budget)?;
    }
    let mut out = Encoder::new(budget, AT);
    out.raw(b"NRZCLAIM01")?;
    encode_math(cas, context, &terms, &mut out)?;
    encode_claim(cas, claim, &guards, &terms, &mut out)?;
    Ok(out.finish())
}

/// Traverse proof premises in schema order, rejecting any unreachable record.
fn proof_order(compiled: &Compiled, budget: &mut Budget) -> Result<Vec<usize>, Fault> {
    let root = compiled.steps.len().checked_sub(1).ok_or_else(invalid)?;
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    let mut pending = vec![(root, false)];
    order
        .try_reserve_exact(compiled.steps.len())
        .map_err(|_| Fault::allocation(AT))?;
    while let Some((current, expanded)) = pending.pop() {
        budget.work(
            size(seen.len(), AT)?
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        if seen.contains(&current) {
            continue;
        }
        let premises = compiled.premises.get(current).ok_or_else(invalid)?;
        if expanded {
            seen.insert(current);
            order.push(current);
        } else {
            pending
                .try_reserve(premises.len().checked_add(1).ok_or_else(invalid)?)
                .map_err(|_| Fault::allocation(AT))?;
            pending.push((current, true));
            for previous in premises.iter().rev() {
                let previous = usize::try_from(*previous).map_err(|_| invalid())?;
                if previous >= current {
                    return Err(invalid());
                }
                pending.push((previous, false));
            }
        }
    }
    if seen.len() == compiled.steps.len() {
        Ok(order)
    } else {
        Err(invalid())
    }
}

/// Encode one application with canonical proof and term references.
fn encode_step(
    compiled: &Compiled,
    current: usize,
    terms: &Terms,
    proofs: &BTreeMap<usize, u32>,
    rules: &BTreeMap<Primitive, u32>,
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    let step = compiled.steps.get(current).ok_or_else(invalid)?;
    index(*rules.get(&step.primitive).ok_or_else(invalid)?, out)?;
    encode_relation(&step.relation, out)?;
    index(terms.index(step.left)?, out)?;
    index(terms.index(step.right)?, out)?;
    let premises = compiled.premises.get(current).ok_or_else(invalid)?;
    out.count(premises.len())?;
    for previous in premises {
        let previous = usize::try_from(*previous).map_err(|_| invalid())?;
        index(*proofs.get(&previous).ok_or_else(invalid)?, out)?;
    }
    out.count(step.path.len())?;
    for position in &step.path {
        index(*position, out)?;
    }
    out.count(step.substitutions.len())?;
    for (key, term) in &step.substitutions {
        out.name(key)?;
        index(terms.index(*term)?, out)?;
    }
    if let Some(value) = &step.certificate {
        value.encode(out)?;
    }
    Ok(())
}

/// Encode a canonical complete witness, rejecting duplicate proof records.
pub(crate) fn encode(
    cas: &Cas,
    compiled: &Compiled,
    budget: &mut Budget,
) -> Result<Vec<u8>, Fault> {
    let final_step = compiled.steps.last().ok_or_else(invalid)?;
    let claim = crate::proof::claim(cas, final_step, budget)?;
    let order = proof_order(compiled, budget)?;
    let mut terms = context_terms(cas, final_step.context, budget)?;
    terms.add(cas, final_step.left, budget)?;
    terms.add(cas, final_step.right, budget)?;
    let guards = guard_order(cas, &claim.residuals, budget)?;
    for (_, term) in &guards {
        terms.add(cas, *term, budget)?;
    }
    let mut manifest = BTreeMap::new();
    for current in &order {
        let step = compiled.steps.get(*current).ok_or_else(invalid)?;
        terms.add(cas, step.left, budget)?;
        terms.add(cas, step.right, budget)?;
        for (_, term) in &step.substitutions {
            terms.add(cas, *term, budget)?;
        }
        manifest.insert(step.primitive.specification().key, step.primitive);
    }
    let mut rules = BTreeMap::new();
    for (position, primitive) in manifest.values().enumerate() {
        rules.insert(*primitive, u32::try_from(position).map_err(|_| invalid())?);
    }
    let records = proof_records(compiled, &order, &terms, &rules, budget)?;
    let mut out = Encoder::witness(budget, AT);
    out.raw(b"NRZCAS01")?;
    out.count(manifest.len())?;
    for primitive in manifest.values() {
        let specification = primitive.specification();
        out.name(specification.key)?;
        index(specification.version, &mut out)?;
    }
    encode_math(cas, final_step.context, &terms, &mut out)?;
    out.byte(0)?;
    encode_claim(cas, &claim, &guards, &terms, &mut out)?;
    out.count(final_step.conditions.hypotheses.len())?;
    for key in &final_step.conditions.hypotheses {
        out.name(key)?;
    }
    out.count(records.len())?;
    for record in &records {
        out.raw(record)?;
    }
    index(
        u32::try_from(records.len().checked_sub(1).ok_or_else(invalid)?).map_err(|_| invalid())?,
        &mut out,
    )?;
    Ok(out.finish())
}

/// Pre-encode proof records with measured storage and unique canonical keys.
fn proof_records(
    compiled: &Compiled,
    order: &[usize],
    terms: &Terms,
    rules: &BTreeMap<Primitive, u32>,
    budget: &mut Budget,
) -> Result<Vec<Vec<u8>>, Fault> {
    let mut proofs = BTreeMap::new();
    let mut unique = BTreeSet::new();
    let mut records = Vec::new();
    let mut total = 0_u64;
    records
        .try_reserve_exact(order.len())
        .map_err(|_| Fault::allocation(AT))?;
    for current in order {
        let mut out = Encoder::witness(budget, AT);
        encode_step(compiled, *current, terms, &proofs, rules, &mut out)?;
        let record = out.finish();
        total = total
            .checked_add(size(record.len(), AT)?)
            .ok_or_else(|| Fault::capacity(BudgetKind::WitnessBytes, AT))?;
        budget.check(BudgetKind::WitnessBytes, total, AT)?;
        budget.work(
            size(unique.len(), AT)?
                .checked_add(1)
                .and_then(|n| n.checked_mul(size(record.len(), AT).ok()?))
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        if !unique.insert(record.clone()) {
            return Err(invalid());
        }
        proofs.insert(
            *current,
            u32::try_from(records.len()).map_err(|_| invalid())?,
        );
        records.push(record);
    }
    Ok(records)
}
