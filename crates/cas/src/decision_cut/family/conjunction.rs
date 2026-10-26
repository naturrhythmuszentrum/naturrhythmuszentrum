// SPDX-License-Identifier: AGPL-3.0-only
//! Complete affine conjunction identities for floor relation and projected range.

use std::collections::BTreeMap;

use super::{AT, invalid};
use crate::budget::{Budget, BudgetKind};
use crate::decision_cut::{affine, evaluate::Value, number_bytes, plan};
use crate::domain_model::RegionExpr;
use crate::error::Fault;
use crate::graph::{Cas, LogicalId, Vertex, size};
use crate::model::PredicateKind;
use crate::number::Rational;

/// One strict or non-strict affine comparison against zero.
pub(super) struct Atom {
    /// Positive is strict; nonnegative owns the lower boundary.
    pub(super) strict: bool,
    /// Constant followed by q and n coefficients.
    pub(super) coefficients: Vec<Rational>,
}

/// Three-valued cut results never enter the universal family proof.
type Values = BTreeMap<LogicalId, Value>;

/// Expand the exact lower/upper constructors into the complete required conjunction.
pub(super) fn expected(
    lower: Option<&Rational>,
    upper: Option<&Rational>,
    relation: bool,
    budget: &mut Budget,
) -> Result<Vec<Atom>, Fault> {
    let zero = Rational::zero();
    let one = Rational::one();
    let minus = Rational::from_ratio(-1, 1, budget)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(4)
        .map_err(|_| Fault::allocation(AT))?;
    if relation {
        result.push(Atom {
            strict: false,
            coefficients: vector([zero.clone(), one.clone(), minus.clone()])?,
        });
        result.push(Atom {
            strict: true,
            coefficients: vector([one.clone(), minus, one.clone()])?,
        });
    }
    let (q, n) = if relation {
        (zero.clone(), one)
    } else {
        (one, zero.clone())
    };
    if let Some(lower) = lower {
        result.push(Atom {
            strict: false,
            coefficients: vector([zero.sub(lower, budget)?, q.clone(), n.clone()])?,
        });
    }
    if let Some(upper) = upper {
        result.push(Atom {
            strict: true,
            coefficients: vector([
                upper.add(&zero, budget)?,
                zero.sub(&q, budget)?,
                zero.sub(&n, budget)?,
            ])?,
        });
    }
    Ok(result)
}

/// Fixed literal coefficients use the same bounded allocation policy as parameter vectors.
fn vector(values: [Rational; 3]) -> Result<Vec<Rational>, Fault> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(3)
        .map_err(|_| Fault::allocation(AT))?;
    result.extend(values);
    Ok(result)
}

/// Every required atom is present, and no extra restriction silently narrows the relation.
pub(super) fn check(
    cas: &Cas,
    root: LogicalId,
    expected: &[Atom],
    values: &Values,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let order = plan::order(cas, &[root], budget)?;
    let bytes = size(order.len(), AT)?.saturating_mul(256);
    budget.with_retained_payload(bytes, AT, |budget| {
        let mut masks = BTreeMap::new();
        for id in &order {
            budget.work(size(masks.len(), AT)?.saturating_add(1), AT)?;
            if let Vertex::Region(region) = cas.state.node(*id)? {
                let mask = region_mask(cas, &region.expression, &masks, expected, values, budget)?;
                masks.insert(*id, mask);
            }
        }
        let count = u32::try_from(expected.len()).map_err(|_| invalid())?;
        let complete = 1_u8
            .checked_shl(count)
            .ok_or_else(invalid)?
            .saturating_sub(1);
        if read(&masks, root)? != complete {
            return Err(invalid());
        }
        Ok(())
    })
}

/// Missing or non-region dependencies never stand for the ambient set.
fn read(masks: &BTreeMap<LogicalId, u8>, id: LogicalId) -> Result<u8, Fault> {
    masks.get(&id).copied().ok_or_else(invalid)
}

/// Only conjunction constructors preserve this exact universal normal form.
fn region_mask(
    cas: &Cas,
    region: &RegionExpr<LogicalId>,
    masks: &BTreeMap<LogicalId, u8>,
    expected: &[Atom],
    values: &Values,
    budget: &mut Budget,
) -> Result<u8, Fault> {
    match region {
        RegionExpr::Ambient => Ok(0),
        RegionExpr::GuardTrue { base, predicate } => {
            Ok(read(masks, *base)? | predicate_mask(cas, *predicate, expected, values, budget)?)
        }
        RegionExpr::Meet(members) => {
            let mut mask = 0;
            for member in members {
                budget.work(1, AT)?;
                mask |= read(masks, *member)?;
            }
            Ok(mask)
        }
        _ => Err(crate::decision_cut::unsupported()),
    }
}

/// `IntegerRange` expands to its inclusive lower and exclusive upper comparisons.
fn predicate_mask(
    cas: &Cas,
    id: LogicalId,
    expected: &[Atom],
    values: &Values,
    budget: &mut Budget,
) -> Result<u8, Fault> {
    let Vertex::Condition(predicate) = cas.state.node(id)? else {
        return Err(invalid());
    };
    let arguments = &predicate.arguments;
    let subject = affine::get(values, *arguments.first().ok_or_else(invalid)?)?;
    match predicate.kind {
        PredicateKind::NonNegative => matches(expected, subject, false, budget),
        PredicateKind::Positive => matches(expected, subject, true, budget),
        PredicateKind::IntegerRange => {
            let lower = affine::get(values, *arguments.get(1).ok_or_else(invalid)?)?;
            let upper = affine::get(values, *arguments.get(2).ok_or_else(invalid)?)?;
            let difference = affine::combine(subject, lower, true, budget)?;
            let low = matches(expected, &difference, false, budget)?;
            drop(difference);
            let difference = affine::combine(upper, subject, true, budget)?;
            Ok(low | matches(expected, &difference, true, budget)?)
        }
        _ => Err(crate::decision_cut::unsupported()),
    }
}

/// Exact coefficient identity includes strictness and prepaid arbitrary-precision comparisons.
fn matches(
    expected: &[Atom],
    coefficients: &[Rational],
    strict: bool,
    budget: &mut Budget,
) -> Result<u8, Fault> {
    budget.check(BudgetKind::ConditionAtoms, size(expected.len(), AT)?, AT)?;
    for (index, atom) in expected.iter().enumerate() {
        budget.work(1, AT)?;
        let mut equal = atom.strict == strict && coefficients.len() == atom.coefficients.len();
        for (left, right) in coefficients.iter().zip(&atom.coefficients) {
            budget.work(number_bytes(left).saturating_add(number_bytes(right)), AT)?;
            equal &= left == right;
        }
        if equal {
            return 1_u8
                .checked_shl(u32::try_from(index).map_err(|_| invalid())?)
                .ok_or_else(invalid);
        }
    }
    Err(invalid())
}
