// SPDX-License-Identifier: AGPL-3.0-only
//! Independently derived domain obligations and explicit hypothesis provenance.

use std::collections::{BTreeMap, BTreeSet};

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{LogicalId, State, Vertex, size};
use crate::model::{Operator, PredicateKind};
use crate::number::Rational;

/// The domain atoms required by the executable identity vocabulary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GuardKind {
    /// The exact original subexpression has a value.
    Defined,
    /// Its value exists and differs from zero.
    NonZero,
}

impl GuardKind {
    /// Map to ordinary condition syntax without claiming truth.
    pub(crate) const fn predicate(self) -> PredicateKind {
        match self {
            Self::Defined => PredicateKind::Defined,
            Self::NonZero => PredicateKind::NonZero,
        }
    }
}

/// One graph-scoped condition reconstructed by the kernel.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct Obligation {
    /// Exact predicate semantics.
    pub(crate) kind: GuardKind,
    /// Expression, including its checked scope signature.
    pub(crate) term: LogicalId,
}

/// Mathematical status; assumptions never masquerade as unconditional facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionStatus {
    /// A closed mathematical fact was checked.
    Proven,
    /// Explicit context hypotheses were used.
    Assumed,
    /// A false condition was established.
    Refuted,
    /// The bounded checker cannot discharge it.
    Unknown,
}

/// Evidence retained by a condition derivation.
#[derive(Clone, Debug, Default)]
struct Evidence {
    /// None means unknown; false is a refutation.
    truth: Option<bool>,
    /// Explicit context keys used, in canonical order.
    hypotheses: BTreeSet<String>,
}

/// A proof's reconstructed hypothesis class.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Conditions {
    /// Every primitive requirement, including discharged ones, for checked substitution.
    pub(crate) requirements: BTreeSet<Obligation>,
    /// Visible undischarged domain predicates.
    pub(crate) residuals: BTreeSet<Obligation>,
    /// Context assumptions used by successful derivations.
    pub(crate) hypotheses: BTreeSet<String>,
}

/// Location shared by condition derivation failures.
const AT: Location = Location::new("proof conditions");

/// Bound set operations before allocating or comparing keys.
fn charge(count: usize, budget: &mut Budget) -> Result<(), Fault> {
    let count = size(count, AT)?
        .checked_add(1)
        .ok_or_else(|| Fault::capacity(BudgetKind::ConditionAtoms, AT))?;
    budget.check(BudgetKind::ConditionAtoms, count, AT)?;
    let work = count
        .checked_mul(
            budget
                .limits()
                .name_bytes
                .checked_add(32)
                .ok_or_else(|| Fault::capacity(BudgetKind::ConditionSteps, AT))?,
        )
        .ok_or_else(|| Fault::capacity(BudgetKind::ConditionSteps, AT))?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        work.checked_mul(8)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?,
        AT,
    )?;
    budget.condition(work, AT)
}

impl Conditions {
    /// Preserve every premise's conditions during proof composition.
    pub(crate) fn extend(&mut self, other: &Self, budget: &mut Budget) -> Result<(), Fault> {
        for requirement in &other.requirements {
            charge(self.requirements.len(), budget)?;
            self.requirements.insert(*requirement);
        }
        for residual in &other.residuals {
            charge(self.residuals.len(), budget)?;
            self.residuals.insert(*residual);
        }
        for key in &other.hypotheses {
            charge(self.hypotheses.len(), budget)?;
            self.hypotheses.insert(key.clone());
        }
        Ok(())
    }

    /// Derive the complete domain of one endpoint, retaining opaque scoped predicates.
    pub(crate) fn endpoint(
        &mut self,
        state: &State,
        context: LogicalId,
        root: LogicalId,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let mut visited = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            budget.condition(
                size(visited.len(), AT)?
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::ConditionSteps, AT))?,
                AT,
            )?;
            if !visited.insert(id) {
                continue;
            }
            let term = state.expression(id)?;
            pending
                .try_reserve(term.operands.len())
                .map_err(|_| Fault::allocation(AT))?;
            pending.extend(term.operands.iter().rev().copied());
            let required = match &term.operator {
                Operator::Rational(value) => {
                    value.check(budget, AT)?;
                    None
                }
                Operator::IntegerPower(exponent) if *exponent <= 0 => Some(Obligation {
                    kind: GuardKind::NonZero,
                    term: *term
                        .operands
                        .first()
                        .ok_or_else(|| Fault::invalid(InvalidCode::Rule, AT))?,
                }),
                Operator::Free(_)
                | Operator::Bound { .. }
                | Operator::Constant(_)
                | Operator::Neg
                | Operator::Add
                | Operator::Sub
                | Operator::Mul
                | Operator::Matrix { .. }
                | Operator::IntegerPower(_) => None,
                Operator::Div => Some(Obligation {
                    kind: GuardKind::NonZero,
                    term: *term
                        .operands
                        .get(1)
                        .ok_or_else(|| Fault::invalid(InvalidCode::Rule, AT))?,
                }),
                _ => Some(Obligation {
                    kind: GuardKind::Defined,
                    term: id,
                }),
            };
            if let Some(required) = required {
                self.require(state, context, required, budget)?;
            }
        }
        Ok(())
    }

    /// Turn only an unknown fact into a residual; a refutation blocks the proof.
    pub(crate) fn require(
        &mut self,
        state: &State,
        context: LogicalId,
        obligation: Obligation,
        budget: &mut Budget,
    ) -> Result<ConditionStatus, Fault> {
        let mut pending = vec![obligation];
        let mut visited = BTreeSet::new();
        let mut status = ConditionStatus::Proven;
        while let Some(required) = pending.pop() {
            charge(visited.len(), budget)?;
            if !visited.insert(required) {
                continue;
            }
            charge(self.requirements.len(), budget)?;
            self.requirements.insert(required);
            let evidence = if required.kind == GuardKind::NonZero {
                nonzero(state, context, required.term, budget)?
            } else {
                assumption(state, context, required, budget)?
            };
            match evidence.truth {
                Some(false) => return Err(Fault::invalid(InvalidCode::RefutedDomain, AT)),
                None => {
                    if required.kind == GuardKind::NonZero {
                        let term = state.expression(required.term)?;
                        if matches!(
                            term.operator,
                            Operator::Neg
                                | Operator::Mul
                                | Operator::Div
                                | Operator::IntegerPower(_)
                        ) {
                            pending
                                .try_reserve(term.operands.len())
                                .map_err(|_| Fault::allocation(AT))?;
                            pending.extend(term.operands.iter().rev().map(|term| Obligation {
                                kind: GuardKind::NonZero,
                                term: *term,
                            }));
                            continue;
                        }
                    }
                    charge(self.residuals.len(), budget)?;
                    self.residuals.insert(required);
                    status = ConditionStatus::Unknown;
                }
                Some(true) => {
                    if !evidence.hypotheses.is_empty() && status != ConditionStatus::Unknown {
                        status = ConditionStatus::Assumed;
                    }
                    for key in evidence.hypotheses {
                        charge(self.hypotheses.len(), budget)?;
                        self.hypotheses.insert(key);
                    }
                }
            }
        }
        Ok(status)
    }
}

/// Find exact assumptions, conjunction projections and positivity implications.
fn assumption(
    state: &State,
    context: LogicalId,
    obligation: Obligation,
    budget: &mut Budget,
) -> Result<Evidence, Fault> {
    for (key, id) in &state.context(context)?.assumptions {
        let Vertex::Condition(condition) = state.node(*id)? else {
            return Err(Fault::invalid(InvalidCode::Reference, AT));
        };
        let mut pending = vec![(&condition.kind, condition.arguments.as_slice(), false)];
        while let Some((kind, arguments, negative)) = pending.pop() {
            budget.condition(1, AT)?;
            if *kind == PredicateKind::And && !negative {
                pending
                    .try_reserve(arguments.len())
                    .map_err(|_| Fault::allocation(AT))?;
                for argument in arguments.iter().rev() {
                    let term = state.expression(*argument)?;
                    if let Operator::Predicate(kind) = &term.operator {
                        pending.push((kind, &term.operands, false));
                    }
                }
            } else if *kind == PredicateKind::Not {
                if let Some(id) = arguments.first() {
                    let term = state.expression(*id)?;
                    if let Operator::Predicate(kind) = &term.operator {
                        pending.push((kind, &term.operands, !negative));
                    }
                }
            } else if arguments == [obligation.term]
                && (*kind == obligation.kind.predicate()
                    || (!negative
                        && obligation.kind == GuardKind::NonZero
                        && *kind == PredicateKind::Positive))
            {
                charge(0, budget)?;
                return Ok(Evidence {
                    truth: Some(!negative),
                    hypotheses: BTreeSet::from([key.clone()]),
                });
            }
        }
    }
    Ok(Evidence::default())
}

/// Reconstruct nonzero facts on an explicit postorder stack.
fn nonzero(
    state: &State,
    context: LogicalId,
    root: LogicalId,
    budget: &mut Budget,
) -> Result<Evidence, Fault> {
    let mut facts = BTreeMap::new();
    let mut numbers = BTreeMap::new();
    let mut retained = 0_u64;
    let mut pending = vec![(root, false)];
    while let Some((id, expanded)) = pending.pop() {
        budget.condition(
            size(facts.len(), AT)?
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::ConditionSteps, AT))?,
            AT,
        )?;
        if facts.contains_key(&id) {
            continue;
        }
        let term = state.expression(id)?;
        if !expanded {
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
            continue;
        }
        let numerical = numerical_value(&term.operator, &term.operands, &numbers, budget)?;
        let intrinsic = if let Some(value) = numerical {
            let evidence = Evidence {
                truth: Some(!value.is_zero()),
                hypotheses: BTreeSet::new(),
            };
            let (numerator, denominator) = value.component_bits();
            retained = retained
                .checked_add(numerator)
                .and_then(|n| n.checked_add(denominator))
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
            budget.check(BudgetKind::StagedPayloadBytes, retained, AT)?;
            numbers.insert(id, value);
            evidence
        } else {
            intrinsic_nonzero(&term.operator, &term.operands, &facts, budget)?
        };
        let result = if intrinsic.truth.is_some() {
            intrinsic
        } else {
            assumption(
                state,
                context,
                Obligation {
                    kind: GuardKind::NonZero,
                    term: id,
                },
                budget,
            )?
        };
        let bytes = size(result.hypotheses.len(), AT)?
            .checked_mul(budget.limits().name_bytes)
            .and_then(|n| n.checked_add(256))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        retained = retained
            .checked_add(bytes)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, retained, AT)?;
        budget.work(bytes, AT)?;
        facts.insert(id, result);
    }
    facts
        .remove(&root)
        .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))
}

/// Recompute a fully rational subexpression; symbolic constants and variables remain opaque.
fn numerical_value(
    operator: &Operator,
    operands: &[LogicalId],
    numbers: &BTreeMap<LogicalId, Rational>,
    budget: &mut Budget,
) -> Result<Option<Rational>, Fault> {
    if let Operator::Rational(value) = operator {
        value.check(budget, AT)?;
        let (numerator, denominator) = value.component_bits();
        budget.work(
            numerator
                .checked_add(denominator)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        return Ok(Some(value.clone()));
    }
    let Some(first) = operands.first().and_then(|id| numbers.get(id)) else {
        return Ok(None);
    };
    let result = match operator {
        Operator::Neg => first.negated(budget),
        Operator::IntegerPower(exponent) => first.pow(*exponent, budget),
        Operator::Add | Operator::Sub | Operator::Mul | Operator::Div => {
            let Some(second) = operands.get(1).and_then(|id| numbers.get(id)) else {
                return Ok(None);
            };
            match operator {
                Operator::Add => first.add(second, budget),
                Operator::Sub => first.sub(second, budget),
                Operator::Mul => first.mul(second, budget),
                Operator::Div => first.div(second, budget),
                _ => return Ok(None),
            }
        }
        _ => return Ok(None),
    };
    result.map(Some).map_err(|fault| {
        if matches!(
            fault.kind,
            crate::error::FaultKind::Invalid(
                InvalidCode::ZeroDenominator | InvalidCode::UndefinedPower
            )
        ) {
            Fault::invalid(InvalidCode::RefutedDomain, AT)
        } else {
            fault
        }
    })
}

/// Closed nonzero rules; unknown operations remain unknown.
fn intrinsic_nonzero(
    operator: &Operator,
    operands: &[LogicalId],
    facts: &BTreeMap<LogicalId, Evidence>,
    budget: &mut Budget,
) -> Result<Evidence, Fault> {
    let child = |position: usize| {
        operands
            .get(position)
            .and_then(|id| facts.get(id))
            .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))
    };
    let mut result = Evidence::default();
    match operator {
        Operator::Rational(value) => {
            value.check(budget, AT)?;
            result.truth = Some(!value.is_zero());
        }
        Operator::Constant(_) => result.truth = Some(true),
        Operator::Neg | Operator::IntegerPower(_) => {
            let base = child(0)?;
            if base.truth == Some(true) {
                result = base.clone();
            } else if matches!(operator, Operator::Neg | Operator::IntegerPower(1..)) {
                result.truth = base.truth;
            }
        }
        Operator::Mul | Operator::Div => {
            let a = child(0)?;
            let b = child(1)?;
            if a.truth == Some(true) && b.truth == Some(true) {
                result.truth = Some(true);
                for key in a.hypotheses.iter().chain(&b.hypotheses) {
                    charge(result.hypotheses.len(), budget)?;
                    result.hypotheses.insert(key.clone());
                }
            } else if a.truth == Some(false)
                || (matches!(operator, Operator::Mul) && b.truth == Some(false))
            {
                result.truth = Some(false);
            }
        }
        _ => {}
    }
    Ok(result)
}
