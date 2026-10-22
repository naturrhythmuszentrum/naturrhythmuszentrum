// SPDX-License-Identifier: AGPL-3.0-only
//! Independent named Real roots and division-free odd-power injectivity.

use super::leaf::{Leaf, Signed};
use crate::graph::Handle;
use crate::model::{Operator, PredicateKind};
use crate::proof_model::{DomainContract, Statement};
use crate::solving::coefficients::{
    arithmetic::Engine,
    model::{CoefficientFailure, Element},
    storage::invalid,
};
use crate::solving::radicals::construction::Extension;
use crate::solving::{RealSignConclusion as Sign, RealSignRule as Rule};

impl Leaf<'_> {
    /// Positive square roots retain the actual bound-slot constraint and the exact radicand sign.
    pub(crate) fn square_root(
        &mut self,
        radicand: &Element,
        positive: &Signed,
    ) -> Result<(Extension, Signed), CoefficientFailure> {
        let engine = Engine {
            state: &self.cas.state,
            domain: &radicand.domain,
        };
        let polynomial = super::derive::binomial(&engine, radicand, 2, self.budget)?;
        let constant = polynomial.coefficients.first().ok_or_else(invalid)?;
        let literal = self
            .frame
            .literal(self.cas, constant, &self.map, self.budget)?;
        let negated = self.expression(Operator::Neg, &[literal])?;
        let sign = self.transport(positive, &negated)?;
        self.predicate(&sign, PredicateKind::Positive)?;
        let extension =
            self.frame
                .adjoin(self.cas, &polynomial, &self.map, Some(2), self.budget)?;
        self.budget.work(
            crate::graph::size(self.frame.steps.len(), super::AT)?,
            super::AT,
        )?;
        let count = self
            .frame
            .steps
            .iter()
            .filter(|step| step.context == self.frame.context)
            .count();
        let slot =
            u32::try_from(count.checked_sub(1).ok_or_else(invalid)?).map_err(|_| invalid())?;
        let positive = self.derive(
            Rule::Root,
            &extension.root,
            Sign::Positive,
            &[],
            vec![slot],
            0,
        )?;
        Ok((extension, positive))
    }

    /// Odd roots are selected independently; no reciprocal of either selected root is introduced.
    pub(crate) fn odd_root(
        &mut self,
        radicand: &Element,
        map: &Handle,
        degree: u32,
    ) -> Result<Handle, CoefficientFailure> {
        let engine = Engine {
            state: &self.cas.state,
            domain: &radicand.domain,
        };
        let polynomial = super::derive::binomial(&engine, radicand, degree, self.budget)?;
        Ok(self
            .frame
            .named(self.cas, &polynomial, map, degree, self.budget)?
            .1)
    }

    /// Exact power equality plus Real odd-power injectivity gives the needed scalar identity.
    pub(crate) fn injective(
        &mut self,
        left: &Handle,
        right: &Handle,
        exponent: u32,
    ) -> Result<u32, CoefficientFailure> {
        let first = self.expression(
            Operator::IntegerPower(i64::from(exponent)),
            std::slice::from_ref(left),
        )?;
        let second = self.expression(
            Operator::IntegerPower(i64::from(exponent)),
            std::slice::from_ref(right),
        )?;
        let powered = self.expression(Operator::Sub, &[first, second])?;
        let zero = self.derive(
            Rule::Identity,
            &powered,
            Sign::Zero,
            &[],
            vec![self.unit],
            0,
        )?;
        let difference = self.expression(Operator::Sub, &[left.clone(), right.clone()])?;
        let result = self.derive(
            Rule::OddPowerReflection,
            &difference,
            Sign::Zero,
            &[&zero],
            Vec::new(),
            exponent,
        )?;
        let region = self.frame.region.clone();
        self.frame.steps.last_mut().ok_or_else(invalid)?.conclusion = Statement::Equality {
            left: left.clone(),
            right: right.clone(),
            domain: region,
            domain_contract: DomainContract::DefinedAndEqual,
        };
        Ok(result.slot)
    }

    /// A checked zero scalar supplies an ordinary equality without cancelling any parameter.
    pub(crate) fn zero_equality(&mut self, source: &Signed) -> Result<u32, CoefficientFailure> {
        let zero = self.rational_sign(&crate::number::Rational::zero())?;
        let difference = self.expression(
            Operator::Sub,
            &[source.subject.clone(), zero.subject.clone()],
        )?;
        let result = self.derive(
            Rule::Sum,
            &difference,
            Sign::Zero,
            &[source, &zero],
            Vec::new(),
            0,
        )?;
        let region = self.frame.region.clone();
        self.frame.steps.last_mut().ok_or_else(invalid)?.conclusion = Statement::Equality {
            left: source.subject.clone(),
            right: zero.subject,
            domain: region,
            domain_contract: DomainContract::DefinedAndEqual,
        };
        Ok(result.slot)
    }
}
