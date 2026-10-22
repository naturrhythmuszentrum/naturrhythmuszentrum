// SPDX-License-Identifier: AGPL-3.0-only
//! The one-root cubic uses independent Real radicals and a separately checked complete count.

use super::{
    derive::Cubic,
    leaf::{Leaf, Signed},
};
use crate::graph::Handle;
use crate::model::{Operator, PredicateKind};
use crate::proof_model::VerifiedWitness;
use crate::solving::coefficients::{
    arithmetic::Engine,
    model::CoefficientFailure,
    storage::{invalid, reserve, vector},
};
use crate::solving::{RealSignConclusion as Sign, RealSignRule as Rule};

impl Leaf<'_> {
    /// Independently check just this local prefix before reusing its nonzero fact in the existing Euclidean producer.
    pub(crate) fn sealed_nonzero(
        &mut self,
        sign: &Signed,
    ) -> Result<VerifiedWitness, CoefficientFailure> {
        self.mark("real-nonzero-replay")?;
        self.predicate(sign, PredicateKind::NonZero)?;
        let mut steps = vector(0, self.budget)?;
        for step in &self.frame.steps {
            self.budget.work(1, super::AT)?;
            if step.context != self.frame.context {
                continue;
            }
            let mut copied = step.clone();
            copied.premises = (0..u32::try_from(steps.len()).map_err(|_| invalid())?).collect();
            reserve(&mut steps, 1, self.budget)?;
            steps.push(copied);
        }
        Ok(self
            .cas
            .check_proof(&self.frame.context, &steps, self.budget)?)
    }

    /// On p=0, D>0 proves q nonzero without choosing its sign or manufacturing an empty stratum.
    fn nonzero_constant(
        &mut self,
        data: &Cubic,
        discriminant: &Signed,
    ) -> Result<VerifiedWitness, CoefficientFailure> {
        let four = self.integer_sign(4)?;
        let positive = self.product_sign(&four, discriminant, false)?;
        let constant = self
            .frame
            .literal(self.cas, &data.constant, &self.map, self.budget)?;
        let square = self.expression(Operator::IntegerPower(2), std::slice::from_ref(&constant))?;
        let positive = self.transport(&positive, &square)?;
        let nonzero = self.derive(
            Rule::EvenPower,
            &constant,
            Sign::NonZero,
            &[&positive],
            Vec::new(),
            2,
        )?;
        let numerator = self.remember(&data.constant, &nonzero)?;
        self.sealed_nonzero(&numerator)
    }

    /// The two cubic radicands come from the computed quadratic resolvent and retain separate odd-root existence.
    fn radical_sum(
        &mut self,
        data: &Cubic,
        discriminant: &Signed,
    ) -> Result<Handle, CoefficientFailure> {
        self.mark("real-one-root-radicals")?;
        let (square, _) = self.square_root(&data.resolvent.radicand, discriminant)?;
        let parent = Engine {
            state: &self.cas.state,
            domain: &data.base.source.domain,
        };
        let center = parent.neg(&data.resolvent.shift, self.budget)?;
        let engine = Engine {
            state: &self.cas.state,
            domain: &square.domain,
        };
        let center = engine.include_parent(&center, self.budget)?;
        let first = engine.add(&center, &square.generator, self.budget)?;
        let second = engine.sub(&center, &square.generator, self.budget)?;
        let left = self.odd_root(&first, &square.map, 3)?;
        let right = self.odd_root(&second, &square.map, 3)?;
        let product = self.expression(Operator::Mul, &[left.clone(), right.clone()])?;
        let expected = self
            .frame
            .literal(self.cas, &data.product, &self.map, self.budget)?;
        self.injective(&product, &expected, 3)?;
        let sum = self.expression(Operator::Add, &[left, right])?;
        let shift = self
            .frame
            .literal(self.cas, &data.base.shift, &self.map, self.budget)?;
        Ok(self.expression(Operator::Sub, &[sum, shift])?)
    }

    /// Count one plus original membership excludes every further Real root of the actual cubic.
    pub(crate) fn single(
        &mut self,
        data: &Cubic,
        linear: usize,
    ) -> Result<Handle, CoefficientFailure> {
        self.mark("real-one-root")?;
        let discriminant = self.coefficient_sign(&data.resolvent.radicand)?;
        let mut premises = vector(1, self.budget)?;
        if linear == 1 {
            premises.push(self.nonzero_constant(data, &discriminant)?);
        }
        let root = self.radical_sum(data, &discriminant)?;
        let source = if linear == 1 {
            super::derive::regional_source(self.cas, self.frame, &data.base, self.budget)?
        } else {
            data.base.source.copy(&self.cas.state, self.budget)?
        };
        let mut certificate = self.count_data(&source, &premises)?;
        self.whole_line(&mut certificate, 1)?;
        self.counted_coverage(certificate, &source, &data.base.scale, &[root])
    }
}
