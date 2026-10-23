// SPDX-License-Identifier: AGPL-3.0-only
//! Checked empty zero sets exclude defined scalar values on precisely their original base.

mod storage;

use super::evaluate::Evaluation;
use super::proof::AT;
use super::value::Value;
use crate::budget::Budget;
use crate::domain_model::RegionExpr;
use crate::error::Fault;
use crate::graph::{Cas, Handle, LogicalId};
use crate::model::{Operator, PredicateKind};
use crate::proof_model::Statement;
use crate::solving::coefficients::storage::{reserve, vector};

/// Anonymous checked scalar subjects retained only within one complete application frame.
pub(crate) struct Lemmas {
    /// Exact independently interpreted subjects, shared by every divisor occurrence.
    pub(crate) values: Vec<Value>,
    /// Conservative owned scratch payload reserved throughout subsequent checking.
    pub(crate) bytes: u64,
}

impl Lemmas {
    /// Retain a compiled subject only after its complete nested payload fits the active allowance.
    fn push(&mut self, value: Value, budget: &mut Budget) -> Result<(), Fault> {
        let bytes = self.bytes.saturating_add(storage::value(&value, budget)?);
        budget.check(crate::budget::BudgetKind::StagedPayloadBytes, bytes, AT)?;
        reserve(&mut self.values, 1, budget)?;
        self.values.push(value);
        self.bytes = bytes;
        Ok(())
    }
}

/// Only an exact zero guard over this base can supply a nonzero theorem here.
pub(crate) fn subject(
    cas: &Cas,
    base: &Handle,
    statement: &Statement,
) -> Result<Option<LogicalId>, Fault> {
    let Statement::Empty { region } = statement else {
        return Ok(None);
    };
    let RegionExpr::GuardTrue {
        base: ancestor,
        predicate,
    } = &cas.state.region(cas.checked(region)?)?.expression
    else {
        return Ok(None);
    };
    if *ancestor != cas.checked(base)? {
        return Ok(None);
    }
    let (PredicateKind::Equal, [left, right]) = super::evidence::predicate(cas, *predicate)? else {
        return Ok(None);
    };
    for (subject, zero) in [(*left, *right), (*right, *left)] {
        if matches!(&cas.state.expression(zero)?.operator, Operator::Rational(value) if value.is_zero())
        {
            return Ok(Some(subject));
        }
    }
    Ok(None)
}

/// Compile anonymous zero-set sign leaves once from the existing checked premise frame.
pub(crate) fn values(
    evaluation: &Evaluation<'_>,
    slots: &[usize],
    budget: &mut Budget,
) -> Result<Lemmas, Fault> {
    #[cfg(test)]
    let _attribution = crate::testing::work_trace::Scope::phase("exclusion.sign-leaves");
    let mut result = Lemmas {
        values: vector(0, budget)?,
        bytes: 0,
    };
    if slots.is_empty() {
        return Ok(result);
    }
    guards(evaluation, &mut result, budget)?;
    for slot in slots {
        budget.work(1, AT)?;
        let premise = evaluation
            .premises
            .get(*slot)
            .ok_or_else(super::proof::invalid)?;
        if let Statement::PredicateFact { predicate, region } = &premise.judgment.statement
            && region == evaluation.region
            && let (PredicateKind::Positive | PredicateKind::NonZero, [subject]) =
                super::evidence::predicate(evaluation.cas, predicate.logical_id())?
        {
            let fact = crate::proof::EmptyFact::Premise(
                u32::try_from(*slot).map_err(|_| super::proof::invalid())?,
            );
            sign(evaluation, *subject, &fact, &mut result, budget)?;
        } else if let Some(subject) = subject(
            evaluation.cas,
            evaluation.region,
            &premise.judgment.statement,
        )? {
            let value = budget.with_retained_payload(result.bytes, AT, |budget| {
                evaluation.unbound().expression(subject, true, budget)
            })?;
            result.push(value, budget)?;
        }
    }
    Ok(result)
}

/// The shared sign-leaf checker validates the original strict fact before interpreting its subject.
fn sign(
    evaluation: &Evaluation<'_>,
    subject: LogicalId,
    fact: &crate::proof::EmptyFact,
    result: &mut Lemmas,
    budget: &mut Budget,
) -> Result<(), Fault> {
    crate::emptiness::nonzero(
        evaluation.cas,
        evaluation.region,
        &evaluation.cas.handle(subject),
        fact,
        evaluation.premises,
        budget,
    )?;
    let value = budget.with_retained_payload(result.bytes, AT, |budget| {
        evaluation.unbound().expression(subject, false, budget)
    })?;
    result.push(value, budget)?;
    Ok(())
}

/// Literal strict ancestor guards give the same anonymous sign leaves as checked premises.
fn guards(
    evaluation: &Evaluation<'_>,
    result: &mut Lemmas,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut pending = vector(1, budget)?;
    pending.push(evaluation.region.logical_id());
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        match &evaluation.cas.state.region(id)?.expression {
            RegionExpr::GuardTrue { base, predicate } => {
                if let (PredicateKind::Positive | PredicateKind::NonZero, [subject]) =
                    super::evidence::predicate(evaluation.cas, *predicate)?
                {
                    sign(
                        evaluation,
                        *subject,
                        &crate::proof::EmptyFact::Guard(evaluation.cas.handle(id)),
                        result,
                        budget,
                    )?;
                }
                pending.push(*base);
            }
            RegionExpr::GuardFalse { base, .. } => pending.push(*base),
            RegionExpr::Meet(members) => {
                reserve(&mut pending, members.len(), budget)?;
                pending.extend(members);
            }
            _ => {}
        }
    }
    Ok(())
}

/// A previously checked defined value excludes an exactly equal query without regional search.
pub(crate) fn matches(value: &Value, facts: &[Value], budget: &mut Budget) -> Result<bool, Fault> {
    for fact in facts {
        budget.work(1, AT)?;
        if same_form(&value.numerator, &fact.numerator, budget)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The independent oracle retains the complete previous exclusion algorithm in tests.
#[cfg(test)]
pub(crate) fn enabled() -> bool {
    !crate::testing::exclusion_oracle::active()
}

/// Production always consumes the independently checked lemma route when available.
#[cfg(not(test))]
pub(crate) const fn enabled() -> bool {
    true
}

/// Identical canonical coefficients suffice; unequal representations retain the full oracle.
fn same_coefficient(
    left: &crate::solving::coefficients::model::Element,
    right: &crate::solving::coefficients::model::Element,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    use crate::solving::coefficients::model::Value as Coefficient;
    budget.work(1, AT)?;
    if left.domain != right.domain {
        return Ok(false);
    }
    match (&left.value, &right.value) {
        (Coefficient::Scalar(a), Coefficient::Scalar(b)) => a.equals(b, budget),
        (Coefficient::Fraction(a), Coefficient::Fraction(b)) => {
            Ok(a.numerator.equals(&b.numerator, budget)?
                && a.denominator.equals(&b.denominator, budget)?)
        }
        (Coefficient::Tower(a), Coefficient::Tower(b)) if a.len() == b.len() => {
            for (first, second) in a.iter().zip(b) {
                if !same_coefficient(first, second, budget)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Exact sparse equality avoids constructing a new subtraction or rerunning factor search.
pub(crate) fn same_form(
    left: &crate::solving::coefficients::forms::Form,
    right: &crate::solving::coefficients::forms::Form,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    if left.terms.len() != right.terms.len() {
        return Ok(false);
    }
    for (first, second) in left.terms.iter().zip(&right.terms) {
        budget.work(
            crate::graph::size(first.powers.len().max(second.powers.len()), AT)?.saturating_add(1),
            AT,
        )?;
        if first.powers != second.powers
            || !same_coefficient(&first.coefficient, &second.coefficient, budget)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// A nonzero monomial makes each of its formal factors nonzero on the same exact frame.
pub(crate) fn monomial<'a>(
    subject: &crate::solving::coefficients::forms::Form,
    facts: &(impl Iterator<Item = &'a Value> + Clone),
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let [term] = subject.terms.as_slice() else {
        return Ok(false);
    };
    if !super::evidence::ground_unit(&term.coefficient.value, budget)? {
        return Ok(false);
    }
    for (variable, _) in &term.powers {
        let mut found = false;
        for fact in (*facts).clone() {
            budget.work(1, AT)?;
            if let [factor] = fact.numerator.terms.as_slice() {
                budget.work(crate::graph::size(factor.powers.len(), AT)?, AT)?;
                found |= factor
                    .powers
                    .iter()
                    .any(|(candidate, exponent)| candidate == variable && *exponent > 0);
            }
            if found {
                break;
            }
        }
        if !found {
            return Ok(false);
        }
    }
    Ok(true)
}
