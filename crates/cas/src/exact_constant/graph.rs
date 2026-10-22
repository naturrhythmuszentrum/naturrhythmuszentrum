// SPDX-License-Identifier: AGPL-3.0-only
//! Checked closed CAS graphs own their syntax; canonical keys contain no runtime store identity.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use super::{ExactConstant, Value, sign};
use crate::budget::{Budget, BudgetKind};
use crate::domain_model::HeaderDraft;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Draft, Handle, LogicalId, State, size};
use crate::model::{AtomicSort, Environment, Field, Operator, Sort};

/// Shared accounting location for owned exact constants and their closed proof rules.
pub(crate) const AT: Location = Location::new("exact constant expression");

/// Immutable normalized syntax and privately reconstructed evidence.
#[derive(Debug)]
pub(crate) struct CheckedConstant {
    /// Sole owner of this constant's immutable normalized expression graph.
    pub(crate) cas: Cas,
    /// Normalized exact expression.
    pub(crate) root: Handle,
    /// Complete canonical graph bytes, excluding runtime identities.
    pub(crate) key: Vec<u8>,
    /// Checked finite-real sign, possibly unresolved.
    pub(crate) sign: sign::Sign,
}

/// A fresh declaration-free real header prevents assumptions from becoming constant facts.
fn empty(budget: &mut Budget) -> Result<(Cas, LogicalId), Fault> {
    let mut cas = Cas::new(budget.limits().clone())?;
    let handles = cas.admit(
        &[Draft::Header(HeaderDraft::Core(Environment::default()))],
        budget,
    )?;
    let header = cas.checked(
        handles
            .first()
            .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))?,
    )?;
    Ok((cas, header))
}

/// Copy and check all original operands before the CAS's finite coefficient normalization.
pub(crate) fn admit(
    source: &Cas,
    root: &Handle,
    budget: &mut Budget,
) -> Result<ExactConstant, Fault> {
    let id = source.checked(root)?;
    let (mut cas, header) = empty(budget)?;
    let original = copy(&source.state, id, &mut cas, header, budget)?;
    finish(cas, original, budget)
}

/// Compose exact graph operands under one shared construction and proof ledger.
pub(crate) fn construct(
    operator: Operator,
    operands: &[&ExactConstant],
    budget: &mut Budget,
) -> Result<ExactConstant, Fault> {
    let (mut cas, header) = empty(budget)?;
    let mut children = Vec::new();
    children
        .try_reserve_exact(operands.len())
        .map_err(|_| Fault::allocation(AT))?;
    for operand in operands {
        let id = match &operand.0 {
            Value::Rational(value) => cas.generated(
                header,
                Operator::Rational(value.clone()),
                real(),
                &[],
                budget,
            )?,
            Value::Expression(value) => copy(
                &value.cas.state,
                value.cas.checked(&value.root)?,
                &mut cas,
                header,
                budget,
            )?,
        };
        children.push(id);
    }
    let root = cas.generated(header, operator, real(), &children, budget)?;
    finish(cas, root, budget)
}

/// Normalize only after finiteness, independently check equality, then own just the exact result.
fn finish(mut cas: Cas, original: LogicalId, budget: &mut Budget) -> Result<ExactConstant, Fault> {
    sign::check(&cas.state, original, budget)?;
    let normalized = crate::polynomial::normalize(&mut cas, original, budget)?;
    let normalized = cancel_equal(&mut cas, normalized, budget)?;
    if !crate::polynomial::equal(&cas.state, original, normalized, true, budget)? {
        return Err(Fault::invalid(InvalidCode::EndpointMismatch, AT));
    }
    if let Operator::Rational(value) = &cas.state.expression(normalized)?.operator {
        return Ok(value.clone().into());
    }
    let sign = sign::check(&cas.state, normalized, budget)?;
    let (mut compact, header) = empty(budget)?;
    let result = copy(&cas.state, normalized, &mut compact, header, budget)?;
    let root = compact.handle(result);
    let key = compact.canonical_snapshot(budget)?;
    Ok(ExactConstant(Value::Expression(Rc::new(CheckedConstant {
        cas: compact,
        root,
        key,
        sign,
    }))))
}

/// Reduce an identical-operand quotient only after checking original finiteness and nonzero divisors.
/// The caller must independently verify equality with the original expression before publication.
fn cancel_equal(cas: &mut Cas, root: LogicalId, budget: &mut Budget) -> Result<LogicalId, Fault> {
    budget.work(1, AT)?;
    let term = cas.state.expression(root)?;
    if matches!(term.operator, Operator::Div)
        && let [numerator, denominator] = term.operands.as_slice()
        && numerator == denominator
    {
        return cas.generated(
            term.header,
            Operator::Rational(crate::Rational::one()),
            real(),
            &[],
            budget,
        );
    }
    Ok(root)
}

/// Copy only the reached scalar syntax; no caller declaration or hypothesis crosses ownership.
fn copy(
    state: &State,
    root: LogicalId,
    target: &mut Cas,
    header: LogicalId,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let mut mapped = BTreeMap::new();
    for id in order(state, root, budget)? {
        let term = state.expression(id)?;
        let children = term
            .operands
            .iter()
            .map(|child| {
                mapped
                    .get(child)
                    .copied()
                    .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let copied = target.generated(header, term.operator.clone(), real(), &children, budget)?;
        mapped.insert(id, copied);
    }
    mapped
        .remove(&root)
        .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))
}

/// Explicit postorder visits every original operand, bounded by reached graph shape and work.
pub(crate) fn order(
    state: &State,
    root: LogicalId,
    budget: &mut Budget,
) -> Result<Vec<LogicalId>, Fault> {
    crate::descriptor::measure(state, root, budget)?;
    let mut visited = BTreeSet::new();
    let mut pending = vec![(root, false)];
    let mut result = Vec::new();
    while let Some((id, expanded)) = pending.pop() {
        let count = size(
            visited
                .len()
                .saturating_add(pending.len())
                .saturating_add(1),
            AT,
        )?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            count.saturating_mul(256),
            AT,
        )?;
        budget.work(count.saturating_mul(64), AT)?;
        if visited.contains(&id) {
            continue;
        }
        let term = state.expression(id)?;
        grammar(term)?;
        if expanded {
            result.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
            result.push(id);
            visited.insert(id);
        } else {
            pending
                .try_reserve(term.operands.len().saturating_add(1))
                .map_err(|_| Fault::allocation(AT))?;
            pending.push((id, true));
            pending.extend(term.operands.iter().rev().map(|child| (*child, false)));
        }
    }
    Ok(result)
}

/// Reject non-real, open or attached syntax before any simplification can hide it.
fn grammar(term: &crate::graph::Expression) -> Result<(), Fault> {
    let scalar = term.sort == real() || term.sort == Sort::Value(AtomicSort::Integer);
    let closed = term.scope.is_empty() && term.binding.is_none() && term.space.is_none();
    let plain = term.attachment.is_none() && term.domain.is_none() && term.arms.is_empty();
    let supported = matches!(
        term.operator,
        Operator::Rational(_)
            | Operator::Constant(_)
            | Operator::Neg
            | Operator::Add
            | Operator::Sub
            | Operator::Mul
            | Operator::Div
            | Operator::IntegerPower(_)
    );
    if scalar && closed && plain && supported {
        Ok(())
    } else {
        Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT))
    }
}

/// Exact constants inhabit the real scalar sort.
const fn real() -> Sort {
    Sort::Value(AtomicSort::Scalar(Field::Real))
}

/// Emit the existing CAS draft vocabulary, retaining exact operators and local dependency order.
pub(crate) fn drafts(
    value: &ExactConstant,
    header: crate::graph::Reference,
    budget: &mut Budget,
) -> Result<Vec<crate::graph::ExpressionDraft>, Fault> {
    let mut result = Vec::new();
    let mut mapped = BTreeMap::new();
    if let Value::Rational(number) = &value.0 {
        number.check(budget, AT)?;
        budget.work(128, AT)?;
        return Ok(vec![draft(
            Operator::Rational(number.clone()),
            header,
            Vec::new(),
        )]);
    }
    let Value::Expression(graph) = &value.0 else {
        return Err(Fault::invalid(InvalidCode::Reference, AT));
    };
    for id in order(&graph.cas.state, graph.cas.checked(&graph.root)?, budget)? {
        let term = graph.cas.state.expression(id)?;
        let operands = term
            .operands
            .iter()
            .map(|child| {
                mapped
                    .get(child)
                    .copied()
                    .map(crate::graph::Reference::Local)
                    .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let index = crate::graph::position_u32(result.len())?;
        result.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        result.push(draft(term.operator.clone(), header.clone(), operands));
        mapped.insert(id, index);
    }
    Ok(result)
}

/// A constant draft has no binder, space, source assumption or free declaration.
const fn draft(
    operator: Operator,
    header: crate::graph::Reference,
    operands: Vec<crate::graph::Reference>,
) -> crate::graph::ExpressionDraft {
    crate::graph::ExpressionDraft {
        space: None,
        operator,
        sort: real(),
        header,
        scope: Vec::new(),
        binding: None,
        operands,
    }
}
