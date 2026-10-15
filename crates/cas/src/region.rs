// SPDX-License-Identifier: AGPL-3.0-only
//! Exact affine truth partitions; no numerical sampling establishes a region fact.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::{AmbientDomain, RegionExpr};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{LogicalId, State, Vertex, size};
use crate::model::{Operator, PredicateKind};
use crate::number::Rational;

/// Untrusted finite-cell evidence compared with an independently reconstructed exact partition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegionCertificate {
    /// Strictly increasing exact rational cuts.
    pub cuts: Vec<Rational>,
    /// Membership for each requested region in order, including rays and singletons.
    pub membership: Vec<Vec<bool>>,
}

/// Exact affine comparison against zero.
#[derive(Clone, Copy, Debug)]
enum Comparison {
    /// Equality to zero.
    Equal,
    /// Strict positivity.
    Positive,
    /// Positivity or equality to zero.
    NonNegative,
    /// Inequality to zero.
    NonZero,
}

/// A topological Boolean program whose arguments always precede their user.
#[derive(Clone, Debug)]
enum Formula {
    /// A literal total Boolean value.
    Constant(bool),
    /// A rational affine atom; None means a constant sign.
    Atom {
        /// Exact root; absent for a constant affine expression.
        cut: Option<Rational>,
        /// The slope is negative, or a constant expression is negative.
        negative: bool,
        /// A constant expression is zero.
        zero: bool,
        /// Relation to zero whose truth is being evaluated.
        comparison: Comparison,
    },
    /// Total conjunction of earlier values.
    And(Vec<usize>),
    /// Total disjunction of earlier values.
    Or(Vec<usize>),
    /// Total negation of one earlier value.
    Not(usize),
}

/// The separate truth and definedness of a predicate.
#[derive(Clone, Copy, Debug)]
struct Meaning {
    /// Program index of the predicate's truth value.
    truth: usize,
    /// Program index of the predicate's definedness.
    defined: usize,
}

/// Meaning requested from a graph dependency.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Mode {
    /// Compile exact region membership.
    Region,
    /// Compile a predicate's truth and definedness.
    Predicate,
    /// Compile an expression's strict definedness.
    Defined,
}

/// One exact partition compiler with an explicit, bounded work stack.
struct Compiler<'a> {
    /// Authoritative syntax only; no producer truth flags.
    state: &'a State,
    /// One declared coordinate.
    coordinate: &'a str,
    /// Topological Boolean records.
    formulas: Vec<Formula>,
    /// Previously compiled graph dependencies.
    meanings: BTreeMap<(Mode, LogicalId), Meaning>,
}

/// Region decision error location.
const AT: Location = Location::new("exact region partition");

/// A valid predicate outside the supported exact fragment remains unresolved.
const fn unknown() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedFragment, AT)
}

impl Compiler<'_> {
    /// Append a bounded Boolean program record before retaining its payload.
    fn push(&mut self, formula: Formula, budget: &mut Budget) -> Result<usize, Fault> {
        let formula = fold(formula, &self.formulas, budget)?;
        let count = size(self.formulas.len(), AT)?
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::ConditionAtoms, AT))?;
        budget.check(BudgetKind::ConditionAtoms, count, AT)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            count
                .checked_mul(256)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?,
            AT,
        )?;
        budget.condition(1, AT)?;
        self.formulas
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        let index = self.formulas.len();
        self.formulas.push(formula);
        Ok(index)
    }

    /// Read a dependency only after its postorder compilation has completed.
    fn get(&self, mode: Mode, id: LogicalId) -> Result<Meaning, Fault> {
        self.meanings.get(&(mode, id)).copied().ok_or_else(unknown)
    }

    /// Compile every reachable typed dependency using an explicit postorder stack.
    fn compile(&mut self, roots: &[LogicalId], budget: &mut Budget) -> Result<(), Fault> {
        let mut pending = Vec::new();
        pending
            .try_reserve(roots.len())
            .map_err(|_| Fault::allocation(AT))?;
        pending.extend(roots.iter().rev().map(|id| (Mode::Region, *id, 0_u64)));
        let mut failed = std::collections::BTreeSet::new();
        while let Some(current) = pending.pop() {
            self.compile_one(current, &mut pending, &mut failed, budget)?;
        }
        if roots
            .iter()
            .any(|id| !self.meanings.contains_key(&(Mode::Region, *id)))
        {
            return Err(unknown());
        }
        Ok(())
    }

    /// Resume one dependency, retaining unknown siblings until an empty base can decide it.
    fn compile_one(
        &mut self,
        (mode, id, depth): (Mode, LogicalId, u64),
        pending: &mut Vec<(Mode, LogicalId, u64)>,
        failed: &mut std::collections::BTreeSet<(Mode, LogicalId)>,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        budget.condition(1, AT)?;
        budget.check(BudgetKind::Depth, depth, AT)?;
        let live = self
            .meanings
            .len()
            .saturating_add(failed.len())
            .saturating_add(pending.len());
        budget.check(BudgetKind::ConditionAtoms, size(live, AT)?, AT)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            size(live, AT)?.saturating_mul(128),
            AT,
        )?;
        if self.meanings.contains_key(&(mode, id)) {
            return Ok(());
        }
        if let Some(meaning) = self.absent(mode, id)? {
            self.meanings.insert((mode, id), meaning);
            return Ok(());
        }
        let dependencies = dependencies(self.state, mode, id, budget)?;
        if let Some((kind, child)) = dependencies
            .into_iter()
            .find(|key| !self.meanings.contains_key(key) && !failed.contains(key))
        {
            let next = depth
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::Depth, AT))?;
            pending.try_reserve(2).map_err(|_| Fault::allocation(AT))?;
            pending.push((mode, id, depth));
            pending.push((kind, child, next));
        } else {
            match self.meaning(mode, id, budget) {
                Ok(meaning) => {
                    self.meanings.insert((mode, id), meaning);
                }
                Err(fault) if crate::case_obligations::unknown(&fault) => {
                    failed.insert((mode, id));
                }
                Err(fault) => return Err(fault),
            }
        }
        Ok(())
    }

    /// A proved empty base can decide membership without evaluating an unreachable predicate.
    fn absent(&self, mode: Mode, id: LogicalId) -> Result<Option<Meaning>, Fault> {
        if mode != Mode::Region {
            return Ok(None);
        }
        let candidates = match &self.state.region(id)?.expression {
            RegionExpr::GuardTrue { base, .. }
            | RegionExpr::GuardFalse { base, .. }
            | RegionExpr::Difference { base, .. } => std::slice::from_ref(base),
            RegionExpr::Meet(members) => members.as_slice(),
            _ => return Ok(None),
        };
        for child in candidates {
            if let Some(value) = self.meanings.get(&(Mode::Region, *child))
                && matches!(
                    self.formulas.get(value.truth),
                    Some(Formula::Constant(false))
                )
            {
                return Ok(Some(*value));
            }
        }
        Ok(None)
    }

    /// Reconstruct one region, predicate or strict definedness formula.
    fn meaning(
        &mut self,
        mode: Mode,
        id: LogicalId,
        budget: &mut Budget,
    ) -> Result<Meaning, Fault> {
        match mode {
            Mode::Region => self.region(id, budget),
            Mode::Predicate => self.predicate(id, budget),
            Mode::Defined => {
                let truth = self.defined(id, budget)?;
                Ok(Meaning { truth, defined: 0 })
            }
        }
    }

    /// Set membership keeps defined-and-true separate from defined-and-false.
    fn region(&mut self, id: LogicalId, budget: &mut Budget) -> Result<Meaning, Fault> {
        let expression = self.state.region(id)?.expression.clone();
        let formula = match expression {
            RegionExpr::Ambient => Formula::Constant(true),
            RegionExpr::Empty => Formula::Constant(false),
            RegionExpr::GuardTrue { base, predicate } => {
                let guard = self.get(Mode::Predicate, predicate)?;
                Formula::And(vec![
                    self.get(Mode::Region, base)?.truth,
                    guard.defined,
                    guard.truth,
                ])
            }
            RegionExpr::GuardFalse { base, predicate } => {
                let guard = self.get(Mode::Predicate, predicate)?;
                let negative = self.push(Formula::Not(guard.truth), budget)?;
                Formula::And(vec![
                    self.get(Mode::Region, base)?.truth,
                    guard.defined,
                    negative,
                ])
            }
            RegionExpr::Meet(members) => Formula::And(self.region_inputs(&members)?),
            RegionExpr::Union(members) => Formula::Or(self.region_inputs(&members)?),
            RegionExpr::Difference { base, excluded } => {
                let negative = self.push(
                    Formula::Not(self.get(Mode::Region, excluded)?.truth),
                    budget,
                )?;
                Formula::And(vec![self.get(Mode::Region, base)?.truth, negative])
            }
        };
        Ok(Meaning {
            truth: self.push(formula, budget)?,
            defined: 0,
        })
    }

    /// Copy already compiled membership references after input measurement.
    fn region_inputs(&self, members: &[LogicalId]) -> Result<Vec<usize>, Fault> {
        let mut result = Vec::new();
        result
            .try_reserve_exact(members.len())
            .map_err(|_| Fault::allocation(AT))?;
        for member in members {
            result.push(self.get(Mode::Region, *member)?.truth);
        }
        Ok(result)
    }

    /// Compile a strict Boolean predicate, preserving its own definedness.
    fn predicate(&mut self, id: LogicalId, budget: &mut Budget) -> Result<Meaning, Fault> {
        let (kind, arguments) = predicate(self.state, id)?;
        let kind = kind.clone();
        let arguments = arguments.to_vec();
        let mut defined = Vec::new();
        defined
            .try_reserve_exact(arguments.len())
            .map_err(|_| Fault::allocation(AT))?;
        let boolean = matches!(
            kind,
            PredicateKind::And | PredicateKind::Or | PredicateKind::Not
        );
        for argument in &arguments {
            defined.push(if boolean {
                self.get(Mode::Predicate, *argument)?.defined
            } else {
                self.get(Mode::Defined, *argument)?.truth
            });
        }
        if kind == PredicateKind::Defined {
            return Ok(Meaning {
                truth: self.push(Formula::And(defined), budget)?,
                defined: 0,
            });
        }
        if defined
            .iter()
            .any(|index| matches!(self.formulas.get(*index), Some(Formula::Constant(false))))
        {
            let absent = self.push(Formula::Constant(false), budget)?;
            return Ok(Meaning {
                truth: absent,
                defined: absent,
            });
        }
        let truth = self.predicate_truth(&kind, &arguments, budget)?;
        Ok(Meaning {
            truth,
            defined: self.push(Formula::And(defined), budget)?,
        })
    }

    /// Compile supported truth independently of its already collected strict requirements.
    fn predicate_truth(
        &mut self,
        kind: &PredicateKind,
        arguments: &[LogicalId],
        budget: &mut Budget,
    ) -> Result<usize, Fault> {
        match kind {
            PredicateKind::And | PredicateKind::Or => {
                let inputs = arguments
                    .iter()
                    .map(|id| self.get(Mode::Predicate, *id).map(|value| value.truth))
                    .collect::<Result<_, _>>()?;
                let formula = if *kind == PredicateKind::And {
                    Formula::And(inputs)
                } else {
                    Formula::Or(inputs)
                };
                self.push(formula, budget)
            }
            PredicateKind::Not => {
                let child = arguments.first().copied().ok_or_else(unknown)?;
                self.push(
                    Formula::Not(self.get(Mode::Predicate, child)?.truth),
                    budget,
                )
            }
            PredicateKind::IntegerRange => {
                let input = arguments.first().copied().ok_or_else(unknown)?;
                let lower = arguments.get(1).copied().ok_or_else(unknown)?;
                let upper = arguments.get(2).copied().ok_or_else(unknown)?;
                let left = self.atom(input, Some(lower), Comparison::NonNegative, budget)?;
                let high = self.atom(input, Some(upper), Comparison::NonNegative, budget)?;
                let right = self.push(Formula::Not(high), budget)?;
                self.push(Formula::And(vec![left, right]), budget)
            }
            PredicateKind::Equal
            | PredicateKind::Positive
            | PredicateKind::NonNegative
            | PredicateKind::NonZero => {
                let input = arguments.first().copied().ok_or_else(unknown)?;
                let other = if *kind == PredicateKind::Equal {
                    arguments.get(1).copied()
                } else {
                    None
                };
                let comparison = match kind {
                    PredicateKind::Equal => Comparison::Equal,
                    PredicateKind::Positive => Comparison::Positive,
                    PredicateKind::NonNegative => Comparison::NonNegative,
                    _ => Comparison::NonZero,
                };
                self.atom(input, other, comparison, budget)
            }
            _ => Err(unknown()),
        }
    }

    /// Derive strict value definedness independently of a simplified coefficient view.
    fn defined(&mut self, id: LogicalId, budget: &mut Budget) -> Result<usize, Fault> {
        let value = self.state.expression(id)?;
        let operator = value.operator.clone();
        let arguments = value.operands.clone();
        let mut required = Vec::new();
        required
            .try_reserve(
                arguments
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::ConditionAtoms, AT))?,
            )
            .map_err(|_| Fault::allocation(AT))?;
        for argument in &arguments {
            required.push(self.get(Mode::Defined, *argument)?.truth);
        }
        match operator {
            Operator::Rational(_)
            | Operator::Free(_)
            | Operator::Bound { .. }
            | Operator::Constant(_)
            | Operator::Neg
            | Operator::Add
            | Operator::Sub
            | Operator::Mul
            | Operator::IntegerEmbed => {}
            Operator::Div => {
                let denominator = arguments.get(1).copied().ok_or_else(unknown)?;
                required.push(self.atom(denominator, None, Comparison::NonZero, budget)?);
            }
            Operator::IntegerPower(exponent) => {
                if exponent <= 0 {
                    let base = arguments.first().copied().ok_or_else(unknown)?;
                    required.push(self.atom(base, None, Comparison::NonZero, budget)?);
                }
            }
            Operator::Predicate(_) => return Ok(self.get(Mode::Predicate, id)?.defined),
            _ => return Err(unknown()),
        }
        self.push(Formula::And(required), budget)
    }

    /// Reconstruct an affine form from exact coefficients, never a sampled value.
    fn atom(
        &mut self,
        input: LogicalId,
        other: Option<LogicalId>,
        comparison: Comparison,
        budget: &mut Budget,
    ) -> Result<usize, Fault> {
        let (mut slope, mut intercept) = affine(self.state, input, self.coordinate, budget)?;
        if let Some(other) = other {
            let (other_slope, other_intercept) =
                affine(self.state, other, self.coordinate, budget)?;
            slope = slope.sub(&other_slope, budget)?;
            intercept = intercept.sub(&other_intercept, budget)?;
        }
        if slope.is_zero() {
            let truth = match comparison {
                Comparison::Equal => intercept.is_zero(),
                Comparison::NonZero => !intercept.is_zero(),
                Comparison::Positive => !intercept.is_negative() && !intercept.is_zero(),
                Comparison::NonNegative => !intercept.is_negative(),
            };
            return self.push(Formula::Constant(truth), budget);
        }
        let cut = Some(intercept.negated(budget)?.div(&slope, budget)?);
        let negative = slope.is_negative();
        let zero = false;
        self.push(
            Formula::Atom {
                cut,
                negative,
                zero,
                comparison,
            },
            budget,
        )
    }
}

/// Fold only already known total Boolean constants, preserving partial predicate definedness.
fn fold(formula: Formula, previous: &[Formula], budget: &mut Budget) -> Result<Formula, Fault> {
    let constant = |index: &usize| match previous.get(*index) {
        Some(Formula::Constant(value)) => Some(*value),
        _ => None,
    };
    let result = match &formula {
        Formula::And(values) => {
            budget.condition(size(values.len(), AT)?, AT)?;
            if values.iter().any(|id| constant(id) == Some(false)) {
                Some(false)
            } else if values.iter().all(|id| constant(id) == Some(true)) {
                Some(true)
            } else {
                None
            }
        }
        Formula::Or(values) => {
            budget.condition(size(values.len(), AT)?, AT)?;
            if values.iter().any(|id| constant(id) == Some(true)) {
                Some(true)
            } else if values.iter().all(|id| constant(id) == Some(false)) {
                Some(false)
            } else {
                None
            }
        }
        Formula::Not(value) => constant(value).map(|value| !value),
        _ => None,
    };
    Ok(result.map_or(formula, Formula::Constant))
}

/// Resolve a Boolean expression or a graph predicate without changing its meaning.
fn predicate(state: &State, id: LogicalId) -> Result<(&PredicateKind, &[LogicalId]), Fault> {
    match state.node(id)? {
        Vertex::Condition(value) => Ok((&value.kind, &value.arguments)),
        Vertex::Expression(value) => match &value.operator {
            Operator::Predicate(kind) => Ok((kind, &value.operands)),
            _ => Err(unknown()),
        },
        _ => Err(unknown()),
    }
}

/// Enumerate only the typed dependencies needed by one compiler node.
fn dependencies(
    state: &State,
    mode: Mode,
    id: LogicalId,
    budget: &mut Budget,
) -> Result<Vec<(Mode, LogicalId)>, Fault> {
    let mut result = Vec::new();
    match mode {
        Mode::Region => match &state.region(id)?.expression {
            RegionExpr::Ambient | RegionExpr::Empty => {}
            RegionExpr::GuardTrue { base, predicate }
            | RegionExpr::GuardFalse { base, predicate } => {
                result
                    .try_reserve_exact(2)
                    .map_err(|_| Fault::allocation(AT))?;
                result.extend([(Mode::Region, *base), (Mode::Predicate, *predicate)]);
            }
            RegionExpr::Meet(members) | RegionExpr::Union(members) => {
                result
                    .try_reserve_exact(members.len())
                    .map_err(|_| Fault::allocation(AT))?;
                result.extend(members.iter().map(|id| (Mode::Region, *id)));
            }
            RegionExpr::Difference { base, excluded } => {
                result
                    .try_reserve_exact(2)
                    .map_err(|_| Fault::allocation(AT))?;
                result.extend([(Mode::Region, *base), (Mode::Region, *excluded)]);
            }
        },
        Mode::Predicate => {
            let (kind, arguments) = predicate(state, id)?;
            let mode = if matches!(
                kind,
                PredicateKind::And | PredicateKind::Or | PredicateKind::Not
            ) {
                Mode::Predicate
            } else {
                Mode::Defined
            };
            result
                .try_reserve_exact(arguments.len())
                .map_err(|_| Fault::allocation(AT))?;
            result.extend(arguments.iter().map(|id| (mode, *id)));
        }
        Mode::Defined => {
            let expression = state.expression(id)?;
            if matches!(expression.operator, Operator::Predicate(_)) {
                result
                    .try_reserve_exact(1)
                    .map_err(|_| Fault::allocation(AT))?;
                result.push((Mode::Predicate, id));
            } else {
                result
                    .try_reserve_exact(expression.operands.len())
                    .map_err(|_| Fault::allocation(AT))?;
                result.extend(expression.operands.iter().map(|id| (Mode::Defined, *id)));
            }
        }
    }
    budget.check(BudgetKind::ConditionAtoms, size(result.len(), AT)?, AT)?;
    budget.condition(size(result.len(), AT)?, AT)?;
    Ok(result)
}

/// Extract exactly two affine coefficients after the separate domain pass.
fn affine(
    state: &State,
    input: LogicalId,
    coordinate: &str,
    budget: &mut Budget,
) -> Result<(Rational, Rational), Fault> {
    let coefficients = crate::polynomial::univariate(state, input, coordinate, budget)?;
    if coefficients.len() > 2 {
        return Err(unknown());
    }
    Ok((
        coefficients.get(1).cloned().unwrap_or_else(Rational::zero),
        coefficients.first().cloned().unwrap_or_else(Rational::zero),
    ))
}

/// Collect and sort exact cuts with a fallible, metered insertion comparison.
fn cuts(formulas: &[Formula], budget: &mut Budget) -> Result<Vec<Rational>, Fault> {
    let mut result: Vec<Rational> = Vec::new();
    for formula in formulas {
        let Formula::Atom { cut: Some(cut), .. } = formula else {
            continue;
        };
        let mut position = 0_usize;
        let mut equal = false;
        for existing in &result {
            match cut.compare(existing, budget)? {
                Ordering::Less => break,
                Ordering::Equal => {
                    equal = true;
                    break;
                }
                Ordering::Greater => {
                    position = position
                        .checked_add(1)
                        .ok_or_else(|| Fault::capacity(BudgetKind::RegionCells, AT))?;
                }
            }
        }
        if equal {
            continue;
        }
        let count = size(result.len(), AT)?
            .checked_add(1)
            .and_then(|n| n.checked_mul(2))
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| Fault::capacity(BudgetKind::RegionCells, AT))?;
        budget.specialized(BudgetKind::RegionCells, count, false, AT)?;
        result.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        result.insert(position, cut.clone());
    }
    Ok(result)
}

/// Determine one affine sign from its exact root's cell position.
fn atom_truth(
    cut: Option<&Rational>,
    negative: bool,
    zero: bool,
    comparison: Comparison,
    cuts: &[Rational],
    cell: usize,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let mut sign = if zero {
        Ordering::Equal
    } else if negative {
        Ordering::Less
    } else {
        Ordering::Greater
    };
    if let Some(cut) = cut {
        let mut position = None;
        for (index, value) in cuts.iter().enumerate() {
            budget.condition(1, AT)?;
            if value == cut {
                position = Some(index);
                break;
            }
        }
        let point = position
            .ok_or_else(unknown)?
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| Fault::capacity(BudgetKind::RegionCells, AT))?;
        sign = cell.cmp(&point);
        if negative {
            sign = sign.reverse();
        }
    }
    Ok(match comparison {
        Comparison::Equal => sign == Ordering::Equal,
        Comparison::Positive => sign == Ordering::Greater,
        Comparison::NonNegative => sign != Ordering::Less,
        Comparison::NonZero => sign != Ordering::Equal,
    })
}

/// Evaluate one cell by coefficient signs and Boolean identities only.
fn evaluate(
    formulas: &[Formula],
    cuts: &[Rational],
    cell: usize,
    budget: &mut Budget,
) -> Result<Vec<bool>, Fault> {
    let mut values: Vec<bool> = Vec::new();
    values
        .try_reserve_exact(formulas.len())
        .map_err(|_| Fault::allocation(AT))?;
    for formula in formulas {
        budget.condition(1, AT)?;
        let value = match formula {
            Formula::Constant(value) => *value,
            Formula::Atom {
                cut,
                negative,
                zero,
                comparison,
            } => atom_truth(
                cut.as_ref(),
                *negative,
                *zero,
                *comparison,
                cuts,
                cell,
                budget,
            )?,
            Formula::Not(index) => !values.get(*index).copied().ok_or_else(unknown)?,
            Formula::And(inputs) | Formula::Or(inputs) => {
                let conjunction = matches!(formula, Formula::And(_));
                let mut value = conjunction;
                for input in inputs {
                    budget.condition(1, AT)?;
                    let child = values.get(*input).copied().ok_or_else(unknown)?;
                    value = if conjunction {
                        value && child
                    } else {
                        value || child
                    };
                }
                value
            }
        };
        values.push(value);
    }
    Ok(values)
}

/// Test whether an exact real cell contains any integer, including negative bounds.
fn inhabited(
    cuts: &[Rational],
    cell: usize,
    integer: bool,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    if !integer {
        return Ok(true);
    }
    if cell % 2 == 1 {
        return Ok(cuts.get(cell / 2).ok_or_else(unknown)?.is_integer());
    }
    let gap = cell / 2;
    if gap == 0 || gap == cuts.len() {
        return Ok(true);
    }
    let lower = cuts
        .get(gap.checked_sub(1).ok_or_else(unknown)?)
        .ok_or_else(unknown)?;
    let upper = cuts.get(gap).ok_or_else(unknown)?;
    let least = lower.floor(budget)?.add(&Rational::one(), budget)?;
    Ok(least.compare(upper, budget)? == Ordering::Less)
}

/// Recompute the complete common partition for a family of exact regions.
pub(crate) fn partition_at(
    state: &State,
    roots: &[LogicalId],
    boundary: Option<&Rational>,
    budget: &mut Budget,
) -> Result<RegionCertificate, Fault> {
    let first = state.region(roots.first().copied().ok_or_else(unknown)?)?;
    let (coordinate, integer) = match &first.ambient {
        AmbientDomain::Real(key) => (key.as_str(), false),
        AmbientDomain::Integer(key) => (key.as_str(), true),
        AmbientDomain::Assignments => ("", false),
        AmbientDomain::Opaque(_) | AmbientDomain::Product { .. } => return Err(unknown()),
    };
    for root in roots {
        let region = state.region(*root)?;
        if region.ambient != first.ambient || region.scope != first.scope {
            return Err(unknown());
        }
    }
    let mut compiler = Compiler {
        state,
        coordinate,
        formulas: vec![Formula::Constant(true)],
        meanings: BTreeMap::new(),
    };
    compiler.compile(roots, budget)?;
    if let Some(boundary) = boundary {
        budget.condition(1, AT)?;
        compiler
            .formulas
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        compiler.formulas.push(Formula::Atom {
            cut: Some(boundary.clone()),
            negative: false,
            zero: false,
            comparison: Comparison::Equal,
        });
    }
    let cuts = cuts(&compiler.formulas, budget)?;
    let count = cuts
        .len()
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| Fault::capacity(BudgetKind::RegionCells, AT))?;
    budget.specialized(BudgetKind::RegionCells, size(count, AT)?, false, AT)?;
    let mut membership = Vec::new();
    membership
        .try_reserve_exact(roots.len())
        .map_err(|_| Fault::allocation(AT))?;
    for _ in roots {
        let mut row = Vec::new();
        row.try_reserve_exact(count)
            .map_err(|_| Fault::allocation(AT))?;
        membership.push(row);
    }
    for cell in 0..count {
        let present = inhabited(&cuts, cell, integer, budget)?;
        let values = evaluate(&compiler.formulas, &cuts, cell, budget)?;
        for (root, row) in roots.iter().zip(&mut membership) {
            let index = compiler.get(Mode::Region, *root)?.truth;
            row.push(present && values.get(index).copied().ok_or_else(unknown)?);
        }
    }
    Ok(RegionCertificate { cuts, membership })
}

/// Require a nonempty connected open real interval for a declared distribution space.
pub(crate) fn open_interval(
    state: &State,
    root: LogicalId,
    budget: &mut Budget,
) -> Result<(), Fault> {
    check_open(&partition(state, &[root], budget)?)
}

/// Require one nonempty run of cells with open exterior endpoints.
pub(crate) fn check_open(certificate: &RegionCertificate) -> Result<(), Fault> {
    let membership = certificate.membership.first().ok_or_else(unknown)?;
    let first = membership
        .iter()
        .position(|value| *value)
        .ok_or_else(unknown)?;
    let last = membership
        .iter()
        .rposition(|value| *value)
        .ok_or_else(unknown)?;
    if first % 2 == 1 || last % 2 == 1 {
        return Err(unknown());
    }
    for (index, value) in membership.iter().enumerate() {
        if index >= first && index <= last && !value {
            return Err(unknown());
        }
    }
    Ok(())
}

/// Recompute an ordinary region partition without adding a boundary cut.
pub(crate) fn partition(
    state: &State,
    roots: &[LogicalId],
    budget: &mut Budget,
) -> Result<RegionCertificate, Fault> {
    partition_at(state, roots, None, budget)
}

/// A boundary point in the executable fragment must be an exact constant on its real axis.
pub(crate) fn boundary_point(
    state: &State,
    boundary: LogicalId,
    budget: &mut Budget,
) -> Result<Rational, Fault> {
    let boundary = state.boundary(boundary)?;
    let AmbientDomain::Real(coordinate) = &boundary.ambient else {
        return Err(unknown());
    };
    let (slope, constant) = affine(state, boundary.location, coordinate, budget)?;
    if !slope.is_zero() {
        return Err(unknown());
    }
    Ok(constant)
}
