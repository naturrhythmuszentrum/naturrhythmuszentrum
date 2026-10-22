// SPDX-License-Identifier: AGPL-3.0-only
//! The three-root cubic derives critical endpoint signs and selects the three count-one Real cells.

use super::{
    derive::Cubic,
    leaf::{Leaf, Signed},
};
use crate::graph::{Handle, Reference};
use crate::model::Operator;
use crate::proof_model::VerifiedWitness;
use crate::solving::coefficients::{
    arithmetic::Engine,
    model::CoefficientFailure,
    storage::{invalid, reserve},
};
use crate::solving::{RealEndpoint, RealSignConclusion as Sign, RealSignRule as Rule};

/// Every critical quantity carries its own independently replayed sign recipe.
struct Critical {
    /// The positive critical square root.
    positive: Signed,
    /// Positive original cubic value at the left critical point.
    left_value: Signed,
    /// Positive negation of the cubic value at the right critical point.
    right_magnitude: Signed,
    /// Sealed local nonvanishing used only by exact Euclidean production.
    nonzero_linear: VerifiedWitness,
}

impl Leaf<'_> {
    /// Literal sign recipes avoid nested mutable borrows of the shared arithmetic ledger.
    pub(crate) fn integer_sign(&mut self, number: i64) -> Result<Signed, CoefficientFailure> {
        let value = crate::number::Rational::from_ratio(number, 1, self.budget)?;
        self.rational_sign(&value)
    }

    /// Negation is a finite ordered-field step on the exact earlier subject.
    pub(crate) fn negative_sign(&mut self, source: &Signed) -> Result<Signed, CoefficientFailure> {
        let target = self.expression(Operator::Neg, std::slice::from_ref(&source.subject))?;
        let sign = match source.sign {
            Sign::Negative => Sign::Positive,
            Sign::Positive => Sign::Negative,
            Sign::Zero => Sign::Zero,
            Sign::NonNegative => Sign::NonPositive,
            Sign::NonPositive => Sign::NonNegative,
            Sign::NonZero => Sign::NonZero,
        };
        Ok(self.derive(Rule::Negation, &target, sign, &[source], Vec::new(), 0)?)
    }

    /// D<0 and a nonnegative square force the odd cube negative, hence the actual critical radicand positive.
    fn critical_radicand(
        &mut self,
        data: &Cubic,
        discriminant: &Signed,
    ) -> Result<Signed, CoefficientFailure> {
        let half = self
            .frame
            .literal(self.cas, &data.resolvent.shift, &self.map, self.budget)?;
        let square = self.expression(Operator::IntegerPower(2), &[half])?;
        let square = self.derive(
            Rule::EvenPower,
            &square,
            Sign::NonNegative,
            &[],
            Vec::new(),
            2,
        )?;
        let engine = Engine {
            state: &self.cas.state,
            domain: &data.product.domain,
        };
        let ratio = engine.neg(&data.product, self.budget)?;
        let ratio = self
            .frame
            .literal(self.cas, &ratio, &self.map, self.budget)?;
        let cube = self.expression(Operator::IntegerPower(3), std::slice::from_ref(&ratio))?;
        let sum = self.expression(Operator::Add, &[square.subject.clone(), cube.clone()])?;
        let negative = self.transport(discriminant, &sum)?;
        let cube = self.derive(
            Rule::Sum,
            &cube,
            Sign::Negative,
            &[&negative, &square],
            Vec::new(),
            0,
        )?;
        let ratio = self.derive(
            Rule::OddPowerReflection,
            &ratio,
            Sign::Negative,
            &[&cube],
            Vec::new(),
            3,
        )?;
        let positive = self.negative_sign(&ratio)?;
        let target = self
            .frame
            .literal(self.cas, &data.product, &self.map, self.budget)?;
        Ok(self.transport(&positive, &target)?)
    }

    /// Both critical values follow from their positive sum and positive product, without splitting the sign of q.
    fn critical_values(
        &mut self,
        data: &Cubic,
        root: &Signed,
        discriminant: &Signed,
    ) -> Result<(Signed, Signed), CoefficientFailure> {
        let cube = self.power_sign(root, 3)?;
        let two = self.integer_sign(2)?;
        let twice = self.product_sign(&two, &cube, false)?;
        let constant = self
            .frame
            .literal(self.cas, &data.constant, &self.map, self.budget)?;
        let left = self.expression(Operator::Add, &[constant.clone(), twice.subject.clone()])?;
        let right = self.expression(Operator::Sub, &[twice.subject.clone(), constant])?;
        let sum = self.expression(Operator::Add, &[left.clone(), right.clone()])?;
        let product = self.expression(Operator::Mul, &[left.clone(), right.clone()])?;
        let sum_positive = self.product_sign(&two, &twice, false)?;
        let sum_positive = self.transport(&sum_positive, &sum)?;
        let negative = self.negative_sign(discriminant)?;
        let four = self.integer_sign(4)?;
        let product_positive = self.product_sign(&four, &negative, false)?;
        let product_positive = self.transport(&product_positive, &product)?;
        let first = self.derive(
            Rule::Sum,
            &left,
            Sign::Positive,
            &[&sum_positive, &product_positive],
            Vec::new(),
            0,
        )?;
        let second = self.derive(
            Rule::Sum,
            &right,
            Sign::Positive,
            &[&sum_positive, &product_positive],
            Vec::new(),
            0,
        )?;
        Ok((first, second))
    }

    /// The derived nonzero linear coefficient feeds the unchanged exact Euclidean algorithm through a sealed local fact.
    fn critical(&mut self, data: &Cubic) -> Result<Critical, CoefficientFailure> {
        self.mark("real-three-root")?;
        let discriminant = self.coefficient_sign(&data.resolvent.radicand)?;
        let radicand = self.critical_radicand(data, &discriminant)?;
        let negative_three = self.integer_sign(-3)?;
        let linear = self.product_sign(&negative_three, &radicand, false)?;
        let actual = self
            .frame
            .literal(self.cas, &data.linear, &self.map, self.budget)?;
        let linear = self.transport(&linear, &actual)?;
        let numerator = self.remember(&data.linear, &linear)?;
        let nonzero_linear = self.sealed_nonzero(&numerator)?;
        self.mark("real-critical-root")?;
        let (_, positive) = self.square_root(&data.product, &radicand)?;
        self.mark("real-critical-signs")?;
        let (left_value, right_magnitude) = self.critical_values(data, &positive, &discriminant)?;
        Ok(Critical {
            positive,
            left_value,
            right_magnitude,
            nonzero_linear,
        })
    }

    /// Exact critical-point signs populate each mandatory row of the generic Sturm certificate.
    fn endpoint_rows(
        &mut self,
        critical: &Critical,
        constant: u32,
    ) -> Result<([u32; 5], [u32; 5], u32), CoefficientFailure> {
        let zero = self.integer_sign(0)?;
        let six = self.integer_sign(6)?;
        let gradient = self.product_sign(&six, &critical.positive, false)?;
        let negative_gradient = self.negative_sign(&gradient)?;
        let negative_left = self.negative_sign(&critical.left_value)?;
        let negative_right = self.negative_sign(&critical.right_magnitude)?;
        let two = self.integer_sign(2)?;
        let order = self.product_sign(&two, &critical.positive, false)?;
        Ok((
            [
                critical.left_value.slot,
                zero.slot,
                negative_gradient.slot,
                negative_left.slot,
                constant,
            ],
            [
                negative_right.slot,
                zero.slot,
                gradient.slot,
                critical.right_magnitude.slot,
                constant,
            ],
            order.slot,
        ))
    }

    /// Three disjoint critical cells contain exactly one root each, and both exterior counts are zero.
    pub(crate) fn three(&mut self, data: &Cubic) -> Result<Handle, CoefficientFailure> {
        let critical = self.critical(data)?;
        let mut certificate = self.count_data(
            &data.base.source,
            std::slice::from_ref(&critical.nonzero_linear),
        )?;
        if certificate.chain.len() != 4 {
            return Err(invalid().into());
        }
        let constant = *certificate.signs.get(3).ok_or_else(invalid)?;
        let (left_row, right_row, order) = self.endpoint_rows(&critical, constant)?;
        reserve(&mut certificate.signs, 21, self.budget)?;
        certificate.signs.extend(left_row);
        certificate.signs.push(order);
        certificate.signs.extend(left_row);
        certificate.signs.extend(right_row);
        certificate.signs.extend(right_row);
        let shift = self
            .frame
            .literal(self.cas, &data.base.shift, &self.map, self.budget)?;
        let negative = self.expression(
            Operator::Neg,
            std::slice::from_ref(&critical.positive.subject),
        )?;
        let left = self.expression(Operator::Sub, &[negative, shift.clone()])?;
        let right = self.expression(Operator::Sub, &[critical.positive.subject, shift])?;
        certificate.cells = vec![
            self.cell(
                RealEndpoint::NegativeInfinity,
                RealEndpoint::RealExpression(Reference::Existing(left.clone())),
            )?,
            self.cell(
                RealEndpoint::RealExpression(Reference::Existing(left)),
                RealEndpoint::RealExpression(Reference::Existing(right.clone())),
            )?,
            self.cell(
                RealEndpoint::RealExpression(Reference::Existing(right)),
                RealEndpoint::PositiveInfinity,
            )?,
        ];
        certificate.counts = vec![1, 1, 1];
        let count = self.count_claim(&certificate)?;
        let roots = self.selected_cells(&certificate, count)?;
        self.counted_coverage(certificate, &data.base.source, &data.base.scale, &roots)
    }
}
