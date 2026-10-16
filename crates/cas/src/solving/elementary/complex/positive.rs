// SPDX-License-Identifier: AGPL-3.0-only
//! Cancellation consumes exact cited equalities and a finite displayed positive cofactor.

#[path = "positive_sign.rs"]
mod sign;

use super::Components;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Handle, LogicalId};
use crate::model::Operator;
use crate::proof_model::{ApplicationDraft, DomainContract, Statement};
use crate::solving::elementary::proof::invalid;
use crate::solving::{
    CertificateConjugationTransport, SolveStatement, StatementCoefficientEquality,
};

/// Separate attribution covers the exact evidence walk and the displayed positivity tree.
pub(crate) const AT: crate::error::Location =
    crate::error::Location::new("positive cofactor cancellation");

impl Components<'_, '_> {
    /// The caller enumerates only explicit certificate slots, never uncited earlier applications.
    fn cited(
        &self,
        draft: &ApplicationDraft,
        slot: u32,
        budget: &mut Budget,
    ) -> Result<&crate::proof_model::CheckedApplication, Fault> {
        budget.work(1, AT)?;
        let fact = super::super::proof::premise(self.evaluation.premises, slot)?;
        if fact.judgment.context != draft.context {
            return Err(invalid());
        }
        Ok(fact)
    }

    /// Scalar equalities retain their exact map, definedness, current context and region.
    fn equality(
        &self,
        draft: &ApplicationDraft,
        certificate: &CertificateConjugationTransport,
        slot: u32,
        budget: &mut Budget,
    ) -> Result<Option<(&Handle, &Handle)>, Fault> {
        let fact = self.cited(draft, slot, budget)?;
        let result = match &fact.judgment.statement {
            Statement::Solve(statement) => match statement.as_ref() {
                SolveStatement::CoefficientEquality(value)
                    if (&value.map, &value.region) == (&certificate.map, &certificate.region) =>
                {
                    Some((&value.left, &value.right))
                }
                _ => None,
            },
            Statement::Equality {
                left,
                right,
                domain,
                domain_contract: DomainContract::DefinedAndEqual,
            } if *domain == certificate.region => Some((left, right)),
            _ => None,
        };
        Ok(result.filter(|(left, right)| {
            left.logical_id().kind == crate::graph::VertexKind::Expression
                && right.logical_id().kind == crate::graph::VertexKind::Expression
        }))
    }

    /// Zero is actual literal syntax, rather than a proposed equality or a reducible partial expression.
    fn literal_zero(&self, handle: &Handle) -> Result<bool, Fault> {
        let expression = self.evaluation.cas.state.expression(handle.logical_id())?;
        Ok(
            matches!(&expression.operator, Operator::Rational(value) if value.is_zero())
                && expression.attachment.is_none()
                && expression.operands.is_empty(),
        )
    }

    /// The source difference is syntactically exact; a named zero needs no synthetic subtraction.
    fn difference(
        &self,
        expression: &Handle,
        first: &Handle,
        second: &Handle,
    ) -> Result<bool, Fault> {
        if expression == first && self.literal_zero(second)? {
            return Ok(true);
        }
        let value = self
            .evaluation
            .cas
            .state
            .expression(expression.logical_id())?;
        Ok(value.operator == Operator::Sub
            && value.operands.as_slice() == [first.logical_id(), second.logical_id()])
    }

    /// The desired factor is the exact left operand of the earlier checked factorization.
    fn cofactor(&self, product: &Handle, factor: &Handle) -> Result<Option<LogicalId>, Fault> {
        let expression = self.evaluation.cas.state.expression(product.logical_id())?;
        let [left, right] = expression.operands.as_slice() else {
            return Ok(None);
        };
        Ok(
            (expression.operator == Operator::Mul && *left == factor.logical_id())
                .then_some(*right),
        )
    }

    /// A checked factorization multiplies back to the exact difference of a checked source equality.
    fn candidate(
        &self,
        draft: &ApplicationDraft,
        certificate: &CertificateConjugationTransport,
        claim: &StatementCoefficientEquality,
        slots: [u32; 2],
        budget: &mut Budget,
    ) -> Result<Option<LogicalId>, Fault> {
        let Some((first, second)) = self.equality(draft, certificate, slots[0], budget)? else {
            return Ok(None);
        };
        let Some((left, right)) = self.equality(draft, certificate, slots[1], budget)? else {
            return Ok(None);
        };
        for (difference, product) in [(left, right), (right, left)] {
            budget.work(1, AT)?;
            if (self.difference(difference, first, second)?
                || self.difference(difference, second, first)?)
                && let Some(cofactor) = self.cofactor(product, &claim.left)?
            {
                return Ok(Some(cofactor));
            }
        }
        Ok(None)
    }

    /// Strict positivity implies a nonzero cofactor, so its checked zero product forces this factor to zero.
    pub(crate) fn cancel(
        &mut self,
        draft: &ApplicationDraft,
        certificate: &CertificateConjugationTransport,
        claim: &StatementCoefficientEquality,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        if !self.literal_zero(&claim.right)? {
            return Err(invalid());
        }
        for source in &certificate.relations {
            for factorization in &certificate.relations {
                budget.work(1, AT)?;
                let Some(cofactor) =
                    self.candidate(draft, certificate, claim, [*source, *factorization], budget)?
                else {
                    continue;
                };
                if self.positive_cofactor(draft, certificate, cofactor, budget)? {
                    return Ok(());
                }
            }
        }
        Err(invalid())
    }
}
