// SPDX-License-Identifier: AGPL-3.0-only
//! A supplied arithmetic tree is a bounded certificate of ordered-real positivity.

use super::super::Components;
use super::AT;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::LogicalId;
use crate::model::{AtomicSort, Field, Operator, PredicateKind, Sort};
use crate::proof_model::{ApplicationDraft, Statement};
use crate::solving::CertificateConjugationTransport;
use crate::solving::coefficients::storage::{invalid, reserve, vector};

/// Sign information is proved, never guessed from a nonzero or complex value.
#[derive(Clone, Copy, Default)]
struct Sign {
    /// A checked lower bound by zero.
    nonnegative: bool,
    /// A checked strict lower bound by zero.
    positive: bool,
    /// A separate cited nonzero fact, which does not itself imply either sign.
    nonzero: bool,
}

impl Components<'_, '_> {
    /// Every sign node is a real scalar in the exact target region's header and scope.
    fn real_node(&self, id: LogicalId, budget: &mut Budget) -> Result<(), Fault> {
        budget.work(1, AT)?;
        let expression = self.evaluation.cas.state.expression(id)?;
        let region = self
            .evaluation
            .cas
            .state
            .region(self.evaluation.region.logical_id())?;
        if (expression.header, &expression.scope, &expression.sort)
            != (
                region.header,
                &region.scope,
                &Sort::Value(AtomicSort::Scalar(Field::Real)),
            )
        {
            return Err(invalid());
        }
        budget.check(BudgetKind::Depth, expression.depth, AT)
    }

    /// Only exact expressions in explicitly cited, checked regional predicates supply leaf signs.
    fn sign_facts(
        &self,
        draft: &ApplicationDraft,
        certificate: &CertificateConjugationTransport,
        id: LogicalId,
        budget: &mut Budget,
    ) -> Result<Sign, Fault> {
        let mut result = Sign::default();
        for slot in certificate.relations.iter().chain(&certificate.fixed_field) {
            let fact = self.cited(draft, *slot, budget)?;
            let Statement::PredicateFact { predicate, region } = &fact.judgment.statement else {
                continue;
            };
            if *region != certificate.region {
                continue;
            }
            let (kind, arguments) = super::super::super::evidence::predicate(
                self.evaluation.cas,
                predicate.logical_id(),
            )?;
            if arguments != [id] {
                continue;
            }
            match kind {
                PredicateKind::Positive => {
                    result = Sign {
                        nonnegative: true,
                        positive: true,
                        nonzero: true,
                    }
                }
                PredicateKind::NonNegative => result.nonnegative = true,
                PredicateKind::NonZero => result.nonzero = true,
                _ => {}
            }
        }
        Ok(result)
    }

    /// The real part of the exact product with its own conjugate is a sum of two real squares.
    fn norm(&self, id: LogicalId, budget: &mut Budget) -> Result<bool, Fault> {
        budget.work(1, AT)?;
        let expression = self.evaluation.cas.state.expression(id)?;
        let [operand] = expression.operands.as_slice() else {
            return Ok(false);
        };
        if expression.operator != Operator::RealPart {
            return Ok(false);
        }
        let product = self.evaluation.cas.state.expression(*operand)?;
        let [left, right] = product.operands.as_slice() else {
            return Ok(false);
        };
        if product.operator != Operator::Mul {
            return Ok(false);
        }
        for (value, candidate) in [(*left, *right), (*right, *left)] {
            budget.work(1, AT)?;
            let conjugate = self.evaluation.cas.state.expression(candidate)?;
            if conjugate.operator == Operator::Conjugate && conjugate.operands.as_slice() == [value]
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Even powers are nonnegative; strictness requires a separately cited nonzero base.
    fn sign_leaf(
        &self,
        draft: &ApplicationDraft,
        certificate: &CertificateConjugationTransport,
        id: LogicalId,
        budget: &mut Budget,
    ) -> Result<Sign, Fault> {
        let facts = self.sign_facts(draft, certificate, id, budget)?;
        if facts.nonnegative {
            return Ok(facts);
        }
        let expression = self.evaluation.cas.state.expression(id)?;
        if let Operator::Rational(value) = &expression.operator {
            let order = value.compare(&crate::number::Rational::zero(), budget)?;
            return Ok(Sign {
                nonnegative: order != std::cmp::Ordering::Less,
                positive: order == std::cmp::Ordering::Greater,
                nonzero: order != std::cmp::Ordering::Equal,
            });
        }
        if let (Operator::IntegerPower(power), [base]) =
            (&expression.operator, expression.operands.as_slice())
            && *power > 0
            && power.unsigned_abs().is_multiple_of(2)
        {
            budget.check(
                BudgetKind::IntegerExponent,
                u64::try_from(*power).map_err(|_| invalid())?,
                AT,
            )?;
            self.real_node(*base, budget)?;
            let strict = self.sign_facts(draft, certificate, *base, budget)?.nonzero;
            return Ok(Sign {
                nonnegative: true,
                positive: strict,
                nonzero: strict,
            });
        }
        Ok(Sign {
            nonnegative: self.norm(id, budget)?,
            ..facts
        })
    }

    /// Products require every factor and sums require every summand to be nonnegative.
    fn sign_operation(&self, id: LogicalId, values: &[Sign]) -> Result<Sign, Fault> {
        let expression = self.evaluation.cas.state.expression(id)?;
        let [left, right] = values else {
            return Err(invalid());
        };
        let nonnegative = left.nonnegative && right.nonnegative;
        let positive = nonnegative
            && match expression.operator {
                Operator::Add => left.positive || right.positive,
                Operator::Mul => left.positive && right.positive,
                _ => return Err(invalid()),
            };
        Ok(Sign {
            nonnegative,
            positive,
            nonzero: positive,
        })
    }

    /// Definedness is checked once for the complete tree before any structural sign reasoning.
    pub(super) fn positive_cofactor(
        &mut self,
        draft: &ApplicationDraft,
        certificate: &CertificateConjugationTransport,
        root: LogicalId,
        budget: &mut Budget,
    ) -> Result<bool, Fault> {
        self.real_node(root, budget)?;
        self.real(root, budget)?;
        let mut pending = vector(1, budget)?;
        pending.push((root, false));
        let mut values = vector(1, budget)?;
        while let Some((id, ready)) = pending.pop() {
            self.real_node(id, budget)?;
            let expression = self.evaluation.cas.state.expression(id)?;
            let leaf = self.sign_leaf(draft, certificate, id, budget)?;
            if !ready
                && !leaf.nonnegative
                && matches!(expression.operator, Operator::Add | Operator::Mul)
            {
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
                let count = if ready { expression.operands.len() } else { 0 };
                let start = values.len().checked_sub(count).ok_or_else(invalid)?;
                let value = if ready {
                    self.sign_operation(id, values.get(start..).ok_or_else(invalid)?)?
                } else {
                    leaf
                };
                values.truncate(start);
                reserve(&mut values, 1, budget)?;
                values.push(value);
            }
        }
        let [result] = values.as_slice() else {
            return Err(invalid());
        };
        Ok(result.positive)
    }
}
