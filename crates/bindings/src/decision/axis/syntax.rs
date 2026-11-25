// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical sign regions and original continuations are declared before any enclosure.

use super::{AxisPhaseSpec, DecisionError};
use crate::Budget;
use nrz_cas::{
    Cas, Handle, Rational, Reference,
    decision::CasePartition,
    domain::{AmbientDomain, CaseArmDraft, CasesDraft, RegionDraft, RegionExpr},
    syntax::{AtomicSort, Constant, Draft, ExpressionDraft, Field, Operator, PredicateKind, Sort},
};

/// Small syntax adapter preserving the original header and full source domain.
pub(super) struct Builder<'a> {
    /// The authoritative graph, borrowed for one declaration or proof construction.
    pub(super) cas: &'a mut Cas,
    /// Original declaration header.
    header: Handle,
    /// Original source context's complete domain.
    domain: Handle,
    /// Ambient type copied from that exact domain.
    ambient: AmbientDomain,
}

impl<'a> Builder<'a> {
    /// Require the supported full source domain; assumptions remain in its original context.
    pub(super) fn new(
        cas: &'a mut Cas,
        spec: &AxisPhaseSpec,
        budget: &mut Budget,
    ) -> Result<Self, DecisionError> {
        let header = cas.context_header(&spec.context, budget)?;
        let domain = cas.admissible_region(&spec.context, budget)?;
        let syntax = cas.describe_syntax(&domain, budget)?;
        let nrz_cas::source::SyntaxNode::Region(
            ambient,
            nrz_cas::source::RegionConstructor::Ambient,
        ) = syntax.node
        else {
            return Err(DecisionError::Case);
        };
        Ok(Self {
            cas,
            header,
            domain,
            ambient,
        })
    }
    /// One admitted object retains the original graph authority.
    pub(super) fn admit(
        &mut self,
        draft: Draft,
        budget: &mut Budget,
    ) -> Result<Handle, DecisionError> {
        self.cas
            .admit(&[draft], budget)?
            .pop()
            .ok_or(DecisionError::Capacity)
    }
    /// Original Real arithmetic, without replacing or dropping any operands.
    pub(super) fn term(
        &mut self,
        operator: Operator,
        operands: &[Handle],
        budget: &mut Budget,
    ) -> Result<Handle, DecisionError> {
        self.admit(
            Draft::Expression(ExpressionDraft {
                operator,
                sort: real(),
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                binding: None,
                space: None,
                operands: operands.iter().cloned().map(Reference::Existing).collect(),
            }),
            budget,
        )
    }
    /// Exact predicates keep ordered arguments and their original header.
    pub(super) fn predicate(
        &mut self,
        kind: PredicateKind,
        operands: &[Handle],
        budget: &mut Budget,
    ) -> Result<Handle, DecisionError> {
        self.admit(
            Draft::Condition {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                kind,
                arguments: operands.iter().cloned().map(Reference::Existing).collect(),
            },
            budget,
        )
    }
    /// Only true guards of the full original source domain form the three sign regions.
    fn region(&mut self, predicate: Handle, budget: &mut Budget) -> Result<Handle, DecisionError> {
        self.admit(
            Draft::Region(RegionDraft {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                ambient: self.ambient.clone(),
                expression: RegionExpr::GuardTrue {
                    base: Reference::Existing(self.domain.clone()),
                    predicate: Reference::Existing(predicate),
                },
            }),
            budget,
        )
    }
    /// Exact full negative/zero/positive partition, with outcomes independent of phase values.
    pub(super) fn partition(
        &mut self,
        coordinate: &Handle,
        budget: &mut Budget,
    ) -> Result<(CasePartition, [[Handle; 2]; 3]), DecisionError> {
        let negative = self.term(Operator::Neg, std::slice::from_ref(coordinate), budget)?;
        let zero = self.term(Operator::Rational(Rational::zero()), &[], budget)?;
        let predicates = [
            self.predicate(PredicateKind::Positive, &[negative], budget)?,
            self.predicate(PredicateKind::Equal, &[coordinate.clone(), zero], budget)?,
            self.predicate(
                PredicateKind::Positive,
                std::slice::from_ref(coordinate),
                budget,
            )?,
        ];
        let mut branches = Vec::new();
        for (predicate, value) in predicates.into_iter().zip([-1, 0, 1]) {
            let region = self.region(predicate, budget)?;
            let value = Rational::from_ratio(value, 1, budget)?;
            let value = self.term(Operator::Rational(value), &[], budget)?;
            let arm = self.admit(
                Draft::CaseArm(CaseArmDraft {
                    header: Reference::Existing(self.header.clone()),
                    scope: Vec::new(),
                    placements: Vec::new(),
                    region: Reference::Existing(region.clone()),
                    value: Reference::Existing(value),
                }),
                budget,
            )?;
            branches.push([region, arm]);
        }
        let branches: [[Handle; 2]; 3] =
            branches.try_into().map_err(|_| DecisionError::Capacity)?;
        let function = self.admit(
            Draft::Cases(CasesDraft {
                sort: real(),
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                domain: Reference::Existing(self.domain.clone()),
                arms: branches
                    .iter()
                    .map(|branch| Reference::Existing(branch[1].clone()))
                    .collect(),
            }),
            budget,
        )?;
        Ok((
            CasePartition {
                function,
                coordinates: vec![coordinate.clone()],
                integer: None,
            },
            branches,
        ))
    }
    /// Both half-pi branches and the original reciprocal exist before a selected sign.
    pub(super) fn continuations(
        &mut self,
        coordinate: &Handle,
        budget: &mut Budget,
    ) -> Result<([Handle; 2], Handle), DecisionError> {
        let one = self.term(Operator::Rational(Rational::one()), &[], budget)?;
        let two = Rational::from_ratio(2, 1, budget)?;
        let two = self.term(Operator::Rational(two), &[], budget)?;
        let pi = self.term(Operator::Constant(Constant::Pi), &[], budget)?;
        let positive = self.term(Operator::Div, &[pi, two], budget)?;
        let negative = self.term(Operator::Neg, std::slice::from_ref(&positive), budget)?;
        let reciprocal = self.term(Operator::Div, &[one, coordinate.clone()], budget)?;
        Ok(([negative, positive], reciprocal))
    }
}

/// The sign question has a Real coordinate, independently of the physical angle kind.
pub(super) const fn real() -> Sort {
    Sort::Value(AtomicSort::Scalar(Field::Real))
}
