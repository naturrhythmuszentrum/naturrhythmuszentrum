// SPDX-License-Identifier: AGPL-3.0-only
//! Whole-domain exact Integer bindings and nonempty count-range intersections.

use super::{AT, invalid};
use crate::budget::Budget;
use crate::decision::IntegerFamily;
use crate::error::Fault;
use crate::graph::{Cas, Handle, LogicalId, Vertex};
use crate::model::{AtomicSort, Operator, PredicateKind, Sort};
use crate::number::Rational;
use crate::proof_model::{
    ApplicationDraft, CheckedApplication, DomainContract, ScopedConditions, Statement,
};

/// Literal endpoints need no premise; each free endpoint consumes exactly one ordered binding.
pub(super) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    family: &IntegerFamily,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let mut cursor = 0;
    let mut conditions = ScopedConditions::default();
    let mut resolved = [None, None];
    for (position, handle) in [&family.lower, &family.upper].into_iter().enumerate() {
        let (number, used) = resolve(cas, draft, handle.as_ref(), premises.get(cursor), budget)?;
        resolved[position] = number;
        if used {
            let premise = premises.get(cursor).ok_or_else(invalid)?;
            crate::statement::merge(cas, &mut conditions, &premise.conditions, budget)?;
            cursor += 1;
        }
    }
    if cursor != premises.len() {
        return Err(invalid());
    }
    if family.lower.is_some() && family.lower == family.upper {
        return Err(invalid());
    }
    nonempty(family, resolved, budget)?;
    Ok(conditions)
}

/// Resolve only the identical endpoint expression under the entire source context.
fn resolve<'a>(
    cas: &'a Cas,
    draft: &ApplicationDraft,
    handle: Option<&Handle>,
    premise: Option<&CheckedApplication>,
    budget: &mut Budget,
) -> Result<(Option<&'a Rational>, bool), Fault> {
    let Some(handle) = handle else {
        return Ok((None, false));
    };
    budget.work(1, AT)?;
    let value = cas.state.expression(cas.checked(handle)?)?;
    let (id, used) = match value.operator {
        Operator::Rational(_) => (cas.checked(handle)?, false),
        Operator::Free(_) | Operator::Neg | Operator::Add | Operator::Sub | Operator::Mul => {
            let premise = premise.ok_or_else(invalid)?;
            if premise.judgment.context != draft.context || !premise.conditions.residuals.is_empty()
            {
                return Err(invalid());
            }
            (binding(cas, draft, handle, premise)?, true)
        }
        _ => return Err(crate::decision_cut::unsupported()),
    };
    let constant = cas.state.expression(id)?;
    if constant.header != value.header
        || constant.sort != Sort::Value(AtomicSort::Integer)
        || !constant.scope.is_empty()
        || constant.binding.is_some()
        || constant.space.is_some()
    {
        return Err(invalid());
    }
    let Operator::Rational(number) = &constant.operator else {
        return Err(invalid());
    };
    number.check(budget, AT)?;
    if !number.is_integer() {
        return Err(invalid());
    }
    Ok((Some(number), used))
}

/// Equality may be a typed value theorem or an exact equality predicate with retained provenance.
fn binding(
    cas: &Cas,
    draft: &ApplicationDraft,
    endpoint: &Handle,
    premise: &CheckedApplication,
) -> Result<LogicalId, Fault> {
    let domain = cas.state.context(cas.checked(&draft.context)?)?.admissible;
    match &premise.judgment.statement {
        Statement::Equality {
            left,
            right,
            domain: region,
            domain_contract: DomainContract::DefinedAndEqual,
        } if left == endpoint && cas.checked(region)? == domain => cas.checked(right),
        Statement::PredicateFact { region, predicate } if cas.checked(region)? == domain => {
            let Vertex::Condition(predicate) = cas.state.node(cas.checked(predicate)?)? else {
                return Err(invalid());
            };
            let [subject, target] = predicate.arguments.as_slice() else {
                return Err(invalid());
            };
            if predicate.kind != PredicateKind::Equal || *subject != cas.checked(endpoint)? {
                return Err(invalid());
            }
            Ok(*target)
        }
        _ => Err(invalid()),
    }
}

/// Every lower/upper pair is ordered, so the intersection contains at least one Integer.
fn nonempty(
    family: &IntegerFamily,
    resolved: [Option<&Rational>; 2],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let count = family.count.as_deref();
    let lower = [resolved[0], count.and_then(|range| range.lower.as_ref())];
    let upper = [resolved[1], count.and_then(|range| range.upper.as_ref())];
    for value in lower.into_iter().chain(upper).flatten() {
        value.check(budget, AT)?;
        if !value.is_integer() {
            return Err(invalid());
        }
    }
    for low in lower.into_iter().flatten() {
        for high in upper.into_iter().flatten() {
            if low.compare(high, budget)? != std::cmp::Ordering::Less {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

/// An exact selected Integer must belong to every declared range constraint.
pub(super) fn contains(
    cas: &Cas,
    draft: &ApplicationDraft,
    family: &IntegerFamily,
    premises: &[CheckedApplication],
    value: &Rational,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut cursor = 0;
    for (upper, handle) in [(false, &family.lower), (true, &family.upper)] {
        let (endpoint, used) = resolve(cas, draft, handle.as_ref(), premises.get(cursor), budget)?;
        cursor += usize::from(used);
        if let Some(endpoint) = endpoint {
            included(value, endpoint, upper, budget)?;
        }
    }
    if cursor != premises.len() {
        return Err(invalid());
    }
    if let Some(count) = &family.count {
        for (upper, endpoint) in [(false, &count.lower), (true, &count.upper)] {
            if let Some(endpoint) = endpoint {
                included(value, endpoint, upper, budget)?;
            }
        }
    }
    Ok(())
}

/// Range lower bounds are inclusive and upper bounds are exclusive, independently of capacity.
fn included(
    value: &Rational,
    endpoint: &Rational,
    upper: bool,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let order = value.compare(endpoint, budget)?;
    let below_upper = upper && order == std::cmp::Ordering::Less;
    let above_lower = !upper && order != std::cmp::Ordering::Less;
    if below_upper || above_lower {
        Ok(())
    } else {
        Err(invalid())
    }
}
