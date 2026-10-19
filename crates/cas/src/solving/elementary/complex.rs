// SPDX-License-Identifier: AGPL-3.0-only
//! Checked real components retain the equations and predicates of actual selected roots.

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared component checks remain crate-private"
)]
pub(crate) mod evidence;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared component checks remain crate-private"
)]
pub(crate) mod mapped;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared component checks remain crate-private"
)]
pub(crate) mod parts;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared component checks remain crate-private"
)]
pub(crate) mod selection;

#[expect(
    clippy::redundant_pub_crate,
    reason = "the conjugation checker uses bounded positive cancellation"
)]
mod positive;

use super::evaluate::Evaluation;
use super::proof::{AT, invalid};
use super::value::Value;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{ExpressionAttachment, LogicalId};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::solving::coefficients::forms::{Form, Variable};
use crate::solving::coefficients::storage::{reserve, vector};
use parts::Parts;

/// Component identities distinguish declarations from every slot of every joint selection.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Atom {
    /// A scoped bound expression without a supplied root-slot substitution.
    Expression(LogicalId),
    /// One free declaration in the current header, shared across lifted binder scopes.
    Declaration(u32),
    /// One ordered slot of the exact graph-owned joint root selection.
    Root(LogicalId, u32),
}

/// One bounded interpretation shares component variables across maps, premises and conclusions.
pub(crate) struct Components<'a, 'b> {
    /// Owning graph, coefficient arithmetic, exact region and earlier checked premises.
    evaluation: &'a Evaluation<'b>,
    /// Shared real and imaginary components for each declaration or selected root slot.
    atoms: Vec<(Atom, Parts)>,
    /// Joint selections whose defining equations and predicates have already been recorded.
    selections: Vec<LogicalId>,
    /// Proved zero polynomials over the current real component variables.
    equations: Vec<Form>,
    /// Exact regional values already known to be nonzero.
    nonzero: Vec<Value>,
    /// Exact regional real values already known to be strictly positive.
    positive: Vec<Value>,
    /// Explicit premise slots already imported once in this interpretation.
    imported: Vec<u32>,
    /// Current nesting across expression and scalar-map dependencies.
    depth: u64,
}

impl<'a, 'b> Components<'a, 'b> {
    /// Region guards initialize the interpretation; explicit premise slots are imported separately.
    pub(crate) fn new(evaluation: &'a Evaluation<'b>, budget: &mut Budget) -> Result<Self, Fault> {
        let mut result = Self {
            evaluation,
            atoms: vector(0, budget)?,
            selections: vector(0, budget)?,
            equations: vector(0, budget)?,
            nonzero: vector(0, budget)?,
            positive: vector(0, budget)?,
            imported: vector(0, budget)?,
            depth: 0,
        };
        result.guards(budget)?;
        Ok(result)
    }

    /// Fresh real components start beyond all declaration slots used by scalar evidence.
    fn atom(&mut self, key: Atom, real: bool, budget: &mut Budget) -> Result<Parts, Fault> {
        for (stored, value) in &self.atoms {
            budget.work(1, AT)?;
            if *stored == key {
                return value.copy(self.evaluation, budget);
            }
        }
        let region = self
            .evaluation
            .cas
            .state
            .region(self.evaluation.region.logical_id())?;
        let offset = self
            .evaluation
            .cas
            .state
            .header(region.header)?
            .environment
            .declarations
            .len();
        let first = self
            .atoms
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(offset))
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(invalid)?;
        let value = Parts {
            real: Value::variable(Variable::Slot(first), &self.evaluation.engine, budget)?,
            imaginary: if real {
                Value::integer(0, &self.evaluation.engine, budget)?
            } else {
                Value::variable(
                    Variable::Slot(first.checked_add(1).ok_or_else(invalid)?),
                    &self.evaluation.engine,
                    budget,
                )?
            },
        };
        reserve(&mut self.atoms, 1, budget)?;
        self.atoms.push((key, value.copy(self.evaluation, budget)?));
        Ok(value)
    }

    /// Literal maps and selected roots are checked before their components enter an identity.
    fn leaf(
        &mut self,
        id: LogicalId,
        bound: &[Parts],
        budget: &mut Budget,
    ) -> Result<Parts, Fault> {
        let expression = self.evaluation.cas.state.expression(id)?;
        match (&expression.operator, &expression.attachment) {
            (Operator::Rational(value), None) => Parts::rational(
                self.evaluation,
                value,
                &crate::number::Rational::zero(),
                budget,
            ),
            (Operator::Bound { depth: 0, slot }, None) if !bound.is_empty() => bound
                .get(usize::try_from(*slot).map_err(|_| invalid())?)
                .ok_or_else(invalid)?
                .copy(self.evaluation, budget),
            (Operator::Free(key), None) => self.declaration(id, key, budget),
            (Operator::Bound { .. }, None) => self.atom(
                Atom::Expression(id),
                expression.sort != Sort::Value(AtomicSort::Scalar(Field::Complex)),
                budget,
            ),
            (Operator::RootValue { slot }, Some(ExpressionAttachment::SelectedRoot(selected))) => {
                self.root(*selected, *slot, budget)
            }
            (
                Operator::ExactCoefficient,
                Some(ExpressionAttachment::ExactCoefficient { value, map }),
            ) => self.literal(*value, *map, budget),
            _ => Err(invalid()),
        }
    }

    /// Lifted occurrences of one declaration keep one component identity across binder scopes.
    fn declaration(
        &mut self,
        id: LogicalId,
        key: &str,
        budget: &mut Budget,
    ) -> Result<Parts, Fault> {
        let expression = self.evaluation.cas.state.expression(id)?;
        let header = self.evaluation.cas.state.header(expression.header)?;
        for (index, declaration) in header.environment.declarations.iter().enumerate() {
            budget.work(1, AT)?;
            if declaration.key != key {
                continue;
            }
            let index = u32::try_from(index).map_err(|_| invalid())?;
            if expression.sort != Sort::Value(AtomicSort::Scalar(Field::Complex)) {
                return Ok(Parts {
                    real: Value::variable(Variable::Slot(index), &self.evaluation.engine, budget)?,
                    imaginary: Value::integer(0, &self.evaluation.engine, budget)?,
                });
            }
            return self.atom(Atom::Declaration(index), false, budget);
        }
        Err(invalid())
    }

    /// A norm is nonzero if either real component is nonzero, or regional evidence proves the norm.
    fn divide(&self, left: &Parts, right: &Parts, budget: &mut Budget) -> Result<Parts, Fault> {
        let engine = &self.evaluation.engine;
        let norm = right.real.mul(&right.real, engine, budget)?.add(
            &right.imaginary.mul(&right.imaginary, engine, budget)?,
            engine,
            budget,
        )?;
        if !super::product::decides(self.nonzero(&norm, budget))?
            && !super::product::decides(self.nonzero(&right.real, budget))?
        {
            self.nonzero(&right.imaginary, budget)?;
        }
        let numerator = left.mul(
            &right.conjugate(self.evaluation, budget)?,
            self.evaluation,
            budget,
        )?;
        Ok(Parts {
            real: numerator.real.div(&norm, engine, budget)?,
            imaginary: numerator.imaginary.div(&norm, engine, budget)?,
        })
    }

    /// Exact complex operations preserve the same component variables and selected choices.
    fn operation(
        &mut self,
        id: LogicalId,
        values: &[Parts],
        bound: &[Parts],
        budget: &mut Budget,
    ) -> Result<Parts, Fault> {
        let evaluation = self.evaluation;
        match (&evaluation.cas.state.expression(id)?.operator, values) {
            (Operator::Add, [a, b]) => a.sum(b, false, evaluation, budget),
            (Operator::Sub, [a, b]) => a.sum(b, true, evaluation, budget),
            (Operator::Mul, [a, b]) => a.mul(b, evaluation, budget),
            (Operator::Div, [a, b]) => self.divide(a, b, budget),
            (Operator::Neg, [value]) => Ok(Parts {
                real: value.real.neg(&evaluation.engine, budget)?,
                imaginary: value.imaginary.neg(&evaluation.engine, budget)?,
            }),
            (Operator::Conjugate, [value]) => value.conjugate(evaluation, budget),
            (Operator::RealPart, [value]) => Ok(Parts {
                real: value.real.copy(&evaluation.engine, budget)?,
                imaginary: Value::integer(0, &evaluation.engine, budget)?,
            }),
            (Operator::ImagPart, [value]) => Ok(Parts {
                real: value.imaginary.copy(&evaluation.engine, budget)?,
                imaginary: Value::integer(0, &evaluation.engine, budget)?,
            }),
            (Operator::IntegerPower(power), [value]) if *power > 0 => {
                value.power(*power, evaluation, budget)
            }
            (_, []) => self.leaf(id, bound, budget),
            _ => Err(invalid()),
        }
    }

    /// Recursion across earlier map dependencies is bounded independently of expression depth.
    fn expression(
        &mut self,
        root: LogicalId,
        bound: &[Parts],
        budget: &mut Budget,
    ) -> Result<Parts, Fault> {
        self.depth = self.depth.checked_add(1).ok_or_else(invalid)?;
        budget.check(BudgetKind::Depth, self.depth, AT)?;
        let result = self.walk(root, bound, budget);
        self.depth = self.depth.checked_sub(1).ok_or_else(invalid)?;
        result
    }

    /// Postfix traversal visits only the supplied closure with charged temporary storage.
    fn walk(
        &mut self,
        root: LogicalId,
        bound: &[Parts],
        budget: &mut Budget,
    ) -> Result<Parts, Fault> {
        let mut pending = vector(1, budget)?;
        pending.push((root, false));
        let mut values = vector(1, budget)?;
        while let Some((id, ready)) = pending.pop() {
            budget.work(1, AT)?;
            let expression = self.evaluation.cas.state.expression(id)?;
            budget.check(BudgetKind::Depth, expression.depth, AT)?;
            if expression.header
                != self
                    .evaluation
                    .cas
                    .state
                    .region(self.evaluation.region.logical_id())?
                    .header
                || expression.space.is_some()
            {
                return Err(invalid());
            }
            if !ready && !expression.operands.is_empty() {
                reserve(
                    &mut pending,
                    expression
                        .operands
                        .len()
                        .checked_add(1)
                        .ok_or_else(invalid)?,
                    budget,
                )?;
                pending.push((id, true));
                pending.extend(
                    expression
                        .operands
                        .iter()
                        .rev()
                        .map(|child| (*child, false)),
                );
            } else {
                let start = values
                    .len()
                    .checked_sub(expression.operands.len())
                    .ok_or_else(invalid)?;
                let value =
                    self.operation(id, values.get(start..).ok_or_else(invalid)?, bound, budget)?;
                values.truncate(start);
                reserve(&mut values, 1, budget)?;
                values.push(value);
            }
        }
        if values.len() != 1 {
            return Err(invalid());
        }
        values.pop().ok_or_else(invalid)
    }

    /// Fixed-field classification checks the imaginary part on the exact supplied region.
    pub(crate) fn real(&mut self, root: LogicalId, budget: &mut Budget) -> Result<(), Fault> {
        let parts = self.expression(root, &[], budget)?;
        self.zero(&parts.imaginary, budget)
    }

    /// A conjugated equality compares actual values under a single component interpretation.
    pub(crate) fn equal(
        &mut self,
        left: LogicalId,
        right: LogicalId,
        conjugate: bool,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let original = self.expression(left, &[], budget)?;
        let transformed = if conjugate {
            original.conjugate(self.evaluation, budget)?
        } else {
            original
        };
        let expected = self.expression(right, &[], budget)?;
        let difference = transformed.sum(&expected, true, self.evaluation, budget)?;
        self.zero(&difference.real, budget)?;
        self.zero(&difference.imaginary, budget)
    }
}
