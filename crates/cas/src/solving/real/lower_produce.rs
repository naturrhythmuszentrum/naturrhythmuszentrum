// SPDX-License-Identifier: AGPL-3.0-only
//! Quadratic and repeated cubic strata retain exact factor multiplicities.

use super::{
    derive::{Cubic, Translated},
    leaf::Leaf,
};
use crate::graph::Handle;
use crate::model::Operator;
use crate::solving::coefficients::{
    arithmetic::Engine, model::CoefficientFailure, storage::invalid,
};
use crate::solving::{RealSignConclusion as Sign, RealSignRule as Rule};

impl Leaf<'_> {
    /// Generic quadratic translation leaves one signed radicand and the actual original scale.
    pub(crate) fn quadratic(
        &mut self,
        data: &Translated,
        sign: usize,
    ) -> Result<Handle, CoefficientFailure> {
        self.mark("real-low-degree")?;
        let shift = self
            .frame
            .literal(self.cas, &data.shift, &self.map, self.budget)?;
        match sign {
            0 => {
                let mut count = self.count_data(&data.source, &[])?;
                self.whole_line(&mut count, 0)?;
                self.counted_coverage(count, &data.source, &data.scale, &[])
            }
            1 => {
                let root = self.expression(Operator::Neg, &[shift])?;
                self.product(&data.scale, &[(root, 2)])
            }
            2 => {
                let positive = self.coefficient_sign(&data.radicand)?;
                let (square, _) = self.square_root(&data.radicand, &positive)?;
                let first =
                    self.expression(Operator::Sub, &[square.root.clone(), shift.clone()])?;
                let negative = self.expression(Operator::Neg, &[square.root])?;
                let second = self.expression(Operator::Sub, &[negative, shift])?;
                self.product(&data.scale, &[(first, 1), (second, 1)])
            }
            _ => Err(invalid().into()),
        }
    }

    /// On D=0 the nontriple root parameter is the derived cube value divided by its nonzero square value.
    pub(crate) fn repeated(
        &mut self,
        data: &Cubic,
        triple: bool,
    ) -> Result<Handle, CoefficientFailure> {
        self.mark("real-low-degree")?;
        let shift = self
            .frame
            .literal(self.cas, &data.base.shift, &self.map, self.budget)?;
        if triple {
            self.triple_zero(data)?;
            let root = self.expression(Operator::Neg, &[shift])?;
            return self.product(&data.base.scale, &[(root, 3)]);
        }
        let engine = Engine {
            state: &self.cas.state,
            domain: &data.base.source.domain,
        };
        let scope = crate::solving::coefficients::model::EvaluationScope {
            context: &self.frame.context,
            region: &self.frame.region,
            premises: &[],
            localization: &[],
        };
        let inverse = super::derive::inverse(&engine, &data.product, &scope, self.budget)?;
        let center = engine.neg(&data.resolvent.shift, self.budget)?;
        let root = engine.mul(&center, &inverse, self.budget)?;
        let root = self
            .frame
            .literal(self.cas, &root, &self.map, self.budget)?;
        let two = self.number(2, 1)?;
        let doubled = self.expression(Operator::Mul, &[two, root.clone()])?;
        let first = self.expression(Operator::Sub, &[doubled, shift.clone()])?;
        let negative = self.expression(Operator::Neg, &[root])?;
        let second = self.expression(Operator::Sub, &[negative, shift])?;
        self.product(&data.base.scale, &[(first, 1), (second, 2)])
    }

    /// A vanishing sum of the derived square and cube forces q=0 when p=0.
    fn triple_zero(&mut self, data: &Cubic) -> Result<(), CoefficientFailure> {
        let discriminant = self.coefficient_sign(&data.resolvent.radicand)?;
        let product = self.coefficient_sign(&data.product)?;
        let cube = self.power_sign(&product, 3)?;
        let sum = self.expression(
            Operator::Add,
            &[discriminant.subject.clone(), cube.subject.clone()],
        )?;
        let zero = self.derive(
            Rule::Sum,
            &sum,
            Sign::Zero,
            &[&discriminant, &cube],
            Vec::new(),
            0,
        )?;
        let half = self
            .frame
            .literal(self.cas, &data.resolvent.shift, &self.map, self.budget)?;
        let square = self.expression(Operator::IntegerPower(2), std::slice::from_ref(&half))?;
        let square = self.transport(&zero, &square)?;
        let half = self.derive(
            Rule::EvenPower,
            &half,
            Sign::Zero,
            &[&square],
            Vec::new(),
            2,
        )?;
        let two = self.integer_sign(2)?;
        let doubled = self.product_sign(&two, &half, false)?;
        let constant = self
            .frame
            .literal(self.cas, &data.constant, &self.map, self.budget)?;
        let constant = self.transport(&doubled, &constant)?;
        let numerator = self.remember(&data.constant, &constant)?;
        self.zero_equality(&numerator)?;
        Ok(())
    }
}
