// SPDX-License-Identifier: AGPL-3.0-only
//! Syntactic hypothesis canonicalization without a general consistency claim.

use std::collections::{BTreeMap, BTreeSet};

use crate::budget::{Budget, BudgetKind};
use crate::encoding::{self, Encoder};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Condition, LogicalId, State, Vertex, encode_id, size};
use crate::model::{Operator, PredicateKind, Scope};

/// One borrowed predicate view; graph storage owns every referenced argument.
struct Predicate<'a> {
    /// Named predicate, absent for a declared Boolean expression.
    kind: Option<&'a PredicateKind>,
    /// Exact outer frames supplied by the context's admissible region.
    scope: &'a Scope,
    /// Predicate operands or one opaque Boolean identity.
    arguments: &'a [LogicalId],
}

/// Convert a condition record to its borrowed predicate view.
fn condition(value: &Condition) -> Predicate<'_> {
    Predicate {
        kind: Some(&value.kind),
        scope: &value.scope,
        arguments: &value.arguments,
    }
}

/// View Boolean syntax without turning a declaration into a proof.
fn expression<'a>(state: &'a State, id: &'a LogicalId) -> Result<Predicate<'a>, Fault> {
    let value = state.expression(*id)?;
    let (kind, arguments) = if let Operator::Predicate(kind) = &value.operator {
        (Some(kind), value.operands.as_slice())
    } else {
        (None, std::slice::from_ref(id))
    };
    Ok(Predicate {
        kind,
        scope: &value.scope,
        arguments,
    })
}

/// Encode an atom independently of whether it appeared as condition or Boolean syntax.
fn atom(value: &Predicate<'_>, budget: &mut Budget) -> Result<Vec<u8>, Fault> {
    let mut out = Encoder::new(budget, Location::new("hypothesis atom"));
    out.byte(u8::from(value.kind.is_some()))?;
    if let Some(kind) = value.kind {
        encoding::predicate(kind, &mut out)?;
    }
    encoding::scope(value.scope, &mut out)?;
    out.count(value.arguments.len())?;
    for argument in value.arguments {
        encode_id(*argument, &mut out)?;
    }
    Ok(out.finish())
}

/// Reject only recognized false positive atoms; all other consistency remains unknown.
fn literal_false(state: &State, value: &Predicate<'_>) -> Result<bool, Fault> {
    if value.kind == Some(&PredicateKind::Or) && value.arguments.is_empty() {
        return Ok(true);
    }
    let Some(argument) = value.arguments.first() else {
        return Ok(false);
    };
    let Operator::Rational(number) = &state.expression(*argument)?.operator else {
        return Ok(false);
    };
    Ok(match value.kind {
        Some(PredicateKind::NonZero) => number.is_zero(),
        Some(PredicateKind::Positive) => number.is_zero() || number.is_negative(),
        Some(PredicateKind::NonNegative) => number.is_negative(),
        _ => false,
    })
}

/// Flatten conjunctions and retain each explicit hypothesis key in the context's key.
pub(crate) fn canonical(
    state: &State,
    scope: &Scope,
    hypotheses: &[(String, LogicalId)],
    budget: &mut Budget,
) -> Result<Vec<u8>, Fault> {
    let at = Location::new("context hypotheses");
    let mut all = BTreeMap::new();
    let mut records = Vec::new();
    records
        .try_reserve_exact(hypotheses.len())
        .map_err(|_| Fault::allocation(at))?;
    let mut total_bytes = 0_u64;
    for (name, id) in hypotheses {
        let Vertex::Condition(value) = state.node(*id)? else {
            return Err(Fault::invalid(InvalidCode::Reference, at));
        };
        let mut pending = vec![condition(value)];
        let mut atoms = BTreeSet::new();
        while let Some(value) = pending.pop() {
            budget.condition(1, at)?;
            if value.scope != scope {
                return Err(Fault::invalid(InvalidCode::Scope, at));
            }
            if value.kind == Some(&PredicateKind::And) {
                pending
                    .try_reserve(value.arguments.len())
                    .map_err(|_| Fault::allocation(at))?;
                for argument in value.arguments.iter().rev() {
                    pending.push(expression(state, argument)?);
                }
                continue;
            }
            if literal_false(state, &value)? {
                return Err(Fault::invalid(InvalidCode::Context, at));
            }
            let negative = value.kind == Some(&PredicateKind::Not);
            let key = if negative {
                let argument = value
                    .arguments
                    .first()
                    .ok_or_else(|| Fault::invalid(InvalidCode::Sort, at))?;
                atom(&expression(state, argument)?, budget)?
            } else {
                atom(&value, budget)?
            };
            let bytes = size(key.len(), at)?;
            total_bytes = total_bytes
                .checked_add(bytes)
                .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, at))?;
            budget.check(BudgetKind::PayloadBytes, total_bytes, at)?;
            let work = size(all.len(), at)?
                .checked_add(1)
                .and_then(|count| count.checked_mul(bytes))
                .and_then(|amount| amount.checked_mul(8))
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?;
            budget.condition(work, at)?;
            if all.get(&key).is_some_and(|previous| *previous != negative) {
                return Err(Fault::invalid(InvalidCode::Context, at));
            }
            if !all.contains_key(&key) {
                let next = size(all.len(), at)?
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::ConditionAtoms, at))?;
                budget.check(BudgetKind::ConditionAtoms, next, at)?;
            }
            all.insert(key.clone(), negative);
            atoms.insert((key, negative));
        }
        records.push((name, atoms));
    }
    let mut out = Encoder::new(budget, at);
    out.count(records.len())?;
    for (name, atoms) in records {
        out.name(name)?;
        out.count(atoms.len())?;
        for (key, negative) in atoms {
            out.byte(u8::from(negative))?;
            out.count(key.len())?;
            out.raw(&key)?;
        }
    }
    Ok(out.finish())
}

/// Preserve the closed-context precondition at legacy proof, service and scalar-projection boundaries.
pub(crate) fn require_closed(state: &State, context: LogicalId) -> Result<(), Fault> {
    let frame = state.context(context)?;
    if state.region(frame.admissible)?.scope.is_empty() {
        return Ok(());
    }
    Err(Fault::invalid(
        InvalidCode::UnsupportedFragment,
        Location::new("closed context required"),
    ))
}
