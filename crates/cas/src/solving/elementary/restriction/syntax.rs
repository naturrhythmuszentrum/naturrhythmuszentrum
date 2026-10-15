// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded structural transport preserves declarations, binders and linked root choices.

use super::super::proof::{AT, invalid};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, ExpressionAttachment, Handle, LogicalId, Vertex};
use crate::model::{AtomicSort, Field, Operator, Scope, Sort};
use crate::proof_model::{CheckedApplication, Statement};
use crate::solving::coefficients::storage::{reserve, vector};
use crate::solving::{RealRestrictionTransport, SolveProblemData, SolveStatement};

/// One correspondence is shared across all branches, so linked choices cannot separate.
pub(crate) struct Correspondence<'a> {
    /// Store holding both explicitly related headers.
    pub(crate) cas: &'a Cas,
    /// Original problem in the real ambient.
    pub(crate) real: &'a SolveProblemData<Handle>,
    /// Earlier covered problem in the complex ambient.
    pub(crate) complex: &'a SolveProblemData<Handle>,
    /// Authoritative ordered declaration and region correspondence.
    pub(crate) transport: &'a RealRestrictionTransport,
    /// Earlier checked facts available for root existence and classification.
    pub(crate) premises: &'a [CheckedApplication],
    /// Consistent real and complex root-selection pairs accumulated during comparison.
    selections: Vec<(LogicalId, LogicalId)>,
}

impl<'a> Correspondence<'a> {
    /// Allocate only the bounded correspondence needed by this application.
    pub(crate) fn new(
        cas: &'a Cas,
        real: &'a SolveProblemData<Handle>,
        complex: &'a SolveProblemData<Handle>,
        transport: &'a RealRestrictionTransport,
        premises: &'a [CheckedApplication],
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        Ok(Self {
            cas,
            real,
            complex,
            transport,
            premises,
            selections: vector(0, budget)?,
        })
    }

    /// Scalar restriction may narrow the standard complex field to its real subfield.
    fn scalar(real: &Field, complex: &Field) -> bool {
        real == complex || (*real == Field::Real && *complex == Field::Complex)
    }

    /// Bound positions and frame widths are invariant under field restriction.
    fn scope(real: &Scope, complex: &Scope) -> bool {
        real.len() == complex.len()
            && real.iter().zip(complex).all(|(left, right)| {
                left.len() == right.len()
                    && left.iter().zip(right).all(|(a, b)| match (a, b) {
                        (AtomicSort::Scalar(real), AtomicSort::Scalar(complex)) => {
                            Self::scalar(real, complex)
                        }
                        _ => a == b,
                    })
            })
    }

    /// Only explicitly named free-coordinate pairs cross a header boundary.
    fn declaration(&self, real: &str, complex: &str, budget: &mut Budget) -> Result<(), Fault> {
        for pair in self
            .transport
            .coefficients
            .iter()
            .chain(&self.transport.unknowns)
        {
            budget.work(1, AT)?;
            let left = self.cas.state.expression(pair.real.logical_id())?;
            let right = self.cas.state.expression(pair.complex.logical_id())?;
            if matches!((&left.operator, &right.operator), (Operator::Free(a), Operator::Free(b)) if a == real && b == complex)
            {
                return Ok(());
            }
        }
        Err(invalid())
    }

    /// Queue ordered dependencies with their storage reservation.
    fn pairs(
        pending: &mut Vec<(LogicalId, LogicalId)>,
        real: &[Handle],
        complex: &[Handle],
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        if real.len() != complex.len() {
            return Err(invalid());
        }
        reserve(pending, real.len(), budget)?;
        pending.extend(
            real.iter()
                .zip(complex)
                .map(|(a, b)| (a.logical_id(), b.logical_id())),
        );
        Ok(())
    }

    /// Every root choice maps consistently and has an independently checked target existence.
    fn selection(
        &mut self,
        real: LogicalId,
        complex: LogicalId,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        for (left, right) in &self.selections {
            budget.work(1, AT)?;
            if *right == complex {
                return if *left == real {
                    Ok(())
                } else {
                    Err(invalid())
                };
            }
        }
        let selected = super::super::selected::selection(self.cas, &self.cas.handle(real))?;
        let relation = super::super::selected::relation(
            self.cas,
            selected.relations.first().ok_or_else(invalid)?,
        )?;
        let proved = self.premises.iter().any(|proof| matches!(&proof.judgment.statement, Statement::Solve(statement) if matches!(statement.as_ref(), SolveStatement::RootExistence(value) if value.relation.logical_id() == real && value.region == relation.region)));
        if !proved {
            return Err(invalid());
        }
        reserve(&mut self.selections, 1, budget)?;
        self.selections.push((real, complex));
        Ok(())
    }

    /// Compare arithmetic syntax and its explicit literal attachments.
    fn expression(
        &mut self,
        real: LogicalId,
        complex: LogicalId,
        pending: &mut Vec<(LogicalId, LogicalId)>,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let left = self.cas.state.expression(real)?;
        let right = self.cas.state.expression(complex)?;
        if left.binding.is_some()
            || right.binding.is_some()
            || left.domain.is_some()
            || right.domain.is_some()
            || !left.arms.is_empty()
            || !right.arms.is_empty()
        {
            return Err(invalid());
        }
        let sorts = match (&left.sort, &right.sort) {
            (Sort::Value(AtomicSort::Scalar(a)), Sort::Value(AtomicSort::Scalar(b))) => {
                Self::scalar(a, b)
            }
            (a, b) => a == b,
        };
        if (left.header, right.header)
            != (
                self.real.header.logical_id(),
                self.complex.header.logical_id(),
            )
            || !sorts
            || !Self::scope(&left.scope, &right.scope)
            || left.space.is_some()
            || right.space.is_some()
            || left.binding != right.binding
        {
            return Err(invalid());
        }
        match (&left.operator, &right.operator) {
            (Operator::Free(a), Operator::Free(b)) => self.declaration(a, b, budget)?,
            (a, b)
                if a == b
                    && matches!(
                        a,
                        Operator::Rational(_)
                            | Operator::Bound { .. }
                            | Operator::Add
                            | Operator::Sub
                            | Operator::Mul
                            | Operator::Div
                            | Operator::Neg
                            | Operator::IntegerPower(_)
                            | Operator::RootValue { .. }
                            | Operator::ExactCoefficient
                            | Operator::Conjugate
                            | Operator::RealPart
                            | Operator::ImagPart
                    ) => {}
            _ => return Err(invalid()),
        }
        match (&left.attachment, &right.attachment) {
            (None, None) => {}
            (
                Some(ExpressionAttachment::SelectedRoot(a)),
                Some(ExpressionAttachment::SelectedRoot(b)),
            ) => {
                self.selection(*a, *b, budget)?;
                reserve(pending, 1, budget)?;
                pending.push((*a, *b));
            }
            (
                Some(ExpressionAttachment::ExactCoefficient { value: a, map: m }),
                Some(ExpressionAttachment::ExactCoefficient { value: b, map: n }),
            ) => {
                reserve(pending, 2, budget)?;
                pending.extend([(*a, *b), (*m, *n)]);
            }
            _ => return Err(invalid()),
        }
        if left.operands.len() != right.operands.len() {
            return Err(invalid());
        }
        reserve(pending, left.operands.len(), budget)?;
        pending.extend(
            left.operands
                .iter()
                .zip(&right.operands)
                .map(|(a, b)| (*a, *b)),
        );
        Ok(())
    }

    /// A predicate's kind, argument order and binder scope survive the transport.
    fn predicate(
        &self,
        real: LogicalId,
        complex: LogicalId,
        pending: &mut Vec<(LogicalId, LogicalId)>,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let (a, left) = super::super::evidence::predicate(self.cas, real)?;
        let (b, right) = super::super::evidence::predicate(self.cas, complex)?;
        if !matches!(
            a,
            crate::model::PredicateKind::Equal
                | crate::model::PredicateKind::NonZero
                | crate::model::PredicateKind::Defined
                | crate::model::PredicateKind::Positive
                | crate::model::PredicateKind::NonNegative
                | crate::model::PredicateKind::And
                | crate::model::PredicateKind::Or
                | crate::model::PredicateKind::Not
        ) {
            return Err(invalid());
        }
        let (lh, ls) = crate::domain_admission::predicate_context(&self.cas.state, real)?;
        let (rh, rs) = crate::domain_admission::predicate_context(&self.cas.state, complex)?;
        if (a, left.len(), lh, rh)
            != (
                b,
                right.len(),
                self.real.header.logical_id(),
                self.complex.header.logical_id(),
            )
            || !Self::scope(ls, rs)
        {
            return Err(invalid());
        }
        reserve(pending, left.len(), budget)?;
        pending.extend(left.iter().zip(right).map(|(x, y)| (*x, *y)));
        Ok(())
    }

    /// A failed set-member candidate cannot leave a partially assigned root correspondence.
    pub(crate) fn attempt(
        &mut self,
        real: &Handle,
        complex: &Handle,
        budget: &mut Budget,
    ) -> Result<bool, Fault> {
        let saved = self.selections.len();
        let matched =
            crate::solving::elementary::product::decides(self.check(real, complex, budget))?;
        if !matched {
            self.selections.truncate(saved);
        }
        Ok(matched)
    }

    /// Traverse only the supplied paired closures, charging each visit and dependency.
    pub(crate) fn check(
        &mut self,
        real: &Handle,
        complex: &Handle,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let mut pending = vector(1, budget)?;
        pending.push((self.cas.checked(real)?, self.cas.checked(complex)?));
        while let Some((left, right)) = pending.pop() {
            budget.work(1, AT)?;
            budget.check(
                BudgetKind::Depth,
                crate::graph::size(pending.len(), AT)?,
                AT,
            )?;
            match (self.cas.state.node(left)?, self.cas.state.node(right)?) {
                (Vertex::Expression(_), Vertex::Expression(_)) => {
                    self.expression(left, right, &mut pending, budget)?;
                }
                (Vertex::Condition(_), Vertex::Condition(_)) => {
                    self.predicate(left, right, &mut pending, budget)?;
                }
                (Vertex::Region(_), Vertex::Region(_)) => {
                    self.region(left, right, &mut pending, budget)?;
                }
                (Vertex::Solve(a), Vertex::Solve(b)) => self.object(a, b, &mut pending, budget)?,
                _ => return Err(invalid()),
            }
        }
        Ok(())
    }
}

// Region and solving-object comparisons share the same bounded work queue.
mod objects;
