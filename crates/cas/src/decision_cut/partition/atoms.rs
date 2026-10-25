// SPDX-License-Identifier: AGPL-3.0-only
//! Exact axis cuts reconstructed from original, globally total affine predicates.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use super::super::{affine, evaluate, number_bytes, temporary, unsupported};
use super::AT;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, LogicalId, Vertex, size};
use crate::model::{Operator, PredicateKind};
use crate::number::Rational;

/// Independently reconstructed total affine operands and singleton Boolean values.
pub(super) type Values = BTreeMap<LogicalId, evaluate::Value>;

/// A total constant or one exact sign-changing hyperplane parallel to all other axes.
pub(super) enum Atom {
    /// A comparison whose affine expression is exactly constant.
    Constant(bool),
    /// One rational boundary and the exact direction of its sign change.
    Cut {
        /// Position in the declared coordinate inventory.
        axis: usize,
        /// Exact zero of the original affine expression.
        point: Rational,
        /// Whether the nonzero affine slope reverses the ordered comparison.
        negative: bool,
        /// Original strict, weak or equality relation against zero.
        comparison: Comparison,
    },
}

/// The supported total scalar comparisons preserve their distinct boundary conventions.
#[derive(Clone, Copy)]
pub(super) enum Comparison {
    /// Strict positivity excludes the singleton boundary.
    Positive,
    /// Nonnegativity owns the singleton boundary.
    NonNegative,
    /// Both strict sides are included and the singleton is excluded.
    NonZero,
    /// Only the exact singleton is included.
    Equal,
}

/// Original atomic meanings and the exact sorted boundaries they induce on every axis.
pub(super) struct Program {
    /// Distinct increasing rational cuts for each declared coordinate.
    pub(super) cuts: Vec<Vec<Rational>>,
    /// Atomic comparison meanings keyed by their original immutable syntax.
    pub(super) atoms: BTreeMap<LogicalId, Atom>,
}

impl Program {
    /// Actual exact-number storage plus conservative map and vector allocation overhead.
    pub(super) fn payload(&self, budget: &mut Budget) -> Result<u64, Fault> {
        let mut bytes = 128_u64;
        for axis in &self.cuts {
            budget.work(1, AT)?;
            bytes = bytes.saturating_add(64);
            for cut in axis {
                budget.work(1, AT)?;
                bytes = bytes.saturating_add(number_bytes(cut));
            }
        }
        for atom in self.atoms.values() {
            budget.work(1, AT)?;
            bytes = bytes.saturating_add(256);
            if let Atom::Cut { point, .. } = atom {
                bytes = bytes.saturating_add(number_bytes(point));
            }
        }
        Ok(bytes)
    }

    /// Insert one rational cut in exact order, retaining no duplicate boundary.
    pub(super) fn insert(
        &mut self,
        axis: usize,
        cut: &Rational,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let bytes = self.payload(budget)?.saturating_add(number_bytes(cut));
        budget.with_retained_payload(bytes, AT, |budget| {
            let values = self.cuts.get_mut(axis).ok_or_else(unsupported)?;
            let mut index = 0;
            for present in values.iter() {
                match compare(cut, present, budget)? {
                    Ordering::Less => break,
                    Ordering::Equal => return Ok(()),
                    Ordering::Greater => index += 1,
                }
            }
            budget.check(
                BudgetKind::RegionCells,
                size(values.len(), AT)?.saturating_mul(2).saturating_add(3),
                AT,
            )?;
            budget.work(number_bytes(cut), AT)?;
            values
                .try_reserve_exact(1)
                .map_err(|_| Fault::allocation(AT))?;
            values.insert(index, cut.clone());
            Ok(())
        })
    }

    /// Locate an owned singleton in its exact axis partition.
    pub(super) fn position(
        &self,
        axis: usize,
        point: &Rational,
        budget: &mut Budget,
    ) -> Result<usize, Fault> {
        for (index, cut) in self
            .cuts
            .get(axis)
            .ok_or_else(unsupported)?
            .iter()
            .enumerate()
        {
            if compare(point, cut, budget)? == Ordering::Equal {
                return index
                    .checked_mul(2)
                    .and_then(|value| value.checked_add(1))
                    .ok_or_else(unsupported);
            }
        }
        Err(unsupported())
    }
}

/// Preserve affine cancellation before requiring a single nonzero coordinate coefficient.
pub(super) fn compile(
    cas: &Cas,
    order: &[LogicalId],
    values: &Values,
    dimensions: usize,
    budget: &mut Budget,
) -> Result<Program, Fault> {
    let mut cuts = Vec::new();
    cuts.try_reserve_exact(dimensions)
        .map_err(|_| Fault::allocation(AT))?;
    cuts.resize_with(dimensions, Vec::new);
    let mut program = Program {
        cuts,
        atoms: BTreeMap::new(),
    };
    for id in order {
        budget.work(1, AT)?;
        let Some((kind, arguments)) = predicate(cas, *id)? else {
            continue;
        };
        let retained = program
            .payload(budget)?
            .saturating_add(temporary(dimensions, budget)?);
        let atom = budget.with_retained_payload(retained, AT, |budget| {
            build(kind, arguments, *id, values, budget)
        })?;
        if let Some(atom) = atom {
            let bytes = match &atom {
                Atom::Constant(_) => 256,
                Atom::Cut { point, .. } => number_bytes(point).saturating_add(256),
            };
            budget.with_retained_payload(bytes, AT, |budget| {
                if let Atom::Cut { axis, point, .. } = &atom {
                    program.insert(*axis, point, budget)?;
                }
                let retained = program.payload(budget)?;
                budget.check(BudgetKind::StagedPayloadBytes, retained, AT)?;
                program.atoms.insert(*id, atom);
                Ok::<(), Fault>(())
            })?;
        }
    }
    Ok(program)
}

/// Boolean constructors keep their original dependencies; atomic comparisons keep exact roots.
fn build(
    kind: &PredicateKind,
    arguments: &[LogicalId],
    id: LogicalId,
    values: &Values,
    budget: &mut Budget,
) -> Result<Option<Atom>, Fault> {
    let comparison = match kind {
        PredicateKind::And | PredicateKind::Or | PredicateKind::Not => return Ok(None),
        PredicateKind::Defined | PredicateKind::IntegerRange => {
            let Some(evaluate::Value::Predicate(truth)) = values.get(&id) else {
                return Err(unsupported());
            };
            return match truth {
                evaluate::Truth::All => Ok(Some(Atom::Constant(true))),
                evaluate::Truth::None => Ok(Some(Atom::Constant(false))),
                evaluate::Truth::Mixed => Err(unsupported()),
            };
        }
        PredicateKind::Positive => Comparison::Positive,
        PredicateKind::NonNegative => Comparison::NonNegative,
        PredicateKind::NonZero => Comparison::NonZero,
        PredicateKind::Equal => Comparison::Equal,
        _ => return Err(unsupported()),
    };
    let first = affine::get(values, *arguments.first().ok_or_else(unsupported)?)?;
    let result = if matches!(comparison, Comparison::Equal) {
        let second = affine::get(values, *arguments.get(1).ok_or_else(unsupported)?)?;
        atom(
            &affine::combine(first, second, true, budget)?,
            comparison,
            budget,
        )?
    } else {
        atom(first, comparison, budget)?
    };
    Ok(Some(result))
}

/// Reduce one affine expression to a constant or a single-axis sign boundary.
fn atom(
    coefficients: &[Rational],
    comparison: Comparison,
    budget: &mut Budget,
) -> Result<Atom, Fault> {
    let constant = coefficients.first().ok_or_else(unsupported)?;
    let mut coordinate = None;
    for (index, coefficient) in coefficients.iter().skip(1).enumerate() {
        budget.work(1, AT)?;
        if !coefficient.is_zero() {
            if coordinate.is_some() {
                return Err(unsupported());
            }
            coordinate = Some((index, coefficient));
        }
    }
    let Some((axis, slope)) = coordinate else {
        let sign = constant.compare(&Rational::zero(), budget)?;
        return Ok(Atom::Constant(holds(comparison, sign)));
    };
    let point = constant.negated(budget)?.div(slope, budget)?;
    Ok(Atom::Cut {
        axis,
        point,
        negative: slope.is_negative(),
        comparison,
    })
}

/// Interpret the original comparison using an exact ordered-field sign.
pub(super) const fn holds(comparison: Comparison, sign: Ordering) -> bool {
    match comparison {
        Comparison::Positive => matches!(sign, Ordering::Greater),
        Comparison::NonNegative => !matches!(sign, Ordering::Less),
        Comparison::NonZero => !matches!(sign, Ordering::Equal),
        Comparison::Equal => matches!(sign, Ordering::Equal),
    }
}

/// Borrow a native condition or Boolean expression without treating other nodes as predicates.
pub(super) fn predicate(
    cas: &Cas,
    id: LogicalId,
) -> Result<Option<(&PredicateKind, &[LogicalId])>, Fault> {
    Ok(match cas.state.node(id)? {
        Vertex::Condition(value) => Some((&value.kind, &value.arguments)),
        Vertex::Expression(value) => match &value.operator {
            Operator::Predicate(kind) => Some((kind, &value.operands)),
            _ => None,
        },
        _ => None,
    })
}

/// Prepay continued-fraction copies and division scratch from both actual operand sizes.
fn compare(left: &Rational, right: &Rational, budget: &mut Budget) -> Result<Ordering, Fault> {
    let scratch = number_bytes(left)
        .saturating_add(number_bytes(right))
        .saturating_mul(4)
        .saturating_add(256);
    budget.with_retained_payload(scratch, AT, |budget| left.compare(right, budget))
}
